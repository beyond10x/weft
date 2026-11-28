# weft: domain sketch (draft 3, 2026-11-26)

Product name: weft (the owner, 2026-11-26). Input for an ESS specification; no spec, no code yet.
Domains only: no existing implementation is assumed.

## What it is

- A compute cloud you deploy yourself: one control plane, N worker nodes.
- A user hands it a workload or a whole topology; it runs it confined.
- Workloads claim resources: GPUs, disks, network addresses, secrets.
- Multi-tenant: tenants, nested sub-accounts, quotas at every level.
- Security default: deny. No egress, no secrets, no peer traffic unless
  declared and allowed.

## Goals, in order

1. GPU cloud: run open models on our own GPUs; no Runpod.
2. Deploy a spec: `deploy system.yaml` (an ESS system) and a service runs.
3. Deploy an agent: `deploy agent.yaml` and a confined agent runs with its
   tools and secrets.
4. Goalpost: the Helm chart of a Kubernetes SaaS product runs on weft
   unchanged (`weft deploy chart/ --values prod.yaml`).
5. Later: train a model from a ModelSpec program, a corpus and a data
   pipeline.

## Structure

### Domains (one file each, one directory per layer under `ess/`, see `docs/layers.md`)

- `shared`: identifiers of every entity, shared value types.
- `accounts`: tenants, sub-accounts, principals, roles, quotas.
- `access`: CLI login, API tokens, management API authorization.
- `formations`: the deployable unit: components, links, border, internal
  names, config, environments.
- `workloads`: package, shape, claims of one component; instances,
  invocations, jobs, boxes, snapshots.
- `deployments`: deployment sources (weft, Compose, Helm, Kubernetes, ESS
  system, Loom agent), translation (moved to cloudspec: last section), releases, plans, simulation, drift.
- `inventory`: resource classes and devices (operator-owned).
- `resources`: volumes, filesystems, buckets, snapshots of an account.
- `artifacts`: content-addressed blobs an account owns.
- `claims`: claims of workloads on resources and their bindings.
- `secrets`: custody, versions, grants, delivery.
- `identity`: workload identity documents, trust, token exchange.
- `infrastructure` (was `cluster`): the cloud itself: placement, nodes, node pools, zones.
- `cluster`: tenant clusters an account creates and deploys formations into (design:tenant-clusters).
- `scheduling`: placing instances on nodes, preemption.
- `nodes`: the control plane's record of each node session, assignment
  and observation.
- `observability`: logs, metrics, alerts, traces (§ 16).
- `pipelines`: forge connections, CI runner pools and jobs, build caches, deploy gates (§ 17).
- `metering`: usage, limits, prices.
- `actors`: every actor and the commands it may run.
- `catalog`: managed services (queue, stream, cache, kv, sql, object
  store, search, vector, scheduler, model endpoint), their plans, engines,
  instances and bindings.

A domain refers to another domain's entity through an identifier in
`shared`, never by a relation, until the integration pass adds relations.

### Planes

- **Management plane**: the user API. Tenants, accounts, formations,
  secrets, deploys. Called by the CLI and by CI.
