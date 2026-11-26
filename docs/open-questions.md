# Open questions from the first specification pass (2026-11-26)

Each line: source file, question, what the file assumes for now.

## protocols/exec-channel.yaml

- Attach, logs, port forward: separate protocols or variants of exec? Assumed: only exec modelled.
- How weft-api learns a node session is lost: from weft-control, or a broken stream? Assumed: an
  api input `session_lost`.
- A running exec process when the node session is lost: kill or keep for reconnect? Not modelled.
- Cancel answered at once or after the node confirms? Assumed: after the node's `killed`; an
  `exited` or `failed` arriving first wins.
- Output after a cancel: sent or dropped? Assumed: dropped at the api.
- TTY flag and window resize? Not modelled; stdin is one chunk.
- Reasons carried by `failed`? Assumed: none.
- Limit of `ess-protospec/1`: "at least one outcome, never silence" cannot be checked, only "at most
  one". An api that never sends `lost` still passes explore.

## ess/domains/secrets.yaml

- Shared IDs requested: `SecretBackendId`, `SecretDeliveryId` (declared locally for now).
- `latest` = newest version by `created_at` (ESS refused a latest-pointer update from the version command).
- Secret name charset/length; max value size; binary values?
- Allowed file paths and env var names for delivery.
- Backends per cluster or per account? Assumed: per cluster.
- Auth to an external vault? Assumed: no credential stored.
- Retiring a backend, moving values, deleting a secret: not declared.
- Re-enable a disabled version? Assumed: no. `latest` with a disabled newest version: refused.
- Grants by command or derived from a release? Assumed: command.
- Does revoking a grant stop instances already holding the value?
- Retention of delivery records.
- ESS: uniqueness guard over two empty optional fields may miss duplicate names in account/formation scope.

## ess/domains/identity.yaml

- Shared IDs requested: `TrustDomainId`, `IdentityDocumentId`, `ExchangePolicyId`, `TokenExchangeId`.
- Missing actor for issuing and expiring documents: added `ControlPlane` system actor to the sketch.
- Trust domain naming? Assumed: operator supplies it.
- X.509 and JWT documents both at start, or JWT on request? Assumed: caller names the format.
- Lifetime bounds. Assumed: 60 s to 3600 s.
- Trust bundle and revoked serials reach peers over the node channel (assumed).
- Key pair made on weft-node; only a signing request leaves it (assumed).
- Key rotation: operator, by hand (assumed). Renewal any time before expiry (assumed).
- Token exchange: the outside service mints the token (assumed). Refused exchanges not audited (assumed).
- Exchange policy writers: admins and tenant owners, not deployers (assumed).
- Withdrawing a policy does not revoke issued tokens (assumed).
- Refused mutual TLS: recorded where? Open.
- Ownership of IdentityDocument by TrustDomain, and of TokenExchange: written as references, unconfirmed.
- SPIFFE, WIMSE, RFC 8693 details marked "(unverified)" in the file.

## protocols/node-channel.yaml

- `explore` ends inconclusive (state bound) because the heartbeat timer never ends; fix in integration: model an end of heartbeats or split the actions file so `max_steps` can drop.
- Timer values chosen by the agent: heartbeat 1000 ms, lease 2500 ms, redial 3000 ms.
- Left out: Pause, Snapshot, Drain, Attach, Detach, IssueIdentity, DeliverSecret, NodeObserved, UsageReported.
- Reported phases `absent`, `started`, `exited`: agent choice.
- Known-node decision modelled as inputs `admit`/`refuse`.
- A new hello while control holds a session replaces it (assumed).
- Reconnect: control re-sends Assign if the node never got it; after lease end a node still running gets Stop (assumed).

## ESS limits met (candidate ESS issues)

- `ess-protospec/1`: no liveness ("at least one outcome"), no not-equal guard, no arithmetic, no session identity, a broken session cannot clear queued messages, one instance only.

## ess/domains/access.yaml

- Shared requested: `CliLoginId`, `AuthorizationDecisionId`, `AccessMode` (enum, cluster uses it too), `RoleName` (accounts decides).
- Missing actors: pre-login CLI (added `Anonymous`), weft-api acting on its own (added `ManagementApi`) to the sketch.
- Access mode owned by access, one row per cluster; no switching (assumed).
- Single-user implicit admin: operator names an existing principal (assumed).
- CLI holds a weft session token, not IdP tokens (assumed). Lifetimes: cluster settings.
- Browser deny of a login: `Denied` state. API token does not reach sub-accounts (assumed).
- Audit rows kept, read through views only; call input not recorded (may hold secrets).
- Admin cannot revoke another principal's login; a token call cannot create a token (assumed).
- Calling principal is an input for now; integration switches to the caller value.
- Synthesis: 227 constructs unscenarioed, mostly server-made codes/digests/expiries.

## ess/domains/formations.yaml

- Model: topology edited row by row; `CommitRevision` freezes rows into an immutable revision (agent choice).
- Shared candidates: `ComponentId`, `Port` (1..65535), `QuotaAmount`.
- Translator is an actor of topology commands; add `Translator` to actors (sketch has it).
- Environment names open set; link protocols Http, Grpc, Tcp, Udp (assumed).
- Private network address range: not modelled; `<formation>.internal` minted by implementation.
- Formation and environment lifecycle Active -> Retired; retire refused while a release is live.
- Links never cross formations or accounts (assumed). Endpoints per formation, not per environment.
- Route path: prefix match. Environment config replaces, not merges.
- Nothing stops a secret value in a Config (gap).
- Empty commit still records a revision (assumed).
- Host ownership proof: external check, refused by default.

