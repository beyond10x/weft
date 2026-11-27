//! The one integration-test binary of weft-testkit.

mod trivial {
    #[test]
    fn repo_root_holds_the_pinned_toolchain() {
        assert!(weft_testkit::repo_root()
            .join("rust-toolchain.toml")
            .is_file());
    }
}

/// The form of the generated crates (`target/generated/`): a tree with a `[workspace]` of its own,
/// excluded from the root workspace, used by a weft crate through a path dependency.
mod probe {
    use weft_probe_api::{echo, ProbeId};

    #[test]
    fn a_crate_of_a_tree_with_its_own_workspace_builds_as_a_path_dependency() {
        let manifest = weft_testkit::repo_root().join("tests/fixtures/generated-probe/Cargo.toml");
        let text = std::fs::read_to_string(&manifest).expect("the probe manifest reads");
        assert!(
            text.lines().any(|line| line.trim() == "[workspace]"),
            "{} declares no [workspace]",
            manifest.display()
        );
        assert_eq!(echo(ProbeId(7)), ProbeId(7));
    }
}

/// The generated code (`target/generated/`, `tools/synth.sh`): `weft-server` builds as a path dependency
/// and answers through its own mapping.
mod generated {
    use weft_server::entry::Refused;

    #[test]
    fn an_unmet_obligation_answers_501_through_the_generated_status() {
        let refused = Refused::Unmet("weft.workloads.PlaceInstance".to_string());
        assert_eq!(refused.status(), 501);
        assert!(!refused.committed());
    }
}

/// The control host (`story:skeleton-control-host`): the `weft` binary on loopback with its SQLite
/// state file and its log, driven through `weft-client` with the operator and tenant steps of
/// `sim/experiments/full-setup.yaml`.
mod control_host {
    use serde_json::{json, Value};
    use weft_client::Expect;
    use weft_testkit::steps::{self, input};
    use weft_testkit::ControlHost;

    /// The row of a view answer whose `key` member is `id`.
    fn row(answer: &Value, key: &str, id: &str) -> Value {
        let rows = answer["rows"]
            .as_array()
            .unwrap_or_else(|| panic!("no rows in {answer}"));
        rows.iter()
            .find(|row| row[key] == json!(id))
            .unwrap_or_else(|| panic!("no row with {key} {id} in {answer}"))
            .clone()
    }

    /// Every member of `sent` equals the member of the same name in `row`.
    fn holds_input(row: &Value, sent: &Value) {
        for (field, value) in sent.as_object().expect("an input is an object") {
            assert_eq!(&row[field], value, "field {field} of {row}");
        }
    }

    /// The rows of statement 1, each read from its view by the identity its command answered.
    fn created_rows(host: &ControlHost, ids: &steps::Path) -> Vec<Value> {
        let client = host.client();
        let view = |name: &str, params: &[(&str, &str)]| client.view(name, params).ok_body(200);
        vec![
            row(
                &view("weft.infrastructure.Clouds", &[]),
                "cloud_id",
                &ids.cloud_id,
            ),
            row(&view("weft.infrastructure.Zones", &[]), "zone", "z1"),
            row(
                &view("weft.infrastructure.PriorityClasses", &[]),
                "name",
                "standard",
            ),
            row(
                &view("weft.infrastructure.NodePools", &[]),
                "node_pool_id",
                &ids.node_pool_id,
            ),
            row(
                &view("weft.infrastructure.Nodes", &[]),
                "node_id",
                &ids.node.node_id,
            ),
            row(
                &view("weft.cluster.Clusters", &[]),
                "cluster_id",
                &ids.cluster_id,
            ),
            row(
                &view("weft.accounts.Tenants", &[]),
                "tenant_id",
                &ids.tenant_id,
            ),
            row(
                &view(
                    "weft.accounts.AccountTree",
                    &[("tenant_id", &ids.tenant_id)],
                ),
                "account_id",
                &ids.account_id,
            ),
            row(
                &view(
                    "weft.accounts.TenantQuotas",
                    &[("tenant_id", &ids.tenant_id)],
                ),
                "quota_id",
                &ids.quota_id,
            ),
        ]
    }

