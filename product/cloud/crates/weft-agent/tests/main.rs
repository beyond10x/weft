//! The one integration-test binary of weft-agent.

/// `confine`: one process of an instance runs as one exec of the `substrate-daemon` the setup hook
/// `tools/test-setup.d/20-substrate.sh` started for this test run; its socket path comes from the
/// environment (`WEFT_SUBSTRATE_SOCKET`).
mod confine {
    use std::io::ErrorKind;
    use std::net::{SocketAddr, TcpListener};
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use weft_agent::confine::{Confine, ExecState, HostFile, Process};

    const SOCKET_VARIABLE: &str = "WEFT_SUBSTRATE_SOCKET";
    const PROBE: &str = env!("CARGO_BIN_EXE_weft-probe");
    const PROBE_MOUNT: &str = "/opt/weft/weft-probe";

    fn socket() -> PathBuf {
        let path = std::env::var_os(SOCKET_VARIABLE).unwrap_or_else(|| {
            panic!("{SOCKET_VARIABLE} is unset: the setup hook tools/test-setup.d/20-substrate.sh starts the daemon")
        });
        let path = PathBuf::from(path);
        assert!(
            path.exists(),
            "{SOCKET_VARIABLE}={} does not exist: no substrate-daemon serves it",
            path.display()
        );
        path
    }

    fn block_on<F: std::future::Future>(future: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("a tokio runtime builds")
            .block_on(future)
    }

    /// Accepts every connection waiting on a non-blocking listener and returns how many there were.
    fn accept_waiting(listener: &TcpListener) -> usize {
        let mut accepted = 0;
        loop {
            match listener.accept() {
                Ok(_) => accepted += 1,
                Err(error) if error.kind() == ErrorKind::WouldBlock => return accepted,
                Err(error) => panic!("accept on the probe listener failed: {error}"),
            }
        }
    }

    fn run(process: &Process) -> (Vec<ExecState>, weft_agent::confine::Exited) {
        let mut states = Vec::new();
        let exited = block_on(async {
            let confine = Confine::connect(&socket())
                .await
                .expect("the daemon answers");
            confine.run(process, |state| states.push(state)).await
        })
        .expect("the exec runs to exec.exited");
        (states, exited)
    }

    #[test]
    fn a_snippet_runs_confined_and_its_exit_and_output_are_observed() {
        let (states, exited) = run(&Process::new(["sh", "-c", "echo weft"]));
        assert_eq!(
            states.first(),
            Some(&ExecState::Accepted),
            "states: {states:?}"
        );
        assert_eq!(
            states.last(),
            Some(&ExecState::Exited),
            "states: {states:?}"
        );
        assert!(
            states.len() >= 2,
            "the exec never left accepted: {states:?}"
        );
        assert_eq!(
            exited.code,
            Some(0),
            "stderr: {}",
            String::from_utf8_lossy(&exited.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&exited.stdout), "weft\n");
    }

    #[test]
    fn the_probe_reaches_host_loopback_unconfined_and_not_from_the_sandbox() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback listener binds");
        listener
            .set_nonblocking(true)
            .expect("the listener turns non-blocking");
        let address: SocketAddr = listener.local_addr().expect("the listener has an address");
        let target = address.to_string();

        // Positive control: the same binary and arguments, unconfined, reach the listener.
        let unconfined = Command::new(PROBE)
            .arg(&target)
            .output()
            .expect("the probe starts unconfined");
        let unconfined_report = String::from_utf8_lossy(&unconfined.stdout).into_owned();
        assert!(
            unconfined_report.starts_with("connected "),
            "the probe cannot connect even unconfined, so it proves no confinement: {unconfined_report:?} {:?}",
            String::from_utf8_lossy(&unconfined.stderr)
        );
        assert_eq!(
            accept_waiting(&listener),
            1,
            "the unconfined probe made one connection"
        );

