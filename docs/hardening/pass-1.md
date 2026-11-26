# weft hardening pass 1: isolation and default-deny guards (2026-11-26)

Techniques of `ess:hardening`: 6 (guard analysis, here: ownership of every identifier input and
guard order) and 8 (design review of the security rules). Input: `ess specify compile --path ess
--format json` (ess 0.99.2, `ess/37`, 0.7 s) on the uncommitted tree of 2026-11-26. No file under
`ess/` was changed. Not run: mutation, random sequences, determinism, metamorphic relations,
caller replay, spec diff (no synthesis or conformance on this host today).

## Numbers

- Commands checked: 401 (393 take at least one identifier input; 170 are reachable by a tenant
  actor: TenantOwner, AccountAdmin, Deployer, Viewer, Automation, Billing).
- Identifier inputs checked (`weft.shared.*Id` naming an entity): 667. Tenant-owned: 507 checked
  for an ownership guard. Operator-owned (cluster, node, zone, resource class, runtime, offering,
  plan, product, price list, translator, trust domain): 138, not account-scoped. Principals: 22,
  see D2.
- Findings, by severity:

| severity | findings (commands) | inputs |
|---|---|---|
| cross-account write, tenant actor | 8 | 15 |
| cross-account read or write, system actor only | 19 | 25 |
| disclosure | 23 | 34 |
| design rule missing | 10 | - |

## The frame every finding sits in

The specification has no caller account. Every "same account" guard compares a row with
`input.account_id`, which the client sends; the rule that the caller holds a role on that account
is written only as a comment (`ess/9-actors/actors.yaml:13-16`) and left to weft-api. So a command
is isolated only if (1) weft-api authorizes the caller for `input.account_id`, or for the
subject's account where the command has no account input, and (2) every other identifier input is
tied to that anchor by a guard. This pass checks (2) and reports (1) as D3. Anchor taken per
command: an `AccountId`/`TenantId` input, else the supplied subject, else the outermost
tenant-owned input. A tie is a `when_related` predicate `<id> != input.<tied input>`, a
`when_subject` predicate of that form, or a row-set selection keyed by two tied inputs, closed
transitively.

Method check (plant first): in a scratch copy, `volume-foreign` (`ess/5-runtime/claims.yaml:346`)
was deleted; the script then reported `DeclareClaim volume_id` untied. Against the real tree it
does not. Scripts (scratch, not committed): `own.jq` (ownership), `disclose.jq` (guard order).

## Cross-account write, tenant actor

- W1 `ess/4-applications/workloads.yaml:2023-2025`, `weft.workloads.DefineWorkload`, inputs
  `formation_id`, `environment_id`, `formation_revision_id`. Nothing ties them to `account_id`,
  not even existence. A Deployer of account A defines a workload with `account_id: A` and the
  formation, environment and component name of account B. The instance's identity document copies
  that path (`ess/5-runtime/identity.yaml:647-656` checks the workload, not the formation's
  account), and `ExchangeToken` (`ess/5-runtime/identity.yaml:943-965`) matches B's
  `ExchangePolicy` by formation, environment and component only: A's instance gets B's
  outside-service tokens. Also inferred: A's instance answers under B's component name inside B's
  formation (links, routes). Guards:
  ```yaml
  - name: formation-foreign
    when_related: {via: input.formation_id, predicate: account_id != input.account_id}
    error: weft.workloads.ForeignOwner
  - name: environment-foreign
    when_related: {via: input.environment_id, predicate: formation_id != input.formation_id}
    error: weft.workloads.ForeignOwner
  - name: revision-foreign
    when_related: {via: input.formation_revision_id, predicate: formation_id != input.formation_id}
    error: weft.workloads.ForeignOwner
  ```
  each after an `exists: false` check on the same input with the same error (see "Disclosure").
  Defense in depth: D10.
- W2 `ess/4-applications/workloads.yaml:4548`, `weft.workloads.CreateBox`, input `volume_id`.
  No guard at all. A Deployer creates a box in its own workload with B's volume id; the box mounts
  B's volume: B's data read and written. Guard (Box takes its account from the workload, `:4575`):
  ```yaml
  - name: volume-missing
    when_related: {via: input.volume_id, exists: false}
    error: weft.workloads.NotFound
  - name: volume-foreign
    when_related:
      entity: weft.resources.Volume
      where:
        all:
          - volume_id == input.volume_id
          - not:
              exists:
                as: w
                in: {entity: weft.workloads.Workload}
                that: {all: [{w.account_id: {eq: {fact: account_id}}}, w.workload_id == input.workload_id]}
      exists: true
    error: weft.workloads.NotFound
  ```
- W3 `ess/4-applications/workloads.yaml:4549`, `weft.workloads.CreateBox`, input `endpoint_id`.
  No guard. The box is reached through another formation's (another account's) endpoint: B's
  endpoint traffic goes to A's box (inferred from `Box.endpoint_id`). Guard: as W2 over
  `weft.formations.Endpoint` with `{w.formation_id: {eq: {fact: formation_id}}}`.
- W4 `ess/6-delivery/deployments.yaml:1112`, `weft.deployments.SubmitDeploymentSource`, input
  `formation_id` (and `helm_values[].environment_id`, read later by `TranslateSource`
  `:1214`). A source of account A is filed under B's formation; `TranslateSource` copies the
  formation into the Translation (`:1283`), and `CreatePlan`/`ApplyPlan` carry no account input.
  Whether the chain reaches a release in B depends on which account weft-api authorizes those on
  (inferred). Guard:
  ```yaml
  - name: formation-foreign
    when_related: {via: input.formation_id, predicate: account_id != input.account_id}
    error: weft.deployments.NotFound
  ```
