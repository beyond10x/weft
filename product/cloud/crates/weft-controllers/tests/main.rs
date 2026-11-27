//! The one integration-test binary of weft-controllers.
//!
//! - [`replicas`]: the acceptance of `story:skeleton-replica-instances`, the R1 level step and
//!   the two hand-run bindings of R1, on the `weft` binary and in-process;
//! - [`obligations`]: `weft.workloads.CreateInstance` and `weft.scheduling.SubmitSchedulingRequest`,
//!   each guard in the selection precedence of its contract;
//! - [`runner`]: the runner of the hand-run bindings on a state file of its own;
//! - [`adversary`]: the cases of the adversary pass.

/// What both modules share: a control host in this process after the operator and tenant path
/// and the workload definitions, and readers of its views and log.
mod world {
    use serde_json::{json, Value};
    use weft_client::{Client, Expect};
    use weft_testkit::steps::{self, Definitions, Path};
    use weft_testkit::InProcess;

    /// The path and the definitions in account `acme`, on an in-process control host whose
    /// controllers have not run.
    pub struct World {
        pub process: InProcess,
        pub path: Path,
        pub ids: Definitions,
    }

    impl World {
        pub fn new() -> Self {
            let process = InProcess::start();
            let path = steps::run_path(process.client());
            let ids = steps::run_definitions(process.client(), &path);
            Self { process, path, ids }
        }

        pub fn ports(&self) -> weft_impl::Ports {
            self.process.ports()
        }
    }

    /// The rows of `weft.workloads.InstanceList` of `workload_id`.
    pub fn instances(client: &Client, workload_id: &str) -> Vec<Value> {
        rows(
            client
                .view(
                    "weft.workloads.InstanceList",
                    &[("workload_id", workload_id)],
                )
                .ok_body(200),
        )
    }

    /// The rows of `weft.scheduling.SchedulingRequests` for `instance_id`.
    pub fn requests_for(client: &Client, instance_id: &Value) -> Vec<Value> {
        rows(
            client
                .view("weft.scheduling.SchedulingRequests", &[])
                .ok_body(200),
        )
        .into_iter()
        .filter(|row| &row["instance_id"] == instance_id)
        .collect()
    }

    fn rows(answer: Value) -> Vec<Value> {
        answer["rows"]
            .as_array()
            .unwrap_or_else(|| panic!("no rows in {answer}"))
            .clone()
    }

    /// The binding lines of `binding` among `lines`.
    pub fn binding_lines(lines: &[Value], binding: &str) -> Vec<Value> {
        lines
            .iter()
            .filter(|line| line["kind"] == json!("binding") && line["binding"] == json!(binding))
            .cloned()
            .collect()
    }

    /// The request lines of `operation` among `lines`.
    pub fn request_lines(lines: &[Value], operation: &str) -> Vec<Value> {
        lines
            .iter()
            .filter(|line| {
                line["kind"] == json!("request") && line["operation"] == json!(operation)
            })
            .cloned()
            .collect()
    }
}

mod replicas {
    use std::time::{Duration, Instant};

    use serde_json::{json, Value};
    use weft_impl::Ports;
    use weft_system::SystemEvent;
    use weft_testkit::steps;
    use weft_testkit::ControlHost;
    use weft_types::behaviour::{
        FormationStorage, InstanceStorage, QuotaStorage, ServiceStorage, WorkloadStorage,
    };
    use weft_types::primitives::Uuid;
    use weft_types::shared::{QuotaId, ServiceId, WorkloadId};
    use weft_types::workloads::{InstanceCreated, ReplicaRequested};

    use super::world::{binding_lines, instances, request_lines, requests_for, World};

    const CREATE_REPLICA: &str = "r1-create-replica";
    const SUBMIT: &str = "r1-submit-scheduling-request";

    /// A `ReplicaRequested` of the world's service for `index`.
    fn replica_requested(w: &World, index: i64) -> SystemEvent {
        SystemEvent::WorkloadsReplicaRequested(ReplicaRequested {
            service_id: ServiceId(Uuid(w.ids.service_id.clone())),
            workload_id: WorkloadId(Uuid(w.ids.workload_id.clone())),
            replica_index: index,
        })
    }

    /// The `InstanceCreated` the world's one instance was created with, read back from its rows.
    fn instance_created(ports: &Ports) -> InstanceCreated {
        let instances = InstanceStorage::list(ports);
        assert_eq!(instances.len(), 1, "{instances:?}");
        let instance = &instances[0].data;
        let workload =
            WorkloadStorage::get(ports, &instance.workload_id).expect("the workload row");
        let formation =
            FormationStorage::get(ports, &workload.data.formation_id).expect("the formation row");
        InstanceCreated {
            instance_id: instance.instance_id.clone(),
            workload_id: instance.workload_id.clone(),
            minimum_isolation: instance.minimum_isolation,
            account_id: instance.account_id.clone(),
            formation_id: workload.data.formation_id.clone(),
            claims: workload.data.claims.clone(),
            service_id: instance.service_id.clone(),
            replica_index: instance.replica_index,
            job_id: None,
            zones: formation.data.zones.clone(),
            priority: workload.data.priority.clone(),
        }
    }

    /// Statement 1: against the running control host, a service defined over HTTP with
    /// `replicas: 1` has, within 2 s, one instance with `replica_index` 0 and one pending
    /// scheduling request for it.
    #[test]
    fn a_service_defined_over_http_has_its_replica_and_its_scheduling_request_within_two_seconds() {
        let host = ControlHost::start();
        let client = host.client();
        let path = steps::run_path(&client);
        let ids = steps::run_definitions(&client, &path);
        let defined = Instant::now();
        let (instance, requests) = loop {
            let rows = instances(&client, &ids.workload_id);
            if let [instance] = rows.as_slice() {
                let requests = requests_for(&client, &instance["instance_id"]);
                if !requests.is_empty() {
                    break (instance.clone(), requests);
                }
            }
            assert!(
                defined.elapsed() < Duration::from_secs(2),
                "no instance with its request 2 s after DefineService: {rows:?}"
            );
            std::thread::sleep(Duration::from_millis(20));
        };
        assert_eq!(instance["replica_index"], json!(0));
        assert_eq!(instance["service_id"], json!(ids.service_id));
        assert_eq!(instance["state"], json!("Pending"));
        assert_eq!(requests.len(), 1, "{requests:?}");
        assert_eq!(requests[0]["state"], json!("Pending"));
        assert_eq!(requests[0]["account_id"], json!(path.account_id));
        assert_eq!(requests[0]["cluster_id"], json!(path.cluster_id));
    }

    /// Statement 2: a replayed `ReplicaRequested` of a live replica is answered `replica-live`
    /// and dropped: one dropped binding line, no retried one, still one instance.
    #[test]
    fn a_replayed_replica_request_of_a_live_replica_is_dropped() {
        let w = World::new();
        w.process.host().tick();
        let client = w.process.client();
        assert_eq!(instances(client, &w.ids.workload_id).len(), 1);
        let seq = w.process.last_seq();
        w.process.host().replay(vec![replica_requested(&w, 0)]);
        let lines = binding_lines(&w.process.log_after(seq), CREATE_REPLICA);
        assert_eq!(
            lines,
            vec![
                json!({"seq": lines[0]["seq"], "kind": "binding", "binding": CREATE_REPLICA, "refusal": "replica-live", "result": "dropped", "attempt": 1})
            ],
        );
        assert_eq!(instances(client, &w.ids.workload_id).len(), 1);
        assert_eq!(w.process.host().held(), 0);
    }

