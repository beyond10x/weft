//! The control host (decision A3): one process serving the generated HTTP surfaces of `weft-api`
//! and `weft-control` over one [`weft_impl::System`].
//!
//! The accept loop is this crate's, not the generated `serve`, which takes the system for good:
//! the controllers and the node-channel endpoint of later stories share it. One writer: the
//! state file is locked for the life of the host, the system sits behind one lock and commands
//! run one at a time. Each request is answered by the `dispatch_with_events` of the module whose
//! route table holds its path, and logged with the events it published ([`weft_impl::log`]) in
//! the same transaction. `/openapi.json` and `/docs` are answered by the host ([`contract`]).
//!
//! The bindings the generated system does not deliver ([`weft_impl::bindings`]) run at the end
//! of every unit of work, over what its commands published, in the same transaction: each
//! attempt is logged as a binding line, then the lines of the events it published. The
//! controllers ([`weft_controllers`]) run in [`Host::tick`], which first attempts again what the
//! bindings hold.

pub mod contract;

use std::collections::HashMap;
use std::io::BufReader;
use std::net::{SocketAddr, TcpListener, ToSocketAddrs};
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use contract::Contract;
use serde_json::Value;
use weft_impl::bindings::{Answer, Runner};
use weft_impl::context::{CallerAttributes, Clock};
use weft_impl::log::{BindingResult, Log};
use weft_impl::{Ports, System};
use weft_server::http::{self, Request, Response};
use weft_server::{weft_api, weft_control};
use weft_system::SystemEvent;
use weft_types::actor::{Actor, Caller};

/// The request header that names the caller: `{"actor": "<qualified actor>", "caller": {...}}`.
/// Test-only: read only where [`authenticate`] is told to trust it.
pub const CALLER_HEADER: &str = "x-weft-caller";