- W5 `ess/8-economy/metering.yaml:1325`, `:1520`, `:1239`, `:1669`: `SetBudget`,
  `SetConcurrency`, `SetDeadline`, `SetRate`, input `formation_id` (Optional). A limit row of
  account A names B's formation. Effect inert unless A is an ancestor of B (`ChargeBudget` selects
  by `input.account_path`), so low. Guard on each:
  ```yaml
  - name: formation-foreign
    when_related: {via: input.formation_id, predicate: account_id != input.account_id}
    error: weft.metering.TargetMismatch
  ```
- W6 `ess/4-applications/workloads.yaml:4155-4156`, `weft.workloads.Invoke`, inputs
  `schedule_id`, `trigger_id`; and `:4318`, `weft.workloads.SubmitJob`, input `schedule_id`. An
  invocation or job records another workload's schedule or trigger as its cause (audit and
  attribution only). Guard:
  ```yaml
  - name: schedule-of-other-workload
    when_related: {via: input.schedule_id, predicate: workload_id != input.workload_id}
    error: weft.workloads.ForeignOwner
  ```
  and the same for `trigger_id`.
- W7 `ess/3-resources/artifacts.yaml:206`, `weft.artifacts.RegisterArtifact`, input `built_from`.
  An artifact claims to be built from another account's artifact (provenance only; the
  `provenance` gate reads `Attestation`). Guard:
  ```yaml
  - name: built-from-foreign
    when_related: {via: input.built_from, predicate: account_id != input.account_id}
    error: weft.artifacts.NotFound
  ```
- W8 `ess/6-delivery/deployments.yaml:1214`, `weft.deployments.TranslateSource`, input
  `environment_id`: tied only through the source's own `helm_values` (`:1256-1267`), which W4
  leaves unchecked. Fixed by W4 plus, in `SubmitDeploymentSource`, a guard that every
  `helm_values[].environment_id` is an environment of `input.formation_id`.

## Cross-account read or write, system actor only

A controller that is wrong, or a node that is compromised (sketch § 11: "reports what it
observed"), crosses accounts here. Pattern for a subject-anchored command:
`when_related: {entity: <E>, where: {all: [<id> == input.<id>, account_id != subject.account_id]}, exists: true}`
(the `subject.` operand is already used at `ess/5-runtime/claims.yaml`, `volume_id == subject.volume_id`).

- `ess/4-applications/workloads.yaml:5306` `RecordSnapshotTaken` (NodeAgent), `artifact_digest`:
  a node points a snapshot of A at B's artifact; A's restore reads B's bytes. Guard:
  `{entity: weft.artifacts.Artifact, where: {all: [artifact_digest == input.artifact_digest, account_id != subject.account_id]}, exists: true}`.
- `ess/4-applications/workloads.yaml:3775` `RecordServiceAsleep` (NodeAgent), `sleep_snapshots`:
  a sleeping service's wake restores snapshots of another workload. Guard: every listed snapshot
  has `workload_id == subject.workload_id`.
- `ess/4-applications/workloads.yaml:4468` `CompleteJob` (Scheduler), `outputs`: job outputs name
  another account's artifacts.
- `ess/5-runtime/claims.yaml:448` `BindClaim` (Scheduler), `volume_id`: a binding names a volume
  other than the claim's. Guard: `when_subject: {predicate: volume_id != input.volume_id}` on the
  claim.
- `ess/5-runtime/nodes.yaml:1130` `Snapshot` (Scheduler), `snapshot_id`: not tied to the
  assignment's instance.
- `ess/5-runtime/scheduling.yaml:767` `PlaceInstance` (Scheduler), `bindings`: bindings of
  another instance's claims (another tenant's devices or volumes) placed with this instance.
- `ess/5-runtime/scheduling.yaml:589`, `:591` `SubmitSchedulingRequest` (ControlPlane,
  ReleaseController), `instance_id`, `formation_id`: not tied to `account_id`; the request's
  `tenant_id` need not be the instance's, and `shared-kernel` (`scheduling.yaml:804`) then
  compares the wrong tenant.
- `ess/5-runtime/scheduling.yaml:1106` `PreemptPlacement` (Scheduler), `preemptor_request_id`.
- `ess/5-runtime/scheduling.yaml:982` `RegisterGang` (ControlPlane), `job_id`.
- `ess/8-economy/metering.yaml:1105-1113` `RecordUsage` (ControlPlane), `formation_id`,
  `instance_id`, `assignment_id`, `volume_id`, `endpoint_id`, `service_instance_id`: usage of one
  account's units billed to another.
- `ess/8-economy/metering.yaml:1383` `ChargeBudget` (ControlPlane), `usage_record_id`.
- `ess/8-economy/metering.yaml:1610`, `:1641` `AcquireConcurrency`, `ReleaseConcurrency`
  (Scheduler), `instance_id`: another account's instance holds or frees a slot.
- `ess/6-delivery/deployments.yaml:1374` `CompleteTranslation` (Translator),
  `formation_revision_id`: a revision of another formation.
- `ess/6-delivery/deployments.yaml:2117` `RecordDrift`, `:1583` `RecordPlanResult`
  (`live_release_id`), `:1812` `StartReleaseStep` (`job_id`) (ReleaseController).
- `ess/2-tenancy/access.yaml:1166-1167` `RecordAuthorization` (ManagementApi), `login_id`,
  `api_token_id`: audit row names another account's credential.

## Disclosure

Each: the ownership refusal has its own error, so a caller learns that a row of another account
(or tenant) exists; marked [state] where a state refusal of the foreign row also answers before
the ownership check. Guard, the same shape for every one: move the ownership outcome directly
after the `exists: false` outcome of the same input and give it that outcome's error, e.g.

```yaml
- name: volume-missing
  when_related: {via: input.volume_id, exists: false}
  error: weft.claims.UnitNotFound
- name: volume-foreign
  when_related: {via: input.volume_id, predicate: account_id != input.account_id}
  error: weft.claims.UnitNotFound
```

- `ess/2-tenancy/accounts.yaml:1089` `AllocateQuota` `account_id` [state: TopLevelQuota];
  `:1092` `parent_quota_id`.