    /// Sets the limit and the remainder of the world's `cpu` quota to `limit`.
    fn set_limit(w: &World, limit: i64) {
        let mut ports = w.ports();
        let key = QuotaId(Uuid(w.path.quota_id.clone()));
        let mut quota = QuotaStorage::get(&ports, &key).expect("the quota row");
        quota.data.limit = limit;
        quota.data.remaining = limit;
        QuotaStorage::put(&mut ports, quota);
    }

    /// Statement 3: a `ReplicaRequested` whose instance would take the account past its `cpu`
    /// quota is answered `over-quota`, off the drop list: retried, not dropped. It is attempted
    /// again on the next tick, then after a wait of two ticks; once the quota is raised, the
    /// next attempt creates it.
    #[test]
    fn a_replica_request_past_the_account_quota_is_retried_until_the_quota_admits_it() {
        let w = World::new();
        w.process.host().tick();
        let client = w.process.client();
        // The one live instance claims 1 cpu: the quota is used up.
        set_limit(&w, 1);
        let seq = w.process.last_seq();
        w.process.host().replay(vec![replica_requested(&w, 1)]);
        let lines = binding_lines(&w.process.log_after(seq), CREATE_REPLICA);
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert_eq!(lines[0]["refusal"], json!("over-quota"));
        assert_eq!(lines[0]["result"], json!("retried"));
        assert_eq!(lines[0]["attempt"], json!(1));
        assert_eq!(instances(client, &w.ids.workload_id).len(), 1);
        assert_eq!(w.process.host().held(), 1);

        // Still used up: the next tick attempts it again and holds it again.
        let seq = w.process.last_seq();
        w.process.host().tick();
        let lines = binding_lines(&w.process.log_after(seq), CREATE_REPLICA);
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert_eq!(lines[0]["result"], json!("retried"));
        assert_eq!(lines[0]["attempt"], json!(2));

        // Raised: the first tick waits, the second attempts it.
        set_limit(&w, 64);
        let seq = w.process.last_seq();
        w.process.host().tick();
        assert_eq!(
            binding_lines(&w.process.log_after(seq), CREATE_REPLICA),
            Vec::<Value>::new()
        );
        w.process.host().tick();
        let after = w.process.log_after(seq);
        let lines = binding_lines(&after, CREATE_REPLICA);
        assert_eq!(lines.len(), 1, "{after:?}");
        assert_eq!(lines[0]["result"], json!("applied"));
        assert_eq!(lines[0]["attempt"], json!(3));
        assert!(lines[0].get("refusal").is_none(), "{lines:?}");
        assert_eq!(binding_lines(&after, SUBMIT)[0]["result"], json!("applied"));
        assert_eq!(instances(client, &w.ids.workload_id).len(), 2);
        assert_eq!(w.process.host().held(), 0);
    }

    /// A held event is attempted again after a wait that doubles from one tick up to 32 ticks,
    /// one binding line per attempt with its number: over 100 ticks, 8 lines, at most
    /// log2(100) + 2. The service, whose replica is held, is not requested again.
    #[test]
    fn a_held_replica_request_is_attempted_after_doubling_waits_with_one_line_per_attempt() {
        const TICKS: u32 = 100;
        let w = World::new();
        set_limit(&w, 0);
        let start = w.process.last_seq();
        let mut attempted_at = Vec::new();
        for tick in 1..=TICKS {
            let seq = w.process.last_seq();
            w.process.host().tick();
            if !binding_lines(&w.process.log_after(seq), CREATE_REPLICA).is_empty() {
                attempted_at.push(tick);
            }
        }
        // Tick 1 requests the replica (attempt 1); waits of 1, 2, 4, 8, 16, then 32, 32.
        assert_eq!(attempted_at, [1, 2, 4, 8, 16, 32, 64, 96]);
        let after = w.process.log_after(start);
        let lines = binding_lines(&after, CREATE_REPLICA);
        let attempts: Vec<Value> = lines.iter().map(|line| line["attempt"].clone()).collect();
        assert_eq!(attempts, (1..=8).map(|n| json!(n)).collect::<Vec<_>>());
        assert!(
            lines
                .iter()
                .all(|line| line["refusal"] == json!("over-quota")
                    && line["result"] == json!("retried")),
            "{lines:?}"
        );
        assert!(
            f64::from(u32::try_from(lines.len()).expect("a count"))
                <= f64::from(TICKS).log2() + 2.0
        );
        assert_eq!(
            request_lines(&after, "weft.workloads.RequestReplica").len(),
            1,
            "{after:?}"
        );

        // Raised: the attempt due at tick 128 creates it.
        set_limit(&w, 64);
        for _ in 0..28 {
            w.process.host().tick();
        }
        assert_eq!(instances(w.process.client(), &w.ids.workload_id).len(), 1);
        assert_eq!(w.process.host().held(), 0);
    }

    /// One replica request per service at a time: a service whose `ReplicaRequested` is held is
    /// skipped, and another service below scale is still requested.
    #[test]
    fn a_service_with_a_held_replica_request_is_skipped_and_another_is_requested() {
        let w = World::new();
        let mut ports = w.ports();
        let first = ServiceId(Uuid(w.ids.service_id.clone()));
        let mut workload =
            WorkloadStorage::get(&ports, &WorkloadId(Uuid(w.ids.workload_id.clone())))
                .expect("the workload row");
        workload.data.workload_id = WorkloadId(Uuid(weft_impl::context::mint()));
        let mut service = ServiceStorage::get(&ports, &first).expect("the service row");
        service.data.service_id = ServiceId(Uuid(weft_impl::context::mint()));
        service.data.workload_id = workload.data.workload_id.clone();
        let second = service.data.service_id.clone();
        WorkloadStorage::put(&mut ports, workload);
        ServiceStorage::put(&mut ports, service);
        set_limit(&w, 0);
        w.process.host().replay(vec![replica_requested(&w, 0)]);
        assert_eq!(w.process.host().held(), 1);

        let seq = w.process.last_seq();
        w.process.host().tick();
        let after = w.process.log_after(seq);
        assert_eq!(
            request_lines(&after, "weft.workloads.RequestReplica").len(),
            1,
            "{after:?}"
        );
        let running = |id: &ServiceId| {
            ServiceStorage::get(&w.ports(), id)
                .expect("the service row")
                .data
                .running_replicas
        };
        assert_eq!((running(&first), running(&second)), (0, 1));
    }