## ess/domains/cluster.yaml

- Decided by integrator: the Scheduler may request pool scale-up/down and drain idle nodes (sketch section 10); the operator sets pool bounds. Brief said operator-only; sketch wins.
- Shared requested: `AccessMode`, `Capacity`, `ResourceAmount`, `IsolationFacts`.
- Name clash: type `ControlPlane` vs actor `ControlPlane`; rename the type in integration.
- Access mode follows placement: single-user only with local; local = 1 control replica (agent decision).
- Cluster policy content beyond minimum isolation? Capacity units per class? Devices (GPU models) in Capacity?
- Isolation facts: also name the levels a node can provide?
- Cluster decommission: never ends for now. Zone names unique across clusters (assumed).
- Pools make worker nodes only; pool image: weft artifact digest or provider image?
- Lost-session node must drain before Gone (assumed). Lowering cluster minimum isolation refused.
- 3 control replicas need 3 control nodes? Pool change applies to new nodes only (assumed).
- Join token max 24 h (agent choice); returned once; checked before node record is read.
- Local cluster refuses a second node? Stale observations refused? Draining the last control node refused?
- mTLS certificate for a joining node: what it presents, who signs.
- Synthesis: 387 scenarios, 251 refusals (filters, `now` checks, row counts).

## ess/domains/deployments.yaml

- Missing actor: `ReleaseController` (release steps, live marking, drift) — added to sketch.
- Goalpost check: `KubernetesKind` enum with map/refuse/OPEN; `KubernetesCoverage` view counts per kind.
- Secret values in helm values: refused (`SecretValueInValues`). Bare Pod, CRDs, operators, RBAC, unlisted kinds (Namespace, PV, StorageClass, webhooks), hostPID/IPC: refused.
- Helm pre-delete, post-delete, test hooks: dropped, mapped with loss.
- Plan diff covers component names only; drift reported, never repaired (assumed).
- Translator verified by artifact digest; caller picks translator; failed translation retryable.
- Stale plan (live release changed after planning) not detected — gap.
- Rollback cannot skip simulation (assumed).
- ESS 0.99.2 `unsupported_construct`: two checks on steps in one command; MarkReleaseLive split in two.
- Determinism and "translator grants no more than declared" not expressible in this domain alone.

## ess/domains/nodes.yaml

- Shared requested: `NodeSessionId`, `ExecSessionId`, `CapacityEntry`, `IsolationFacts` (cluster wants the same).
- `LoseNodeSession` actor: ControlPlane (system actor).
- Isolation facts hold the levels a node can provide (assumed; cluster asked the same).
- Fencing: a node that cannot renew a lease stops the instance itself? Left to protocol.
- Node redial while old session Open: refused until marked lost (protocol agent assumed: replaces). CONFLICT to settle.
- Heartbeat timeout 30 s here vs 1000 ms heartbeat / 2500 ms lease in protocol: settle values.
- Lease per assignment, length set by control plane. Resume added. Failed does not end an assignment.
- Assign isolation check needs "latest observation lists level": external refusal for now.
- Synthesis: 90 constructs unscenarioed.

## ess/domains/scheduling.yaml

- Shared requested: `SchedulingRequestId`, `GangId`, `ScaleRequestId`.
- Placement in two steps (Reserved, then Bind) because ESS forbids `affects:` beside `creates:`.
- Preemption recorded on the victim PlacementDecision (Evicting + fields), no own entity.
- Spot = lowest preemptible priority class (assumed). Priority classes cluster-wide; cross-tenant preemption allowed (assumed).
- Tenant rule: only MicroVm counts as own kernel; other levels never share a node across tenants (assumed).
- Zone rules = allowed zone list only, no spread/affinity. Claim `sharing` not read yet.
- Scheduler names binding ids that resources reserved (assumed). Preempted instance gets a new request from workloads.
- No pending timeout; which pool a scale-up asks; idle time before scale-down: open.
- Who answers scale requests: cluster domain or scheduling? Settle with cluster (scheduler may scale pools).
- ESS limit: row-set count compares only with a literal, so BindGang cannot check gang size.

## ess/domains/accounts.yaml

- Shared requested: `RoleBindingId`, `QuotaId`, `ExportId`.
- Quota resize = release then draw (ESS arithmetic only on the addressed row); limit is 0 in between. ESS limit candidate.
- EffectiveRoles view cannot walk the account tree; authorizer must apply bindings downward. ESS limit candidate.
- Names: a-z, 0-9, `-`, 1-63 chars (assumed). Roles fixed set; `billing` reads usage, quotas, prices.
- No account moves, no stored path, one user in one tenant only (assumed).
- Parent usage does not lower its remaining quota (assumed). Class without quota row = 0.
- Closing an account with running workloads; returning its quotas: not handled.
- Owner grants all roles; admin grants viewer, deployer, admin, billing.
- Exports: same tenant only, no acceptance step, not inherited by sub-accounts (assumed).
- Synthesis: 37 scenarios, 92 refusals.

## ess/domains/catalog.yaml

