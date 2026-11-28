//! The one integration-test binary of weft-controllers.
//!
//! - [`replicas`]: the acceptance of `story:skeleton-replica-instances`, the R1 level step and
//!   the two hand-run bindings of R1, on the `weft` binary and in-process;
//! - [`obligations`]: `weft.workloads.CreateInstance` and `weft.scheduling.SubmitSchedulingRequest`,
//!   each guard in the selection precedence of its contract;
//! - [`runner`]: the runner of the hand-run bindings on a state file of its own;
//! - [`adversary`]: the cases of the adversary pass;
//! - [`placement`], [`repair`] and [`r4_bindings`]: the acceptance of
//!   `story:skeleton-controllers`, the Scheduler's decision step, the R4 level step and the four
//!   hand-run bindings of R4, in-process on a world whose node is made schedulable ([`ready`]);
//! - [`place_instance`] and [`assign`]: `weft.scheduling.PlaceInstance` and `weft.nodes.Assign`,
//!   each guard in the selection precedence of its contract.

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
    use weft_types::workloads::{InstanceCreated, ReplicaRequested, WorkloadSnapshot};

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
            scope_id: instance.scope_id.clone(),
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
        assert_eq!(requests[0]["scope_id"], json!(path.scope_id));
        assert_eq!(requests[0]["formation_id"], json!(ids.formation_id));
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

    /// Takes the row of the world's workload away: a `ReplicaRequested` of its service names a
    /// workload no row holds, and `CreateInstance` answers `no-workload`, off the drop list of
    /// `r1-create-replica`: retried. The row, to put back.
    fn take_workload(w: &World) -> WorkloadSnapshot {
        let mut ports = w.ports();
        let key = WorkloadId(Uuid(w.ids.workload_id.clone()));
        let row = WorkloadStorage::get(&ports, &key).expect("the workload row");
        WorkloadStorage::delete(&mut ports, &key);
        row
    }

    /// Puts the workload's row back: the next attempt creates the instance.
    fn put_back(w: &World, row: WorkloadSnapshot) {
        WorkloadStorage::put(&mut w.ports(), row);
    }
    /// Statement 3, as the blocks moved it: `CreateInstance` is out of the outcome group `quota`.
    /// A `ReplicaRequested` whose instance takes the scope past its `cpu` quota creates the
    /// instance and its scheduling request; `weft.scheduling.PlaceInstance` refuses to place it
    /// (`quota-exceeded`, `placement::a_request_the_quota_refuses_is_attempted_after_doubling_waits_with_one_line_each`).
    #[test]
    fn a_replica_request_past_the_scope_quota_is_created_and_left_to_placement() {
        let w = World::new();
        w.process.host().tick();
        let client = w.process.client();
        // The one live instance claims 1 cpu: the quota is used up.
        set_limit(&w, 1);
        let seq = w.process.last_seq();
        w.process.host().replay(vec![replica_requested(&w, 1)]);
        let after = w.process.log_after(seq);
        let lines = binding_lines(&after, CREATE_REPLICA);
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert_eq!(lines[0]["result"], json!("applied"));
        assert!(lines[0].get("refusal").is_none(), "{lines:?}");
        assert_eq!(binding_lines(&after, SUBMIT)[0]["result"], json!("applied"));
        assert_eq!(instances(client, &w.ids.workload_id).len(), 2);
        assert_eq!(w.process.host().held(), 0);
    }

    /// A refusal of `CreateInstance` off the drop list of `r1-create-replica` (`no-workload`: the
    /// workload's row is gone) is retried, not dropped. It is attempted again
    /// on the next tick, then after a wait of two ticks; once the row is back, the next
    /// attempt creates it.
    #[test]
    fn a_replica_request_refused_off_the_drop_list_is_retried_until_it_is_admitted() {
        let w = World::new();
        w.process.host().tick();
        let client = w.process.client();
        let workload = take_workload(&w);
        let seq = w.process.last_seq();
        w.process.host().replay(vec![replica_requested(&w, 1)]);
        let lines = binding_lines(&w.process.log_after(seq), CREATE_REPLICA);
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert_eq!(lines[0]["refusal"], json!("no-workload"));
        assert_eq!(lines[0]["result"], json!("retried"));
        assert_eq!(lines[0]["attempt"], json!(1));
        assert_eq!(instances(client, &w.ids.workload_id).len(), 1);
        assert_eq!(w.process.host().held(), 1);

        // Still gone: the next tick attempts it again and holds it again.
        let seq = w.process.last_seq();
        w.process.host().tick();
        let lines = binding_lines(&w.process.log_after(seq), CREATE_REPLICA);
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert_eq!(lines[0]["result"], json!("retried"));
        assert_eq!(lines[0]["attempt"], json!(2));

        // Back: the first tick waits, the second attempts it.
        put_back(&w, workload);
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
        let workload = take_workload(&w);
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
                .all(|line| line["refusal"] == json!("no-workload")
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

        // Back: the attempt due at tick 128 creates it.
        put_back(&w, workload);
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
        take_workload(&w);
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
        BoxId, FormationId, GangId, InstanceId, InvocationId, JobId, PrincipalId,
        PriorityClassName, QuotaId, ScopeId, ServiceId, SessionId, WarmPoolId, WorkloadId,
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
        let scope = ScopeId(Uuid(w.path.scope_id.clone()));
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
                    scope_id: scope.clone(),
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
                    scope_id: scope.clone(),
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
                    scope_id: scope.clone(),
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
                    scope_id: scope.clone(),
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
                    scope_id: scope,
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

        // Out of the outcome group `quota` (the blocks): with no quota for the class left, a live
        // replica still answers `replica-live`, and a new one is created; `PlaceInstance`
        // admits it by the scope's quota
        // (`place_instance::place_instance_admits_by_the_quota_of_the_scope_and_every_quota_above_it`).
        QuotaStorage::delete(&mut ports, &QuotaId(Uuid(w.path.quota_id.clone())));
        refuses(&w, base.clone(), "replica-live");
        creates(
            &w,
            CreateInstance {
                replica_index: Some(2),
                ..base.clone()
            },
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
        assert_eq!(data.scope_id, ScopeId(Uuid(w.path.scope_id.clone())));
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
        assert_eq!(event.scope_id, data.scope_id);
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
            scope_id: created.scope_id.clone(),
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

    /// A gang of the scope `scope` in `state`: its identity.
    fn gang(w: &World, scope: &str, state: GangState) -> GangId {
        let gang_id = GangId(id());
        GangStorage::put(
            &mut w.ports(),
            GangSnapshot {
                state,
                data: GangData {
                    gang_id: gang_id.clone(),
                    scope_id: ScopeId(Uuid(scope.to_owned())),
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
        let globex_scope = steps::record_account_scope(client, &globex);
        let globex_caller = steps::account_caller(&globex_tenant, &globex, &globex_scope);
        let globex_formation = steps::create_formation(client, &globex_caller, &globex_scope);

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
            scope_id: ScopeId(id()),
            formation_id: FormationId(id()),
            priority: PriorityClassName("nope".to_owned()),
            gang_id: Some(GangId(id())),
            ..base.clone()
        };
        declines(&w, missing.clone(), "unknown-scope");
        let missing = SubmitSchedulingRequest {
            scope_id: ScopeId(Uuid(globex_scope.clone())),
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
        let closed = gang(&w, &w.path.scope_id, GangState::Bound);
        declines(
            &w,
            SubmitSchedulingRequest {
                formation_id: FormationId(Uuid(globex_formation)),
                gang_id: Some(closed.clone()),
                ..base.clone()
            },
            "formation-of-other-scope",
        );
        declines(
            &w,
            SubmitSchedulingRequest {
                scope_id: ScopeId(Uuid(globex_scope.clone())),
                gang_id: Some(closed.clone()),
                ..base.clone()
            },
            "formation-of-other-scope",
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
        let foreign_gang = gang(&w, &globex_scope, GangState::Forming);
        declines(
            &w,
            SubmitSchedulingRequest {
                gang_id: Some(foreign_gang),
                ..base.clone()
            },
            "gang-of-other-scope",
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
    fn submit_scheduling_request_writes_what_the_priority_class_formation_and_scope_name() {
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
        assert_eq!(data.scope_id, base.scope_id);
        assert_eq!(data.formation_id, base.formation_id);
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
        assert_eq!(scheduling_requested.scope_id, data.scope_id);
        assert_eq!(scheduling_requested.min_isolation, data.min_isolation);
        assert_eq!(scheduling_requested.priority, data.priority);
    }

    /// `replica-live` counts an instance in `Pending`, `Scheduled`, `Starting`, `Running` or
    /// `Paused` and no other: in a live state replica 0 is live; in any other state it is
    /// created again. The quota counts these states but `Pending`, and a `Pending` instance
    /// only while a decision of it holds a node, at `PlaceInstance` since the blocks took
    /// `CreateInstance` out of the outcome group `quota`
    /// (`place_instance::the_quota_counts_an_instance_from_scheduled_on_and_a_pending_one_only_while_placed`).
    #[test]
    fn replica_live_counts_the_five_live_states_and_no_other() {
        let w = World::new();
        let mut ports = w.ports();
        let set_all = |ports: &mut Ports, state: InstanceState| {
            for mut row in InstanceStorage::list(ports) {
                row.state = state;
                InstanceStorage::put(ports, row);
            }
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
        }
        for state in [
            InstanceState::Exited,
            InstanceState::Failed,
            InstanceState::Evicted,
        ] {
            set_all(&mut ports, state);
            creates(&w, replica(&w));
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
    use weft_types::behaviour::{InstanceStorage, ServiceStorage, WorkloadStorage};
    use weft_types::primitives::Uuid;
    use weft_types::shared::{ServiceId, WorkloadId};
    use weft_types::workloads::WorkloadSnapshot;

    use super::world::{binding_lines, instances, World};

    const CREATE_REPLICA: &str = "r1-create-replica";

    /// Sets the service `service_id` to `replicas` replicas.
    fn scale(ports: &mut Ports, service_id: &str, replicas: i64) {
        let key = ServiceId(Uuid(service_id.to_owned()));
        let mut service = ServiceStorage::get(ports, &key).expect("the service row");
        service.data.replicas = replicas;
        ServiceStorage::put(ports, service);
    }

    /// Takes the row of the workload `workload_id` away: a `ReplicaRequested` of its service
    /// names a workload no row holds, and `CreateInstance` answers `no-workload`, which
    /// `r1-create-replica` retries (`ess/components.yaml`). The row, to put back.
    fn take(ports: &mut Ports, workload_id: &str) -> WorkloadSnapshot {
        let key = WorkloadId(Uuid(workload_id.to_owned()));
        let row = WorkloadStorage::get(ports, &key).expect("the workload row");
        WorkloadStorage::delete(ports, &key);
        row
    }

    fn put_back_in(state: &FsPath, row: WorkloadSnapshot) {
        let mut ports = Ports::open(state).expect("the stopped host's state file opens");
        WorkloadStorage::put(&mut ports, row);
    }

    /// A service of 2 replicas whose `CreateInstance` is refused off the drop list (its
    /// workload's row is gone): tick 1 requests replica 0 and `CreateInstance` answers
    /// `no-workload` (held); tick 2 attempts it again and the level step, which sees no live
    /// instance of index 0, requests index 0 a second time. Once the row is back, the service
    /// must hold replicas 0 and 1 (`ess/4-applications/workloads.yaml` RequestReplica OPEN: "the
    /// controller requests one replica per service at a time and waits for InstanceCreated or
    /// the refusal").
    #[test]
    fn a_held_replica_is_not_requested_twice_and_every_index_is_made_once_it_is_admitted() {
        let w = World::new();
        let mut ports = w.ports();
        scale(&mut ports, &w.ids.service_id, 2);
        let workload = take(&mut ports, &w.ids.workload_id);
        let key = ServiceId(Uuid(w.ids.service_id.clone()));

        w.process.host().tick();
        w.process.host().tick();
        assert_eq!(InstanceStorage::list(&w.ports()).len(), 0);

        WorkloadStorage::put(&mut ports, workload);
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
            "replicas made and the service's live count after the workload's row was back"
        );
    }

    /// A replica whose `CreateInstance` was refused off the drop list (`no-workload`) is held
    /// for another attempt; the control host restarts; the workload's row is back.
    /// `r1-create-replica` is `delivery: at_least_once` and retries every refusal but
    /// `replica-live` and `workload-retired` (`ess/components.yaml`), and `RequestReplica`
    /// already counted the replica, so the level step never asks again: the instance must
    /// still be made. Replica 0 runs; replica 1 is the held one.
    #[test]
    fn a_held_replica_is_made_after_the_control_host_restarts() {
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
        let workload = {
            let mut ports = Ports::open(&host.state_path()).expect("the state file opens");
            scale(&mut ports, &ids.service_id, 2);
            take(&mut ports, &ids.workload_id)
        };
        let mut host = host.restart();
        let seq = host.last_seq();

        // Positive control: the tick requested replica 1 and its creation is held.
        let started = Instant::now();
        loop {
            let held = binding_lines(&host.log_after(seq), CREATE_REPLICA);
            if held.iter().any(|line| {
                line["refusal"] == json!("no-workload") && line["result"] == json!("retried")
            }) {
                break;
            }
            assert!(
                started.elapsed() < Duration::from_secs(5),
                "no held no-workload attempt of r1-create-replica: {held:?}"
            );
            std::thread::sleep(Duration::from_millis(20));
        }

        host.stop();
        put_back_in(&host.state_path(), workload);
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
                "3 s after the restart with the workload's row back the service has no replica 1: \
                 the held ReplicaRequested was lost with the process"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

/// What the scheduling modules share: the world after one tick (its instance and pending
/// scheduling request), its node made schedulable, and readers of the clock.
mod ready {
    use time::OffsetDateTime;
    use weft_impl::context::{instant, mint, timestamp, Clock};
    use weft_impl::Ports;
    use weft_types::behaviour::{
        NodeSessionStorage, NodeStorage, PlacementDecisionStorage, SchedulingRequestStorage,
    };
    use weft_types::infrastructure::NodeState;
    use weft_types::nodes::{NodeSessionData, NodeSessionSnapshot, NodeSessionState};
    use weft_types::primitives::{Timestamp, Uuid};
    use weft_types::scheduling::{
        PlacementDecisionData, PlacementDecisionSnapshot, PlacementDecisionState,
        SchedulingRequestSnapshot, SchedulingRequestState,
    };
    use weft_types::shared::{
        ClusterId, IsolationFacts, IsolationLevel, NodeId, NodeSessionId, PlacementDecisionId,
        ResourceAmount, ResourceClassName, StampedAt, TenantId,
    };

    use super::world::World;

    /// The world after one tick, with its node `Ready`, offering `Container`, with `cpu` 4 and,
    /// where asked, an open session.
    pub struct Ready {
        pub w: World,
        pub node: NodeId,
        pub session: Option<NodeSessionId>,
        pub request: SchedulingRequestSnapshot,
    }

    impl Ready {
        /// With the node's session open.
        pub fn new() -> Self {
            Self::with_session(true)
        }

        /// With the node's session open where `open`, else with none.
        pub fn with_session(open: bool) -> Self {
            let w = World::new();
            w.process.host().tick();
            let ports = w.ports();
            let requests = SchedulingRequestStorage::list(&ports);
            assert_eq!(requests.len(), 1, "{requests:?}");
            assert_eq!(requests[0].state, SchedulingRequestState::Pending);
            let node = NodeId(Uuid(w.path.node.node_id.clone()));
            make_ready(&ports, &node, &[IsolationLevel::Container]);
            let session = open.then(|| open_session(&ports, &node));
            Self {
                request: requests[0].clone(),
                w,
                node,
                session,
            }
        }

        pub fn ports(&self) -> Ports {
            self.w.ports()
        }

        /// The cluster the request's formation is pinned to: the path's default cluster.
        pub fn cluster(&self) -> ClusterId {
            ClusterId(Uuid(self.w.path.cluster_id.clone()))
        }

        /// The tenant of the account that holds the request's scope.
        pub fn tenant(&self) -> TenantId {
            TenantId(Uuid(self.w.path.tenant_id.clone()))
        }

        /// A `Bound`, unassigned placement of the request's instance on the node at
        /// `Container`, decided a second ago, the request `Placed` as `BindPlacement` leaves it.
        pub fn bound(&self) -> PlacementDecisionId {
            let mut ports = self.ports();
            let request = &self.request.data;
            let decision_id = PlacementDecisionId(Uuid(mint()));
            PlacementDecisionStorage::put(
                &mut ports,
                PlacementDecisionSnapshot {
                    state: PlacementDecisionState::Bound,
                    data: PlacementDecisionData {
                        decision_id: decision_id.clone(),
                        request_id: request.request_id.clone(),
                        instance_id: request.instance_id.clone(),
                        tenant_id: self.tenant(),
                        scope_id: request.scope_id.clone(),
                        node_id: self.node.clone(),
                        zone: weft_types::shared::ZoneName("z1".to_owned()),
                        formation_id: request.formation_id.clone(),
                        topology_group_id: None,
                        isolation: IsolationLevel::Container,
                        min_isolation: request.min_isolation,
                        assigned: false,
                        bindings: Vec::new(),
                        gang_id: None,
                        priority_value: request.priority_value,
                        preemptible: request.preemptible,
                        grace_seconds: request.grace_seconds,
                        decided_at: StampedAt(at(-1)),
                        preempted_by: None,
                        preemption_reason: None,
                        evicting_since: None,
                        released_at: None,
                    },
                },
            );
            let mut placed = SchedulingRequestStorage::get(&ports, &request.request_id)
                .expect("the request row");
            placed.state = SchedulingRequestState::Placed;
            SchedulingRequestStorage::put(&mut ports, placed);
            decision_id
        }

        /// Sets the state of the node.
        pub fn set_node_state(&self, state: NodeState) {
            let mut ports = self.ports();
            let mut row = NodeStorage::get(&ports, &self.node).expect("the node row");
            row.state = state;
            NodeStorage::put(&mut ports, row);
        }
    }

    /// Makes `node` `Ready`, with `cpu` 4 and the isolation levels `levels`.
    pub fn make_ready(ports: &Ports, node: &NodeId, levels: &[IsolationLevel]) {
        let mut ports = ports.clone();
        let mut row = NodeStorage::get(&ports, node).expect("the node row");
        row.state = NodeState::Ready;
        row.data.capacity = Some(vec![ResourceAmount {
            resource_class: ResourceClassName("cpu".to_owned()),
            amount: 4,
        }]);
        row.data.isolation_facts = Some(IsolationFacts {
            kvm_present: false,
            cgroups_delegated: true,
            egress_denied: false,
            gpu_passthrough: false,
            levels: levels.to_vec(),
        });
        NodeStorage::put(&mut ports, row);
    }

    /// A copy of `like` under the identity `id`, in its pool and zone, with an open session.
    pub fn another_node(ports: &Ports, like: &NodeId, id: &str) -> NodeId {
        let mut writer = ports.clone();
        let mut row = NodeStorage::get(ports, like).expect("the node row");
        let node = NodeId(Uuid(id.to_owned()));
        row.data.node_id = node.clone();
        NodeStorage::put(&mut writer, row);
        open_session(ports, &node);
        node
    }

    /// An open session of `node`.
    pub fn open_session(ports: &Ports, node: &NodeId) -> NodeSessionId {
        let mut ports = ports.clone();
        let session_id = NodeSessionId(Uuid(mint()));
        NodeSessionStorage::put(
            &mut ports,
            NodeSessionSnapshot {
                state: NodeSessionState::Open,
                data: NodeSessionData {
                    session_id: session_id.clone(),
                    node_id: node.clone(),
                    agent_version: "test".to_owned(),
                    opened_at: at(-60),
                    last_seen_at: at(-1),
                    ended_at: None,
                    end_reason: None,
                },
            },
        );
        session_id
    }

    /// The wall clock.
    pub fn now() -> OffsetDateTime {
        Clock::system().now()
    }

    /// The wall clock `seconds` from now.
    pub fn at(seconds: i64) -> Timestamp {
        timestamp(now() + time::Duration::seconds(seconds))
    }

    /// Whether `stamp` lies between `before` and `after`.
    pub fn between(stamp: &Timestamp, before: OffsetDateTime, after: OffsetDateTime) -> bool {
        instant(stamp).is_some_and(|instant| before <= instant && instant <= after)
    }
}

/// The acceptance of `story:skeleton-controllers` on the Scheduler's decision step, statements 1
/// to 3, and the step's candidates, its one placement per request and its waits.
mod placement {
    use weft_controllers::placement::{PLACE_INSTANCE, STEP};
    use weft_impl::bindings::r4_bind_placement;
    use weft_impl::context::mint;
    use weft_system::SystemEvent;
    use weft_types::behaviour::{QuotaStorage, SchedulingRequestStorage};
    use weft_types::infrastructure::NodeState;
    use weft_types::nodes::AssignmentState;
    use weft_types::primitives::Uuid;
    use weft_types::scheduling::{
        InstancePlaced, PlacementDecisionState, PlacementDecisions, SchedulingRequestState,
    };
    use weft_types::shared::{GangId, IsolationLevel, PlacementDecisionId, QuotaId, ZoneName};
    use weft_types::workloads::InstanceState;

    use super::ready::{another_node, between, make_ready, now, Ready};
    use super::world::request_lines;

    /// A node identity that sorts before any minted one in the view's order.
    const FIRST: &str = "00000000-0000-4000-8000-000000000000";

    fn decisions(r: &Ready) -> Vec<PlacementDecisions> {
        r.w.process
            .host()
            .system()
            .weft_control
            .placement_decisions()
            .expect("the view reads")
    }

    fn set_limit(r: &Ready, limit: i64) {
        let mut ports = r.ports();
        let key = QuotaId(Uuid(r.w.path.quota_id.clone()));
        let mut quota = QuotaStorage::get(&ports, &key).expect("the quota row");
        quota.data.limit = limit;
        quota.data.remaining = limit;
        QuotaStorage::put(&mut ports, quota);
    }

    /// The `PlaceInstance` request lines of one tick.
    fn tick(r: &Ready) -> usize {
        let seq = r.w.process.last_seq();
        r.w.process.host().tick();
        request_lines(&r.w.process.log_after(seq), PLACE_INSTANCE).len()
    }

    /// Statements 1 and 2: one tick places the pending request on the Ready node with an open
    /// session, `decided_at` between the wall-clock reads around the tick; the hand-run `r4-*`
    /// bindings leave an `Active` assignment of the instance on that node, whose `assigned_at`
    /// is the stamped `decided_at`, and the instance `Scheduled` on the node.
    #[test]
    fn one_tick_places_a_pending_request_and_the_r4_bindings_assign_and_schedule_it() {
        let r = Ready::new();
        let before = now();
        r.w.process.host().tick();
        let after = now();

        let rows: Vec<PlacementDecisions> = decisions(&r)
            .into_iter()
            .filter(|row| row.request_id == r.request.data.request_id)
            .collect();
        assert_eq!(rows.len(), 1, "{rows:?}");
        let decision = &rows[0];
        assert_eq!(decision.node_id, r.node);
        assert!(
            between(&decision.decided_at.0, before, after),
            "decided_at {:?} not between {before} and {after}",
            decision.decided_at
        );
        assert_eq!(
            (decision.state, decision.assigned),
            (PlacementDecisionState::Bound, true)
        );

        let system = r.w.process.host().system();
        let assignments: Vec<_> = system
            .weft_control
            .assignments()
            .expect("the view reads")
            .into_iter()
            .filter(|row| row.instance_id == r.request.data.instance_id)
            .collect();
        assert_eq!(assignments.len(), 1, "{assignments:?}");
        let assignment = &assignments[0];
        assert_eq!(assignment.state, AssignmentState::Active);
        assert_eq!(assignment.node_id, r.node);
        assert_eq!(assignment.placement_decision_id, decision.decision_id);
        assert_eq!(assignment.assigned_at, decision.decided_at.0);
        assert!(between(&assignment.assigned_at, before, after));
        let instance = system
            .weft_control
            .instances()
            .expect("the view reads")
            .into_iter()
            .find(|row| row.instance_id == r.request.data.instance_id)
            .expect("the instance's row");
        assert_eq!(instance.state, InstanceState::Scheduled);
        assert_eq!(instance.node_id.as_ref(), Some(&r.node));
    }

    /// Statement 3: the only Ready node has no open session: one tick adds no decision and the
    /// request stays pending.
    #[test]
    fn a_tick_adds_no_decision_where_the_only_ready_node_has_no_open_session() {
        let r = Ready::with_session(false);
        r.w.process.host().tick();
        assert!(decisions(&r).is_empty(), "{:?}", decisions(&r));
        let request = SchedulingRequestStorage::get(&r.ports(), &r.request.data.request_id)
            .expect("the request row");
        assert_eq!(request.state, SchedulingRequestState::Pending);
        let pending =
            r.w.process
                .host()
                .system()
                .weft_control
                .pending_requests()
                .expect("the view reads");
        assert_eq!(pending.len(), 1, "{pending:?}");
    }

    /// The weakest level the request admits is tried first, and no level below its minimum: a
    /// node offering `MicroVm`, `ProcessSandbox` and `Container` places a `Container` request
    /// at `Container`, in one `PlaceInstance`.
    #[test]
    fn the_weakest_admitted_level_is_placed_first() {
        let r = Ready::new();
        make_ready(
            &r.ports(),
            &r.node,
            &[
                IsolationLevel::MicroVm,
                IsolationLevel::ProcessSandbox,
                IsolationLevel::Container,
            ],
        );
        assert_eq!(tick(&r), 1);
        let rows = decisions(&r);
        assert_eq!(rows.len(), 1, "{rows:?}");
        assert_eq!(rows[0].isolation, IsolationLevel::Container);
    }

    /// Only the levels a node offers are tried: a node offering only `MicroVm` places a
    /// `Container` request at `MicroVm`, in one `PlaceInstance`.
    #[test]
    fn only_the_levels_a_node_offers_are_tried() {
        let r = Ready::new();
        make_ready(&r.ports(), &r.node, &[IsolationLevel::MicroVm]);
        assert_eq!(tick(&r), 1);
        let rows = decisions(&r);
        assert_eq!(rows.len(), 1, "{rows:?}");
        assert_eq!(rows[0].isolation, IsolationLevel::MicroVm);
    }

    /// A node the request cannot take gives way to the next candidate: the first node in the
    /// view's order lies in a zone outside the cluster and is refused, the world's node places.
    #[test]
    fn a_refused_candidate_gives_way_to_the_next() {
        let r = Ready::new();
        let ports = r.ports();
        let first = another_node(&ports, &r.node, FIRST);
        let mut writer = ports.clone();
        let mut row = weft_types::behaviour::NodeStorage::get(&ports, &first).expect("the row");
        row.data.zone = ZoneName("z9".to_owned());
        weft_types::behaviour::NodeStorage::put(&mut writer, row);

        assert_eq!(tick(&r), 2);
        let rows = decisions(&r);
        assert_eq!(rows.len(), 1, "{rows:?}");
        assert_eq!(rows[0].node_id, r.node);
    }

    /// The step stops at the first placement: of two nodes that could take it, the first in the
    /// view's order takes it and the second is not asked.
    #[test]
    fn the_step_stops_at_the_first_placement() {
        let r = Ready::new();
        let first = another_node(&r.ports(), &r.node, FIRST);
        assert_eq!(tick(&r), 1);
        let rows = decisions(&r);
        assert_eq!(rows.len(), 1, "{rows:?}");
        assert_eq!(rows[0].node_id, first);
    }

    /// Two replicas that each fill a node of `cpu` 1: the first node in the view's order takes
    /// one, and the other, refused there (`no-capacity`), is placed on the next node.
    #[test]
    fn a_replica_the_first_node_has_no_room_for_is_placed_on_the_next() {
        let r = Ready::new();
        let mut ports = r.ports();
        let mut row = weft_types::behaviour::NodeStorage::get(&ports, &r.node).expect("the row");
        row.data.capacity = Some(vec![weft_types::shared::ResourceAmount {
            resource_class: weft_types::shared::ResourceClassName("cpu".to_owned()),
            amount: 1,
        }]);
        weft_types::behaviour::NodeStorage::put(&mut ports, row);
        let first = another_node(&ports, &r.node, FIRST);
        let key = weft_types::shared::ServiceId(Uuid(r.w.ids.service_id.clone()));
        let mut service =
            weft_types::behaviour::ServiceStorage::get(&ports, &key).expect("the service row");
        service.data.replicas = 2;
        weft_types::behaviour::ServiceStorage::put(&mut ports, service);

        for _ in 0..3 {
            r.w.process.host().tick();
        }
        let mut nodes: Vec<_> = decisions(&r)
            .into_iter()
            .filter(|row| row.state == PlacementDecisionState::Bound)
            .map(|row| row.node_id)
            .collect();
        nodes.sort_by(|a, b| a.0 .0.cmp(&b.0 .0));
        assert_eq!(nodes, [first, r.node.clone()]);
    }

    /// A refusal of the request itself (`quota-exceeded`) ends its attempt on the first node;
    /// the attempts back off from one tick doubling up to 32, one `PlaceInstance` each: over
    /// 100 ticks, 8 lines, at most log2(100) + 2, and the wait lives in the state file. Once the
    /// quota admits it, the next attempt places it and the wait is let go.
    #[test]
    fn a_request_the_quota_refuses_is_attempted_after_doubling_waits_with_one_line_each() {
        const TICKS: u32 = 100;
        let r = Ready::new();
        another_node(&r.ports(), &r.node, FIRST);
        set_limit(&r, 0);
        let mut attempted_at = Vec::new();
        for at in 1..=TICKS {
            let lines = tick(&r);
            assert!(lines <= 1, "tick {at}: {lines} PlaceInstance lines");
            if lines == 1 {
                attempted_at.push(at);
            }
        }
        assert_eq!(attempted_at, [1, 2, 4, 8, 16, 32, 64, 96]);
        assert!(
            f64::from(u32::try_from(attempted_at.len()).expect("a count"))
                <= f64::from(TICKS).log2() + 2.0
        );
        let waits = r.ports().store().step_waits(STEP);
        assert_eq!(waits.len(), 1, "{waits:?}");
        assert_eq!(
            (waits[0].subject.as_str(), waits[0].attempts),
            (r.request.data.request_id.0 .0.as_str(), 8)
        );

        set_limit(&r, 64);
        let mut placed_at = None;
        for at in TICKS + 1..=TICKS + 28 {
            if tick(&r) > 0 {
                placed_at = Some(at);
            }
        }
        assert_eq!(placed_at, Some(128));
        assert_eq!(decisions(&r).len(), 1);
        assert!(r.ports().store().step_waits(STEP).is_empty());
    }

    /// A request whose `InstancePlaced` the bindings hold for another attempt is not placed
    /// again.
    #[test]
    fn a_request_with_a_held_instance_placed_is_skipped() {
        let r = Ready::new();
        let event = SystemEvent::SchedulingInstancePlaced(InstancePlaced {
            decision_id: PlacementDecisionId(Uuid(mint())),
            request_id: r.request.data.request_id.clone(),
            node_id: r.node.clone(),
            isolation: IsolationLevel::Container,
            bindings: Vec::new(),
        });
        let encoded = (r4_bind_placement::ENTRY.encode)(&event).expect("it encodes");
        r.ports()
            .store()
            .hold(r4_bind_placement::ENTRY.id, &encoded.to_string(), 1, 1000);
        assert_eq!(tick(&r), 0);
        assert!(decisions(&r).is_empty());
    }

    /// A gang member is left to `BindGang`: the step does not place it.
    #[test]
    fn a_gang_member_is_skipped() {
        let r = Ready::new();
        let mut ports = r.ports();
        let mut request = r.request.clone();
        request.data.gang_id = Some(GangId(Uuid(mint())));
        SchedulingRequestStorage::put(&mut ports, request);
        assert_eq!(tick(&r), 0);
    }

    /// A request with no candidate sends nothing and does not wait; a request that waits and
    /// leaves the view has its wait let go.
    #[test]
    fn waits_are_kept_only_for_attempts_that_failed_of_requests_still_pending() {
        let r = Ready::new();
        r.set_node_state(NodeState::Cordoned);
        assert_eq!(tick(&r), 0);
        assert!(r.ports().store().step_waits(STEP).is_empty());

        r.set_node_state(NodeState::Ready);
        set_limit(&r, 0);
        assert_eq!(tick(&r), 1);
        assert_eq!(r.ports().store().step_waits(STEP).len(), 1);
        let mut ports = r.ports();
        let mut request = r.request.clone();
        request.state = SchedulingRequestState::Withdrawn;
        SchedulingRequestStorage::put(&mut ports, request);
        assert_eq!(tick(&r), 0);
        assert!(r.ports().store().step_waits(STEP).is_empty());
    }
}

/// The acceptance of `story:skeleton-controllers` on the R4 level step, statement 4, and the
/// step's follow-up, its one command per placement and its waits.
mod repair {
    use weft_controllers::repair::{ASSIGN, RECORD_PLACEMENT_ASSIGNED, STEP};
    use weft_impl::bindings::{r4_assign, r4_record_placement_assigned, Entry};
    use weft_impl::context::mint;
    use weft_system::SystemEvent;
    use weft_types::behaviour::{AssignmentStorage, PlacementDecisionStorage};
    use weft_types::infrastructure::NodeState;
    use weft_types::nodes::{
        AssignmentData, AssignmentSnapshot, AssignmentState, Assignments, DesiredState,
        InstanceAssigned,
    };
    use weft_types::primitives::Uuid;
    use weft_types::scheduling::PlacementBound;
    use weft_types::shared::{AssignmentId, IsolationLevel, PlacementDecisionId, StampedAt};

    use super::ready::{at, between, now, Ready};
    use super::world::request_lines;

    fn assignments(r: &Ready) -> Vec<Assignments> {
        r.w.process
            .host()
            .system()
            .weft_control
            .assignments()
            .expect("the view reads")
    }

    /// The `Assign` and `RecordPlacementAssigned` request lines of one tick.
    fn tick(r: &Ready) -> (usize, usize) {
        let seq = r.w.process.last_seq();
        r.w.process.host().tick();
        let lines = r.w.process.log_after(seq);
        (
            request_lines(&lines, ASSIGN).len(),
            request_lines(&lines, RECORD_PLACEMENT_ASSIGNED).len(),
        )
    }

    fn assigned(r: &Ready, decision: &PlacementDecisionId) -> bool {
        PlacementDecisionStorage::get(&r.ports(), decision)
            .expect("the decision row")
            .data
            .assigned
    }

    /// Statement 4: one tick of the repair step leaves an `Active` assignment of a bound,
    /// unassigned placement, `assigned_at` between the wall-clock reads around the tick; the
    /// placement is then recorded assigned.
    #[test]
    fn one_tick_assigns_a_bound_unassigned_placement() {
        let r = Ready::new();
        let decision = r.bound();
        let before = now();
        assert_eq!(tick(&r), (1, 0));
        let after = now();
        let rows: Vec<Assignments> = assignments(&r)
            .into_iter()
            .filter(|row| row.placement_decision_id == decision)
            .collect();
        assert_eq!(rows.len(), 1, "{rows:?}");
        assert_eq!(rows[0].state, AssignmentState::Active);
        assert!(
            between(&rows[0].assigned_at, before, after),
            "assigned_at {:?} not between {before} and {after}",
            rows[0].assigned_at
        );
        assert!(assigned(&r, &decision));
        assert!(r.ports().store().step_waits(STEP).is_empty());
    }

    /// An `already-assigned` answer is followed by `RecordPlacementAssigned` for the row.
    #[test]
    fn an_already_assigned_placement_is_recorded_assigned() {
        let r = Ready::new();
        let decision = r.bound();
        let mut ports = r.ports();
        AssignmentStorage::put(
            &mut ports,
            AssignmentSnapshot {
                state: AssignmentState::Active,
                data: AssignmentData {
                    assignment_id: AssignmentId(Uuid(mint())),
                    instance_id: r.request.data.instance_id.clone(),
                    node_id: r.node.clone(),
                    session_id: r.session.clone().expect("an open session"),
                    placement_decision_id: decision.clone(),
                    min_isolation: IsolationLevel::Container,
                    desired: DesiredState::Run,
                    snapshot_id: None,
                    assigned_at: at(-1),
                    lease_renewed_at: at(-1),
                    ended_at: None,
                },
            },
        );
        assert_eq!(tick(&r), (1, 1));
        assert!(assigned(&r, &decision));
        assert_eq!(assignments(&r).len(), 1);
        assert!(r.ports().store().step_waits(STEP).is_empty());
        assert_eq!(tick(&r), (0, 0));
    }

    /// A placement whose node is not ready is attempted after doubling waits, one `Assign`
    /// each, and assigned by the first attempt after the node is ready again.
    #[test]
    fn a_refused_assignment_is_attempted_after_doubling_waits() {
        let r = Ready::new();
        let decision = r.bound();
        r.set_node_state(NodeState::Cordoned);
        let mut attempted_at = Vec::new();
        for at in 1..=20 {
            let (assigns, records) = tick(&r);
            assert_eq!(records, 0);
            assert!(assigns <= 1, "tick {at}: {assigns} Assign lines");
            if assigns == 1 {
                attempted_at.push(at);
            }
        }
        assert_eq!(attempted_at, [1, 2, 4, 8, 16]);
        let waits = r.ports().store().step_waits(STEP);
        assert_eq!(waits.len(), 1, "{waits:?}");
        assert_eq!(
            (waits[0].subject.as_str(), waits[0].attempts, waits[0].wait),
            (decision.0 .0.as_str(), 5, 12)
        );

        r.set_node_state(NodeState::Ready);
        let mut assigned_at = None;
        for at in 21..=32 {
            if tick(&r).0 > 0 {
                assigned_at = Some(at);
            }
        }
        assert_eq!(assigned_at, Some(32));
        assert!(assigned(&r, &decision));
        assert!(r.ports().store().step_waits(STEP).is_empty());
    }

    /// Holds `event` for `entry` with a wait no tick of the test ends: its id.
    fn hold(r: &Ready, entry: &Entry, event: &SystemEvent) -> i64 {
        let store = r.ports();
        let encoded = (entry.encode)(event).expect("it encodes");
        store.store().hold(entry.id, &encoded.to_string(), 1, 1000);
        store.store().held().last().expect("the held event").id
    }

    /// A placement whose `PlacementBound` (`r4-assign`) or `InstanceAssigned`
    /// (`r4-record-placement-assigned`) the bindings hold is not sent again.
    #[test]
    fn a_placement_with_a_held_delivery_is_skipped() {
        let r = Ready::new();
        let decision = r.bound();
        let bound = SystemEvent::SchedulingPlacementBound(PlacementBound {
            decision_id: decision.clone(),
            instance_id: r.request.data.instance_id.clone(),
            node_id: r.node.clone(),
            isolation: IsolationLevel::Container,
            min_isolation: IsolationLevel::Container,
            decided_at: StampedAt(at(-1)),
        });
        let held = hold(&r, &r4_assign::ENTRY, &bound);
        assert_eq!(tick(&r), (0, 0));
        r.ports().store().release(held);

        let assigned_event = SystemEvent::NodesInstanceAssigned(InstanceAssigned {
            assignment_id: AssignmentId(Uuid(mint())),
            instance_id: r.request.data.instance_id.clone(),
            node_id: r.node.clone(),
            session_id: r.session.clone().expect("an open session"),
            min_isolation: IsolationLevel::Container,
            placement_decision_id: decision.clone(),
        });
        let held = hold(&r, &r4_record_placement_assigned::ENTRY, &assigned_event);
        assert_eq!(tick(&r), (0, 0));
        r.ports().store().release(held);

        assert_eq!(tick(&r), (1, 0));
        assert!(assigned(&r, &decision));
    }
}

/// Statement 5 of `story:skeleton-controllers`: each `r4-*` binding drops one refusal of its
/// drop list and retries one refusal off it, in the log after the `seq` read before each event.
mod r4_bindings {
    use serde_json::{json, Value};
    use weft_impl::context::mint;
    use weft_system::SystemEvent;
    use weft_types::behaviour::{AssignmentStorage, PlacementDecisionStorage};
    use weft_types::infrastructure::NodeState;
    use weft_types::nodes::{AssignmentState, InstanceAssigned};
    use weft_types::primitives::Uuid;
    use weft_types::scheduling::{InstancePlaced, PlacementBound, PlacementDecisionState};
    use weft_types::shared::{InstanceId, IsolationLevel, PlacementDecisionId};

    use super::ready::Ready;
    use super::world::binding_lines;

    /// The one line of `binding` after `event` is replayed: its refusal and result.
    fn answer(r: &Ready, event: SystemEvent, binding: &str) -> (Value, Value) {
        let seq = r.w.process.last_seq();
        r.w.process.host().replay(vec![event]);
        let lines = binding_lines(&r.w.process.log_after(seq), binding);
        assert_eq!(lines.len(), 1, "{binding}: {lines:?}");
        (lines[0]["refusal"].clone(), lines[0]["result"].clone())
    }

    /// `event`, replayed alone, gives `binding` the refusal and result named.
    fn expect(r: &Ready, binding: &str, event: SystemEvent, refusal: &str, result: &str) {
        assert_eq!(
            answer(r, event, binding),
            (json!(refusal), json!(result)),
            "{binding}"
        );
    }

    #[test]
    fn each_r4_binding_drops_one_refusal_and_retries_another() {
        let r = Ready::new();
        r.w.process.host().tick();
        let ports = r.ports();
        let decisions = PlacementDecisionStorage::list(&ports);
        assert_eq!(decisions.len(), 1, "{decisions:?}");
        let decision = decisions[0].clone();
        assert_eq!(decision.state, PlacementDecisionState::Bound);
        let assignments = AssignmentStorage::list(&ports);
        assert_eq!(assignments.len(), 1, "{assignments:?}");
        let assignment = assignments[0].clone();
        let data = &decision.data;

        let placed = |decision_id: PlacementDecisionId| {
            SystemEvent::SchedulingInstancePlaced(InstancePlaced {
                decision_id,
                request_id: data.request_id.clone(),
                node_id: data.node_id.clone(),
                isolation: data.isolation,
                bindings: Vec::new(),
            })
        };
        let bound = |instance_id: InstanceId| {
            SystemEvent::SchedulingPlacementBound(PlacementBound {
                decision_id: data.decision_id.clone(),
                instance_id,
                node_id: data.node_id.clone(),
                isolation: data.isolation,
                min_isolation: data.min_isolation,
                decided_at: data.decided_at.clone(),
            })
        };
        let assigned = |placement_decision_id: PlacementDecisionId| {
            SystemEvent::NodesInstanceAssigned(InstanceAssigned {
                assignment_id: assignment.data.assignment_id.clone(),
                instance_id: data.instance_id.clone(),
                node_id: data.node_id.clone(),
                session_id: assignment.data.session_id.clone(),
                min_isolation: IsolationLevel::Container,
                placement_decision_id,
            })
        };
        let fresh = || PlacementDecisionId(Uuid(mint()));

        // r4-bind-placement: an InstancePlaced of a bound decision; one naming no decision.
        let bind = "r4-bind-placement";
        expect(
            &r,
            bind,
            placed(data.decision_id.clone()),
            "wrong-state",
            "dropped",
        );
        expect(&r, bind, placed(fresh()), "no-such-placement", "retried");

        // r4-assign: a PlacementBound of an assigned placement; r4-schedule-instance answers the
        // same event `wrong-state`, its instance already Scheduled.
        let event = bound(data.instance_id.clone());
        let seq = r.w.process.last_seq();
        r.w.process.host().replay(vec![event]);
        let lines = r.w.process.log_after(seq);
        for (binding, refusal) in [
            ("r4-assign", "already-assigned"),
            ("r4-schedule-instance", "wrong-state"),
        ] {
            let lines = binding_lines(&lines, binding);
            assert_eq!(lines.len(), 1, "{binding}: {lines:?}");
            assert_eq!(
                (lines[0]["refusal"].clone(), lines[0]["result"].clone()),
                (json!(refusal), json!("dropped")),
                "{binding}"
            );
        }

        // r4-schedule-instance: a PlacementBound naming no instance.
        expect(
            &r,
            "r4-schedule-instance",
            bound(InstanceId(Uuid(mint()))),
            "no-such-instance",
            "retried",
        );

        // r4-assign: the assignment ended and the node cordoned.
        let mut writer = ports.clone();
        let mut ended = assignment.clone();
        ended.state = AssignmentState::Ended;
        AssignmentStorage::put(&mut writer, ended);
        r.set_node_state(NodeState::Cordoned);
        expect(
            &r,
            "r4-assign",
            bound(data.instance_id.clone()),
            "node-not-ready",
            "retried",
        );

        // r4-record-placement-assigned: a released placement; one naming no placement.
        let mut released = decision.clone();
        released.state = PlacementDecisionState::Released;
        PlacementDecisionStorage::put(&mut writer, released);
        let record = "r4-record-placement-assigned";
        expect(
            &r,
            record,
            assigned(data.decision_id.clone()),
            "not-bound",
            "dropped",
        );
        expect(
            &r,
            record,
            assigned(fresh()),
            "no-such-placement",
            "retried",
        );
    }
}

/// `weft.scheduling.PlaceInstance`: each guard in the selection precedence of its contract, and
/// what `placed` writes.
mod place_instance {
    use weft_impl::bindings::outcome_name;
    use weft_impl::context::mint;
    use weft_impl::Ports;
    use weft_types::behaviour::{
        BindingStorage, ClaimStorage, ClusterNodePoolStorage, ClusterStorage, DeviceStorage,
        InstanceStorage, NodeSessionStorage, NodeStorage, PlacementDecisionStorage, QuotaStorage,
        SchedulingRequestStorage, TopologyGroupStorage, TopologyLinkStorage, WorkloadStorage,
    };
    use weft_types::claims::{
        BindingData, BindingSnapshot, BindingState, ClaimData, ClaimHolder, ClaimSnapshot,
        ClaimState, InstanceHolder,
    };
    use weft_types::cluster::{ClusterNodePoolData, ClusterNodePoolSnapshot, ClusterNodePoolState};
    use weft_types::infrastructure::NodeState;
    use weft_types::inventory::{DeviceData, DeviceSnapshot, DeviceState};
    use weft_types::nodes::NodeSessionState;
    use weft_types::primitives::{Timestamp, Uuid};
    use weft_types::scheduling::obligations::PlaceInstanceBehavior;
    use weft_types::scheduling::{
        InstancePlaced, PlaceInstance, PlaceInstanceOutcome, PlacementDecisionState,
        SchedulingRequestState,
    };
    use weft_types::shared::{
        AccountId, BindingId, ClaimId, CloudId, ClusterId, ClusterNodePoolId, DeviceId,
        FormationId, InstanceId, IsolationLevel, NodeId, NodePoolId, QuotaId, ResourceAmount,
        ResourceClassName, SchedulingRequestId, ScopeId, Sharing, StampedAt, TenantId,
        TopologyGroupId, TopologyId, TopologyLinkId, ZoneName,
    };
    use weft_types::topology::{
        GroupRole, TopologyGroupData, TopologyGroupSnapshot, TopologyGroupState, TopologyLinkData,
        TopologyLinkSnapshot, TopologyLinkState,
    };
    use weft_types::workloads::InstanceState;

    use super::ready::{at, Ready};

    fn id() -> Uuid {
        Uuid(mint())
    }

    /// The input that places the request on the world's node at `Container`.
    fn base(r: &Ready) -> PlaceInstance {
        let request = &r.request.data;
        let node = NodeStorage::get(&r.ports(), &r.node).expect("the node row");
        PlaceInstance {
            request_id: request.request_id.clone(),
            tenant_id: r.tenant(),
            scope_id: request.scope_id.clone(),
            cluster_id: r.cluster(),
            formation_id: request.formation_id.clone(),
            node_id: r.node.clone(),
            node_pool_id: node.data.node_pool_id,
            zone: node.data.zone,
            isolation: IsolationLevel::Container,
            bindings: Vec::new(),
            cluster_node_pool_id: None,
            topology_group_id: None,
            peer_groups: Vec::new(),
            decided_at: StampedAt(at(-1)),
        }
    }

    /// `input` answers `outcome` and writes no decision.
    fn refuses(r: &Ready, input: &PlaceInstance, outcome: &str) {
        let mut ports = r.ports();
        let before = PlacementDecisionStorage::list(&ports).len();
        let answer =
            PlaceInstanceBehavior::place_instance(&mut ports, input.clone()).expect("an outcome");
        assert_eq!(outcome_name(&answer), outcome, "{input:?}: {answer:?}");
        assert_eq!(PlacementDecisionStorage::list(&ports).len(), before);
    }

    fn claim(ports: &mut Ports, scope: &ScopeId, class: &str, amount: i64) -> ClaimId {
        let claim_id = ClaimId(id());
        ClaimStorage::put(
            ports,
            ClaimSnapshot {
                state: ClaimState::Bound,
                data: ClaimData {
                    claim_id: claim_id.clone(),
                    scope_id: scope.clone(),
                    holder: ClaimHolder::Instance(InstanceHolder {
                        instance_id: InstanceId(id()),
                    }),
                    class_name: ResourceClassName(class.to_owned()),
                    amount,
                    sharing: Sharing::Exclusive,
                    device_model: None,
                    volume_id: None,
                    filesystem_id: None,
                    bucket_id: None,
                    domain_name: None,
                    egress_allow: Vec::new(),
                    binding_id: None,
                },
            },
        );
        claim_id
    }

    fn binding(
        ports: &mut Ports,
        claim_id: &ClaimId,
        node: Option<&NodeId>,
        state: BindingState,
        devices: Vec<DeviceId>,
    ) -> BindingId {
        let binding_id = BindingId(id());
        BindingStorage::put(
            ports,
            BindingSnapshot {
                state,
                data: BindingData {
                    binding_id: binding_id.clone(),
                    claim_id: claim_id.clone(),
                    node_id: node.cloned(),
                    zone: None,
                    device_ids: devices,
                    volume_id: None,
                    address: None,
                },
            },
        );
        binding_id
    }

    fn pool(
        ports: &mut Ports,
        cluster: &ClusterId,
        scope: &ScopeId,
        node_pool: Option<NodePoolId>,
    ) -> ClusterNodePoolId {
        let pool_id = ClusterNodePoolId(id());
        ClusterNodePoolStorage::put(
            ports,
            ClusterNodePoolSnapshot {
                state: ClusterNodePoolState::Ready,
                data: ClusterNodePoolData {
                    cluster_node_pool_id: pool_id.clone(),
                    cluster_id: cluster.clone(),
                    scope_id: scope.clone(),
                    name: "pool".to_owned(),
                    zone: ZoneName("z1".to_owned()),
                    machine_type: "m1".to_owned(),
                    image: "weft-node".to_owned(),
                    min_size: 0,
                    max_size: 1,
                    node_pool_id: node_pool,
                },
            },
        );
        pool_id
    }

    fn set_request(
        r: &Ready,
        change: impl FnOnce(&mut weft_types::scheduling::SchedulingRequestSnapshot),
    ) {
        let mut ports = r.ports();
        let mut row = SchedulingRequestStorage::get(&ports, &r.request.data.request_id)
            .expect("the request row");
        change(&mut row);
        SchedulingRequestStorage::put(&mut ports, row);
    }

    fn set_cluster(r: &Ready, change: impl FnOnce(&mut weft_types::cluster::ClusterSnapshot)) {
        let mut ports = r.ports();
        let mut row = ClusterStorage::get(&ports, &r.cluster()).expect("the cluster row");
        change(&mut row);
        ClusterStorage::put(&mut ports, row);
    }

    fn set_node(r: &Ready, change: impl FnOnce(&mut weft_types::infrastructure::NodeSnapshot)) {
        let mut ports = r.ports();
        let mut row = NodeStorage::get(&ports, &r.node).expect("the node row");
        change(&mut row);
        NodeStorage::put(&mut ports, row);
    }

    fn set_limit(r: &Ready, limit: i64) {
        let mut ports = r.ports();
        let key = QuotaId(Uuid(r.w.path.quota_id.clone()));
        let mut quota = QuotaStorage::get(&ports, &key).expect("the quota row");
        quota.data.limit = limit;
        quota.data.remaining = limit;
        QuotaStorage::put(&mut ports, quota);
    }

    /// A decision of `request` on the world's node for `tenant` at `Container`, `Reserved`.
    fn reserved(r: &Ready, request: SchedulingRequestId, tenant: TenantId) {
        let mut ports = r.ports();
        let decision = r.bound();
        let mut row = PlacementDecisionStorage::get(&ports, &decision).expect("the decision");
        row.state = PlacementDecisionState::Reserved;
        row.data.request_id = request;
        row.data.tenant_id = tenant;
        row.data.instance_id = InstanceId(id());
        PlacementDecisionStorage::put(&mut ports, row);
        // `bound` leaves the request Placed; the guards below set its state themselves.
        let mut request = SchedulingRequestStorage::get(&ports, &r.request.data.request_id)
            .expect("the request row");
        request.state = SchedulingRequestState::Pending;
        SchedulingRequestStorage::put(&mut ports, request);
    }

    /// From the last guard to the first, each case added to the ones before it: the answer is
    /// the case just added, which precedes every guard whose case still holds.
    #[test]
    fn place_instance_answers_each_guard_in_its_precedence() {
        let r = Ready::new();
        let mut ports = r.ports();
        let scope = r.request.data.scope_id.clone();
        let cluster = r.cluster();
        let mut input = base(&r);
        let node_pool = input.node_pool_id.clone();

        // External branches, in declaration order (checked last to first).
        let gpu = claim(&mut ports, &scope, "gpu", 1);
        let device = DeviceId(id());
        DeviceStorage::put(
            &mut ports,
            DeviceSnapshot {
                state: DeviceState::Wiping,
                data: DeviceData {
                    device_id: device.clone(),
                    node_id: r.node.clone(),
                    class_name: ResourceClassName("gpu".to_owned()),
                    hardware_id: "h".to_owned(),
                    vendor: "v".to_owned(),
                    model: "m".to_owned(),
                    memory_bytes: None,
                    driver_version: None,
                    parent_device_id: None,
                    interconnect_id: None,
                },
            },
        );
        let wiping = binding(
            &mut ports,
            &gpu,
            Some(&r.node),
            BindingState::Reserved,
            vec![device],
        );
        input.bindings = vec![wiping];
        refuses(&r, &input, "device-not-wiped");
        set_limit(&r, 0);
        refuses(&r, &input, "quota-exceeded");
        let cpu = claim(&mut ports, &scope, "cpu", 4);
        binding(
            &mut ports,
            &cpu,
            Some(&r.node),
            BindingState::Reserved,
            Vec::new(),
        );
        refuses(&r, &input, "no-capacity");

        // The present-related refusals, last to first.
        input.isolation = IsolationLevel::UserKernel;
        refuses(&r, &input, "isolation-not-offered");
        let own_pool = pool(&mut ports, &cluster, &scope, node_pool.clone());
        let group = TopologyGroupId(id());
        let peer = TopologyGroupId(id());
        let topology = TopologyId(id());
        TopologyGroupStorage::put(
            &mut ports,
            TopologyGroupSnapshot {
                state: TopologyGroupState::Active,
                data: TopologyGroupData {
                    topology_group_id: group.clone(),
                    topology_id: topology.clone(),
                    name: "hub".to_owned(),
                    node_pools: vec![node_pool.clone().expect("the node's pool")],
                    role: GroupRole::Hub,
                    position: None,
                    parent_group_id: None,
                },
            },
        );
        TopologyLinkStorage::put(
            &mut ports,
            TopologyLinkSnapshot {
                state: TopologyLinkState::Declared,
                data: TopologyLinkData {
                    topology_link_id: TopologyLinkId(id()),
                    topology_id: topology,
                    from_group_id: group.clone(),
                    to_group_id: peer.clone(),
                    bandwidth_mbps: 1,
                    latency_us: 1,
                    crossable: false,
                },
            },
        );
        input.cluster_node_pool_id = Some(own_pool.clone());
        input.topology_group_id = Some(group.clone());
        input.peer_groups = vec![peer];
        refuses(&r, &input, "crosses-closed-link");
        let mut row = TopologyGroupStorage::get(&ports, &group).expect("the group");
        row.state = TopologyGroupState::Removed;
        TopologyGroupStorage::put(&mut ports, row);
        refuses(&r, &input, "group-not-of-node");
        let other_cluster = ClusterId(id());
        input.cluster_node_pool_id = Some(pool(&mut ports, &other_cluster, &scope, None));
        refuses(&r, &input, "cluster-pool-differs");
        let mut row = ClusterNodePoolStorage::get(&ports, &own_pool).expect("the pool");
        row.data.node_pool_id = Some(NodePoolId(id()));
        ClusterNodePoolStorage::put(&mut ports, row);
        refuses(&r, &input, "node-outside-dedicated-pools");
        pool(&mut ports, &ClusterId(id()), &scope, node_pool);
        refuses(&r, &input, "node-in-other-clusters-pool");
        input.node_pool_id = Some(NodePoolId(id()));
        refuses(&r, &input, "node-pool-differs");
        set_cluster(&r, |row| row.data.cloud_id = CloudId(id()));
        refuses(&r, &input, "node-in-other-cloud");
        input.zone = ZoneName("z2".to_owned());
        refuses(&r, &input, "node-zone-differs");
        let session = r.session.clone().expect("an open session");
        let mut row = NodeSessionStorage::get(&ports, &session).expect("the session");
        row.state = NodeSessionState::Closed;
        NodeSessionStorage::put(&mut ports, row);
        refuses(&r, &input, "node-not-connected");
        set_node(&r, |row| row.state = NodeState::Cordoned);
        refuses(&r, &input, "node-not-schedulable");
        reserved(&r, SchedulingRequestId(id()), TenantId(id()));
        refuses(&r, &input, "shared-kernel");
        set_request(&r, |row| row.data.zones = vec![ZoneName("z1".to_owned())]);
        refuses(&r, &input, "outside-zones");
        input.zone = ZoneName("z9".to_owned());
        refuses(&r, &input, "outside-cluster-zones");
        set_request(&r, |row| {
            row.data.min_isolation = IsolationLevel::ProcessSandbox
        });
        set_cluster(&r, |row| row.data.min_isolation = IsolationLevel::Container);
        input.isolation = IsolationLevel::ProcessSandbox;
        refuses(&r, &input, "below-cluster-container");
        set_cluster(&r, |row| {
            row.data.min_isolation = IsolationLevel::UserKernel
        });
        refuses(&r, &input, "below-cluster-user-kernel");
        set_cluster(&r, |row| row.data.min_isolation = IsolationLevel::MicroVm);
        refuses(&r, &input, "below-cluster-microvm");
        set_request(&r, |row| row.data.min_isolation = IsolationLevel::Container);
        refuses(&r, &input, "below-container");
        set_request(&r, |row| {
            row.data.min_isolation = IsolationLevel::UserKernel
        });
        refuses(&r, &input, "below-user-kernel");
        set_request(&r, |row| row.data.min_isolation = IsolationLevel::MicroVm);
        refuses(&r, &input, "below-microvm");
        let elsewhere = NodeId(id());
        input.bindings.push(binding(
            &mut ports,
            &gpu,
            Some(&elsewhere),
            BindingState::Reserved,
            Vec::new(),
        ));
        refuses(&r, &input, "binding-on-other-node");
        let foreign = claim(&mut ports, &ScopeId(id()), "gpu", 1);
        input.bindings.push(binding(
            &mut ports,
            &foreign,
            None,
            BindingState::Reserved,
            Vec::new(),
        ));
        refuses(&r, &input, "binding-of-other-account");
        set_cluster(&r, |row| row.data.formations.clear());
        refuses(&r, &input, "other-cluster");
        let mut instance = InstanceStorage::get(&ports, &r.request.data.instance_id)
            .expect("the request's instance");
        let mut workload = WorkloadStorage::get(&ports, &instance.data.workload_id)
            .expect("the instance's workload");
        workload.data.formation_id = FormationId(id());
        WorkloadStorage::put(&mut ports, workload);
        refuses(&r, &input, "instance-of-other-formation");
        set_request(&r, |row| row.data.formation_id = FormationId(id()));
        refuses(&r, &input, "other-formation");
        instance.data.scope_id = ScopeId(id());
        InstanceStorage::put(&mut ports, instance);
        refuses(&r, &input, "instance-of-other-scope");
        input.tenant_id = TenantId(id());
        refuses(&r, &input, "other-tenant");
        set_request(&r, |row| row.data.scope_id = ScopeId(id()));
        refuses(&r, &input, "other-account");
        reserved(&r, r.request.data.request_id.clone(), TenantId(id()));
        refuses(&r, &input, "already-reserved");
        set_request(&r, |row| row.state = SchedulingRequestState::Unschedulable);
        refuses(&r, &input, "not-pending");

        // The rows the input names, last to first.
        input.cluster_id = ClusterId(id());
        refuses(&r, &input, "no-such-cluster");
        input.node_id = NodeId(id());
        refuses(&r, &input, "no-such-node");
        input.request_id = SchedulingRequestId(id());
        refuses(&r, &input, "no-such-request");

        // The input refusal: after now, or no instant at all.
        input.decided_at = StampedAt(at(3600));
        refuses(&r, &input, "decided-in-future");
        input.decided_at = StampedAt(Timestamp("not an instant".to_owned()));
        refuses(&r, &input, "decided-in-future");
    }

    /// `placed` writes a `Reserved` decision with the request's and the input's fields and
    /// publishes `InstancePlaced`. The request's own instance does not count against its quota,
    /// and of the bindings only the live ones on the node hold its capacity.
    #[test]
    fn place_instance_writes_the_decision_from_the_request_and_the_input() {
        let r = Ready::new();
        let mut ports = r.ports();
        let scope = r.request.data.scope_id.clone();
        set_limit(&r, 1);
        let cpu = claim(&mut ports, &scope, "cpu", 4);
        binding(
            &mut ports,
            &cpu,
            Some(&r.node),
            BindingState::Released,
            Vec::new(),
        );
        binding(
            &mut ports,
            &cpu,
            Some(&NodeId(id())),
            BindingState::Active,
            Vec::new(),
        );
        set_node(&r, |row| {
            row.data.capacity = Some(vec![ResourceAmount {
                resource_class: ResourceClassName("cpu".to_owned()),
                amount: 1,
            }]);
        });
        let input = base(&r);
        let placed = match PlaceInstanceBehavior::place_instance(&mut ports, input.clone())
            .expect("an outcome")
        {
            PlaceInstanceOutcome::Placed { instance_placed } => instance_placed,
            other => panic!("{other:?}"),
        };
        let request = &r.request.data;
        assert_eq!(
            placed,
            InstancePlaced {
                decision_id: placed.decision_id.clone(),
                request_id: request.request_id.clone(),
                node_id: r.node.clone(),
                isolation: IsolationLevel::Container,
                bindings: Vec::new(),
            }
        );
        let row = PlacementDecisionStorage::get(&ports, &placed.decision_id).expect("the row");
        assert_eq!(row.state, PlacementDecisionState::Reserved);
        let data = row.data;
        assert_eq!(data.request_id, request.request_id);
        assert_eq!(data.instance_id, request.instance_id);
        assert_eq!(data.tenant_id, input.tenant_id);
        assert_eq!(data.scope_id, request.scope_id);
        assert_eq!(data.node_id, r.node);
        assert_eq!(data.zone, input.zone);
        assert_eq!(data.formation_id, request.formation_id);
        assert_eq!(data.topology_group_id, None);
        assert_eq!(data.isolation, IsolationLevel::Container);
        assert_eq!(data.min_isolation, request.min_isolation);
        assert!(!data.assigned);
        assert_eq!(data.bindings, Vec::<BindingId>::new());
        assert_eq!(data.gang_id, request.gang_id);
        assert_eq!(data.priority_value, request.priority_value);
        assert_eq!(data.preemptible, request.preemptible);
        assert_eq!(data.grace_seconds, request.grace_seconds);
        assert_eq!(data.decided_at, input.decided_at);
        assert_eq!(
            (
                data.preempted_by,
                data.preemption_reason,
                data.evicting_since,
                data.released_at
            ),
            (None, None, None, None)
        );
        let request = SchedulingRequestStorage::get(&ports, &request.request_id).expect("it");
        assert_eq!(request.state, SchedulingRequestState::Pending);
    }

    /// The answer of `input`, whatever it writes.
    fn answer(r: &Ready, input: &PlaceInstance) -> String {
        let mut ports = r.ports();
        outcome_name(
            &PlaceInstanceBehavior::place_instance(&mut ports, input.clone()).expect("an outcome"),
        )
    }

    /// Another request of the world's tenant, a copy of the world's request claiming `cpu`
    /// `amount`, and its decision on the world's node in `state`, naming `bindings`. The world's
    /// request is left `Pending`.
    fn held(
        r: &Ready,
        amount: i64,
        state: PlacementDecisionState,
        bindings: Vec<BindingId>,
    ) -> weft_types::shared::PlacementDecisionId {
        let mut ports = r.ports();
        let mut other = r.request.clone();
        other.data.request_id = SchedulingRequestId(id());
        other.data.instance_id = InstanceId(id());
        other.state = SchedulingRequestState::Placed;
        other.data.claims = vec![weft_types::workloads::ClaimRequest {
            name: "cpu".to_owned(),
            class: ResourceClassName("cpu".to_owned()),
            amount,
            sharing: None,
            selector: std::collections::BTreeMap::new(),
        }];
        SchedulingRequestStorage::put(&mut ports, other.clone());
        let decision = r.bound();
        let mut row = PlacementDecisionStorage::get(&ports, &decision).expect("the decision");
        row.state = state;
        row.data.request_id = other.data.request_id;
        row.data.instance_id = other.data.instance_id;
        row.data.bindings = bindings;
        PlacementDecisionStorage::put(&mut ports, row);
        set_request(r, |row| row.state = SchedulingRequestState::Pending);
        decision
    }

    /// The world's request claims, in `cpu`, `wanted`; the node's capacity is `cpu` `capacity`.
    fn sized(r: &Ready, wanted: i64, capacity: i64) {
        set_request(r, |row| {
            row.data.claims = vec![weft_types::workloads::ClaimRequest {
                name: "cpu".to_owned(),
                class: ResourceClassName("cpu".to_owned()),
                amount: wanted,
                sharing: None,
                selector: std::collections::BTreeMap::new(),
            }];
        });
        set_node(r, |row| {
            row.data.capacity = Some(vec![ResourceAmount {
                resource_class: ResourceClassName("cpu".to_owned()),
                amount: capacity,
            }]);
        });
    }

    /// `no-capacity` counts the claims of each decision that holds the node (`Reserved`,
    /// `Bound`, `Evicting`), and none of a `Released` one.
    #[test]
    fn the_claims_of_each_decision_that_holds_the_node_take_its_capacity() {
        for (state, expected) in [
            (PlacementDecisionState::Reserved, "no-capacity"),
            (PlacementDecisionState::Bound, "no-capacity"),
            (PlacementDecisionState::Evicting, "no-capacity"),
            (PlacementDecisionState::Released, "placed"),
        ] {
            let r = Ready::new();
            sized(&r, 1, 1);
            held(&r, 1, state, Vec::new());
            assert_eq!(answer(&r, &base(&r)), expected, "a {state:?} decision");
        }
    }

    /// A live binding that a live decision on the node names is that decision's claim, counted
    /// once with the decision; a live binding no decision names is counted on its own.
    #[test]
    fn a_binding_of_a_live_decision_is_counted_once_and_an_unnamed_one_on_its_own() {
        let r = Ready::new();
        sized(&r, 1, 2);
        let mut ports = r.ports();
        let scope = r.request.data.scope_id.clone();
        let cpu = claim(&mut ports, &scope, "cpu", 1);
        let named = binding(
            &mut ports,
            &cpu,
            Some(&r.node),
            BindingState::Active,
            Vec::new(),
        );
        held(&r, 1, PlacementDecisionState::Bound, vec![named]);
        assert_eq!(answer(&r, &base(&r)), "placed", "2 - 1 leaves 1");

        let r = Ready::new();
        sized(&r, 1, 2);
        let mut ports = r.ports();
        let scope = r.request.data.scope_id.clone();
        let cpu = claim(&mut ports, &scope, "cpu", 1);
        binding(
            &mut ports,
            &cpu,
            Some(&r.node),
            BindingState::Active,
            Vec::new(),
        );
        held(&r, 1, PlacementDecisionState::Bound, Vec::new());
        assert_eq!(answer(&r, &base(&r)), "no-capacity", "2 - 1 - 1 leaves 0");
    }

    /// `quota-exceeded` reads the quota of the account that holds the request's scope and
    /// every quota above it, the cases `CreateInstance` answered `over-quota` before the blocks
    /// took it out of the outcome group `quota`: another live instance of the scope counts, a
    /// quota above the account's bounds it too, with no quota for the class none of it may be
    /// held, and an instance of another scope does not count.
    #[test]
    fn place_instance_admits_by_the_quota_of_the_scope_and_every_quota_above_it() {
        let r = Ready::new();
        let mut ports = r.ports();
        // Another live instance of the scope, of the same workload: it and the request claim
        // 1 cpu each.
        let mut sibling = InstanceStorage::get(&ports, &r.request.data.instance_id)
            .expect("the request's instance");
        sibling.data.instance_id = InstanceId(id());
        sibling.data.service_id = None;
        sibling.state = InstanceState::Running;
        InstanceStorage::put(&mut ports, sibling.clone());
        set_limit(&r, 1);
        refuses(&r, &base(&r), "quota-exceeded");

        // A quota above the account's bounds it too.
        let key = QuotaId(Uuid(r.w.path.quota_id.clone()));
        let mut quota = QuotaStorage::get(&ports, &key).expect("the quota row");
        let parent = QuotaId(id());
        let mut above = quota.clone();
        above.data.quota_id = parent.clone();
        above.data.account_id = AccountId(id());
        above.data.limit = 1;
        QuotaStorage::put(&mut ports, above);
        quota.data.limit = 64;
        quota.data.parent_quota_id = Some(parent);
        QuotaStorage::put(&mut ports, quota.clone());
        refuses(&r, &base(&r), "quota-exceeded");

        // No quota for the class: none of it may be held.
        QuotaStorage::delete(&mut ports, &key);
        refuses(&r, &base(&r), "quota-exceeded");

        // An instance of another scope does not count against this scope's quota.
        quota.data.limit = 1;
        quota.data.parent_quota_id = None;
        QuotaStorage::put(&mut ports, quota);
        sibling.data.scope_id = ScopeId(id());
        InstanceStorage::put(&mut ports, sibling);
        assert_eq!(answer(&r, &base(&r)), "placed");
    }

    /// Let every decision go, so the next case is answered on its own.
    fn release_all(ports: &mut Ports) {
        for mut decision in PlacementDecisionStorage::list(ports) {
            decision.state = PlacementDecisionState::Released;
            PlacementDecisionStorage::put(ports, decision);
        }
    }

    /// A decision of `instance` on the world's node in `state`, of a request with no row (so
    /// it takes no capacity of the node); the world's request is left `Pending`.
    fn decision_of(r: &Ready, instance: &InstanceId, state: PlacementDecisionState) {
        let mut ports = r.ports();
        let decision = r.bound();
        let mut row = PlacementDecisionStorage::get(&ports, &decision).expect("the decision");
        row.state = state;
        row.data.request_id = SchedulingRequestId(id());
        row.data.instance_id = instance.clone();
        PlacementDecisionStorage::put(&mut ports, row);
        set_request(r, |row| row.state = SchedulingRequestState::Pending);
    }

    /// The quota counts an instance from `Scheduled` on (`Scheduled`, `Starting`, `Running`,
    /// `Paused`), a `Pending` one only while a decision of it holds a node (`Reserved`,
    /// `Bound`, `Evicting`), and no other: with a quota of 1 cpu, another instance of the scope
    /// that holds leaves no room for the request; one that does not hold leaves the request
    /// placed. A `Pending` instance with no decision holds nothing, so two `Pending` instances
    /// past the quota do not refuse each other
    /// (`adversary_cloud_blocks_rust::of_two_pending_instances_the_quota_admits_one_is_placed`).
    #[test]
    fn the_quota_counts_an_instance_from_scheduled_on_and_a_pending_one_only_while_placed() {
        let r = Ready::new();
        let mut ports = r.ports();
        let mut sibling = InstanceStorage::get(&ports, &r.request.data.instance_id)
            .expect("the request's instance");
        sibling.data.instance_id = InstanceId(id());
        sibling.data.service_id = None;
        set_limit(&r, 1);
        for state in [
            InstanceState::Scheduled,
            InstanceState::Starting,
            InstanceState::Running,
            InstanceState::Paused,
        ] {
            sibling.state = state;
            InstanceStorage::put(&mut ports, sibling.clone());
            refuses(&r, &base(&r), "quota-exceeded");
        }

        sibling.state = InstanceState::Pending;
        InstanceStorage::put(&mut ports, sibling.clone());
        for state in [
            PlacementDecisionState::Reserved,
            PlacementDecisionState::Bound,
            PlacementDecisionState::Evicting,
        ] {
            decision_of(&r, &sibling.data.instance_id, state);
            refuses(&r, &base(&r), "quota-exceeded");
            release_all(&mut ports);
        }
        decision_of(
            &r,
            &sibling.data.instance_id,
            PlacementDecisionState::Released,
        );
        assert_eq!(
            answer(&r, &base(&r)),
            "placed",
            "a Pending instance whose decision is Released"
        );
        release_all(&mut ports);
        assert_eq!(
            answer(&r, &base(&r)),
            "placed",
            "a Pending instance with no decision"
        );
        release_all(&mut ports);

        for state in [
            InstanceState::Exited,
            InstanceState::Failed,
            InstanceState::Evicted,
        ] {
            sibling.state = state;
            InstanceStorage::put(&mut ports, sibling.clone());
            assert_eq!(answer(&r, &base(&r)), "placed", "a {state:?} instance");
            release_all(&mut ports);
        }
    }

    /// `instance-of-other-scope` and `instance-of-other-formation` deny by default: a request
    /// whose instance row is gone, or whose instance's workload row is gone, is answered as a
    /// missing request, and nothing is placed.
    #[test]
    fn a_request_whose_instance_or_its_workload_is_gone_is_answered_as_missing() {
        let r = Ready::new();
        let mut ports = r.ports();
        let instance = InstanceStorage::get(&ports, &r.request.data.instance_id)
            .expect("the request's instance");
        WorkloadStorage::delete(&mut ports, &instance.data.workload_id);
        refuses(&r, &base(&r), "instance-of-other-formation");
        InstanceStorage::delete(&mut ports, &instance.data.instance_id);
        refuses(&r, &base(&r), "instance-of-other-scope");
    }
}

/// `weft.nodes.Assign`: each guard in the selection precedence of its contract, and what
/// `assigned` writes.
mod assign {
    use weft_impl::bindings::outcome_name;
    use weft_impl::context::mint;
    use weft_types::behaviour::{
        AssignmentStorage, NodeSessionStorage, NodeStorage, PlacementDecisionStorage,
    };
    use weft_types::infrastructure::NodeState;
    use weft_types::nodes::obligations::AssignBehavior;
    use weft_types::nodes::{
        Assign, AssignOutcome, AssignmentData, AssignmentSnapshot, AssignmentState, DesiredState,
        InstanceAssigned, NodeSessionState,
    };
    use weft_types::primitives::{Timestamp, Uuid};
    use weft_types::scheduling::PlacementDecisionState;
    use weft_types::shared::{
        AssignmentId, InstanceId, IsolationLevel, NodeId, PlacementDecisionId, StampedAt,
    };

    use super::ready::{at, make_ready, Ready};

    fn id() -> Uuid {
        Uuid(mint())
    }

    /// `input` answers `outcome` and writes no assignment.
    fn refuses(r: &Ready, input: &Assign, outcome: &str) {
        let mut ports = r.ports();
        let before = AssignmentStorage::list(&ports).len();
        let answer = AssignBehavior::assign(&mut ports, input.clone()).expect("an outcome");
        assert_eq!(outcome_name(&answer), outcome, "{input:?}: {answer:?}");
        assert_eq!(AssignmentStorage::list(&ports).len(), before);
    }

    fn input(r: &Ready, decision: &PlacementDecisionId) -> Assign {
        Assign {
            instance_id: r.request.data.instance_id.clone(),
            node_id: r.node.clone(),
            placement_decision_id: decision.clone(),
            assigned_at: StampedAt(at(-1)),
        }
    }

    #[test]
    fn assign_answers_each_guard_in_its_precedence() {
        let r = Ready::new();
        let mut ports = r.ports();
        let decision = r.bound();
        let mut input = input(&r, &decision);

        let mut node = NodeStorage::get(&ports, &r.node).expect("the node");
        node.data.isolation_facts = None;
        NodeStorage::put(&mut ports, node);
        refuses(&r, &input, "isolation-unavailable");
        make_ready(&ports, &r.node, &[IsolationLevel::ProcessSandbox]);
        refuses(&r, &input, "isolation-unavailable");

        r.set_node_state(NodeState::Cordoned);
        refuses(&r, &input, "node-not-ready");
        AssignmentStorage::put(
            &mut ports,
            AssignmentSnapshot {
                state: AssignmentState::Active,
                data: AssignmentData {
                    assignment_id: AssignmentId(id()),
                    instance_id: r.request.data.instance_id.clone(),
                    node_id: NodeId(id()),
                    session_id: r.session.clone().expect("an open session"),
                    placement_decision_id: PlacementDecisionId(id()),
                    min_isolation: IsolationLevel::Container,
                    desired: DesiredState::Run,
                    snapshot_id: None,
                    assigned_at: at(-10),
                    lease_renewed_at: at(-10),
                    ended_at: None,
                },
            },
        );
        refuses(&r, &input, "already-assigned");
        let session = r.session.clone().expect("an open session");
        let mut row = NodeSessionStorage::get(&ports, &session).expect("the session");
        row.state = NodeSessionState::Lost;
        NodeSessionStorage::put(&mut ports, row);
        refuses(&r, &input, "no-open-session");
        let mut row = PlacementDecisionStorage::get(&ports, &decision).expect("the decision");
        let bound = row.clone();
        row.state = PlacementDecisionState::Reserved;
        PlacementDecisionStorage::put(&mut ports, row.clone());
        refuses(&r, &input, "placement-not-bound");
        row.data.node_id = NodeId(id());
        PlacementDecisionStorage::put(&mut ports, row.clone());
        refuses(&r, &input, "placement-mismatch");
        row.data.node_id = bound.data.node_id.clone();
        PlacementDecisionStorage::put(&mut ports, row);
        input.instance_id = InstanceId(id());
        refuses(&r, &input, "placement-mismatch");

        input.node_id = NodeId(id());
        refuses(&r, &input, "no-such-node");
        input.placement_decision_id = PlacementDecisionId(id());
        refuses(&r, &input, "no-such-placement");

        input.assigned_at = StampedAt(at(3600));
        refuses(&r, &input, "assigned-in-future");
        input.assigned_at = StampedAt(Timestamp("not an instant".to_owned()));
        refuses(&r, &input, "assigned-in-future");
    }

    /// `assigned` writes an `Active` assignment on the node's open session at the placement's
    /// isolation, `assigned_at` and `lease_renewed_at` the stamped instant, and publishes
    /// `InstanceAssigned`. A node that offers only a stronger level than the placement's takes
    /// it.
    #[test]
    fn assign_writes_the_assignment_on_the_open_session() {
        let r = Ready::new();
        let mut ports = r.ports();
        let decision = r.bound();
        make_ready(&ports, &r.node, &[IsolationLevel::MicroVm]);
        let input = input(&r, &decision);
        let event = match AssignBehavior::assign(&mut ports, input.clone()).expect("an outcome") {
            AssignOutcome::Assigned { instance_assigned } => instance_assigned,
            other => panic!("{other:?}"),
        };
        let session = r.session.clone().expect("an open session");
        assert_eq!(
            event,
            InstanceAssigned {
                assignment_id: event.assignment_id.clone(),
                instance_id: input.instance_id.clone(),
                node_id: r.node.clone(),
                session_id: session.clone(),
                min_isolation: IsolationLevel::Container,
                placement_decision_id: decision.clone(),
            }
        );
        let row = AssignmentStorage::get(&ports, &event.assignment_id).expect("the row");
        assert_eq!(row.state, AssignmentState::Active);
        assert_eq!(
            row.data,
            AssignmentData {
                assignment_id: event.assignment_id.clone(),
                instance_id: input.instance_id,
                node_id: r.node.clone(),
                session_id: session,
                placement_decision_id: decision,
                min_isolation: IsolationLevel::Container,
                desired: DesiredState::Run,
                snapshot_id: None,
                assigned_at: input.assigned_at.0.clone(),
                lease_renewed_at: input.assigned_at.0,
                ended_at: None,
            }
        );
    }
}

/// The cases of the adversary pass on `story:skeleton-controllers`.
mod adversary_placement {
    use weft_impl::bindings::outcome_name;
    use weft_impl::context::mint;
    use weft_impl::Ports;
    use weft_types::behaviour::{
        AssignmentStorage, BindingStorage, ClaimStorage, NodeStorage, PlacementDecisionStorage,
        SchedulingRequestStorage, ServiceStorage,
    };
    use weft_types::claims::{
        BindingData, BindingSnapshot, BindingState, ClaimData, ClaimHolder, ClaimSnapshot,
        ClaimState, InstanceHolder,
    };
    use weft_types::nodes::{AssignmentData, AssignmentSnapshot, AssignmentState, DesiredState};
    use weft_types::primitives::Uuid;
    use weft_types::scheduling::obligations::PlaceInstanceBehavior;
    use weft_types::scheduling::{PlaceInstance, PlacementDecisionState, SchedulingRequestState};
    use weft_types::shared::{
        AssignmentId, BindingId, ClaimId, InstanceId, IsolationLevel, NodeId, PlacementDecisionId,
        ResourceAmount, ResourceClassName, SchedulingRequestId, ServiceId, Sharing, StampedAt,
        TenantId,
    };

    use super::ready::{at, make_ready, Ready};

    fn id() -> Uuid {
        Uuid(mint())
    }

    /// Sets the node's observed `cpu` capacity.
    fn set_cpu(r: &Ready, amount: i64) {
        let mut ports = r.ports();
        let mut row = NodeStorage::get(&ports, &r.node).expect("the node row");
        row.data.capacity = Some(vec![ResourceAmount {
            resource_class: ResourceClassName("cpu".to_owned()),
            amount,
        }]);
        NodeStorage::put(&mut ports, row);
    }

    /// The input that places the world's request on its node at `isolation`.
    fn input(r: &Ready, isolation: IsolationLevel) -> PlaceInstance {
        let request = &r.request.data;
        let node = NodeStorage::get(&r.ports(), &r.node).expect("the node row");
        PlaceInstance {
            request_id: request.request_id.clone(),
            tenant_id: r.tenant(),
            scope_id: request.scope_id.clone(),
            cluster_id: r.cluster(),
            formation_id: request.formation_id.clone(),
            node_id: r.node.clone(),
            node_pool_id: node.data.node_pool_id,
            zone: node.data.zone,
            isolation,
            bindings: Vec::new(),
            cluster_node_pool_id: None,
            topology_group_id: None,
            peer_groups: Vec::new(),
            decided_at: StampedAt(at(-1)),
        }
    }

    fn outcome(r: &Ready, input: PlaceInstance) -> String {
        let mut ports: Ports = r.ports();
        outcome_name(&PlaceInstanceBehavior::place_instance(&mut ports, input).expect("an outcome"))
    }

    /// A decision of another tenant's instance on the world's node at `isolation`, in `state`;
    /// the world's request left `Pending`.
    fn other_tenant(r: &Ready, isolation: IsolationLevel, state: PlacementDecisionState) {
        let mut ports = r.ports();
        let decision = r.bound();
        let mut row = PlacementDecisionStorage::get(&ports, &decision).expect("the decision");
        row.state = state;
        row.data.request_id = SchedulingRequestId(id());
        row.data.tenant_id = TenantId(id());
        row.data.instance_id = InstanceId(id());
        row.data.isolation = isolation;
        PlacementDecisionStorage::put(&mut ports, row);
        let mut request = SchedulingRequestStorage::get(&ports, &r.request.data.request_id)
            .expect("the request row");
        request.state = SchedulingRequestState::Pending;
        SchedulingRequestStorage::put(&mut ports, request);
    }

    /// Target 3, over-commit across ticks. A service of 2 replicas, each claiming `cpu` 1, and
    /// one Ready node with an open session whose observed `cpu` capacity is 1: the node can
    /// hold one of them. Tick 1 places replica 0 and creates replica 1's request; tick 2 must
    /// not place replica 1 on the same node (`no-capacity`, `ess/5-runtime/scheduling.yaml`:
    /// "the node's free capacity does not cover the claims").
    #[test]
    fn two_replicas_that_each_fill_the_node_are_not_both_placed_on_it() {
        let r = Ready::new();
        set_cpu(&r, 1);
        let mut ports = r.ports();
        let key = ServiceId(Uuid(r.w.ids.service_id.clone()));
        let mut service = ServiceStorage::get(&ports, &key).expect("the service row");
        service.data.replicas = 2;
        ServiceStorage::put(&mut ports, service);

        for _ in 0..3 {
            r.w.process.host().tick();
        }
        let requests = SchedulingRequestStorage::list(&r.ports());
        assert_eq!(
            requests.len(),
            2,
            "positive control, two requests: {requests:?}"
        );
        let holding: Vec<_> = PlacementDecisionStorage::list(&r.ports())
            .into_iter()
            .filter(|row| {
                row.data.node_id == r.node
                    && matches!(
                        row.state,
                        PlacementDecisionState::Reserved | PlacementDecisionState::Bound
                    )
            })
            .map(|row| (row.data.request_id.0 .0, row.state))
            .collect();
        assert_eq!(
            holding.len(),
            1,
            "node with cpu 1 holds {} placements of cpu 1 each: {holding:?}",
            holding.len()
        );
    }

    /// Target 1, `no-capacity`. The placement names its own `Reserved` binding on the node
    /// (`binding-on-other-node`, `device-not-wiped`: "a reserved binding of this placement"),
    /// whose claim is the request's `cpu` 1; the node's capacity is `cpu` 1. The binding is the
    /// request's own claim, so the request's claims fit: counting the binding and the claims
    /// both takes 2 of 1.
    #[test]
    fn a_placement_naming_its_own_reserved_binding_does_not_count_its_claim_twice() {
        let r = Ready::new();
        set_cpu(&r, 1);
        let mut ports = r.ports();
        let claim_id = ClaimId(id());
        ClaimStorage::put(
            &mut ports,
            ClaimSnapshot {
                state: ClaimState::Bound,
                data: ClaimData {
                    claim_id: claim_id.clone(),
                    scope_id: r.request.data.scope_id.clone(),
                    holder: ClaimHolder::Instance(InstanceHolder {
                        instance_id: r.request.data.instance_id.clone(),
                    }),
                    class_name: ResourceClassName("cpu".to_owned()),
                    amount: 1,
                    sharing: Sharing::Exclusive,
                    device_model: None,
                    volume_id: None,
                    filesystem_id: None,
                    bucket_id: None,
                    domain_name: None,
                    egress_allow: Vec::new(),
                    binding_id: None,
                },
            },
        );
        let binding_id = BindingId(id());
        BindingStorage::put(
            &mut ports,
            BindingSnapshot {
                state: BindingState::Reserved,
                data: BindingData {
                    binding_id: binding_id.clone(),
                    claim_id,
                    node_id: Some(r.node.clone()),
                    zone: None,
                    device_ids: Vec::new(),
                    volume_id: None,
                    address: None,
                },
            },
        );
        let mut placing = input(&r, IsolationLevel::Container);
        placing.bindings = vec![binding_id];
        assert_eq!(outcome(&r, placing), "placed");
    }

    /// Target 4, repair. The instance has an `Active` assignment that belongs to another
    /// placement, on another node; this placement is bound and unassigned. `Assign` answers
    /// `already-assigned` (an `Active` assignment of the instance), and the step then records
    /// this placement assigned, though no assignment of it exists
    /// (`RecordPlacementAssigned`: "weft.nodes created the assignment of a bound placement").
    #[test]
    fn an_already_assigned_answer_for_another_placement_does_not_record_this_one() {
        let r = Ready::new();
        let decision = r.bound();
        let mut ports = r.ports();
        AssignmentStorage::put(
            &mut ports,
            AssignmentSnapshot {
                state: AssignmentState::Active,
                data: AssignmentData {
                    assignment_id: AssignmentId(id()),
                    instance_id: r.request.data.instance_id.clone(),
                    node_id: NodeId(id()),
                    session_id: r.session.clone().expect("an open session"),
                    placement_decision_id: PlacementDecisionId(id()),
                    min_isolation: IsolationLevel::Container,
                    desired: DesiredState::Run,
                    snapshot_id: None,
                    assigned_at: at(-10),
                    lease_renewed_at: at(-10),
                    ended_at: None,
                },
            },
        );
        r.w.process.host().tick();
        let of_decision = AssignmentStorage::list(&r.ports())
            .into_iter()
            .filter(|row| row.data.placement_decision_id == decision)
            .count();
        let row = PlacementDecisionStorage::get(&r.ports(), &decision).expect("the decision");
        assert_eq!(
            (row.data.assigned, of_decision),
            (false, 0),
            "placement recorded assigned with {of_decision} assignments of its own"
        );
    }

    /// Target 6, a guard mutant: `holds` without `Bound`. Another tenant's instance runs on the
    /// node in a `Bound` placement at `Container`: placing this tenant's request there at
    /// `Container` shares a kernel (`shared-kernel`, `state: [Reserved, Bound, Evicting]`).
    #[test]
    fn a_bound_placement_of_another_tenant_refuses_a_kernel_sharing_placement() {
        let r = Ready::new();
        other_tenant(&r, IsolationLevel::Container, PlacementDecisionState::Bound);
        assert_eq!(
            outcome(&r, input(&r, IsolationLevel::Container)),
            "shared-kernel"
        );
        let mut ports = r.ports();
        for mut row in PlacementDecisionStorage::list(&ports) {
            if row.state == PlacementDecisionState::Bound {
                row.state = PlacementDecisionState::Evicting;
                PlacementDecisionStorage::put(&mut ports, row);
            }
        }
        assert_eq!(
            outcome(&r, input(&r, IsolationLevel::Container)),
            "shared-kernel"
        );
    }

    /// Target 6, a guard mutant: `shares_kernel` always true. A `MicroVm` placement shares no
    /// kernel, so another tenant's `Container` placement on the node does not refuse it
    /// (`shared-kernel`: `when: {isolation: [UserKernel, Container, ProcessSandbox]}`).
    #[test]
    fn a_microvm_placement_beside_another_tenants_container_is_placed() {
        let r = Ready::new();
        make_ready(
            &r.ports(),
            &r.node,
            &[IsolationLevel::Container, IsolationLevel::MicroVm],
        );
        other_tenant(&r, IsolationLevel::Container, PlacementDecisionState::Bound);
        assert_eq!(outcome(&r, input(&r, IsolationLevel::MicroVm)), "placed");
    }
}

/// Adversary pass on `story:skeleton-end-to-end`: `weft.edge.RecordBackendNotReady` and the
/// hand-run binding `edge-backend-restarting` on an instance whose backend is in traffic
/// (`ess/5-runtime/edge.yaml` `taken-out`, `ess/components.yaml` `edge-backend-restarting`),
/// which the end-to-end test never reaches: on a first start no backend exists.
mod adversary_edge {
    use serde_json::{json, Value};
    use weft_impl::bindings::{edge_backend_restarting, Answer, Runner};
    use weft_impl::context::mint;
    use weft_impl::log::BindingResult;
    use weft_system::SystemEvent;
    use weft_types::behaviour::{InstanceStorage, RouteBackendStorage};
    use weft_types::edge::{BackendSetChanged, RouteBackendState};
    use weft_types::primitives::Uuid;
    use weft_types::shared::{InstanceId, WorkloadId};
    use weft_types::workloads::{InstanceHealthStarting, InstanceReady};

    use super::world::{binding_lines, World};

    /// The world after one tick, its instance's backend added and `Ready`.
    struct InTraffic {
        w: World,
        instance: InstanceId,
        workload: WorkloadId,
    }

    impl InTraffic {
        fn new() -> Self {
            let w = World::new();
            w.process.host().tick();
            let instances = InstanceStorage::list(&w.ports());
            assert_eq!(instances.len(), 1, "{instances:?}");
            let instance = instances[0].data.instance_id.clone();
            let workload = instances[0].data.workload_id.clone();
            let this = Self {
                w,
                instance,
                workload,
            };
            let lines = this.replay(this.ready(), "edge-backend-ready");
            assert_eq!(lines, [(Value::Null, json!("applied"))], "positive control");
            assert_eq!(this.backends(), [RouteBackendState::Ready]);
            this
        }

        fn starting(&self) -> SystemEvent {
            SystemEvent::WorkloadsInstanceHealthStarting(InstanceHealthStarting {
                instance_id: self.instance.clone(),
                workload_id: self.workload.clone(),
            })
        }

        fn ready(&self) -> SystemEvent {
            SystemEvent::WorkloadsInstanceReady(InstanceReady {
                instance_id: self.instance.clone(),
                workload_id: self.workload.clone(),
            })
        }

        /// The states of the instance's backends.
        fn backends(&self) -> Vec<RouteBackendState> {
            RouteBackendStorage::list(&self.w.ports())
                .into_iter()
                .filter(|backend| backend.data.instance_id == self.instance)
                .map(|backend| backend.state)
                .collect()
        }

        /// Replays `event` on the host: the (refusal, result) of each line of `binding`.
        fn replay(&self, event: SystemEvent, binding: &str) -> Vec<(Value, Value)> {
            self.replay_lines(event, binding).0
        }

        /// As [`InTraffic::replay`], with the names of the event lines after it.
        fn replay_lines(
            &self,
            event: SystemEvent,
            binding: &str,
        ) -> (Vec<(Value, Value)>, Vec<Value>) {
            let seq = self.w.process.last_seq();
            self.w.process.host().replay(vec![event]);
            let lines = self.w.process.log_after(seq);
            let answers = binding_lines(&lines, binding)
                .iter()
                .map(|line| (line["refusal"].clone(), line["result"].clone()))
                .collect();
            let events = lines
                .iter()
                .filter(|line| line["kind"] == json!("event"))
                .map(|line| line["name"].clone())
                .collect();
            (answers, events)
        }
    }

    /// A restart of an instance in traffic (`RecordProcessRestarted` `restarted` publishes
    /// `InstanceHealthStarting`): `edge-backend-restarting` applies, the backend reads `NotReady`
    /// and `BackendSetChanged` is logged; a repeated delivery is `not-ready`, dropped; the next
    /// `InstanceReady` is `known` for `edge-backend-ready` and puts the backend back through
    /// `edge-backend-restore`.
    #[test]
    fn a_restart_takes_a_ready_backend_out_of_traffic_once_and_readiness_puts_it_back() {
        let t = InTraffic::new();

        let (restarting, events) = t.replay_lines(t.starting(), "edge-backend-restarting");
        assert_eq!(restarting, [(Value::Null, json!("applied"))]);
        assert_eq!(events, [json!("weft.edge.BackendSetChanged")]);
        assert_eq!(t.backends(), [RouteBackendState::NotReady]);

        let (again, events) = t.replay_lines(t.starting(), "edge-backend-restarting");
        assert_eq!(again, [(json!("not-ready"), json!("dropped"))]);
        assert_eq!(events, Vec::<Value>::new());
        assert_eq!(t.backends(), [RouteBackendState::NotReady]);

        assert_eq!(
            t.replay(t.ready(), "edge-backend-ready"),
            [(json!("known"), json!("dropped"))]
        );
        assert_eq!(t.backends(), [RouteBackendState::Ready]);
        assert_eq!(
            t.replay(t.ready(), "edge-backend-restore"),
            [(json!("not-out-of-traffic"), json!("dropped"))],
            "a second readiness finds the backend in traffic already"
        );
    }

    /// The generated System delivers `InstanceHealthStarting` to no binding of its own (it is
    /// removed from synthesis): a redelivery leaves the backend `Ready`. The runner's delivery of
    /// the same event takes it out with `BackendSetChanged { ready: false }` naming the backend.
    #[test]
    fn only_the_runner_takes_a_backend_out_and_its_event_reads_ready_false() {
        let t = InTraffic::new();
        let ports = t.w.ports();
        let backend_id = RouteBackendStorage::list(&ports)
            .into_iter()
            .find(|backend| backend.data.instance_id == t.instance)
            .expect("the backend")
            .data
            .backend_id;
        let mut system = weft_impl::system(&ports);

        let _ = system.redeliver(&t.starting());
        assert_eq!(
            t.backends(),
            [RouteBackendState::Ready],
            "a generated binding of InstanceHealthStarting took the backend out"
        );

        let runner = Runner::open(&ports);
        let answers = runner.deliver(&mut system, vec![t.starting()]);
        assert_eq!(
            answers,
            [Answer {
                binding: "edge-backend-restarting",
                refusal: None,
                result: BindingResult::Applied,
                attempt: 1,
                events: vec![SystemEvent::EdgeBackendSetChanged(BackendSetChanged {
                    backend_id,
                    instance_id: t.instance.clone(),
                    workload_id: t.workload.clone(),
                    ready: false,
                })],
            }]
        );
        assert_eq!(runner.held(), 0);
        assert_eq!(t.backends(), [RouteBackendState::NotReady]);
    }

    /// The held form of `InstanceHealthStarting` reads back as the event, and another event has
    /// none.
    #[test]
    fn the_held_form_of_instance_health_starting_reads_back() {
        let instance_id = InstanceId(Uuid(mint()));
        let workload_id = WorkloadId(Uuid(mint()));
        let starting = SystemEvent::WorkloadsInstanceHealthStarting(InstanceHealthStarting {
            instance_id: instance_id.clone(),
            workload_id: workload_id.clone(),
        });
        let ready = SystemEvent::WorkloadsInstanceReady(InstanceReady {
            instance_id,
            workload_id,
        });
        let entry = &edge_backend_restarting::ENTRY;
        let held = (entry.encode)(&starting).expect("a held form");
        assert_eq!((entry.decode)(&held), Ok(starting));
        assert_eq!((entry.encode)(&ready), None);
    }
}

/// Adversary of `story:cloud-blocks-rust`: the quota moved from `CreateInstance` to
/// `PlaceInstance`, and the scope checks the blocks dropped from `SubmitSchedulingRequest`.
mod adversary_cloud_blocks_rust {
    use serde_json::json;
    use weft_controllers::placement::STEP;
    use weft_impl::bindings::outcome_name;
    use weft_impl::context::mint;
    use weft_system::SystemEvent;
    use weft_testkit::steps;
    use weft_types::behaviour::{
        ClusterStorage, InstanceStorage, NodeStorage, QuotaStorage, SchedulingRequestStorage,
    };
    use weft_types::cluster::ClusterState;
    use weft_types::primitives::Uuid;
    use weft_types::scheduling::obligations::PlaceInstanceBehavior;
    use weft_types::scheduling::{
        PlaceInstance, PlacementDecisions, SchedulingRequestSnapshot, SchedulingRequestState,
    };
    use weft_types::shared::{
        AccountId, InstanceId, IsolationLevel, QuotaId, SchedulingRequestId, ScopeId, ServiceId,
        StampedAt, WorkloadId,
    };
    use weft_types::workloads::{InstanceState, ReplicaRequested};

    use super::ready::{another_node, at, Ready};

    /// A node identity that sorts before any minted one in the view's order.
    const FIRST: &str = "00000000-0000-4000-8000-000000000000";

    fn decisions(r: &Ready) -> Vec<PlacementDecisions> {
        r.w.process
            .host()
            .system()
            .weft_control
            .placement_decisions()
            .expect("the view reads")
    }

    fn set_limit(r: &Ready, limit: i64) {
        let mut ports = r.ports();
        let key = QuotaId(Uuid(r.w.path.quota_id.clone()));
        let mut quota = QuotaStorage::get(&ports, &key).expect("the quota row");
        quota.data.limit = limit;
        quota.data.remaining = limit;
        QuotaStorage::put(&mut ports, quota);
    }

    /// The input that places `request` on the world's node at `Container`, as the Scheduler
    /// sends it.
    fn input(r: &Ready, request: &SchedulingRequestSnapshot) -> PlaceInstance {
        let node = NodeStorage::get(&r.ports(), &r.node).expect("the node row");
        PlaceInstance {
            request_id: request.data.request_id.clone(),
            tenant_id: r.tenant(),
            scope_id: request.data.scope_id.clone(),
            cluster_id: r.cluster(),
            formation_id: request.data.formation_id.clone(),
            node_id: r.node.clone(),
            node_pool_id: node.data.node_pool_id,
            zone: node.data.zone,
            isolation: IsolationLevel::Container,
            bindings: Vec::new(),
            cluster_node_pool_id: None,
            topology_group_id: None,
            peer_groups: Vec::new(),
            decided_at: StampedAt(at(-1)),
        }
    }

    fn place(r: &Ready, input: PlaceInstance) -> String {
        let mut ports = r.ports();
        outcome_name(&PlaceInstanceBehavior::place_instance(&mut ports, input).expect("an outcome"))
    }

    /// Before the blocks, `CreateInstance` refused an instance past the scope's quota, so the
    /// live instances of a scope never held more than its quota and `PlaceInstance` could count
    /// a `Pending` one as held. Now an instance past the quota is created `Pending` too: two
    /// `Pending` instances of 1 cpu each under a quota of 1 cpu each count the other as held,
    /// and neither is ever placed. The quota admits one of them; one of them is placed.
    #[test]
    fn of_two_pending_instances_the_quota_admits_one_is_placed() {
        let r = Ready::new();
        set_limit(&r, 1);
        let mut ports = r.ports();
        let mut sibling = InstanceStorage::get(&ports, &r.request.data.instance_id)
            .expect("the request's instance");
        sibling.data.instance_id = InstanceId(Uuid(mint()));
        sibling.data.service_id = None;
        sibling.state = InstanceState::Pending;
        InstanceStorage::put(&mut ports, sibling.clone());
        let mut other = r.request.clone();
        other.data.request_id = SchedulingRequestId(Uuid(mint()));
        other.data.instance_id = sibling.data.instance_id.clone();
        SchedulingRequestStorage::put(&mut ports, other.clone());

        let first = place(&r, input(&r, &r.request));
        let second = place(&r, input(&r, &other));
        assert!(
            first == "placed" || second == "placed",
            "a quota of 1 cpu and two Pending instances of 1 cpu: neither placed ({first}, {second})"
        );
    }

    /// The same through the running controllers: the world's replica is `Pending` within its
    /// quota of 1 cpu; a second replica past the quota is created (the blocks took
    /// `CreateInstance` out of the outcome group `quota`). Before, the first replica was placed
    /// on the next tick and the second refused at creation; now the Scheduler places neither.
    #[test]
    fn a_replica_past_the_quota_does_not_keep_the_replica_within_it_from_being_placed() {
        let r = Ready::new();
        set_limit(&r, 1);
        r.w.process
            .host()
            .replay(vec![SystemEvent::WorkloadsReplicaRequested(
                ReplicaRequested {
                    service_id: ServiceId(Uuid(r.w.ids.service_id.clone())),
                    workload_id: WorkloadId(Uuid(r.w.ids.workload_id.clone())),
                    replica_index: 1,
                },
            )]);
        let pending = SchedulingRequestStorage::list(&r.ports())
            .into_iter()
            .filter(|row| row.state == SchedulingRequestState::Pending)
            .count();
        assert_eq!(pending, 2, "the replica past the quota has its request");
        for _ in 0..4 {
            r.w.process.host().tick();
        }
        let placed: Vec<_> = decisions(&r)
            .into_iter()
            .filter(|row| row.instance_id == r.request.data.instance_id)
            .collect();
        assert_eq!(
            placed.len(),
            1,
            "the replica within the quota is not placed: {:?}",
            decisions(&r)
        );
    }

    /// The blocks dropped `SubmitSchedulingRequest`'s check that the instance is the request's
    /// scope's (`instance-of-other-account`); the story moves the dropped scope checks to
    /// weft's run-time admission. A ReleaseController of `globex` (granted
    /// `SubmitSchedulingRequest`) asks to schedule `acme`'s placed instance in its own scope and
    /// formation: the Scheduler must not place `acme`'s instance as `globex`'s, under
    /// `globex`'s tenant (which `shared-kernel` then compares) and quota.
    #[test]
    fn another_scope_cannot_get_this_scope_s_instance_placed() {
        let r = Ready::new();
        r.w.process.host().tick();
        let acme_instance = r.request.data.instance_id.clone();
        let acme_scope = r.request.data.scope_id.clone();
        assert_eq!(
            decisions(&r)
                .iter()
                .filter(|row| row.instance_id == acme_instance)
                .count(),
            1,
            "acme's request is placed first"
        );
        another_node(&r.ports(), &r.node, FIRST);

        let client = r.w.process.client();
        let tenant = steps::create_tenant(client, "globex");
        let account = steps::create_account(client, &tenant);
        let scope = steps::record_account_scope(client, &account);
        steps::grant_top_quota(client, &account);
        let caller = steps::account_caller(&tenant, &account, &scope);
        let formation = steps::create_formation(client, &caller, &scope);
        steps::pin_formation(client, &caller, &scope, &r.w.path.cluster_id, &formation);

        let answer = client
            .command(
                "weft.scheduling.SubmitSchedulingRequest",
                "weft.actors.ReleaseController",
                caller.clone(),
                json!({
                    "instance_id": acme_instance.0 .0,
                    "scope_id": scope,
                    "formation_id": formation,
                    "claims": [{"name": "cpu", "class": "cpu", "amount": 1, "selector": {}}],
                    "min_isolation": "Container",
                    "zones": [],
                    "priority": r.request.data.priority.0,
                    "gang_id": null,
                }),
            )
            .expect("the control host answers");
        let outcome = answer.body["outcome"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        for _ in 0..2 {
            r.w.process.host().tick();
        }
        let foreign: Vec<_> = decisions(&r)
            .into_iter()
            .filter(|row| row.instance_id == acme_instance && row.scope_id != acme_scope)
            .collect();
        assert!(
            foreign.is_empty(),
            "SubmitSchedulingRequest answered {outcome} ({}); acme's instance placed in another scope: {foreign:?}",
            answer.status
        );
    }

    /// Catches the mutant "the quota counts only the instances of the request's own scope": a
    /// `Running` instance of another scope, whose account's quota draws from the quota of the
    /// account that holds the request's scope, holds part of that quota.
    #[test]
    fn an_instance_of_a_scope_whose_quota_draws_from_this_one_counts_against_it() {
        let r = Ready::new();
        let client = r.w.process.client();
        let tenant = steps::create_tenant(client, "initech");
        let account = steps::create_account(client, &tenant);
        let scope = steps::record_account_scope(client, &account);
        let mut ports = r.ports();
        let key = QuotaId(Uuid(r.w.path.quota_id.clone()));
        let mut child = QuotaStorage::get(&ports, &key).expect("the quota row");
        child.data.quota_id = QuotaId(Uuid(mint()));
        child.data.account_id = AccountId(Uuid(account));
        child.data.parent_quota_id = Some(key);
        child.data.limit = 64;
        QuotaStorage::put(&mut ports, child);
        set_limit(&r, 1);
        let mut sibling = InstanceStorage::get(&ports, &r.request.data.instance_id)
            .expect("the request's instance");
        sibling.data.instance_id = InstanceId(Uuid(mint()));
        sibling.data.service_id = None;
        sibling.data.scope_id = ScopeId(Uuid(scope));
        sibling.state = InstanceState::Running;
        InstanceStorage::put(&mut ports, sibling);
        assert_eq!(place(&r, input(&r, &r.request)), "quota-exceeded");
    }

    /// Catches the mutant "the placement step takes a deleted cluster as the formation's":
    /// `PlaceInstance` has no guard on the cluster's state, so the step's filter is the only
    /// thing that keeps a request of a formation pinned to a deleted cluster from being placed.
    /// The step sends nothing for it and does not wait.
    #[test]
    fn a_request_of_a_formation_pinned_to_a_deleted_cluster_is_not_sent() {
        let r = Ready::new();
        let mut ports = r.ports();
        let mut cluster = ClusterStorage::get(&ports, &r.cluster()).expect("the cluster row");
        cluster.state = ClusterState::Deleted;
        ClusterStorage::put(&mut ports, cluster);
        r.w.process.host().tick();
        assert!(decisions(&r).is_empty(), "{:?}", decisions(&r));
        assert!(r.ports().store().step_waits(STEP).is_empty());
    }
}