- **Control plane**: state store, scheduler, identity issuer (translators: cloudspec's).
  Decides; runs no workload.
- **Worker plane**: one node agent per node; starts, observes and stops
  instances. Holds no state the control plane does not know.
- **Data plane**: traffic to endpoints: the edge gateway, links between
  components, egress through the node's filter.

### Components (`ess/components.yaml`)

- `weft-api`: the user API (management plane).
- `weft-control`: scheduler, identity issuer, secret delivery (translators: cloudspec's).
- `weft-node`: the node agent (worker plane).
- `weft-edge`: the gateway in front of public endpoints (data plane).
- `weft` (CLI): a client of `weft-api`; holds no other power.

In the `local` placement all four server components run in one process.

### Surfaces (each its own ESS document)

- Domain model: `ess/` (`ess/37`).
- User API: `surfaces/http/weft-api.yaml` (`ess-http/1`), commands and
  views of the domains on HTTP.
- CLI: `surfaces/cli/weft.yaml` (`ess-cli/2`), the CLI presentation of the
  same commands and views.
- Node channel: `protocols/node-channel.yaml` (`ess-protospec/1`), the
  session between `weft-control` and `weft-node`.
- Exec channel: `protocols/exec-channel.yaml` (`ess-protospec/1`), a
  stream from the CLI through `weft-api` and `weft-node` into an instance:
  exec, attach, logs, port forward.

### Channels

- CLI -> `weft-api`: HTTP, authenticated per access mode.
- `weft-node` -> `weft-control`: the node dials out, mutual TLS, one
  long-lived session per node. Workers open no inbound port.
- `weft-api` -> `weft-node` for exec: carried inside the node's session,
  never a direct connection to the node.
- Instance -> `weft-node`: a local endpoint inside the instance for its
  identity document and its secrets; reachable from that instance only.

### Actors

Humans and clients:

- `ClusterOperator`: runs the cloud itself: clusters, node pools, nodes,
  price lists. Sees no tenant's secrets or workloads' data.
- `TenantOwner`: creates accounts, grants roles, sets quotas in a tenant.
- `AccountAdmin`: manages one account and its sub-accounts.
- `Deployer`: deploys formations, sets secrets, scales, execs.
- `Viewer`: reads state, logs and usage.
- `Automation`: an API token acting with one role in one account (CI).

System actors:

- `NodeAgent`: a joined node; reports observations, receives assignments.
- `Scheduler`: places instances, preempts.
- `Translator`: writes translations of deployment sources (moves to cloudspec: last section).
- `Instance`: a running workload, acting through its workload identity.
- `Anonymous`: a CLI before login (starts a device login only).
- `ManagementApi`: weft-api acting on its own: expiry of logins and
  tokens, audit records.
- `ReleaseController`: weft-control running releases: steps, marking
  live, recording drift.
- `ControlPlane`: weft-control acting on its own: issues and expires
  identity documents, decides secret delivery, expires leases.

Sections 1 to 12 below map to domains: 1 accounts, 2 access, 3 formations,
4 workloads, 5 deployments, 6 resources, 7 secrets, 8 identity, 9 cluster,
10 scheduling, 11 nodes, 12 metering, 13 catalog.

## 1. Accounts

- `Tenant`: top-level customer. Owns everything below it.
- `Account`: a sub-account inside a tenant or inside another account.
  Nested to any depth. Example: tenant `acme`, accounts `acme/research`,
  `acme/research/gpu-lab`.
- `Principal`: a user, a service account, or a workload identity.
- `Role` + `RoleBinding`: what a principal may do on an account and below
  (`viewer`, `deployer`, `admin`, `billing`).
- `Quota`: per account and resource class. A child's quota is at most its
  parent's remaining quota.
- Nothing crosses accounts unless an `Export` names it (an endpoint shared
  to another account).

## 2. Access (the user's side)

- The user drives everything with a CLI. The CLI is a client of the
  `ManagementAPI`; it holds no other power.
- `CliContext`: one cluster + one account, like a kube context. A user can
  hold several and switch.
- Login depends on the cluster's `AccessMode`:
  - `single-user`: local placement. One implicit admin. The API listens on
    a local socket readable only by the owner. No login.
  - `federated`: CLI login through the browser (device flow), tokens from
    an external identity provider. Like `fly auth login`.
- `ApiToken`: for CI and scripts; scoped to one account and a role; expires.
- Every management call is authorized against RoleBindings and audited.

## 3. Formation (the outer frame)

A formation is what a user deploys: a set of components that belong
together, their allowed links, and their border.

- `Formation`: named, inside one account. Owns a private network, a secret
  scope and a quota slice.
- `FormationRevision`: immutable snapshot of the formation's document.
- `Component`: one workload inside the formation (section 4).
- `Link`: an allowed call from one component to another (protocol, port).
  Components without a link cannot reach each other.
- `Border`: the components reachable from outside the formation, with
  their public `Endpoint`s and `Route`s (host and path to a component and
  port; what a Kubernetes Ingress says). Everything else is internal.
- `InternalName`: each component has a DNS name inside the formation's
  private network (`orders.shop.internal`); a Link is what lets a caller
  resolve and reach it.
- `Config`: named non-secret data (files, key-value pairs) of the
  formation, mounted into components as files or environment variables.
  What a Kubernetes ConfigMap says.
- `Environment`: a named target of the same formation (`dev`, `staging`,
  `prod`). Each has its own secrets, config values, quotas and scale
  settings.

Releases, plans, simulation and drift belong to `deployments` (section 5).

### Topology as data

- The formation document is the topology. Changing the border, adding a
  link or splitting a component is an edit and a new revision.

Example:

```yaml
formation: shop
components:
  gateway: { package: { kind: image, ref: ghcr.io/acme/gw@sha256:... }, shape: service }
  orders:  { package: { kind: ess-system, ref: ./orders/system.yaml }, shape: service }
  agent:   { package: { kind: agent, ref: ./support-agent.yaml }, shape: service }
  llm:     { package: { kind: model, ref: qwen3-32b }, shape: service,
             claims: { gpu: { model: nvidia-h100, count: 1 } } }
links:
  - gateway -> orders
  - gateway -> agent
  - agent -> orders
  - agent -> llm
border: [gateway]
```

## 4. Workload (one component)

Three independent axes:

1. **Package**: what the bytes are.
2. **Shape**: how long it lives and how it starts.
3. **Claims**: which resources it holds.

### Package: plain kinds

Run as they are. Every package is an `Artifact`: content-addressed, signed,
verified by the node before start.

- `snippet`: source text + language.
- `bundle`: a directory of code with an entry point and a dependency lock.
- `wasm`: a WebAssembly module or component.
- `image`: an OCI image by digest.
- `process`: a command + arguments in a given root filesystem.
- `machine`: a full VM image (kernel + root disk).
- `model`: model weights + a serving runtime, for inference calls.
- `browser`: a headless browser session driven by a protocol.
- `build`: a recipe whose output is another Artifact.

### Package: spec kinds

A spec is not run as it is. cloudspec's inbound translator turns it into a cloudspec
architecture of plain components, which weft runs (section 5, last section).

- `ess-system`: an ESS system. Becomes an API service with its state store,
  its commands and views, its conformance suite as health check.
- `agent`: an agent document. Becomes an agent runtime instance, its tools,
  its model access and its sandbox for tool effects.
- `training` (later): a ModelSpec program + corpus + data pipeline. Becomes
  a gang Job on GPUs that writes a model Artifact.
- `graph`: a pipeline or DAG of other workloads.

### Shape

- `Invocation`: one call, one result, a deadline.
- `Job`: runs to completion, retries, writes outputs.
  `parallelism` (array of N tasks), `gang` (all N start together or none).
- `Service`: long-running, N replicas, scales between bounds.
  `stateful`: each replica keeps a stable name and its own volume.
- `Box`: one virtual machine the user owns, like a Fly Machine. Package
  `image` or `machine`. Lifecycle: created, starting, started, stopping,
  stopped, suspended, destroyed. The user starts, stops, suspends, execs
  into it and resizes it. Its volume stays across stops. A `lease` gives
  one caller exclusive update rights for a time; `cordon` takes it out of
  its endpoint's traffic. (Fly Machines API, docs.fly.io/machines/api,
  read 2026-11-26.)
- `Session`: interactive and attached (terminal, notebook, agent sandbox).
- `Daemon`: one instance per matching node.
- `Schedule`: starts an Invocation or Job on a calendar rule.

Shared by all shapes: `Instance` (one running copy on one node),
`Trigger`, `Snapshot` (freeze and restore an instance), `WarmPool`.

### Inside one instance

- `Process`: an instance runs one or more processes that share its network
  and volumes. Roles: `main`, `init` (runs to completion before main),
  `sidecar` (runs beside main). What a Kubernetes Pod's containers say.
- `Check`: a health probe of a process: `startup`, `ready` (gets traffic),
  `alive` (restart when it fails). Kinds: HTTP, TCP, command.
- `RestartPolicy`: never, on failure, always; with a backoff.
- `ScaleRule`: replica bounds and the metric that moves them (CPU,
  requests, queue length). What a Kubernetes HorizontalPodAutoscaler says.
  `min: 0` is allowed for a Service that answers requests.

### Sleep and wake (the owner, 2026-11-26)

Example: a customer's ticketing system, written as an ESS spec and run by
Spindle, and an SSO service. At night nobody uses them: zero instances,
no compute cost. In the morning the first request wakes them; load then
scales them out to their upper bound.

- A Service with `min: 0` has a `SleepPolicy`: idle time before sleep
  (no request for N minutes), and the sleep kind:
  - `stop`: instances end; the next wake is a cold start.
  - `suspend`: instances are snapshotted and their memory freed; the next
    wake restores the snapshot (faster than a cold start).
- `Wake`: the edge (`weft-edge`) or a link holds the first request,
  asks for an instance, and forwards the request when one is ready. The
  caller waits up to a `WakeDeadline`; past it, the request fails with
  a named answer, never silence.
- Other wake sources: a queue message for a consumer, a schedule, an
  explicit `weft wake`.
- `ServiceState` of a scaled-to-zero Service: asleep, waking, awake,
  draining-to-sleep.
- While asleep, compute meters read zero. Storage (volumes, the state of
  an ESS system) is still metered. # OPEN: is sleeping storage free in
  some pricing model?
- Autoscaling out uses ScaleRule; a scale step that exceeds a quota or
  budget is refused by name and the service stays at its current size.
- `DisruptionBudget`: at most N replicas down at once for a drain or a
  rollout.

### Claims

See section 6. A component lists claims; the formation adds the implicit
ones (its private network, its workload identity).

## 5. Deployments

Goalpost (the owner, 2026-11-26): the Helm chart of a Kubernetes SaaS product
deploys and runs on weft unchanged.

### Deployment sources

A user deploys a cloudspec architecture. cloudspec, not weft, translates each source below
into one (last section); the mapping stays as written. Kinds:

- `weft`: weft's own formation document (section 3).
- `compose`: a Docker Compose file. Service -> component, network ->
  links, volumes -> volume claims, ports -> border.
- `helm`: a Helm chart + values. Rendered with its values, then read as
  `kubernetes`. Helm hooks become ordered release steps.
- `kubernetes`: a set of Kubernetes manifests.
  - Deployment -> Service shape; StatefulSet -> stateful Service;
    DaemonSet -> Daemon; Job -> Job; CronJob -> Schedule.
  - Pod containers -> Processes; probes -> Checks.
  - Service -> InternalName + Link targets; Ingress -> Border Routes.
  - ConfigMap -> Config; Secret -> Secret (values go to secret custody,
    never into the formation document).
  - PersistentVolumeClaim -> volume claim; resource limits and
    `nvidia.com/gpu` -> claims.
  - ServiceAccount -> workload identity; HorizontalPodAutoscaler ->
    ScaleRule; PodDisruptionBudget -> DisruptionBudget;
    NetworkPolicy -> Links.
  - Refused by name: privileged pods, host paths, host network, kernel
    capabilities beyond the isolation level. # OPEN: custom resources
    (CRDs) and operators, and workloads that call the Kubernetes API.
- `ess-system`: an ESS system, run by Spindle. Becomes an API service with its state
  store, its commands and views, its conformance suite as health check.
- `loom-agent`: a `loom.agent/1` document. Becomes an agent runtime
  instance, its tools, its model access and its sandbox for tool effects.
- `training` (later): a ModelSpec program + corpus + data pipeline.

A spec source can also be one component's package (an `ess-system`
component inside a `weft` formation); then its translation becomes a
sub-formation in that place.