    /// Statements 1 to 4: the operator and tenant path through the binary, the log after a
    /// `seq`, the join token, and a restart on the same state file.
    #[test]
    fn the_operator_and_tenant_path_reads_back_logs_joins_and_survives_a_restart() {
        let host = ControlHost::start();
        let client = host.client();

        let cloud_id = steps::create_cloud(&client);
        steps::declare_zone(&client, &cloud_id, "z1");
        steps::declare_zone(&client, &cloud_id, "z2");
        steps::define_priority_class(&client);
        let node_pool_id = steps::create_node_pool(&client);
        let node = steps::provision_pool_node(&client, &node_pool_id);
        let cluster_id = steps::create_default_cluster(&client, &cloud_id);
        let before = host.last_seq();
        let tenant_id = steps::create_tenant(&client, "acme");
        let after_tenant = host.log_after(before);
        let account_id = steps::create_account(&client, &tenant_id);
        let quota_id = steps::grant_top_quota(&client, &account_id);
        let ids = steps::Path {
            cloud_id,
            node_pool_id,
            node,
            cluster_id,
            tenant_id,
            account_id,
            quota_id,
        };

        // 1. Each created row read back by its identity, each input field as sent.
        let rows = created_rows(&host, &ids);
        holds_input(&rows[0], &input::create_cloud());
        holds_input(&rows[1], &input::declare_zone(&ids.cloud_id, "z1"));
        holds_input(&rows[2], &input::define_priority_class());
        holds_input(&rows[3], &input::create_node_pool());
        holds_input(
            &rows[4],
            &json!({
                "node_pool_id": ids.node_pool_id,
                "join_token_expires_at": ids.node.join_token_expires_at,
            }),
        );
        holds_input(&rows[5], &input::create_default_cluster(&ids.cloud_id));
        holds_input(&rows[6], &input::create_tenant("acme"));
        holds_input(&rows[7], &input::create_account(&ids.tenant_id));
        holds_input(&rows[8], &input::grant_top_quota(&ids.account_id));
        assert_eq!(rows[6]["name"], json!("acme"));
        assert_eq!(
            (rows[8]["resource_class"].clone(), rows[8]["limit"].clone()),
            (json!("cpu"), json!(64))
        );
        assert_eq!(
            (
                rows[3]["name"].clone(),
                rows[3]["zone"].clone(),
                rows[3]["max_size"].clone()
            ),
            (json!("shared"), json!("z1"), json!(8))
        );

        // 2. The log after the seq read before CreateTenant: its request line, then its event.
        assert!(!after_tenant.is_empty(), "no log line after seq {before}");
        assert!(
            after_tenant
                .iter()
                .all(|line| line["seq"].as_u64().expect("a seq") > before),
            "a line at or below seq {before}: {after_tenant:?}"
        );
        let request = after_tenant
            .iter()
            .find(|line| {
                line["kind"] == "request" && line["operation"] == "weft.accounts.CreateTenant"
            })
            .unwrap_or_else(|| panic!("no request line for CreateTenant in {after_tenant:?}"));
        assert_eq!(request["actor"], json!("weft.actors.ClusterOperator"));
        let status = request["status"].as_u64().expect("a status");
        assert!((200..300).contains(&status), "status {status}");
        let event = after_tenant
            .iter()
            .find(|line| line["kind"] == "event" && line["name"] == "weft.accounts.TenantCreated")
            .unwrap_or_else(|| panic!("no event line for TenantCreated in {after_tenant:?}"));
        assert!(
            event["seq"].as_u64() > request["seq"].as_u64(),
            "{event} is not after {request}"
        );

        // 3. JoinNode with another token is refused before the row; with the issued one it joins.
        let state = |host: &ControlHost| {
            let nodes = host
                .client()
                .view("weft.infrastructure.Nodes", &[])
                .ok_body(200);
            row(&nodes, "node_id", &ids.node.node_id)["state"].clone()
        };
        let wrong = steps::join_node(&client, &ids.node.node_id, "not-the-issued-token");
        assert_eq!(wrong["outcome"], json!("token-invalid"), "{wrong}");
        assert_eq!(state(&host), json!("Provisioning"));
        let joined = steps::join_node(&client, &ids.node.node_id, &ids.node.join_token);
        assert_eq!(joined["outcome"], json!("joined"), "{joined}");
        assert_eq!(state(&host), json!("Joining"));

        // 4. The same rows after a restart on the same state file.
        let before_restart = created_rows(&host, &ids);
        let host = host.restart();
        assert_eq!(created_rows(&host, &ids), before_restart);
    }

    /// Statement 5: an obligation off the path answers 501, committed false, naming its plan
    /// entry, and writes nothing.
    #[test]
    fn an_obligation_off_the_path_answers_501_and_writes_no_row() {
        let host = ControlHost::start();
        let client = host.client();
        let cloud_id = steps::create_cloud(&client);
        steps::declare_zone(&client, &cloud_id, "z1");
        let tenant_id = steps::create_tenant(&client, "acme");
        let account_id = steps::create_account(&client, &tenant_id);
        let clusters = || client.view("weft.cluster.Clusters", &[]).ok_body(200)["rows"].clone();
        let before = clusters();

        let answer = client
            .command(
                "weft.cluster.CreateCluster",
                "weft.actors.TenantOwner",
                json!({"tenant_id": tenant_id, "account_id": account_id}),
                json!({
                    "account_id": account_id,
                    "cloud_id": cloud_id,
                    "name": "acme-main",
                    "control_plane": "Shared",
                    "min_isolation": "Container",
                    "zones": ["z1"],
                }),
            )
            .expect("the control host answers");
        assert_eq!(answer.status, 501, "{}", answer.body);
        assert_eq!(answer.body["committed"], json!(false), "{}", answer.body);
        let refused = answer.body["refused"].as_str().expect("a refusal");
        assert!(
            refused.contains("command behaviour `weft.cluster.CreateCluster`"),
            "{refused}"
        );
        assert!(refused.contains("PLAN.md"), "{refused}");
        assert_eq!(clusters(), before);
    }