        // The confined run: the same binary, bound read-only into the exec, the same arguments.
        let probe = Path::new(PROBE)
            .canonicalize()
            .expect("the probe binary has a canonical path");
        let process =
            Process::new([PROBE_MOUNT, target.as_str()]).file(HostFile::new(probe, PROBE_MOUNT));
        let (states, exited) = run(&process);
        let confined_report = String::from_utf8_lossy(&exited.stdout).into_owned();
        assert_eq!(
            states.last(),
            Some(&ExecState::Exited),
            "states: {states:?}"
        );
        assert!(
            confined_report.starts_with("refused ") || confined_report.starts_with("unreachable "),
            "the confined probe did not report refused or unreachable: {confined_report:?} (exit {:?}, stderr {:?})",
            exited.code,
            String::from_utf8_lossy(&exited.stderr)
        );
        assert_eq!(
            accept_waiting(&listener),
            0,
            "the sandbox reached the host loopback listener"
        );
    }

    /// Adversary: a node runs more than four processes at once. `Confine::run` holds one event
    /// stream of the daemon per run, and substrate 0.29.0 grants four streams per subject
    /// (`EventStreamPolicy::production`, `streams_per_subject: 4`): the fifth concurrent run is
    /// refused (`event.stream-capacity`) before its process starts.
    #[test]
    fn five_processes_run_at_once_on_one_daemon() {
        const RUNS: usize = 5;
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(RUNS));
        let threads: Vec<_> = (0..RUNS)
            .map(|n| {
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    let script = format!("sleep 2; echo run{n}");
                    let process = Process::new(["sh", "-c", script.as_str()]);
                    block_on(async {
                        let confine = Confine::connect(&socket()).await?;
                        barrier.wait();
                        confine.run(&process, |_| {}).await
                    })
                })
            })
            .collect();
        let ended: Vec<_> = threads
            .into_iter()
            .map(|thread| thread.join().expect("a run thread ends"))
            .collect();
        for (n, end) in ended.iter().enumerate() {
            match end {
                Ok(exited) => {
                    assert_eq!(exited.code, Some(0), "run {n}");
                    assert_eq!(String::from_utf8_lossy(&exited.stdout), format!("run{n}\n"));
                }
                Err(error) => panic!("run {n} of {RUNS} concurrent runs failed: {error}"),
            }
        }
    }

    /// Adversary: a node agent's daemon lives past its first thousand events. `Confine::run`
    /// opens the event stream from the start of the daemon's history (cursor `None`), and
    /// substrate 0.29.0 replays at most 16 pages of 64 events before it ends the stream with
    /// `event.catch-up-limit`; the history holds 10,000 events. Every run after the 1,024th event
    /// fails, though the daemon is healthy. The events here come from empty workspaces created and
    /// destroyed, the cheapest record the daemon writes; a run of processes writes the same kind.
    #[test]
    fn a_process_runs_on_a_daemon_with_more_than_a_thousand_events() {
        use b10x_substrate_sdk::Client;
        use std::time::{Duration, Instant};

        const PAST_CATCH_UP: u64 = 16 * 64 + 64;
        let filled = block_on(async {
            let client = Client::builder()
                .unix_socket(socket())
                .connect()
                .await
                .expect("the daemon answers");
            let started = Instant::now();
            loop {
                let page = client
                    .events(None, 1)
                    .await
                    .expect("the daemon lists its events");
                let first = page.first_retained_sequence.unwrap_or(1);
                let retained = page.through_sequence.saturating_sub(first) + 1;
                if retained >= PAST_CATCH_UP {
                    return retained;
                }
                assert!(
                    started.elapsed() < Duration::from_secs(40),
                    "the daemon recorded only {retained} events in 40 s, the case cannot build its state"
                );
                for _ in 0..16 {
                    let workspace = client
                        .workspace()
                        .empty()
                        .create()
                        .await
                        .expect("an empty workspace is created");
                    workspace
                        .destroy()
                        .await
                        .expect("the workspace is destroyed");
                }
            }
        });
        let ended = block_on(async {
            let confine = Confine::connect(&socket())
                .await
                .expect("the daemon answers");
            confine
                .run(&Process::new(["sh", "-c", "echo weft"]), |_| {})
                .await
        });
        match ended {
            Ok(exited) => {
                assert_eq!(exited.code, Some(0));
                assert_eq!(String::from_utf8_lossy(&exited.stdout), "weft\n");
            }
            Err(error) => panic!("the run after {filled} daemon events failed: {error}"),
        }
    }

    /// Adversary: the states `run` reports belong to its own exec. A second exec starts while the
    /// first runs, so the first run's event stream carries the second's `operation.accepted`
    /// (`exec.start`) and its exec events; none of them may enter the first run's states. The
    /// states of each run follow the lifecycle, each at most once: `accepted`, then `running`,
    /// then `exited`.
    #[test]
    fn the_states_of_a_run_are_its_own_exec_s_while_another_exec_starts() {
        fn rank(state: ExecState) -> u8 {
            match state {
                ExecState::Accepted => 0,
                ExecState::Running => 1,
                _ => 2,
            }
        }
        let long = std::thread::spawn(|| run(&Process::new(["sh", "-c", "sleep 2; echo long"])));
        std::thread::sleep(std::time::Duration::from_millis(700));
        let (short_states, short) = run(&Process::new(["sh", "-c", "echo short"]));
        let (long_states, long) = long.join().expect("the long run ends");
        assert_eq!(String::from_utf8_lossy(&long.stdout), "long\n");
        assert_eq!(String::from_utf8_lossy(&short.stdout), "short\n");
        for states in [&long_states, &short_states] {
            assert_eq!(
                states.first(),
                Some(&ExecState::Accepted),
                "states: {states:?}"
            );
            assert_eq!(
                states.last(),
                Some(&ExecState::Exited),
                "states: {states:?}"
            );
            assert!(
                states.windows(2).all(|pair| rank(pair[0]) < rank(pair[1])),
                "the states are out of lifecycle order, so another exec's event entered them: {states:?}"
            );
        }
        assert!(
            long_states.contains(&ExecState::Running),
            "the long run never reported running: {long_states:?}"
        );
    }
}