### Translation (cloudspec's rules since 2026-11-28: last section)

- `Translator`: registered per source kind and version.
- `Translation`: source digest + translator version -> FormationRevision +
  `CoverageReport`.
- `CoverageReport`: every object of the source with its outcome: mapped
  (to which formation element), mapped with loss (what was dropped), or
  refused (the named reason). A deploy with a refused object stops.
- Deterministic: the same source and translator version give the same
  revision, byte for byte.
- A translator cannot grant more than the source declares: an agent that
  names no secret gets no secret grant.
- A new source kind arrives as a new translator, not as a change to the
  scheduler or the node agent.

### Releases

- `Release`: a FormationRevision applied to an Environment.
  Lifecycle: planned, applying, live, superseded, rolled-back, failed.
- `ReleaseStep`: ordered steps of a release (pre-install job, rollout,
  post-install job); a failed step stops the release.
- `RolloutStrategy`: replace, rolling (bounded by DisruptionBudget),
  blue-green.
- `Plan`: the diff between the live release and a new revision, plus a
  simulation, shown before apply.
- `Simulation`: a release checked without running it:
  - placement: does every instance fit the cluster's capacity and quota;
  - policy: does every link, egress and secret grant pass the account's
    policy;
  - reachability: which component can reach which, inside and outside.
  Result: pass, or a list of named violations.