    /// A row written into a state file through the storage port before a start reads back.
    #[test]
    fn a_seeded_row_reads_back_through_its_view() {
        let (tenant_id, tenant) = weft_testkit::tenant_row("seeded");
        let host = ControlHost::start_seeded(vec![tenant]);
        let rows = host
            .client()
            .view("weft.accounts.Tenants", &[])
            .ok_body(200);
        let found = row(&rows, "tenant_id", &tenant_id);
        assert_eq!(
            (found["name"].clone(), found["state"].clone()),
            (json!("seeded"), json!("Active"))
        );
    }

    /// Statement 6: the caller header is read only with the switch on; one build, both settings.
    #[test]
    fn the_caller_header_is_trusted_only_with_the_switch_on() {
        let dir = weft_testkit::TempDir::new();
        let host = weft_bin::Host::open(
            &dir.path().join("state.db"),
            &dir.path().join("control-host.log"),
        )
        .expect("the host opens");
        let caller = json!({"actor": "weft.actors.ClusterOperator", "caller": {}}).to_string();
        let create = weft_server::http::Request {
            method: "POST".to_owned(),
            path: "/weft/commands/CreateTenant".to_owned(),
            headers: vec![("x-weft-caller".to_owned(), caller)],
            body: input::create_tenant("acme").to_string().into_bytes(),
            ..Default::default()
        };
        let tenants = weft_server::http::Request {
            method: "GET".to_owned(),
            path: "/weft/views/Tenants".to_owned(),
            ..Default::default()
        };
        let rows = |host: &weft_bin::Host| {
            let answer = host.handle(&tenants, false);
            assert_eq!(answer.status, 200, "{}", answer.body);
            let body: Value = serde_json::from_str(&answer.body).expect("JSON");
            body["rows"].as_array().expect("rows").len()
        };

        let off = host.handle(&create, false);
        assert_eq!(off.status, 403, "{}", off.body);
        let body: Value = serde_json::from_str(&off.body).expect("JSON");
        assert_eq!(body["refused"], json!("not granted"), "{body}");
        assert_eq!(rows(&host), 0);

        let on = host.handle(&create, true);
        assert!((200..300).contains(&on.status), "{}", on.body);
        let body: Value = serde_json::from_str(&on.body).expect("JSON");
        assert_eq!(body["outcome"], json!("created"), "{body}");
        assert_eq!(rows(&host), 1);
    }

    /// A control host in this process on the state file and log of `dir`.
    fn open_host(dir: &weft_testkit::TempDir) -> weft_bin::Host {
        weft_bin::Host::open(
            &dir.path().join("state.db"),
            &dir.path().join("control-host.log"),
        )
        .expect("the host opens")
    }

    /// Sends `CreateTenant` named `name` as the cluster operator, trusted.
    fn create_tenant_in(host: &weft_bin::Host, name: &str) {
        let caller = json!({"actor": "weft.actors.ClusterOperator", "caller": {}}).to_string();
        let answer = host.handle(
            &weft_server::http::Request {
                method: "POST".to_owned(),
                path: "/weft/commands/CreateTenant".to_owned(),
                headers: vec![("x-weft-caller".to_owned(), caller)],
                body: input::create_tenant(name).to_string().into_bytes(),
                ..Default::default()
            },
            true,
        );
        assert_eq!(answer.status, 202, "{}", answer.body);
    }

    /// The `seq` of every line of the log file; each line must be a whole JSON line.
    fn log_seqs(dir: &weft_testkit::TempDir) -> Vec<u64> {
        let text =
            std::fs::read_to_string(dir.path().join("control-host.log")).expect("the log reads");
        assert!(
            text.is_empty() || text.ends_with('\n'),
            "a cut-off last line: {text:?}"
        );
        text.lines()
            .map(|line| {
                serde_json::from_str::<Value>(line).expect("a JSON line")["seq"]
                    .as_u64()
                    .expect("a seq")
            })
            .collect()
    }

    /// A command's rows and its log lines commit together in the state file; a log file that
    /// lost lines after their commit (a crash before the file write) gets them back on start.
    #[test]
    fn log_lines_the_file_lost_after_their_commit_come_back_on_start() {
        let dir = weft_testkit::TempDir::new();
        let host = open_host(&dir);
        create_tenant_in(&host, "one");
        let written = log_seqs(&dir);
        assert_eq!(
            written,
            vec![1, 2],
            "the request line and the TenantCreated line"
        );
        drop(host);

        std::fs::write(dir.path().join("control-host.log"), "").expect("the log is emptied");
        let host = open_host(&dir);
        assert!(
            host.notes()
                .iter()
                .any(|note| note.contains("appended 2 line")),
            "{:?}",
            host.notes()
        );
        assert_eq!(log_seqs(&dir), written);
        create_tenant_in(&host, "two");
        assert_eq!(log_seqs(&dir), vec![1, 2, 3, 4]);
    }