- Shared requested: `OfferingPlanId`, `PublisherId`, `BackupId`, `RestoreId`, `SleepPolicy`, `SleepKind` (workloads uses them too).
- No lifecycles given for engines, plans, publishers, products, versions, installations (agent chose minimal).
- Retired offering with live instances; engine swap moves instances automatically? Open.
- Durability levels beyond yes/no; what a plan limits; may a plan forbid sleep?
- How system and installation formations are created (deployments integration).
- Bindings per environment? `admin` binding mode needs a higher role?
- Backups: retention, on deprovision, of a Degraded instance; restore into another instance.
- Product secret parameters; third-party publisher trust; source check at publish; uninstall data; downgrade allowed?
- ESS limits: subject guard beside row-set guard refused; list guard beside related guards refused; two-hop checks -> external refusals.
- Durable -> ephemeral plan change not refused (needs two rows in one guard).

## ess/domains/resources.yaml

- Bucket and filesystem kept here; catalog object-store overlap to settle in integration.
- Binding in two steps (reserve, commit all-or-none) because of ESS create+affect limit.
- Network classes are rows only; claim holds domain name and egress allow list; binding holds address.
- Egress destination: one host + port (assumed); CIDR/wildcard?
- Operator defines every class (assumed). Amounts integer in base unit (unit not stored).
- GPU wipe after every binding (assumed). Node agent reports slicing; sliced GPU never bound whole.
- Faulted bound device keeps binding Active; scheduling moves the instance.
- Binding does not check device vs claim class/model/amount; scheduler trusted (gap).
- Readers cannot stay attached while a writer attaches; snapshot only without writer (assumed).
- Filesystem zone-scoped; bucket names cluster-unique; non-empty bucket delete refused (assumed).
- `cache` class = node scratch (assumed). Domain ownership proof and certificates: external check, not modelled.
- Relations all `references`; Binding->Claim, VolumeSnapshot->Volume could be `owns`.

## ess/domains/metering.yaml

- Shared requested: `PriceListId`, `PriceId`, `DeadlineId`, `BudgetId`, `ConcurrencyId`, `RateId`, `PricingAssignmentId`, `StatementId`, `OfferingPlanId`.
- Prices keyed per meter (sketch said both per class and per meter): settle in sketch.
- One currency per price list; calendar-month billing; spot = factor of list price (assumed). Scheduling treats spot as a priority class: align.
- One price list per cluster; price at record write time; unpriced meter not recorded (assumed).
- Budget does not renew; spent-budget stop grace belongs to workloads.
- Concurrency counter can leak; acquiring several limits not one transaction (gap).
- "Children inherit" pricing cannot be expressed (tree walk). ESS limit candidate.
- Statement lines and totals computed outside; sum unchecked. Corrections undefined.
- RecordUsage takes account path, price as inputs (trust in weft-control).
- Budget charge applied twice? Quota check against usage or held claims? Open.
- ESS limits: `accepts: nothing` cannot emit an event; missing-row refusal precedence (ESS-COMMAND-004).
- Synthesis: 85 unscenarioed, 21 unanswered.

## ess/4-applications/workloads.yaml

- Shared requested: `WorkloadId`, `RuntimeId`, `ServiceId`, `SessionId`, enum `Sharing`.
- Guards cannot compare enum order: each isolation minimum lists the weaker levels (ESS limit candidate; `ordered: true` exists for min/max only).
- Actors for autoscale, sleep, wake, calendar: Scheduler or NodeAgent (agent choice).
- Workload belongs to a revision or a live release? Which kinds count as untrusted code (MicroVm default)?
- Signature format, trusted signing keys, artifact revocation: open.
- Daemon node matching; calendar syntax; Trigger kinds; session idle timeout; invocation payload size.
- Box: no suspending state; resize while started; max lease length.
- Exec needs its own role beyond Deployer?
- What marks a Service as answering requests for `min: 0`?
- Does a warm instance hold its secrets before it serves?
- Ownership of instances by Job/Box/Session/WarmPool: all `references`.

## ess/0-foundation/artifacts.yaml (moved from workloads)

- `ArtifactKind` copied from `workloads.PackageKind` (layer 0 may not name workloads); add a kind for formation documents and deployment sources (decided in integration 2).
- Artifact checks in workloads are now `external:` outcomes; in `DefineWorkload` runtime errors now answer before artifact errors.
- Signature format and trusted keys; revocation (not in the sketch): open.

## Export moved accounts -> formations

- Decided: `RemoveEndpoint` refuses while an active export names the endpoint (integration 2, formations).
- `CloseAccount` no longer revokes exports: who reacts to `AccountClosed` (an event reaction) is open.
- `CreateExport` account checks are now `external`; NotFound and Closed share `NoSuchAccount`.

## Integration 2 follow-ups (integrator)

- `weft.shared.ClusterId` is the identity type of both `cluster.Cluster` and `access.ClusterAccess`; ESS then cannot infer the relation `identity.TrustDomain.cluster_id -> cluster.Cluster`. Decide after the access pass: ClusterAccess keyed by its own id with a `cluster_id` relation, or merged into Cluster.
- Stale comments: cluster `RequestNodePoolScaleUp`/`DrainNode` ("operator only"), identity lines 29-30 ("declared here until moved").
- Hardening candidate: now that relations exist, `external` refusals that only stood in for "the row exists / belongs to the account" can become `when_related` guards (metering markers 355, 1071, 1829; artifact checks in workloads; account checks in formations `CreateExport`).
- Add `weft.shared.Port` (Integer 1..65535) and a non-negative amount rule on `ResourceAmount`; then drop the local copies (formations `Port`, `NegativeQuotaAmount` guards).
- Account close vs active exports: formations reacts to `AccountClosed`, or accounts refuses to close with active exports? Open.