/// `join` (`story:skeleton-node-join-session`): `weft-agent` joins the node `ProvisionPoolNode`
/// made with its token, reports what it observed, and opens its session with a `hello` frame on
/// the node channel of the control host, which records it as `OpenNodeSession` sent by the
/// ControlPlane. Each test runs a `ControlHost` of its own.
mod join {
    use std::time::{Duration, Instant};

    use serde_json::{json, Value};
    use time::format_description::well_known::Rfc3339;
    use time::OffsetDateTime;
    use weft_client::{Client, Expect};
    use weft_testkit::steps::{self, Provisioned};
    use weft_testkit::{ControlHost, NodeAgent};

    /// How long the agent may take to its first `hello` answer.
    const HELLO_LIMIT: Duration = Duration::from_secs(30);

    /// A control host with one zone, one pool and one provisioned node.
    fn provisioned() -> (ControlHost, Provisioned) {
        let host = ControlHost::start();
        let client = host.client();
        let cloud_id = steps::create_cloud(&client);
        steps::declare_zone(&client, &cloud_id, "z1");
        let pool = steps::create_node_pool(&client);
        let node = steps::provision_pool_node(&client, &pool);
        (host, node)
    }

    /// The row of `view` whose `key` is `id`.
    fn row(client: &Client, view: &str, key: &str, id: &str) -> Value {
        let answer = client.view(view, &[]).ok_body(200);
        answer["rows"]
            .as_array()
            .expect("rows")
            .iter()
            .find(|row| row[key] == json!(id))
            .unwrap_or_else(|| panic!("no row with {key} {id} in {view}: {answer}"))
            .clone()
    }

    fn rows(client: &Client, view: &str) -> usize {
        client.view(view, &[]).ok_body(200)["rows"]
            .as_array()
            .expect("rows")
            .len()
    }

    fn instant(text: &str) -> OffsetDateTime {
        OffsetDateTime::parse(text, &Rfc3339)
            .unwrap_or_else(|error| panic!("{text} is not RFC 3339: {error}"))
    }

    fn hellos(lines: &[Value]) -> Vec<Value> {
        lines
            .iter()
            .filter(|line| line.get("hello").is_some())
            .cloned()
            .collect()
    }