- `Rollback`: a new release of an earlier revision.
- `Drift`: the difference between the live release and what the nodes
  report.

## 6. Resources

### One generic model

Every resource is a `ResourceClass` with five attributes. The scheduler
reads only these attributes.

- `unit`: count, quantity or attachment.
- `divisible`: whole units only, or slices.
- `sharing`: exclusive, shared read-only, shared read-write.
- `scope`: node-local or movable within a zone or cluster.
- `lifetime`: dies with the instance, or lives on its own.

`Claim` (class + amount + sharing) -> `Binding` (concrete units on a node).

### Classes

- Compute: `cpu`, `memory`, `gpu`, `gpu-slice`, `accelerator`,
  `interconnect`.
- Storage: `scratch`, `volume`, `filesystem`, `bucket`, `snapshot`, `cache`.
- Network: `endpoint`, `domain`, `private-network`, `public-address`,
  `egress`, `bandwidth`.
- Security: `secret`, `workload-identity`, `key`.
- Limits (consumed): `deadline`, `budget`, `concurrency`, `rate`.

## 7. Secrets

- `Secret`: a named value in an account, formation or environment scope.
- `SecretVersion`: immutable; a rotation adds a version. A release pins
  versions or follows `latest`.
- `SecretGrant`: which component (by workload identity) may receive which
  secret, as a file or an environment variable.