/// Why a control host could not open.
#[derive(Debug)]
pub struct Error(String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

/// The system, its ports and its log.
pub struct Host {
    system: Mutex<System>,
    /// Used only while `system` is locked.
    bindings: Runner,
    ports: Ports,
    log: Log,
    operations: HashMap<String, String>,
    contract: Contract,
    notes: Vec<String>,
}

impl Host {
    /// A control host over the state file `state` with the log `log`, on the system clock.
    ///
    /// # Errors
    ///
    /// What opening the state file or the log refuses.
    pub fn open(state: &Path, log: &Path) -> Result<Self, Error> {
        Self::open_with_clock(state, log, Clock::system())
    }

    /// As [`Host::open`], with the clock `clock`.
    ///
    /// # Errors
    ///
    /// As [`Host::open`].
    pub fn open_with_clock(state: &Path, log: &Path, clock: Clock) -> Result<Self, Error> {
        let ports =
            Ports::open_with_clock(state, clock).map_err(|error| Error(error.to_string()))?;
        let (log, notes) = Log::open(log, &ports)
            .map_err(|error| Error(format!("control host log {}: {error}", log.display())))?;
        Ok(Self {
            system: Mutex::new(weft_impl::system(&ports)),
            bindings: Runner::open(&ports),
            ports,
            log,
            operations: operations(),
            contract: Contract::new().map_err(Error)?,
            notes,
        })
    }

    /// What opening repaired, one note each (a cut-off log line dropped, lines the log file
    /// lacked appended from the state file).
    pub fn notes(&self) -> &[String] {
        &self.notes
    }

    /// The system, for a caller in this process that runs it under the same lock.
    pub fn system(&self) -> MutexGuard<'_, System> {
        self.system
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// The log.
    pub fn log(&self) -> &Log {
        &self.log
    }

    /// Answers one request, as the caller [`authenticate`] finds with `trust_callers`, and logs
    /// it: the request line, then one line per event it published. The rows the request writes
    /// and its log lines commit in one transaction of the state file; the log file gets the
    /// lines after the commit.
    pub fn handle(&self, request: &Request, trust_callers: bool) -> Response {
        let (caller, attributes) = authenticate(request, trust_callers);
        let actor = caller.as_ref().map(|caller| caller.actor.name());
        if let Some(answer) = self.contract.answer(request) {
            self.log.request(None, actor, answer.status);
            return answer;
        }
        let operation = self.operations.get(&request.path).map(String::as_str);
        self.execute(operation, actor, |system| {
            self.ports.set_caller(attributes);
            let answer = if weft_api::ROUTES
                .iter()
                .any(|(_, path)| *path == request.path)
            {
                weft_api::dispatch_with_events(system, caller.as_ref(), request)
            } else {
                weft_control::dispatch_with_events(system, caller.as_ref(), request)
            };
            self.ports.set_caller(CallerAttributes::default());
            answer
        })
    }

    /// Runs `work` on the system as the request of `operation` sent by `actor`, in one
    /// transaction of the state file with its log lines: the request line, then one line per
    /// event `work` answers. The log file gets the lines after the commit.
    ///
    /// An answer that says `committed: false` (the `501` of an unmet obligation: "nothing was
    /// written") commits nothing: what `work` wrote, and any line written during it, is rolled
    /// back. Its request line is still logged, as every answered request is; it has no event
    /// lines.
    pub fn execute(
        &self,
        operation: Option<&str>,
        actor: Option<&str>,
        work: impl FnOnce(&mut System) -> (Response, Vec<SystemEvent>),
    ) -> Response {
        self.unit(|unit| unit.run(operation, actor, work))
    }

    /// Runs `work`, one unit of several commands ([`Unit::run`]), in one transaction of the state
    /// file with the log lines of every command, in their order. What the commands published is
    /// then delivered to the hand-run bindings ([`weft_impl::bindings`]) in the same transaction,
    /// each attempt logged as a binding line and the lines of what it published. The unit commits
    /// nothing when a command answers `committed: false` or `work` calls [`Unit::abort`]: then
    /// every command's request line is still logged, without event lines, no binding runs, and
    /// the bindings hold what they held before the unit (the events they hold are rows of the
    /// same transaction).
    pub fn unit<T>(&self, work: impl FnOnce(&mut Unit<'_>) -> T) -> T {
        let mut system = self.system();
        let (value, steps, committed) = self.ports.store().transaction(|| {
            let mut unit = Unit {
                system: &mut system,
                runner: &self.bindings,
                steps: Vec::new(),
                pending: Vec::new(),
                abort: false,
            };
            let value = work(&mut unit);
            if !unit.abort {
                unit.deliver();
            }
            let Unit { steps, abort, .. } = unit;
            if abort {
                self.log.discard();
                return ((value, steps, false), false);
            }
            for step in &steps {
                match step {
                    Step::Request {
                        operation,
                        actor,
                        status,
                        ..
                    } => {
                        self.log
                            .request(operation.as_deref(), actor.as_deref(), *status);
                    }
                    Step::Binding {
                        binding,
                        refusal,
                        result,
                        attempt,
                        ..
                    } => {
                        self.log
                            .binding(binding, refusal.as_deref(), *result, *attempt);
                    }
                }
                for event in step.events() {
                    self.log.event(event);
                }
            }
            ((value, steps, true), true)
        });
        if !committed {
            for step in &steps {
                if let Step::Request {
                    operation,
                    actor,
                    status,
                    ..
                } = step
                {
                    self.log
                        .request(operation.as_deref(), actor.as_deref(), *status);
                }
            }
        }
        self.log.flush();
        value
    }

    /// One tick of the controllers, as one unit: first the events the bindings hold are
    /// attempted again ([`Runner::retry`]), then every step of [`weft_controllers::tick`] sends
    /// its commands, given the events still held, each logged as a request line, and what they
    /// published goes to the bindings.
    pub fn tick(&self) {
        self.unit(|unit| {
            unit.retry();
            let held = unit.runner.held_events();
            for sent in weft_controllers::tick(unit.system, &held) {
                unit.run(Some(sent.operation), Some(sent.actor.name()), |_| {
                    (sent.answer, sent.events)
                });
            }
        });
    }

    /// Delivers `events` to the hand-run bindings as one unit, as if a command had published
    /// them: a repeated delivery, for a test. The events themselves are not logged; each
    /// binding's attempt is.
    pub fn replay(&self, events: Vec<SystemEvent>) {
        self.unit(|unit| unit.pending.extend(events));
    }

    /// How many events the hand-run bindings hold for another attempt.
    pub fn held(&self) -> usize {
        let _system = self.system();
        self.bindings.held()
    }

    /// The ports of the system.
    pub fn ports(&self) -> &Ports {
        &self.ports
    }

    /// Serves `listener` one connection at a time, until the listener fails for good: each
    /// request is answered by [`Host::handle`] with `trust_callers`.
    pub fn serve(&self, listener: &TcpListener, trust_callers: bool) {
        serve(listener, |request| self.handle(request, trust_callers));
    }
}

/// The commands of one unit of work of [`Host::unit`].
pub struct Unit<'a> {
    system: &'a mut System,
    runner: &'a Runner,
    steps: Vec<Step>,
    /// What the unit's commands published that no binding was given yet.
    pending: Vec<SystemEvent>,
    abort: bool,
}

/// One command of a unit, or one attempt of a hand-run binding, as its log lines record it.
enum Step {
    Request {
        operation: Option<String>,
        actor: Option<String>,
        status: u16,
        events: Vec<String>,
    },
    Binding {
        binding: &'static str,
        refusal: Option<String>,
        result: BindingResult,
        attempt: u32,
        events: Vec<String>,
    },
}

impl Step {
    /// The names of the events the step published.
    fn events(&self) -> &[String] {
        match self {
            Self::Request { events, .. } | Self::Binding { events, .. } => events,
        }
    }
}

/// The names of `events`.
fn names(events: &[SystemEvent]) -> Vec<String> {
    events
        .iter()
        .map(|event| event.name().to_string())
        .collect()
}

impl Unit<'_> {
    /// Gives the hand-run bindings what the unit's commands published so far.
    fn deliver(&mut self) {
        let events = std::mem::take(&mut self.pending);
        let answers = self.runner.deliver(self.system, events);
        self.record(answers);
    }

    /// Attempts again what the hand-run bindings hold.
    fn retry(&mut self) {
        let answers = self.runner.retry(self.system);
        self.record(answers);
    }

    fn record(&mut self, answers: Vec<Answer>) {
        for answer in answers {
            self.steps.push(Step::Binding {
                binding: answer.binding,
                refusal: answer.refusal,
                result: answer.result,
                attempt: answer.attempt,
                events: names(&answer.events),
            });
        }
    }

    /// Runs `work` on the system as the request of `operation` sent by `actor`: its answer. An
    /// answer that says `committed: false` makes the whole unit commit nothing.
    pub fn run(
        &mut self,
        operation: Option<&str>,
        actor: Option<&str>,
        work: impl FnOnce(&mut System) -> (Response, Vec<SystemEvent>),
    ) -> Response {
        let (answer, events) = work(self.system);
        if uncommitted(&answer) {
            self.abort = true;
        }
        self.steps.push(Step::Request {
            operation: operation.map(str::to_owned),
            actor: actor.map(str::to_owned),
            status: answer.status,
            events: names(&events),
        });
        self.pending.extend(events);
        answer
    }

    /// Makes the unit commit nothing: what its commands wrote is rolled back.
    pub fn abort(&mut self) {
        self.abort = true;
    }
}