    /// `r1-create-replica` drops `workload-retired`: a replica of a retired workload is not held.
    #[test]
    fn a_replica_request_of_a_retired_workload_is_dropped() {
        let w = World::new();
        let mut ports = w.ports();
        let key = WorkloadId(Uuid(w.ids.workload_id.clone()));
        let mut workload = WorkloadStorage::get(&ports, &key).expect("the workload row");
        workload.state = weft_types::workloads::WorkloadState::Retired;
        WorkloadStorage::put(&mut ports, workload);
        let seq = w.process.last_seq();
        w.process.host().replay(vec![replica_requested(&w, 0)]);
        let lines = binding_lines(&w.process.log_after(seq), CREATE_REPLICA);
        assert_eq!(
            lines,
            vec![
                json!({"seq": lines[0]["seq"], "kind": "binding", "binding": CREATE_REPLICA, "refusal": "workload-retired", "result": "dropped", "attempt": 1})
            ],
        );
        assert_eq!(instances(w.process.client(), &w.ids.workload_id).len(), 0);
        assert_eq!(w.process.host().held(), 0);
    }

    /// Statement 4: a replayed `InstanceCreated` of an instance whose scheduling request exists
    /// is answered `already-requested` and dropped; still one request for it.
    #[test]
    fn a_replayed_instance_created_of_a_requested_instance_is_dropped() {
        let w = World::new();
        w.process.host().tick();
        let client = w.process.client();
        let created = instance_created(&w.ports());
        let instance_id = json!(created.instance_id.0 .0);
        assert_eq!(requests_for(client, &instance_id).len(), 1);
        let seq = w.process.last_seq();
        w.process
            .host()
            .replay(vec![SystemEvent::WorkloadsInstanceCreated(created)]);
        let lines = binding_lines(&w.process.log_after(seq), SUBMIT);
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert_eq!(lines[0]["refusal"], json!("already-requested"));
        assert_eq!(lines[0]["result"], json!("dropped"));
        assert_eq!(requests_for(client, &instance_id).len(), 1);
        assert_eq!(w.process.host().held(), 0);
    }

    /// `r1-submit-scheduling-request` reacts only where `job_id` is not defined: an
    /// `InstanceCreated` of a job's task is not submitted.
    #[test]
    fn an_instance_created_of_a_job_task_is_not_submitted() {
        let w = World::new();
        w.process.host().tick();
        let mut created = instance_created(&w.ports());
        created.job_id = Some(weft_types::shared::JobId(Uuid(weft_impl::context::mint())));
        let seq = w.process.last_seq();
        w.process
            .host()
            .replay(vec![SystemEvent::WorkloadsInstanceCreated(created)]);
        assert_eq!(w.process.log_after(seq), Vec::<Value>::new());
    }

    /// Statement 5: one tick sends one `RequestReplica` for a service below scale (the positive
    /// control), and none for the same service at scale. The tick's request line carries the
    /// Scheduler and the contract's status; its bindings create the instance and its request.
    #[test]
    fn a_tick_requests_a_replica_for_a_service_below_scale_and_none_at_scale() {
        let w = World::new();
        let seq = w.process.last_seq();
        w.process.host().tick();
        let after = w.process.log_after(seq);
        let sent = request_lines(&after, "weft.workloads.RequestReplica");
        assert_eq!(sent.len(), 1, "{after:?}");
        assert_eq!(sent[0]["actor"], json!("weft.actors.Scheduler"));
        assert_eq!(sent[0]["status"], json!(202));
        let names: Vec<&str> = after
            .iter()
            .map(|line| {
                line["name"]
                    .as_str()
                    .or_else(|| line["binding"].as_str())
                    .or_else(|| line["operation"].as_str())
                    .unwrap_or_default()
            })
            .collect();
        assert_eq!(
            names,
            [
                "weft.workloads.RequestReplica",
                "weft.workloads.ReplicaRequested",
                CREATE_REPLICA,
                "weft.workloads.InstanceCreated",
                SUBMIT,
                "weft.scheduling.SchedulingRequested",
            ],
            "{after:?}"
        );
        let rows = instances(w.process.client(), &w.ids.workload_id);
        assert_eq!(rows.len(), 1, "{rows:?}");
        assert_eq!(rows[0]["replica_index"], json!(0));

        let seq = w.process.last_seq();
        w.process.host().tick();
        let after = w.process.log_after(seq);
        assert_eq!(
            request_lines(&after, "weft.workloads.RequestReplica"),
            Vec::<Value>::new()
        );
        assert_eq!(after, Vec::<Value>::new());
    }

    /// The replica index is the lowest below `replicas` with no live instance: with replica 0
    /// live and the count raised to 3, ticks request 1 and then 2.
    #[test]
    fn a_tick_requests_the_lowest_index_with_no_live_instance() {
        let w = World::new();
        w.process.host().tick();
        let mut ports = w.ports();
        let key = ServiceId(Uuid(w.ids.service_id.clone()));
        let mut service =
            weft_types::behaviour::ServiceStorage::get(&ports, &key).expect("the service row");
        service.data.replicas = 3;
        weft_types::behaviour::ServiceStorage::put(&mut ports, service);
        w.process.host().tick();
        w.process.host().tick();
        let mut indexes: Vec<i64> = instances(w.process.client(), &w.ids.workload_id)
            .iter()
            .map(|row| row["replica_index"].as_i64().expect("a replica index"))
            .collect();
        indexes.sort_unstable();
        assert_eq!(indexes, [0, 1, 2]);
    }

    #[test]
    fn a_tick_requests_again_the_index_of_a_replica_that_is_no_longer_live() {
        let w = World::new();
        w.process.host().tick();
        let mut ports = w.ports();
        let mut failed = InstanceStorage::list(&ports)
            .into_iter()
            .next()
            .expect("the instance of replica 0");
        failed.state = weft_types::workloads::InstanceState::Failed;
        InstanceStorage::put(&mut ports, failed);
        let key = ServiceId(Uuid(w.ids.service_id.clone()));
        let mut service =
            weft_types::behaviour::ServiceStorage::get(&ports, &key).expect("the service row");
        service.data.running_replicas = 0;
        weft_types::behaviour::ServiceStorage::put(&mut ports, service);
        w.process.host().tick();
        let mut replicas: Vec<(Option<i64>, String)> = InstanceStorage::list(&w.ports())
            .into_iter()
            .map(|row| (row.data.replica_index, format!("{:?}", row.state)))
            .collect();
        replicas.sort_unstable();
        assert_eq!(
            replicas,
            [
                (Some(0), "Failed".to_owned()),
                (Some(0), "Pending".to_owned())
            ]
        );
    }
}