- `SecretBackend`: where values live: the cluster's own encrypted store, or
  an external vault. Cluster state holds references only.
- Rules:
  - a value is written once through the API and never returned by it;
  - a value reaches an instance only at start, only through a grant;
  - every delivery is an audit event (secret, version, instance, time);
  - a value never appears in logs, plans, simulations or events.

## 8. Workload identity

- Every instance gets an identity at start:
  `tenant/account/formation/environment/component/instance`.
- `IdentityDocument`: short-lived, signed by the cluster, renewed by the node
  agent before it expires. No long-lived key in the instance.
- Uses:
  - calls between components: mutual TLS with identity certificates; a
    Link is checked against both identities;
  - calls to outside services: exchange the identity for a scoped token of
    that service;
  - secret delivery: a SecretGrant names identities, not instances.
- Standards to check: SPIFFE (identity format and SVIDs), IETF WIMSE
  working group drafts, OAuth token exchange (RFC 8693, from memory, not
  checked).

## 9. Cluster

- `Cluster`: name, placement, access mode, policy.
- `Placement`: `local`, `hosts` (one VM, one EC2, many EC2), `kubernetes`.
- `ControlPlane`: management API, scheduler, state store. 1 or 3 replicas.
- `Node`: role `control`, `worker` or both. Lifecycle: provisioning,
  joining, ready, cordoned, draining, gone. Reports `Capacity` and
  observed `IsolationFacts`.
- `NodePool`: template for nodes (machine type, image, min/max size).
- `Zone`: failure domain.

## 10. Scheduling

- Input: instance, claims, minimum isolation, formation's zone rules.
- Output: `PlacementDecision` or `Unschedulable` with the reason.
- Gang: all tasks of a gang bound, or none.
- `Priority` and `Preemption` with a grace period and a reason.
- Scale up by asking a NodePool for a node; drain idle nodes.
- `IsolationLevel`, strongest first: `microvm`, `user-kernel`, `container`,
  `process-sandbox`. A workload's minimum is never weakened. Spec kinds and
  untrusted code default to `microvm`.
- Instances of different tenants never share a kernel; a GPU is wiped
  between tenants.

## 11. Node control

- Each worker runs a `NodeAgent`; it dials out to the control plane
  (mutual TLS). Workers need no inbound port.
- Control plane to agent: `Assign`, `Stop`, `Pause`, `Snapshot`, `Drain`,
  `Attach`, `Detach`, `IssueIdentity`, `DeliverSecret`.
- Agent to control plane: `Heartbeat`, `InstanceObserved`, `NodeObserved`,
  `UsageReported`.
- The agent reports what it observed, never what it was told.
- An `Assignment` has a lease; a lost node's instances are rescheduled
  after the lease ends.

## 12. Metering

- `UsageRecord`: account, formation, instance, resource class, amount.
- Usage rolls up the account tree; quotas are checked at every level.
- `PriceList`: price per resource class per unit.

### Meters (the owner, 2026-11-26: "track the use of I/O, disk I/O, network in
and out, CPU use")

The node agent measures every instance; the control plane aggregates.

- `cpu-seconds`, `memory-gib-seconds`, `gpu-seconds` (per GPU model),
  `gpu-slice-seconds`.
- `disk-read-bytes`, `disk-write-bytes`, `disk-read-ops`, `disk-write-ops`.
- `network-ingress-bytes`, `network-egress-bytes`, each split into
  internal (inside the cluster) and external.
- `storage-gib-hours` per volume, filesystem and object store.
- `requests` per endpoint, `invocations` per function.
- `offering-units` per managed service (messages, operations).

### Pricing models

- `pay-per-use`: a rate per meter unit.
- `reserved`: committed capacity per period at a fixed price; use above it
  is charged per use.
- `subscription`: a flat price per period for an offering or product plan.
- `free-tier`: an allowance per meter and period before any charge.
- `spot`: preemptible capacity at a lower rate; the scheduler may evict it
  first.
- A `PricingAssignment` binds an account (or a formation, product or
  offering plan) to one pricing model; children inherit.
- `Statement`: charges per account and billing period, from usage records
  and the pricing in force. Payment collection is out of scope.

## 13. Catalog (managed services)

Two levels of use (the owner, 2026-11-26): build and deploy your own images, or
take a ready service from weft by name. "We are building a cloud, from
the specs."

