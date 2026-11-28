//! The one integration-test binary of weft-testkit.

mod ratchet;

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
    pub(super) fn row(answer: &Value, key: &str, id: &str) -> Value {
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
        steps::configure_cloud(&client, &cloud_id);
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
        let scope_id = steps::record_account_scope(&client, &account_id);
        let quota_id = steps::grant_top_quota(&client, &account_id);
        let ids = steps::Path {
            cloud_id,
            node_pool_id,
            node,
            cluster_id,
            tenant_id,
            account_id,
            scope_id,
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
        let scope_id = steps::record_account_scope(&client, &account_id);
        let clusters = || client.view("weft.cluster.Clusters", &[]).ok_body(200)["rows"].clone();
        let before = clusters();

        let answer = client
            .command(
                "weft.cluster.CreateCluster",
                "weft.actors.TenantOwner",
                steps::account_caller(&tenant_id, &account_id, &scope_id),
                json!({
                    "scope_id": scope_id,
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

/// The workload definitions of the skeleton path (`story:skeleton-workload-definitions`): after
/// the operator and tenant steps, the formation, its component, revision and environment, the
/// artifact, a runtime, the workload and its service, read back through their views; then each
/// guard of the seven obligations, in the specification's declaration order.
mod definitions {
    use serde_json::{json, Value};
    use weft_client::Expect;
    use weft_testkit::steps::{self, input};
    use weft_testkit::ControlHost;
    use weft_types::behaviour::EnvironmentStorage;
    use weft_types::primitives::Uuid;
    use weft_types::shared::EnvironmentId;

    use super::control_host::row;

    /// The rows of a view answer.
    fn rows(answer: &Value) -> Vec<Value> {
        answer["rows"]
            .as_array()
            .unwrap_or_else(|| panic!("no rows in {answer}"))
            .clone()
    }

    /// Statements 1 and 2: each row read back by the identity its command answered, with the
    /// values sent.
    #[test]
    fn the_definitions_read_back_with_the_identities_answered_and_the_values_sent() {
        let mut host = ControlHost::start();
        let client = host.client();
        let path = steps::run_path(&client);
        let ids = steps::run_definitions(&client, &path);
        let view = |name: &str, params: &[(&str, &str)]| client.view(name, params).ok_body(200);

        // 1. The formation, pinned to the default cluster, its component, revision and environment,
        // and the artifact.
        let formations = view(
            "weft.architecture.Formations",
            &[("scope_id", &path.scope_id)],
        );
        let formation = row(&formations, "formation_id", &ids.formation_id);
        assert_eq!(
            (formation["name"].clone(), formation["scope_id"].clone()),
            (json!("site"), json!(path.scope_id)),
            "{formation}"
        );
        let cluster = row(
            &view("weft.cluster.Clusters", &[]),
            "cluster_id",
            &path.cluster_id,
        );
        assert_eq!(
            cluster["formations"],
            json!([ids.formation_id]),
            "{cluster}"
        );
        let in_formation = [("formation_id", ids.formation_id.as_str())];
        let components = view("weft.architecture.FormationComponents", &in_formation);
        let component = row(&components, "component_id", &ids.component_id);
        assert_eq!(component["name"], json!("site"), "{component}");
        let revisions = view("weft.architecture.FormationRevisions", &in_formation);
        let revision = row(&revisions, "revision_id", &ids.revision_id);
        assert_eq!(
            revision["formation_id"],
            json!(ids.formation_id),
            "{revision}"
        );
        let environments = view("weft.architecture.Environments", &in_formation);
        let environment = row(&environments, "environment_id", &ids.environment_id);
        assert_eq!(environment["name"], json!("prod"), "{environment}");
        let artifacts = view("weft.artifacts.Artifacts", &[]);
        let artifact = row(&artifacts, "artifact_digest", &ids.artifact_digest);
        assert_eq!(
            artifact["artifact_digest"],
            json!(input::ARTIFACT_DIGEST),
            "{artifact}"
        );
        // `Artifacts` has no `kind`; the scope's `ArtifactList` shows it.
        let listed = view(
            "weft.artifacts.ArtifactList",
            &[("scope_id", &path.scope_id)],
        );
        let artifact = row(&listed, "artifact_digest", &ids.artifact_digest);
        assert_eq!(artifact["kind"], json!("Snippet"), "{artifact}");

        // 2. The workload of the Snippet runtime with its one Main process, and its service.
        let workloads = view("weft.workloads.WorkloadList", &in_formation);
        let workload = row(&workloads, "workload_id", &ids.workload_id);
        assert_eq!(workload["package_kind"], json!("Snippet"), "{workload}");
        // `WorkloadList` has no `processes`; the workload's `WorkloadDetail` shows them.
        let detail = view(
            "weft.workloads.WorkloadDetail",
            &[("workload_id", &ids.workload_id)],
        );
        let detail = row(&detail, "workload_id", &ids.workload_id);
        let processes = rows(&json!({"rows": detail["processes"]}));
        assert_eq!(processes.len(), 1, "{detail}");
        assert_eq!(
            (
                processes[0]["role"].clone(),
                processes[0]["command"].clone()
            ),
            (json!("Main"), json!(["sh", "-c", "echo weft"])),
            "{detail}"
        );
        assert_eq!(detail["runtime_id"], json!(ids.runtime_id), "{detail}");
        let services = view("weft.workloads.Services", &[]);
        let service = row(&services, "service_id", &ids.service_id);
        assert_eq!(
            (service["replicas"].clone(), service["workload_id"].clone()),
            (json!(1), json!(ids.workload_id)),
            "{service}"
        );

        // `protected` is in no view of the environment: read through the storage port.
        host.stop();
        let ports = weft_impl::Ports::open(&host.state_path()).expect("the state file opens");
        let environment =
            EnvironmentStorage::get(&ports, &EnvironmentId(Uuid(ids.environment_id.clone())))
                .expect("the environment row");
        assert!(!environment.data.protected, "{environment:?}");
    }

    /// Statement 3: a runtime that does not run `Snippet` is refused `runtime-kind` and adds no
    /// workload; the same request naming the `Snippet` runtime is accepted.
    #[test]
    fn define_workload_naming_a_runtime_without_snippet_answers_runtime_kind_and_adds_no_workload()
    {
        let host = ControlHost::start();
        let client = host.client();
        let path = steps::run_path(&client);
        let caller = steps::path_caller(&path);
        let scope_id = path.scope_id.as_str();
        let formation_id = steps::create_formation(&client, &caller, scope_id);
        steps::add_component(&client, &caller, &formation_id, scope_id);
        let revision_id = steps::commit_revision(&client, &caller, &formation_id, scope_id);
        let environment_id = steps::create_environment(&client, &caller, &formation_id, scope_id);
        steps::register_artifact(&client, &caller, scope_id);
        let image_runtime = steps::register_runtime(&client, &["Image"]);
        let snippet_runtime = steps::register_runtime(&client, &["Snippet"]);
        let request = |runtime_id: &str| {
            input::define_workload(input::WorkloadOf {
                scope_id,
                formation_id: &formation_id,
                formation_revision_id: &revision_id,
                environment_id: &environment_id,
                runtime_id,
            })
        };
        let workloads = || {
            rows(
                &client
                    .view(
                        "weft.workloads.WorkloadList",
                        &[("formation_id", &formation_id)],
                    )
                    .ok_body(200),
            )
        };

        let refused = client
            .command(
                "weft.workloads.DefineWorkload",
                steps::DEPLOYER,
                caller.clone(),
                request(&image_runtime),
            )
            .expect("the control host answers");
        assert_eq!(
            refused.body["outcome"],
            json!("runtime-kind"),
            "{}",
            refused.body
        );
        assert_eq!(workloads(), Vec::<Value>::new());

        let workload_id = steps::define_workload(&client, &caller, request(&snippet_runtime));
        let listed = workloads();
        assert_eq!(listed.len(), 1, "{listed:?}");
        assert_eq!(listed[0]["workload_id"], json!(workload_id));
    }

    /// Each guard of the seven obligations, in declaration order, against a control host in
    /// this process: its storage port arranges what no command of the path reaches (a retired
    /// formation, a deleted cluster, a closed account, a revoked artifact, a retired runtime or
    /// workload, another account's cluster or workload).
    mod guards {
        use std::net::TcpListener;
        use std::sync::Arc;

        use serde_json::{json, Value};
        use weft_client::Client;
        use weft_impl::context::mint;
        use weft_impl::Ports;
        use weft_testkit::steps::{self, input, Definitions, Path};
        use weft_types::behaviour::{
            AccountStorage, ArtifactStorage, ClusterStorage, ComponentStorage, EnvironmentStorage,
            FormationRevisionStorage, FormationStorage, QuotaStorage, RuntimeStorage, ScopeStorage,
            ServiceStorage, WorkloadStorage,
        };
        use weft_types::primitives::Uuid;
        use weft_types::shared::{
            AccountId, ArtifactDigest, ClusterId, EnvironmentId, FormationId, RuntimeId, ScopeId,
            WorkloadId,
        };
        use weft_types::workloads::obligations::CreateInstanceBehavior;

        const CREATE_FORMATION: &str = "weft.architecture.CreateFormation";
        const ADD_COMPONENT: &str = "weft.architecture.AddComponent";
        const COMMIT_REVISION: &str = "weft.architecture.CommitRevision";
        const CREATE_ENVIRONMENT: &str = "weft.architecture.CreateEnvironment";
        const REGISTER_ARTIFACT: &str = "weft.artifacts.RegisterArtifact";
        const DEFINE_WORKLOAD: &str = "weft.workloads.DefineWorkload";
        const DEFINE_SERVICE: &str = "weft.workloads.DefineService";
        const PIN_FORMATION: &str = "weft.cluster.PinFormation";
        const RECORD_ACCOUNT_SCOPE: &str = "weft.accounts.RecordAccountScope";

        /// The path and the definitions of [`steps::run_definitions`] in account `acme` and its
        /// scope, and a second tenant `globex` with its account and scope, on a control host served
        /// from this process.
        struct World {
            host: Arc<weft_bin::Host>,
            client: Client,
            path: Path,
            ids: Definitions,
            /// A caller acting in `acme`'s account.
            caller: Value,
            /// The scope of `globex`'s account.
            other_scope: String,
            /// A caller acting in `globex`'s account.
            other: Value,
            _dir: weft_testkit::TempDir,
        }

        impl World {
            fn new() -> Self {
                let dir = weft_testkit::TempDir::new();
                let host = Arc::new(
                    weft_bin::Host::open(
                        &dir.path().join("state.db"),
                        &dir.path().join("control-host.log"),
                    )
                    .expect("the host opens"),
                );
                let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port binds");
                let client = Client::new(listener.local_addr().expect("the listener's address"));
                let serving = Arc::clone(&host);
                std::thread::spawn(move || serving.serve(&listener, true));
                let path = steps::run_path(&client);
                let ids = steps::run_definitions(&client, &path);
                let caller = steps::path_caller(&path);
                let other_tenant = steps::create_tenant(&client, "globex");
                let other_account = steps::create_account(&client, &other_tenant);
                let other_scope = steps::record_account_scope(&client, &other_account);
                let other = steps::account_caller(&other_tenant, &other_account, &other_scope);
                Self {
                    host,
                    client,
                    path,
                    ids,
                    caller,
                    other_scope,
                    other,
                    _dir: dir,
                }
            }

            /// The scope of `acme`'s account.
            fn scope(&self) -> &str {
                &self.path.scope_id
            }

            fn ports(&self) -> Ports {
                self.host.ports().clone()
            }

            /// The outcome `command` answers to `input` sent as `actor` with `caller`.
            fn outcome(&self, actor: &str, caller: &Value, command: &str, input: &Value) -> String {
                let answer = self
                    .client
                    .command(command, actor, caller.clone(), input.clone())
                    .expect("the control host answers");
                answer.body["outcome"]
                    .as_str()
                    .unwrap_or_else(|| {
                        panic!(
                            "{command} {input}: no outcome in {} {}",
                            answer.status, answer.body
                        )
                    })
                    .to_owned()
            }

            /// `input` answers the refusal `outcome`, and the rows `count` counts stay as many.
            fn refuses(
                &self,
                actor: &str,
                caller: &Value,
                command: &str,
                input: &Value,
                outcome: &str,
                count: fn(&Ports) -> usize,
            ) {
                let before = count(&self.ports());
                assert_eq!(
                    self.outcome(actor, caller, command, input),
                    outcome,
                    "{command} {input}"
                );
                assert_eq!(
                    count(&self.ports()),
                    before,
                    "{command} {input} wrote a row"
                );
            }

            /// `input` answers the accepting `outcome`: the body.
            fn accepts(
                &self,
                actor: &str,
                caller: &Value,
                command: &str,
                input: &Value,
                outcome: &str,
            ) -> Value {
                let answer = self
                    .client
                    .command(command, actor, caller.clone(), input.clone())
                    .expect("the control host answers");
                assert_eq!(
                    answer.body["outcome"],
                    json!(outcome),
                    "{command} {input}: {}",
                    answer.body
                );
                answer.body
            }

            /// `input` of `command` sent by a Deployer of `globex`'s account on a formation of
            /// `acme`: refused as a missing formation (`NoSuchFormation`), and the formations,
            /// components and revisions are as they were.
            fn isolated(&self, command: &str, input: &Value) {
                let rows = |ports: &Ports| {
                    format!(
                        "{:?} {:?} {:?}",
                        FormationStorage::list(ports),
                        ComponentStorage::list(ports),
                        FormationRevisionStorage::list(ports)
                    )
                };
                let before = rows(&self.ports());
                let answer = self
                    .client
                    .command(command, steps::DEPLOYER, self.other.clone(), input.clone())
                    .expect("the control host answers");
                let outcome = answer.body["outcome"].as_str().unwrap_or_default();
                assert!(
                    ["not-callers-scope", "formation-of-other-scope"].contains(&outcome),
                    "{command} {input}: {} {}",
                    answer.status,
                    answer.body
                );
                assert!(
                    answer.body.to_string().contains("NoSuchFormation"),
                    "{command} {input}: {}",
                    answer.body
                );
                assert_eq!(
                    rows(&self.ports()),
                    before,
                    "{command} {input} changed a row"
                );
            }

            /// A formation of `acme` named `name` in the default cluster, with one component when
            /// `component`: its identity.
            fn formation(&self, name: &str, component: bool) -> String {
                let input =
                    json!({"scope_id": self.scope(), "name": name, "zones": [], "parity": []});
                let body = self.accepts(
                    steps::ACCOUNT_ADMIN,
                    &self.caller,
                    CREATE_FORMATION,
                    &input,
                    "created",
                );
                let id = published(&body, "formation_id");
                if component {
                    steps::add_component(&self.client, &self.caller, &id, self.scope());
                }
                id
            }

            /// `AddComponent` of a component `name` to `acme`'s formation `formation`.
            fn component(&self, formation: &str, name: &str) {
                let input =
                    json!({"formation_id": formation, "scope_id": self.scope(), "name": name});
                self.accepts(
                    steps::DEPLOYER,
                    &self.caller,
                    ADD_COMPONENT,
                    &input,
                    "added",
                );
            }

            /// A copy of environment `of` under a new identity, changed by `change`: its
            /// identity.
            fn environment(
                &self,
                of: &str,
                change: impl FnOnce(&mut weft_types::architecture::EnvironmentSnapshot),
            ) -> String {
                let mut ports = self.ports();
                let key = EnvironmentId(Uuid(of.to_owned()));
                let mut row = EnvironmentStorage::get(&ports, &key).expect("the environment row");
                let id = mint();
                row.data.environment_id = EnvironmentId(Uuid(id.clone()));
                change(&mut row);
                EnvironmentStorage::put(&mut ports, row);
                id
            }

            /// Moves formation `id` to `Retired` through the storage port.
            fn retire_formation(&self, id: &str) {
                let mut ports = self.ports();
                let key = FormationId(Uuid(id.to_owned()));
                let mut row = FormationStorage::get(&ports, &key).expect("the formation row");
                row.state = weft_types::architecture::FormationState::Retired;
                FormationStorage::put(&mut ports, row);
            }

            /// A copy of the default cluster under a new identity, changed by `change`: its
            /// identity.
            fn cluster(
                &self,
                change: impl FnOnce(&mut weft_types::cluster::ClusterSnapshot),
            ) -> String {
                let mut ports = self.ports();
                let key = ClusterId(Uuid(self.path.cluster_id.clone()));
                let mut row = ClusterStorage::get(&ports, &key).expect("the default cluster row");
                let id = mint();
                row.data.cluster_id = ClusterId(Uuid(id.clone()));
                change(&mut row);
                ClusterStorage::put(&mut ports, row);
                id
            }

            /// A copy of workload `of` under a new identity, changed by `change`: its identity.
            fn workload(
                &self,
                of: &str,
                change: impl FnOnce(&mut weft_types::workloads::WorkloadSnapshot),
            ) -> String {
                let mut ports = self.ports();
                let key = WorkloadId(Uuid(of.to_owned()));
                let mut row = WorkloadStorage::get(&ports, &key).expect("the workload row");
                let id = mint();
                row.data.workload_id = WorkloadId(Uuid(id.clone()));
                change(&mut row);
                WorkloadStorage::put(&mut ports, row);
                id
            }

            /// The request of the skeleton's workload in `acme`'s formation `site`.
            fn workload_request(&self) -> Value {
                input::define_workload(input::WorkloadOf {
                    scope_id: self.scope(),
                    formation_id: &self.ids.formation_id,
                    formation_revision_id: &self.ids.revision_id,
                    environment_id: &self.ids.environment_id,
                    runtime_id: &self.ids.runtime_id,
                })
            }

            /// `RegisterArtifact` of `digest` of `kind` by the account `account_id` with
            /// `caller`.
            fn artifact(&self, caller: &Value, account_id: &str, digest: &str, kind: &str) {
                let mut input = input::register_artifact(account_id);
                input["artifact_digest"] = json!(digest);
                input["kind"] = json!(kind);
                self.accepts(
                    steps::DEPLOYER,
                    caller,
                    REGISTER_ARTIFACT,
                    &input,
                    "registered",
                );
            }

            /// `RegisterRuntime` of `kinds` at minimum isolation `minimum`: its identity.
            fn runtime(&self, kinds: &[&str], minimum: &str) -> String {
                let mut input = input::register_runtime(kinds);
                input["minimum_isolation"] = json!(minimum);
                let body = self.accepts(
                    steps::OPERATOR,
                    &json!({}),
                    "weft.workloads.RegisterRuntime",
                    &input,
                    "registered",
                );
                published(&body, "runtime_id")
            }
        }

        /// The member `field` of the first published event's payload.
        fn published(body: &Value, field: &str) -> String {
            body["published"][0]["payload"][field]
                .as_str()
                .unwrap_or_else(|| panic!("no {field} in the published payload of {body}"))
                .to_owned()
        }

        /// `base` with the member at `pointer` (a JSON pointer) replaced by `value`.
        fn with(base: &Value, pointer: &str, value: Value) -> Value {
            let mut changed = base.clone();
            *changed
                .pointer_mut(pointer)
                .unwrap_or_else(|| panic!("no {pointer} in {base}")) = value;
            changed
        }

        fn formations(ports: &Ports) -> usize {
            FormationStorage::list(ports).len()
        }

        fn components(ports: &Ports) -> usize {
            ComponentStorage::list(ports).len()
        }

        fn revisions(ports: &Ports) -> usize {
            FormationRevisionStorage::list(ports).len()
        }

        fn environments(ports: &Ports) -> usize {
            EnvironmentStorage::list(ports).len()
        }

        fn artifacts(ports: &Ports) -> usize {
            ArtifactStorage::list(ports).len()
        }

        fn workloads(ports: &Ports) -> usize {
            WorkloadStorage::list(ports).len()
        }

        fn services(ports: &Ports) -> usize {
            ServiceStorage::list(ports).len()
        }

        /// The pins of every cluster: how many formations the clusters list together.
        fn pinned(ports: &Ports) -> usize {
            ClusterStorage::list(ports)
                .iter()
                .map(|cluster| cluster.data.formations.len())
                .sum()
        }

        #[test]
        fn create_formation_answers_each_guard_in_declaration_order() {
            let w = World::new();
            let refuses = |caller: &Value, input: Value, outcome: &str| {
                w.refuses(
                    steps::ACCOUNT_ADMIN,
                    caller,
                    CREATE_FORMATION,
                    &input,
                    outcome,
                    formations,
                );
            };
            let request = |scope: &str, name: &str| json!({"scope_id": scope, "name": name, "zones": ["z1"], "parity": []});
            let with_parity = |scope: &str, name: &str, parity: Value| {
                with(&request(scope, name), "/parity", parity)
            };

            // no-such-scope: a scope no row holds, the caller acting in it; and a closed one.
            let nobody = mint();
            let nobody_caller = steps::account_caller(&w.path.tenant_id, &mint(), &nobody);
            refuses(&nobody_caller, request(&nobody, "a"), "no-such-scope");
            let closing_tenant = steps::create_tenant(&w.client, "initech");
            let closing_account = steps::create_account(&w.client, &closing_tenant);
            let closing = steps::record_account_scope(&w.client, &closing_account);
            let mut ports = w.ports();
            let key = ScopeId(Uuid(closing.clone()));
            let mut scope = ScopeStorage::get(&ports, &key).expect("the scope row");
            scope.state = weft_types::scope::ScopeState::Closed;
            ScopeStorage::put(&mut ports, scope);
            let closing_caller = steps::account_caller(&closing_tenant, &closing_account, &closing);
            refuses(&closing_caller, request(&closing, "a"), "no-such-scope");

            // not-callers-scope.
            refuses(&w.other, request(w.scope(), "a"), "not-callers-scope");
            let one_scope = json!([{"scopes": [w.scope()], "facts": ["rustc"]}]);
            refuses(
                &w.other,
                with_parity(w.scope(), "a", one_scope.clone()),
                "not-callers-scope",
            );

            // parity-form: a parity of fewer than two scopes or of no fact, after
            // not-callers-scope and ahead of name-taken (`site` is taken).
            let two_scopes = json!([w.scope(), w.other_scope]);
            refuses(
                &w.caller,
                with_parity(w.scope(), "a", one_scope.clone()),
                "parity-form",
            );
            refuses(
                &w.caller,
                with_parity(
                    w.scope(),
                    "a",
                    json!([{"scopes": two_scopes.clone(), "facts": []}]),
                ),
                "parity-form",
            );
            refuses(
                &w.caller,
                with_parity(w.scope(), "site", one_scope),
                "parity-form",
            );

            let body = w.accepts(
                steps::ACCOUNT_ADMIN,
                &w.caller,
                CREATE_FORMATION,
                &with_parity(
                    w.scope(),
                    "own",
                    json!([{"scopes": two_scopes, "facts": ["rustc"]}]),
                ),
                "created",
            );

            // The row: the values sent, its internal domain, Active, in no cluster yet.
            let id = FormationId(Uuid(published(&body, "formation_id")));
            let row = FormationStorage::get(&w.ports(), &id).expect("the formation row");
            assert_eq!(row.state, weft_types::architecture::FormationState::Active);
            assert_eq!(row.data.internal_domain, "own.internal");
            assert_eq!(row.data.scope_id, ScopeId(Uuid(w.scope().to_owned())));
            assert_eq!(
                row.data.zones,
                vec![weft_types::shared::ZoneName("z1".to_owned())]
            );
            assert!(row.data.quota_slice.is_empty());
            assert_eq!(row.data.parity.len(), 1);
            assert_eq!(row.data.parity[0].scopes.len(), 2);
            assert_eq!(row.data.parity[0].facts, vec!["rustc".to_owned()]);
            assert!(
                !ClusterStorage::list(&w.ports())
                    .iter()
                    .any(|cluster| cluster.data.formations.contains(&id)),
                "a new formation is pinned to no cluster"
            );

            // name-taken: `site` again in `acme`; in `globex`'s scope the name is free.
            refuses(&w.caller, request(w.scope(), "site"), "name-taken");
            w.accepts(
                steps::ACCOUNT_ADMIN,
                &w.other,
                CREATE_FORMATION,
                &request(&w.other_scope, "site"),
                "created",
            );
        }

        /// The guards the blocks cut from `CreateFormation` (the formation's cluster) are
        /// `PinFormation`'s, each denying by default.
        #[test]
        fn pin_formation_answers_each_guard_in_declaration_order() {
            let w = World::new();
            let refuses = |caller: &Value, input: Value, outcome: &str| {
                w.refuses(
                    steps::ACCOUNT_ADMIN,
                    caller,
                    PIN_FORMATION,
                    &input,
                    outcome,
                    pinned,
                );
            };
            let request = |scope: &str, cluster: &str, formation: &str| json!({"scope_id": scope, "cluster_id": cluster, "formation_id": formation});
            let default = w.path.cluster_id.clone();
            let fresh = w.formation("fresh", false);

            // not-callers-account: the caller acts in another scope.
            refuses(
                &w.other,
                request(w.scope(), &default, &fresh),
                "not-callers-account",
            );
            // no-such-cluster, no-such-formation.
            refuses(
                &w.caller,
                request(w.scope(), &mint(), &fresh),
                "no-such-cluster",
            );
            refuses(
                &w.caller,
                request(w.scope(), &default, &mint()),
                "no-such-formation",
            );
            // formation-foreign: `globex`'s formation named from `globex`'s scope... by `acme`.
            let globex = steps::create_formation(&w.client, &w.other, &w.other_scope);
            refuses(
                &w.caller,
                request(w.scope(), &default, &globex),
                "formation-foreign",
            );
            // cluster-foreign: a cluster of another scope, and a deleted one.
            let other_scope = ScopeId(Uuid(w.other_scope.clone()));
            let foreign = w.cluster(|row| row.data.scope_id = Some(other_scope));
            refuses(
                &w.caller,
                request(w.scope(), &foreign, &fresh),
                "cluster-foreign",
            );
            let deleted = w.cluster(|row| row.state = weft_types::cluster::ClusterState::Deleted);
            refuses(
                &w.caller,
                request(w.scope(), &deleted, &fresh),
                "cluster-foreign",
            );
            // formation-retired.
            let retired = w.formation("retired", false);
            w.retire_formation(&retired);
            refuses(
                &w.caller,
                request(w.scope(), &default, &retired),
                "formation-retired",
            );
            // already-pinned: `site` runs in the default cluster; another cluster of its own
            // scope does not take it a second time.
            let acme = ScopeId(Uuid(w.scope().to_owned()));
            let own = w.cluster(|row| {
                row.data.scope_id = Some(acme);
                row.data.formations.clear();
            });
            refuses(
                &w.caller,
                request(w.scope(), &own, &w.ids.formation_id),
                "already-pinned",
            );

            // pinned: into a cluster of its own scope; the cluster lists it.
            let body = w.accepts(
                steps::ACCOUNT_ADMIN,
                &w.caller,
                PIN_FORMATION,
                &request(w.scope(), &own, &fresh),
                "pinned",
            );
            assert_eq!(
                body["published"][0]["payload"],
                json!({"cluster_id": own, "formation_id": fresh}),
                "{body}"
            );
            let row =
                ClusterStorage::get(&w.ports(), &ClusterId(Uuid(own))).expect("the cluster row");
            assert_eq!(row.data.formations, vec![FormationId(Uuid(fresh))]);
        }

        /// `RecordAccountScope` records one scope on one account, both ways unique.
        #[test]
        fn record_account_scope_answers_each_guard_in_declaration_order() {
            let w = World::new();
            let record = |account: &str, scope: &str| {
                w.outcome(
                    steps::CONTROL_PLANE,
                    &json!({}),
                    RECORD_ACCOUNT_SCOPE,
                    &json!({"account_id": account, "scope_id": scope}),
                )
            };
            let tenant = steps::create_tenant(&w.client, "initech");
            let account = steps::create_account(&w.client, &tenant);
            let opened = ScopeStorage::list(&w.ports())
                .into_iter()
                .find(|scope| scope.data.name == account)
                .expect("account-opens-scope opened the account's scope");
            let opened = opened.data.scope_id.0 .0;
            let held = |account: &str| {
                AccountStorage::get(&w.ports(), &AccountId(Uuid(account.to_owned())))
                    .expect("the account row")
                    .data
                    .scope_id
            };

            assert_eq!(record(&mint(), &opened), "no-such-account");
            assert_eq!(record(&account, &mint()), "no-such-scope");
            // scope-taken: `acme`'s scope is held by `acme`'s account.
            assert_eq!(record(&account, w.scope()), "scope-taken");
            assert_eq!(held(&account), None);

            assert_eq!(record(&account, &opened), "recorded");
            assert_eq!(held(&account), Some(ScopeId(Uuid(opened.clone()))));
            // scope-taken: the account holds a scope already, the same one or another.
            assert_eq!(record(&account, &opened), "scope-taken");
            assert_eq!(record(&account, &w.other_scope), "scope-taken");
            assert_eq!(held(&account), Some(ScopeId(Uuid(opened))));
        }

        #[test]
        fn add_component_answers_each_guard_in_declaration_order() {
            let w = World::new();
            let refuses = |caller: &Value, input: Value, outcome: &str| {
                w.refuses(
                    steps::DEPLOYER,
                    caller,
                    ADD_COMPONENT,
                    &input,
                    outcome,
                    components,
                );
            };
            let request = |formation: &str, account: &str, name: &str| json!({"formation_id": formation, "scope_id": account, "name": name});
            let site = w.ids.formation_id.clone();

            refuses(
                &w.other,
                request(&site, w.scope(), "a"),
                "not-callers-scope",
            );
            refuses(
                &w.caller,
                request(&mint(), w.scope(), "a"),
                "no-such-formation",
            );
            let other_account = w.other_scope.clone();
            refuses(
                &w.other,
                request(&site, &other_account, "a"),
                "formation-of-other-scope",
            );
            let retired = w.formation("retired", true);
            w.retire_formation(&retired);
            refuses(&w.caller, request(&retired, w.scope(), "a"), "retired");
            refuses(&w.caller, request(&site, w.scope(), "site"), "name-taken");

            // In another formation the name is free; the row has its internal name.
            let other = w.formation("other", false);
            let body = w.accepts(
                steps::DEPLOYER,
                &w.caller,
                ADD_COMPONENT,
                &request(&other, w.scope(), "site"),
                "added",
            );
            assert_eq!(
                body["published"][0]["payload"]["internal_name"],
                json!("site.other.internal"),
                "{body}"
            );
            let id = weft_types::shared::ComponentId(Uuid(published(&body, "component_id")));
            let row = ComponentStorage::get(&w.ports(), &id).expect("the component row");
            assert_eq!(row.data.internal_name.0, "site.other.internal");
            assert!(!row.data.on_border);
        }

        #[test]
        fn commit_revision_answers_each_guard_and_numbers_its_revisions() {
            let w = World::new();
            let refuses = |caller: &Value, input: Value, outcome: &str| {
                w.refuses(
                    steps::DEPLOYER,
                    caller,
                    COMMIT_REVISION,
                    &input,
                    outcome,
                    revisions,
                );
            };
            let request = |formation: &str, account: &str| json!({"formation_id": formation, "scope_id": account});
            let site = w.ids.formation_id.clone();

            refuses(&w.other, request(&site, w.scope()), "not-callers-scope");
            refuses(&w.caller, request(&mint(), w.scope()), "no-such-formation");
            let other_account = w.other_scope.clone();
            refuses(
                &w.other,
                request(&site, &other_account),
                "formation-of-other-scope",
            );
            let retired = w.formation("retired", true);
            w.retire_formation(&retired);
            refuses(&w.caller, request(&retired, w.scope()), "retired");
            let bare = w.formation("bare", false);
            refuses(&w.caller, request(&bare, w.scope()), "empty");

            // The first revision of `site` froze it before environment `prod`: the second,
            // number 2, has another digest; the third, of the same document, the second's; a
            // component more makes number 4 with another digest again.
            let commit = |formation: &str| {
                let body = w.accepts(
                    steps::DEPLOYER,
                    &w.caller,
                    COMMIT_REVISION,
                    &request(formation, w.scope()),
                    "committed",
                );
                let payload = body["published"][0]["payload"].clone();
                (
                    payload["number"].clone(),
                    payload["document_digest"].clone(),
                )
            };
            let first = FormationRevisionStorage::get(
                &w.ports(),
                &weft_types::shared::FormationRevisionId(Uuid(w.ids.revision_id.clone())),
            )
            .expect("the first revision row");
            assert_eq!(first.data.number, 1);
            let (number, digest) = commit(&w.ids.formation_id);
            assert_eq!(number, json!(2));
            assert_ne!(digest, json!(first.data.document_digest.0));
            assert_eq!(commit(&w.ids.formation_id), (json!(3), digest.clone()));
            steps::add_component(&w.client, &w.caller, &bare, w.scope());
            w.accepts(
                steps::DEPLOYER,
                &w.caller,
                ADD_COMPONENT,
                &json!({"formation_id": w.ids.formation_id, "scope_id": w.scope(), "name": "api"}),
                "added",
            );
            let (number, fourth) = commit(&w.ids.formation_id);
            assert_eq!(number, json!(4));
            assert_ne!(fourth, digest);
            assert!(
                fourth
                    .as_str()
                    .is_some_and(|text| text.len() == 71 && text.starts_with("sha256:")),
                "{fourth}"
            );
            // Another formation starts at 1.
            assert_eq!(commit(&bare).0, json!(1));
        }

        /// `AddComponent` on `acme`'s formation by a Deployer of `globex`'s account, acting in its
        /// own account or naming `acme`'s: `NoSuchFormation`, and no row changes.
        #[test]
        fn a_deployer_of_another_account_gets_no_such_formation_adding_a_component() {
            let w = World::new();
            for account in [w.other_scope.clone(), w.scope().to_owned()] {
                let input = json!({"formation_id": w.ids.formation_id, "scope_id": account, "name": "intruder"});
                w.isolated(ADD_COMPONENT, &input);
            }
        }

        /// `CommitRevision` of `acme`'s formation by a Deployer of `globex`'s account, acting in
        /// its own account or naming `acme`'s: `NoSuchFormation`, and no row changes.
        #[test]
        fn a_deployer_of_another_account_gets_no_such_formation_committing_a_revision() {
            let w = World::new();
            for account in [w.other_scope.clone(), w.scope().to_owned()] {
                let input = json!({"formation_id": w.ids.formation_id, "scope_id": account});
                w.isolated(COMMIT_REVISION, &input);
            }
        }

        #[test]
        fn create_environment_answers_each_guard_in_declaration_order() {
            let w = World::new();
            let refuses = |caller: &Value, input: Value, outcome: &str| {
                w.refuses(
                    steps::ACCOUNT_ADMIN,
                    caller,
                    CREATE_ENVIRONMENT,
                    &input,
                    outcome,
                    environments,
                );
            };
            let request = |formation: &str, account: &str, name: &str| json!({"formation_id": formation, "scope_id": account, "name": name, "protected": true});
            let site = w.ids.formation_id.clone();

            refuses(
                &w.other,
                request(&site, w.scope(), "staging"),
                "not-callers-scope",
            );
            refuses(
                &w.caller,
                request(&mint(), w.scope(), "staging"),
                "no-such-formation",
            );
            let globex = steps::create_formation(&w.client, &w.other, &w.other_scope);
            refuses(
                &w.caller,
                request(&globex, w.scope(), "staging"),
                "formation-of-other-scope",
            );
            let retired = w.formation("retired", false);
            w.retire_formation(&retired);
            refuses(
                &w.caller,
                request(&retired, w.scope(), "staging"),
                "retired",
            );
            refuses(&w.caller, request(&site, w.scope(), "prod"), "name-taken");

            let body = w.accepts(
                steps::ACCOUNT_ADMIN,
                &w.caller,
                CREATE_ENVIRONMENT,
                &request(&site, w.scope(), "staging"),
                "created",
            );
            let id = weft_types::shared::EnvironmentId(Uuid(published(&body, "environment_id")));
            let row = EnvironmentStorage::get(&w.ports(), &id).expect("the environment row");
            assert!(row.data.protected);
            assert_eq!(row.data.scope_id, ScopeId(Uuid(w.scope().to_owned())));
            assert_eq!(
                row.data.origin,
                weft_types::architecture::EnvironmentOrigin::Manual
            );
            assert_eq!(row.data.name.0, "staging");
            assert_eq!(
                row.state,
                weft_types::architecture::EnvironmentState::Active
            );
        }

        #[test]
        fn register_artifact_answers_each_guard_in_declaration_order() {
            let w = World::new();
            let refuses = |caller: &Value, input: Value, outcome: &str| {
                w.refuses(
                    steps::DEPLOYER,
                    caller,
                    REGISTER_ARTIFACT,
                    &input,
                    outcome,
                    artifacts,
                );
            };
            let base = with(
                &input::register_artifact(w.scope()),
                "/artifact_digest",
                json!("sha256:next"),
            );

            refuses(&w.other, base.clone(), "not-callers-scope");
            refuses(&w.caller, with(&base, "/signature", json!("")), "unsigned");
            // Both unsigned and of a negative size: the first declared answers.
            let unsigned_negative = with(
                &with(&base, "/signature", json!("")),
                "/size_bytes",
                json!(-1),
            );
            refuses(&w.caller, unsigned_negative, "unsigned");
            refuses(
                &w.caller,
                with(&base, "/size_bytes", json!(-1)),
                "negative-size",
            );
            // bad-signature: no signing key is trusted yet; the decider takes it for none.
            refuses(
                &w.caller,
                with(&base, "/built_from", json!("sha256:none")),
                "built-from-missing",
            );
            w.artifact(&w.other, &w.other_scope, "sha256:globex", "Snippet");
            refuses(
                &w.caller,
                with(&base, "/built_from", json!("sha256:globex")),
                "built-from-foreign",
            );
            refuses(
                &w.caller,
                with(&base, "/artifact_digest", json!(input::ARTIFACT_DIGEST)),
                "exists",
            );
            // One digest for the whole cluster: another account's registration of it exists too.
            let theirs = with(
                &input::register_artifact(&w.other_scope),
                "/artifact_digest",
                json!(input::ARTIFACT_DIGEST),
            );
            refuses(&w.other, theirs, "exists");

            let built = with(&base, "/built_from", json!(input::ARTIFACT_DIGEST));
            w.accepts(
                steps::DEPLOYER,
                &w.caller,
                REGISTER_ARTIFACT,
                &built,
                "registered",
            );
            let row = ArtifactStorage::get(&w.ports(), &ArtifactDigest("sha256:next".to_owned()))
                .expect("the artifact row");
            assert_eq!(row.state, weft_types::artifacts::ArtifactState::Registered);
            assert_eq!(row.data.kind, weft_types::artifacts::ArtifactKind::Snippet);
            assert_eq!(
                row.data.built_from,
                Some(ArtifactDigest(input::ARTIFACT_DIGEST.to_owned()))
            );
            assert_eq!(row.data.scope_id, ScopeId(Uuid(w.scope().to_owned())));
        }

        #[test]
        fn define_workload_answers_each_guard_in_declaration_order() {
            let w = World::new();
            let refuses = |caller: &Value, input: Value, outcome: &str| {
                w.refuses(
                    steps::DEPLOYER,
                    caller,
                    DEFINE_WORKLOAD,
                    &input,
                    outcome,
                    workloads,
                );
            };
            let base = w.workload_request();
            let check = json!({
                "role": "Ready", "kind": "Tcp", "port": 8080, "path": null, "command": null,
                "service": null, "initial_delay_seconds": 0, "period_seconds": 10,
                "timeout_seconds": 1, "failure_threshold": 3, "success_threshold": 1,
            });
            let sidecar = with(&input::process(), "/role", json!("Sidecar"));

            // The input guards.
            refuses(&w.other, base.clone(), "not-callers-scope");
            refuses(
                &w.caller,
                with(&base, "/package_kind", json!("Agent")),
                "spec-kind",
            );
            refuses(
                &w.caller,
                with(&base, "/shape", json!("Box")),
                "box-package",
            );
            // run-as-missing: isolation None names the host user it runs as (default deny),
            // after box-package and ahead of no-main.
            let unsandboxed = with(&base, "/isolation", json!("None"));
            refuses(&w.caller, unsandboxed.clone(), "run-as-missing");
            refuses(
                &w.caller,
                with(&unsandboxed, "/processes", json!([])),
                "run-as-missing",
            );
            refuses(
                &w.caller,
                with(&unsandboxed, "/shape", json!("Box")),
                "box-package",
            );
            refuses(&w.caller, with(&base, "/processes", json!([])), "no-main");
            refuses(
                &w.caller,
                with(&base, "/processes", json!([sidecar])),
                "no-main",
            );
            let http_without_port =
                with(&with(&check, "/kind", json!("Http")), "/port", json!(null));
            let unchecked = with(&base, "/processes/0/checks", json!([http_without_port]));
            refuses(&w.caller, unchecked, "invalid-check");
            refuses(
                &w.caller,
                with(
                    &base,
                    "/processes/0/checks",
                    json!([with(&check, "/period_seconds", json!(0))]),
                ),
                "invalid-check",
            );
            refuses(
                &w.caller,
                with(&base, "/restart_policy/backoff_initial_seconds", json!(0)),
                "invalid-restart-policy",
            );
            refuses(
                &w.caller,
                with(&base, "/restart_policy/backoff_max_seconds", json!(0)),
                "invalid-restart-policy",
            );
            // Settings tagged with another shape: refused with the input refusals, ahead of
            // every row read (here, a formation that does not exist).
            let job_settings = json!({
                "shape": "job",
                "value": {"parallelism": 1, "gang": false, "max_retries": 0, "backoff": "PT1S"},
            });
            refuses(
                &w.caller,
                with(&base, "/settings", job_settings.clone()),
                "settings-shape-mismatch",
            );
            refuses(
                &w.caller,
                with(
                    &with(&base, "/settings", job_settings),
                    "/formation_id",
                    json!(mint()),
                ),
                "settings-shape-mismatch",
            );
            let daemon_settings = json!({"shape": "daemon", "value": {"node_selector": {}}});
            refuses(
                &w.caller,
                with(&base, "/settings", daemon_settings),
                "settings-shape-mismatch",
            );

            // Existence, in declaration order, ahead of every predicate: another account's
            // formation and no runtime answer `no-runtime`.
            let globex = steps::create_formation(&w.client, &w.other, &w.other_scope);
            steps::add_component(&w.client, &w.other, &globex, &w.other_scope);
            refuses(
                &w.caller,
                with(&base, "/formation_id", json!(mint())),
                "formation-missing",
            );
            refuses(
                &w.caller,
                with(&base, "/environment_id", json!(mint())),
                "environment-missing",
            );
            refuses(
                &w.caller,
                with(&base, "/formation_revision_id", json!(mint())),
                "revision-missing",
            );
            refuses(
                &w.caller,
                with(&base, "/runtime_id", json!(mint())),
                "no-runtime",
            );
            let foreign_without_runtime = with(
                &with(&base, "/formation_id", json!(globex)),
                "/runtime_id",
                json!(mint()),
            );
            refuses(&w.caller, foreign_without_runtime, "no-runtime");
            // A component the formation does not have: after `no-runtime`, ahead of the
            // predicates (another account's formation without the component answers it too).
            let absent = with(&base, "/component", json!("absent"));
            refuses(&w.caller, absent.clone(), "component-missing");
            refuses(
                &w.caller,
                with(&absent, "/runtime_id", json!(mint())),
                "no-runtime",
            );
            refuses(
                &w.caller,
                with(&absent, "/formation_id", json!(globex)),
                "component-missing",
            );

            // The predicates on the related rows.
            refuses(
                &w.caller,
                with(&base, "/formation_id", json!(globex)),
                "formation-foreign",
            );
            let other = w.formation("other", true);
            // A component of another formation is not one of this formation's.
            w.component(&other, "elsewhere");
            refuses(
                &w.caller,
                with(&base, "/component", json!("elsewhere")),
                "component-missing",
            );
            let other_revision = steps::commit_revision(&w.client, &w.caller, &other, w.scope());
            let other_environment =
                steps::create_environment(&w.client, &w.caller, &other, w.scope());
            refuses(
                &w.caller,
                with(&base, "/environment_id", json!(other_environment)),
                "environment-foreign",
            );
            refuses(
                &w.caller,
                with(&base, "/formation_revision_id", json!(other_revision)),
                "revision-foreign",
            );
            let retired = w.runtime(&["Snippet"], "Container");
            let mut ports = w.ports();
            let key = RuntimeId(Uuid(retired.clone()));
            let mut runtime = RuntimeStorage::get(&ports, &key).expect("the runtime row");
            runtime.state = weft_types::workloads::RuntimeState::Retired;
            RuntimeStorage::put(&mut ports, runtime);
            refuses(
                &w.caller,
                with(&base, "/runtime_id", json!(retired)),
                "runtime-retired",
            );
            // A retired environment, then a retired formation: after the foreign checks, ahead
            // of the runtime's state.
            let retired_environment = w.environment(&w.ids.environment_id, |row| {
                row.state = weft_types::architecture::EnvironmentState::Retired
            });
            let in_retired_environment = with(&base, "/environment_id", json!(retired_environment));
            refuses(
                &w.caller,
                in_retired_environment.clone(),
                "environment-retired",
            );
            refuses(
                &w.caller,
                with(&in_retired_environment, "/runtime_id", json!(retired)),
                "environment-retired",
            );
            let in_other = with(
                &with(&base, "/formation_id", json!(other)),
                "/formation_revision_id",
                json!(other_revision),
            );
            let in_other = with(&in_other, "/environment_id", json!(other_environment));
            w.retire_formation(&other);
            refuses(&w.caller, in_other.clone(), "formation-retired");
            refuses(
                &w.caller,
                with(&in_other, "/runtime_id", json!(retired)),
                "formation-retired",
            );
            let other_retired_environment = w.environment(&other_environment, |row| {
                row.state = weft_types::architecture::EnvironmentState::Retired
            });
            refuses(
                &w.caller,
                with(
                    &in_other,
                    "/environment_id",
                    json!(other_retired_environment),
                ),
                "formation-retired",
            );
            // Another account's retired formation answers as another account's.
            w.retire_formation(&globex);
            refuses(
                &w.caller,
                with(&base, "/formation_id", json!(globex)),
                "formation-foreign",
            );
            let image = w.runtime(&["Image", "Wasm"], "Container");
            refuses(
                &w.caller,
                with(&base, "/runtime_id", json!(image)),
                "runtime-kind",
            );
            let microvm = w.runtime(&["Snippet"], "MicroVm");
            refuses(
                &w.caller,
                with(&base, "/runtime_id", json!(microvm)),
                "below-runtime-isolation",
            );
            let user_kernel = w.runtime(&["Snippet"], "UserKernel");
            refuses(
                &w.caller,
                with(&base, "/runtime_id", json!(user_kernel)),
                "below-runtime-isolation",
            );
            let sandboxed = with(&base, "/isolation", json!("ProcessSandbox"));
            refuses(&w.caller, sandboxed, "below-runtime-isolation");
            // None, the weakest level, with its host user: below a runtime of minimum Container
            // and below one of minimum ProcessSandbox.
            let mut as_deployer = unsandboxed.clone();
            as_deployer["run_as"] = json!({"user": "deployer"});
            refuses(&w.caller, as_deployer.clone(), "below-runtime-isolation");
            let process_sandbox = w.runtime(&["Snippet"], "ProcessSandbox");
            refuses(
                &w.caller,
                with(&as_deployer, "/runtime_id", json!(process_sandbox)),
                "below-runtime-isolation",
            );

            // The artifact, decided from the artifacts domain's rows.
            refuses(
                &w.caller,
                with(&base, "/artifact_digest", json!("sha256:none")),
                "no-artifact",
            );
            w.artifact(&w.other, &w.other_scope, "sha256:globex", "Snippet");
            refuses(
                &w.caller,
                with(&base, "/artifact_digest", json!("sha256:globex")),
                "artifact-foreign",
            );
            w.artifact(&w.caller, w.scope(), "sha256:revoked", "Snippet");
            let mut ports = w.ports();
            let key = ArtifactDigest("sha256:revoked".to_owned());
            let mut artifact = ArtifactStorage::get(&ports, &key).expect("the artifact row");
            artifact.state = weft_types::artifacts::ArtifactState::Revoked;
            ArtifactStorage::put(&mut ports, artifact);
            refuses(
                &w.caller,
                with(&base, "/artifact_digest", json!("sha256:revoked")),
                "artifact-revoked",
            );
            w.artifact(&w.caller, w.scope(), "sha256:image", "Image");
            refuses(
                &w.caller,
                with(&base, "/artifact_digest", json!("sha256:image")),
                "artifact-kind",
            );

            // Accepted: a stronger isolation on the MicroVm runtime, and a valid check.
            let strong = with(
                &with(&base, "/runtime_id", json!(microvm)),
                "/isolation",
                json!("MicroVm"),
            );
            w.accepts(
                steps::DEPLOYER,
                &w.caller,
                DEFINE_WORKLOAD,
                &strong,
                "defined",
            );
            let checked = with(
                &base,
                "/processes",
                json!([sidecar, with(&input::process(), "/checks", json!([check]))]),
            );
            let body = w.accepts(
                steps::DEPLOYER,
                &w.caller,
                DEFINE_WORKLOAD,
                &checked,
                "defined",
            );
            let id = WorkloadId(Uuid(published(&body, "workload_id")));
            let row = WorkloadStorage::get(&w.ports(), &id).expect("the workload row");
            assert_eq!(row.state, weft_types::workloads::WorkloadState::Defined);
            assert_eq!(row.data.processes.len(), 2);
            assert_eq!(
                row.data.runtime_id,
                RuntimeId(Uuid(w.ids.runtime_id.clone()))
            );
            assert_eq!(row.data.priority.0, "standard");
            assert_eq!(row.data.run_as, None);

            // Accepted: isolation None on a runtime of minimum None, with its host user, which
            // the row records.
            let unsandboxed_runtime = w.runtime(&["Snippet"], "None");
            let body = w.accepts(
                steps::DEPLOYER,
                &w.caller,
                DEFINE_WORKLOAD,
                &with(&as_deployer, "/runtime_id", json!(unsandboxed_runtime)),
                "defined",
            );
            let id = WorkloadId(Uuid(published(&body, "workload_id")));
            let row = WorkloadStorage::get(&w.ports(), &id).expect("the workload row");
            assert_eq!(row.data.isolation, weft_types::shared::IsolationLevel::None);
            assert_eq!(
                row.data.run_as,
                Some(weft_types::workloads::RunAs {
                    user: "deployer".to_owned()
                })
            );
        }

        #[test]
        fn define_service_answers_each_guard_in_declaration_order() {
            let w = World::new();
            let refuses = |caller: &Value, input: Value, outcome: &str| {
                w.refuses(
                    steps::DEPLOYER,
                    caller,
                    DEFINE_SERVICE,
                    &input,
                    outcome,
                    services,
                );
            };
            w.component(&w.ids.formation_id, "second");
            let second = steps::define_workload(
                &w.client,
                &w.caller,
                with(&w.workload_request(), "/component", json!("second")),
            );
            let base = input::define_service(&second, w.scope());
            let bounds = |min: i64, max: i64, replicas: i64| {
                with(
                    &with(
                        &with(&base, "/scale_rule/min_replicas", json!(min)),
                        "/scale_rule/max_replicas",
                        json!(max),
                    ),
                    "/replicas",
                    json!(replicas),
                )
            };
            let sleep = json!({"idle_after": "PT5M", "kind": "Stop"});

            refuses(&w.other, base.clone(), "not-callers-scope");
            refuses(&w.caller, bounds(-1, 6, 1), "bad-bounds");
            refuses(&w.caller, bounds(3, 2, 2), "bad-bounds");
            refuses(&w.caller, bounds(2, 6, 1), "bad-bounds");
            refuses(&w.caller, bounds(1, 6, 7), "bad-bounds");
            refuses(&w.caller, bounds(0, 6, 0), "sleep-needs-policy");
            refuses(
                &w.caller,
                with(&base, "/sleep_policy", sleep.clone()),
                "policy-needs-min-zero",
            );
            refuses(
                &w.caller,
                with(&base, "/workload_id", json!(mint())),
                "no-workload",
            );
            let globex = ScopeId(Uuid(w.other_scope.clone()));
            let foreign = w.workload(&second, |row| row.data.scope_id = globex);
            refuses(
                &w.caller,
                with(&base, "/workload_id", json!(foreign)),
                "workload-of-other-scope",
            );
            let retired = w.workload(&second, |row| {
                row.state = weft_types::workloads::WorkloadState::Retired
            });
            refuses(
                &w.caller,
                with(&base, "/workload_id", json!(retired)),
                "workload-retired",
            );
            let job = w.workload(&second, |row| {
                row.data.shape = weft_types::workloads::ShapeKind::Job
            });
            refuses(
                &w.caller,
                with(&base, "/workload_id", json!(job)),
                "not-a-service",
            );
            refuses(
                &w.caller,
                with(&base, "/workload_id", json!(w.ids.workload_id)),
                "exists",
            );

            // A service that sleeps: min 0 with its policy.
            let sleeper = w.workload(&second, |_| {});
            let asleep = with(
                &with(&bounds(0, 6, 0), "/sleep_policy", sleep),
                "/workload_id",
                json!(sleeper),
            );
            w.accepts(
                steps::DEPLOYER,
                &w.caller,
                DEFINE_SERVICE,
                &asleep,
                "defined",
            );
            let body = w.accepts(steps::DEPLOYER, &w.caller, DEFINE_SERVICE, &base, "defined");
            let id = weft_types::shared::ServiceId(Uuid(published(&body, "service_id")));
            let row = ServiceStorage::get(&w.ports(), &id).expect("the service row");
            assert_eq!(row.state, weft_types::workloads::ServiceState::Awake);
            assert_eq!((row.data.replicas, row.data.running_replicas), (1, 0));
            assert_eq!(row.data.scope_id, ScopeId(Uuid(w.scope().to_owned())));
            assert_eq!(row.data.workload_id, WorkloadId(Uuid(second)));
            assert!(row.data.sleep_snapshots.is_empty() && row.data.backoff_until.is_none());
        }

        /// Sets the limit of `acme`'s `cpu` quota.
        fn set_limit(w: &World, limit: i64) {
            let mut ports = w.ports();
            let key = weft_types::shared::QuotaId(Uuid(w.path.quota_id.clone()));
            let mut quota = QuotaStorage::get(&ports, &key).expect("the quota row");
            quota.data.limit = limit;
            QuotaStorage::put(&mut ports, quota);
        }

        /// Creates replica 0 of the definitions' service: a `Pending` instance holding the
        /// workload's 1 cpu.
        fn hold_one_cpu(w: &World) {
            let mut ports = w.ports();
            let input = weft_types::workloads::CreateInstance {
                workload_id: WorkloadId(Uuid(w.ids.workload_id.clone())),
                job_id: None,
                task_index: None,
                invocation_id: None,
                box_id: None,
                session_id: None,
                replica_index: Some(0),
                warm_pool_id: None,
                service_id: Some(weft_types::shared::ServiceId(Uuid(
                    w.ids.service_id.clone(),
                ))),
            };
            let answer =
                CreateInstanceBehavior::create_instance(&mut ports, input).expect("an outcome");
            assert!(
                matches!(
                    answer,
                    weft_types::workloads::CreateInstanceOutcome::Created { .. }
                ),
                "{answer:?}"
            );
        }

        /// `over-quota` of `DefineWorkload` (outcome group `quota`, an external branch the
        /// contract answers before its default `defined`): after every other guard, from the
        /// claims of one instance of the workload alone against each quota of the account's
        /// chain. What the account's live instances hold does not count: a workload is defined
        /// while its account is full, and `CreateInstance` asks for its instances.
        #[test]
        fn define_workload_answers_over_quota_before_defined_from_one_instance_s_claims() {
            let w = World::new();
            let base = w.workload_request();
            set_limit(&w, 0);
            w.refuses(
                steps::DEPLOYER,
                &w.caller,
                DEFINE_WORKLOAD,
                &base,
                "over-quota",
                workloads,
            );
            w.refuses(
                steps::DEPLOYER,
                &w.caller,
                DEFINE_WORKLOAD,
                &with(&base, "/artifact_digest", json!("sha256:none")),
                "no-artifact",
                workloads,
            );
            set_limit(&w, 1);
            w.refuses(
                steps::DEPLOYER,
                &w.caller,
                DEFINE_WORKLOAD,
                &with(&base, "/claims/0/amount", json!(2)),
                "over-quota",
                workloads,
            );
            hold_one_cpu(&w);
            w.accepts(
                steps::DEPLOYER,
                &w.caller,
                DEFINE_WORKLOAD,
                &base,
                "defined",
            );
        }

        /// `over-quota` of `DefineService`: after every other guard, from the claims of
        /// `replicas` instances of the workload alone against each quota of the account's
        /// chain; what the account's live instances hold does not count.
        #[test]
        fn define_service_answers_over_quota_before_defined_from_its_replicas_claims() {
            let w = World::new();
            let second = w.workload(&w.ids.workload_id, |_| {});
            let request = |workload: &str, replicas: i64| {
                with(
                    &input::define_service(workload, w.scope()),
                    "/replicas",
                    json!(replicas),
                )
            };
            set_limit(&w, 2);
            w.refuses(
                steps::DEPLOYER,
                &w.caller,
                DEFINE_SERVICE,
                &request(&second, 3),
                "over-quota",
                services,
            );
            let retired = w.workload(&w.ids.workload_id, |row| {
                row.state = weft_types::workloads::WorkloadState::Retired
            });
            w.refuses(
                steps::DEPLOYER,
                &w.caller,
                DEFINE_SERVICE,
                &request(&retired, 3),
                "workload-retired",
                services,
            );
            hold_one_cpu(&w);
            let body = w.accepts(
                steps::DEPLOYER,
                &w.caller,
                DEFINE_SERVICE,
                &request(&second, 2),
                "defined",
            );
            let id = weft_types::shared::ServiceId(Uuid(published(&body, "service_id")));
            let row = ServiceStorage::get(&w.ports(), &id).expect("the service row");
            assert_eq!(row.data.replicas, 2);
        }

        /// Cases of the adversary pass on `story:skeleton-workload-definitions`: the values the
        /// seven obligations write and publish, the inputs of the document digest, and the
        /// shape/settings agreement.
        mod adversary {
            use serde_json::{json, Value};
            use weft_testkit::steps;
            use weft_types::behaviour::{
                FormationRevisionStorage, ServiceStorage, WorkloadStorage,
            };
            use weft_types::primitives::Uuid;
            use weft_types::shared::{
                ArtifactDigest, ComponentName, EnvironmentId, FormationRevisionId, IsolationLevel,
                ServiceId, WorkloadId,
            };

            use super::{
                published, with, workloads, World, ADD_COMPONENT, COMMIT_REVISION,
                CREATE_ENVIRONMENT, CREATE_FORMATION, DEFINE_SERVICE, DEFINE_WORKLOAD,
                REGISTER_ARTIFACT,
            };

            /// The payload of the first event `body` published.
            fn payload(body: &Value) -> Value {
                body["published"][0]["payload"].clone()
            }

            /// `CommitRevision` of `formation` by `acme`: the published (number, digest).
            fn commit(w: &World, formation: &str) -> (Value, Value) {
                let body = w.accepts(
                    steps::DEPLOYER,
                    &w.caller,
                    COMMIT_REVISION,
                    &json!({"formation_id": formation, "scope_id": w.scope()}),
                    "committed",
                );
                let payload = payload(&body);
                (
                    payload["number"].clone(),
                    payload["document_digest"].clone(),
                )
            }

            /// `ess/4-applications/workloads.yaml`, DefineWorkload `settings-shape-mismatch`: the
            /// shape and the tag of `settings` agree. A `Service` workload carrying `job`
            /// settings is not defined, and no workload row is added.
            #[test]
            fn a_workload_whose_settings_tag_is_not_its_shape_is_not_defined() {
                let w = World::new();
                w.component(&w.ids.formation_id, "mismatched");
                let job_settings = json!({
                    "shape": "job",
                    "value": {"parallelism": 1, "gang": false, "max_retries": 0, "backoff": "PT1S"},
                });
                let mismatched = with(
                    &with(&w.workload_request(), "/component", json!("mismatched")),
                    "/settings",
                    job_settings,
                );
                let before = workloads(&w.ports());
                let answer = w
                    .client
                    .command(
                        DEFINE_WORKLOAD,
                        steps::DEPLOYER,
                        w.caller.clone(),
                        mismatched.clone(),
                    )
                    .expect("the control host answers");
                assert_ne!(
                    answer.body["outcome"],
                    json!("defined"),
                    "shape Service with settings tagged job was defined: {} {}",
                    answer.status,
                    answer.body
                );
                assert_eq!(workloads(&w.ports()), before, "a workload row was added");
            }

            /// The workload row and `WorkloadDefined` hold every value sent, on a request whose
            /// isolation (`MicroVm`) is stronger than the runtime's minimum (`Container`): a
            /// row that took the runtime's minimum, or a payload field from another input, is
            /// caught.
            #[test]
            fn define_workload_writes_and_publishes_the_values_sent() {
                let w = World::new();
                w.component(&w.ids.formation_id, "strong");
                let request = with(
                    &with(&w.workload_request(), "/component", json!("strong")),
                    "/isolation",
                    json!("MicroVm"),
                );
                let body = w.accepts(
                    steps::DEPLOYER,
                    &w.caller,
                    DEFINE_WORKLOAD,
                    &request,
                    "defined",
                );
                let sent = payload(&body);
                for field in [
                    "scope_id",
                    "formation_id",
                    "environment_id",
                    "component",
                    "package_kind",
                    "artifact_digest",
                    "shape",
                    "isolation",
                ] {
                    assert_eq!(
                        sent[field], request[field],
                        "WorkloadDefined.{field}: {body}"
                    );
                }
                let id = WorkloadId(Uuid(published(&body, "workload_id")));
                let row = WorkloadStorage::get(&w.ports(), &id).expect("the workload row");
                assert_eq!(row.data.isolation, IsolationLevel::MicroVm);
                assert_eq!(row.data.shape, weft_types::workloads::ShapeKind::Service);
                assert_eq!(row.data.component, ComponentName("strong".to_owned()));
                assert_eq!(
                    row.data.environment_id,
                    EnvironmentId(Uuid(w.ids.environment_id.clone()))
                );
                assert_eq!(
                    row.data.formation_revision_id,
                    FormationRevisionId(Uuid(w.ids.revision_id.clone()))
                );
                assert_eq!(
                    row.data.artifact_digest,
                    ArtifactDigest(steps::input::ARTIFACT_DIGEST.to_owned())
                );
                assert!(
                    matches!(
                        row.data.settings,
                        weft_types::workloads::ShapeSettings::Service(_)
                    ),
                    "{:?}",
                    row.data.settings
                );
                assert_eq!(row.data.claims.len(), 1);
                let policy = &row.data.restart_policy;
                assert_eq!(
                    (
                        policy.backoff_initial_seconds,
                        policy.backoff_max_seconds,
                        policy.backoff_limit,
                        policy.backoff_window_seconds
                    ),
                    (1, 60, 5, 600)
                );
            }

            /// A sleeping service's row keeps its sleep policy and scale rule, and
            /// `ServiceDefined` carries the replicas and scale rule sent.
            #[test]
            fn define_service_writes_and_publishes_the_values_sent() {
                let w = World::new();
                w.component(&w.ids.formation_id, "sleeper");
                let workload = steps::define_workload(
                    &w.client,
                    &w.caller,
                    with(&w.workload_request(), "/component", json!("sleeper")),
                );
                let base = steps::input::define_service(&workload, w.scope());
                let request = with(
                    &with(
                        &with(&base, "/scale_rule/min_replicas", json!(0)),
                        "/replicas",
                        json!(0),
                    ),
                    "/sleep_policy",
                    json!({"idle_after": "PT5M", "kind": "Stop"}),
                );
                let body = w.accepts(
                    steps::DEPLOYER,
                    &w.caller,
                    DEFINE_SERVICE,
                    &request,
                    "defined",
                );
                let sent = payload(&body);
                assert_eq!(sent["workload_id"], json!(workload), "{body}");
                assert_eq!(sent["replicas"], json!(0), "{body}");
                assert_eq!(sent["scale_rule"], request["scale_rule"], "{body}");
                let id = ServiceId(Uuid(published(&body, "service_id")));
                let row = ServiceStorage::get(&w.ports(), &id).expect("the service row");
                assert_eq!(row.data.replicas, 0);
                assert_eq!(
                    (
                        row.data.scale_rule.min_replicas,
                        row.data.scale_rule.max_replicas
                    ),
                    (0, 6)
                );
                let policy = row.data.sleep_policy.expect("the sleep policy sent");
                assert_eq!(policy.idle_after.0, "PT5M");
            }

            /// The payloads of `CreateFormation`, `AddComponent`, `CreateEnvironment`,
            /// `RegisterArtifact` and `CommitRevision` carry the values sent, and the revision's
            /// payload is its row.
            #[test]
            fn the_formation_and_artifact_events_carry_the_values_sent() {
                let w = World::new();
                let formation =
                    json!({"scope_id": w.scope(), "name": "events", "zones": [], "parity": []});
                let body = w.accepts(
                    steps::ACCOUNT_ADMIN,
                    &w.caller,
                    CREATE_FORMATION,
                    &formation,
                    "created",
                );
                let sent = payload(&body);
                assert_eq!(
                    (sent["scope_id"].clone(), sent["name"].clone()),
                    (json!(w.scope()), json!("events")),
                    "{body}"
                );
                let id = published(&body, "formation_id");

                let body = w.accepts(
                    steps::DEPLOYER,
                    &w.caller,
                    ADD_COMPONENT,
                    &json!({"formation_id": id, "scope_id": w.scope(), "name": "api"}),
                    "added",
                );
                let sent = payload(&body);
                assert_eq!(
                    (sent["formation_id"].clone(), sent["name"].clone()),
                    (json!(id), json!("api")),
                    "{body}"
                );

                let body = w.accepts(
                    steps::ACCOUNT_ADMIN,
                    &w.caller,
                    CREATE_ENVIRONMENT,
                    &json!({"formation_id": id, "scope_id": w.scope(), "name": "staging", "protected": false}),
                    "created",
                );
                let sent = payload(&body);
                assert_eq!(
                    (sent["formation_id"].clone(), sent["name"].clone()),
                    (json!(id), json!("staging")),
                    "{body}"
                );

                let mut artifact = steps::input::register_artifact(w.scope());
                artifact["artifact_digest"] = json!("sha256:events");
                artifact["kind"] = json!("Wasm");
                artifact["signer_key_id"] = json!("k-events");
                let body = w.accepts(
                    steps::DEPLOYER,
                    &w.caller,
                    REGISTER_ARTIFACT,
                    &artifact,
                    "registered",
                );
                let sent = payload(&body);
                for field in ["artifact_digest", "scope_id", "kind", "signer_key_id"] {
                    assert_eq!(
                        sent[field], artifact[field],
                        "ArtifactRegistered.{field}: {body}"
                    );
                }

                let body = w.accepts(
                    steps::DEPLOYER,
                    &w.caller,
                    COMMIT_REVISION,
                    &json!({"formation_id": id, "scope_id": w.scope()}),
                    "committed",
                );
                let sent = payload(&body);
                assert_eq!(sent["formation_id"], json!(id), "{body}");
                let revision = FormationRevisionId(Uuid(published(&body, "revision_id")));
                let row =
                    FormationRevisionStorage::get(&w.ports(), &revision).expect("the revision row");
                assert_eq!(
                    (json!(row.data.number), json!(row.data.document_digest.0)),
                    (sent["number"].clone(), sent["document_digest"].clone()),
                    "{body}"
                );
            }

            /// A formation's document is its own rows: a component or an environment added to
            /// another formation of the account leaves its digest as it was.
            #[test]
            fn another_formations_rows_leave_the_digest_as_it_was() {
                let w = World::new();
                let site = w.ids.formation_id.clone();
                let (_, before) = commit(&w, &site);
                let other = w.formation("neighbour", true);
                steps::create_environment(&w.client, &w.caller, &other, w.scope());
                let (number, after) = commit(&w, &site);
                assert_eq!(number, json!(3));
                assert_eq!(
                    after, before,
                    "another formation's rows changed site's digest"
                );
            }

            /// The digest is the SHA-256 of the canonical form `commit_revision.rs` documents:
            /// `["twin", [[name, internal name, on border], ...], [[name, protected], ...]]`,
            /// each list sorted by name, compact JSON. Components added in reverse order and a
            /// protected environment; other formations of the store hold rows too. The literal is
            /// `printf '%s' '<that document>' | sha256sum`.
            #[test]
            fn the_digest_is_the_documented_canonical_form() {
                let w = World::new();
                let twin = w.formation("twin", false);
                for name in ["e", "d", "c", "b", "a"] {
                    w.accepts(
                        steps::DEPLOYER,
                        &w.caller,
                        ADD_COMPONENT,
                        &json!({"formation_id": twin, "scope_id": w.scope(), "name": name}),
                        "added",
                    );
                }
                w.accepts(
                    steps::ACCOUNT_ADMIN,
                    &w.caller,
                    CREATE_ENVIRONMENT,
                    &json!({"formation_id": twin, "scope_id": w.scope(), "name": "prod", "protected": true}),
                    "created",
                );
                let (number, digest) = commit(&w, &twin);
                assert_eq!(number, json!(1));
                assert_eq!(
                    digest,
                    json!(
                        "sha256:bb3b0e3a3d8b7aada2cabe5eefa65099db3fb80c42f138cdfcb50e03b1f4864e"
                    )
                );
            }
        }
    }
}

/// The walking skeleton end to end (`story:skeleton-end-to-end`, `docs/implementation.md`
/// § Walking skeleton): one control host and one `weft-agent` beside the substrate-daemon of the
/// test run; the operator, tenant and workload steps of `sim/experiments/full-setup.yaml` in the
/// default cluster over HTTP; the controllers and the hand-run bindings of the control host take
/// the service's replica to a placed, assigned instance, which the agent starts confined and
/// records running. The processes are started once, for the one test of this module.
mod skeleton {
    use std::time::{Duration, Instant};

    use serde_json::{json, Value};
    use weft_client::{Client, Expect};
    use weft_testkit::{steps, ControlHost, NodeAgent};

    /// How long the path may take from its first command to the instance reading `Running`.
    const RUN_LIMIT: Duration = Duration::from_secs(30);

    /// The commands of the path that are sent as requests: the operator's, the node agent's (its
    /// `hello` is `OpenNodeSession` as the ControlPlane), the tenant's, the controllers' and the
    /// node agent's two records of its instance.
    const REQUESTS: &[&str] = &[
        "weft.infrastructure.CreateCloud",
        "weft.infrastructure.ConfigureCloud",
        "weft.infrastructure.DeclareZone",
        "weft.infrastructure.DefinePriorityClass",
        "weft.workloads.RegisterRuntime",
        "weft.infrastructure.CreateNodePool",
        "weft.infrastructure.ProvisionPoolNode",
        "weft.cluster.CreateDefaultCluster",
        "weft.accounts.CreateTenant",
        "weft.accounts.GrantTopQuota",
        "weft.infrastructure.JoinNode",
        "weft.infrastructure.RecordNodeObservation",
        "weft.nodes.OpenNodeSession",
        "weft.accounts.CreateAccount",
        "weft.accounts.RecordAccountScope",
        "weft.architecture.CreateFormation",
        "weft.cluster.PinFormation",
        "weft.architecture.AddComponent",
        "weft.architecture.CommitRevision",
        "weft.architecture.CreateEnvironment",
        "weft.artifacts.RegisterArtifact",
        "weft.workloads.DefineWorkload",
        "weft.workloads.DefineService",
        "weft.workloads.RequestReplica",
        "weft.scheduling.PlaceInstance",
        "weft.workloads.RecordArtifactVerified",
        "weft.workloads.RecordInstanceRunning",
    ];

    /// The hand-run bindings of the path whose command takes effect.
    const APPLIED: &[&str] = &[
        "r1-create-replica",
        "r1-submit-scheduling-request",
        "r4-bind-placement",
        "r4-assign",
        "r4-schedule-instance",
        "r4-record-placement-assigned",
        "edge-backend-ready",
    ];

    /// The row of `weft.workloads.Instances` of the service `service_id`, once there is one.
    fn instance_of(client: &Client, service_id: &str) -> Option<Value> {
        let answer = client.view("weft.workloads.Instances", &[]).ok_body(200);
        answer["rows"]
            .as_array()
            .expect("rows")
            .iter()
            .find(|row| row["service_id"] == json!(service_id))
            .cloned()
    }

    /// Statements 1 to 7 of the story's acceptance.
    #[test]
    fn place_instance_leads_to_a_confined_snippet_that_runs_and_is_recorded_running() {
        let host = ControlHost::start();
        let client = host.client();
        let before_seq = host.last_seq();

        let first_command = Instant::now();
        let path = steps::run_path(&client);
        let mut agent = NodeAgent::start(&host, &path.node.node_id, &path.node.join_token);
        let ids = steps::run_definitions(&client, &path);

        // 1. Within 30 s of the first command the service's instance reads Running.
        let instance_id = loop {
            let row = instance_of(&client, &ids.service_id);
            if let Some(row) = row.as_ref().filter(|row| row["state"] == json!("Running")) {
                break row["instance_id"]
                    .as_str()
                    .expect("an instance_id")
                    .to_owned();
            }
            assert!(
                first_command.elapsed() < RUN_LIMIT,
                "the service's instance does not read Running {RUN_LIMIT:?} after the first \
                 command: {row:?}; agent report: {:?}; log: {:?}",
                agent.report(),
                host.log_after(before_seq)
            );
            std::thread::sleep(Duration::from_millis(20));
        };

        // 2. The report file holds the exit line of its process.
        let exit = json!({"instance_id": instance_id, "exit_code": 0, "stdout": "weft\n"});
        agent.wait_for(RUN_LIMIT, |lines| lines.contains(&exit));

        let log = host.log_after(before_seq);
        let requests = |operation: &str| -> Vec<&Value> {
            log.iter()
                .filter(|line| line["kind"] == "request" && line["operation"] == operation)
                .collect()
        };
        let status = |line: &Value| line["status"].as_u64().expect("a status");

        // 3. A request line with a 2xx status for each command of the path sent as a request.
        for operation in REQUESTS {
            assert!(
                requests(operation)
                    .iter()
                    .any(|line| (200..300).contains(&status(line))),
                "no 2xx request line of {operation}: {:?}; log: {log:?}",
                requests(operation)
            );
        }

        // 4. No request line with status 501.
        let unmet: Vec<&Value> = log
            .iter()
            .filter(|line| line["kind"] == "request" && line["status"] == json!(501))
            .collect();
        assert!(unmet.is_empty(), "request lines answered 501: {unmet:?}");

        let bindings = |id: &str| -> Vec<&Value> {
            log.iter()
                .filter(|line| line["kind"] == "binding" && line["binding"] == id)
                .collect()
        };
        // 5. A binding line `applied` for each binding of the path whose command takes effect.
        for id in APPLIED {
            assert!(
                bindings(id).iter().any(|line| line["result"] == "applied"),
                "no applied line of {id}: {:?}; log: {log:?}",
                bindings(id)
            );
        }

        // 6. `edge-backend-restore` refused `not-out-of-traffic`, dropped.
        assert!(
            bindings("edge-backend-restore")
                .iter()
                .any(|line| line["refusal"] == "not-out-of-traffic" && line["result"] == "dropped"),
            "{:?}; log: {log:?}",
            bindings("edge-backend-restore")
        );

        // 7. `edge-backend-restarting` refused `not-ready`, dropped.
        assert!(
            bindings("edge-backend-restarting")
                .iter()
                .any(|line| line["refusal"] == "not-ready" && line["result"] == "dropped"),
            "{:?}; log: {log:?}",
            bindings("edge-backend-restarting")
        );
    }
}