## actors (wave 3, 2026-11-26)

- decided: `IssueIdentityDocument` and `DeliverSecret` run by ControlPlane (sketch § 11 and the ControlPlane line of the draft win over the per-domain lines).
- decided: Viewer may run `CloseExecSession` (closes the logs session it opened). Applied by the integrator.
- decided: view grants for every domain come in the views pass (goal item 3), not now.
- OPEN: `system: true` on system actors makes conformance wait for their commands instead of running them; decide in the soundness pass from the synthesis counts.
- OPEN: `RecordServiceAwake`: NodeAgent (draft) or Scheduler (workloads.yaml comment).

## observability (wave 3, 2026-11-26)

- 28 `# OPEN:` markers in `ess/5-runtime/observability.yaml` (store and query API, stream split, metric names, default retention, plan retention limit, sink auth and egress, webhook signing, edge/link series, rule over label sets, silencing, sampling of instance spans).
- decided: ScaleRule and SleepPolicy name a series by value (sketch line 630 fixed).
- decided: actor grants (actors.yaml, "integrator, 2026-11-26"): NodeAgent opens and closes log streams and registers series; ControlPlane expires, retires, fires, resolves, records delivery; account roles TenantOwner, AccountAdmin, Deployer, Automation write sinks, targets, rules, policies; Instance registers its own custom series.

## health (wave 3, 2026-11-26)

- Health is a field `Instance.health` (`InstanceHealth`), not a second lifecycle. 7 `# OPEN:` in workloads.yaml, each with the default the author took (timeout may exceed period; backoff factor 2; one ready check per instance; sidecar restart takes the instance out of traffic).
- `Check` and `RestartPolicy` lengths are now `*_seconds` numbers: translators and surfaces must use those names.
- TODO: formations (edge, links), catalog (service bindings) and deployments (rollout step) read `ReadyInstances` / `ComponentReadyInstances`.

## components and bindings (wave 3, 2026-11-26)

- decided: 2 ESS components (weft-api: access, accounts, artifacts, formations; weft-control: the rest). weft-node and weft-edge are protocol peers (ESS-COMPONENT-007 refuses a component without a domain). Sketch § 14.
- decided: loop steps take one of four forms (edge binding, level gap view, time guard, many-rows command); sketch § 14. Wave 4 adds the fields and commands per domain.
- decided: no `system: true` on system actors (it would refuse every binding).
- decided: the node agent `Record*` call is the one source of an observation (R4 question).
- OPEN: split `secrets` and `deployments` between weft-api and weft-control, or keep whole in weft-control (taken: whole).
- OPEN: EvictInstance over the disruption budget when the reason is NodeLost (taken: allowed; a lost node is not a voluntary disruption).

## nodes (wave 4, 2026-11-26)

- decided: ControlPlane holds the lease clock; it may run `ReleaseAssignment` (Scheduler keeps its grant for voluntary release).
- R4 edge: `PlacementBound` -> `Assign(instance_id, node_id, placement_decision_id)`; Assign finds the open session itself. R2/R3: `AssignmentEnded`/`AssignmentReleased` carry `placement_decision_id`; level views `StoppedActiveAssignments`, `ExpiredLeases`.
- ESS refuses `now` in an `affects:` filter: time-form steps run once per row.
- TODO scheduling: `BindGang` emits only `GangBound`; gang members get no `PlacementBound`.
- OPEN: PlacementDecision stores the requested minimum isolation beside the chosen level?

## account close (wave 4, 2026-11-26)

- decided: account close proceeds; R6 bindings on `AccountClosed` revoke exports (`RevokeAccountExports`), API tokens (`RevokeAccountApiTokens`), telemetry config (4 observability commands; sinks and targets retry until policies and rules are gone).
- Instance ended (`InstanceExited`/`Failed`/`Evicted`) -> `CloseInstanceLogStreams`, `RetireInstanceMetricSeries`.
- ESS-COMMAND-009: `affects:` beside `instances:` refused; one many-rows command per entity.
- Set-effect commands: generated Rust/Go and Entity Runtime do not support them yet (author report; not checked).

## scheduling and claims (wave 4, 2026-11-26)

- R1/R3 edge: `InstanceCreated`/`InstanceEvicted` -> `SubmitSchedulingRequest` (workloads adds account_id, formation_id, claims, zones, priority, minimum_isolation). R4 level: `UnassignedPlacements` + `InstanceAssigned` -> `RecordPlacementAssigned` (integrator added `InstanceAssigned.placement_decision_id`). R5 many rows: `ClaimReleased` -> `ReleaseClaimBindings`.
- Contract change: `SubmitSchedulingRequest` reads the tenant from the account (no `tenant_id` input); claims are `List<weft.workloads.ClaimRequest>`.
- OPEN: gang Job instances in the R1/R3 binding: skip, or carry the gang id on Instance (taken: skip until gangs get their own loop).
- OPEN: `ReleaseClaim` on a Bound claim (taken: refused; release the binding first).

## deployments (wave 4, 2026-11-26)