/// `weft.workloads.CreateInstance` and `weft.scheduling.SubmitSchedulingRequest` called on the
/// ports of an in-process host after the path and the definitions: each guard in the selection
/// precedence of its contract (`PLAN.md`), and the rows and event of the accepting outcome.
mod obligations {
    use serde_json::json;
    use weft_impl::bindings::outcome_name;
    use weft_impl::context::mint;
    use weft_impl::Ports;
    use weft_testkit::steps;
    use weft_types::behaviour::{
        BoxStorage, GangStorage, InstanceStorage, InvocationStorage, JobStorage,
        PriorityClassStorage, QuotaStorage, SchedulingRequestStorage, ServiceStorage,
        SessionStorage, WarmPoolStorage, WorkloadStorage,
    };
    use weft_types::primitives::{Timestamp, Uuid};
    use weft_types::scheduling::obligations::SubmitSchedulingRequestBehavior;
    use weft_types::scheduling::{
        GangData, GangSnapshot, GangState, SchedulingRequestState, SubmitSchedulingRequest,
        SubmitSchedulingRequestOutcome,
    };
    use weft_types::shared::{
        AccountId, BoxId, FormationId, GangId, InstanceId, InvocationId, JobId, PrincipalId,
        PriorityClassName, QuotaId, ServiceId, SessionId, TenantId, WarmPoolId, WorkloadId,
    };
    use weft_types::workloads::obligations::CreateInstanceBehavior;
    use weft_types::workloads::{
        BoxData, BoxSize, BoxSnapshot, BoxState, CreateInstance, CreateInstanceOutcome,
        InstanceCreated, InstanceState, InvocationData, InvocationSnapshot, InvocationState,
        JobData, JobSnapshot, JobState, SessionData, SessionKind, SessionSnapshot, SessionState,
        WarmPoolData, WarmPoolSnapshot, WarmPoolState, WorkloadState,
    };

    use super::world::World;

    fn id() -> Uuid {
        Uuid(mint())
    }

    fn instances(ports: &Ports) -> usize {
        InstanceStorage::list(ports).len()
    }

    fn requests(ports: &Ports) -> usize {
        SchedulingRequestStorage::list(ports).len()
    }

    /// The replica request of the world's service for index 0.
    fn replica(w: &World) -> CreateInstance {
        CreateInstance {
            workload_id: WorkloadId(Uuid(w.ids.workload_id.clone())),
            job_id: None,
            task_index: None,
            invocation_id: None,
            box_id: None,
            session_id: None,
            replica_index: Some(0),
            warm_pool_id: None,
            service_id: Some(ServiceId(Uuid(w.ids.service_id.clone()))),
        }
    }

    /// `input` answers `outcome` and adds no instance.
    fn refuses(w: &World, input: CreateInstance, outcome: &str) {
        let mut ports = w.ports();
        let before = instances(&ports);
        let answer =
            CreateInstanceBehavior::create_instance(&mut ports, input.clone()).expect("an outcome");
        assert_eq!(outcome_name(&answer), outcome, "{input:?}: {answer:?}");
        assert_eq!(instances(&ports), before, "{input:?} added an instance");
    }

    /// `input` creates an instance: its event.
    fn creates(w: &World, input: CreateInstance) -> InstanceCreated {
        let mut ports = w.ports();
        match CreateInstanceBehavior::create_instance(&mut ports, input.clone())
            .expect("an outcome")
        {
            CreateInstanceOutcome::Created { instance_created } => instance_created,
            other => panic!("{input:?}: {other:?}"),
        }
    }

    /// A copy of the world's workload under a new identity, changed by `change`: its identity.
    fn workload_copy(
        w: &World,
        change: impl FnOnce(&mut weft_types::workloads::WorkloadSnapshot),
    ) -> WorkloadId {
        let mut ports = w.ports();
        let mut row = WorkloadStorage::get(&ports, &WorkloadId(Uuid(w.ids.workload_id.clone())))
            .expect("the workload row");
        let workload = WorkloadId(id());
        row.data.workload_id = workload.clone();
        change(&mut row);
        WorkloadStorage::put(&mut ports, row);
        workload
    }

    /// The owner rows of an instance (a job, an invocation, a box, a session, a warm pool), each
    /// of the workload `of`: their identities.
    struct Owners {
        job: JobId,
        invocation: InvocationId,
        boxed: BoxId,
        session: SessionId,
        warm_pool: WarmPoolId,
    }

    fn owners(w: &World, of: &WorkloadId) -> Owners {
        let mut ports = w.ports();
        let account = AccountId(Uuid(w.path.account_id.clone()));
        let owners = Owners {
            job: JobId(id()),
            invocation: InvocationId(id()),
            boxed: BoxId(id()),
            session: SessionId(id()),
            warm_pool: WarmPoolId(id()),
        };
        JobStorage::put(
            &mut ports,
            JobSnapshot {
                state: JobState::Running,
                data: JobData {
                    job_id: owners.job.clone(),
                    workload_id: of.clone(),
                    account_id: account.clone(),
                    parallelism: 1,
                    gang: false,
                    max_retries: 0,
                    retries_used: 0,
                    succeeded_tasks: 0,
                    outputs: Vec::new(),
                    failure: None,
                    schedule_id: None,
                },
            },
        );
        InvocationStorage::put(
            &mut ports,
            InvocationSnapshot {
                state: InvocationState::Accepted,
                data: InvocationData {
                    invocation_id: owners.invocation.clone(),
                    workload_id: of.clone(),
                    account_id: account.clone(),
                    deadline_at: Timestamp("2026-01-01T00:00:00Z".to_owned()),
                    request: Vec::new(),
                    result: None,
                    failure: None,
                    instance_id: None,
                    schedule_id: None,
                    trigger_id: None,
                },
            },
        );
        BoxStorage::put(
            &mut ports,
            BoxSnapshot {
                state: BoxState::Created,
                data: BoxData {
                    box_id: owners.boxed.clone(),
                    workload_id: of.clone(),
                    account_id: account.clone(),
                    size: BoxSize {
                        cpus: 1,
                        memory_mib: 256,
                        gpus: 0,
                    },
                    volume_id: None,
                    endpoint_id: None,
                    instance_id: None,
                    cordoned: false,
                    lease_holder: None,
                    lease_nonce: None,
                    lease_expires_at: None,
                },
            },
        );
        SessionStorage::put(
            &mut ports,
            SessionSnapshot {
                state: SessionState::Open,
                data: SessionData {
                    session_id: owners.session.clone(),
                    workload_id: of.clone(),
                    account_id: account.clone(),
                    kind: SessionKind::Terminal,
                    principal_id: PrincipalId(id()),
                    attached: false,
                },
            },
        );
        WarmPoolStorage::put(
            &mut ports,
            WarmPoolSnapshot {
                state: WarmPoolState::Active,
                data: WarmPoolData {
                    warm_pool_id: owners.warm_pool.clone(),
                    workload_id: of.clone(),
                    account_id: account,
                    size: 1,
                },
            },
        );
        owners
    }