    /// Statements 1 to 3: Provisioning before the agent, Ready after it; its first `hello` is
    /// opened, and the session row and the control host log record it.
    #[test]
    fn the_agent_joins_its_node_and_opens_its_session_on_the_first_hello() {
        let (host, node) = provisioned();
        let client = host.client();
        let nodes = "weft.infrastructure.Nodes";
        assert_eq!(
            row(&client, nodes, "node_id", &node.node_id)["state"],
            json!("Provisioning")
        );

        let before_seq = host.last_seq();
        let before = OffsetDateTime::now_utc();
        let mut agent = NodeAgent::start(&host, &node.node_id, &node.join_token);
        let lines = agent.wait_for(HELLO_LIMIT, |lines| !hellos(lines).is_empty());
        let after = OffsetDateTime::now_utc();

        // 1. The node reads Ready.
        let joined = row(&client, nodes, "node_id", &node.node_id);
        assert_eq!(joined["state"], json!("Ready"), "{joined}");
        assert!(
            joined["isolation_facts"]["levels"]
                .as_array()
                .is_some_and(|levels| levels.contains(&json!("Container"))),
            "{joined}"
        );

        // 2. The first hello is opened, and its session is Open, opened between the two reads.
        let first = &hellos(&lines)[0];
        assert_eq!(first["hello"], json!("opened"), "{lines:?}");
        let session_id = first["session_id"].as_str().expect("a session_id");
        let session = row(&client, "weft.nodes.NodeSessions", "session_id", session_id);
        assert_eq!(
            (session["node_id"].clone(), session["state"].clone()),
            (json!(node.node_id), json!("Open")),
            "{session}"
        );
        let opened_at = instant(session["opened_at"].as_str().expect("opened_at"));
        assert!(
            before <= opened_at && opened_at <= after,
            "opened_at {opened_at} is not between {before} and {after}"
        );

        // 3. The log after the seq read before the agent started holds OpenNodeSession as the
        // ControlPlane, answered 2xx.
        let log = host.log_after(before_seq);
        let request = log
            .iter()
            .find(|line| {
                line["kind"] == "request" && line["operation"] == "weft.nodes.OpenNodeSession"
            })
            .unwrap_or_else(|| {
                panic!("no OpenNodeSession request line after seq {before_seq}: {log:?}")
            });
        assert_eq!(
            request["actor"],
            json!("weft.actors.ControlPlane"),
            "{request}"
        );
        let status = request["status"].as_u64().expect("a status");
        assert!((200..300).contains(&status), "{request}");
    }

    /// Statement 4: OpenNodeSession over HTTP as the node agent is not granted, and writes no
    /// session.
    #[test]
    fn open_node_session_sent_by_the_node_agent_is_not_granted() {
        let (host, node) = provisioned();
        let client = host.client();
        let sessions = "weft.nodes.NodeSessions";
        let before = rows(&client, sessions);
        let answer = client
            .command(
                "weft.nodes.OpenNodeSession",
                steps::NODE_AGENT,
                json!({}),
                json!({
                    "node_id": node.node_id,
                    "agent_version": "0.1.0",
                    "received_at": OffsetDateTime::now_utc().format(&Rfc3339).expect("RFC 3339"),
                }),
            )
            .expect("the control host answers");
        assert_eq!(answer.status, 403, "{}", answer.body);
        assert_eq!(
            answer.body["refused"],
            json!("not granted"),
            "{}",
            answer.body
        );
        assert_eq!(rows(&client, sessions), before);
    }

    /// A restarted agent dials over its node's still-Open session (`replace-open-session`): it
    /// gets a new session, and the old one reads Superseded.
    #[test]
    fn a_restarted_agent_gets_a_new_session_and_its_old_one_is_superseded() {
        let (host, node) = provisioned();
        let client = host.client();
        let mut first = NodeAgent::start(&host, &node.node_id, &node.join_token);
        let lines = first.wait_for(HELLO_LIMIT, |lines| !hellos(lines).is_empty());
        let old = hellos(&lines)[0]["session_id"].clone();
        drop(first);

        let mut second = NodeAgent::start(&host, &node.node_id, &node.join_token);
        let lines = second.wait_for(HELLO_LIMIT, |lines| !hellos(lines).is_empty());
        let hello = &hellos(&lines)[0];
        assert_eq!(hello["hello"], json!("opened"), "{lines:?}");
        assert_ne!(hello["session_id"], old);
        let sessions = "weft.nodes.NodeSessions";
        let old = row(
            &client,
            sessions,
            "session_id",
            old.as_str().expect("a session id"),
        );
        assert_eq!(old["state"], json!("Superseded"), "{old}");
        let new = row(
            &client,
            sessions,
            "session_id",
            hello["session_id"].as_str().expect("a session id"),
        );
        assert_eq!(new["state"], json!("Open"), "{new}");
    }

