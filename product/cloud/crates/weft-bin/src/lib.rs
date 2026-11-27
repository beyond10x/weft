//! The control host (decision A3): one process serving the generated HTTP surfaces of `weft-api`
//! and `weft-control` over one [`weft_impl::System`].
//!
//! The accept loop is this crate's, not the generated `serve`, which takes the system for good:
//! the controllers and the node-channel endpoint of later stories share it. One writer: the
//! system sits behind one lock and commands run one at a time. Each request is answered by the
//! `dispatch_with_events` of the module whose route table holds its path, then logged with the
//! events it published ([`weft_impl::log`]).

use std::collections::HashMap;
use std::io::BufReader;
use std::net::TcpListener;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use serde_json::Value;
use weft_impl::context::{CallerAttributes, Clock};
use weft_impl::log::Log;
use weft_impl::{Ports, System};
use weft_server::http::{self, Request, Response};
use weft_server::{weft_api, weft_control};
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
    ports: Ports,
    log: Log,
    operations: HashMap<String, String>,
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
        let log = Log::open(log)
            .map_err(|error| Error(format!("control host log {}: {error}", log.display())))?;
        Ok(Self {
            system: Mutex::new(weft_impl::system(&ports)),
            ports,
            log,
            operations: operations(),
        })
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
    /// it: the request line, then one line per event it published.
    pub fn handle(&self, request: &Request, trust_callers: bool) -> Response {
        let (caller, attributes) = authenticate(request, trust_callers);
        let mut system = self.system();
        self.ports.set_caller(attributes);
        let (answer, events) = if weft_api::ROUTES
            .iter()
            .any(|(_, path)| *path == request.path)
        {
            weft_api::dispatch_with_events(&mut *system, caller.as_ref(), request)
        } else {
            weft_control::dispatch_with_events(&mut *system, caller.as_ref(), request)
        };
        self.ports.set_caller(CallerAttributes::default());
        self.log.request(
            self.operations.get(&request.path).map(String::as_str),
            caller.map(|caller| caller.actor.name()),
            answer.status,
        );
        for event in &events {
            self.log.event(event.name());
        }
        answer
    }

    /// Serves `listener` one connection at a time, until the listener fails for good.
    pub fn serve(&self, listener: &TcpListener, trust_callers: bool) {
        for connection in listener.incoming() {
            let Ok(connection) = connection else {
                std::thread::sleep(std::time::Duration::from_millis(10));
                continue;
            };
            let _ = connection.set_read_timeout(Some(http::READ_TIMEOUT));
            let _ = connection.set_write_timeout(Some(http::WRITE_TIMEOUT));
            let mut reader = BufReader::new(connection);
            let (answer, refused) = match http::read(&mut reader) {
                Ok(request) => (self.handle(&request, trust_callers), false),
                Err(refusal) => (refusal, true),
            };
            let mut stream = reader.into_inner();
            if http::write(&mut stream, &answer).is_ok() && refused {
                http::linger(&mut stream);
            }
        }
    }
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