    #[test]
    fn create_instance_answers_each_guard_in_its_precedence() {
        let w = World::new();
        let base = replica(&w);
        let other = WorkloadId(id());
        let foreign = owners(&w, &other);
        let own = owners(&w, &base.workload_id);

        // Existence, in declaration order.
        refuses(
            &w,
            CreateInstance {
                workload_id: WorkloadId(id()),
                ..base.clone()
            },
            "no-workload",
        );
        let missing = CreateInstance {
            job_id: Some(JobId(id())),
            invocation_id: Some(InvocationId(id())),
            box_id: Some(BoxId(id())),
            session_id: Some(SessionId(id())),
            warm_pool_id: Some(WarmPoolId(id())),
            service_id: Some(ServiceId(id())),
            ..base.clone()
        };
        refuses(&w, missing.clone(), "no-job");
        let missing = CreateInstance {
            job_id: Some(own.job.clone()),
            ..missing
        };
        refuses(&w, missing.clone(), "no-invocation");
        let missing = CreateInstance {
            invocation_id: Some(own.invocation.clone()),
            ..missing
        };
        refuses(&w, missing.clone(), "no-box");
        let missing = CreateInstance {
            box_id: Some(own.boxed.clone()),
            ..missing
        };
        refuses(&w, missing.clone(), "no-session");
        let missing = CreateInstance {
            session_id: Some(own.session.clone()),
            ..missing
        };
        refuses(&w, missing.clone(), "no-warm-pool");
        let missing = CreateInstance {
            warm_pool_id: Some(own.warm_pool.clone()),
            ..missing
        };
        refuses(&w, missing, "no-service");

        // A retired workload whose service is missing: existence answers first.
        let retired = workload_copy(&w, |row| row.state = WorkloadState::Retired);
        refuses(
            &w,
            CreateInstance {
                workload_id: retired.clone(),
                service_id: Some(ServiceId(id())),
                ..base.clone()
            },
            "no-service",
        );

        // The predicates, in declaration order: a retired workload whose owners are all another
        // workload's answers `workload-retired`, and each foreign owner after it.
        let all_foreign = CreateInstance {
            job_id: Some(foreign.job.clone()),
            invocation_id: Some(foreign.invocation.clone()),
            box_id: Some(foreign.boxed.clone()),
            session_id: Some(foreign.session.clone()),
            warm_pool_id: Some(foreign.warm_pool.clone()),
            ..base.clone()
        };
        refuses(
            &w,
            CreateInstance {
                workload_id: retired,
                ..all_foreign.clone()
            },
            "workload-retired",
        );
        refuses(&w, all_foreign.clone(), "foreign-job");
        let all_foreign = CreateInstance {
            job_id: Some(own.job.clone()),
            ..all_foreign
        };
        refuses(&w, all_foreign.clone(), "foreign-invocation");
        let all_foreign = CreateInstance {
            invocation_id: Some(own.invocation.clone()),
            ..all_foreign
        };
        refuses(&w, all_foreign.clone(), "foreign-box");
        let all_foreign = CreateInstance {
            box_id: Some(own.boxed.clone()),
            ..all_foreign
        };
        refuses(&w, all_foreign.clone(), "foreign-session");
        let all_foreign = CreateInstance {
            session_id: Some(own.session.clone()),
            ..all_foreign
        };
        refuses(&w, all_foreign, "foreign-warm-pool");
        let copy = workload_copy(&w, |_| {});
        refuses(
            &w,
            CreateInstance {
                workload_id: copy.clone(),
                ..base.clone()
            },
            "foreign-service",
        );

        // Replica 0 is created, and a second request for it is `replica-live`.
        let created = creates(&w, base.clone());
        refuses(&w, base.clone(), "replica-live");
        // Only a replica's request is checked: without an index, another instance is made.
        creates(
            &w,
            CreateInstance {
                replica_index: None,
                ..base.clone()
            },
        );
        // An ended replica is not live: its index is made again.
        let mut ports = w.ports();
        let mut ended =
            InstanceStorage::get(&ports, &created.instance_id).expect("the instance row");
        ended.state = InstanceState::Exited;
        InstanceStorage::put(&mut ports, ended);
        creates(&w, base.clone());
        // Another service's replica of the same index is not this one's.
        let mut other_service =
            ServiceStorage::get(&ports, base.service_id.as_ref().expect("a service"))
                .expect("the service row");
        let other_service_id = ServiceId(id());
        other_service.data.service_id = other_service_id.clone();
        ServiceStorage::put(&mut ports, other_service);
        creates(
            &w,
            CreateInstance {
                service_id: Some(other_service_id.clone()),
                ..base.clone()
            },
        );
        // Index 1 of the world's service is not index 0.
        creates(
            &w,
            CreateInstance {
                replica_index: Some(1),
                ..base.clone()
            },
        );

        // Over quota: 4 live instances claim 4 cpu, the limit is 4. A live replica still
        // answers `replica-live` first.
        let mut quota = QuotaStorage::get(&ports, &QuotaId(Uuid(w.path.quota_id.clone())))
            .expect("the quota row");
        assert_eq!(
            InstanceStorage::list(&ports)
                .iter()
                .filter(|row| row.state != InstanceState::Exited)
                .count(),
            4
        );
        quota.data.limit = 4;
        QuotaStorage::put(&mut ports, quota.clone());
        refuses(&w, base.clone(), "replica-live");
        refuses(
            &w,
            CreateInstance {
                replica_index: Some(2),
                ..base.clone()
            },
            "over-quota",
        );
        // An instance of another account does not count against this account's quota.
        let mut stranger =
            InstanceStorage::get(&ports, &created.instance_id).expect("the instance row");
        stranger.state = InstanceState::Running;
        stranger.data.instance_id = InstanceId(id());
        stranger.data.service_id = None;
        stranger.data.account_id = AccountId(id());
        InstanceStorage::put(&mut ports, stranger);
        quota.data.limit = 5;
        QuotaStorage::put(&mut ports, quota.clone());
        creates(
            &w,
            CreateInstance {
                replica_index: Some(2),
                ..base.clone()
            },
        );
        // A quota above the account's bounds it too.
        let parent = QuotaId(id());
        let mut above = quota.clone();
        above.data.quota_id = parent.clone();
        above.data.account_id = AccountId(id());
        above.data.limit = 5;
        QuotaStorage::put(&mut ports, above);
        quota.data.limit = 64;
        quota.data.parent_quota_id = Some(parent);
        QuotaStorage::put(&mut ports, quota.clone());
        refuses(
            &w,
            CreateInstance {
                replica_index: Some(3),
                ..base.clone()
            },
            "over-quota",
        );
        // No quota for the class: none of it may be held.
        QuotaStorage::delete(&mut ports, &quota.data.quota_id);
        refuses(
            &w,
            CreateInstance {
                replica_index: Some(3),
                ..base.clone()
            },
            "over-quota",
        );
    }

