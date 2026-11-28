# weft hardening pass 2: state-space exploration with weft-sim (2026-11-26)

> weft-sim moved to cloudspec on 2026-11-28 as
> [cloudspec-sim](https://github.com/beyond10x/cloudspec/tree/2ed72c2866bbd165da2de75250b2bfcb340a96ae/crates/cloudspec-sim).
> This record keeps the names and paths of its time: `weft-sim` is the binary then, and
> `tools/sim/...` a path of weft at the revision the record cites (`crates/cloudspec-sim/...` of
> cloudspec now). To rerun a command below, install cloudspec-sim with `task sim-install` (it pins
> the revision) and call `target/cloudspec-sim/bin/cloudspec-sim` with the same arguments, `--spec
> ess` included. `task sim-build` and `task sim-test` no longer exist; cloudspec tests the tool.

Technique: random command sequences and experiments on the in-memory interpreted model
(`ess:hardening` "random command sequences", "model-based check"), with `tools/sim` (`weft-sim`,
design:simulation-and-kpis stage 1). ESS engine: beyond10x/ess main `c0b5f560`, crate
`ess-conformance`, `Interpreted::for_model`.

## First run (before fixes)

- explore, seed 1, 4096 walks x 300 steps, 8 threads: 119 s, about 10,300 steps/s.
- Outcomes reached 1107 / 2327, states 134 / 310, transitions 74 / 205; 190,517 distinct abstract
  states; 9,091 branch points; 0 invariant violations.
- Experiments: `sim/experiments/duplicate-delivery.yaml` (search `fit`), `sim/experiments/node-loss.yaml`
  (search `exhaustive`, 9 points).

## Findings and what changed

1. `nodes.yaml` set `last_seen_at: {generated: true}` (a minted timestamp, a fixed 2020 date in the
   interpreter): `SilentNodeSessions` listed healthy sessions and the R3 controller marked them Lost;
   pattern `healthy-node-lost` matched at 9 / 9 points of node-loss.
   Changed: `OpenNodeSession` and `Heartbeat` take `received_at` (stamped by weft-control's channel
   endpoint on receipt; the node's frame carries no time) and set `last_seen_at` from it; a new first
   outcome `received-in-future` (`received_at > now`, error `weft.nodes.ReceivedInFuture`) stops a
   caller from postponing its own loss. ESS 0.99.2 refuses `now` as a value (ESS-COMMAND-009).
   After: 0 / 3 matches with a heartbeat every tick (converged in 0-2 steps); 6 / 6 matches with a
   heartbeat every 60 or 90 s against the 30 s timeout, which are real losses. The timeout value
   stays an OPEN in `nodes.yaml`.
2. 13 commands of `formations.yaml` had no `unknown_instance` branch (`RemoveRoute` `:1673` and
   others): an unknown id got a refusal no outcome declares (2,704 answers, seed 1, 1024 walks).
   Changed: an `unknown-<x>` outcome with the existing not-found error in each. After: 0 undeclared
   refusals in the whole spec. The old `no-such-<x>` related checks are now unreachable but ESS
   0.99.2 requires them (ESS-COMMAND-005).
3. Open: the interpreter answers `Unsupported` 50,405 times for guards over a field no outcome
   writes or an absent Optional (`accounts.yaml:520`, `:570`, `:1216`, `:1277`;
   `identity.yaml:539`; `cluster.yaml:574`). Not changed yet.
4. Open: `PlaceInstance` never accepts in explore, so the chain formation -> workload -> instance ->
   placement -> assignment is not reached and the no-double-placement and tenant-isolation checks
   run over empty assignments.

## Numbers after the fixes

- explore, seed 1, 1024 walks: outcomes reached 1089 / 2340 (was 1059 / 2327 on the same settings).
- node-loss: 0 violations; duplicate-delivery: 0 violations.
- Validate: `weft v1 — 23 file(s), valid`; layers green.

## What the ESS API lacked (for the ESS epic)

No row read or scenario fork on `Interpreted`; `execute::execute_at` takes no caller and runs no
grants or bindings; bindings report no outcome; no drop or reorder of a bound event; the spec loader
is private to `ess-cli`; `execute_command` errors when a binding fails although the command changed
state; minted timestamps are unrelated to the command clock.

## Full setup (2026-11-26)

Experiment `sim/experiments/full-setup.yaml`, built only from the specification (no `ess/` change).
Setup, 127 commands: a cloud with zones z1 and z2, a priority class, a runtime; a shared pool for the
default cluster; tenants acme and globex, an account each, top quotas, an acme sub-account with an
allocated and drawn quota; acme's cluster with three dedicated pools, nodes provisioned, joined,
observed Ready and with an open session, and a topology whose three groups are the pools; acme's
formations `shop` (web and mobile call api) and `etl` (ingest -> transform -> load) in acme's
cluster, globex's `site` in the default cluster; revisions, environments, artifacts, one service
per component. Per tick of 30 s: heartbeats, R3 (lose silent sessions, release expired leases), R1
(`ServicesBelowScale -> RequestReplica`), a scheduler that tries every node from the weakest
isolation (Container, then MicroVm) and keeps the first `placed`, the node agent (artifact verified,
running), lease renewals; traffic `FanIn` on shop and `Pipeline` on etl, 12 messages per link per
tick, sent to the model as `AutoscaleService`, and counted in topology links crossed between the
groups of the two running instances' nodes. Fault at tick 3: node 0 of acme's group g0 goes silent
and is cordoned. Mutations: topology Star (Hub, Spoke, Spoke) or Ring (three Members), replicas 1
or 3, nodes per pool 2 or 4; `search: exhaustive`, 8 points.

Rerun: `tools/sim/target/debug/weft-sim simulate --spec ess --experiment
sim/experiments/full-setup.yaml --out sim/out/full-setup --threads 8 --exit-when-done` (8 threads,
111 s; exit 1: the convergence violation below).

### How far the chain gets

The whole chain is accepted at every point (`chain-reached` 33 / 33): cloud, zones, pools, nodes
joined and Ready, sessions, default and tenant cluster, topology and groups, tenants, accounts,
quotas, formations, components, links, revisions, environments, artifacts, workloads, services,
`RequestReplica` -> `CreateInstance` -> `SubmitSchedulingRequest` (bindings) -> `PlaceInstance`
-> `BindPlacement`, `Assign`, `ScheduleInstance`, `RecordPlacementAssigned` (bindings) ->
`RecordArtifactVerified` -> `RecordInstanceRunning` (`running-ready`), the last command accepted.
Finding 4 above was the explorer, not the spec: a random walk does not build the 20 ties
`PlaceInstance` reads (a Ready node with isolation facts in the request's cluster, zone and pool).

Guards that held (point Ring, 3 replicas, 4 nodes per pool): `PlaceInstance` refused
`node-in-other-clusters-pool` 72, `node-outside-dedicated-pools` 64, `shared-kernel` 17 (globex
next to acme below MicroVm), `node-not-schedulable` 14 (the cordoned node); placed 160.

### Where the spec stops the chain

1. First, and it holds everything after `running`: `Assign` sets `assigned_at` and
   `lease_expires_at` `{generated: true}` (`ess/5-runtime/nodes.yaml:1064`, `:1065`), and
   `RenewLease` sets it the same way (`:1244`). The interpreter mints 2020-01-19 while the clock
   reads 2026-01-01, so every new assignment is in `ExpiredLeases` (`:1933`) at once, `RenewLease`
   answers `lease-expired` (`:1235`) 165 of 165 times, and R3 releases every assignment in the next
   tick (144 releases): evict, replica ended, requested again, placed again. An instance runs for one
   tick; the controllers never converge (violation `convergence` at 8 of 8 points). The spec would
   need the expiry from the caller's clock, as finding 1 did for `received_at`: `Assign` and
   `RenewLease` take `lease_expires_at` (weft-control stamps it, a `when` refuses one not after
   `now`), or ESS accepts `now + <duration>` in `sets:` (ESS 0.99.2 refuses `now` as a value,
   ESS-COMMAND-009).
2. `PlaceInstance` reads the node's `Ready` state (`ess/5-runtime/scheduling.yaml:954`) but not
   its session; `Assign` refuses `no-open-session` (`nodes.yaml:1016`) and `r4-assign` retries it
   without bound (`ess/components.yaml:1162`, OPEN at `:1156`). With the fault's cordon left out, the
   scheduler places on the silent node (Ready, session Lost), and that one `PlaceInstance` did not
   return within 250 s (the run ends at the 280 s bound; with the cordon all 8 points take 111 s):
   an unbounded retry runs up to 65,536 attempts in the interpreter (`ess-conformance`
   `interpret/bindings.rs:32`, inferred, not traced). The spec would need a `PlaceInstance` guard on an
   open `NodeSession` of the node, or a retry bound with an escalation event on `r4-assign`.
3. `RecordClusterNodePoolReady` has no row to start from (`ess/components.yaml:1718`, OPEN):
   `NodePoolCreated` does not name the cluster pool. The simulator takes `node_pool_id` from the
   binding's `NodePoolCreated`; weft-control would need a view of Requested cluster pools and the
   cluster pool's id on `CreateNodePool`, or a binding on `NodePoolCreated` that carries it.
4. No view shows a formation's cluster (`ess/4-applications/formations.yaml:2211` `Formations` has
   no `cluster_id`); the pattern "placed outside its formation's cluster" reads the copy on
   `SchedulingRequest.cluster_id`.

### Numbers

| topology | replicas | nodes per pool | steps | messages | link-crossings | violations | pattern matches |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Star | 1 | 2 | 670 | 480 | 768 | 1 | 0 |
| Ring | 1 | 2 | 670 | 480 | 444 | 1 | 0 |
| Star | 3 | 2 | 1269 | 480 | 542 | 1 | 0 |
| Ring | 3 | 2 | 1269 | 480 | 366 | 1 | 0 |
| Star | 1 | 4 | 806 | 480 | 324 | 1 | 0 |
| Ring | 1 | 4 | 806 | 480 | 324 | 1 | 0 |
| Star | 3 | 4 | 1423 | 480 | 346 | 1 | 0 |
| Ring | 3 | 4 | 1423 | 480 | 298 | 1 | 0 |

Sum over points: Star 1980 link-crossings, Ring 1432; best point Ring, 3 replicas, 4 nodes per
pool. With three groups a Ring links every pair and a Star sends spoke to spoke through the hub, so
the scheduler, which spreads replicas round robin and reads no topology, decides the rest. Patterns
`two-tenants-on-one-node-below-microvm`, `placed-on-another-clusters-pool`,
`placed-outside-dedicated-pools`, `quota-drawn-above-allocation`, `quota-remaining-above-limit`:
0 matches at 8 points. They hold only for one-tick placements: finding 1 releases every assignment
before a second tenant's could stay beside it.

### What changed in weft-sim

- Setup steps: `repeat` takes `${var}`; `${node_${i}}` resolves the inner name first; a literal
  `null` leaves an Optional out; `bind` reads events of commands bindings ran (`Event.field`).
- Controllers: `filter`, `limit`, `rounds` (a level loop that fills a gap of several), `steady`
  (not counted for convergence), `each` (candidates tried in order with `vary` values until the
  model accepts one, `spread` round robin): the scheduler.
- Traffic: `formation`, `account`, `pattern`, `rate`; KPIs `messages`, `link-crossings`,
  `messages-unrouted`; `step` optional (the model input).
- Faults: `silence` (a lost node's agent sends nothing; the control plane still sends to it).
- Patterns: `view` rows with `params` (a list joins reads), `as`, `where` (`== != < <= > >=`),
  `absent`; view patterns are checked at the end of each tick.
- Mutations: kind `vars` (one value sets several variables). Experiment `chain`: KPIs
  `chain-reached` and `chain-stop`.
- The tenant-isolation check reads each assignment's tenant from its `PlacementDecision` (it read
  `Instances` and `AccountTree` before and saw no tenant). An answer is accepted only on a
  non-error outcome. A command does not move the simulated clock (ticks do): with 1 s per command a
  tick of 100 commands silenced every session. node-loss now matches `healthy-node-lost` at 3 of 9
  points (heartbeat every 90 s), was 6 of 9: a heartbeat every 60 s is exactly 30 s old at a tick
  and no longer counted silent. A command over 1 s is written to the notes.
- Tests: 9 in `tests/main.rs` (2 new: nested variables; view patterns with `where` and `absent`),
  0.14 s.

### Fixes of the stops (2026-11-26)

1. Lease times from weft-control's clock. ESS 0.99.2 cannot add a duration in `sets:` (only
   `now ± <literal>` in a guard or filter), so the lease end is not stored:
   `Assignment.lease_expires_at` became `lease_renewed_at` (`ess/5-runtime/nodes.yaml:137`), and the
   lease ends 60 s after it (`ExpiredLeases` filter `:1962`, `RenewLease` `lease-expired` `:1266`,
   `ReleaseAssignment` `lease-held` `:1304`; views `:1919`, `:1937`, `:1953`, `:1967`). The length
   stays an OPEN, taken 60 s, twice the heartbeat timeout (`:989`).
   - `Assign` takes `assigned_at` (`:1014`), first outcome `assigned-in-future` (`:1017`, new error
     `weft.nodes.AssignedInFuture` `:531`), and sets `assigned_at` and `lease_renewed_at` from it
     (`:1086`).
   - `RenewLease` takes `received_at` (`:1247`), first outcome `received-in-future` (`:1249`, error
     `ReceivedInFuture`), and sets `lease_renewed_at` from it (`:1273`).
   - A binding has no clock, so the time travels with the placement: `PlaceInstance` takes
     `decided_at` (`ess/5-runtime/scheduling.yaml:855`; it was minted too, `:1110`), first outcome
     `decided-in-future` (`:857`, new error `DecidedInFuture` `:444`); `PlacementBound` carries it
     (`:538`, payload `:1153`); `r4-assign` maps `assigned_at: event.decided_at`
     (`ess/components.yaml:1154`). The level step on `UnassignedPlacements` stamps the Scheduler's
     clock (comments `components.yaml:993`, `scheduling.yaml:1726`). No binding sends `RenewLease`.
   - Experiment: `scheduler-place` sends `decided_at: "${now}"`
     (`sim/experiments/full-setup.yaml:439`), `lease-renewals` sends `received_at: "${now}"` (`:465`).
2. `PlaceInstance` outcome `node-not-connected` (`scheduling.yaml:982`, new error `NodeNotConnected`
   `:440`): no `weft.nodes.NodeSession` of the node in state Open; placed after
   `node-not-schedulable`. ESS expresses it (the row-set form of `Assign` `no-open-session`).
   `r4-assign` drops `no-open-session` (`components.yaml:1168`, retry list `:1176`): it follows only a
   session lost between placement and delivery; the placement stays in `UnassignedPlacements`. OPEN
   there: a node that never redials keeps that placement Bound, and no lease releases it.
3. `Formations` view shows `cluster_id` (`ess/4-applications/formations.yaml:2227`, sketch line 46).

Numbers after (same command, 8 threads, 3.7 s, was 95.5 s; exit 0):

| topology | replicas | nodes per pool | steps | messages | link-crossings | violations | pattern matches |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Star | 1 | 2 | 295 | 480 | 660 | 0 | 0 |
| Ring | 1 | 2 | 295 | 480 | 420 | 0 | 0 |
| Star | 3 | 2 | 499 | 480 | 640 | 0 | 0 |
| Ring | 3 | 2 | 499 | 480 | 400 | 0 | 0 |
| Star | 1 | 4 | 417 | 480 | 240 | 0 | 0 |
| Ring | 1 | 4 | 417 | 480 | 240 | 0 | 0 |
| Star | 3 | 4 | 637 | 480 | 260 | 0 | 0 |
| Ring | 3 | 4 | 637 | 480 | 260 | 0 | 0 |

- Sums: Star 1800 link-crossings (was 1980), Ring 1320 (was 1432); converged in 3 ticks at every
  point (was unconverged at 8 of 8); `chain-reached` 33 / 33.
- The point the report keeps (1 replica, 4 nodes per pool): `RenewLease` `renewed` 68,
  `ReleaseAssignment` `released` 1 (the silenced node's lease), `PlaceInstance` `placed` 8. Before
  (3 replicas, 4 nodes per pool): `lease-expired` 165 of 165, `released` 144, `placed` 160.
- Without the fault's cordon (scratch copy of the experiment): 4.0 s, 0 violations, `PlaceInstance`
  `node-not-connected` 1 on the silent node (was a run over 250 s).
- node-loss: 0 violations, `healthy-node-lost` 3 / 9 (unchanged); duplicate-delivery: 0 violations.
- Patterns stay at 0 matches; they now hold over assignments that stay.
- Validate: `weft v1 — 23 file(s), valid`; layers green.

### A placement whose node never takes it (2026-11-26)

The OPEN of fix 2: a placement whose node never redials stays Bound and unassigned, holds its
units, and its instance runs nowhere. Specified as a time step of R3 in level form, like
`ExpiredLeases`:

- View `weft.scheduling.OverdueUnassignedPlacements` (`ess/5-runtime/scheduling.yaml:1795`): Bound,
  `assigned == false`, `decided_at < now - 120s`. The length stays an OPEN (`:1208`), taken 120 s:
  twice the lease, so a node that redials within a lease length still takes the placement through
  the level step on `UnassignedPlacements`. A row is also a placement Assign keeps refusing
  (`node-not-ready`, `isolation-unavailable`); OPEN (`:1792`): a gang member's row is released alone.
- `ReleasePlacement` (`:1211`) holds the rule: outcome `assignment-pending` (`:1220`, new error
  `AssignmentPending` `:448`) refuses a Bound unassigned placement younger than 120 s;
  `released-unassigned` (`:1224`) releases an older one and emits `PlacementReleased` and the new
  `UnassignedPlacementReleased` (`:558`, decision, instance, node). The Scheduler runs it per row
  (already granted).
- Binding `r3-evict-on-unassigned-release` (`ess/components.yaml:1359`): the event evicts the
  instance (NodeLost); `r3-replica-ended-on-eviction` and R1 request the replica again, and
  `PlaceInstance` refuses the node while it has no open session. Step table `:993`, r4-assign
  comment `:1173`, event list `:851`; `assignment-pending` at `r2-release-placement-on-end` and
  `r3-release-placement-on-lease-end` is retried as transient (`:1261`, `:1351`).
- OPEN for the integrator: `ess/9-actors/actors.yaml` does not name the view for the Scheduler
  (outside this change's files), so no actor is read-granted it; weft-sim reads views without a
  grant.

Experiment `sim/experiments/stuck-placement.yaml`: the full setup with `lost_bindings: [r4-assign]`
(`:396`), so a bound placement reaches its node only through the level step
`r4-assign-unassigned` (`:433`), one tick after it was placed; `r3-release-overdue-placements`
(`:427`) runs `ReleasePlacement` on the new view. Fault at tick 1: node g0-0 closes its session
(`CloseNodeSession`, `:403`) and goes silent; not cordoned, it stays Ready. weft-sim: experiment
field `lost_bindings` (`tools/sim/src/experiment.rs:52`; `spec::load_without`,
`tools/sim/src/spec.rs:20`, refuses a name no file declares); test 10 of 10 in `tests/main.rs`,
0.08 s.

Result (8 threads, 3.3 s, exit 0): 0 violations at 8 of 8 points, converged 6 ticks after the
fault, `chain-reached` 33 / 33, patterns 0 matches. At the kept point (Star, 1 replica, 4 nodes per
pool): Assign `no-open-session` on g0-0 at ticks 1 to 4, `ReleasePlacement` `released-unassigned`
at tick 5, `PlaceInstance` `node-not-connected` on g0-0 and `placed` on another node,
`RecordInstanceRunning` `running-ready`, Assign `assigned` at tick 6. Same at the base point
(`--no-search`, 0.5 s).

| topology | replicas | nodes per pool | steps | link-crossings | violations | pattern matches |
| --- | --- | --- | --- | --- | --- | --- |
| Star | 1 | 2 | 357 | 948 | 0 | 0 |
| Ring | 1 | 2 | 357 | 612 | 0 | 0 |
| Star | 3 | 2 | 619 | 912 | 0 | 0 |
| Ring | 3 | 2 | 619 | 576 | 0 | 0 |
| Star | 1 | 4 | 511 | 336 | 0 | 0 |
| Ring | 1 | 4 | 511 | 336 | 0 | 0 |
| Star | 3 | 4 | 789 | 372 | 0 | 0 |
| Ring | 3 | 4 | 789 | 372 | 0 | 0 |

- The other experiments, rerun: full-setup 0 violations, 3.6 s, numbers unchanged from the table
  above; node-loss 0 violations, `healthy-node-lost` 3 / 9; duplicate-delivery 0 violations.
- In weft-sim the node agent's controllers read `workloads.Instances`, not the node's assignments,
  so an instance placed on g0-0 runs before it is assigned; the eviction takes it from Running.
- Validate: `weft v1 — 23 file(s), valid`; layers green.

## Pass 3: Unsupported guards (2026-11-26)

Finding 3. Every guard was a comparison over an Optional field that the row holds absent
(`{cleared: true}`): ESS 0.99.2 reads `==`, `!=` or `>` over it as Unknown (Kleene), while
`defined(x)` reads False and makes an `all:` False. The interpreter itself is consistent; ESS gives
no warning, so the case is `docs/ess-gaps.md` gap 25. Each guard now states the absent case.

| guard | why Unknown | change | Unsupported before | outcomes after (before) |
| --- | --- | --- | --- | --- |
| `accounts.CreateSubAccount/name-taken` (`ess/2-tenancy/accounts.yaml:540`) | top-level accounts have no parent | `defined(parent_account_id)` first | 2,700 | created 495 (0), name-taken 40 (0), parent-closed 389 (0) |
| `accounts.CloseAccount/has-open-children` (`:589`) | same | same | 2,418 | closed 818 (0), has-open-children 198 (0), wrong-state 407 (0) |
| `CloseAccount/closed`, `affects:` on RoleBinding (`:603`) | tenant-wide bindings have no account; showed once the guard above was decided (401) | `defined(account_id)` first | 0 (401 between) | as above |
| `accounts.AddUser/subject-taken` (`:640`) | service accounts and workload principals have no subject | `defined(subject)` | 1,319 | added 1176 (544), subject-taken 983 (446) |
| `accounts.AddWorkloadPrincipal/identity-taken` (`:733`) | users and service accounts have no workload identity | `defined(workload_identity)` | 1,359 | added 1201 (478), identity-taken 942 (379) |
| `accounts.DrawQuota/other-parent` (`:1259`) | a top-level quota has no parent quota | `any: [not defined(parent_quota_id), parent_quota_id != input.parent_quota_id]`; `top-level` removed | 1,060 | other-parent 310 (0), drawn 5 (0) |
| `accounts.ReleaseQuota/other-parent` (`:1316`) | same | same | 1,586 | other-parent 494 (0), changed-since-read 8 (0) |
| `identity.FinishKeyRotation/overlap-not-over` (`ess/5-runtime/identity.yaml:560`) | a trust domain not rotating has no end | `all: [defined(rotation_ends_at), rotation_ends_at > now]` | 2,052 | wrong-state 2188 (0) |
| `cluster.CreateCluster/name-taken` (`ess/3-resources/cluster.yaml:612`) | default clusters have no account | `defined(account_id)` first | 93 | created 353 (239) |
| `accounts.BindRole/already-bound` (`:845`) | a tenant-wide binding with the same principal and role | `defined(account_id)` first | 0 (not met on seed 1) | |
| `secrets.CreateSecret/name-taken` (`ess/5-runtime/secrets.yaml:488`) | Account and Formation scopes leave formation or environment absent (was an UNMAPPED note) | compare only the fields the scope has, each behind `defined` | 0 (two secrets of one name not met) | probe: `name-taken` on a second Account-scope secret |

- `top-level` of DrawQuota and ReleaseQuota: a top-level quota is not drawn from the named parent
  either, so it answers `QuotaNotFound` at `other-parent`. Before, a top-level quota of any account
  answered `TopLevelQuota`, which the pass-1 disclosure rule of those commands is meant to prevent.
- `RotateSigningKey` (`identity.yaml:519`): `rotation_ends_at` was minted (a 2020 instant in the
  interpreter), so every rotation was over at once and `overlap-not-over` could never hold, the
  same defect as finding 1. It is now an input stamped by weft-control's issuer (its clock plus
  `document_ttl_seconds`); first outcome `end-not-ahead` (`:521`, new error `RotationEndNotAhead`
  `:343`) refuses an end not after `now`. The random walk never sends an instant after its clock
  (`end-not-ahead` 3184 of 3184; already so for `access.CreateApiToken/expiry-not-in-future` and
  `infrastructure.ProvisionPoolNode/expiry-not-future`), so explore lost `rotated` (34) and
  `finished` (22); weft-sim would need candidates past the clock.
- `secrets.DeliverSecret`: explore reaches only `no-grant` (359): a walk never builds grant,
  version, Starting instance and identity document together. A scratch experiment (full-setup
  plus a trust domain, a backend, globex's secret, version and grant; controllers
  `IssueIdentityDocument` and `DeliverSecret` on globex's Starting instance) reaches `delivered`
  (SecretDelivery Recorded) at the first Starting tick, then `not-at-start` once Running. Not a
  spec defect. The same experiment reaches `FinishKeyRotation` `wrong-state`, `RotateSigningKey`
  `end-not-ahead` and `rotated`, then `overlap-not-over`.
- weft-sim: a setup step whose answer is `unsupported` does not fail the run (`--exit-when-done`
  exit 0 on the repro of gap 25).

Explore, seed 1, 1024 walks x 300 steps, 8 threads:

| | Unsupported (guards) | other Unsupported | outcomes | states | transitions | time |
| --- | --- | --- | --- | --- | --- | --- |
| before | 12,587 over 8 guards | 1 (`RemoveClusterNodePool`, binding budget) | 1070 / 2348 | 127 / 310 | 69 / 205 | 25.7 s |
| after | 0 | 1 (same) | 1116 / 2347 | 131 / 310 | 72 / 205 | 34.6 s |

- Experiments with `--exit-when-done`: duplicate-delivery, full-setup, node-loss, stuck-placement
  exit 0, 0 violations; node-loss `healthy-node-lost` 3 / 9 (unchanged).
- Validate: `weft v1 — 23 file(s), valid`; layers green.

## Explore from setup (2026-11-26)

Every explore walk started from an empty store with random inputs, so a command never accepted
could be unreachable in the spec or only rare. weft-sim changes (`tools/sim`, no `ess/` change):

- `explore --from <experiment.yaml>` runs the experiment's `setup:` once (95 requests for
  full-setup, 0.07 s), then every walk replays those requests from an empty scenario first.
  `Interpreted` has no store copy or fork, so the setup is replayed per walk: 65-129 ms a walk,
  about 10 % of the run's thread time; 0 of 1024 walks answered a step differently from the
  recorded setup. The setup's outcomes count once in the run.
- Guided input drawing (explore only, not `simulate`): an identity input takes a live row of its
  entity 80 % of the time (a row seen with the other chosen ids preferred, a non-terminal state
  75 %). A field that a guard compares with a field of the row another input names
  (`<row field> != input.x` in a refusal, `==` in an acceptance; 233 such ties in the spec) is
  copied from that row 85 % of the time. A required row (`when_related: {entity, where, exists}`
  refused when absent; 7 in the spec) gets a live row in an admitted state 50 % of the time. A
  Timestamp input is `now` ± 1 s, 30 s, 5 min or 1 h 60 % of the time. Shadow rows take the
  stored fields from the request and the events at creation, and copy only values the answers
  showed: a `sets:` literal, counter or generated value is never copied.
- `--retry-bound N` (default 16; 0 keeps the spec as written): a binding the spec retries without
  bound stops after N attempts. The interpreter retries up to 65,536 times in one step; from the
  setup's store `RemoveClusterNodePool` took 59 s a request and `RequestReplica` 4.4 s (32 walks in
  371 s). With the bound: 16 ms. A binding that uses its whole bound is a finding (`unbounded-retry`
  in `errors` and the top gaps). `result.json` lists `slowest_commands`.
- Per command never accepted: `refusals` (all requests) and `refusals_live` (requests whose every
  identity input named a live row) by answer, and a `class`. `blocked-by:<file:line>`: at least 8
  requests named live rows, all got the same refusal, and its guard reads a stored row (a state, a
  field predicate, a row set); for an interpreter `unsupported` answer the line is the guard it
  names. `rare`: the refusals vary, too few requests named live rows, or the one refusal is an
  input-only `when:` or an existence check (the gap is the drawing or an upstream command). The
  page lists them under "Commands never accepted".
- A setup step not accepted as declared (its `expect`, or an accepting outcome when it has none)
  fails the run: `simulate` exits 3 after writing its result, `explore --from` exits 2 before any
  walk. This closes the weft-sim gap listed in pass 3. The four committed experiments exit 0.

Explore, seed 1, 1024 walks x 300 steps, 8 threads, spec unchanged between the runs:

| | outcomes | states | transitions | never accepted | blocked-by | time |
| --- | --- | --- | --- | --- | --- | --- |
| before (old weft-sim) | 1116 / 2347 | 131 / 310 | 72 / 205 | 232 | n/a | 29.0 s |
| guided, empty start | 1175 / 2347 | 142 / 310 | 81 / 205 | 218 | 3 | 17.4-29.7 s |
| guided, `--from full-setup.yaml` | 1312 / 2347 | 160 / 310 | 91 / 205 | 185 | 14 | 79.5-159.6 s |

Times vary with host load. `RotateSigningKey/rotated` 0 -> 16 and `FinishKeyRotation/finished`
0 -> 6 (empty start; 6 and 1 from the setup), from the Timestamps near the clock.

The commands named before: `DrawQuota` is accepted. `PlaceInstance` and `DeliverSecret` stay `rare`
from the setup too: no request named a live SchedulingRequest. The setup stops at services;
`RequestReplica` is accepted only 10 times in 1024 walks, because `index-above-count`
(`input.replica_index >= replicas`) is an ordered guard the ties do not copy. A setup that runs the
experiment's controllers would start walks from placed instances. `CreateRunnerPool` is `rare`: all
82 requests answered `isolation-too-weak` (`ess/6-delivery/pipelines.yaml:750`, input-only
`isolation != MicroVm`), a drawing gap.

### Findings: blocked-by

From the setup (14) and from an empty store (3 more). A blocked-by is inferred: the verdict says
whether the guard is a spec gap or the row it needs comes from a command the walks rarely reach.

| command | refusal | guard | requests | verdict |
| --- | --- | --- | --- | --- |
| `workloads.RecordArtifactVerified` | `unsupported` at `wrong-node` | `ess/4-applications/workloads.yaml:2407` | 10 | spec gap: `node_id != input.node_id` over an Instance with no node is Unknown (gap 25, as pass 3) |
| `workloads.RecordProcessStarted` | `unsupported` at `wrong-node` | `workloads.yaml:2906` | 11 | spec gap, same |
| `workloads.RecordProcessRestarted` | `unsupported` at `wrong-node` | `workloads.yaml:2789` | 8 | spec gap, same |
| `workloads.RecordInstancePaused` | `unsupported` at `wrong-node` | `workloads.yaml:3006` | 9 | spec gap, same |
| `workloads.RecordInstanceFailed` | `unsupported` at `wrong-node` | `workloads.yaml:3148` | 8 | spec gap, same |
| `access.RevokeLogin` | `unsupported` at `not-own-login` | `ess/2-tenancy/access.yaml:995` | 13 | spec gap: `principal_id != input.caller_id` over a login with no principal yet is Unknown |
| `observability.CreateAlertRule` | `node-series` | `ess/5-runtime/observability.yaml:1260` | 2050 (920 empty) | upstream: account series come only from `RegisterInstanceSeries`, which needs an instance on a node |
| `identity.IssueIdentityDocument` | `instance-not-on-node` | `ess/5-runtime/identity.yaml:662` | 15 | upstream: no instance is placed (PlaceInstance) |
| `workloads.TakeSnapshot` | `not-live` | `workloads.yaml:5469` | 19 | upstream: no instance Running |
| `workloads.BeginSleep` | `no-policy` | `workloads.yaml:3819` | 1691 | upstream: no service has a sleep policy; `DefineService` `policy-needs-min-zero` (`:3462`, input-only) is a drawing gap |
| `workloads.RecordServiceAsleep` | `wrong-state` | `workloads.yaml:3879` | 89 | upstream: BeginSleep |
| `workloads.EndReplicaBackoff` | `not-backing-off` | `workloads.yaml:4205` | 1671 | upstream: no replica backs off |
| `access.ApproveLogin` | `user-code-mismatch` | `access.yaml:832` | 11 | drawing: the generated user code reaches no input |
| `access.CollectLoginToken` | `device-code-mismatch` | `access.yaml:917` | 27 | drawing: the device code is a secret of the response, compared as a digest |
| `infrastructure.CordonNode` | `wrong-state` | `ess/1-infrastructure/infrastructure.yaml:1115` | 75 (empty start) | upstream: no node Ready from an empty store; accepted from the setup |
| `infrastructure.UncordonNode` | `wrong-state` | `infrastructure.yaml:1136` | 66 (empty start) | upstream, same |

- Unbounded retry: binding `cluster-delete-node-pool` (`ess/components.yaml:1767`) used its whole
  bound 1126 times from the setup (5 from an empty store): `DeleteNodePool` answers
  `pool-has-nodes`, which only the infrastructure scale-down level step lifts, so retrying inside
  the binding cannot succeed. Same cause as the `RemoveClusterNodePool` budget answer in pass 3.
- Rerun: `task sim-explore-from` (or `tools/sim/target/debug/weft-sim explore --spec ess --seed 1
  --walks 1024 --steps 300 --threads 8 --from sim/experiments/full-setup.yaml --out
  sim/out/explore-from`); without `--from`, `task sim-explore`. Tests: `task sim-test`, 12 tests,
  each of the two new ones under 0.3 s.

## Pass 4: Optional guards in workloads and access (2026-11-26)

Blocked-by findings 1-6 of "Explore from setup", same cause as pass 3 (gap 25). Each guard now
states the absent case; a static scan of the compiled IR (every subject, related and row-set guard
of `weft.workloads` and `weft.access` that names an Optional field of the row it reads without
`defined()` of that field) found 21 more of the same shape. All fixed:

| guard | absent when | change |
| --- | --- | --- |
| `wrong-node` of `RecordArtifactVerified` (`ess/4-applications/workloads.yaml:2417`), `RecordArtifactRejected` (`:2460`), `RecordInstanceRunning` (`:2495`), `RecordCheckResult` (`:2567`), `RecordProcessRestarted` (`:2799`), `RecordProcessStarted` (`:2916`), `RecordInstancePaused` (`:3016`), `RecordInstanceResumed` (`:3046`), `RecordInstanceExited` (`:3103`), `RecordInstanceFailed` (`:3158`) | instance not scheduled, no node | `any: [not defined(node_id), node_id != input.node_id]`: no node's report about an unplaced instance is taken |
| `access.RevokeLogin/not-own-login` (`ess/2-tenancy/access.yaml:998`) | login Pending, no principal | `any: [not defined(principal_id), …]`: a pending login is nobody's own (NotOwnLogin before its state is read) |
| `access.RecordAuthorization/login-of-other-principal` (`access.yaml:1242`, row set) | same | same, in the `where`: an authorization under a pending login is a CredentialMismatch |
| `leased` of `StartBox`, `StopBox`, `SuspendBox`, `ResizeBox`, `DestroyBox`, `CordonBox`, `UncordonBox` (`workloads.yaml:4755`, `:4820`, `:4877`, `:4921`, `:4959`, `:5078`, `:5113`) | box never leased | `defined(lease_expires_at)` first; `not defined(lease_nonce)` beside the nonce compare |
| `AcquireBoxLease/held` (`:5004`) | same | `all: [defined(lease_expires_at), lease_expires_at > now]` |
| `RecordBoxStarted/foreign-instance` (`:4787`) | instance of no box | `any: [not defined(box_id), …]` |
| `RecordReplicaEnded/other-service` (`:4157`) | instance of no service | `any: [not defined(service_id), …]` |
| `RecordServiceAsleep/suspend-without-snapshots` (`:3859`) | service without sleep policy | `defined(sleep_policy)` first |
| `CreateInstance/replica-live` (`:2306`, row set) | request or instance not a replica | `when: [defined(service_id), defined(replica_index)]`, and both `defined` in the `where` |
| `CreateBox/volume-foreign`, `endpoint-foreign` (`:4684`, `:4698`, row set) | optional input absent | `when: defined(volume_id)` / `defined(endpoint_id)` |

`EndReplicaBackoff/not-yet` (`backoff_until > now`) is left: `not-backing-off` before it refuses
the absent case. The scan is clean apart from it.

Explore `--from full-setup.yaml`, seed 1, 1024 x 300, 8 threads:

| | Unsupported | outcomes | states | transitions | never accepted | blocked-by | time |
| --- | --- | --- | --- | --- | --- | --- | --- |
| before | 104 (92 in these two files) | 1312 / 2347 | 160 / 310 | 91 / 205 | 185 | 14 | 159.7 s |
| after | 15 (`OpenLogStream` 13, `RegisterInstanceSeries` 1, `RequestScaleDown` 1) | 1327 / 2347 | 161 / 310 | 92 / 205 | 184 | 14 | 270.7 s (host load) |

- `RevokeLogin`, `RecordProcessRestarted` and `RecordInstancePaused` are no longer blocked-by
  (`rare`). `RecordArtifactVerified`, `RecordProcessStarted` and `RecordInstanceFailed` stay
  blocked-by, now on the decided `WrongNode`, with `RecordArtifactRejected`, `RecordCheckResult`,
  `RecordInstanceExited` and `RecordInstanceResumed` (unsupported before, now decided). Why the
  node is empty: no walk schedules an instance. `ScheduleInstance/scheduled` is reached 0 times;
  its 7 requests on a live instance all answer `below-minimum-isolation` (input-only, a drawing
  gap: the isolation is drawn without regard to the workload's minimum), and `PlaceInstance` is
  `rare` (no live SchedulingRequest). Not a spec gap: the `full-setup` experiment, whose
  controllers schedule and start instances, reaches `RecordArtifactVerified/starting` and
  `RecordInstanceRunning/running-ready`. Closing these needs weft-sim (copy the isolation from the
  workload's minimum, or a setup that runs the controllers), not the spec.
- Experiments with `--exit-when-done`: duplicate-delivery, full-setup, node-loss, stuck-placement
  exit 0, 0 violations.
- Validate: `weft v1 — 23 file(s), valid`; layers green.

## Pass 4: pools (2026-11-26)

The dedicated node pool lifecycle of a tenant cluster had two holes: `cluster-delete-node-pool`
retried `DeleteNodePool`'s `pool-has-nodes` inside the step (the pool never drains there), and
`RecordClusterNodePoolReady` had no caller (NodePoolCreated did not name the cluster pool).

- Correlation: `weft.infrastructure.NodePool.requested_for: Optional<Uuid>`
  (`ess/1-infrastructure/infrastructure.yaml:162`), an input of `CreateNodePool` (`:702`) and a
  field of `NodePoolCreated` (`:408`); `cluster-create-node-pool` maps the cluster pool id into it
  through a declared `conversions:` entry (`ess/components.yaml:47`). Plain `Uuid`, because
  `weft.shared.ClusterNodePoolId` in layer 1 fails `tools/check-layers.sh` (upward reference).
- Drain: NodePool gets a state `Draining` (transition `drain`; `delete` from Active or Draining),
  command `DrainNodePool` (`:862`: minimum and desired size 0, event `NodePoolDrainStarted`), and
  `ProvisionPoolNode/pool-draining` (`:999`). Views `TenantNodePools` (`:1351`),
  `DrainingNodePools` (`:1363`), `DrainingPoolNodes` (`:1374`, filter `node_pool_id.state ==
  Draining`).
- Cluster (`ess/3-resources/cluster.yaml`): `RecordClusterNodePoolReady/made-for-another-pool`
  (`:861`) refuses an infrastructure pool made for another cluster pool (closes the OPEN "one pool
  recorded for two"). New `RecordRemovedClusterNodePool` (`:890`): a pool removed while Requested
  had its infrastructure pool created and never deleted; this records it and emits
  `ClusterNodePoolRemoved` again. A separate command because ESS answers a subject-state outcome
  before a related-row check (ESS-COMMAND-004), so `made-for-another-pool` could not come first in
  one command. Views `UnlinkedClusterNodePools` (`:1386`), `RemovedClusterNodePools` (`:1403`).
- Loops (`ess/components.yaml:1764`, step-table lines too): ready (Unlinked joined with
  TenantNodePools on `requested_for` -> RecordClusterNodePoolReady or RecordRemovedClusterNodePool),
  drain (RemovedClusterNodePools -> DrainNodePool), nodes (DrainingPoolNodes -> DrainNode or
  RemoveNode), delete (DrainingNodePools -> DeleteNodePool), all ControlPlane (grants
  `ess/9-actors/actors.yaml:981`). `cluster-delete-node-pool` (`:1802`) now drops `pool-has-nodes`
  and `wrong-state`; an empty pool is still deleted there at once.
- Experiment `sim/experiments/cluster-pool-lifecycle.yaml`: three pools (with Ready nodes and one
  unjoined node, empty, removed while Requested), mutations nodes 0/2/4 x fault tick 0/2: 6 of 6
  converge, 0 violations, pattern `removed-pool-left-active` 0 matches (a copy without the drain and
  record-removed controllers matches it in 6 of 6). Chain 7/7 at fault tick 2 with nodes.

Explore `--from full-setup.yaml`, seed 1, 1024 walks x 300 steps, 8 threads:

| | unbounded-retry `cluster-delete-node-pool` | `RemoveClusterNodePool` | outcomes | time |
| --- | --- | --- | --- | --- |
| before | 1098 | 60.8 ms a send (slowest command) | 1327 / 2347 | 249.9 s (host load) |
| after | 0 | not among the slowest | 1341 / 2359 | 91.9 s |
| after, `--retry-bound 0` | 0 | not among the slowest | 1341 / 2359 | 142.8 s |

- All five experiments exit 0 with `--exit-when-done`, 0 violations. Validate:
  `weft v1 — 23 file(s), valid`; layers green.
- Left: `RemoveNode/not-empty` (external) while an instance is still assigned to a draining node;
  nothing here moves it (OPEN at the loops block). A Draining pool cannot return to Active (OPEN).
  An edge on `NodePoolCreated` could record Ready now; not added, since the four older experiments
  record Ready in their setup and would answer `wrong-state`.

## Pass 4: settle (2026-11-26)

`explore --from` walks started after the setup's requests, before any controller ran, so the store
held services but no instance. weft-sim changes (`tools/sim`, no `ess/` change):

- `explore --from <exp> --settle N`: after the setup, up to N ticks of the experiment's
  controllers (30 s of clock each, no traffic or faults) until a tick in which no non-`steady`
  controller sent anything. The setup and settle are recorded with the clock of each request
  (`simulate::Recorded`) and replayed per walk. full-setup settles at tick 1: 95 setup requests,
  60 settle requests, 7 instances placed, scheduled and running.
- `--controller-share P`: a share P of walk steps is a controller tick on the walk's store
  (`simulate::Ticker` swaps the walk's session into the simulator). Controller requests count in
  the walk like its own and go in its trace with `by: controller`; every trace step has `at`, the
  clock before the send, which `replay` restores (walks' 1 h clock jumps were not in the trace).
  P = 0 draws nothing extra: walks without it are the same as before.
- `result.json` has `commands` (accepted answers and outcomes reached per command) and
  `outcome_names` (the name of each `outcome_hits` entry).
- Tasks: `sim-explore-from` adds `--settle 12`; new `sim-explore-ticks` (`--controller-share 0.1`,
  32 walks). Test `settle_and_controller_ticks_start_walks_from_what_the_controllers_did`
  (`tests/fixture-settle`), 0.12 s; 13 tests pass.

Numbers. A frozen copy of `ess/` (it validated at 11:20; other sessions were editing). Before: the
old weft-sim, 1024 walks x 300 steps, 8 threads. After: 16-walk probes only; the 1024-walk run
after the change was stopped by the owner's order.

| | outcomes | PlaceInstance | ScheduleInstance | IssueIdentityDocument | DeliverSecret | replay a walk |
| --- | --- | --- | --- | --- | --- | --- |
| before, 1024 walks | 1334 / 2351 | 0 (`rare`) | 0 (`rare`) | 0 (blocked-by `identity.yaml:662`) | 0 (`rare`) | 124 ms |
| before, 16 walks | 828 | 0 | 0 | 0 | 0 | 56 ms |
| `--settle 12`, 16 walks | 925 | 7 | 7 | 12 (7 outcomes) | 0 (0 requests) | 288 ms |
| `--settle 12 --controller-share 0.1`, 16 walks | 901 | not counted | not counted | not counted | not counted | 1326 ms |

- In the settle run PlaceInstance, ScheduleInstance, RecordInstanceRunning and
  RecordArtifactVerified are accepted (by the settle and its bindings); walks themselves drew no
  accepted ScheduleInstance, so the isolation drawing gap stands for walks.
- Cost per walk with `--settle` (summed thread time, 16 walks): setup and settle replay 288 ms,
  the walk's 300 steps 851 ms. Replay is a quarter of a walk, not the dominant cost.
- Cost with `--controller-share 0.1`: 454 ticks sent 9,154 requests in 406 s summed, about 0.9 s a
  tick and 25 s a walk, 78 % of the run; own steps 5.7 s a walk. The cost is the interpreter's per
  request on a store with running instances (PlaceInstance 95 ms, RequestReplica 126 ms,
  ReleaseAssignment 114 ms a send): the walks' 1 h clock jumps expire every lease, so each tick
  releases, re-requests and re-places the replicas. A store fork would not lower this cost; the
  settle replay (288 ms a walk) is what a fork would save.

Left (the next step): run `task sim-explore-from` (1024 walks, `--settle 12`) and
`task sim-explore-ticks`, fill a 1024-walk after row and the share row from `result.json`
`commands`, then every experiment with `--exit-when-done`, validate and layers. Not run in this
pass.

## Seed variance of explore (2026-11-26)

Asked by the owner (11:55 CET): "could we run multiple seeds in parallell ?", after
pass 4 reported 1312 -> 1327 outcomes from one seed each.

Run: `weft-sim explore --spec ess --from sim/experiments/full-setup.yaml --seed <s> --walks 1024
--steps 300 --threads 4`, seeds 1-8, without `--settle`, default `--retry-bound 16`. Each seed was its
own slot job (`slot --tokens 4 --timeout 30m`) on build node `build-1d`, all 8 at once, spec and binary at
commit `f0b7278` (binary sha256 prefix `2f5da953462cb6d7`, the same on the node and here). Every seed ran
307,295 steps; 0 violations in every seed.

Per seed: outcomes reached of 2359, states of 311, transitions of 206, abstract states, wall time.

- seed 1: 1341, 161, 91, 76,453, 212 s
- seed 2: 1344, 163, 93, 76,207, 215 s
- seed 3: 1332, 161, 92, 76,419, 216 s
- seed 4: 1341, 163, 93, 76,297, 212 s
- seed 5: 1316, 158, 89, 75,491, 214 s
- seed 6: 1333, 162, 93, 76,301, 215 s
- seed 7: 1295, 156, 89, 75,377, 215 s
- seed 8: 1330, 159, 90, 76,218, 215 s

Band over 8 seeds:

- outcomes: min 1295, median 1332.5, max 1344 (spread 49, 2.1 % of 2359).
- states: min 156, median 161, max 163.
- transitions: min 89, median 91.5, max 93.

What it shows:

- The pass-4 step 1312 -> 1327 (+15, one seed each, on an earlier spec with 2347 outcomes) is smaller
  than the spread between seeds of one spec (49). It is not evidence that the guards fix raised
  coverage; within this band a single-seed change under about 49 outcomes says nothing.
- Seed 1 reproduces the pass-4 pools number (1341) on the same spec: one seed is deterministic, the
  variance is between seeds.
- For a ratchet KPI on "outcomes reached", use the minimum (or median) of 8 seeds with fixed seeds 1-8,
  not one seed.
- 8 seeds in parallel take one seed's wall time on the build node (212-216 s).

## Pass 5: stamped time (2026-11-26)

Finding (independent review): times the model decides on with `now` came from command inputs, and
that weft-control stamps them was only a comment (`nodes.yaml` OpenNodeSession, Heartbeat). The
grants said otherwise: `ess/9-actors/actors.yaml` gave `OpenNodeSession`, `Heartbeat` and
`RenewLease` to `NodeAgent`, so a node could send any `received_at` not after `now` and postpone
its own loss or lease end; `RotateSigningKey` gave the operator `rotation_ends_at`, which the
comment called the issuer's clock plus `document_ttl_seconds`.

The rule as a type and a check:

- `weft.shared.StampedAt`, a newtype of Timestamp (`ess/0-foundation/shared.yaml:396`), marks an
  instant weft-control reads from its own clock. Conversions into Timestamp and
  Optional<Timestamp> are declared (`ess/components.yaml:56`, `:62`); none into StampedAt, so an
  edge maps a stamped input only from a stamped event field (`r4-assign` from
  `PlacementBound.decided_at`, now StampedAt with `PlacementDecision.decided_at` and its three
  views, `scheduling.yaml:227`, `:542`, `:1742`, `:1784`, `:1804`).
- `tools/check-stamped-time.sh` reads `ess specify compile` output and fails when a command with
  a StampedAt input (direct or nested in a declared type) is granted to an actor outside
  `tools/stamped-time-actors` (ControlPlane, Scheduler, ReleaseController), on a conversion into
  StampedAt, and on a listed actor the model does not declare. `--inventory` lists every Timestamp
  and StampedAt input with its grants; `--negative` copies `ess/`, grants NodeAgent
  `weft.nodes.Heartbeat` and passes only when the check refuses it
  (`granted outside the stamped-time set: weft.nodes.Heartbeat to weft.actors.NodeAgent`). Task
  `stamped-time` runs both inside `task check`, 0.5 s each. Rule in `AGENTS.md` § Rules for the
  specification.

Inventory (27 Timestamp inputs of commands):

| class | inputs |
| --- | --- |
| stamped by weft-control (7, now StampedAt) | `nodes.OpenNodeSession.received_at`, `nodes.Heartbeat.received_at`, `nodes.RenewLease.received_at`, `nodes.Assign.assigned_at`, `scheduling.PlaceInstance.decided_at`, `identity.RotateSigningKey.rotation_ends_at` (clock plus TTL), `workloads.RecordScheduleFired.fired_at` |
| chosen by a user (10) | `access.CreateApiToken.expires_at`, `formations.CreatePreviewEnvironment.expires_at`, `infrastructure.IssueJoinToken` / `ProvisionNode` / `ProvisionPoolNode` `join_token_expires_at` (bounded to 24 h), `pipelines.AddDeployGate.window_opens_at` / `window_closes_at`, `workloads.AcquireBoxLease.expires_at`, `workloads.Invoke.deadline_at`, `workloads.WakeService.deadline_at` |
| other (10) | `access.ExchangeFederatedToken.expires_at` (the ID token's signed `exp`); the node's own measurement, stored beside a minted `recorded_at` and compared with no `now`: `infrastructure.RecordNodeObservation.observed_at`, `nodes.InstanceObserved.observed_at`, `nodes.NodeObserved.observed_at`, `nodes.UsageReported.window_start` / `window_end`; period bounds weft-control chooses, checked only to have ended: `metering.RecordUsage` and `metering.IssueStatement` `period_start` / `period_end` |

Grants changed:

- `OpenNodeSession`, `SupersedeNodeSession`, `Heartbeat`, `RenewLease`: NodeAgent -> ControlPlane
  (`actors.yaml:730`, `:1006`). The node's dial, heartbeat and renewal frames carry no time;
  weft-control's node-channel endpoint records each as the ControlPlane with `received_at` from
  its clock on receipt, over the session it holds open for the node, with node_id from the
  session's certificate (`nodes.yaml:863`, `:1248`; `protocols/node-channel.yaml` header).
  `SupersedeNodeSession` has no time; it moved with `OpenNodeSession` because both are the dial.
- `RotateSigningKey`: ClusterOperator -> ControlPlane. The operator runs the new
  `RequestKeyRotation` (`identity.yaml:532`): no time, state Active -> new `RotationRequested`
  (`:112`), field `requested_signing_key_ref`, event `KeyRotationRequested` (`:418`).
  `RotateSigningKey` (`:562`) moves RotationRequested -> Rotating, new refusal
  `key-not-requested` (`:581`, error `KeyNotRequested` `:355`). Level step: view
  `RequestedKeyRotations` (`:1163`) -> `RotateSigningKey` -> ControlPlane with its clock plus
  `document_ttl_seconds` (`components.yaml:1041`). `CreateTrustDomain`'s
  `cluster-has-trust-domain` counts RotationRequested too (`:502`).
- Scheduler keeps `Assign`, `PlaceInstance`, `RecordScheduleFired`.

Not expressed: `RenewLease` still does not check the node's session. A `session_id` input with
`session-of-other-node` and `session-not-open` was tried and is refused by ess 0.99.2
(ESS-COMMAND-004: `lease-expired` and `renewed` answer at precedence step 3, a related-row guard
at step 4); the session's check stays the endpoint's (UNMAPPED at `nodes.yaml:1248`).

weft-sim: experiment steps and controllers take `relays: <actor>` (`tools/sim/src/experiment.rs`
`Step`, `Controller`; `simulate.rs` `send_step`): a fault's `silence` compares that actor with
`silent_actor`, so a silent node's frames stay unsent when the ControlPlane records them. The
experiments send OpenNodeSession, Heartbeat and RenewLease as ControlPlane relaying NodeAgent. 13
tests pass (0.11 s).

Result, worktree weft-sim, `--threads 4 --exit-when-done`, every experiment exit 0, 0 violations:
cluster-pool-lifecycle 0.1 s, duplicate-delivery 0.1 s, full-setup 4.1 s, node-loss 0.1 s,
stuck-placement 4.8 s. Node outcomes equal the main checkout's weft-sim on main's spec
(full-setup: Heartbeat `seen` 1057, RenewLease `renewed` 1156, LoseNodeSession `lost` 9,
ReleaseAssignment `released` 17; stuck-placement: Assign `no-open-session` 68,
ReleasePlacement `released-unassigned` 17; node-loss: `lost` 13). Without `relays` (scratch copy
of full-setup) the silent node kept heartbeating: `lost` 0, `released` 0. Validate:
`weft v1 — 23 file(s), valid`; layers green; no synthesis run (gap 24).

## Pass 6: system invariants in explore (2026-11-26)

Finding (independent review): explore's "0 violations" covered entity invariants only. The four
system invariants (no-double-placement, quota-conservation, tenant-isolation, idempotency) ran
only in `simulate`, on 8 mutation points; the pass-4 settle row at 1024 walks never ran.

weft-sim changes (`tools/sim`, no `ess/` change):

- `explore` checks no-double-placement, quota-conservation and tenant-isolation after every N-th
  step of a walk (`--check-every N`, default 1, 0 off; a controller tick counts as one step) and
  at the end of the walk; `--invariants LIST|none`, default all four. The checks are simulate's
  (`simulate::system_checks`). Each invariant is reported once per walk, at the first step it
  fails, with walk, seed, step and the last command.
- idempotency: on the same cadence an accepted step of a command that an `at_least_once` binding
  invokes (`Catalog::redelivered`, 41 commands; ESS: "At least once, so the command must be
  idempotent") is sent again at the same instant and must change no parameterless
  read-your-writes view. The duplicate is in the trace with `by: duplicate`.
- `--settle N` now also checks convergence (sketch § 14 "Convergence is a checked property"):
  after each walk the experiment's controllers tick with no new input up to N ticks; a walk whose
  controllers still send requests is `no-convergence`, named with the busy controllers. The ticks
  are in the trace (`by: converge`). A setup that does not settle is one too.
- A violating walk keeps its trace (three per invariant; per command for idempotency). `replay`
  checks the invariants after every step and prints the step where one starts to fail; every
  replay below answered as recorded (0 answers differ) and found its violation at the same step.
- `simulate`'s tenant-isolation check counted a node with two tenants and any instance below
  MicroVm; it now counts tenants that each have an instance below MicroVm (the spec's
  `shared-kernel`, `ess/5-runtime/scheduling.yaml:973`). A MicroVm has its own kernel: the old
  rule flagged 9 legal placements after convergence ticks.
- `sim/experiments/full-setup.yaml`: two controllers the stand-in scheduler and node agent lacked
  (sim defects the convergence check found, 10 of 32 walks in a probe). `scheduler-declare-unschedulable`:
  a pending request no node took in a tick is declared Unschedulable (`NoCapacity`; the spec leaves
  when open, `scheduling.yaml:51`); before, `PlaceInstance` answered `outside-cluster-zones` 180
  times a tick forever. `node-report-verify-failed`: an instance whose artifact verification was
  refused (`revoked`, `workloads.yaml:2427`) is reported failed (`RecordInstanceFailed`, which the
  `fail` transition allows from Scheduled); before, it stayed Scheduled and was re-verified every
  tick. The five experiments still exit 0 with 0 violations.
- Tests (`tests/fixture-invariants`, one model whose Assign guard the test removes): one test per
  check, each with a model that breaks it and one that does not: no-double-placement 0.10 s,
  idempotency 0.07 s, no-convergence 0.05 s; 16 tests pass in 0.17 s.

Cost of the checks. Seed 1, 1024 walks x 300 steps, 4 threads on `build-1d`: checks off 173 s,
on 181 s (+5 %); the checks take 65 s of the run's thread time (about 9 %), 4,319 duplicates.
32-walk probe here: every step 1.5 s summed, every 10th step 0.1 s. Per step is the default.
With `--settle 12` the convergence ticks take 566-590 s of thread time per seed (about 2 ticks
a walk, 2,061-2,075 ticks a seed), a quarter of the run.

Runs: `weft-sim explore --spec ess --from sim/experiments/full-setup.yaml --seed <s> --walks 1024
--steps 300 --threads 4 --max-seconds 1700`, seeds 1-8, each its own slot job (`--tokens 4
--timeout 30m`) on `build-1d`, at commit `505670b` (binary sha256 prefix `c2edb2371a0f0480`).
Every seed ran all 1024 walks. Outcomes of 2443, states of 328, transitions of 221.

a) invariants on, no settle:

| seed | outcomes | states | transitions | idempotency | other invariants | time |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | 1390 | 169 | 97 | 639 | 0 | 181 s |
| 2 | 1379 | 168 | 98 | 643 | 0 | 176 s |
| 3 | 1387 | 167 | 94 | 639 | 0 | 180 s |
| 4 | 1382 | 168 | 96 | 650 | 0 | 190 s |
| 5 | 1385 | 169 | 97 | 632 | 0 | 181 s |
| 6 | 1377 | 168 | 98 | 642 | 0 | 184 s |
| 7 | 1375 | 168 | 97 | 636 | 0 | 183 s |
| 8 | 1379 | 167 | 95 | 636 | 0 | 191 s |
| min / median / max | 1375 / 1380.5 / 1390 | 167 / 168 / 169 | 94 / 97 / 98 | 632 / 639 / 650 | 0 | 176 / 182 / 191 s |

b) the same with `--settle 12` (the missing pass-4 "after" row; the setup settles at tick 1, 155
requests):

| seed | outcomes | states | transitions | idempotency | no-convergence | other invariants | time |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | 1563 | 197 | 115 | 577 | 0 | 0 | 569 s |
| 2 | 1554 | 196 | 113 | 554 | 0 | 0 | 571 s |
| 3 | 1560 | 195 | 114 | 612 | 0 | 0 | 570 s |
| 4 | 1557 | 195 | 113 | 562 | 0 | 0 | 578 s |
| 5 | 1554 | 195 | 114 | 627 | 0 | 0 | 578 s |
| 6 | 1554 | 195 | 113 | 632 | 0 | 0 | 422 s |
| 7 | 1550 | 196 | 114 | 594 | 0 | 0 | 501 s |
| 8 | 1558 | 193 | 113 | 583 | 0 | 0 | 573 s |
| min / median / max | 1550 / 1555.5 / 1563 | 193 / 195 / 197 | 113 / 113.5 / 115 | 554 / 588.5 / 632 | 0 | 0 | 422 / 570.5 / 578 s |

The settle raises outcomes by 175 (median), well outside the 8-seed band of either row (15 and 13).
With it `PlaceInstance` and `ScheduleInstance` are accepted 2,679-2,999 times a seed and
`IssueIdentityDocument` 719-771 (0 in row a); most come from the settle and the convergence ticks.
`DeliverSecret` stays 0.

Violations and their class:

- idempotency, `weft.infrastructure.CreateNodePool`, every one of the 9,858 (all seeds, both
  rows): spec defect. The binding `cluster-create-node-pool` (`ess/components.yaml:1911`)
  delivers it `at_least_once`, and its outcomes (`ess/1-infrastructure/infrastructure.yaml:703`-`716`)
  have no refusal for a request already served: a second delivery with the same `requested_for`
  creates a second NodePool (`NodePools`, `TenantNodePools` one row more). The duplicate in every
  kept trace carried a `requested_for`. The spec would need a refusal before `created` when a
  NodePool with `requested_for == input.requested_for` exists (only for a set `requested_for`; an
  operator's own pool has none), dropped at the binding. Replay: `weft-sim replay --spec ess
  --trace sim/out/inv-settle/s1/result.json --walk 0` (violation at step 92).
- Fixed in weft-sim before the runs above (sim defects): the duplicate was first sent 1 s after
  its step, so `weft.nodes.SilentNodeSessions` (it reads `now`) changed between the fingerprints:
  18 of 3,225 idempotency violations in the first run (SetWorkloadReplicas, StartWaking,
  WakeService); the tenant-isolation rule (9, above); the two full-setup controllers (above).
- No-double-placement, quota-conservation, tenant-isolation and no-convergence: 0 in every seed.

Spec finding beside the invariants (in `errors`, not a violation): `weft.nodes.ReleaseAssignment`
answers `unsupported` about 3,100 times a seed in row b, `RecordInstanceFailed` 495-822,
`RecordInstanceExited` and `EvictInstance` about 150 each. The cause is the binding into
`weft.observability.RetireInstanceMetricSeries`: its `instances: where: instance_id ==
input.instance_id` (`ess/5-runtime/observability.yaml:1943`) is Unknown over a node's series,
whose `instance_id` is Optional and absent (`:273`). The spec would need `defined(instance_id)`
before the compare (the form pass 4 used for `wrong-node`).

Rerun: build with `slot --fetch tools/sim/target/debug/weft-sim task sim-build`, then the command
above per seed as a slot job (`slot --tokens 4 --timeout 30m --fetch sim/out/<dir>/s<n> ...`).

### Pass 6 fixes (integrator, 2026-11-26)

- `weft.infrastructure.CreateNodePool` gains the first-row refusal `already-requested`
  (`PoolAlreadyRequested`) when a NodePool with the same `requested_for` exists; binding
  `cluster-create-node-pool` drops it. A repeated delivery no longer makes a second pool.
- `weft.observability`: four `instance_id == input.instance_id` filters (`OpenLogStream` x2,
  `CloseInstanceLogStreams`, `RetireInstanceMetricSeries`) read `defined(instance_id)` first.
- After, seed 1 (`--from full-setup`, 1024 x 300, 4 threads, invariants on): violations 0 (was
  idempotency about 640 per seed); unsupported answers 8, all `OpenLogStream` (was about 3,100
  in `ReleaseAssignment` per seed); outcomes 1393 / 2444 (one seed: inside the seed band).
- Gate: `weft v1 — 25 file(s), valid`; layers green; stamped-time and its negative pass; all 5
  experiments exit 0.

## Pass 7: open items of pass 6 (2026-11-26)

Run: `task sim-explore-from` (seed 1, 1024 walks x 300 steps, 8 threads, settle 12) on main
`4138723` and after the changes below.

- Before: outcomes 1557 / 2444, states 196 / 328, transitions 114 / 221, 0 violations, 531
  `Unsupported` answers (one guard), 239 s.
- After findings 1-2: outcomes 1568 / 2444, states 198 / 328, transitions 117 / 221, 0 `Unsupported` answers,
  686 `idempotency` violations on one command, 200 s. After finding 3: 0 violations, outcomes 1572.
- `weft.node` outcomes 22 -> 34 of 38; `weft.workloads` 262 -> 271. Other domains moved by 1-3
  either way, inside the seed variance of the walk (section "Seed variance of explore").

### Findings and what changed

1. `observability.ExpireLogStream/not-yet-expired` compared `retained_until > now` with
   `retained_until` absent on an Open stream: 531 `Unsupported` answers. The 8 `OpenLogStream`
   answers of pass 6 were gone already (pass 6 fixes). Changed: `defined(retained_until)` first in
   the guard and in the view `LogStreamsPastRetention`. The same guard went before the 3 other
   comparisons of an Optional field with `now` (`pipelines.TimeOutRunnerJob/not-yet`, the view
   `OverdueRunnerJobs`, the window check of `pipelines` deploy gates); explore had not reached
   them.
2. `node.StartProcess/started` had 0 hits (2076 `process-not-declared`): a process name was a
   plain `String`, so the walk never named a process an instance declares. Changed: a newtype
   `weft.shared.ProcessName` for `workloads.Process.name` and every `process` field and input of
   `weft.workloads`, `weft.node` and `weft.observability` (25 declarations). After: `started` 814
   hits, `RestartProcess/restarted` 389, `StopProcess/stopped` 204, `MarkRestartDue/due` 88.
3. Found through finding 2: a repeated delivery of `node.ProcessRestarted` made
   `workloads.RecordProcessRestarted` count the restart twice (`restarts: {increment: 1}`), 686
   violations of the `idempotency` check (binding `r9-record-process-restarted`, at least once),
   an OPEN in `ess/components.yaml`. Changed: the command takes the node's own count of the
   process (`restarts`, `node.LocalProcess.restarts` after the restart) and the instance keeps
   `max(subject.restarts, input.restarts)`: the most restarts of any one of its processes, so a
   repeat changes nothing. `count-above-total` compares the window with the node's count;
   `invalid-count` also refuses a count below 1. Left in the binding's comment: a repeat that
   arrives after the process started again sets health back to Starting until its next ready
   check. After: 0 violations, outcomes 1572 / 2444, 0 `Unsupported`, 0 undeclared refusals,
   248 s; experiments 0 violations in all 5.
4. Walks reach Active assignments now: `nodes.Assign/assigned` 2901 hits, `CompleteDrain/drained`
   34, `RemoveNode/not-empty` 1 (the pool open items of pass 4). A Draining pool going back to
   Active stays an OPEN at `ess/1-infrastructure/infrastructure.yaml:874`.

## Pass 8: isolation, the untied commands of pass 1 (2026-11-26)

Work list: the "Untied" and "FOLLOW-UP" lines of `docs/hardening/pass-1.md` for workloads (:564),
deployments (:559), pipelines (:539) and formations (:514-515). Rule: the frame of pass 1 (anchor,
tie, closed transitively). Branch `hardening/isolation-2`, one commit per domain.

Check (scratch, not committed): a jq script over `ess specify compile --path ess --format json`
lists every command a tenant actor may send whose identifier inputs (operator rows left out) are not
all tied, transitively, to an anchor `<x> != caller.account_id` (input `when`) or
`account_id != caller.account_id` (subject). Plant first: with `secret-of-other-account` of
`AddSecretVersion` (`ess/5-runtime/secrets.yaml:548`) deleted in a scratch copy it reported
`AddSecretVersion untied=secret_id`; on the real tree it reports nothing for secrets.

| domain | untied before | untied after | commands tied | rows given `account_id` |
| --- | --- | --- | --- | --- |
| workloads | 24 | 3 | 21 | Invocation, Job, Session, Schedule, Trigger, WarmPool |
| deployments | 5 | 0 | 5 | Translation, Plan, Release |
| pipelines | 8 | 2 | 6 | RunnerJob, BuildCache, DeployGate, GateCheck |
| formations | 27 | 10 | 17 | Environment |

Every new row field is copied at create (from the workload, source, translation, release, plan,
pool, gate or formation, or from the account input the create already ties) and has a
`references` relation to `weft.accounts.Account`. Every new refusal answers with the not-found
error of the row it guards, after the existence checks and before any state check (no
disclosure). 78 outcomes were added (2444 -> 2522 declared).

### What changed

- workloads: 12 commands addressed by the row get `not-callers-account` on the subject
  (`CancelJob` `ess/4-applications/workloads.yaml:4720`, Attach/Detach/CloseSession,
  Pause/Resume/DeleteSchedule, Disable/Enable/DeleteTrigger, Resize/DeleteWarmPool). 9 commands
  addressed through a workload or an instance get an `account_id` input, an input guard against
  the caller and `workload-of-other-account` or `instance-of-other-account`
  (`WorkloadNotFound`, `NotFound`): DefineService (:3502), CreateSchedule, CreateTrigger,
  CreateWarmPool, OpenSession, SubmitJob, CreateBox, RestoreSnapshot, TakeSnapshot (:5667).
- deployments: TranslateSource (`ess/6-delivery/deployments.yaml:1272`), CreatePlan (:1567),
  PlanRollback (:1669), ApplyPlan (:1813) get an `account_id` input, an input guard and a
  source/translation/release/plan-of-other-account tie; DiscardPlan (:1782) a subject guard. All
  answer `NotFound`. Translation got `account_id` beside Plan and Release, so CreatePlan ties its
  translation in one hop. The four OPEN notes saying these could not be tied are removed.
- pipelines: the 6 commands whose account was on a related row: AddDeployGate
  (`ess/6-delivery/pipelines.yaml:1421`: `account_id` input, input guard, row set
  `environment-of-other-account` at :1515 over the environment's formation,
  `EnvironmentUnavailable`), CancelRunnerJob (:1123), EvictBuildCache (:1388), RemoveDeployGate
  (:1555), ApproveGateCheck (:1745), RejectGateCheck (:1796) (subject guard, `NotFound`, before
  `gate-mismatch`).
- formations: 15 commands anchored on `formation_id` get the secrets pattern (`account_id`
  input, input guard, `formation-of-other-account`, `NoSuchFormation`): RemoveComponent
  (`ess/4-applications/formations.yaml:1192`), RemoveFromBorder, RemoveLink, RemoveEndpoint,
  RemoveRoute, ChangeConfig, RemoveConfig, UnmountConfig,
  ChangeConfigOverride, RemoveConfigOverride, SetScale, ChangeScale, RemoveScale,
  CreateEnvironment (:914), CreatePreviewEnvironment. Environment gets `account_id`, so
  SetEnvironmentQuota (:974) and RetireEnvironment get a subject guard (`NoSuchEnvironment`).
- `sim/experiments/full-setup.yaml`, `stuck-placement.yaml`: `account_id` added to the inputs of
  DefineService and CreateEnvironment (new required input).

### What stays untied and why

- workloads Invoke (:4309), StopInstance (:3301), WakeService (:3972): Instance or Scheduler send
  them too and declare no caller attributes; a caller read is then refused (ESS-COMMAND-004,
  probed). OPEN at each: split the tenant path into its own command, or give those actors an
  account.
- pipelines DisconnectForge (:717), RetireRunnerPool (:841): ESS-COMMAND-009 (a row-set guard
  beside a subject guard), out of this pass as pass 1 marked it.
- formations: the 9 commands Translator also sends, left as the brief says: AddComponent (:1151),
  AddEndpoint (:1480), AddLink (:1279), AddRoute (:1734), AddToBorder (:1372), CommitRevision
  (:880), CreateConfig (:1839), MountConfig (:1983), SetConfigOverride (:2085). Translator now
  declares caller attributes (`ess/9-actors/actors.yaml:28`), so the reason of pass 1 is gone;
  they can be tied the same way in a later pass. CreateExport (:1582): `to_account_id` is
  another account by design (same tenant, external), `endpoint_id` is held by an external
  (`endpoint-not-in-account`).
- formations RetireEnvironment: `releases-live` is `before_row` and answers before the new
  account guard, so another account's environment with a live release answers `ReleasesLive`
  (OPEN at :1123).

### Explore and gate

`task sim-explore-from` (seed 1, 1024 walks x 300 steps, 8 threads, settle 12). Before: the same
weft-sim binary on a snapshot of `4ce56ee`.

| | outcomes | states | transitions | violations | unsupported | undeclared | time |
| --- | --- | --- | --- | --- | --- | --- | --- |
| before | 1566 / 2444 | 197 / 328 | 116 / 221 | 0 | 0 | 0 | 283.3 s |
| after | 1594 / 2522 | 195 / 328 | 114 / 221 | 0 | 1 | 0 | 283.9 s |

30 of the 78 new outcomes were reached (23 ties, 7 subject guards). No input
`not-callers-account` was: weft-sim takes the caller's account from the request's `account_id`
(`tools/sim/src/explore.rs:106`). Outcomes that existed before: 1564 reached against 1566, and no
changed command lost an accepting outcome. States and transitions moved by 2 each, inside the
seed variance. The one `unsupported` answer was
`observability.OpenLogStream/instance-not-on-node` (`ess/5-runtime/observability.yaml:720-723`,
`node_id != input.node_id` on an Instance with no node yet, walk 550), unchanged since pass 7 and
reached because the walks shifted. Fixed by the integrator in this branch, as in pass 7 finding 1:
`any: [not defined(node_id), node_id != input.node_id]` (an instance with no node is on no node),
and the same in the guard at :1167-1171. After, rebased on main `3946ced` (step 2b): 0 `unsupported`, 0
violations, outcomes 1590 / 2525, 288 s.
`task sim-experiments`: 0 violations in all 5. `slot task check`: exit 0.

## Step 2b exercised (2026-11-26)

Before, explore had 0 hits on the `PlaceInstance` guards `cluster-pool-differs`, `group-not-of-node`
and `crosses-closed-link` (PR 13): no walk builds a `Custom` topology with a closed link.

- Experiment `sim/experiments/closed-link.yaml`: one account cluster, two dedicated pools a and b
  with one Ready node each, a `Custom` topology with group A (pool a) and group B (pool b), a
  declared link A-B with `crossable: false`, formation shop with a service of two replicas. Every
  `expect:` is reached: instance 1 on pool a, group A, no peers -> `placed`; instance 2 on pool b,
  group B, peers [A] -> `crosses-closed-link`; group A for the node of pool b ->
  `group-not-of-node`; cluster pool a for the node of pool b -> `cluster-pool-differs`; instance 2
  on pool a, group A, peers [A] -> `placed`. No spec defect: all five answered as written.
- New system check `closed-link` (`tools/sim/src/simulate.rs`, in `ALL_INVARIANTS`, so explore
  runs it after every step): a violation when two active (not Released) placements of one
  formation sit in groups a Declared link with `crossable == false` joins, either direction. It
  reads `weft.scheduling.PlacementDecisions`, which gained `formation_id` and `topology_group_id`
  (`ess/5-runtime/scheduling.yaml`), and `weft.cluster.TopologyLinks` of the account of each
  formation placed in two groups or more. KPI `closed_link_pairs`.
- Tests (`tools/sim/tests/main.rs`, fixture `tests/fixture-closed-link`): with the guard, the
  crossing placement is refused and the check passes (open link, another formation, a released
  placement beside a live one in the closed pair); without it (guard lines cut from a copy), one
  violation at the crossing step. 0.01 s each. On a scratch copy of `ess/` without
  `crosses-closed-link`, the experiment's second placement is `placed` and the check fires.
- Numbers: `task sim-experiments` 0 violations in all 6. `task sim-explore-from`: 0 violations,
  0 `unsupported` or `undeclared` errors, outcomes 1557 / 2447, 640 walks in the 280 s cap at a
  load average of 45-60. Beside it, with `--invariants` without `closed-link` at a load of 19-29:
  960 walks, outcomes 1571; the walk count follows the host load, the check costs under 1 ms a
  step. `slot task check` exit 0, test step 0.2 s.