- decided: R1 starts at `ReleaseStepStarted` (kind Rollout, carries workload_id, replicas), not at release live (sketch fixed). Rollout waits via `RunningRolloutSteps` + `CompleteRolloutStep(ready_replicas)`.
- R7: `DriftDetected` -> `ReapplyDrift` (escalate emits `DriftEscalated`) or `EscalateDrift`; level view `OpenDrifts`.
- 6 OPEN in deployments.yaml (daemon replicas, escalated-drift resolution, failed reapply, escalate state record, trusted ready count, who ends old instances).

## workloads loops (wave 4, 2026-11-26)

- R1 level: `ServicesBelowScale` -> `RequestReplica` -> `ReplicaRequested` -> `CreateInstance`; scale-in `ServicesAboveScale` -> `StopInstance`. R2: `InstanceExited`/`Failed` -> `RecordReplicaEnded`; time `ServiceBackoffsDue` -> `EndReplicaBackoff`. R6: `StopAccountInstances`, `RetireAccountServices`, level `PendingStopsToWithdraw`. R8: `WakesOverdue` -> `MissWake`. R9: `AliveCheckFailed` means past threshold.
- ESS-COMMAND-009: no set effect beside a create; no `{increment:}` on a set effect. The replica counter is kept by `RequestReplica`/`RecordReplicaEnded`.
- decided: R3 is evicted -> `RecordReplicaEnded` -> R1 creates a new instance whose `InstanceCreated` carries the scheduling request (the scheduling comment that sends `InstanceEvicted` to `SubmitSchedulingRequest` is replaced).
- decided: Scheduler runs `RequestReplica`, `EndReplicaBackoff` and the `PendingStopsToWithdraw` stops.
- decided: the `InstanceFailed` binding maps `backoff: true` (a rejected artifact does not loop without a wait).
- decided: scale-in stops the highest replica index.
- decided: `Formation.zones: List<ZoneName>` and `Workload.priority: PriorityClassName` (the two UNMAPPED homes).
- OPEN: a double `RequestReplica` for one index leaves the count one high; Daemon, Job tasks, WarmPool have no level loop; account close vs boxes, sessions, jobs, schedules, triggers, warm pools.

## bindings (wave 4, 2026-11-26)

- 34 bindings; commands without component 40 -> 0, events 45 -> 0, commands without actor 16 -> 0. 12 gap views granted to Scheduler, ControlPlane, ReleaseController.
- decided: PriorityClass moves to `weft.cluster` (layer 1) so `Workload.priority` points down; `SetWorkloadReplicas` closes R1 (agent running).
- OPEN (components.yaml, 19 lines): escalation events outside deployments (`no-claims`, `isolation-unavailable` -> `AssignmentRefused`, `count-empty` -> `ReplicaCountDrifted`); gap views for `ExpireLogin`, `ExpireApiToken`, `ExpireIdentityDocument`, `ExpireInvocation`; Jobs outside gangs skipped by scheduling binding; R9 restart lives on the node.
- done: PriorityClass in `weft.cluster`; `Workload.priority_class` references it; `SetWorkloadReplicas` + binding `r1-set-workload-replicas` close R1. OPEN: PriorityClass per cluster; DefineWorkload does not check the class; r1 refusals over bounds have no escalation event.

## soundness formations (2026-11-26)

- 0 failed (was 16). 001/003/004 = 4/74/54 remain, mostly ESS synthesis gap 9 in `docs/ess-gaps.md`.
- OPEN: `RetireEnvironment` checks a live release before the environment exists (`before_row: true`).
- 14 external refusals kept: releases-live x2 (layer 6), over-quota x4 (list arithmetic), host-not-verified, export-not-permitted, 6 account checks (converting them hits gap 9).
- done: `ResourceAmount` invariant `amount >= 0` (integrator).

## soundness resources (2026-11-26)

- 0 failed of 61 resources scenarios. decided: restore-from-snapshot is its own command `RestoreVolume` (CreateVolume lost `source_snapshot_id`); integrator added it to components and 3 actors.
- `DeleteVolume/has-writer` reads `Attached`; filesystem/bucket in-use stays external (claims moves no state on them). Kept external: quota-exceeded x5 (metering above), bucket not-empty (object store).
- ESS gap: synthesis cannot arrange a snapshot row whose fields `SnapshotVolume` copies from the volume (probe in job scratch res-probe).
- TODO claims: `CommitVolumeWriter/committed` unreachable (`claim-missing`).

## soundness resources (2026-11-26)

- 0 failed of 61 resources scenarios. decided: restore-from-snapshot is its own command `RestoreVolume` (CreateVolume lost `source_snapshot_id`); integrator added it to components and 3 actors.
- `DeleteVolume/has-writer` reads `Attached`; filesystem/bucket in-use stays external (claims moves no state on them). Kept external: quota-exceeded x5 (metering above), bucket not-empty (object store).
- ESS gap 12 (`docs/ess-gaps.md`): a row whose fields are copied from another row cannot be arranged.
- TODO claims: `CommitVolumeWriter/committed` unreachable (`claim-missing`).

## soundness nodes (2026-11-26)

