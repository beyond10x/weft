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
}