    #[test]
    fn create_instance_writes_the_instance_and_publishes_what_the_scheduler_reads() {
        let w = World::new();
        let ports = w.ports();
        let event = creates(&w, replica(&w));
        let row = InstanceStorage::get(&ports, &event.instance_id).expect("the instance row");
        let workload = WorkloadStorage::get(&ports, &WorkloadId(Uuid(w.ids.workload_id.clone())))
            .expect("the workload row");
        assert_eq!(row.state, InstanceState::Pending);
        let data = &row.data;
        assert_eq!(data.workload_id, workload.data.workload_id);
        assert_eq!(data.account_id, AccountId(Uuid(w.path.account_id.clone())));
        assert_eq!(data.artifact_digest, workload.data.artifact_digest);
        assert_eq!(data.minimum_isolation, workload.data.isolation);
        assert_eq!(data.processes, workload.data.processes);
        assert_eq!(data.restart_policy, workload.data.restart_policy);
        assert_eq!(data.replica_index, Some(0));
        assert_eq!(
            data.service_id,
            Some(ServiceId(Uuid(w.ids.service_id.clone())))
        );
        assert!(!data.replica_counted_out && !data.ready);
        assert_eq!(data.desired, weft_types::workloads::DesiredRun::Run);
        assert_eq!(data.health, weft_types::workloads::InstanceHealth::Unknown);
        assert_eq!((data.restarts, data.restarts_in_window), (0, 0));
        assert!(data.node_id.is_none() && data.isolation.is_none() && data.job_id.is_none());
        assert_eq!(data.broken_invariant(), None);

        assert_eq!(event.workload_id, workload.data.workload_id);
        assert_eq!(event.minimum_isolation, workload.data.isolation);
        assert_eq!(event.account_id, data.account_id);
        assert_eq!(
            event.formation_id,
            FormationId(Uuid(w.ids.formation_id.clone()))
        );
        assert_eq!(event.claims, workload.data.claims);
        assert_eq!(event.service_id, data.service_id);
        assert_eq!(event.replica_index, Some(0));
        assert_eq!(event.job_id, None);
        let formation = weft_types::behaviour::FormationStorage::get(&ports, &event.formation_id)
            .expect("the formation row");
        assert!(!formation.data.zones.is_empty());
        assert_eq!(event.zones, formation.data.zones);
        assert_eq!(event.priority, PriorityClassName("standard".to_owned()));
    }

    /// The request the binding sends for `created`.
    fn submission(created: &InstanceCreated) -> SubmitSchedulingRequest {
        SubmitSchedulingRequest {
            instance_id: created.instance_id.clone(),
            account_id: created.account_id.clone(),
            formation_id: created.formation_id.clone(),
            claims: created.claims.clone(),
            min_isolation: created.minimum_isolation,
            zones: created.zones.clone(),
            priority: created.priority.clone(),
            gang_id: None,
        }
    }

    fn submit(w: &World, input: SubmitSchedulingRequest) -> SubmitSchedulingRequestOutcome {
        let mut ports = w.ports();
        SubmitSchedulingRequestBehavior::submit_scheduling_request(&mut ports, input)
            .expect("an outcome")
    }

    /// `input` answers `outcome` and adds no request.
    fn declines(w: &World, input: SubmitSchedulingRequest, outcome: &str) {
        let before = requests(&w.ports());
        let answer = submit(w, input.clone());
        assert_eq!(outcome_name(&answer), outcome, "{input:?}: {answer:?}");
        assert_eq!(requests(&w.ports()), before, "{input:?} added a request");
    }

    /// A gang of `tenant` in `state`: its identity.
    fn gang(w: &World, tenant: &str, state: GangState) -> GangId {
        let gang_id = GangId(id());
        GangStorage::put(
            &mut w.ports(),
            GangSnapshot {
                state,
                data: GangData {
                    gang_id: gang_id.clone(),
                    tenant_id: TenantId(Uuid(tenant.to_owned())),
                    job_id: JobId(id()),
                    size: 2,
                    registered_at: Timestamp("2026-01-01T00:00:00Z".to_owned()),
                    ended_at: None,
                },
            },
        );
        gang_id
    }

    #[test]
    fn submit_scheduling_request_answers_each_guard_in_its_precedence() {
        let w = World::new();
        let base = submission(&creates(&w, replica(&w)));
        let client = w.process.client();
        let globex_tenant = steps::create_tenant(client, "globex");
        let globex = steps::create_account(client, &globex_tenant);
        let globex_caller = steps::account_caller(&globex_tenant, &globex);
        let globex_formation =
            steps::create_formation(client, &globex_caller, &globex, &w.path.cluster_id);
        let acme_caller = steps::account_caller(&w.path.tenant_id, &w.path.account_id);
        let mut other_input =
            steps::input::create_formation(&w.path.account_id, &w.path.cluster_id);
        other_input["name"] = json!("other");
        let other_formation = client
            .command(
                "weft.formations.CreateFormation",
                steps::ACCOUNT_ADMIN,
                acme_caller,
                other_input,
            )
            .expect("the control host answers")
            .body["published"][0]["payload"]["formation_id"]
            .as_str()
            .expect("a formation id")
            .to_owned();

        // The input guard.
        declines(
            &w,
            SubmitSchedulingRequest {
                claims: Vec::new(),
                instance_id: InstanceId(id()),
                ..base.clone()
            },
            "no-claims",
        );

        // Existence, in declaration order, ahead of every predicate.
        let missing = SubmitSchedulingRequest {
            account_id: AccountId(id()),
            instance_id: InstanceId(id()),
            formation_id: FormationId(id()),
            priority: PriorityClassName("nope".to_owned()),
            gang_id: Some(GangId(id())),
            ..base.clone()
        };
        declines(&w, missing.clone(), "unknown-account");
        let missing = SubmitSchedulingRequest {
            account_id: AccountId(Uuid(globex.clone())),
            ..missing
        };
        declines(&w, missing.clone(), "unknown-instance");
        let missing = SubmitSchedulingRequest {
            instance_id: base.instance_id.clone(),
            ..missing
        };
        declines(&w, missing.clone(), "unknown-formation");
        let missing = SubmitSchedulingRequest {
            formation_id: base.formation_id.clone(),
            ..missing
        };
        declines(&w, missing.clone(), "unknown-priority");
        let missing = SubmitSchedulingRequest {
            priority: base.priority.clone(),
            ..missing
        };
        declines(&w, missing, "unknown-gang");

        // The predicates, in declaration order.
        let closed = gang(&w, &w.path.tenant_id, GangState::Bound);
        let mismatched = SubmitSchedulingRequest {
            account_id: AccountId(Uuid(globex.clone())),
            formation_id: FormationId(Uuid(other_formation.clone())),
            gang_id: Some(closed.clone()),
            ..base.clone()
        };
        declines(&w, mismatched.clone(), "instance-of-other-account");
        declines(
            &w,
            SubmitSchedulingRequest {
                account_id: base.account_id.clone(),
                formation_id: FormationId(Uuid(globex_formation)),
                ..mismatched.clone()
            },
            "formation-of-other-account",
        );
        declines(
            &w,
            SubmitSchedulingRequest {
                account_id: base.account_id.clone(),
                ..mismatched
            },
            "formation-not-instances",
        );
        let mut ports = w.ports();
        let standard = PriorityClassName("standard".to_owned());
        let mut class =
            PriorityClassStorage::get(&ports, &standard).expect("the priority class row");
        class.state = weft_types::infrastructure::PriorityClassState::Retired;
        PriorityClassStorage::put(&mut ports, class.clone());
        declines(
            &w,
            SubmitSchedulingRequest {
                gang_id: Some(closed.clone()),
                ..base.clone()
            },
            "retired-priority",
        );
        class.state = weft_types::infrastructure::PriorityClassState::Active;
        PriorityClassStorage::put(&mut ports, class);
        declines(
            &w,
            SubmitSchedulingRequest {
                gang_id: Some(closed.clone()),
                ..base.clone()
            },
            "gang-closed",
        );
        let foreign_gang = gang(&w, &globex_tenant, GangState::Forming);
        declines(
            &w,
            SubmitSchedulingRequest {
                gang_id: Some(foreign_gang),
                ..base.clone()
            },
            "gang-of-other-tenant",
        );

        // Requested once; a second request while it is pending is `already-requested`, also
        // ahead of a closed gang.
        let SubmitSchedulingRequestOutcome::Requested {
            scheduling_requested,
        } = submit(&w, base.clone())
        else {
            panic!("the first submission is not requested");
        };
        declines(&w, base.clone(), "already-requested");
        declines(
            &w,
            SubmitSchedulingRequest {
                gang_id: Some(closed),
                ..base.clone()
            },
            "already-requested",
        );
        let mut request = SchedulingRequestStorage::get(&ports, &scheduling_requested.request_id)
            .expect("the request");
        request.state = SchedulingRequestState::Unschedulable;
        SchedulingRequestStorage::put(&mut ports, request.clone());
        declines(&w, base.clone(), "already-requested");
        // A placed request is answered: the instance may be submitted again.
        request.state = SchedulingRequestState::Placed;
        SchedulingRequestStorage::put(&mut ports, request);
        assert_eq!(outcome_name(&submit(&w, base.clone())), "requested");
        // A request of another instance is not this one's.
        let second = submission(&creates(
            &w,
            CreateInstance {
                replica_index: Some(1),
                ..replica(&w)
            },
        ));
        assert_eq!(outcome_name(&submit(&w, second)), "requested");
    }

