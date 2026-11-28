//! The one integration-test binary of weft-agent.

mod seeds;

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
    /// `hello`'s arrival instant is pushed to the second returned list, each
    /// `RecordNodeObservation`'s input to the third.
    #[allow(clippy::type_complexity, reason = "three handles of one test fixture")]
    fn stand_in(
        hello: impl Fn(usize) -> Option<(u16, Value)> + Send + 'static,
    ) -> (
        std::net::SocketAddr,
        std::sync::Arc<std::sync::Mutex<Vec<Instant>>>,
        std::sync::Arc<std::sync::Mutex<Vec<Value>>>,
    ) {
        use std::io::{BufRead, BufReader, Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a loopback port");
        let address = listener.local_addr().expect("an address");
        let arrivals = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let seen = arrivals.clone();
        let observations = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let observed = observations.clone();
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
                        observed
                            .lock()
                            .expect("the observations lock")
                            .push(serde_json::from_slice(&body).unwrap_or(Value::Null));
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
        (address, arrivals, observations)
    }

    /// The node's observation is what its `NodeFacts` port answered, and nothing else: a stub
    /// port that answers what the local adapter never does (level `MicroVm`, 3 CPUs) is what
    /// `RecordNodeObservation` carries, and a fact the port withholds is reported `false`.
    #[test]
    fn the_observation_is_what_the_node_facts_port_answered() {
        use weft_platform::node::{
            Capacity, Exec, ExecError, ExecState, Exited, Facts, Isolation, IsolationLevel,
            NodeFacts, Process,
        };
        use weft_platform::Refusal;

        struct Stub;
        impl NodeFacts for Stub {
            fn observe(&self) -> Result<Facts, Refusal> {
                Ok(Facts {
                    capacity: vec![Capacity {
                        resource_class: "cpu".to_owned(),
                        amount: 3,
                    }],
                    isolation: Isolation {
                        levels: vec![IsolationLevel::MicroVm],
                        cgroups_delegated: None,
                        egress_denied: Some(true),
                        kvm_present: Some(true),
                        gpu_passthrough: None,
                    },
                })
            }
        }
        impl Exec for Stub {
            fn run(&self, _: &Process, _: &mut dyn FnMut(ExecState)) -> Result<Exited, ExecError> {
                Err(Refusal::NotServed {
                    operation: "run".to_owned(),
                }
                .into())
            }
        }

        let (control, _, observations) = stand_in(|_| Some((409, json!({"published": []}))));
        let report = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("observation-{}.jsonl", std::process::id()));
        let options = weft_agent::agent::Options::parse([
            "--control".to_owned(),
            format!("http://{control}"),
            "--node-id".to_owned(),
            "node-1".to_owned(),
            "--join-token".to_owned(),
            "token".to_owned(),
            "--substrate".to_owned(),
            "/nonexistent/substrate.sock".to_owned(),
            "--report".to_owned(),
            report.to_string_lossy().into_owned(),
        ])
        .expect("the arguments parse");
        let ended = weft_agent::agent::run(&options, &Stub, &Stub);
        let _ = std::fs::remove_file(&report);
        assert!(ended.is_err(), "the refused hello ends the run: {ended:?}");
        let observations = observations.lock().expect("the observations lock").clone();
        assert_eq!(observations.len(), 1, "{observations:?}");
        assert_eq!(
            observations[0]["capacity"],
            json!([{"resource_class": "cpu", "amount": 3}])
        );
        assert_eq!(
            observations[0]["isolation_facts"],
            json!({
                "levels": ["MicroVm"],
                "cgroups_delegated": false,
                "egress_denied": true,
                "kvm_present": true,
                "gpu_passthrough": false,
            })
        );
    }

    /// Adversary (weft#29): a fact the port answers present and `false` is reported `false`.
    /// The case above pins absent -> `false` and `true` -> `true` only, so `fact.is_some()` in
    /// place of `fact.unwrap_or(false)` (`isolation_facts` in `agent.rs`) passes the whole suite
    /// and reports a GPU passed through on every node the local adapter observes (it answers
    /// `gpu_passthrough: Some(false)`, and `kvm_present: Some(false)` on a host without KVM).
    #[test]
    fn a_fact_the_port_answers_false_is_reported_false() {
        use weft_platform::node::{
            Capacity, Exec, ExecError, ExecState, Exited, Facts, Isolation, IsolationLevel,
            NodeFacts, Process,
        };
        use weft_platform::Refusal;

        struct Stub;
        impl NodeFacts for Stub {
            fn observe(&self) -> Result<Facts, Refusal> {
                Ok(Facts {
                    capacity: vec![Capacity {
                        resource_class: "cpu".to_owned(),
                        amount: 1,
                    }],
                    isolation: Isolation {
                        levels: vec![IsolationLevel::Container],
                        cgroups_delegated: Some(false),
                        egress_denied: Some(false),
                        kvm_present: Some(false),
                        gpu_passthrough: Some(false),
                    },
                })
            }
        }
        impl Exec for Stub {
            fn run(&self, _: &Process, _: &mut dyn FnMut(ExecState)) -> Result<Exited, ExecError> {
                Err(Refusal::NotServed {
                    operation: "run".to_owned(),
                }
                .into())
            }
        }

        let (control, _, observations) = stand_in(|_| Some((409, json!({"published": []}))));
        let report = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("observation-false-{}.jsonl", std::process::id()));
        let options = weft_agent::agent::Options::parse([
            "--control".to_owned(),
            format!("http://{control}"),
            "--node-id".to_owned(),
            "node-1".to_owned(),
            "--join-token".to_owned(),
            "token".to_owned(),
            "--substrate".to_owned(),
            "/nonexistent/substrate.sock".to_owned(),
            "--report".to_owned(),
            report.to_string_lossy().into_owned(),
        ])
        .expect("the arguments parse");
        let ended = weft_agent::agent::run(&options, &Stub, &Stub);
        let _ = std::fs::remove_file(&report);
        assert!(ended.is_err(), "the refused hello ends the run: {ended:?}");
        let observations = observations.lock().expect("the observations lock").clone();
        assert_eq!(observations.len(), 1, "{observations:?}");
        assert_eq!(
            observations[0]["isolation_facts"],
            json!({
                "levels": ["Container"],
                "cgroups_delegated": false,
                "egress_denied": false,
                "kvm_present": false,
                "gpu_passthrough": false,
            })
        );
    }

    /// A refused `hello` is reported with its status and ends the agent with a failure
    /// (`session-refused`: the node closes and does not dial again).
    #[test]
    fn a_refused_hello_is_reported_and_ends_the_agent() {
        let (control, arrivals, _) = stand_in(|_| {
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
        let (control, arrivals, _) =
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

    /// The usage line names the control URL `http://<host>:<port>`. Where `localhost` resolves
    /// to `::1` first (`getent ahosts localhost`), a control host on its default `--listen
    /// 127.0.0.1` is reached through `--control http://localhost:<port>` only when the agent
    /// dials every resolved address in order. This case passes on a resolver that gives
    /// `127.0.0.1` first; `dial::*` checks the order without the resolver.
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
        let answer = Client::new(options.control.clone()).view("weft.infrastructure.Nodes", &[]);
        assert!(
            answer.as_ref().is_ok_and(|answer| answer.status == 200),
            "--control {url} dials {} only: {answer:?}",
            options.control
        );
    }
}

/// The dial of the control host tries every address in order until one connects, as
/// `TcpStream::connect` does over `to_socket_addrs`; the order is given here, not resolved, so
/// the cases hold on any resolver.
mod dial {
    use std::net::{SocketAddr, TcpListener};
    use std::time::Duration;

    use weft_client::Client;
    use weft_testkit::ControlHost;

    /// A loopback address nothing listens on: a port the kernel gave out, then closed.
    fn dead_address() -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port binds");
        listener.local_addr().expect("the listener has an address")
    }

    #[test]
    fn the_dial_connects_to_the_second_address_when_nothing_listens_on_the_first() {
        let dead = dead_address();
        let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port binds");
        let live = listener.local_addr().expect("the listener has an address");
        let stream = weft_client::dial(&[dead, live], Duration::from_secs(5))
            .expect("the second address connects");
        assert_eq!(stream.peer_addr().expect("a peer"), live);
    }

    #[test]
    fn a_client_whose_first_address_is_dead_reaches_the_control_host_at_the_second() {
        let host = ControlHost::start();
        let answer = Client::new(vec![dead_address(), host.address()])
            .view("weft.infrastructure.Nodes", &[]);
        assert!(
            answer.as_ref().is_ok_and(|answer| answer.status == 200),
            "the client gave up after the first address: {answer:?}"
        );
    }

    #[test]
    fn the_dial_of_only_dead_addresses_fails_with_the_last_error() {
        let error = weft_client::dial(&[dead_address(), dead_address()], Duration::from_secs(5))
            .expect_err("nothing listens");
        assert_eq!(error.kind(), std::io::ErrorKind::ConnectionRefused);
    }
}

