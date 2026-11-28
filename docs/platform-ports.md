# weft platform ports

Status: design, 2026-11-28. Epic `epic:platform-ports` (`.engineering/planning/epic/platform-ports.md`).
Order: the owner, 2026-11-28 about 00:55 CET: "let's do the architectural change. Make the design
right now, and then still implement the most cheapest, most local, least distributed substrate
layer or ports and adapter layer possible"; about 00:50 CET: "substrate is absolutely right for
now" (epic § Order).

Sources and the revisions read:

- weft: this tree, branch off `main` at `89cca7b`.
- ESS: `ess 0.99.2`, the release `ess/ess-inputs.yaml:2` requires; repository paths are read at tag
  `0.99.2` of `beyond10x/ess` and written `ess@0.99.2:<path>:<line>`.
- substrate: tag `0.29.0` of `beyond10x/substrate`, the release weft pins
  (`tools/test-setup.d/20-substrate.sh:33`, SDK rev `1a10a30d68` in
  `product/cloud/crates/weft-agent/Cargo.toml:29`); written `substrate@0.29.0:<path>:<line>`.
- connectors: commit `56969ef30` of `beyond10x/connectors`; written `connectors@56969ef30:<path>:<line>`.

## Contents

1. Terms
2. The two layers
3. Rules every port follows
4. What the specification needs from the place weft runs
5. Node-layer ports
6. Cloud-layer ports
7. Outside the ports
8. Adapters per placement
9. How the specification declares the ports
10. Substrate gaps
11. Connectors gaps
12. The first build
13. Stories of the epic

## 1. Terms

- **Port**: a Rust trait in weft's code that names one capability weft needs from the place it
  runs, with its operations, inputs, outputs and named refusals. Not the two ports ESS synthesis
  generates (`storage`, one per entity, and `context`, `docs/implementation.md:45-47`); those stay
  as they are in `product/cloud/crates/weft-impl/src/storage/mod.rs` and
  `product/cloud/crates/weft-impl/src/context/mod.rs`.
- **Adapter**: the code that serves one port for one placement.
- **Placement**: `weft.infrastructure.Placement`, `Local | Hosts | Aws | Kubernetes`
  (`ess/1-infrastructure/infrastructure.yaml:23-25`), stored on the cloud and fixed at creation
  (`ess/1-infrastructure/infrastructure.yaml:77-79`).
- **Place**: what an adapter reaches: a substrate daemon, a process table, a vendor API through
  connectors, a Kubernetes API.
- **Step**: a level or time step of a controller (`ess/components.yaml:1079-1082`): it reads a gap
  view, calls a port, and records what it observed through a command of the specification.

## 2. The two layers

| Layer | Port | Served by | In code in the first build |
|---|---|---|---|
| node | N1 `exec` | substrate | yes, local adapter |
| node | N2 `node-facts` | substrate | yes, local adapter |
| node | N3 `workloads` | substrate (future family) | no |
| node | N4 `endpoints` | substrate (future family) | no |
| node | N5 `volumes` | substrate (future family) | no |
| node | N6 `images` | substrate (future family) | no |
| node | N7 `devices` | not on substrate's roadmap | no |
| cloud | C1 `machines` | weft's adapters | yes, local adapter |
| cloud | C2 `edge` | weft's adapters | no |
| cloud | C3 `dns` | weft's adapters | no |
| cloud | C4 `certificates` | weft's adapters | no |
| cloud | C5 `objects` | weft's adapters | no |
| cloud | C6 `secret-store` | weft's adapters | no |
| cloud | C7 `zone-volumes` | weft's adapters | no |
| cloud | C8 `managed-services` | weft on its own nodes; a vendor adapter as an option | no |
| cloud | C9 `network` | weft's adapters | no |

Node layer: 7 ports. Cloud layer: 9 ports.

- The node layer is what runs on one node. Substrate names these families: "machine, workspace,
  exec, session, workload, image, volume, endpoint and operation"
  (`substrate@0.29.0:architecture/domain-model.md:8-18`); "Workloads, images, volumes and endpoints
  remain future resource families" (`substrate@0.29.0:README.md:5`).
- The cloud layer spans the installation. Substrate leaves it to its consumers: "Fleets, placement,
  and federation belong to consumers. substrate is not a scheduler"
  (`substrate@0.29.0:docs/VISION.md:64-66`); "TLS termination, DNS, and public routing are out of
  scope for v1" (`substrate@0.29.0:docs/VISION.md:89-90`).
- Adapters of the cloud layer that reach a vendor API do so through connectors, so weft holds no
  vendor credential: a caller of a served connectors host holds only that host's bearer
  (`connectors@56969ef30:crates/connectors-host/src/server.rs:93-101`).

## 3. Rules every port follows