    #[test]
    fn submit_scheduling_request_writes_what_the_priority_class_formation_and_account_name() {
        let w = World::new();
        let ports = w.ports();
        let base = submission(&creates(&w, replica(&w)));
        let SubmitSchedulingRequestOutcome::Requested {
            scheduling_requested,
        } = submit(&w, base.clone())
        else {
            panic!("not requested");
        };
        let row = SchedulingRequestStorage::get(&ports, &scheduling_requested.request_id)
            .expect("the request row");
        assert_eq!(row.state, SchedulingRequestState::Pending);
        let data = &row.data;
        assert_eq!(data.instance_id, base.instance_id);
        assert_eq!(data.tenant_id, TenantId(Uuid(w.path.tenant_id.clone())));
        assert_eq!(data.account_id, base.account_id);
        assert_eq!(data.formation_id, base.formation_id);
        assert_eq!(data.cluster_id.0 .0, w.path.cluster_id);
        assert_eq!(data.claims, base.claims);
        assert_eq!(data.min_isolation, base.min_isolation);
        assert_eq!(data.zones, base.zones);
        assert_eq!(data.priority, base.priority);
        assert_eq!(
            (
                data.priority_value,
                data.preempts,
                data.preemptible,
                data.grace_seconds
            ),
            (100, false, true, 30)
        );
        assert_eq!(data.gang_id, None);
        assert!(data.unschedulable_reason.is_none() && data.unschedulable_detail.is_none());
        assert!(
            data.requested_at.0.ends_with('Z') && data.requested_at.0.len() >= 20,
            "{:?}",
            data.requested_at
        );
        assert_eq!(scheduling_requested.request_id, data.request_id);
        assert_eq!(scheduling_requested.instance_id, data.instance_id);
        assert_eq!(scheduling_requested.tenant_id, data.tenant_id);
        assert_eq!(scheduling_requested.cluster_id, data.cluster_id);
        assert_eq!(scheduling_requested.min_isolation, data.min_isolation);
        assert_eq!(scheduling_requested.priority, data.priority);
    }

    /// `replica-live` and the quota count an instance in `Pending`, `Scheduled`, `Starting`,
    /// `Running` or `Paused` and no other: with a quota of 1 cpu and every instance in a live
    /// state, replica 0 is live and replica 1 over the quota; in any other state both are
    /// created.
    #[test]
    fn replica_live_and_the_quota_count_the_five_live_states_and_no_other() {
        let w = World::new();
        let mut ports = w.ports();
        let key = QuotaId(Uuid(w.path.quota_id.clone()));
        let mut quota = QuotaStorage::get(&ports, &key).expect("the quota row");
        quota.data.limit = 1;
        QuotaStorage::put(&mut ports, quota);
        let set_all = |ports: &mut Ports, state: InstanceState| {
            for mut row in InstanceStorage::list(ports) {
                row.state = state;
                InstanceStorage::put(ports, row);
            }
        };
        let second = CreateInstance {
            replica_index: Some(1),
            ..replica(&w)
        };
        creates(&w, replica(&w));
        for state in [
            InstanceState::Pending,
            InstanceState::Scheduled,
            InstanceState::Starting,
            InstanceState::Running,
            InstanceState::Paused,
        ] {
            set_all(&mut ports, state);
            refuses(&w, replica(&w), "replica-live");
            refuses(&w, second.clone(), "over-quota");
        }
        for state in [
            InstanceState::Exited,
            InstanceState::Failed,
            InstanceState::Evicted,
        ] {
            set_all(&mut ports, state);
            creates(&w, replica(&w));
            set_all(&mut ports, state);
            creates(&w, second.clone());
        }
    }
}

/// The runner of the hand-run bindings (`weft_impl::bindings`) with a binding of its own whose
/// port answers an unmet obligation, on a state file of its own.
mod runner {
    use weft_impl::bindings::{Answer, Entry, Invoked, Runner};
    use weft_impl::context::mint;
    use weft_impl::log::BindingResult;
    use weft_impl::{Ports, System};
    use weft_system::SystemEvent;
    use weft_types::primitives::Uuid;
    use weft_types::shared::{ServiceId, WorkloadId};
    use weft_types::workloads::ReplicaRequested;

    /// Reacts to `ReplicaRequested` as `r1-create-replica` does; its port answers an unmet
    /// obligation, as one whose transport failed.
    fn unmet(_: &mut System, event: &SystemEvent) -> Option<Invoked> {
        matches!(event, SystemEvent::WorkloadsReplicaRequested(_))
            .then(|| Invoked::Unmet("the port is down".to_owned()))
    }

    static UNMET: Entry = Entry {
        id: "unmet",
        drop: &[],
        invoke: unmet,
        encode: weft_impl::bindings::r1_create_replica::ENTRY.encode,
        decode: weft_impl::bindings::r1_create_replica::ENTRY.decode,
    };

    static ENTRIES: &[&Entry] = &[&UNMET];