/// Serves `listener` one connection at a time, until the listener fails for good: each request
/// is answered by `answer`, the router of a binary that serves more than [`Host::handle`] on the
/// same listener (the node channel of `weft`).
pub fn serve(listener: &TcpListener, answer: impl Fn(&Request) -> Response) {
    for connection in listener.incoming() {
        let Ok(connection) = connection else {
            std::thread::sleep(std::time::Duration::from_millis(10));
            continue;
        };
        let _ = connection.set_read_timeout(Some(http::READ_TIMEOUT));
        let _ = connection.set_write_timeout(Some(http::WRITE_TIMEOUT));
        let mut reader = BufReader::new(connection);
        let (answer, refused) = match http::read(&mut reader) {
            Ok(request) => (answer(&request), false),
            Err(refusal) => (refusal, true),
        };
        let mut stream = reader.into_inner();
        if http::write(&mut stream, &answer).is_ok() && refused {
            http::linger(&mut stream);
        }
    }
}

/// Whether `answer` says its command committed nothing: a JSON body whose `committed` member is
/// `false`.
fn uncommitted(answer: &Response) -> bool {
    answer.body.contains("\"committed\"")
        && serde_json::from_str::<Value>(&answer.body)
            .is_ok_and(|body| body["committed"] == Value::Bool(false))
}