    /// A stand-in control host on loopback: it answers `JoinNode` `joined` and
    /// `RecordNodeObservation` `ready`, and each `hello` with what `hello` gives for its number
    /// (from 0): `Some((status, body))`, or `None` to close the connection unanswered. Each
    /// `hello`'s arrival instant is pushed to the returned list.
    fn stand_in(
        hello: impl Fn(usize) -> Option<(u16, Value)> + Send + 'static,
    ) -> (
        std::net::SocketAddr,
        std::sync::Arc<std::sync::Mutex<Vec<Instant>>>,
    ) {
        use std::io::{BufRead, BufReader, Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a loopback port");
        let address = listener.local_addr().expect("an address");
        let arrivals = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let seen = arrivals.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { continue };
                let mut reader = BufReader::new(stream);
                let mut head = String::new();
                let mut length = 0;
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                        break;
                    }
                    if let Some((name, value)) = line.split_once(':') {
                        if name.eq_ignore_ascii_case("content-length") {
                            length = value.trim().parse().unwrap_or(0);
                        }
                    }
                    head.push_str(&line);
                }
                let mut body = vec![0; length];
                let _ = reader.read_exact(&mut body);
                let path = head
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or_default()
                    .to_owned();
                let answer = match path.as_str() {
                    "/weft/commands/JoinNode" => {
                        Some((202, json!({"outcome": "joined", "published": []})))
                    }
                    "/weft/commands/RecordNodeObservation" => {
                        Some((202, json!({"outcome": "ready", "published": []})))
                    }
                    "/node/hello" => {
                        let number = {
                            let mut seen = seen.lock().expect("the arrivals lock");
                            seen.push(Instant::now());
                            seen.len() - 1
                        };
                        hello(number)
                    }
                    _ => Some((404, json!({"refused": "not here"}))),
                };
                let mut stream = reader.into_inner();
                if let Some((status, body)) = answer {
                    let body = body.to_string();
                    let _ = write!(
                        stream,
                        "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                }
            }
        });
        (address, arrivals)
    }

    /// A refused `hello` is reported with its status and ends the agent with a failure
    /// (`session-refused`: the node closes and does not dial again).
    #[test]
    fn a_refused_hello_is_reported_and_ends_the_agent() {
        let (control, arrivals) = stand_in(|_| {
            Some((
                409,
                json!({"outcome": "already-open", "error": "weft.nodes.SessionAlreadyOpen", "published": []}),
            ))
        });
        let mut agent = NodeAgent::start_at(control, "node-1", "token");
        let status = agent.wait_exit(HELLO_LIMIT);
        assert!(!status.success(), "{status}");
        assert_eq!(
            hellos(&agent.report()),
            vec![json!({"hello": "refused", "status": 409})]
        );
        assert_eq!(arrivals.lock().expect("the arrivals lock").len(), 1);
    }

    /// A `hello` whose connection is lost, unanswered, is sent again after the redial delay of
    /// the protocol model (3000 ms; `lose-transport-dialing`, `redial`), and no line is reported
    /// for it.
    #[test]
    fn a_hello_whose_connection_is_lost_is_sent_again_after_the_redial_delay() {
        let (control, arrivals) =
            stand_in(|number| (number > 0).then(|| (200, json!({"session_id": "session-1"}))));
        let mut agent = NodeAgent::start_at(control, "node-1", "token");
        let lines = agent.wait_for(HELLO_LIMIT, |lines| !hellos(lines).is_empty());
        assert_eq!(
            hellos(&lines),
            vec![json!({"hello": "opened", "session_id": "session-1"})]
        );
        let arrivals = arrivals.lock().expect("the arrivals lock").clone();
        assert_eq!(arrivals.len(), 2, "{arrivals:?}");
        let gap = arrivals[1] - arrivals[0];
        assert!(
            gap >= Duration::from_millis(3000),
            "the hello after the lost connection came {gap:?} later, before the redial delay"
        );
    }
}

/// Adversary cases of `story:skeleton-node-join-session` that need no substrate daemon.
mod join_adversary {
    use weft_agent::agent::Options;
    use weft_client::Client;
    use weft_testkit::ControlHost;

    /// The usage line names the control URL `http://<host>:<port>`. `Options::parse` keeps the
    /// first address the host name resolves to, and the agent dials only that one. Where
    /// `localhost` resolves to `::1` first (this host: `getent ahosts localhost`), a control
    /// host on its default `--listen 127.0.0.1` is never reached through
    /// `--control http://localhost:<port>`.
    #[test]
    fn a_control_url_naming_localhost_reaches_a_control_host_on_127_0_0_1() {
        let host = ControlHost::start();
        let url = format!("http://localhost:{}", host.address().port());
        let options = Options::parse(
            [
                "--control",
                url.as_str(),
                "--node-id",
                "n",
                "--join-token",
                "t",
                "--substrate",
                "s",
                "--report",
                "r",
            ]
            .map(str::to_owned),
        )
        .expect("the arguments parse");
        let answer = Client::new(options.control).view("weft.infrastructure.Nodes", &[]);
        assert!(
            answer.as_ref().is_ok_and(|answer| answer.status == 200),
            "--control {url} dials {} only: {answer:?}",
            options.control
        );
    }
}