    /// A last log line cut off in its write is dropped on start, with a note, and the log goes
    /// on from the state file.
    #[test]
    fn a_cut_off_last_log_line_is_dropped_on_start_with_a_note() {
        let dir = weft_testkit::TempDir::new();
        let host = open_host(&dir);
        create_tenant_in(&host, "one");
        drop(host);
        let path = dir.path().join("control-host.log");
        let mut text = std::fs::read_to_string(&path).expect("the log reads");
        text.push_str("{\"seq\":3,\"kind\":\"requ");
        std::fs::write(&path, text).expect("the log is cut");

        let host = open_host(&dir);
        assert!(
            host.notes()
                .iter()
                .any(|note| note.contains("dropped an incomplete last line")),
            "{:?}",
            host.notes()
        );
        assert_eq!(log_seqs(&dir), vec![1, 2]);
        create_tenant_in(&host, "two");
        assert_eq!(log_seqs(&dir), vec![1, 2, 3, 4]);
    }

    /// A refusal commits nothing: a request whose work writes a row and then answers `501`
    /// `committed: false` (an unmet obligation) leaves no row and no event line; its request line
    /// is logged, as every answered request is.
    #[test]
    fn a_request_that_writes_a_row_and_then_refuses_leaves_no_row() {
        let dir = weft_testkit::TempDir::new();
        let host = open_host(&dir);
        let (tenant_id, tenant) = weft_testkit::tenant_row("written-then-refused");
        let answer = host.execute(Some("weft.test.WritesThenRefuses"), None, |_| {
            host.ports().clone().put_row(tenant);
            (
                weft_server::http::Response::unfinished(
                    "unmet obligation: command behaviour `weft.test.WritesThenRefuses`",
                    false,
                ),
                Vec::new(),
            )
        });
        assert_eq!(answer.status, 501, "{}", answer.body);

        let log =
            weft_client::log_after(&dir.path().join("control-host.log"), 0).expect("the log reads");
        assert_eq!(
            log,
            vec![json!({
                "seq": 1,
                "kind": "request",
                "operation": "weft.test.WritesThenRefuses",
                "actor": null,
                "status": 501,
            })]
        );
        let tenants = host.handle(
            &weft_server::http::Request {
                method: "GET".to_owned(),
                path: "/weft/views/Tenants".to_owned(),
                ..Default::default()
            },
            false,
        );
        let body: Value = serde_json::from_str(&tenants.body).expect("JSON");
        assert!(
            body["rows"]
                .as_array()
                .expect("rows")
                .iter()
                .all(|row| row["tenant_id"] != json!(tenant_id)),
            "the refused request's row was committed: {body}"
        );
    }

    /// A state file held by a control host is refused to a second one in the same process too.
    #[test]
    fn a_held_state_file_is_refused() {
        let dir = weft_testkit::TempDir::new();
        let _held = open_host(&dir);
        let refused =
            weft_bin::Host::open(&dir.path().join("state.db"), &dir.path().join("other.log"))
                .err()
                .expect("the second open is refused");
        assert!(
            refused.to_string().contains("held by another control host"),
            "{refused}"
        );
    }

    /// Cases of the adversary pass on `story:skeleton-control-host`.
    mod adversary {
        use std::io::{BufRead, BufReader, Read, Write};
        use std::net::{SocketAddr, TcpStream};
        use std::process::{Command, Stdio};
        use std::sync::mpsc;
        use std::time::Duration;

        use serde_json::{json, Value};
        use weft_client::{Client, Expect};
        use weft_testkit::steps;
        use weft_testkit::ControlHost;

        /// `src/storage/mod.rs` promises `list` "in key order, the order the generated memory
        /// store keeps". The memory store sorts by `MemoryKey::Text(String)` (`memory.rs`,
        /// `rows.sort_by(...)`); the SQLite store sorts the JSON text of the key, where the
        /// closing quote (0x22) sorts above a space (0x20). Zone names are free text
        /// (`weft.shared.ZoneName` is a newtype of `String`), so the Zones view of the control
        /// host answers `a b` before `a`, and the memory store `a` before `a b`.
        #[test]
        fn zones_read_back_in_the_order_of_the_generated_memory_store() {
            use weft_types::behaviour::ZoneStorage;
            use weft_types::infrastructure::{ZoneData, ZoneSnapshot, ZoneState};

            let host = ControlHost::start();
            let client = host.client();
            let cloud_id = steps::create_cloud(&client);
            let names = ["a", "a b"];
            for name in names {
                steps::declare_zone(&client, &cloud_id, name);
            }
            let view = client.view("weft.infrastructure.Zones", &[]).ok_body(200);
            let served: Vec<String> = view["rows"]
                .as_array()
                .expect("rows")
                .iter()
                .map(|row| row["zone"].as_str().expect("a zone").to_owned())
                .collect();

            let mut memory = weft_server::memory::MemoryPorts::default();
            for name in names.iter().rev() {
                ZoneStorage::put(
                    &mut memory,
                    ZoneSnapshot {
                        state: ZoneState::Declared,
                        data: ZoneData {
                            zone: weft_types::shared::ZoneName((*name).to_owned()),
                            cloud_id: weft_types::shared::CloudId(weft_types::primitives::Uuid(
                                cloud_id.clone(),
                            )),
                        },
                    },
                );
            }
            let kept: Vec<String> = ZoneStorage::list(&memory)
                .into_iter()
                .map(|row| row.data.zone.0)
                .collect();
            assert_eq!(kept, vec!["a".to_owned(), "a b".to_owned()]);
            assert_eq!(served, kept, "the Zones view of the control host");
        }