- `Offering`: a kind of managed service weft provides, with a simplified
  API of its own:
  - `queue`: messages with acknowledgement, retry, dead letter.
  - `stream`: an ordered, replayable event log with consumers.
  - `cache`: key-value with expiry, no durability promise.
  - `kv`: durable key-value with versions.
  - `sql`: a relational database.
  - `object-store`: buckets of objects (moves `bucket` out of resources).
  - `search`, `vector`: text and vector indexes.
  - `scheduler`: timed delivery of messages.
  - `model-endpoint`: an inference endpoint of a model package.
  - `sso`: single sign-on a customer configures for its own users
    (identity provider connections, applications, user directory).
- `OfferingPlan`: a size tier of an offering (limits, durability,
  replicas, price).
- `Engine`: the implementation behind an offering, versioned (for example
  NATS JetStream behind `queue` and `stream`). Internal: the user sees the
  offering's API, never the engine's. An engine can be swapped without a
  change of the offering's API.
- `ServiceInstance`: one provisioned offering in an account or a
  formation. Lifecycle: provisioning, ready, degraded, updating,
  deprovisioning, gone.
- `ServiceBinding`: a component's access to a service instance with a
  mode (publish, consume, read, write, admin). Authenticated by the
  component's workload identity; no static credentials. Links to service
  instances follow the same default deny as links between components.
- `Backup` and `Restore` for durable offerings.
- weft runs on weft: a service instance is translated into components of a
  formation owned by the system, on the same nodes, with the same
  isolation and metering.
- Each offering's API is its own ESS system (future
  `offerings/<kind>/ess/`); SDKs are generated from it.

### Products (ready-made applications)

- `Product`: a packaged application a customer enables in an account,
  published as a versioned formation template. Example: the beyond10x
  `slot` build server (workers with many CPUs, a cache volume, an
  endpoint for build clients).
- Difference to an offering: weft operates an offering behind its own
  API; a product runs as the customer's own formation, which the customer
  can see, scale and pay for per use.
- `ProductVersion`: immutable; names its deployment source, its
  parameters (with a schema), the offerings and resource classes it needs.
- `Installation`: a product version enabled in an account with parameter
  values. It creates a formation through `deployments`; upgrade moves it
  to a newer version; uninstall removes the formation.
- `Publisher`: who publishes a product (the cluster operator first;
  third parties later).

## 14. Reconciliation (the owner, 2026-11-26)

The owner: "similar to Kubernetes and the reconciliation … expected end state … current state … based
on the inputs, context, state, some actions will be taken by the system to reach the new state."

- Desired state is what a user or a release wrote: a live `Release`, a `FormationRevision`, a
  `ScaleSetting`, a `Claim`, an `Export`, a `SleepPolicy`.
- Observed state is what a node agent reported: `InstanceObservation`, `NodeObservation`,
  `UsageReport`, check results. The agent reports what it saw, never what it was told (§ 11).
- A controller is a system actor (`ReleaseController`, `Scheduler`, `ControlPlane`, `NodeAgent`)
  that reacts to a change of either side with a command that moves the observed state toward the
  desired one.
- In the specification each reaction is an ESS binding in `ess/components.yaml`: an event invokes
  a command, with a `mapping`, a `delivery` and an `on_failure` (`retry`, `drop`, or `escalate`
  with the event that records it). A reaction that can fail silently is not admitted.
- First reconciliation loops:
  - R1 rollout step started -> the workload's desired replica count set; the Scheduler fills the gap (a release goes live only after its rollout steps saw ready instances, § 15).
  - R2 instance exited or failed under a restart policy -> a new instance, with backoff.
  - R3 node session lost, lease ended -> its instances evicted and rescheduled.
  - R4 placement bound -> assignment sent to the node.
  - R5 claim released -> its bindings released, devices wiped.
  - R6 account closed -> its exports revoked, tokens expired, instances stopped.
  - R7 drift recorded -> the release step re-applied or the drift escalated.
  - R8 wake requested -> an instance scheduled; deadline missed -> named failure.
  - R9 alive check failed past its threshold -> process restart (node-local, recorded).
- Three forms of a loop step (integrator, 2026-11-26, from the first bindings pass):
  - Edge: one event -> one command, an ESS binding. The event carries every field the command's
    input needs; a domain adds the field rather than the binding guessing it.
  - Level: a "gap view" lists the rows where desired and observed differ (a field compare on one
    row, for example `running_replicas < replicas`; ESS counts rows only against a literal, so the
    counter is stored and kept by the commands that create and end instances). The controller
    actor reads the view and runs one command per row. This is how a fan-out (one release, N
    instances) is written, and how a missed edge is repaired.
  - Time: a step started by time (lease end, wake deadline, backoff, token expiry) is a command of
    a system actor with a guard on `now` (`lease_until < now`). ESS has no timer; the clock is the
    actor's, the rule is the guard's.
  - Many rows at once (R6: account closed): one command whose effect moves every matching row
    (`affects` with a filter), invoked by one binding.