- 0 failed in nodes. 001/003/004 = 14/33/20, mostly: no Assignment can be created because `scheduling.PlaceInstance/placed` is unreachable, because `accounts.CreateAccount/created` is unreachable (ESS gap 9).
- New view `ExecSessions`; `Assign/node-not-ready` reads the cluster Node (external 13 -> 12); kept: 9 client-certificate checks (mTLS), isolation-unavailable, binding-not-claimed, exec not-permitted.
- ESS gaps 13-15 added.

## soundness accounts (2026-11-26)

- 0 failed; 63 outcomes without scenario, 58 from ESS gap 9 (root cause named: `selects` in `ess/crates/verify/ess-conformance/src/synthesize/related_guard.rs:2026` never evaluates a `when_related` row-set guard). No Account, Quota or RoleBinding can be arranged: this blocks scheduling, nodes, formations, claims downstream.
- New views `RoleBindingIndex` (granted ManagementApi), `QuotaIndex` (granted ControlPlane, Scheduler).
- Kept external: BindRole and BindTenantOwner grant-not-permitted (tree walk; caller principal not an actor attribute).

## soundness catalog (2026-11-26)

- 0 failed; 104 outcomes without scenario, about 98 from ESS gap 9 (probe in job scratch catalog-probe, under 1 s). Subset synthesis took 474-526 s (12 domains).
- External 12 -> 10. Contract change: `BindService` takes `account_id` (cross-account refused by guard). `engine-wrong-kind` written per OfferingKind value.
- Kept external: quota-exceeded x4 (tree walk), mode-not-offered, invalid-parameter-value x2 (JSON Schema), required-offering-unavailable x2 (second hop), PublishProductVersion no-such-offering (list guard refused).

## soundness metering (2026-11-26)

- 0 failed (was 2); external 1 -> 0 (`RecordUsage` reads `weft.nodes.Assignment`; new optional `assignment_id`). 41 outcomes without scenario (was 46), causes ESS limits; matches active ESS story `a-creator-under-a-row-set-and-when-is-arranged`.
- Subset synthesis 499-583 s (largest subset).
- OPEN: dropped `FreeTierTerms` rule `allowances.count >= 1` (synthesizer built only empty lists); `now - 1s` guard workaround on future-period/period-not-ended; AssignPricing already-assigned split per target kind.

## soundness access (2026-11-26)

- 0 failed in access; 29 outcomes without scenario (was 31). External 7 -> 6 (`unknown-principal` reads Principal; new refusal `principal-not-user`: only user principals get a CLI login).
- decided: login codes and expiry stay server-set (RFC 8628); the login flow gets covered by synthesis seeds, not by client-chosen codes.
- TODO integrator: seed pass. Authored rows in `ess/scenarios/` + `--synthesis-seed` in `tools/synth-domain.sh` and the gate: federated ClusterAccess, Pending and Approved CliLogin with known codes, expired ApiToken, an Account, a Price.
- TODO accounts: CreateAccount/CreateSubAccount look the tenant up with `where:`; access pass suggests `when_related via input.tenant_id` (accounts pass found via + row set hits gap 9; retry after the ESS fix).
- Kept external: identity-rejected x2 (IdP), token-caller, lifetime-too-long, role-exceeds-caller, account-unavailable (until gap 9 is fixed).

## soundness inventory, artifacts (2026-11-26)

- 0 failed in both. artifacts: 0 refusals. inventory: 8 x 003 on ReportDevice + 5 x 004, gap 9 variant (row set `known` + `via` over an optional parent reference); probe in scratch ess-probes/gap9b-inventory.
- decided: RegisterArtifact gets an account check (deny by default) after the ESS fix.
- Kept external: RetireResourceClass in-use (claims above), RegisterArtifact bad-signature (crypto outside weft).

## soundness cluster (2026-11-26)

- Join token does not leak (0 ESS-CF-DISCLOSURE/PAYLOAD); the failures were `workloads.RecordSnapshotTaken/no-artifact` (workloads pass has it). 22 unsupported -> 0 (`defined(node_pool_id)` in DeleteNodePool/pool-has-nodes); pool-size refusals apply only to Active pools.
- Kept external: JoinNode token-invalid (token not stored), RecordNodeObservation foreign-node (mTLS), RemoveNode not-empty (nodes above).
- Seeds needed: a used join token, an expired join token. ESS: a command never sets up its own repeat call (`token-used`).

## soundness observability (2026-11-26)

- 0 failed in observability; 77 outcomes without scenario, mostly gap 9. Two duplicate checks never fired (absent == absent is unknown): new `events-already-open`, `formation-policy-exists`. `MetricSeries.identity_document_id` stored.
- 7 `*Index` views over all accounts granted to ControlPlane (integrator).
- Seeds needed: bulk commands (008) x6, `SetTelemetryPolicy/unknown-environment`.

## soundness observability (2026-11-26)

- 0 failed in observability; 77 outcomes without scenario, mostly gap 9. Two duplicate checks never fired (absent == absent is unknown): new `events-already-open`, `formation-policy-exists`. `MetricSeries.identity_document_id` stored.
- 7 `*Index` views over all accounts granted to ControlPlane (integrator).
- Seeds needed: bulk commands (008) x6, `SetTelemetryPolicy/unknown-environment`.

## soundness secrets, identity (2026-11-26)