        /// The log's `seq` "counts up from 1 over the life of the file", and the control host
        /// is "one writer". A second `weft` started on the same state file and log either
        /// refuses to start or the log's seqs stay unique; today it starts, and both processes
        /// count from the same last line.
        #[test]
        fn a_second_control_host_on_the_same_state_file_does_not_duplicate_log_seqs() {
            let host = ControlHost::start();
            let mut second = Command::new(weft_testkit::control_host_binary())
                .arg("--state")
                .arg(host.state_path())
                .arg("--log")
                .arg(host.log_path())
                .arg("--port")
                .arg("0")
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .expect("the binary starts");
            let stdout = second.stdout.take().expect("stdout is piped");
            let (lines, received) = mpsc::channel();
            std::thread::spawn(move || {
                let mut line = String::new();
                let _ = BufReader::new(stdout).read_line(&mut line);
                let _ = lines.send(line);
            });
            let line = received
                .recv_timeout(Duration::from_secs(20))
                .unwrap_or_default();
            let Some(address) = line
                .split_whitespace()
                .nth(1)
                .and_then(|address| address.parse::<SocketAddr>().ok())
            else {
                // The second host refused to start: the one writer holds.
                let _ = second.kill();
                let _ = second.wait();
                return;
            };
            let other = Client::new(address);
            steps::create_tenant(&host.client(), "one");
            steps::create_tenant(&other, "two");
            steps::create_tenant(&host.client(), "three");
            let _ = second.kill();
            let _ = second.wait();

            let seqs: Vec<u64> = host
                .log_after(0)
                .iter()
                .map(|line| line["seq"].as_u64().expect("a seq"))
                .collect();
            assert!(
                seqs.windows(2).all(|pair| pair[0] < pair[1]),
                "log seqs are not strictly increasing with two hosts on one state file: {seqs:?}"
            );
        }

        /// One `GET` on the control host: the status and the JSON body.
        fn get(address: SocketAddr, path: &str) -> (u16, Value) {
            let mut stream = TcpStream::connect(address).expect("the control host accepts");
            stream
                .set_read_timeout(Some(Duration::from_secs(20)))
                .expect("a timeout");
            write!(
                stream,
                "GET {path} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n"
            )
            .expect("the request is written");
            let mut answer = Vec::new();
            stream.read_to_end(&mut answer).expect("the answer reads");
            let text = String::from_utf8_lossy(&answer).into_owned();
            let (head, body) = text.split_once("\r\n\r\n").expect("a head and a body");
            let status = head
                .split(' ')
                .nth(1)
                .and_then(|code| code.parse().ok())
                .expect("a status");
            (status, serde_json::from_str(body).unwrap_or(Value::Null))
        }

        /// The control host serves `weft-api` and `weft-control` on one port (decision A3).
        /// Both declare `GET /openapi.json`; the host answers it with the `weft-api` document
        /// only, so the contract of every `weft-control` path it serves, `CreateCloud` among
        /// them, cannot be read from it.
        #[test]
        fn the_served_contract_describes_every_command_path_the_host_answers() {
            let host = ControlHost::start();
            let client = host.client();
            steps::create_cloud(&client);
            let (status, contract) = get(host.address(), "/openapi.json");
            assert_eq!(status, 200, "{contract}");
            let paths = contract["paths"].as_object().expect("paths");
            for path in ["/weft/commands/CreateTenant", "/weft/commands/CreateCloud"] {
                assert!(
                    paths.contains_key(path),
                    "GET /openapi.json does not describe {path}, which the host answers"
                );
            }
        }

        /// E1 restored by hand: the join token is answered once and kept only as its SHA-256.
        /// No existing case would fail if the raw token were stored and compared. Also: a used
        /// token does not join again, and a token of another node or an empty one is invalid.
        #[test]
        fn the_join_token_is_kept_nowhere_and_joins_only_its_own_node_once() {
            let host = ControlHost::start();
            let client = host.client();
            let cloud_id = steps::create_cloud(&client);
            steps::declare_zone(&client, &cloud_id, "z1");
            let pool = steps::create_node_pool(&client);
            let first = steps::provision_pool_node(&client, &pool);
            let second = steps::provision_pool_node(&client, &pool);

            let outcome =
                |node: &str, token: &str| steps::join_node(&client, node, token)["outcome"].clone();
            assert_eq!(
                outcome(&second.node_id, &first.join_token),
                json!("token-invalid")
            );
            assert_eq!(outcome(&second.node_id, ""), json!("token-invalid"));
            assert_eq!(outcome(&first.node_id, &first.join_token), json!("joined"));
            assert_eq!(
                outcome(&first.node_id, &first.join_token),
                json!("token-used")
            );

            let dir = host.state_path();
            let dir = dir.parent().expect("a directory");
            for entry in std::fs::read_dir(dir).expect("the directory reads") {
                let path = entry.expect("an entry").path();
                let bytes = std::fs::read(&path).expect("the file reads");
                let text = String::from_utf8_lossy(&bytes);
                for token in [&first.join_token, &second.join_token] {
                    assert!(
                        !text.contains(token.as_str()),
                        "{} holds a join token",
                        path.display()
                    );
                }
            }
        }