/// The addresses `--listen <listen> --port <port>` names. Where the build trusts the caller
/// header (`trust_callers`), anyone who reaches the port may name any actor, so every address
/// must be loopback.
///
/// # Errors
///
/// An address that does not resolve, or with `trust_callers` one that is not loopback.
pub fn listen_addresses(
    listen: &str,
    port: u16,
    trust_callers: bool,
) -> Result<Vec<SocketAddr>, String> {
    let addresses: Vec<SocketAddr> = (listen, port)
        .to_socket_addrs()
        .map_err(|error| format!("--listen {listen}: {error}"))?
        .collect();
    if addresses.is_empty() {
        return Err(format!("--listen {listen} names no address"));
    }
    if trust_callers {
        if let Some(address) = addresses.iter().find(|address| !address.ip().is_loopback()) {
            return Err(format!(
                "--listen {listen}: this build trusts the {CALLER_HEADER} header (feature test-callers), so it listens on loopback only, not on {}",
                address.ip()
            ));
        }
    }
    Ok(addresses)
}

/// Who sent `request`: with `trust_callers`, the actor and attributes its `X-Weft-Caller` header
/// names; otherwise, and for a header that names no declared actor, no caller, which every
/// command answers `403` `not granted`.
pub fn authenticate(request: &Request, trust_callers: bool) -> (Option<Caller>, CallerAttributes) {
    let none = (None, CallerAttributes::default());
    if !trust_callers {
        return none;
    }
    let Some((_, header)) = request
        .headers
        .iter()
        .find(|(name, _)| name == CALLER_HEADER)
    else {
        return none;
    };
    let Ok(value) = serde_json::from_str::<Value>(header) else {
        return none;
    };
    let Some(actor) = Actor::ALL
        .iter()
        .copied()
        .find(|actor| value["actor"].as_str() == Some(actor.name()))
    else {
        return none;
    };
    let attribute = |name: &str| value["caller"][name].as_str().map(str::to_owned);
    (
        Some(Caller { actor }),
        CallerAttributes {
            tenant_id: attribute("tenant_id"),
            account_id: attribute("account_id"),
            principal_id: attribute("principal_id"),
        },
    )
}

/// The qualified name of the command or view each path serves, read from the startup records
/// of both surfaces.
fn operations() -> HashMap<String, String> {
    let mut names = HashMap::new();
    for line in weft_api::STARTUP.iter().chain(weft_control::STARTUP) {
        let Ok(record) = serde_json::from_str::<Value>(&format!("{line}}}")) else {
            continue;
        };
        for path in record["paths"].as_array().into_iter().flatten() {
            if let (Some(path), Some(name), Some("command" | "view")) = (
                path["path"].as_str(),
                path["name"].as_str(),
                path["serves"].as_str(),
            ) {
                names.insert(path.to_owned(), name.to_owned());
            }
        }
    }
    names
}