1. **Keyed by weft's identity.** Every operation that changes the place takes the identity weft
   already minted for the thing (`node_id`, `volume_id`, `bucket_id`, `certificate_id`,
   `assignment_id` and process name). A repeat with the same key answers the first outcome and
   changes nothing. Substrate keys its mutations the same way with a client-minted `op`
   (`substrate@0.29.0:docs/design/01-contract.md:20-26`); an adapter derives the `op` from weft's key, so a
   retry after an unanswered call is the same operation ("Never mint a new `op` for an automatic
   mutation retry", `substrate@0.29.0:docs/design/01-contract.md:330`).
2. **Observed, not claimed.** An operation answers what it observed after the change, or
   `accepted`; the outcome weft records is what `observe` reads from the place, never the request
   echoed back. This is substrate's rule: "Every resource mutation answers with state re-read from
   the driver, never the request echoed back. Absence stays absence: unknown is never rendered as
   zero or as success" (`substrate@0.29.0:docs/VISION.md:48-50`), and weft's: "The agent reports what it
   observed, never what it was told" (`docs/domain-sketch.md:477`). An `observe` that cannot reach
   the place answers `Unknown`, never `Gone`.
3. **Recorded through the specification.** What a step observed enters weft's state only through
   a command of the specification, sent as the actor the specification grants (the Report and
   Record commands of § 4). A port writes no row.
4. **Named refusals.** Each port has a closed refusal enum. Three refusals are shared by all:
   `Unavailable { detail }` (the place did not answer; transient, the step tries again on its next
   tick), `NotServed { operation }` (this placement's adapter does not serve the operation), and
   `Invalid { field, reason }` (the input breaks a bound the place states). A substrate refusal is
   carried with its code (`{class, code, message, retriable}`,
   `substrate@0.29.0:docs/design/01-contract.md:302-305`).
5. **One adapter per port per control host**, chosen from the placement the control host serves
   (§ 9, S1). No code outside an adapter names substrate's SDK or starts a node process (§ 12.5).
6. **Default deny.** A capability an adapter cannot prove is refused, never assumed: "A missing
   isolation or capability guarantee is a named refusal, never silent degradation" (substrate's
   invariant 3, `substrate@0.29.0:AGENTS.md:38-39`); "Isolation, secrets and identity default to deny"
   (`AGENTS.md` § Rules for the specification).

## 4. What the specification needs from the place weft runs

`ess/` has 23 domains (`ess/system.yaml:10-33`) and 439 commands:

```sh
for f in ess/*/*.yaml; do
  awk '/^commands:/{c=1;next} /^[a-z_]+:/{c=0} c && /^  - name: weft\./{n++} END{print n+0}' "$f"
done | paste -sd+ | bc     # 439
```

The names and comments of all 439 were read, and the commands that may act outside weft's own
state were read in full. Two patterns occur:

- **A. Request, then observation.** The command records the wish and emits an event; something
  outside makes it real; a later Report or Record command, by a granted actor, records what was
  observed. Example: `CreateBucket` (`ess/3-resources/resources.yaml:733`) and
  `ReportBucketProvisioned` (`ess/3-resources/resources.yaml:775`), "What a provisioner reports for a
  storage unit it was asked to create" (`ess/3-resources/resources.yaml:32`). The port call is made by
  a step between the two.
- **B. Inside the decision.** An `external:` branch refuses when the place says no. The port call is
  made by the decider of the context port (`product/cloud/crates/weft-impl/src/context/mod.rs:202-208`,
  `product/cloud/crates/weft-impl/src/context/decider.rs:1-22`) while the command runs. Example:
  `AddSecretVersion/unavailable`, "the secret's backend does not store the value"
  (`ess/5-runtime/secrets.yaml:551-554`).

The commands whose effect leaves weft's state, by port. "Recorded by" is the command that records
the observation (pattern A) or "external" (pattern B).

| Command | Effect outside weft's state | Port | Recorded by |
|---|---|---|---|
| `infrastructure.CreateNodePool` `ess/1-infrastructure/infrastructure.yaml:692` (desired size = `min_size`, `:740`), `RequestNodePoolScaleUp` `:811` | the pool needs more machines | C1 `machines.make` | `ProvisionPoolNode` `:989`, then the node's `JoinNode` `:1080` and `RecordNodeObservation` `:1118` |
| `infrastructure.ProvisionPoolNode` `:989` ("The pool's provider records the machine it made", `:985`) | a machine that holds the join token | C1 `machines.make` | `JoinNode` `:1080` |
| `infrastructure.IssueJoinToken` `:1041` | the new token reaches the machine | C1 `machines.make` | `JoinNode` `:1080` |
| `infrastructure.RequestNodePoolScaleDown` `:841`, `DrainNodePool` `:875`, `DrainNode` `:1215`, `RemoveNode` `:1237`, `DeleteNodePool` `:899` | the machine goes | C1 `machines.remove` | `RemoveNode` records `Gone` first; the step then removes and observes |
| `infrastructure.ProvisionNode` `:937` ("The operator records a machine it brought itself (a host on `hosts`)", `:930-932`) | none: the operator brought the machine | none | `JoinNode` |
| `scheduling.RequestScaleUp` `ess/5-runtime/scheduling.yaml:1558`, `RequestScaleDown` `:1608` | through `RequestNodePoolScaleUp`/`Down` | C1 | `RecordScaleFulfilled` `:1668`, `RecordScaleRefused` `:1690` |
| `inventory.ReportDevice` .. `ReportDeviceRemoved` `ess/1-infrastructure/inventory.yaml:311-472`, `ResetDevice` `:450` | a device listed, wiped, sliced | N7 `devices` | the Report commands themselves (NodeAgent) |
| `resources.CreateVolume` `ess/3-resources/resources.yaml:361`, `RestoreVolume` `:402` | a volume, node-local (`node_id` set, `:61-62`) or in a zone | N5 `volumes` or C7 `zone-volumes` | `ReportVolumeProvisioned` `:463` (NodeAgent, `ess/9-actors/actors.yaml:700`) |
| `resources.SnapshotVolume` `:534`, `DeleteVolume` `:495`, `DeleteVolumeSnapshot` `:609` | snapshot, delete | N5 or C7 | `ReportVolumeSnapshotTaken` `:577` |
| `resources.CreateFilesystem` `:634`, `DeleteFilesystem` `:702` | a shared filesystem | C7 `zone-volumes` | `ReportFilesystemProvisioned` `:670` |
| `resources.CreateBucket` `:733`, `DeleteBucket` `:808` (`not-empty`, "the object store still holds objects in the bucket", `:827-829`) | a bucket | C5 `objects` | `ReportBucketProvisioned` `:775`; `not-empty` external |
| `artifacts.RegisterArtifact` `ess/3-resources/artifacts.yaml:196` | the bytes of a content-addressed artifact | C5 `objects` (artifact store) | none: the bytes are stored before the command |
| `workloads.RecordArtifactVerified` `ess/4-applications/workloads.yaml:2483` | the artifact fetched, its digest checked, a root filesystem made | N6 `images`, C5 `objects` | itself (NodeAgent) |
| `nodes.Assign` `ess/5-runtime/nodes.yaml:1015`, `node.StartProcess` `ess/5-runtime/node.yaml:303` | a process of the instance runs | N3 `workloads`; N1 `exec` for a process that ends within exec bounds | `workloads.RecordInstanceRunning` `ess/4-applications/workloads.yaml:2565` |
| `node.RestartProcess` `ess/5-runtime/node.yaml:415`, `StopProcess` `:503`, `StopAssignmentProcesses` `:534` | restart, stop | N3 | `workloads.RecordProcessStarted` `:2986`, `RecordInstanceExited` `:3173` |
| `nodes.Stop` `ess/5-runtime/nodes.yaml:1113`, `Pause` `:1137`, `Resume` `:1163`, `Snapshot` `:1189` | stop, freeze, thaw, checkpoint | N3 | `nodes.InstanceObserved` `:1377`, `workloads.RecordSnapshotTaken` `:5738` |
| `nodes.Attach` `:1645`, `Detach` `:1691` | a volume mounted into an instance | N5 | `nodes.InstanceObserved` `:1377` |
| `nodes.OpenExecSession` `:1724`, `ExecEnded` `:1840` | a command run inside the instance | N1 `exec` | `ExecEnded` |
| `workloads.RecordCheckResult` `ess/4-applications/workloads.yaml:2630` | HTTP, TCP or command probes (`docs/domain-sketch.md:274-275`) | N4 `endpoints`, N1 `exec` | itself (NodeAgent) |
| `infrastructure.RecordNodeObservation` `ess/1-infrastructure/infrastructure.yaml:1118`, `nodes.NodeObserved` `ess/5-runtime/nodes.yaml:1439`, `UsageReported` `:1483` | capacity, isolation facts, usage measured on the node | N2 `node-facts` | themselves (NodeAgent) |
| `node.OpenLocalEndpointSession` `ess/5-runtime/node.yaml:561` (`not-from-instance`, "the call did not come from inside instance_id's own network namespace", `:568-571`) | the instance's local endpoint | N4 `endpoints` | external |
| `edge.RecordBackendReady` `ess/5-runtime/edge.yaml:309` .. `RemoveBackend` `:408` | an address of each ready instance the edge forwards to | N4 `endpoints` | the edge commands themselves |
| `formations.AddEndpoint` `ess/4-applications/formations.yaml:1506` (`host-not-verified`, `:1524-1527`), `RemoveEndpoint` `:1542`, `AddRoute` `:1760`, `RemoveRoute` `:1820` | a public address for the host; proof the account controls the host | C2 `edge`, C3 `dns` | external (`host-not-verified`) |
| `edge.RequestCertificate` `ess/5-runtime/edge.yaml:565`, `RetryCertificate` `:666`, `RetireEndpointCertificates` `:689` (issuer OPEN, `:153-154`) | a certificate for the host | C4 `certificates` | `RecordCertificateIssued` `:598`, `RecordCertificateFailed` `:639` (EdgeGateway, `ess/9-actors/actors.yaml:907-908`) |
| `formations.AddLink` `ess/4-applications/formations.yaml:1398`, `RemoveLink` `:1458`; `Formation.internal_domain` `:90-93` | traffic allowed between two components; internal names | C9 `network` | none |
| `claims.DeclareClaim` `ess/5-runtime/claims.yaml:311` (`egress-denied` `:389-391`, `domain-not-verified` `:392-394`) | egress policy; domain proof | C9 `network`, C3 `dns` | external |
| `identity.CreateTrustDomain` `ess/5-runtime/identity.yaml:484` ("The key itself is made and held by key custody", `:482-483`), `RotateSigningKey` `:562`, `IssueIdentityDocument` `:676`, `RenewIdentityDocument` `:802` | a signing key made and used | C6 `secret-store` (keys) | none |
| `secrets.RegisterSecretBackend` `ess/5-runtime/secrets.yaml:388` (`unreachable` `:408-411`), `AddSecretVersion` `:525` (`unavailable` `:551-554`), `DeliverSecret` `:741` (`unavailable` `:882-885`) | a value stored, read; the vault answers | C6 `secret-store` | external |
| `observability.OpenLogStream` `ess/5-runtime/observability.yaml:689` | the output of a process | N3, N1 | `CloseLogStream` `:771` |
| `observability.ConfigureBucketSink` `:874` | telemetry written to a bucket | C5 `objects` | `RecordSinkDelivered` `:927`, `RecordSinkFailing` `:950` (ControlPlane) |
| `pipelines.CreateBuildCache` `ess/6-delivery/pipelines.yaml:1157` ("The controller provisioned the volume (weft.resources) first", `:1156`) | through `resources.CreateVolume` | N5 or C7 | `ReportVolumeProvisioned` |
| `catalog.ProvisionServiceInstance` `ess/7-services/catalog.yaml:1095`, `ChangeServicePlan` `:1180`, `MoveServiceInstanceEngine` `:1264`, `DeprovisionServiceInstance` `:1362` | a running service instance | C8 `managed-services` | `ReportServiceInstanceReady` `:1309`, `Degraded` `:1334`, `Gone` `:1394` (ControlPlane) |
| `catalog.BackupServiceInstance` `:1577`, `RestoreServiceInstance` `:1671` | a backup stored, a restore run | C8, C5 | `ReportBackupCompleted` `:1620` .. `ReportRestoreFailed` `:1754` |
| `catalog.InstallProduct` `:1954`, `UpgradeInstallation` `:2049`, `UninstallProduct` `:2208` | a formation of the product runs | weft's own nodes (N-layer through the scheduler) | `ReportInstallationInstalled` `:2159` .. `Uninstalled` `:2234` |
| `infrastructure.CreateCloud` `ess/1-infrastructure/infrastructure.yaml:510` | which adapters serve the cloud | adapter selection (§ 9, S1) | none today |
| `infrastructure.ScaleControlPlane` `:599` | control host replicas | installation, § 7 | none |

## 5. Node-layer ports

### N1 `exec`: one confined process that runs to its end

- Operations: `run(process, on_state) -> Exited`. Input: `argv`, `env`, read-only host files,
  limits (timeout, CPU time, memory, processes, output bytes; "Substrate takes no default for any
  of them", `product/cloud/crates/weft-agent/src/confine.rs:41-49`, `:63-70`). Output: exit code,
  stdout, stderr, `truncated` (`:102-111`). States reported in order: `accepted`, `running`,
  `exited`, `cancelled`, `expired`, `unknown` (`:336-363`), as weft's own enum, not substrate's
  re-exported type (`:23` today).
- Refusals: `EmptyCommand`, `HostPath`, `Ended { state, refusal }`, `NoExit(after)`
  (`confine.rs:113-128`), `Refused { class, code, message, retriable }` (a substrate refusal such
  as `exec.sandbox-unavailable`, `substrate@0.29.0:README.md:12-14`, with its class `Refused`,
  `Conflict`, `Unserved`, `Exhausted` or `Failed` and substrate's retriable flag), `Invalid
  { field, reason }` (limits outside substrate's contract bounds), `Unavailable`.
- Idempotency: one `run` is one substrate operation with its own `op`; the adapter does not retry a
  run whose start answered unknown, it reports `Unavailable`.
- Observed: from the daemon's event history, never from the start's answer
  (`confine.rs:168-174`, `:243-297`); the workspace is destroyed and the exec retired at the end
  (`:398-407`).
- No network: every exec runs under `--unshare-net` unless an aperture is named
  (`confine.rs:7-8`; `substrate@0.29.0:README.md:437`).
- Serves: `node.StartProcess` of a snippet instance on the skeleton path
  (`story:skeleton-node-run-assignment`), `nodes.OpenExecSession`, command checks of
  `workloads.RecordCheckResult`, runner jobs (`pipelines.StartRunnerJob` `ess/6-delivery/pipelines.yaml:1013`).

### N2 `node-facts`: what the node observes about itself

- Operations: `observe() -> NodeFacts { capacity, isolation }` where isolation is
  `{levels, cgroups_delegated, egress_denied, kvm_present, gpu_passthrough}` and capacity the
  node's CPUs, as `weft-agent` reports them today (`product/cloud/crates/weft-agent/src/agent.rs:194-227`);
  later `usage(instance) -> Usage` from substrate's per-exec measurement (`usage: Option<ExecUsage>`,
  `cpu_time_us`, `memory_peak_bytes`, `io_read_bytes`, `substrate@0.29.0:crates/substrate-wire/src/lib.rs:2389`,
  `:2419-2440`).
- Refusals: `Unavailable`.
- Idempotency: reads only.
- Observed: `GET /v1/machine` (`Client::refresh_machine`, `agent.rs:213-217`); a fact the daemon
  withholds is absent, never `false`.
- Serves: `RecordNodeObservation`, `nodes.NodeObserved`, `nodes.UsageReported`; the scheduler's
  isolation guard reads the levels (`ess/5-runtime/nodes.yaml:1070-1074`).

### N3 `workloads`: the long-lived processes of an instance

- Operations: `start(key, spec)`, `stop(key, grace)`, `restart(key)`, `pause(key)`, `resume(key)`,
  `checkpoint(key) -> snapshot ref`, `observe(key)`, `list()`. Key: `(assignment_id, process)`.
  Spec: root filesystem (from N6), `argv`, `env`, mounts (N5), endpoints (N4), limits, isolation
  level, restart policy `never`. weft decides restarts itself (R9: `node.MarkRestartDue`,
  `RestartProcess`, `ess/5-runtime/node.yaml:370-415`), so the port never restarts on its own.
- Output of `observe`: `{state: starting | running | paused | exited | stopped | unknown,
  last_exit, observed_at}`.
- Refusals: `IsolationNotServed { level }` (the node cannot give the level; the scheduler's guard
  should have kept the instance away), `ImageNotReady`, `NotServed` (for example `checkpoint`),
  `Invalid`, `Unavailable`.
- Idempotency: a second `start` for a key that runs answers its observation.
- Observed: `observe`; substrate's workload status is `{desired, observed: {state, probed_at},
  restarts, last_exit}`, and "A driver that cannot currently answer reports `observed: null`"
  (`substrate@0.29.0:docs/design/01-contract.md:216-217`).
- Serves: `node.StartProcess`, `RestartProcess`, `StopProcess`, `StopAssignmentProcesses`;
  `nodes.Stop`, `Pause`, `Resume`, `Snapshot`; `observability.OpenLogStream`.
- Why not `exec`: substrate execs have "no restart policy — that is what workloads are for"
  (`substrate@0.29.0:docs/design/01-contract.md:193-194`), and every exec has a timeout
  (`confine.rs:41-49`).

### N4 `endpoints`: traffic into, out of and beside an instance

- Operations: `expose(key, port, exposure: loopback | node) -> observed address`, `withdraw(key,
  port)`, `tunnel(key, port)` (the exec channel's port forward, `protocols/exec-channel.yaml:12-13`),
  `local_endpoint(key) -> socket inside the instance` with the caller's namespace identity
  (`node.OpenLocalEndpointSession`, `ess/5-runtime/node.yaml:568-571`), `allow_egress(key,
  destination)` (`claims` egress allow list, `ess/5-runtime/claims.yaml:389-391`).
- Refusals: `ExposureRefused` (LAN exposure not allowed), `PortInUse`, `UnknownInstance`,
  `EgressNotAllowed`, `NotServed`, `Unavailable`.
- Idempotency: keyed by `(key, port)` and `(key, destination)`.
- Observed: the answer carries "the observed address, which may differ from the requested one"
  (`substrate@0.29.0:docs/design/01-contract.md:248-250`).
- Serves: the edge's backends (`edge.RecordBackendReady`), links between components
  (`formations.AddLink`), HTTP and TCP checks, the local endpoint for identity documents and
  secrets (`node.RequestIdentityDocument` `ess/5-runtime/node.yaml:625`, `RequestSecret` `:655`), port
  forward.

### N5 `volumes`: node-local storage mounted into an instance

- Operations: `create(volume_id, size)`, `attach(volume_id, key, path, read_only)`,
  `detach(volume_id, key)`, `snapshot(volume_id) -> snapshot ref`, `delete(volume_id)`,
  `observe(volume_id)`.
- Refusals: `InUse` (delete while attached; substrate: "deletion is refused (`conflict`) while
  mounted", `substrate@0.29.0:docs/design/01-contract.md:247`), `NoSpace`, `NotServed`, `Unavailable`.
- Idempotency: keyed by `volume_id`.
- Observed: `observe`.
- Serves: `resources.CreateVolume` (node-local), `SnapshotVolume`, `nodes.Attach`, `Detach`,
  `pipelines.CreateBuildCache`.

### N6 `images`: artifact to root filesystem

- Operations: `prepare(digest, kind) -> root ref` (bytes from C5 `objects`, digest checked on the
  node), `release(root ref)`, `observe(digest)`.
- Kinds: the package kinds of `weft.artifacts.ArtifactKind`
  (`ess/3-resources/artifacts.yaml:14-30`); an adapter names the kinds it serves.
- Refusals: `DigestMismatch` (recorded as `workloads.RecordArtifactRejected`), `KindNotServed`,
  `NotFound`, `Unavailable`.
- Idempotency: keyed by digest; images "are recorded by observed digest even when given as a tag"
  (`substrate@0.29.0:docs/design/01-contract.md:213`).
- Serves: `workloads.RecordArtifactVerified` (`ess/4-applications/workloads.yaml:2483`), "verified
  by the node before start" (`docs/domain-sketch.md:222-223`).

### N7 `devices`: accelerators on a node

- Operations: `list()`, `wipe(device)`, `slice(device, n)`, `observe(device)`.
- Refusals: `DeviceBusy`, `NotServed`, `Unavailable`.
- Serves: the `weft.inventory` Report commands (`ess/1-infrastructure/inventory.yaml:311-472`); "a GPU
  is wiped between tenants" (`docs/domain-sketch.md:466-467`).

## 6. Cloud-layer ports

### C1 `machines`: a node for a node pool

- Operations:
  - `make(MachineRequest) -> Machine`. Input: `node_id`, the pool (`node_pool_id`, zone,
    `machine_type`, `image`, `ess/1-infrastructure/infrastructure.yaml:142-149`), the join token, the
    control host's URL. Output: the observed machine `{node_id, state, detail}`.
  - `remove(node_id) -> Removed | Absent`.
  - `observe(node_id) -> Starting | Running | Gone | Unknown`; `list() -> [(node_id, state)]`.
- Refusals: `MachineTypeNotServed`, `ImageNotServed`, `ZoneNotServed`, `CapacityExhausted` (the
  place has no room; local: a bound on machines per host), `NotServed`, `Unavailable`.
- Idempotency: keyed by `node_id`. A `make` for a node whose machine runs starts none and answers
  that machine. `remove` of an absent machine answers `Absent`.
- Observed: `observe` asks the place every time (local: the process lives and its command line
  names the node, the check `tools/test-setup.d/20-substrate.sh:56-61` uses for the daemon; aws:
  the instance description). The join itself is observed through the specification: the node's
  own `JoinNode` and `RecordNodeObservation` make it `Ready` (`ess/1-infrastructure/infrastructure.yaml:246-255`).
  weft keeps no record that a machine was made; it asks.
- Steps (weft-controllers, actor ControlPlane):
  - `pool-scale`: an `Active` pool whose `desired_size` exceeds its nodes that are not `Gone` gets
    `ProvisionPoolNode`. The count against a field of another row is not expressible in ESS
    (`ess/1-infrastructure/infrastructure.yaml:987-988`, `docs/ess-gaps.md:18`), so the step counts in code
    from the views `NodePools` and `Nodes` (`ess/1-infrastructure/infrastructure.yaml:1345`, `:1398`).
  - `pool-machines`: a pool node in `Provisioning` whose machine `observe` answers `Gone` gets
    `IssueJoinToken` (a fresh token; the old one is void, `:1039-1040`), then `make`. One path for a
    node the step provisioned, a node the operator provisioned, and a control host that restarted
    between `ProvisionPoolNode` and `make` (the one-time token was lost with it).
  - `gone-machines`: a node `Gone` whose machine is not gone gets `remove`; a machine of a node id
    the store does not hold gets `remove`; a pool node `Ready` whose machine is `Gone` is drained
    and removed (`DrainNode`, `RemoveNode`, granted to ControlPlane at
    `ess/9-actors/actors.yaml:1034-1036`).
  - Only clouds whose placement the control host serves are stepped (§ 9, S1).
- Serves: `CreateNodePool`, `RequestNodePoolScaleUp`/`Down`, `ProvisionPoolNode`,
  `IssueJoinToken`, `DrainNode`, `RemoveNode`, `DeleteNodePool`, `scheduling.RequestScaleUp`, and
  the tenant-cluster pools made by binding `cluster-create-node-pool` (`ess/components.yaml:1911-1931`).

### C2 `edge`: a public address in front of weft-edge

- Operations: `publish(listener {hosts, port}) -> observed public address`, `withdraw(listener)`,
  `observe(listener)`. Routing to ready backends stays in weft-edge (`weft.edge.RouteBackend`,
  `ess/5-runtime/edge.yaml:24-33`); the port only gives weft-edge an address the public reaches.
- Refusals: `PortInUse`, `AddressExhausted`, `NotServed`, `Unavailable`.
- Idempotency: keyed by listener (host set and port).
- Serves: `formations.AddEndpoint`, `RemoveEndpoint`, `AddRoute`, `RemoveRoute`; "Data plane:
  traffic to endpoints: the edge gateway" (`docs/domain-sketch.md:69-70`).

### C3 `dns`: names and proof of control

- Operations: `challenge(host, account) -> record to publish`, `verify(host, account) -> Proved |
  NotProved`, `upsert(host, address)`, `remove(host)`, `observe(host)` (what a resolver answers).
- Refusals: `NotProved`, `ZoneNotServed`, `NotServed`, `Unavailable`.
- Idempotency: keyed by host.
- Serves: `formations.AddEndpoint/host-not-verified` (`ess/4-applications/formations.yaml:1524-1527`; "how
  an account proves it controls a host (DNS record, delegated zone)" is OPEN at `:1504-1505`),
  `claims.DeclareClaim/domain-not-verified` (`ess/5-runtime/claims.yaml:392-394`), the DNS records of
  C2's addresses.

### C4 `certificates`: TLS for public hosts

- Operations: `issue(certificate_id, host)`, `observe(certificate_id) -> Pending | Issued
  {not_after} | Failed {reason}`, `retire(certificate_id)`.
- Refusals: `HostNotProved` (C3), `IssuerRefused { reason }`, `NotServed`, `Unavailable`.
- Idempotency: keyed by `certificate_id`.
- Observed: `observe`; "The end of validity is the issuer's, not the edge's clock"
  (`ess/5-runtime/edge.yaml:595-596`).
- Serves: `edge.RequestCertificate`, `RetryCertificate`, `RetireEndpointCertificates`; recorded by
  `RecordCertificateIssued` and `RecordCertificateFailed`. The issuer is OPEN
  (`ess/5-runtime/edge.yaml:153-154`).

### C5 `objects`: buckets and the artifact store

- Operations: `create_bucket(bucket_id)`, `delete_bucket(bucket_id)` (refused while not empty),
  `is_empty(bucket_id)`, `put(bucket_id, key, bytes)`, `get(bucket_id, key)`, `list(bucket_id,
  prefix)`. The artifact store is a bucket of weft's own, keyed by digest.
- Refusals: `NotEmpty`, `NameTaken`, `NotFound`, `QuotaExceeded`, `NotServed`, `Unavailable`.
- Idempotency: keyed by `bucket_id` and `(bucket_id, key)`; an artifact `put` of an existing digest
  is a no-op.
- Serves: `resources.CreateBucket`, `DeleteBucket/not-empty`, `artifacts.RegisterArtifact`,
  `workloads.RecordArtifactVerified` (the node fetches the bytes), `observability.ConfigureBucketSink`,
  `catalog.BackupServiceInstance`. Lowering: "buckets to S3" (`docs/domain-sketch.md:715`).

### C6 `secret-store`: secret values and signing keys

- Operations: `probe(backend)`, `put(value) -> value_ref` (written once), `get(value_ref)` (only for
  a delivery), `disable(value_ref)`; keys: `create_key() -> key_ref`, `sign(key_ref, bytes)`,
  `public_key(key_ref)`.
- Backends: `SecretBackendKind` `ClusterStore | ExternalVault` (`ess/5-runtime/secrets.yaml:72-87`). The
  model holds references only (`docs/domain-sketch.md:421-422`); "a value is written once through
  the API and never returned by it" (`:424`); a value never appears in logs or events (`:427`).
- Refusals: `BackendUnavailable` (the `external:` branches `ess/5-runtime/secrets.yaml:408-411`,
  `:551-554`, `:882-885`), `ValueDisabled`, `NotServed`.
- Idempotency: `put` keyed by `(secret_id, version_id)`; `create_key` by the trust domain.
- Serves: `RegisterSecretBackend`, `AddSecretVersion`, `DeliverSecret` (pattern B),
  `identity.CreateTrustDomain`, `RotateSigningKey`, `IssueIdentityDocument` (key custody,
  `ess/5-runtime/identity.yaml:482-483`).

### C7 `zone-volumes`: storage that outlives a node

- Operations: as N5, plus `attach_to_node(volume_id, node_id)`, `detach_from_node`, and
  `create_filesystem`, `delete_filesystem` for shared filesystems.
- Refusals: `ZoneMismatch`, `InUse`, `NotServed`, `Unavailable`.
- Serves: `resources.CreateVolume` (zone scope), `RestoreVolume`, `SnapshotVolume`,
  `CreateFilesystem`, `DeleteFilesystem`. "what provisions a zone volume" is OPEN
  (`ess/3-resources/resources.yaml:40-42`). Lowering: "volumes to EBS" (`docs/domain-sketch.md:715`),
  "volume -> PVC" (`:717`).

### C8 `managed-services`: catalog offerings

- Operations: `provision(service_instance_id, kind, plan limits, engine)`, `change_plan`,
  `move_engine`, `deprovision`, `backup -> backup ref`, `restore(backup ref)`, `observe -> Ready |
  Degraded | Gone`.
- Refusals: `KindNotServed`, `PlanNotServed`, `QuotaExceeded`, `NotServed`, `Unavailable`.
- Serves: `ProvisionServiceInstance` .. `ReportRestoreFailed` (`ess/7-services/catalog.yaml:1095-1754`).
- Default adapter on every placement: weft itself. "weft runs on weft: a service instance is
  translated into components of a formation owned by the system, on the same nodes, with the same
  isolation and metering" (`docs/domain-sketch.md:548-550`); the control plane reports "the instance's
  system formation running" (`ess/7-services/catalog.yaml:1305-1306`). That adapter uses the node layer
  through the scheduler and is weft's own translator code, not a platform adapter. A vendor adapter
  (a managed database of the cloud vendor) is an option of the `aws` placement only.

### C9 `network`: the private network of a formation

- Operations: `join(formation_id, key, internal_name)`, `allow(link)`, `revoke(link)`,
  `resolve(internal_name)`, `egress_policy(cloud) -> allowed destinations`.
- Refusals: `LinkDenied`, `EgressNotAllowed`, `NotServed`, `Unavailable`.
- Serves: `formations.AddLink`, `RemoveLink` ("Components without a link cannot reach each other",
  `ess/4-applications/formations.yaml:256-257`), `InternalName` (`:37-39`), `Formation.internal_domain`
  (`:90-93`), `claims.DeclareClaim/egress-denied`, the network meters
  (`ess/8-economy/metering.yaml:52-54`). Lowering: "Link -> NetworkPolicy" (`docs/domain-sketch.md:716`).

## 7. Outside the ports

- **Outside services by name.** The identity provider's keys (`access.ExchangeFederatedToken/bad-signature`,
  `ess/2-tenancy/access.yaml:1565`), a forge installation (`pipelines.ConnectForge`,
  `ess/6-delivery/pipelines.yaml:660`), an outside service's token endpoint
  (`identity.ExchangeToken/outside-service-refused`, `ess/5-runtime/identity.yaml:1101-1103`), telemetry
  sinks and notification targets (`ess/5-runtime/observability.yaml:829`, `:1029`). They are the same
  service wherever weft runs; weft reaches them through connectors, not through a placement adapter.
- **`external:` branches over weft's own rows.** Many `external:` branches of `ess/` read rows of
  another domain that a guard may not read (for example `ess/3-resources/cluster.yaml:724-727`,
  `ess/4-applications/workloads.yaml:2242-2253`, `ess/5-runtime/scheduling.yaml:1156-1166`). The decider answers them
  from the store; no port.
- **Cryptographic checks** (signatures of artifacts, attestations, identity documents:
  `ess/3-resources/artifacts.yaml:226`, `:338`, `ess/5-runtime/secrets.yaml:880`): weft code over bytes it holds;
  the keys come from C6.
- **Mutual TLS of the node session** (`ess/5-runtime/nodes.yaml:768-771` and the eight branches like it):
  weft's node channel, a later story (`docs/implementation.md:22-24`).
- **How weft itself is installed**: the control host's replicas (`ScaleControlPlane`,
  `ess/1-infrastructure/infrastructure.yaml:599`), its store, its listener. The lowering of
  `docs/domain-sketch.md:711-718` decides it per placement; `local` is "one process is control plane
  and worker; single-user access" (`:713`), and the control host is one binary
  (`docs/implementation.md:18-20`).

## 8. Adapters per placement

`local`: one Linux host, `substrate-daemon`, no vendor account, loopback only. `hosts`: hosts the
operator brought, each with `substrate-daemon`. `aws`: vendor APIs through connectors. `kubernetes`:
substrate's Kubernetes namespace driver for the node layer, the cluster's API through connectors.
"Built" marks the adapters of the first build; every other cell is designed, not built.

| Port | local | hosts | aws | kubernetes |
|---|---|---|---|---|
| N1 `exec` | substrate host driver, built | substrate host driver | substrate host driver on each instance | substrate namespace driver; exec admitted only where its probe observed confinement (`substrate@0.29.0:README.md:5-8`) |
| N2 `node-facts` | substrate `GET /v1/machine`, built | same | same | same, namespace driver |
| N3 `workloads` | not served until substrate's workload family; a process that ends within exec bounds runs through N1 | as local | as local | not served until the namespace driver serves workloads |
| N4 `endpoints` | not served (no inbound path into an exec, § 10) | as local | as local | as local |
| N5 `volumes` | not served (§ 10) | as local | as local | not served |
| N6 `images` | kind `Snippet` only: the process runs on the root substrate gives every exec | as local | as local | as local |
| N7 `devices` | not served | not served | not served | not served |
| C1 `machines` | a `weft-agent` process on this host beside its `substrate-daemon`, built (§ 12.3) | not served: the operator brings the host and records it with `ProvisionNode` (`ess/1-infrastructure/infrastructure.yaml:930-932`) | an instance from a launch template per pool ("NodePools to EC2 launch templates", `docs/domain-sketch.md:715`), through connectors | a `weft-agent` pod per node of the pool through the Kubernetes API (connectors provider, `connectors@56969ef30:README.md:270`); the cluster's own autoscaler owns its machines |
| C2 `edge` | weft-edge on a loopback port | weft-edge on a host's public interface | a load balancer through connectors | a Service of type LoadBalancer or an Ingress |
| C3 `dns` | names under `.localhost` only, proved by their form | the operator's DNS through connectors, or not served | the vendor's DNS through connectors | not served (the cluster's DNS operator) |
| C4 `certificates` | a CA weft makes in the control host's state directory | ACME | the vendor's certificate service through connectors | a Certificate object of the cluster's issuer |
| C5 `objects` | a directory under the control host's state directory | a directory on the control host; nodes fetch over the node channel | the vendor's object store ("buckets to S3", `docs/domain-sketch.md:715`) | an object store through connectors |
| C6 `secret-store` | `ClusterStore`: an encrypted file in the control host's state directory | `ClusterStore` | `ClusterStore`, or `ExternalVault` = the vendor's secret service | "Secret -> Secret from the backend" (`docs/domain-sketch.md:718`) |
| C7 `zone-volumes` | the node's N5 (one host is one zone) | not served | "volumes to EBS" (`docs/domain-sketch.md:715`) | "volume -> PVC" (`docs/domain-sketch.md:717`) |
| C8 `managed-services` | weft on its own nodes | same | same, or the vendor's managed database | same |
| C9 `network` | not served (needs N4) | not served (needs N4 and a network between hosts) | the vendor's private network and its security groups | "Link -> NetworkPolicy" (`docs/domain-sketch.md:716`) |

## 9. How the specification declares the ports

### What ESS 0.99.2 offers

- **`external:`** is free text, "What outside the input decides this branch"
  (`ess@0.99.2:schemas/generated/ess.schema.json:2979`). Synthesis answers every branch through one
  context method, `fn external(&mut self, command: ExternalCommand<'_>, outcome: &'static str) ->
  bool;` (`ess@0.99.2:crates/generate/ess-synth/src/rust/behaviour.rs:472-477`), called with the command's
  input and the outcome name only (`:2023-2024`). The text reaches the plan's prose, "externally
  decided ({cause})" (`ess@0.99.2:crates/generate/ess-synth/src/plan.rs:963-966`), and nothing else. A
  conformance scenario forces a branch by command and outcome
  (`ess@0.99.2:crates/verify/ess-conformance/src/scenario.rs:2407-2424`).
- **Components** have the keys `owns`, `accepts`, `publishes`, `reached_by`, `cli`, `settings`,
  `refs`, and no other (`ess@0.99.2:schemas/generated/ess.schema.json:2330-2432`). A **setting** is "One
  configuration input a component declares", typed by any type the specification declares
  (`ess@0.99.2:schemas/generated/ess.schema.json:2277-2330`).
- **Topology `requires`** is an untyped `{kind: name}` pair with no operations
  (`ess@0.99.2:crates/specify/ess-domain/src/topology.rs:60-70`); synthesis defers every workload
  (`ess@0.99.2:crates/generate/ess-synth/src/plan.rs:1813-1830`).
- **Bindings** invoke one command of the same specification (`invoke.command`,
  `ess@0.99.2:schemas/generated/ess.schema.json:2843-2859`); a binding to another specification's command is
  refused (`ess@0.99.2:docs/design/composition-contract-roles.md:13-17`).
- **Events with an outside producer**: an event no outcome emits may come from outside; a binding
  reads it and synthesis names the channel as an obligation
  (`ess@0.99.2:crates/generate/ess-synth/src/plan.rs:1433-1457`).
- **Composition** (`ess-composition/4+`, roles `implements` and `gates`) links two specifications by
  command; it compares inputs only and does not reach synthesis or conformance of the dependent
  specification (`ess@0.99.2:docs/design/composition-contract-roles.md:28-56`, `:105-113`).
- **Realization** picks one implementation per component
  (`ess@0.99.2:website/docs/concepts/ess.md:49-54`).

So ESS can carry, today: the placement a component serves (a setting), each pattern-B refusal (an
`external:` branch, with the port named only in its text), and each pattern-A observation (a
Report or Record command). It cannot carry a port as a declared dependency with operations, which
port answers an `external:` branch, the link from a request to its observation, or an adapter per
placement.

### Spec changes proposed

- **S1. The placement a control host serves.**
  - `ess/components.yaml`, component `weft-control`: add
    `settings: [{name: placement, type: weft.infrastructure.Placement, required: true, summary: The
    placement whose adapters this control host runs.}]`.
  - `ess/1-infrastructure/infrastructure.yaml`, `CreateCloud` (`:510`): a new outcome before `created`:
    `name: placement-not-served`, `external: "port machines: the control host serves another
    placement than input.placement"`, `error: weft.infrastructure.PlacementNotServed`; and the error
    type `weft.infrastructure.PlacementNotServed` beside `SingleUserOutsideLocal` (`:319`).
  - The decider (`product/cloud/crates/weft-impl/src/context/decider.rs`) answers it from the control host's
    `--placement`.
- **S2. The ControlPlane makes pool nodes.** `ess/9-actors/actors.yaml`, ControlPlane `may:`
  (`:966-1079`): add `weft.infrastructure.ProvisionPoolNode`, `weft.infrastructure.IssueJoinToken`,
  and the views `weft.infrastructure.Clouds`, `NodePools`, `Nodes`. Today only the ClusterOperator
  may send the two commands (`ess/9-actors/actors.yaml:59-60`). `ess/components.yaml`, the level-step block
  (`:1104-1147`): add the three C1 steps of § 6 as lines
  `(machines) infrastructure.NodePools + infrastructure.Nodes -> infrastructure.ProvisionPoolNode -> ControlPlane`,
  `(machines) infrastructure.Nodes (pool nodes Provisioning, machine Gone) -> infrastructure.IssueJoinToken, port machines.make -> ControlPlane`,
  `(machines) infrastructure.Nodes (Gone, machine not Gone) -> port machines.remove -> ControlPlane`.
  Needed by the first build (§ 12.4).
- **S3. A machine the place refused.** `ess/1-infrastructure/infrastructure.yaml`: command
  `weft.infrastructure.RecordNodeMachineFailed {node_id, reason: String}` (ControlPlane), from
  `Provisioning` to `Gone` (transition `remove`, `:268-270`), emitting `NodeMachineFailed {node_id,
  reason}`; and a binding in `ess/components.yaml` from `NodeMachineFailed` to
  `scheduling.RecordScaleRefused` where a scale request is open. Without it a pool whose `make` is
  refused (`CapacityExhausted`) holds a `Provisioning` node until its token expires.
- **S4. Port names in `external:` texts.** Every pattern-B branch starts its text with
  `port <name>: `: `ess/5-runtime/secrets.yaml:410`, `:553`, `:884` (`secret-store`),
  `ess/3-resources/resources.yaml:829` (`objects`), `ess/4-applications/formations.yaml:1526` and
  `ess/5-runtime/claims.yaml:393` (`dns`), `ess/5-runtime/claims.yaml:390` (`network`),
  `ess/5-runtime/node.yaml:571` (`endpoints`). A check, `tools/check-ports.sh` in `task check`, fails on
  a `port <name>:` whose name is not one of the 16 of § 2. The plan of a synthesis then names the
  port beside each obligation (`ess@0.99.2:crates/generate/ess-synth/src/plan.rs:963-966`).
- **S5. A step block for every port.** `ess/components.yaml`, after the level-step block: a comment
  block "Platform ports" with one line per pattern-A step, `gap view -> port operation ->
  recording command -> actor`, for N3, N5, N6, C1, C4, C5, C7, C8. Where the recording command's
  actor is wrong for a cloud-layer step, the grant moves: `ReportBucketProvisioned` and
  `ReportFilesystemProvisioned` (a cloud resource; granted to NodeAgent at
  `ess/9-actors/actors.yaml:702-703`) are also granted to ControlPlane. `tools/check-ports.sh` checks that
  every view and command a line names exists in `ess/`.

Each change of `ess/` passes `ess specify validate --path ess --strict-requires` and is
acknowledged with `tools/check-spec-diff.sh --acknowledge` (`AGENTS.md` § Layout).

### ESS gaps (for the ESS maintainers; continue `docs/ess-gaps.md` at 26)

- **26. A component cannot declare a port.** Wanted: a component key, for example
  `uses: [{port, operations: [{name, input, output, refusals}]}]`, that validate checks (each
  operation's types exist), synthesis emits as a trait beside `Context`, and conformance answers
  with a double. Today component keys are closed
  (`ess@0.99.2:schemas/generated/ess.schema.json:2432`) and topology `requires` has no operations and is
  deferred by synthesis (`ess@0.99.2:crates/generate/ess-synth/src/plan.rs:1813-1830`).
- **27. An `external:` branch cannot name what answers it.** Wanted: `external: {port, operation,
  text}`, so the decider of each port is generated per port, and a conformance double answers by
  port. Today the generated call carries the command and the outcome name only
  (`ess@0.99.2:crates/generate/ess-synth/src/rust/behaviour.rs:2023-2024`); S4 is the workaround.
- **28. A requested effect is not linked to its observation.** Wanted: an outcome that `requests:
  {port, operation}` and a recording command that `answers:` it, so validate can name a request
  no command answers, and conformance can check that every request reaches an answer. Today the
  request (an event) and the observation (a Report command) are unrelated in the model; S5's
  comment block is the workaround.
- **29. No adapter per placement.** Wanted: a realization keyed by a component setting's value
  (here `placement`), naming per port the implementation for that value. Today a realization names
  one implementation per component (`ess@0.99.2:website/docs/concepts/ess.md:53`).
- Gap 4 (level and time steps, `docs/ess-gaps.md:13-15`) and gap 6 (a row-set count compares only
  with a literal, `docs/ess-gaps.md:18`) apply to every C1 step: `pool-scale` counts in code.

## 10. Substrate gaps

For the substrate maintainers. Ordered by weft's first need; each matched to a track of
`substrate@0.29.0:ROADMAP.md:24-33` or "not on its roadmap".

1. **A long-lived, supervised process from a root filesystem, on the host driver** (N3). First need:
   every instance that does not end within an exec's bounds: any `Service` replica, and the next
   epic's `ess-system` workloads run by Spindle (`docs/implementation.md:29-31`). The workload family is
   designed (`substrate@0.29.0:docs/design/01-contract.md:196-218`) and absent from the wire
   (`substrate@0.29.0:STATUS.md:272-273`). Track: Docker driver, "immutable image-backed workloads",
   proposed (`substrate@0.29.0:ROADMAP.md:30`); `story:docker-image-backed-workload-slice` proposed. A
   workload on the host driver without a Docker daemon: not on its roadmap.
2. **An inbound endpoint into a confined exec or workload** (N4): the edge's backends, HTTP and TCP
   checks, links. "Nothing today carries a connection into that namespace"
   (`substrate@0.29.0:.engineering/planning/story/inbound-endpoint-for-a-confined-exec.md:30`); that story
   and `story:confinement-profile-ingress-routes` are drafts. Not on its roadmap (no track).
3. **A per-instance local endpoint** (N4): a socket the node agent serves inside each exec's network
   namespace, from which the agent learns which instance called (`ess/5-runtime/node.yaml:568-571`). Not on
   its roadmap.
4. **A secret value the consumer gives one run at its start** (C6 delivery through N3): substrate's
   secret slots "resolve from daemon-config named secrets" (`substrate@0.29.0:docs/design/01-contract.md:214-215`);
   weft delivers per-instance values at start (`docs/domain-sketch.md:425`). Not on its roadmap.
5. **Images from an artifact without a Docker daemon** (N6): a root filesystem from a content-addressed
   archive, digest observed. Image family designed (`substrate@0.29.0:docs/design/01-contract.md:220-236`);
   track Docker driver, proposed (`substrate@0.29.0:ROADMAP.md:30`). Without Docker: not on its roadmap.
6. **A read-write volume mounted into an exec or workload** (N5). Designed
   (`substrate@0.29.0:docs/design/01-contract.md:238-246`); not on its roadmap (no track, no story).
7. **Egress destinations a consumer names per run, within bounds the operator sets** (N4
   `allow_egress`, the claim's allow list). Apertures are declared by the operator
   (`substrate@0.29.0:adr/0013-egress-apertures-are-declared-by-the-operator.md`) and a request names them
   only by name (`substrate@0.29.0:README.md:435-473`). Not on its roadmap.
8. **Freeze, thaw and checkpoint of a running process** (N3 `pause`, `resume`, `checkpoint`): the
   signals are `Int`, `Term`, `Kill` (`substrate@0.29.0:crates/substrate-wire/src/lib.rs:2333-2337`). Not on
   its roadmap.
9. **Network byte counters per exec** (N2 usage, meters `network-ingress-external-bytes`,
   `network-egress-external-bytes`, `ess/8-economy/metering.yaml:52-54`): usage has CPU, memory, processes
   and block I/O (`substrate@0.29.0:crates/substrate-wire/src/lib.rs:2419-2440`). Not on its roadmap.
10. **The `MicroVm` isolation level** (N3; "untrusted code default to `microvm`",
    `docs/domain-sketch.md:463-465`). Track: Firecracker driver, proposed, "current dev nodes provide no
    KVM surface" (`substrate@0.29.0:ROADMAP.md:31`).
11. **Exec on the Kubernetes namespace driver on an enforcing network plugin** (placement
    `kubernetes`, N1). Track: Kubernetes deployment and driver, active
    (`substrate@0.29.0:ROADMAP.md:29`).
12. **Accelerator inventory, wipe and passthrough** (N7). Not on its roadmap.

## 11. Connectors gaps

- No AWS provider: `story:parity-aws-reads` is a draft for reads only and "Needs an AWS Signature
  Version 4 auth profile" (`connectors@56969ef30:.engineering/planning/story/parity-aws-reads.md:5`, `:26`).
  Every `aws` adapter of § 8 waits for writes of compute, load balancers, DNS, certificates, object
  storage and secrets.
- A served connectors host runs writes only under standing grants
  (`connectors@56969ef30:docs/running-services.md:209-212`); a step that makes machines needs a grant per
  operation and instance.
- Whether the Kubernetes provider writes pods or StatefulSets is unverified: its operations are
  restricted by resource kind (`connectors@56969ef30:README.md:270`).

## 12. The first build

The cheapest, most local, least distributed build: three ports in code (N1 `exec`, N2
`node-facts`, C1 `machines`), each with its `local` adapter only. Every other port stays a design.
The epic's acceptance is met by these three: the control host and the node reach the place only
through ports; a test holds the boundary; with placement `local` a pool's node is made by the
machines port and joins on its own (`.engineering/planning/epic/platform-ports.md`, § Acceptance).

### 12.1 The crate

`product/cloud/crates/weft-platform` (workspace member by `Cargo.toml:3`):

- `src/lib.rs`: the shared refusals (§ 3.4).
- `src/node.rs`: traits `Exec` and `NodeFacts`, and their types (`Process`, `HostFile`, `Limits`,
  `Exited`, `ExecState`, `NodeFacts`).
- `src/cloud.rs`: trait `Machines` and its types (`MachineRequest`, `MachineState`).
- `src/local/`: the local adapters, the only place that names `b10x_substrate_sdk` or starts a node
  process: `substrate.rs` (`Exec`, `NodeFacts`), `machines.rs` (`Machines`).
- `src/bin/weft-probe.rs`: moved from `product/cloud/crates/weft-agent/src/bin/weft-probe.rs`, the confinement test's
  probe (`CARGO_BIN_EXE_weft-probe` is only set for the tests of the crate that builds it).
- `tests/main.rs`: modules `exec` (the confine tests, moved), `machines`, `boundary`.
- Dependencies: `b10x-substrate-sdk` (moved from `product/cloud/crates/weft-agent/Cargo.toml:29`, same rev),
  `tokio`, `serde_json`, `libc`. No dependency on `weft-impl` or on generated code, so it builds
  without `target/generated/`.

The traits are synchronous: `weft-agent` is a blocking program (`product/cloud/crates/weft-agent/src/agent.rs:107-164`), and
the substrate adapter keeps its own current-thread runtime, as `agent.rs:208-219` does today.

### 12.2 `confine.rs` behind the node-layer ports

- `product/cloud/crates/weft-agent/src/confine.rs` moves to `product/cloud/crates/weft-platform/src/local/substrate.rs`
  unchanged in behaviour; `Confine` implements `Exec`.
- `agent.rs:200-227` (`isolation_facts`, the second direct SDK call) and `agent.rs:194-198`
  (`capacity`) move into the same adapter as `NodeFacts`.
- `agent::run` takes `&dyn Exec` and `&dyn NodeFacts`; `main.rs` builds the local adapter from
  `--substrate <socket>`. The agent's arguments do not change (`agent.rs:56-57`).
- `weft-agent`'s manifest drops `b10x-substrate-sdk` and depends on `weft-platform`.
- `story:skeleton-node-run-assignment` (draft) starts its instance process "through `confine`"; after
  this move it calls the `Exec` port. Its acceptance does not change.

### 12.3 The local machines adapter

- Configured with the `weft-agent` binary, the control host's URL, a state directory
  (`<state file>.machines/`), and the substrate supply: `Socket(path)`, a daemon that already
  serves (the tests: `WEFT_SUBSTRATE_SOCKET` from `tools/test-setup.d/20-substrate.sh`, so no test
  starts a daemon), or `Start(daemon binary)`, which starts one `substrate-daemon` for the host the
  way the setup hook does (a transient user unit with `Delegate=yes`,
  `tools/test-setup.d/20-substrate.sh:144-174`) when none answers at `<state dir>/substrate.sock`.
- One daemon per host, shared by the host's nodes: a daemon governs "one tenant on one machine"
  (`substrate@0.29.0:docs/VISION.md:64-65`), and weft is that tenant.
- `make(request)`: when `observe(node_id)` answers `Running`, answer it; else start
  `weft-agent --control <url> --node-id <id> --join-token <token> --substrate <socket> --report
  <state dir>/<node id>/report.jsonl` (the arguments of `agent.rs:56-57`), stdin closed, stderr to
  `<state dir>/<node id>/agent.log`, with `PR_SET_PDEATHSIG` so it ends with the control host;
  write its PID to `<state dir>/<node id>/pid`. The token is passed on the command line only, never
  written to the state directory.
- `observe(node_id)`: `Running` when the PID lives and `/proc/<pid>/cmdline` holds `--node-id
  <id>`; `Gone` otherwise.
- `remove(node_id)`: `SIGTERM`, then `SIGKILL` after 2 s, reap, delete the directory; `Absent`
  when nothing runs.
- `weft-testkit`'s `NodeAgent::start_at` (`product/cloud/crates/weft-testkit/src/node_agent.rs:58-83`) starts its agent
  through this adapter, so the tests that start agents today keep their text.

### 12.4 A pool's node, made and joined

- `weft-bin` takes `--machines local --node-agent <path> --substrate <socket>`; without
  `--machines` it makes no machine. The steps act only on clouds whose placement is `Local`, read
  from the view `weft.infrastructure.Clouds`; S1 later replaces that constant with the control
  host's `--placement`. `Host::tick`
  (`product/cloud/crates/weft-bin/src/lib.rs:234`) runs the C1 steps after the existing steps, under the
  same lock.
- `weft-controllers` adds `src/machines.rs` with the steps `pool-scale`, `pool-machines` and
  `gone-machines` of § 6 C1, sending through `send` (`product/cloud/crates/weft-controllers/src/lib.rs:47-64`) as
  ControlPlane; it needs S2's grants.
- The walking skeleton's tests run the control host without `--machines`, so
  `story:skeleton-end-to-end` and `story:skeleton-conformance-suite` keep their path: the
  end-to-end test sends `ProvisionPoolNode` and starts its agent itself (through 12.3); the
  conformance target starts `weft-bin` without `--machines`, so no scenario makes a machine (one line
  in that story's context). `skeleton-end-to-end` creates its cloud with placement `Hosts`
  (`product/cloud/crates/weft-testkit/src/steps.rs:28-36`), which the local adapter does not step.
- The new test: a control host with `--machines local`, then `CreateCloud
  {placement: Local, access_mode: SingleUser, control_plane: {replicas: 1}}` (the only combination
  `CreateCloud` accepts for `Local`, `ess/1-infrastructure/infrastructure.yaml:519-539`), `DeclareZone`,
  `CreateNodePool {min_size: 1}`; it reads the pool's node `Ready` in `weft.infrastructure.Nodes`,
  and sends neither `ProvisionPoolNode` nor starts an agent.

### 12.5 The boundary test

`product/cloud/crates/weft-platform/tests/main.rs`, module `boundary`. It reads files only, no build:

- Allowed: `product/cloud/crates/weft-platform/src/local/` and `product/cloud/crates/weft-platform/tests/`.
- Fails when a `Cargo.toml` under `product/` other than `product/cloud/crates/weft-platform/Cargo.toml`
  names `b10x-substrate-sdk`.
- Fails when a `.rs` file under `product/` outside the allowed paths names `b10x_substrate_sdk`.
- Fails when a `.rs` file under `product/` outside the allowed paths calls `Command::new` and names
  `weft-agent`, `substrate-daemon`, `node_agent_binary` or `WEFT_AGENT`.
- Each rule has a fixture tree that holds one violation (`tests/fixtures/boundary/fail-*`) on which
  the check fails, and the repository tree, on which it passes.
- Not covered and allowed: `tools/test-setup.d/20-substrate.sh`, a test-run fixture outside
  `product/` that starts the one daemon of a test run (`~/beyond10x/AGENTS.md` § Tests stay fast).

### 12.6 Budget

Every test of the first build is timed before it is added; one over 5 s is named with its time.
The pool-node test starts one control host and one agent beside the setup hook's daemon; its bound
is 60 s, and it reads `Ready` within 10 s. No test starts a `substrate-daemon` of its own except
one test of `Start` in `ports-local-machines`, whose time is named in that story's report.

## 13. Stories of the epic

`depends_on` names stories of this list; "substrate N" names a gap of § 10.

### The first build

| Slug | Title | Acceptance | Scope | Depends on |
|---|---|---|---|---|
| `ports-node-exec-facts` | The `weft-platform` crate with the `exec` and `node-facts` ports; `confine` moves behind them | `product/cloud/crates/weft-platform/Cargo.toml` is the one manifest under `product/` that names `b10x-substrate-sdk`, and the moved confine tests and `weft-agent`'s join tests pass in `slot task check`. | `product/cloud/crates/weft-platform/` (new), `product/cloud/crates/weft-agent/` (`Cargo.toml`, `src/agent.rs`, `src/main.rs`, `src/lib.rs`, `src/confine.rs` removed, `src/bin/weft-probe.rs` moved, `tests/main.rs`), `Cargo.lock` | none |
| `ports-local-machines` | The `machines` port with its local adapter: a node is a `weft-agent` process beside this host's `substrate-daemon` | A second `make` for a running node starts no second process and answers the same machine; `observe` answers `Running` while the agent lives and `Gone` after `remove`; `weft-testkit`'s agents start through the adapter and the join tests pass unchanged. | `product/cloud/crates/weft-platform/src/cloud.rs`, `src/local/machines.rs`, `tests/main.rs` module `machines`; `product/cloud/crates/weft-testkit/src/node_agent.rs`, `Cargo.toml` | `ports-node-exec-facts` |
| `ports-boundary-test` | A test fails when code outside an adapter calls substrate's SDK or starts a node process | The module `boundary` passes on the repository tree and fails on each of its `fail-*` fixture trees (a manifest, an SDK path, a node start). | `product/cloud/crates/weft-platform/tests/main.rs` module `boundary`, `tests/fixtures/boundary/` | `ports-node-exec-facts`, `ports-local-machines` |
| `ports-local-pool-node` | With placement local, a pool's node is made by the `machines` port and joins on its own | A control host with `--machines local` reads the node of a pool created with `min_size: 1` `Ready` in `weft.infrastructure.Nodes` within 10 s, the test having sent no `ProvisionPoolNode` and started no agent; a control host without `--machines` makes no node for the same commands. | `ess/9-actors/actors.yaml` and `ess/components.yaml` (S2), `quality/spec-diff-acknowledgements.json`, `product/cloud/crates/weft-controllers/src/machines.rs`, `src/lib.rs`, `tests/main.rs`; `product/cloud/crates/weft-bin/src/main.rs`, `src/lib.rs`; `product/cloud/crates/weft-testkit/src/lib.rs`, `tests/main.rs` module `local_pool` | `ports-local-machines` |
| `ports-machines-remove` | A node that is gone loses its machine; a machine without a node is removed | After `RemoveNode` of a node the local adapter made, its agent process has ended by the next tick; an agent the adapter started for a node id the store does not hold ends on the next tick. | `product/cloud/crates/weft-controllers/src/machines.rs`, `tests/main.rs` | `ports-local-pool-node` |

### The specification

| Slug | Title | Acceptance | Scope | Depends on |
|---|---|---|---|---|
| `ports-spec-placement` | weft-control declares the placement it serves; `CreateCloud` refuses another (S1) | A control host started with `--placement local` answers `CreateCloud {placement: Hosts}` with `placement-not-served` and `CreateCloud {placement: Local, access_mode: SingleUser, replicas 1}` with `created`. | `ess/components.yaml`, `ess/1-infrastructure/infrastructure.yaml`, `quality/spec-diff-acknowledgements.json`, `product/cloud/crates/weft-impl/src/context/decider.rs`, `product/cloud/crates/weft-bin/src/main.rs`, `product/cloud/crates/weft-controllers/src/machines.rs` (the `Local` constant of § 12.4 becomes the served placement) | `ports-local-pool-node` |
| `ports-spec-machine-failed` | A machine the place refused is recorded and the node goes (S3) | When the local adapter answers `CapacityExhausted`, the step sends `RecordNodeMachineFailed`, the node reads `Gone`, and the pool step provisions no node past `max_size`. | `ess/1-infrastructure/infrastructure.yaml`, `ess/9-actors/actors.yaml`, `ess/components.yaml`, `product/cloud/crates/weft-impl/src/behaviours/`, `product/cloud/crates/weft-controllers/src/machines.rs` | `ports-machines-remove` |
| `ports-spec-port-names` | Every `external:` branch a port answers names its port, and a check holds it (S4) | `tools/check-ports.sh` passes on the tree and fails on a fixture whose `external:` names a port outside § 2. | the eight `external:` lines of S4, `tools/check-ports.sh`, `tools/check.d/`, a fixture directory | none |
| `ports-spec-step-block` | `ess/components.yaml` lists every port step with its gap view and recording command (S5) | `tools/check-ports.sh` fails when a step line names a view or command that `ess/` does not declare, and passes on the tree. | `ess/components.yaml`, `ess/9-actors/actors.yaml`, `tools/check-ports.sh` | `ports-spec-port-names` |
| `ports-ess-gaps` | ESS gaps 26-29 filed with the ESS maintainers | `docs/ess-gaps.md` lists gaps 26-29, each with the id of the story in ESS's planning store. | `docs/ess-gaps.md` | none |
| `ports-substrate-gaps` | The substrate gaps of § 10 handed to the substrate maintainers | Each gap of § 10 names a story id in substrate's planning store or the maintainers' recorded answer. | `docs/platform-ports.md` § 10 | none |

### Designed, not built

| Slug | Title | Acceptance | Scope | Depends on |
|---|---|---|---|---|
| `ports-node-workloads-local` | The `workloads` port on substrate's workload family | A `Service` instance whose process runs until stopped reads `Running`, and `nodes.Stop` ends it. | `product/cloud/crates/weft-platform/src/node.rs`, `src/local/substrate.rs`, `product/cloud/crates/weft-agent/` | `ports-node-exec-facts`, substrate 1 |
| `ports-node-endpoints-local` | The `endpoints` port: backends, checks and the local endpoint | An HTTP check of an instance's port passes through the port, and the edge forwards a request to it. | same crates | `ports-node-workloads-local`, substrate 2, 3 |
| `ports-node-images-local` | The `images` port: artifact bytes to a root filesystem | An `Image` artifact whose bytes do not match its digest is recorded `RecordArtifactRejected`. | same crates | `ports-cloud-objects-local`, substrate 5 |
| `ports-node-volumes-local` | The `volumes` port: node-local volumes | A volume written by one instance is read by the next instance it is attached to. | same crates | `ports-node-workloads-local`, substrate 6 |
| `ports-cloud-objects-local` | The `objects` port: buckets and the artifact store in a directory | `DeleteBucket` of a bucket that holds an object answers `not-empty`; an empty one is deleted. | `product/cloud/crates/weft-platform/src/cloud.rs`, `src/local/` | `ports-spec-step-block` |
| `ports-cloud-secret-store-local` | The `secret-store` port: `ClusterStore` and signing keys | A value added with `AddSecretVersion` is delivered once at start and appears in no log line. | same | `ports-spec-port-names` |
| `ports-cloud-edge-local` | The `edge` port: weft-edge on a loopback listener | A request to the listener with a route's host reaches a ready backend. | same, `product/cloud/crates/weft-bin/` | `ports-node-endpoints-local` |
| `ports-cloud-dns-certificates-local` | `dns` for `.localhost` names and `certificates` from a local CA | `AddEndpoint` of a `.localhost` host is accepted and another host refused `host-not-verified`; the endpoint's certificate is recorded issued. | same | `ports-cloud-edge-local` |
| `ports-hosts-adapters` | Adapters of the `hosts` placement | A host the operator recorded with `ProvisionNode` joins over the network with its token and runs an instance. | `product/cloud/crates/weft-platform/src/hosts/` | `ports-spec-placement` |
| `ports-aws-adapters` | Adapters of the `aws` placement through connectors | A pool's node is made as an instance through connectors and joins. | `product/cloud/crates/weft-platform/src/aws/` | `ports-spec-placement`, § 11 |
| `ports-kubernetes-adapters` | Adapters of the `kubernetes` placement | A pool's node is made as a `weft-agent` pod through connectors, joins, and runs an exec on the namespace driver. | `product/cloud/crates/weft-platform/src/kubernetes/` | `ports-spec-placement`, substrate 11, § 11 |