        /// A reopened log goes on from its last line; no existing case restarts and then reads
        /// the log.
        #[test]
        fn the_log_seq_goes_on_after_a_restart() {
            let host = ControlHost::start();
            steps::create_tenant(&host.client(), "one");
            let before = host.last_seq();
            assert!(before > 0);
            let host = host.restart();
            steps::create_tenant(&host.client(), "two");
            let after = host.log_after(before);
            let first = after.first().expect("a line after the restart");
            assert_eq!(first["seq"], json!(before + 1), "{after:?}");
        }
    }
}

/// Adversary cases of `story:skeleton-node-join-session`: the node-channel endpoint of the
/// control host (`weft-bin/src/node_channel.rs`) and the behaviours `OpenNodeSession` and
/// `RecordNodeObservation`, against `ess/5-runtime/nodes.yaml`,
/// `ess/1-infrastructure/infrastructure.yaml` and `protocols/node-channel.yaml`.
mod node_channel_adversary {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::time::Duration;

    use serde_json::{json, Value};
    use weft_client::{Client, Expect};
    use weft_impl::context::{instant, timestamp};
    use weft_testkit::steps::{self, Provisioned};
    use weft_testkit::ControlHost;
    use weft_types::primitives::Timestamp;

    const CONTROL_PLANE: &str = "weft.actors.ControlPlane";
    const SESSIONS: &str = "weft.nodes.NodeSessions";
    const NODES: &str = "weft.infrastructure.Nodes";

    fn provision(client: &Client) -> Provisioned {
        let cloud_id = steps::create_cloud(client);
        steps::declare_zone(client, &cloud_id, "z1");
        let pool = steps::create_node_pool(client);
        steps::provision_pool_node(client, &pool)
    }

    /// A provisioned node joined with its token (Joining), as the agent does before `hello`.
    fn joined(client: &Client) -> Provisioned {
        let node = provision(client);
        let body = steps::join_node(client, &node.node_id, &node.join_token);
        assert_eq!(body["outcome"], json!("joined"), "{body}");
        node
    }

    fn rows(client: &Client, view: &str) -> Vec<Value> {
        client.view(view, &[]).ok_body(200)["rows"]
            .as_array()
            .expect("rows")
            .clone()
    }

    fn row(client: &Client, view: &str, key: &str, id: &str) -> Value {
        rows(client, view)
            .into_iter()
            .find(|row| row[key] == json!(id))
            .unwrap_or_else(|| panic!("no row with {key} {id} in {view}"))
    }

    fn hello(client: &Client, node_id: &str) -> weft_client::Answer {
        client
            .post(
                "/node/hello",
                &json!({"node_id": node_id, "agent_version": "0.1.0"}),
            )
            .expect("the control host answers")
    }

    fn open_as_control_plane(client: &Client, node_id: &str, received_at: &str) -> Value {
        client
            .command(
                "weft.nodes.OpenNodeSession",
                CONTROL_PLANE,
                json!({}),
                json!({"node_id": node_id, "agent_version": "0.1.0", "received_at": received_at}),
            )
            .expect("the control host answers")
            .body
    }

    fn observe(client: &Client, node_id: &str, cpus: u64) -> Value {
        client
            .command(
                "weft.infrastructure.RecordNodeObservation",
                steps::NODE_AGENT,
                json!({}),
                json!({
                    "node_id": node_id,
                    "capacity": [{"resource_class": "cpu", "amount": cpus}],
                    "isolation_facts": {
                        "kvm_present": false,
                        "cgroups_delegated": true,
                        "egress_denied": true,
                        "gpu_passthrough": false,
                        "levels": ["Container"],
                    },
                    "observed_at": timestamp(time::OffsetDateTime::now_utc()).0,
                }),
            )
            .expect("the control host answers")
            .body
    }

    fn at(text: &Value) -> time::OffsetDateTime {
        let text = text
            .as_str()
            .unwrap_or_else(|| panic!("not a timestamp: {text}"));
        instant(&Timestamp(text.to_owned())).unwrap_or_else(|| panic!("{text} is not RFC 3339"))
    }

    /// One raw HTTP exchange: the status and the body of the answer.
    fn raw(host: &ControlHost, request: &str) -> (u16, String) {
        let mut stream = TcpStream::connect(host.address()).expect("the control host listens");
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .expect("a read timeout");
        stream
            .write_all(request.as_bytes())
            .expect("the request is sent");
        let mut answer = String::new();
        let _ = stream.read_to_string(&mut answer);
        let status = answer
            .split_whitespace()
            .nth(1)
            .and_then(|status| status.parse().ok())
            .unwrap_or_else(|| panic!("not an HTTP answer: {answer:?}"));
        let body = answer
            .split_once("\r\n\r\n")
            .map_or("", |(_, body)| body)
            .to_owned();
        (status, body)
    }