/// `follow` (`story:skeleton-node-run-assignment`): from a seeded state of two nodes, each with
/// one `Scheduled` instance and an `Active` assignment, `weft-agent` follows its own work list:
/// it verifies its instance (`RecordArtifactVerified`), starts its process through the `exec`
/// port, records it running (`RecordInstanceRunning`), and writes its exit to the report file.
/// The other node's instance is never started. Each test runs a `ControlHost` of its own.
mod follow {
    use std::path::Path;
    use std::time::{Duration, Instant};

    use serde_json::{json, Value};
    use weft_client::{Client, Expect};
    use weft_testkit::{ControlHost, NodeAgent};

    use crate::seeds::{self, Seed};

    /// How long the agent may take from its start to the exit of its instance's process.
    const RUN_LIMIT: Duration = Duration::from_secs(30);

    /// A control host on the seeded state, with this node's join token digest beside its node,
    /// as `ProvisionPoolNode` keeps it. The digest is not a row: the host is started once on the
    /// rows, stopped, given the digest through the storage port, and started again before any
    /// agent runs.
    fn seeded(seed: &Seed) -> ControlHost {
        let mut host = ControlHost::start_seeded(seed.rows.clone());
        host.stop();
        {
            let ports = weft_impl::Ports::open(&host.state_path()).expect("the state file opens");
            ports.store().set_join_token_digest(
                &weft_types::shared::NodeId(weft_types::primitives::Uuid(
                    seed.this.node_id.clone(),
                )),
                &weft_impl::context::decider::join_token_digest(&seed.join_token),
            );
        }
        host.restart()
    }