- `ess/2-tenancy/accounts.yaml:773`, `:776` `BindRole` `principal_id`, `account_id`
  (CrossTenantBinding vs PrincipalNotFound/AccountNotFound: a principal or account id of another
  tenant exists).
- `ess/2-tenancy/accounts.yaml:836` `BindTenantOwner` `principal_id`.
- `ess/2-tenancy/accounts.yaml:1162` `DrawQuota`, `:1208` `ReleaseQuota`, `quota_id` [state].
- `ess/7-services/catalog.yaml:1424`, `:1431` `BindService` `service_instance_id`,
  `formation_id`.
- `ess/7-services/catalog.yaml:1135` `ProvisionServiceInstance` `formation_id`.
- `ess/7-services/catalog.yaml:1640` `RestoreServiceInstance` `backup_id`.
- `ess/5-runtime/claims.yaml:346`, `:349`, `:352` `DeclareClaim` `volume_id`, `filesystem_id`,
  `bucket_id`.
- `ess/6-delivery/deployments.yaml:1496` `CreatePlan` `translation_id` [state: DeployRefused,
  TranslationNotReady].
- `ess/6-delivery/deployments.yaml:1544` `PlanRollback` `release_id`.
- `ess/5-runtime/observability.yaml:895` `ConfigureBucketSink` `bucket_id`.
- `ess/5-runtime/observability.yaml:1246`, `:1256` `CreateAlertRule` `metric_series_id`,
  `notification_target_id`.
- `ess/5-runtime/observability.yaml:1485`, `:1495`, `:1500`, `:1517`, `:1534`
  `SetTelemetryPolicy` `formation_id`, `environment_id`, `logs_sink_id`, `metrics_sink_id`,
  `traces_sink_id`.
- `ess/3-resources/resources.yaml:411` `RestoreVolume` `source_snapshot_id`; `:527`
  `SnapshotVolume` `volume_id`.
- `ess/5-runtime/secrets.yaml:598` `GrantSecret` `pinned_version_id`.
- `ess/4-applications/workloads.yaml:5372` `RestoreSnapshot` `snapshot_id` [state:
  SnapshotNotReady].
- `ess/4-applications/formations.yaml:1440`, `:1444` `CreateExport` `from_account_id`,
  `to_account_id`: external NoSuchAccount vs CrossTenantExport; give both NoSuchAccount.
- `ess/2-tenancy/access.yaml:1366` `CreateFederationRule` `trusted_issuer_id`: external
  IssuerNotOfTenant vs IssuerUnavailable (`:1353`); give it IssuerUnavailable.
- `ess/4-applications/workloads.yaml:2140` `DefineWorkload` `artifact_digest`: external
  ArtifactForeignAccount vs ArtifactNotFound; give it ArtifactNotFound.
- `ess/3-resources/artifacts.yaml:214` `RegisterArtifact` `artifact_digest`: the artifact's
  identity is its digest across all accounts, and `exists` answers ArtifactExists before the
  signature (`:220`). Anyone with RegisterArtifact learns whether any account holds given bytes,
  and can squat a public image's digest so its owner's later registration is refused and its
  `DefineWorkload` answers ArtifactForeignAccount. Guard: key the artifact by account
  (`(account_id, artifact_digest)`, a design change, D8); until then
  ```yaml
  - name: exists
    when_related:
      entity: weft.artifacts.Artifact
      where: {all: [artifact_digest == input.artifact_digest, account_id == input.account_id]}
      exists: true
    error: weft.artifacts.ArtifactExists
  ```
  needs the per-account key to be satisfiable.
- `ess/2-tenancy/access.yaml:1526` `ExchangeFederatedToken`: `bad-signature` answers last, after
  every claim check. With a forged, unsigned token an outsider walks the claims (account, role,
  repository, ref, workflow, environment) of another tenant's rule: NoMatchingRule until all
  match, then IdTokenSignatureInvalid. Guard: the signature first,
  ```yaml
  - name: bad-signature
    external: the ID token's signature does not verify against the keys at the issuer's JWKS location
    error: weft.access.NoMatchingRule
  ```
  as the second outcome (after `unknown-issuer`, which names the JWKS location), with the rule
  checks' error.

## Design rules missing

- D1 `ess/6-delivery/deployments.yaml:1698`, `StartRelease`: design `ci-cd-support.md:58`
  "`StartRelease` refuses while a gate of the target environment has no passed check for the
  plan". No guard reads `GateCheck`: every deploy gate is skipped (the design itself lists this
  under "Not yet specified"). Guard:
  ```yaml
  - name: gate-not-passed
    when_related:
      entity: weft.pipelines.DeployGate
      where:
        all:
          - environment_id == subject.environment_id
          - state == Active
          - not:
              exists:
                as: c
                in: {entity: weft.pipelines.GateCheck}
                that: {all: [{c.deploy_gate_id: {eq: {fact: deploy_gate_id}}}, c.plan_id == subject.plan_id, c.state == Passed]}
      exists: true
    error: weft.deployments.GateNotPassed   # new error
  ```
- D2 caller identity is a client input in 17 commands: `caller_id` (CreateApiToken,
  CreateFederationRule, RevokeLogin, AddDeployGate, ApproveGateCheck, RejectGateCheck),
  `created_by`/`submitted_by` (CreatePlan, ApplyPlan, PlanRollback, SubmitDeploymentSource),
  `principal_id`/`holder` (ApproveLogin, OpenExec, OpenExecSession, OpenSession, AttachSession,
  DetachSession, AcquireBoxLease). Worst: `ess/6-delivery/pipelines.yaml:1634`,
  `ApproveGateCheck`: the approver is whoever the request names; `not-approver` (`:1651`) is an
  external over that name. Known OPEN (`ess/2-tenancy/access.yaml:968`). `ess/37` has caller
  values (`ess/16`): declare `attributes: [{name: principal_id, type: weft.shared.PrincipalId}]`
  on every actor that may send these, drop the input, and write `{caller: principal_id}` in
  `sets:`/`payload:` and `caller.principal_id` in guards, e.g. `ApproveLogin` keeps
  `principal_id` only if weft-api, not the browser, supplies it.