    /// `ess/5-runtime/nodes.yaml` line 755 ("Decided in integration", matching
    /// `protocols/node-channel.yaml` `replace-open-session`): "a node that redials while control
    /// still holds its old session Open gets a new session, and the old one moves to
    /// Superseded. The redial is two commands in this order: SupersedeNodeSession ends the old
    /// session, then OpenNodeSession opens the new one." Line 805: "A redial over a still-Open
    /// session runs SupersedeNodeSession first." The endpoint sends only OpenNodeSession, so a
    /// restarted agent (nothing ends an Open session in this story: no heartbeat, no lease loss)
    /// is refused `already-open` on every redial, for good.
    #[test]
    fn a_hello_over_a_still_open_session_supersedes_it_and_opens_a_new_one() {
        let host = ControlHost::start();
        let client = host.client();
        let node = joined(&client);
        let first = hello(&client, &node.node_id);
        assert_eq!(first.status, 200, "{}", first.body);
        let old = first.body["session_id"]
            .as_str()
            .expect("a session_id")
            .to_owned();

        // The agent restarts and dials again.
        let again = hello(&client, &node.node_id);
        assert_eq!(
            again.status, 200,
            "a redial over the node's still-Open session is answered {} {}, not with a new session",
            again.status, again.body
        );
        let new = again.body["session_id"].as_str().expect("a session_id");
        assert_ne!(new, old);
        assert_eq!(
            row(&client, SESSIONS, "session_id", &old)["state"],
            json!("Superseded")
        );
        assert_eq!(
            row(&client, SESSIONS, "session_id", new)["state"],
            json!("Open")
        );
    }

    /// `weft-bin/src/lib.rs` `Host::execute`: "Its request line is still logged, as every
    /// answered request is"; the module doc: each request is "logged with the events it
    /// published". The node channel answers a frame it refuses itself (`400` a hello that is not
    /// a frame, `404` an unknown frame, `405` a hello that is not POST) without
    /// `Host::execute`, so no request line is written for it.
    #[test]
    fn a_frame_the_node_channel_refuses_has_its_request_line_in_the_log() {
        let host = ControlHost::start();
        let client = host.client();
        let before = host.last_seq();

        let not_a_frame = client
            .post("/node/hello", &json!("not a frame"))
            .expect("the control host answers");
        let unknown = client
            .post("/node/bye", &json!({}))
            .expect("the control host answers");
        let (get, _) = raw(
            &host,
            "GET /node/hello HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n",
        );
        let answered = vec![not_a_frame.status, unknown.status, get];
        assert_eq!(answered, vec![400, 404, 405]);

        let logged: Vec<u64> = host
            .log_after(before)
            .iter()
            .filter(|line| line["kind"] == "request")
            .filter_map(|line| line["status"].as_u64())
            .collect();
        assert_eq!(
            logged,
            vec![400, 404, 405],
            "the request lines of the three refused node-channel frames"
        );
    }

    /// `received-in-future` (`when: received_at > now`): no existing case sends a receipt
    /// instant after now, so dropping the guard leaves the suite green.
    #[test]
    fn a_receipt_instant_after_now_is_refused_and_opens_no_session() {
        let host = ControlHost::start();
        let client = host.client();
        let node = joined(&client);
        let later = timestamp(time::OffsetDateTime::now_utc() + time::Duration::hours(1)).0;
        let body = open_as_control_plane(&client, &node.node_id, &later);
        assert_eq!(body["outcome"], json!("received-in-future"), "{body}");
        assert!(rows(&client, SESSIONS).is_empty());
    }

    /// `identity-refused` for a node that is not "a joined node of the cluster": unknown,
    /// `Provisioning` and `Gone`. No existing case sends a hello for any of them, so
    /// `joined = true` leaves the suite green.
    #[test]
    fn a_hello_for_a_node_that_has_not_joined_is_identity_refused() {
        let gone = weft_impl::context::mint();
        let host = ControlHost::start_seeded(vec![node_row(
            &gone,
            weft_types::infrastructure::NodeState::Gone,
        )]);
        let client = host.client();
        let provisioning = provision(&client);
        let unknown = weft_impl::context::mint();
        for (what, node_id) in [
            ("unknown", unknown.as_str()),
            ("Provisioning", provisioning.node_id.as_str()),
            ("Gone", gone.as_str()),
        ] {
            let answer = hello(&client, node_id);
            assert_eq!(
                answer.body["outcome"],
                json!("identity-refused"),
                "{what}: {} {}",
                answer.status,
                answer.body
            );
        }
        assert!(rows(&client, SESSIONS).is_empty());
    }