- 0 failed in both. External: secrets 7 -> 5, identity 5 -> 2; stored checks replace them, so scenarios dropped (secrets 17 -> 8, identity 39 -> 23) until gap 9 and `workloads.DefineWorkload/defined` (Runtime row) are fixed.
- Contract: `DeliverSecret.identity_document_id` (stored, related); `IssueIdentityDocument.workload_id`. New refusal `document-of-other-account`.
- OPEN: DeliverSecret accepts only Starting instances; account-scope secrets do not reach sub-account instances (taken: deny).
- Seeds needed: expired identity document, rotation past its end.

## soundness claims (2026-11-26)

- 0 failed in claims. External 7 -> 4 (class checks read inventory ResourceClass). CommitBinding (8) and CommitVolumeWriter (6) still unreachable: ESS gaps 19-20.
- decided: an unfiltered `BindingIndex` view, granted to ControlPlane and Scheduler only (same pattern as accounts and observability index views); integrator adds it.
- Kept external: egress-denied, domain-not-verified (DNS), quota-exceeded (metering above), BindClaim device-missing (list guard must stand alone).

## soundness scheduling (2026-11-26)

- 0 failed (BindGang fixed). External 9 -> 5 (PlaceInstance reads cluster Node; RequestScaleUp reads NodePool). Remaining refusals: gap 9 (no Account) blocks SubmitSchedulingRequest and all downstream.
- Kept external: no-capacity (sum over rows), quota-exceeded (metering above), device-not-wiped, RequestScaleDown node-not-in-pool / pool-at-minimum (4 guard forms failed).
- ESS: no sum over rows in a guard (gap 6); nested row check in BindGang/member-waiting.

## soundness workloads (2026-11-26)

- 1721 scenarios, 1721 passed, 0 failed (was 4). External 12 -> 8. Check/RestartPolicy bounds moved from types to invariants on Workload and Instance (named refusals became reachable). `no-main` rewritten equivalently.
- Kept external: DefineWorkload artifact checks x4 (as guards synthesis ran 40 min and 16 GB), EvictInstance over-budget (count vs stored value), AutoscaleService over-budget (metering above), CreateTrigger not-on-border, quota group.
- Remaining: Instance 69, Service 52, Schedule 4 (gap 9); Job, Box, Session, Snapshot (the runtime/package-kind guard leaves 1-4 workload setups).

## CI/CD (design:ci-cd-support, 2026-11-26)

First line of each marker in the new entities and commands; the full text and the default taken are at the cited line.