    /// An unmet obligation is retried: the event is held in the state file, attempted again
    /// after waits of 1 and 2 ticks, still held after the state file is closed and opened
    /// again, and due at the first tick of the runner that opens it.
    #[test]
    fn an_unmet_obligation_is_held_in_the_state_file_and_due_when_a_runner_opens() {
        let dir = weft_testkit::TempDir::new();
        let state = dir.path().join("state.db");
        let event = SystemEvent::WorkloadsReplicaRequested(ReplicaRequested {
            service_id: ServiceId(Uuid(mint())),
            workload_id: WorkloadId(Uuid(mint())),
            replica_index: 0,
        });
        {
            let ports = Ports::open(&state).expect("the state file opens");
            let mut system = weft_impl::system(&ports);
            let runner = Runner::open_with(&ports, ENTRIES);
            let answers = runner.deliver(&mut system, vec![event.clone()]);
            assert_eq!(
                answers,
                [Answer {
                    binding: "unmet",
                    refusal: None,
                    result: BindingResult::Retried,
                    attempt: 1,
                    events: Vec::new(),
                }]
            );
            assert_eq!(runner.held_events(), std::slice::from_ref(&event));
            let attempts = |answers: Vec<Answer>| -> Vec<u32> {
                answers.iter().map(|answer| answer.attempt).collect()
            };
            assert_eq!(attempts(runner.retry(&mut system)), [2]);
            assert_eq!(attempts(runner.retry(&mut system)), Vec::<u32>::new());
            assert_eq!(attempts(runner.retry(&mut system)), [3]);
            let held = ports.store().held();
            assert_eq!((held[0].attempts, held[0].wait), (3, 4), "{held:?}");
        }

        let ports = Ports::open(&state).expect("the state file opens again");
        let mut system = weft_impl::system(&ports);
        let runner = Runner::open_with(&ports, ENTRIES);
        assert_eq!(runner.held_events(), [event]);
        let held = ports.store().held();
        assert_eq!((held[0].attempts, held[0].wait), (3, 1), "{held:?}");
        let answers = runner.retry(&mut system);
        assert_eq!(answers.len(), 1, "{answers:?}");
        assert_eq!(
            (answers[0].attempt, answers[0].result),
            (4, BindingResult::Retried)
        );
        assert_eq!(runner.held(), 1);
    }
}

/// Adversary pass on `story:skeleton-replica-instances`: a replica the Scheduler requested while
/// its `CreateInstance` is held for another attempt.
mod adversary {
    use std::path::Path as FsPath;
    use std::time::{Duration, Instant};

    use serde_json::json;
    use weft_impl::Ports;
    use weft_testkit::steps;
    use weft_testkit::ControlHost;
    use weft_types::behaviour::{InstanceStorage, QuotaStorage, ServiceStorage};
    use weft_types::primitives::Uuid;
    use weft_types::shared::{QuotaId, ServiceId};

    use super::world::{binding_lines, instances, World};

    const CREATE_REPLICA: &str = "r1-create-replica";

    fn set_limit(ports: &mut Ports, quota_id: &str, limit: i64) {
        let key = QuotaId(Uuid(quota_id.to_owned()));
        let mut quota = QuotaStorage::get(ports, &key).expect("the quota row");
        quota.data.limit = limit;
        quota.data.remaining = limit;
        QuotaStorage::put(ports, quota);
    }

    fn set_limit_in(state: &FsPath, quota_id: &str, limit: i64) {
        let mut ports = Ports::open(state).expect("the stopped host's state file opens");
        set_limit(&mut ports, quota_id, limit);
    }

    /// A service of 2 replicas whose account quota is used up: tick 1 requests replica 0 and
    /// `CreateInstance` answers `over-quota` (held); tick 2 attempts it again and the level step,
    /// which sees no live instance of index 0, requests index 0 a second time. Once the quota
    /// admits both, the service must hold replicas 0 and 1 (`ess/4-applications/workloads.yaml`
    /// RequestReplica OPEN: "the controller requests one replica per service at a time and waits
    /// for InstanceCreated or the refusal").
    #[test]
    fn a_held_replica_is_not_requested_twice_and_every_index_is_made_once_the_quota_admits_it() {
        let w = World::new();
        let mut ports = w.ports();
        set_limit(&mut ports, &w.path.quota_id, 0);
        let key = ServiceId(Uuid(w.ids.service_id.clone()));
        let mut service = ServiceStorage::get(&ports, &key).expect("the service row");
        service.data.replicas = 2;
        ServiceStorage::put(&mut ports, service);

        w.process.host().tick();
        w.process.host().tick();
        assert_eq!(InstanceStorage::list(&w.ports()).len(), 0);

        set_limit(&mut ports, &w.path.quota_id, 64);
        for _ in 0..4 {
            w.process.host().tick();
        }
        let mut indexes: Vec<i64> = instances(w.process.client(), &w.ids.workload_id)
            .iter()
            .map(|row| row["replica_index"].as_i64().expect("a replica index"))
            .collect();
        indexes.sort_unstable();
        let service = ServiceStorage::get(&w.ports(), &key).expect("the service row");
        assert_eq!(
            (indexes, service.data.running_replicas),
            (vec![0, 1], 2),
            "replicas made and the service's live count after the quota was raised"
        );
    }

    /// A replica whose `CreateInstance` was answered `over-quota` is held for another attempt;
    /// the control host restarts; the quota is raised. `r1-create-replica` is
    /// `delivery: at_least_once` and retries `over-quota` (`ess/components.yaml`), and
    /// `RequestReplica` already counted the replica, so the level step never asks again: the
    /// instance must still be made.
    ///
    /// The quota is used up by replica 0 of the service, not by a limit of 0 before the
    /// definitions: `DefineWorkload` and `DefineService` answer `over-quota` themselves when the
    /// workload's claims alone pass a limit. Replica 1 is the held one.
    #[test]
    fn a_replica_held_over_quota_is_made_after_the_control_host_restarts() {
        let mut host = ControlHost::start();
        let path = steps::run_path(&host.client());
        let ids = steps::run_definitions(&host.client(), &path);
        let started = Instant::now();
        while instances(&host.client(), &ids.workload_id).is_empty() {
            assert!(
                started.elapsed() < Duration::from_secs(2),
                "no replica 0 2 s after the definitions"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        host.stop();
        {
            let mut ports = Ports::open(&host.state_path()).expect("the state file opens");
            set_limit(&mut ports, &path.quota_id, 1);
            let key = ServiceId(Uuid(ids.service_id.clone()));
            let mut service = ServiceStorage::get(&ports, &key).expect("the service row");
            service.data.replicas = 2;
            ServiceStorage::put(&mut ports, service);
        }
        let mut host = host.restart();
        let seq = host.last_seq();

        // Positive control: the tick requested replica 1 and its creation is held.
        let started = Instant::now();
        loop {
            let held = binding_lines(&host.log_after(seq), CREATE_REPLICA);
            if held.iter().any(|line| {
                line["refusal"] == json!("over-quota") && line["result"] == json!("retried")
            }) {
                break;
            }
            assert!(
                started.elapsed() < Duration::from_secs(5),
                "no held over-quota attempt of r1-create-replica: {held:?}"
            );
            std::thread::sleep(Duration::from_millis(20));
        }

        host.stop();
        set_limit_in(&host.state_path(), &path.quota_id, 64);
        let host = host.restart();
        let client = host.client();
        let started = Instant::now();
        loop {
            let rows = instances(&client, &ids.workload_id);
            if rows.len() > 1 {
                let mut indexes: Vec<i64> = rows
                    .iter()
                    .map(|row| row["replica_index"].as_i64().expect("a replica index"))
                    .collect();
                indexes.sort_unstable();
                assert_eq!(indexes, [0, 1], "{rows:?}");
                break;
            }
            assert!(
                started.elapsed() < Duration::from_secs(3),
                "3 s after the restart with the quota raised the service has no instance: the held \
                 ReplicaRequested was lost with the process"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}