    /// A seeded `weft.infrastructure.Node` row `node_id` in `state`.
    fn node_row(node_id: &str, state: weft_types::infrastructure::NodeState) -> weft_impl::Row {
        use weft_types::infrastructure::{NodeData, NodeRole, NodeSnapshot};
        use weft_types::primitives::Uuid;
        use weft_types::shared::{CloudId, NodeId, ZoneName};
        weft_impl::Row::Node(NodeSnapshot {
            state,
            data: NodeData {
                node_id: NodeId(Uuid(node_id.to_owned())),
                cloud_id: CloudId(Uuid(weft_impl::context::mint())),
                zone: ZoneName("z1".to_owned()),
                node_pool_id: None,
                role: NodeRole::Worker,
                join_token_expires_at: timestamp(time::OffsetDateTime::now_utc()),
                join_token_used: true,
                capacity: None,
                isolation_facts: None,
                observed_at: None,
            },
        })
    }

    /// `opened` sets `last_seen_at: input.received_at` and `opened_at: {generated: true}`
    /// (now); the existing cases read only `opened_at`, so writing the receipt instant into both
    /// leaves the suite green.
    #[test]
    fn a_session_is_last_seen_at_its_receipt_instant_and_opened_now() {
        let host = ControlHost::start();
        let client = host.client();
        let node = joined(&client);
        let received = time::OffsetDateTime::now_utc() - time::Duration::minutes(10);
        let before = time::OffsetDateTime::now_utc();
        let body = open_as_control_plane(&client, &node.node_id, &timestamp(received).0);
        let after = time::OffsetDateTime::now_utc();
        assert_eq!(body["outcome"], json!("opened"), "{body}");
        let session = row(&client, SESSIONS, "node_id", &node.node_id);
        assert_eq!(at(&session["last_seen_at"]), received, "{session}");
        let opened = at(&session["opened_at"]);
        assert!(before <= opened && opened <= after, "{session}");
    }

    /// No frame of the node carries a time (`protocols/node-channel.yaml`): a `received_at` in
    /// the hello frame is not read; the receipt instant is the control host's.
    #[test]
    fn a_receipt_instant_in_the_hello_frame_is_not_read() {
        let host = ControlHost::start();
        let client = host.client();
        let node = joined(&client);
        let before = time::OffsetDateTime::now_utc();
        let answer = client
            .post(
                "/node/hello",
                &json!({
                    "node_id": node.node_id,
                    "agent_version": "0.1.0",
                    "received_at": "2001-01-01T00:00:00Z",
                }),
            )
            .expect("the control host answers");
        let after = time::OffsetDateTime::now_utc();
        assert_eq!(answer.status, 200, "{}", answer.body);
        let session = row(&client, SESSIONS, "node_id", &node.node_id);
        let seen = at(&session["last_seen_at"]);
        assert!(before <= seen && seen <= after, "{session}");
    }

    /// `RecordNodeObservation` in each outcome this story decides: `no-such-node`, `not-joined`
    /// (Provisioning), `ready` (Joining), `observed` (Ready, Cordoned) with what was observed
    /// written, and a Cordoned node stays Cordoned. The existing cases reach only `ready`,
    /// through the agent.
    #[test]
    fn record_node_observation_takes_each_outcome_and_writes_what_was_observed() {
        let cordoned = weft_impl::context::mint();
        let host = ControlHost::start_seeded(vec![node_row(
            &cordoned,
            weft_types::infrastructure::NodeState::Cordoned,
        )]);
        let client = host.client();

        let unknown = observe(&client, &weft_impl::context::mint(), 1);
        assert_eq!(unknown["outcome"], json!("no-such-node"), "{unknown}");

        let cloud_id = steps::create_cloud(&client);
        steps::declare_zone(&client, &cloud_id, "z1");
        let pool = steps::create_node_pool(&client);
        let provisioning = steps::provision_pool_node(&client, &pool);
        let body = observe(&client, &provisioning.node_id, 1);
        assert_eq!(body["outcome"], json!("not-joined"), "{body}");
        let untouched = row(&client, NODES, "node_id", &provisioning.node_id);
        assert_eq!(untouched["state"], json!("Provisioning"), "{untouched}");
        assert!(untouched["capacity"].is_null(), "{untouched}");

        let node = steps::provision_pool_node(&client, &pool);
        let body = steps::join_node(&client, &node.node_id, &node.join_token);
        assert_eq!(body["outcome"], json!("joined"), "{body}");
        let body = observe(&client, &node.node_id, 2);
        assert_eq!(body["outcome"], json!("ready"), "{body}");
        let body = observe(&client, &node.node_id, 7);
        assert_eq!(body["outcome"], json!("observed"), "{body}");
        let ready = row(&client, NODES, "node_id", &node.node_id);
        assert_eq!(ready["state"], json!("Ready"), "{ready}");
        assert_eq!(ready["capacity"][0]["amount"], json!(7), "{ready}");

        let body = observe(&client, &cordoned, 3);
        assert_eq!(body["outcome"], json!("observed"), "{body}");
        let kept = row(&client, NODES, "node_id", &cordoned);
        assert_eq!(kept["state"], json!("Cordoned"), "{kept}");
        assert_eq!(kept["capacity"][0]["amount"], json!(3), "{kept}");
    }
}