- The node agent's `Record*` call is the one source of an observation; no binding copies an
  observation into a second record.
- System actors stay ordinary actors (no `system: true`): ESS rejects a binding that invokes the
  command of a system actor.
- Components: ESS lets only the component that owns a domain accept its commands, and refuses a
  component that owns no domain. `ess/components.yaml` has four: `weft-api`, `weft-control`,
  `weft-node` (domain `weft.node`: local processes, R9 restart, the local endpoint) and `weft-edge`
  (domain `weft.edge`: route backends, held wake requests, certificates), design:components
  (2026-11-26). `weft-api` forwards the user commands of control-owned domains to `weft-control`
  (D1, the owner, 2026-11-26); the CLI reaches both through one URL.
- Convergence is a checked property: from any reachable state with no new input, every loop ends
  with observed equal to desired or with a named refusal (conformance and protocol exploration).

## 15. Health

- `Check` per process (§ 4): `startup`, `ready`, `alive`; kinds HTTP, TCP, command, gRPC; period,
  timeout, initial delay, failure and success thresholds.
- Instance health, separate from its run state: `unknown` -> `starting` -> `ready` / `not-ready`;
  `alive` failing past its threshold restarts the process.
- Readiness gates traffic: an edge route, a link and a service binding forward only to ready
  instances. A rollout step waits for the new instances to be ready before it ends the old ones.
- Restart backoff: exponential with a cap; past N restarts in a window the instance is
  `backing-off` (what Kubernetes calls CrashLoopBackOff) and the event is named.
- A node's own health (`NodeObservation`, heartbeat) is separate: missing heartbeats end the lease
  (R3).

## 16. Observability (logs, metrics, traces)

New domain `observability`, layer 5 (runtime). Kept apart from metering: billing reads
`UsageReport`; telemetry is operational data. The volume of telemetry ingested and stored may be
metered.

- Logs:
  - `LogStream`: stdout and stderr of one process of one instance, plus weft's own events about the
    instance (scheduled, started, check failed, restarted, evicted).
  - Retention per account, bounded by the plan.
  - `TelemetrySink`: forward to an outside receiver (OTLP, syslog, a bucket of the account).
  - `weft logs` reads a stream live through the exec channel's `Logs` kind and the stored tail.
- Metrics:
  - System metrics per instance and node: CPU, memory, disk I/O, network in and out, restarts,
    check results; at the edge and links: requests, errors, latency.
  - Custom metrics: an instance pushes OTLP to the node-local endpoint, authenticated by its
    workload identity.
  - `MetricSeries` (name, labels, owner account). `ScaleRule` and `SleepPolicy` (layer 4) name a series by metric name and labels, a value, never a reference upward.
  - `AlertRule`: a condition over a series and a window -> `Alert` firing / resolved -> a
    `NotificationTarget` (webhook, email, a queue of the catalog).
- Traces:
  - The node-local endpoint takes OTLP spans from instances.
  - The edge and links start a span per request and propagate W3C trace context.
  - `TelemetryPolicy` per formation or environment: sampling (ratio, always on error), retention,
    which signals go to which sink.
- Isolation: an account reads only its own telemetry. The `ClusterOperator` reads node metrics and
  weft's own components' telemetry, never tenant logs or spans (§ Actors).
- Simulation: the browser simulator (§ Verification) shows series and alerts as the model emits
  them.

## 17. CI/CD (the owner, 2026-11-26)

Design: `.engineering/planning/design/ci-cd-support.md` (AEP `design:ci-cd-support`). Three
stages: CD from an existing CI (OIDC federation in `access`, deploy gates, preview environments),
then ephemeral CI runners on weft (`pipelines`, layer 6), then pipelines defined in weft (deferred).

## Verification and simulation (the owner, 2026-11-26)

Every part of weft is a formal ESS document, so the cloud can be tested
and simulated before any implementation exists.

- Conformance: `ess verify conform` synthesizes a suite from the domains
  and runs it against the interpreted target (the spec itself) and later
  against each real component.
- Protocols: `ess verify protocol explore` checks the node and exec
  channels over every interleaving within bounds.
- Hardening (skill `ess:hardening`): mutation of the spec and the
  implementation, random command sequences, model-based checks.
- Fault injection: node loss, lease expiry, message loss on a channel,
  a slow wake, a full disk, a GPU that fails its wipe.
- Scenarios: crafted day-in-the-life runs, e.g. the SSO service asleep
  at night, 2,000 logins at 08:00, scale-out to its bound, a quota
  refusal, scale back to zero.