- D3 no caller account: 41 tenant-reachable commands take `account_id`/`tenant_id` as the anchor
  and 128 anchor on a subject; none compares it with the caller (no actor declares
  `attributes:`). With `attributes: [{name: account_id, type: weft.shared.AccountId}]` on the
  tenant actors, each account-anchored command gets
  ```yaml
  - name: not-callers-account
    when: account_id != caller.account_id
    error: <domain>.NotFound
  ```
  and each subject-anchored one `when_subject: {predicate: account_id != caller.account_id}`.
  Open: a role binds "on an account and below" (sketch line 140); equality admits only the
  context account, so sub-accounts need a decision (an `account_path` attribute, or the context
  account per sub-account).
- D4 `ess/2-tenancy/access.yaml:401-404`: design `ci-cd-support.md:46` "limited to named
  formations and environments". `formation_names`/`environment_names` are stored and read by no
  guard; `FederatedExchange` (`:1530-1547`) does not carry them, so nothing downstream can hold
  them. Written as weft-api's duty (UNMAPPED, `:397`). Guard: copy them into the exchange and
  check them where a token deploys (`CreatePlan`, `StartRelease`), or move the rule above layer 4
  with `FormationId`/`EnvironmentId`.
- D5 `ess/2-tenancy/access.yaml:1341`: design `ci-cd-support.md:116-117` "a rule for a production
  environment requires a protected ref". Held only on the rule's own `production` flag, which its
  author sets; `Environment.protected` (`ess/4-applications/formations.yaml:193`) is read by no
  guard. A rule with `production: false` naming environment `prod` deploys from any ref. Guard:
  where the exchanged token deploys, refuse when the environment is protected and the exchange's
  `ref` was not protected:
  ```yaml
  - name: protected-environment-unprotected-ref
    when_related: {via: input.environment_id, predicate: protected == true}
    when: ref_protected == false
    error: weft.deployments.ProtectedRefRequired   # new error
  ```
  (needs D4's exchange fields on the deploying command).
- D6 `ess/6-delivery/pipelines.yaml:130`: design `ci-cd-support.md:118` "egress allow-listed per
  pool". `RunnerPool.egress_allow` is stored and never applied: `AssignRunnerJob` (`:904`) checks
  the instance's isolation, not its egress claims. Guard in `AssignRunnerJob`: refuse an instance
  whose workload's egress claims are not a subset of the pool's `egress_allow`.
- D7 `ess/6-delivery/pipelines.yaml:1096`, `CreateBuildCache`: design `ci-cd-support.md:119` "a
  cache written by a pull request from a fork is never read by a `main` build". The read and write
  guards hold it per cache row (`:1181`, `:1242`), but one volume may back a protected and an
  unprotected cache: a fork job writes the unprotected cache, a `main` build reads the same bytes.
  Guard:
  ```yaml
  - name: volume-in-use
    when_related:
      entity: weft.pipelines.BuildCache
      where: {all: [volume_id == input.volume_id, state == Active]}
      exists: true
    error: weft.pipelines.VolumeUnavailable
  ```
- D8 `ess/3-resources/artifacts.yaml:43` (entity `Artifact`): design sketch § 1 line 145
  "Nothing crosses accounts". An artifact is one row per digest for the whole cluster, owned by
  the first account that registers it (see Disclosure, `RegisterArtifact`). Identity
  `(account_id, artifact_digest)` or a per-account registration row.
- D9 `ess/5-runtime/secrets.yaml:617`, `:773`: brief's rule "a secret is delivered only to an
  instance of its own scope". Account scope is a guard (`:754`); formation and environment scope
  only externals. `Secret` stores `formation_id`/`environment_id` and `IdentityDocument` stores
  both, so a guard can hold it in `DeliverSecret`:
  ```yaml
  - name: document-outside-formation-scope
    when_related:
      entity: weft.secrets.Secret
      where:
        all:
          - secret_id == input.secret_id
          - defined(formation_id)
          - not:
              exists:
                as: d
                in: {entity: weft.identity.IdentityDocument}
                that: {all: [{d.formation_id: {eq: {fact: formation_id}}}, d.identity_document_id == input.identity_document_id]}
      exists: true
    error: weft.secrets.IdentityOutsideScope
  ```
  and the same for `environment_id`.
- D10 `ess/5-runtime/identity.yaml:647`, `:943-965`: sketch § 8 line 431 identity
  `tenant/account/formation/...`. `IssueIdentityDocument` does not check that the formation is the
  account's, and `ExchangeToken` never compares the document's account with the policy's. Either
  closes W1's token theft if W1 regresses. Guards:
  ```yaml
  # IssueIdentityDocument
  - name: formation-of-other-account
    when_related: {via: input.formation_id, predicate: account_id != input.account_id}
    error: weft.identity.IdentityPathMismatch
  # ExchangeToken
  - name: policy-of-other-account
    when_related:
      entity: weft.identity.ExchangePolicy
      where:
        all:
          - exchange_policy_id == input.exchange_policy_id
          - not:
              exists:
                as: d
                in: {entity: weft.identity.IdentityDocument}
                that: {all: [{d.account_id: {eq: {fact: account_id}}}, d.identity_document_id == input.identity_document_id]}
      exists: true
    error: weft.identity.PolicyNotForIdentity
  ```

## Security rules found held

- Federation rule names a repository exactly, wildcard refused: `ess/2-tenancy/access.yaml:1329`;
  matched by equality at exchange: `:1484-1493`.
- Protected ref required when the rule says so: `ess/2-tenancy/access.yaml:1500` (but see D5).
- Token lifetime bounded (1 to 3600 s): `ess/2-tenancy/access.yaml:1347`.
- Every exchange is a record with its claims: `ess/2-tenancy/access.yaml:1530-1547`.
- Runners at MicroVm: pool `ess/6-delivery/pipelines.yaml:735`, runner instance `:933`.
- One job per runner instance: `ess/6-delivery/pipelines.yaml:936`.
- No access to another pool's cache: `ess/6-delivery/pipelines.yaml:1177`, `:1194` (write),
  `:1238`, `:1250` (read).
- Unprotected ref never writes a protected cache: `ess/6-delivery/pipelines.yaml:1181`; protected
  ref never reads an unprotected cache: `:1242` (per row; see D7).
- Cache, pool, connection, artifact of the same account: `ess/6-delivery/pipelines.yaml:747-762`,
  `:1118-1128`, `:927-932`.
- Instances of different tenants never share a kernel: `ess/5-runtime/scheduling.yaml:804`.
- Minimum isolation never weakened: placement `ess/5-runtime/scheduling.yaml:787-797`, workload
  vs runtime `ess/4-applications/workloads.yaml:2114`, cluster policy
  `ess/1-infrastructure/cluster.yaml:558`.
- GPU wiped between holders: lifecycle `ess/1-infrastructure/inventory.yaml:158-160` (bind only
  from Free, release goes to Wiping); placement external `ess/5-runtime/scheduling.yaml:856`.
- Secret value reaches an instance only at start: `ess/5-runtime/secrets.yaml:736`; only through
  a live grant for that identity and secret: `:695-703`; only to a live document of that
  instance: `:741-746`; only within the secret's account: `:754`.
- Secret value never returned: no view or stored field holds it (`ess/5-runtime/secrets.yaml:800`).
- Node reports only about its own session and assignments: `NotOwnNode` guards in
  `InstanceObserved` `ess/5-runtime/nodes.yaml:1295-1310`, `NodeObserved` `:1354-1360`, `UsageReported` `:1401-1422`.
- Claimed units are the claiming account's: `ess/5-runtime/claims.yaml:346-354`.
- Export stays within the tenant and names an endpoint of the exporting account: externals
  `ess/4-applications/formations.yaml:1440-1462`.

## Could not decide

- Views: no actor is granted a view (`ess/9-actors/actors.yaml:18-21`), so reads through views
  (`*OfAccount` with an `account_id` parameter) were not checked; they carry the same D3 hole.
- Whether `ref_protected` and the claims of `ExchangeFederatedToken` are taken from the verified
  token or from the request body: the spec cannot tell; moving `bad-signature` first makes it moot for
  refusals but not for the record.
- Whether a parent account's admin may act in a child account's formation (W5, D3).

```findings
- {file: ess/4-applications/workloads.yaml, line: 2023, category: spec-gap, severity: blocker, verdict: CONFIRMED, origin: pre-existing, message: "DefineWorkload ties formation_id, environment_id and formation_revision_id to nothing, so a workload of one account joins another account's formation and its identity matches that formation's exchange policies"}
- {file: ess/4-applications/workloads.yaml, line: 4548, category: spec-gap, severity: blocker, verdict: CONFIRMED, origin: pre-existing, message: "CreateBox mounts any volume_id with no existence or account check"}
- {file: ess/4-applications/workloads.yaml, line: 4549, category: spec-gap, severity: blocker, verdict: CONFIRMED, origin: pre-existing, message: "CreateBox takes any endpoint_id with no existence or formation check"}
- {file: ess/6-delivery/deployments.yaml, line: 1112, category: spec-gap, severity: blocker, verdict: CONFIRMED, origin: pre-existing, message: "SubmitDeploymentSource files a source under a formation of another account"}
- {file: ess/8-economy/metering.yaml, line: 1325, category: spec-gap, severity: warning, verdict: CONFIRMED, origin: pre-existing, message: "SetBudget, SetConcurrency, SetDeadline and SetRate accept a formation_id of another account"}
- {file: ess/4-applications/workloads.yaml, line: 4155, category: spec-gap, severity: note, verdict: CONFIRMED, origin: pre-existing, message: "Invoke and SubmitJob accept a schedule_id or trigger_id of another workload"}
- {file: ess/3-resources/artifacts.yaml, line: 206, category: spec-gap, severity: note, verdict: CONFIRMED, origin: pre-existing, message: "RegisterArtifact accepts built_from naming another account's artifact"}
- {file: ess/6-delivery/deployments.yaml, line: 1214, category: spec-gap, severity: note, verdict: CONFIRMED, origin: pre-existing, message: "TranslateSource environment_id is tied only to the source's unchecked helm_values"}
- {file: ess/4-applications/workloads.yaml, line: 5306, category: spec-gap, severity: warning, verdict: CONFIRMED, origin: pre-existing, message: "RecordSnapshotTaken lets a node point a snapshot at another account's artifact"}
- {file: ess/4-applications/workloads.yaml, line: 3775, category: spec-gap, severity: warning, verdict: CONFIRMED, origin: pre-existing, message: "RecordServiceAsleep accepts snapshots of another workload"}
- {file: ess/4-applications/workloads.yaml, line: 4468, category: spec-gap, severity: warning, verdict: CONFIRMED, origin: pre-existing, message: "CompleteJob accepts outputs owned by another account"}
- {file: ess/5-runtime/claims.yaml, line: 448, category: spec-gap, severity: warning, verdict: CONFIRMED, origin: pre-existing, message: "BindClaim binds a volume other than the claim's"}
- {file: ess/5-runtime/nodes.yaml, line: 1130, category: spec-gap, severity: warning, verdict: CONFIRMED, origin: pre-existing, message: "Snapshot takes a snapshot_id not tied to the assignment's instance"}
- {file: ess/5-runtime/scheduling.yaml, line: 767, category: spec-gap, severity: warning, verdict: CONFIRMED, origin: pre-existing, message: "PlaceInstance accepts bindings of another instance's claims"}
- {file: ess/5-runtime/scheduling.yaml, line: 589, category: spec-gap, severity: warning, verdict: CONFIRMED, origin: pre-existing, message: "SubmitSchedulingRequest ties instance_id and formation_id to nothing"}
- {file: ess/5-runtime/scheduling.yaml, line: 1106, category: spec-gap, severity: note, verdict: CONFIRMED, origin: pre-existing, message: "PreemptPlacement preemptor_request_id is not tied"}
- {file: ess/5-runtime/scheduling.yaml, line: 982, category: spec-gap, severity: note, verdict: CONFIRMED, origin: pre-existing, message: "RegisterGang job_id is not tied to the tenant"}
- {file: ess/8-economy/metering.yaml, line: 1105, category: spec-gap, severity: warning, verdict: CONFIRMED, origin: pre-existing, message: "RecordUsage ties none of its six unit identifiers to the account it bills"}
- {file: ess/8-economy/metering.yaml, line: 1383, category: spec-gap, severity: note, verdict: CONFIRMED, origin: pre-existing, message: "ChargeBudget usage_record_id is not tied to the budget's account"}
- {file: ess/8-economy/metering.yaml, line: 1610, category: spec-gap, severity: note, verdict: CONFIRMED, origin: pre-existing, message: "AcquireConcurrency and ReleaseConcurrency accept an instance of another account"}
- {file: ess/6-delivery/deployments.yaml, line: 1374, category: spec-gap, severity: note, verdict: CONFIRMED, origin: pre-existing, message: "CompleteTranslation accepts a revision of another formation"}
- {file: ess/6-delivery/deployments.yaml, line: 2117, category: spec-gap, severity: note, verdict: CONFIRMED, origin: pre-existing, message: "RecordDrift, RecordPlanResult and StartReleaseStep accept an instance, release or job of another formation"}
- {file: ess/2-tenancy/access.yaml, line: 1166, category: spec-gap, severity: note, verdict: CONFIRMED, origin: pre-existing, message: "RecordAuthorization names a login or token of another account"}
- {file: ess/2-tenancy/accounts.yaml, line: 1089, category: spec-gap, severity: warning, verdict: CONFIRMED, origin: pre-existing, message: "AllocateQuota, DrawQuota and ReleaseQuota answer existence and state of another account's quota before the ownership refusal"}
- {file: ess/2-tenancy/accounts.yaml, line: 773, category: spec-gap, severity: warning, verdict: CONFIRMED, origin: pre-existing, message: "BindRole and BindTenantOwner answer CrossTenantBinding for a principal or account of another tenant, distinct from not found"}
- {file: ess/7-services/catalog.yaml, line: 1424, category: spec-gap, severity: warning, verdict: CONFIRMED, origin: pre-existing, message: "BindService, ProvisionServiceInstance and RestoreServiceInstance disclose foreign rows by a distinct error"}
- {file: ess/5-runtime/claims.yaml, line: 346, category: spec-gap, severity: warning, verdict: CONFIRMED, origin: pre-existing, message: "DeclareClaim answers ForeignResource for another account's volume, filesystem or bucket, distinct from UnitNotFound"}
- {file: ess/6-delivery/deployments.yaml, line: 1496, category: spec-gap, severity: warning, verdict: CONFIRMED, origin: pre-existing, message: "CreatePlan and PlanRollback answer state of another formation's translation or release before the mismatch refusal"}
- {file: ess/5-runtime/observability.yaml, line: 895, category: spec-gap, severity: warning, verdict: CONFIRMED, origin: pre-existing, message: "ConfigureBucketSink, CreateAlertRule and SetTelemetryPolicy disclose foreign rows by a distinct error"}
- {file: ess/3-resources/resources.yaml, line: 411, category: spec-gap, severity: warning, verdict: CONFIRMED, origin: pre-existing, message: "RestoreVolume and SnapshotVolume answer ForeignResource distinct from UnitNotFound"}
- {file: ess/5-runtime/secrets.yaml, line: 598, category: spec-gap, severity: note, verdict: CONFIRMED, origin: pre-existing, message: "GrantSecret discloses a version of another secret by a distinct error"}
- {file: ess/4-applications/workloads.yaml, line: 5372, category: spec-gap, severity: note, verdict: CONFIRMED, origin: pre-existing, message: "RestoreSnapshot answers the state of another workload's snapshot before the ownership refusal"}
- {file: ess/4-applications/formations.yaml, line: 1440, category: spec-gap, severity: note, verdict: CONFIRMED, origin: pre-existing, message: "CreateExport, CreateFederationRule and DefineWorkload externals disclose foreign rows by a distinct error"}
- {file: ess/3-resources/artifacts.yaml, line: 214, category: spec-gap, severity: warning, verdict: CONFIRMED, origin: pre-existing, message: "RegisterArtifact answers ArtifactExists for any account's digest before the signature, a content oracle and a squatting vector"}
- {file: ess/2-tenancy/access.yaml, line: 1526, category: spec-gap, severity: warning, verdict: CONFIRMED, origin: pre-existing, message: "ExchangeFederatedToken checks the signature last, so a forged token reveals another tenant's rule claims"}
- {file: ess/6-delivery/deployments.yaml, line: 1698, category: spec-gap, severity: blocker, verdict: CONFIRMED, origin: pre-existing, message: "StartRelease reads no GateCheck, so every deploy gate can be skipped"}
- {file: ess/6-delivery/pipelines.yaml, line: 1634, category: spec-gap, severity: blocker, verdict: CONFIRMED, origin: pre-existing, message: "the caller principal is a request input in 17 commands, including the approver of ApproveGateCheck"}
- {file: ess/9-actors/actors.yaml, line: 13, category: spec-gap, severity: blocker, verdict: CONFIRMED, origin: pre-existing, message: "no actor declares a caller account, so no guard ties input.account_id or a subject's account to the caller"}
- {file: ess/2-tenancy/access.yaml, line: 401, category: spec-gap, severity: warning, verdict: CONFIRMED, origin: pre-existing, message: "a federation rule's formation and environment limits are stored and enforced by no guard"}
- {file: ess/2-tenancy/access.yaml, line: 1341, category: spec-gap, severity: blocker, verdict: CONFIRMED, origin: pre-existing, message: "the protected-ref rule for production holds only on the rule's self-declared flag; Environment.protected is read by no guard"}
- {file: ess/6-delivery/pipelines.yaml, line: 130, category: spec-gap, severity: warning, verdict: CONFIRMED, origin: pre-existing, message: "a runner pool's egress allow-list is never applied to its runner instances"}
- {file: ess/6-delivery/pipelines.yaml, line: 1096, category: spec-gap, severity: warning, verdict: CONFIRMED, origin: pre-existing, message: "CreateBuildCache lets one volume back a protected and an unprotected cache"}
- {file: ess/3-resources/artifacts.yaml, line: 43, category: spec-gap, severity: warning, verdict: CONFIRMED, origin: pre-existing, message: "an artifact's identity is its digest across all accounts"}
- {file: ess/5-runtime/secrets.yaml, line: 773, category: spec-gap, severity: note, verdict: CONFIRMED, origin: pre-existing, message: "a secret's formation and environment scope is held only by externals though both sides are stored"}
- {file: ess/5-runtime/identity.yaml, line: 943, category: spec-gap, severity: warning, verdict: CONFIRMED, origin: pre-existing, message: "ExchangeToken and IssueIdentityDocument never compare the formation's or policy's account with the document's"}
- {file: ess/6-delivery/deployments.yaml, line: 1123, category: false-positive, severity: note, verdict: CONFIRMED, origin: pre-existing, message: "translator_id, InstallProduct version_id, ReleaseCacheWrite runner_job_id and Attach binding_id were flagged by the script but are operator rows or tied by an == or external guard"}
```

## What changed (applied 2026-11-26, one agent per domain file, integrator)

Frame: `ess/9-actors/actors.yaml` gives TenantOwner, AccountAdmin, Deployer, Viewer, Billing,
Automation, ControlPlane and ReleaseController the caller attributes `principal_id`, `tenant_id`,
`account_id` (`account_id` = the account the request acts in). Whole tree after: `weft v1 — 22
file(s), valid`, layers green. Not yet measured by synthesis.

ESS 0.99.2 limits met while applying (input for `docs/ess-gaps.md`): a caller value is compared
only with an input or a stored field of the subject, never a related row; existence checks answer
before every other related-row check; an input `when` guard answers before any row; a row-set guard
is refused beside a `when_subject` guard (ESS-COMMAND-009); a predicate cannot select inside a
union field.

### claims
- D3: DeclareClaim, ReleaseClaim not-callers-account -> NotFound.
- BindClaim claim-mismatch (ClaimMismatch); DeclareClaim foreign units answer UnitNotFound; unknown-class first.
- Cannot read caller: BindClaim, CommitBinding, CommitVolumeWriter, ReleaseBinding (Scheduler); ReleaseClaimBindings (R5).
- Not applied: DeclareClaim holder inside union ClaimHolder; Claims view.
- ESS: an ownership check cannot sit right after its own existence check; existence checks answer first.
- Risk: BindClaim second claim guard, earlier ESS-SYNTH-008.
### identity
- D3: CreateExchangePolicy, WithdrawExchangePolicy -> NotFound.
- D10: IssueIdentityDocument no-such-formation/environment, formation-of-other-account, environment-of-other-formation (IdentityPathMismatch); ExchangeToken policy-of-other-account (PolicyNotForIdentity).
- Cannot read caller: ExchangeToken (Instance), RenewIdentityDocument (NodeAgent), RevokeIdentityDocument, Issue/ExpireIdentityDocument, trust-domain commands.
- New finding: RevokeIdentityDocument does not limit a NodeAgent to its own node.
### artifacts
- D3: RegisterArtifact, RevokeArtifact -> NotFound.
- W7: built-from-missing/foreign -> NotFound. bad-signature before_row (signature before digest existence).
- Not applied: RecordAttestation (related-row anchor). D8 artifact keyed by digest across accounts: design change, OPEN in file.
### resources
- D3: 9 ties (Create/Restore/SnapshotVolume, CreateFilesystem, CreateBucket by input; 4 deletes by subject) -> NotFound.
- Disclosure: RestoreVolume snapshot-foreign, SnapshotVolume volume-foreign -> UnitNotFound; ForeignResource error removed.
- Not covered: Report* (NodeAgent); CreateBucket name-taken discloses cluster-wide bucket names (OPEN, design); views.
### secrets
- D3: 5 caller ties + 4 secret-of-other-account ties (existing not-found errors); input-only guard first (ESS order).
- New input account_id on AddSecretVersion, GrantSecret, DisableSecretVersion, RevokeSecretGrant (a related row cannot be compared with the caller).
- Disclosure: GrantSecret version-of-other-secret -> NoSuchVersion. D9: DeliverSecret document-outside-formation/environment-scope.
- Not covered: RegisterSecretBackend (operator), DeliverSecret (ControlPlane), GrantSecret identity-outside-scope stays external.
### formations
- D3: 6 ties (CreateFormation, SetFormationQuota, RetireFormation, CreateExport incl tenant, RevokeExport from-account, OPEN).
- Disclosure: from/to-other-tenant -> NoSuchAccount; CrossTenantExport removed; endpoint-not-in-account before already-exported.
- Untied: 9 commands also sent by Translator; 15 anchored on formation_id or rows without account_id.
- FOLLOW-UP (integrator): Translator gets caller attributes (acts for the source account); 15 commands get account_id input + formation-of-other-account tie (secrets pattern).
- ESS: a command may not combine a row-set guard with a when_subject guard (compile error seen in access, nodes, scheduling mid-edit).
### accounts
- D3: 15 ties (tenant and account, input and subject).
- Disclosure: BindRole, BindTenantOwner, AllocateQuota, DrawQuota, ReleaseQuota, RevokeTenantOwner foreign rows -> not-found, before state checks. CrossTenantBinding unused.
- Untied: ClusterOperator-only commands; BindTenantOwner (ClusterOperator) still tells missing from closed tenant.
- ESS: a when guard on an input answers first; a stored-row guard is refused beside a row-set guard (CloseAccount ties on input).
- D1 consequence: drawing from a parent quota needs the parent as the request account.
### catalog
- D3: 11 caller ties + 3 row ties (foreign service instance -> not-found).
- Disclosure: foreign formation/instance/backup -> not-found before state checks (:1135 :1424 :1431 :1640). 4 unused errors removed.
- New input account_id: UnbindService, BackupServiceInstance, RestoreServiceInstance.
- Untied: DeprovisionServiceInstance (ESS-COMMAND-009 beside still-bound row set; UNMAPPED), CheckServiceAccess (Instance).
### metering
- D3: 12 caller ties -> NotFound. W5: Set* formation exists and in account -> NotFound.
- RecordUsage: 6 row-set ties (UnitOfAnotherAccount, new error); service instances skip the instance tie (OPEN).
- Untied: AdmitWork, Acquire/ReleaseConcurrency (Scheduler); price-list commands (ClusterOperator).
- Not applied: ChargeBudget scope (ESS-COMMAND-009); Acquire/ReleaseConcurrency sub-account instances (UNMAPPED).
### observability
- D3: 7 caller ties -> NotFound. Disclosure: foreign bucket/series/target/formation/sink/environment -> Unknown*, before state checks. 5 Foreign* errors removed.
- Untied: RemoveTelemetrySink, RemoveNotificationTarget (ESS-COMMAND-009 beside in-use row sets; OPEN). NodeAgent/Instance commands.
### pipelines
- D3: ConnectForge, CreateRunnerPool, CreateBuildCache. D2: AddDeployGate created_by, Approve/RejectGateCheck decided_by from caller; caller_id input removed (3).
- D7: CreateBuildCache volume-in-use (VolumeInUse, new).
- Untied: Assign/Start/FinishRunnerJob (Scheduler, NodeAgent); DisconnectForge, RetireRunnerPool (ESS-COMMAND-009); 6 commands whose account is on a related row.
- D6 not applied: instance reachable only via Claim.holder union; needs plain instance field on Claim (claims domain).
### scheduling
- 15 guards: SubmitSchedulingRequest instance/formation exist, same account, formation of the workload; PlaceInstance account + bindings of that account on the node; RegisterGang job/account/tenant/gang Job (JobNotGang, new); PreemptPlacement not own request, gang preemptor.
- New input account_id: PlaceInstance, RegisterGang (one nested walk per guard in ESS 0.99.2).
- Not applied: caller ties (no tenant sender; binding caller unclear); PreemptPlacement zone tie (OPEN); PlaceInstance nonexistent binding id passes.
### nodes
- D3: OpenExecSession, CloseExecSession; instance-of-other-account and no-such-instance -> AssignmentNotFound; instance-not-assigned earlier.
- D2: OpenExecSession principal from caller; input principal_id removed; new input account_id; ExecSession.account_id field.
- Snapshot (:1130): instance-not-assigned, no-such-snapshot, snapshot-of-other-instance; new input instance_id; errors SnapshotNotFound, SnapshotNotOfInstance. Risk: Snapshot/ordered scenario.
- Not applied: Viewer Logs-only (role rule).
### access
- D3: 6 ties (CreateApiToken, CreateFederationRule, RegisterTrustedIssuer, RevokeApiToken, WithdrawFederationRule, WithdrawTrustedIssuer x2).
- D2: CreateApiToken, CreateFederationRule created_by from caller; caller_id removed. New input tenant_id on WithdrawTrustedIssuer.
- RecordAuthorization 2 ties; ExchangeFederatedToken bad-signature first; issuer-not-of-tenant -> IssuerUnavailable; FederatedExchange stores ref_protected, formation_names, environment_names (for D4/D5 in deployments).
- Untied: Approve/Deny/RevokeLogin (ClusterOperator); issuer tenant stays external; withdrawn foreign issuer -> StateConflict.
### deployments
- D1: StartRelease gate-not-passed (every active gate has a passed check; GateNotPassed, new).
- D3: SubmitDeploymentSource, WithdrawDeploymentSource. D2: submitted_by, created_by (CreatePlan, PlanRollback, ApplyPlan) from caller; 4 inputs removed.
- W4, W8, CreatePlan/PlanRollback disclosure, CompleteTranslation/RecordPlanResult/RecordDrift/StartReleaseStep ties; StartReleaseStep workload tied to environment (OutsideRelease, new).
- Untied: TranslateSource, CreatePlan, PlanRollback, DiscardPlan, ApplyPlan (account on related row; Plan/Release store none). D5 not applied.
- Risk: row sets added to SubmitDeploymentSource, CreatePlan (ess#642).
### workloads
- D3: 19 ties. D2: OpenExec, OpenSession, AttachSession, DetachSession, AcquireBoxLease principal from caller (5 inputs removed); CloseSession owner check.
- W1 (6), W2, W3, W6 (Invoke 4, SubmitJob 2), RecordSnapshotTaken, RestoreSnapshot, DefineWorkload artifact -> ArtifactNotFound; RecordServiceAsleep snapshot existence.
- Untied: Invoke, StopInstance, WakeService (Instance, Scheduler); rows without account: Invocation, Job, Session, Schedule, Trigger, WarmPool (FOLLOW-UP: account field copied from workload).
- Not applied: CompleteJob outputs, RecordServiceAsleep snapshot workload (row set beside subject guards; OPEN).
- Risk: DefineWorkload W1 formation checks (gap 21 history).