    /// The row of `weft.workloads.Instances` of `instance_id`.
    fn instance(client: &Client, instance_id: &str) -> Value {
        let answer = client.view("weft.workloads.Instances", &[]).ok_body(200);
        answer["rows"]
            .as_array()
            .expect("rows")
            .iter()
            .find(|row| row["instance_id"] == json!(instance_id))
            .unwrap_or_else(|| panic!("no instance {instance_id}: {answer}"))
            .clone()
    }

    /// The `pass` of the `started` line of `instance_id`.
    fn started_pass(lines: &[Value], instance_id: &str) -> Option<u64> {
        lines
            .iter()
            .find(|line| line["started"] == json!(instance_id))
            .and_then(|line| line["pass"].as_u64())
    }

    /// The highest number of a `{"pass": n}` line.
    fn last_pass(lines: &[Value]) -> Option<u64> {
        lines
            .iter()
            .filter(|line| line.get("started").is_none())
            .filter_map(|line| line["pass"].as_u64())
            .max()
    }

    /// The exit line of `instance_id`.
    fn exit_line<'a>(lines: &'a [Value], instance_id: &str) -> Option<&'a Value> {
        lines.iter().find(|line| {
            line["instance_id"] == json!(instance_id) && line.get("exit_code").is_some()
        })
    }

    /// The report holds the exit line of `instance_id` and a pass line after the pass that
    /// started it.
    fn finished(lines: &[Value], instance_id: &str) -> bool {
        exit_line(lines, instance_id).is_some()
            && started_pass(lines, instance_id)
                .is_some_and(|started| last_pass(lines).is_some_and(|last| last > started))
    }

    /// Whether any member of `line` is `value`.
    fn names(line: &Value, value: &str) -> bool {
        line.as_object()
            .is_some_and(|members| members.values().any(|member| member == &json!(value)))
    }

    /// Statements 1 to 8: the agent's instance reads `Running`, its process ran through the exec
    /// port on substrate-daemon and printed `weft`; the other node's instance is untouched; the
    /// control host log holds one 2xx `RecordInstanceRunning` and the two edge bindings of the
    /// instance's first readiness as declared.
    #[test]
    fn the_agent_starts_its_assigned_instance_confined_and_records_it_running() {
        let seed = seeds::two_nodes();
        let host = seeded(&seed);
        let client = host.client();
        let this = &seed.this.instance_id;
        let other = &seed.other.instance_id;
        assert_eq!(instance(&client, this)["state"], json!("Scheduled"));

        let before_seq = host.last_seq();
        let mut agent = NodeAgent::start(&host, &seed.this.node_id, &seed.join_token);
        let lines = agent.wait_for(RUN_LIMIT, |lines| finished(lines, this));

        // 1. Its instance reads Running.
        let row = instance(&client, this);
        assert_eq!(row["state"], json!("Running"), "{row}");

        // 2. The exit line of its process.
        assert!(
            lines.contains(&json!({"instance_id": this, "exit_code": 0, "stdout": "weft\n"})),
            "{lines:?}"
        );

        // 3. After a pass later than the one that started its instance, the other node's
        // instance still reads Scheduled.
        let row = instance(&client, other);
        assert_eq!(row["state"], json!("Scheduled"), "{row}");

        // 4. No line names the other instance, and none has its output.
        for line in &lines {
            assert!(
                !names(line, other),
                "a line names the other instance: {line}"
            );
            assert!(
                !line["stdout"]
                    .as_str()
                    .is_some_and(|stdout| stdout.contains("other")),
                "a line has the other instance's output: {line}"
            );
        }

        let log = host.log_after(before_seq);
        // 5. One request line of RecordInstanceRunning, answered 2xx.
        let running: Vec<&Value> = log
            .iter()
            .filter(|line| {
                line["kind"] == "request"
                    && line["operation"] == "weft.workloads.RecordInstanceRunning"
            })
            .collect();
        assert_eq!(running.len(), 1, "{log:?}");
        let status = running[0]["status"].as_u64().expect("a status");
        assert!((200..300).contains(&status), "{}", running[0]);

        let bindings = |id: &str| -> Vec<&Value> {
            log.iter()
                .filter(|line| line["kind"] == "binding" && line["binding"] == id)
                .collect()
        };
        // 6. One edge-backend-ready line, applied.
        let ready = bindings("edge-backend-ready");
        assert_eq!(ready.len(), 1, "{log:?}");
        assert_eq!(ready[0]["result"], json!("applied"), "{}", ready[0]);

        // 7. One edge-backend-restore line, not-out-of-traffic dropped.
        let restore = bindings("edge-backend-restore");
        let dropped: Vec<&&Value> = restore
            .iter()
            .filter(|line| line["refusal"] == "not-out-of-traffic" && line["result"] == "dropped")
            .collect();
        assert_eq!(dropped.len(), 1, "{log:?}");

        // 8. No edge-backend-restore line retried.
        assert!(
            restore.iter().all(|line| line["result"] != "retried"),
            "{restore:?}"
        );
    }

    /// The agent starts its processes through the `exec` port and through nothing else: with a
    /// stub port that runs nothing and answers `stub`, the report holds the stub's output, the
    /// stub was given the instance's command, and the instance still reads `Running`.
    #[test]
    fn the_agent_starts_its_processes_only_through_the_exec_port() {
        use std::sync::{Arc, Mutex};
        use weft_platform::node::{
            Capacity, Exec, ExecError, ExecState, Exited, Facts, Isolation, IsolationLevel,
            NodeFacts, Process,
        };
        use weft_platform::Refusal;

        struct Stub(Arc<Mutex<Vec<Vec<String>>>>);
        impl NodeFacts for Stub {
            fn observe(&self) -> Result<Facts, Refusal> {
                Ok(Facts {
                    capacity: vec![Capacity {
                        resource_class: "cpu".to_owned(),
                        amount: 1,
                    }],
                    isolation: Isolation {
                        levels: vec![IsolationLevel::Container],
                        cgroups_delegated: Some(true),
                        egress_denied: Some(true),
                        kvm_present: Some(false),
                        gpu_passthrough: Some(false),
                    },
                })
            }
        }
        impl Exec for Stub {
            fn run(
                &self,
                process: &Process,
                on_state: &mut dyn FnMut(ExecState),
            ) -> Result<Exited, ExecError> {
                self.0
                    .lock()
                    .expect("the argv lock")
                    .push(process.argv.clone());
                on_state(ExecState::Accepted);
                on_state(ExecState::Running);
                on_state(ExecState::Exited);
                Ok(Exited {
                    code: Some(0),
                    stdout: b"stub\n".to_vec(),
                    stderr: Vec::new(),
                    truncated: false,
                })
            }
        }

        let seed = seeds::two_nodes();
        let host = seeded(&seed);
        let client = host.client();
        let this = seed.this.instance_id.clone();
        let report = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("follow-stub-{}.jsonl", std::process::id()));
        let _ = std::fs::remove_file(&report);
        let options = weft_agent::agent::Options::parse([
            "--control".to_owned(),
            format!("http://{}", host.address()),
            "--node-id".to_owned(),
            seed.this.node_id.clone(),
            "--join-token".to_owned(),
            seed.join_token.clone(),
            "--substrate".to_owned(),
            "/nonexistent/substrate.sock".to_owned(),
            "--report".to_owned(),
            report.to_string_lossy().into_owned(),
        ])
        .expect("the arguments parse");
        let argv = Arc::new(Mutex::new(Vec::new()));
        let stub: &'static Stub = Box::leak(Box::new(Stub(Arc::clone(&argv))));
        // The agent holds its session until it is stopped; its thread ends with the test.
        std::thread::spawn(move || weft_agent::agent::run(&options, stub, stub));

        let start = Instant::now();
        let lines = loop {
            let text = std::fs::read_to_string(&report).unwrap_or_default();
            let complete = text.rfind('\n').map_or("", |end| &text[..end]);
            let lines: Vec<Value> = complete
                .lines()
                .map(|line| serde_json::from_str(line).expect("a report line"))
                .collect();
            if finished(&lines, &this) {
                break lines;
            }
            assert!(
                start.elapsed() < RUN_LIMIT,
                "the report is not complete after {RUN_LIMIT:?}: {lines:?}"
            );
            std::thread::sleep(Duration::from_millis(20));
        };
        let _ = std::fs::remove_file(&report);

        assert!(
            lines.contains(&json!({"instance_id": this, "exit_code": 0, "stdout": "stub\n"})),
            "{lines:?}"
        );
        assert_eq!(
            argv.lock().expect("the argv lock").clone(),
            vec![vec![
                "sh".to_owned(),
                "-c".to_owned(),
                seeds::THIS_SNIPPET.to_owned()
            ]]
        );
        let row = instance(&client, &this);
        assert_eq!(row["state"], json!("Running"), "{row}");
    }
}