- Simulator in the browser: drive the interpreted spec with a generated
  load (requests, deploys, node failures) and render the cluster live:
  instances waking and sleeping, scale up and down, placements, meters
  and costs as they accrue. ESS's `ess ui` (`ess-ui/1`) runs a UI against
  the HTTP surface a spec serves; whether it can show a time-driven
  simulation is unverified.
- The same scenarios run later against the real cloud; a difference
  between simulation and reality is a defect of one of them.

## CLI (high level)

```console
weft login                      # federated clusters only
weft context use acme/research
weft deploy shop.yaml --env dev # plan, simulate, ask, apply
weft deploy system.yaml         # a single spec becomes a one-component formation
weft plan shop.yaml --env prod
weft invoke shop/orders ...
weft secret set shop/dev STRIPE_KEY
weft logs shop/agent
weft account create acme/research/gpu-lab --quota gpu=4
```

## Lowering (how weft itself is deployed)

- `local`: one process is control plane and worker; single-user access.
- `hosts`: control plane + NodeAgent per host; microVMs.
- `aws`: NodePools to EC2 launch templates; volumes to EBS; buckets to S3.
- `kubernetes`: Formation -> namespace; Link -> NetworkPolicy; Service ->
  Deployment; stateful -> StatefulSet; volume -> PVC; isolation ->
  RuntimeClass; Secret -> Secret from the backend.

## Ecosystem formats

Other specs of the ecosystem enter only as cloudspec architectures, through
cloudspec's inbound translators (last section). The cloud's own domains reuse none of them.

- `loom.agent/1`: the agent document; Loom owns the format
  (`agency/README.md`). Input of cloudspec's agent translator.
- ESS `system.yaml`: input of cloudspec's ESS-system translator.
- ModelSpec: input of a later cloudspec translator.

## Open points

- GPU in a microVM needs PCI passthrough. Unverified which VMMs support it.
- MicroVMs need hardware virtualization: on EC2 a bare-metal instance or
  nested virtualization. Unverified which instance types qualify.
- What an `ess-system` translates to: which runtime serves an ESS system's
  commands and views, and where its state lives.
- `bucket` and `filesystem` sit in resources and in the catalog's
  `object-store`; the integration pass decides one home.
- Out of scope: multi-region, a web console, billing beyond PriceList.

## Translation moved to cloudspec (2026-11-28)

The owner, 2026-11-28 about 02:14 CET (decision D5): cloudspec is the lowest common point, the
intermediate representation. Every source translates into cloudspec, every target is generated
out of it, and weft runs only cloudspec specifications. About 02:08 CET: weft is the runtime of a
cloudspec specification; the enterprise multi-tenant cloud of this sketch is its first example.

- The deployment sources of section 5 (Compose, Helm, Kubernetes, ESS system, agent, training)
  are inbound translators of cloudspec (cloudspec epic `translation`); weft receives a cloudspec
  architecture. Until cloudspec has its architecture format, a weft formation document stands
  in for it.
- The translator rules of section 5 "Translation" stay as written and are cloudspec's now:
  registered per source kind and version, deterministic, a coverage report of mapped, mapped
  with loss and refused, a refused object stops a deploy, no grant beyond the source. cloudspec
  adds a `foreign` element for what a reader cannot express, and an equivalence check of source
  and target.
- Not changed yet: `ess/6-delivery/deployments.yaml` (`Translator`, `DeploymentSource`,
  `Translation`, `CoverageEntry`, `SourceKind`) and `RegisterTranslator`, `RetireTranslator`
  (`ess/9-actors/actors.yaml:100-101`). Their removal, or their reduction to a record of
  cloudspec's translation, is a later specification change (`epic:agent-workloads`).

## Run-time records stay in weft (D7 A, 2026-11-28)

The owner picked D7 A (relayed by the host operator, 2026-11-28 before 05:00 CET), after a
boundary review of three reviewers: cloudspec holds the design-time pieces (blocks, contracts,
needs, the binding format, static admission, the reference in-memory runtime); weft keeps its
run-time records: `Instance`, `PlacementDecision`, `SecretDelivery`, `Release` and `ReleaseStep`,
`Drift`, `Alert`, `Engine`, `OfferingPlan`, the actors `Scheduler`, `NodeAgent` and
`ControlPlane`, its engines, the bound configuration and run-time admission. Entities whose side
is not settled yet (`Invocation`, `Session`, `Snapshot`, `Plan`, `Translation`,
`ServiceInstance` and others) stay in cloudspec for now, listed as open in cloudspec's
`docs/boundary.md`. weft's `ess/` takes the design-time blocks from a vendored cloudspec at a
pinned commit (cloudspec D2 A, `scripts/vendor.sh`) once cloudspec sends the commit.