- `ess/2-tenancy/access.yaml:317` OPEN: the design gives no end of a trusted issuer. Taken: Withdrawn, refused while an active
- `ess/2-tenancy/access.yaml:341` UNMAPPED: whether a tenant owns its trusted issuers (removed with the tenant) or only
- `ess/2-tenancy/access.yaml:382` OPEN: an absent workflow or forge environment matches any; a present one must equal the
- `ess/2-tenancy/access.yaml:392` UNMAPPED: "a rule for a production environment": weft.formations.Environment has no
- `ess/2-tenancy/access.yaml:397` UNMAPPED: "limited to named formations and environments". FormationId and EnvironmentId
- `ess/2-tenancy/access.yaml:429` OPEN: what a close of the rule's account does to its rules (section 14 R6 names tokens).
- `ess/2-tenancy/access.yaml:444` UNMAPPED: whether the exchanged token is a weft.access.ApiToken row (so a call made with it is
- `ess/2-tenancy/access.yaml:449` OPEN: a refused exchange changes nothing, so it leaves no row here. Whether refused attempts
- `ess/2-tenancy/access.yaml:451` OPEN: may an exchanged token be revoked before it expires (within token_lifetime_seconds)?
- `ess/2-tenancy/access.yaml:1221` OPEN: which role trusts an issuer for a tenant. Taken: the TenantOwner (actors.yaml,
- `ess/2-tenancy/access.yaml:1305` OPEN: the upper bound of a rule's token lifetime cap; the design gives only the default of
- `ess/2-tenancy/access.yaml:1433` OPEN: does the CLI name the rule, or does weft-api search the issuer's rules for a match?
- `ess/3-resources/artifacts.yaml:97` UNMAPPED: the design says "builder = pool"; no source says how a pool is spelled as a builder
- `ess/3-resources/artifacts.yaml:99` UNMAPPED: the run is the forge's run id (text). Whether the attestation also names the
- `ess/3-resources/artifacts.yaml:102` OPEN: the signature of the attestation itself (SLSA envelopes are signed). Taken: a detached
- `ess/3-resources/artifacts.yaml:104` OPEN: may an attestation be withdrawn (a compromised builder)? Taken: no; append-only. A
- `ess/3-resources/artifacts.yaml:267` OPEN: which actor records it; the design says only that the job's artifact gets one. Taken:
- `ess/6-delivery/pipelines.yaml:42` OPEN: the end of a connection; the design gives none. Taken: Disconnected, refused while an
- `ess/6-delivery/pipelines.yaml:44` OPEN: how a webhook is authenticated (the forge's webhook signature) is the edge's, before any
- `ess/6-delivery/pipelines.yaml:91` UNMAPPED: "a runner workload (image or Box)": no source says whether the pool names an
- `ess/6-delivery/pipelines.yaml:98` OPEN: `max_runners` is the controller's bound when it starts runners (QueuedRunnerJobs); ESS
- `ess/6-delivery/pipelines.yaml:256` UNMAPPED: whether evicting the cache deletes its volume (the cache owning the volume) or
- `ess/6-delivery/pipelines.yaml:281` UNMAPPED: "per pool" reads as the pool owning its caches (no meaning without it), but no
- `ess/6-delivery/pipelines.yaml:316` UNMAPPED: whether an environment owns its gates (removed when it is retired); written as
- `ess/6-delivery/pipelines.yaml:318` OPEN: a window is one absolute interval (opens_at, closes_at). Whether it recurs (weekdays,
- `ess/6-delivery/pipelines.yaml:320` OPEN: may a gate be changed in place? Taken: no; remove it and add another.
- `ess/6-delivery/pipelines.yaml:705` OPEN: may a pool that runs only trusted code (own repositories, protected refs) run below
- `ess/6-delivery/pipelines.yaml:790` OPEN: what retiring a pool does to its caches (UNMAPPED on BuildCache). Taken: nothing here;
- `ess/6-delivery/pipelines.yaml:902` OPEN: the instance's image is not compared with the pool's runner artifact (a third row);
- `ess/6-delivery/pipelines.yaml:1446` OPEN: removing a gate lifts a protection; whether it needs an approval of its own is not said.
- `ess/6-delivery/pipelines.yaml:1530` OPEN: the plan's artifact digests live in weft.deployments and weft.workloads; with them as a
- `ess/6-delivery/pipelines.yaml:1627` OPEN: may the plan's author approve their own plan (four eyes)? Not said. Taken: allowed.
- `ess/4-applications/formations.yaml:947` OPEN: a sleep policy by default (design § Stage 1.3) is the workloads' scale setting, not set
- `ess/4-applications/formations.yaml:997` OPEN: the pull-request-closed webhook calls RetireEnvironment (no edge domain yet, sketch § 14).
- Integrator, 2026-11-26: `Environment.protected` now exists (formations); `access.yaml:392` and `:397` can read it once a federation rule names its environments.

## soundness deployments (2026-11-26)

- Not green. 0 failed scenarios of deployments; the release chain past Planned gets no scenario.
- Run 1 (original file): subset 1763 scenarios, deployments refusals 001x6, 003x31, 004x49, 008x1; synthesis 442 s.
- Last complete probe (deployments + shared only): 73 scenarios, 73 passed; refusals 001x10, 003x17, 004x18, 008x2; 403 s (original: 39 scenarios, 288 s). Not re-measured after the last revert.
- Runs 3-6 did not finish (600-1500 s caps); one of them held 22 GB and ran the host out of memory (05:14 CET).
- `external`: 0 before, 0 after.
- StartRelease 001x3: its row sets on `subject.plan_id` and `subject.environment_id` stay undetermined, so no release reaches Applying. A variant that reaches it ran past 1200 s; reverted.
- AddReleaseStep/release-not-planned 001: beside a row set only the creator's state is arranged.
- CreatePlan deploy-refused / not-translated, RefuseTranslation refused / entry-not-refused 003: the bounded search (8 and 2 arrangements) finds no row in the needed state.
- ReapplyDrift/wrong-state, StartReleaseStep/no-such-step 008: need authored scenarios (seed pass).
- Contract changes, for the owner's review:
  - TranslateSource: `already-translated` removed; digest, formation, translator version read from rows.
  - SubmitDeploymentSource: input `translator_id`, outcome `translator-not-for-source`.
  - RefuseTranslation: input `entry_id` replaces the row set; outcome `entry-not-refused`.
  - CreatePlan: `deploy-refused` reads the translation's state; input `formation_id` removed.
  - ApplyPlan: input `environment_id` and `plan-mismatch` removed; `already-applied`, `release-in-progress` moved to StartRelease ("in progress" = Applying).
  - MarkReleaseLive, MarkRollbackLive: address the release; `environment_id`, `release-mismatch` removed.
  - StartReleaseStep: addresses the step; `release_id`, `position` removed.
  - PlanRollback: formation and revision copied from the release.
- New UNMAPPED markers: deployments.yaml lines 1268, 1690, 2204 (lines as of the pass).
- ESS limits named (input for docs/ess-gaps.md): a row set beside a guard comparing a related row's field with an input; `partitioned()` in `related_guard.rs` counts `unknown_instance`/`when_subject` as conflicting with two related rows; a row set keyed by the entity's own identity is never arranged; cost and memory grow without bound once the release chain is reachable.

## soundness pipelines, first measurement (integrator, 2026-11-26)

- Subset of 13 domains (deployments and workloads among them, through `GateCheck.plan_id`): 2141 scenarios, 2136 passed, 0 failed, 5 unsupported (identity `FinishKeyRotation`, not pipelines). Synthesis 2134 s on the build node, run 96 s.
- Pipelines refusals: 001x2, 003x27, 004x77. Reached: only the 15 `no-such-*` / `wrong-state` outcomes.
- Root: `ConnectForge/connected` gets no scenario. Its `Secret` row cannot be arranged because `CreateSecret/created` gets none; that one needs an `Account`, and `CreateAccount/created` is gap 9 (`via:` beside a uniqueness row set; beyond10x/ess PR 835).
- Second root: `AddDeployGate` needs an `Environment`; `CreateEnvironment/created` is gap 9 too.
- `--synthesis-seed` does not reach these: it supplies only a command's own subject row, and both rows are related rows named by an input.
- ConnectForge's `foreign-secret` (a related row's field compared with an input) beside the `exists` row set is the shape the deployments pass names as blocking; to be re-measured once `Account` is reachable.
