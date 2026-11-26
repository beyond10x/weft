---
format: aep.planning-md/3
id: design:ci-cd-support
kind: design
status: draft
title: 'CI/CD support: CD from existing CI, runners on weft, deploy gates'
summary: Federated OIDC deploys from GitHub/GitLab jobs, deploy gates, preview environments; then weft-hosted ephemeral CI runners with build caches; weft-native pipelines deferred.
revision: 3
---
## Goal

weft supports CI/CD for the systems that run on it: a change pushed to a forge (GitHub, GitLab,
Forgejo) is built and tested, and a release reaches a weft environment with no long-lived
credential anywhere, behind gates the specification can check.

Goalpost: the beyond10x repositories run their GitHub Actions jobs on runners hosted by weft
(scale to zero, a build cache that survives the runner, `slot` inside), and deploy to weft from the
same job with `weft deploy`, authenticated by the job's own OIDC token.

## What weft already has (read before adding anything)

- Deployments: sources, translation, plans, releases, rollout steps, rollback, drift
  (`docs/domain-sketch.md` § 5; `ess/6-delivery/deployments.yaml`, `SourceKind` line 26).
- Rollout gated by readiness and drift repair (sketch § 14 R1, R7; § 15).
- Artifacts: content-addressed, signed, `built_from` (`ess/3-resources/artifacts.yaml`, entity
  `Artifact` line 43).
- Workload shapes `Job` and `Box`, sleep and wake, scale to zero (sketch § 4).
- Workload identity and token exchange from weft to an outside service (sketch § 8). The other
  direction, an outside identity (a CI job) to a weft API token, does not exist yet.
- The `slot` build server as a catalog product (sketch line 556).
- Metering of CPU, memory, disk I/O, network (sketch § 12).

## Three stages, in order

The simplest working path first. Each stage is useful alone.

### Stage 1: CD from an existing CI (no runner of weft's)

A job in GitHub Actions or GitLab CI runs `weft deploy`. Nothing is stored in the forge.

1. Federation (access, layer 2):
   - `TrustedIssuer`: an OIDC issuer weft accepts (issuer URL, JWKS location, audience), per
     tenant. Examples: `https://token.actions.githubusercontent.com`, a GitLab instance.
   - `FederationRule`: which tokens of an issuer become which weft identity. It matches claims
     (repository, ref or ref pattern, workflow, environment) and grants one role in one account,
     limited to named formations and environments, with a token lifetime cap (default 15 min).
   - `ExchangeFederatedToken`: the CLI sends the job's OIDC token; weft checks signature, issuer,
     audience, expiry and the rule's claims, and returns a short-lived API token (OAuth 2.0 token
     exchange, RFC 8693; JWT bearer, RFC 7523; to be checked against the RFC text). A token that
     matches no rule is refused by name. Default deny.
   - Each exchange is recorded (who, which rule, which claims) as an authorization record.
2. Deploy gates (pipelines domain, layer 6, read by deployments):
   - `DeployGate` on an environment: kinds `approval` (a principal with a role approves),
     `signed-artifact` (every artifact signed by a named key), `provenance` (built by a named
     builder from a named repository), `conformance` (an ESS conformance report, green, for the
     model digest of an ess-system source), `window` (a time window).
   - `GateCheck`: one evaluation of one gate for one plan: passed, failed, waiting (approval).
   - `StartRelease` refuses while a gate of the target environment has no passed check for the
     plan. The refusal names the gate.
3. Preview environments (formations, layer 4): an `Environment` gets an origin (`manual`,
   `pull-request`), the source reference (repository, pull request number), an expiry, and a sleep
   policy by default. `weft deploy --preview` from a pull-request job creates or updates it;
   closing the pull request (forge webhook, stage 2) or the expiry retires it.
4. CLI: `weft deploy` detects a CI environment (`ACTIONS_ID_TOKEN_REQUEST_URL`, GitLab's
   `CI_JOB_JWT_V2` successor) and does the exchange itself; `weft gate approve <plan>`.

### Stage 2: runners on weft

weft hosts the runners of an outside CI: GitHub Actions self-hosted runners, GitLab runners.

- `ForgeConnection`: an account's link to a forge (GitHub App installation, GitLab group token
  held as a weft secret). Receives webhooks at the edge: job queued, pull request closed, push.
- `RunnerPool`: a forge connection, the labels it serves (`runs-on: [weft, gpu]`), a runner
  workload (image or Box), the isolation level (default `MicroVm`: CI runs untrusted code), the
  resource class, min 0, max N, a per-job time limit (default 10 min, at most 30 min, the same
  bounds as the slot pool in `~/beyond10x/AGENTS.md`), and a cache.
- `RunnerJob`: one CI job served by one runner instance. States: queued, assigned, running,
  succeeded, failed, timed-out, cancelled. A runner is ephemeral: one job, then the instance ends
  and its disk is wiped. The job id of the forge is stored for audit.
- `BuildCache`: a volume per pool and cache key (for example the Cargo lock digest), attached
  read-write to one job at a time or read-only to many, size cap, least-recently-used eviction.
  `sccache` and `slot` inside the runner use it.
- Scale: a queued-job webhook is the edge trigger (one job, one runner: sketch § 14, edge form);
  a gap view of queued jobs without a runner repairs a missed webhook (level form); the job time
  limit is a time-form command.
- Metering: runner seconds per resource class, cache GB-hours. The pricing models of § 12 apply
  (pay per use; a free tier of N minutes).
- Provenance: an artifact built in a `RunnerJob` gets an attestation (builder = pool, source =
  repository and commit, run = forge run id), in SLSA provenance form (to be checked). The
  `provenance` gate reads it.

### Stage 3: pipelines defined in weft (later, if stage 2 is not enough)

A `Pipeline` document in the repository (steps as weft Jobs, a DAG, caches, artifacts, gates),
run by weft without an outside CI. Not modelled now. Reasons to wait: stages 1 and 2 cover the
beyond10x goalpost; GitHub Actions and GitLab CI syntax already exist and teams keep them;
a weft pipeline format is a large surface. Decide after stage 2 runs.

## Where the nouns live

- access (layer 2): `TrustedIssuer`, `FederationRule`, `FederatedExchange` (the record).
- formations (layer 4): `Environment` origin, source reference, expiry (fields, no new entity).
- pipelines (new domain, layer 6, beside deployments): `ForgeConnection`, `RunnerPool`,
  `RunnerJob`, `BuildCache`, `DeployGate`, `GateCheck`.
- artifacts (layer 3): `Attestation` (provenance), referenced by the gate.
- metering (layer 8): meters `runner-seconds`, `cache-gb-hours`.
- catalog (layer 7): a product "CI runners" (a runner pool template), beside `slot`.

Layer check: pipelines (6) names accounts, formations, workloads, resources, secrets, artifacts,
cluster: all lower. Deployments (6) reads `GateCheck` in the same layer.

## Security

- No long-lived credential in a forge. Stage 1 uses the job's OIDC token; stage 2 uses a GitHub App
  (short-lived installation tokens) held as a weft secret.
- A federation rule names a repository exactly; a wildcard over repositories is refused. A rule for
  a production environment requires a protected ref (`refs/heads/main`, a tag pattern).
- Runners run at `MicroVm` isolation by default, egress allow-listed per pool, no access to other
  pools' caches; a cache written by a pull request from a fork is never read by a `main` build.
- Every exchange, gate decision and approval is a record with the principal or the token's claims.

## Verification

- The domains are ESS: each new command gets its scenarios on the interpreted target
  (`tools/synth-domain.sh`).
- Hardening: replay of real GitHub OIDC tokens (recorded, not live) against the federation rules;
  a mutation pass on the gate guards (a gate that can be skipped is the worst defect here).
- The `conformance` gate closes the loop for ESS systems: a release of an ess-system source goes
  live only with a green conformance report for its model digest (the same evidence AEP records).

## Open questions (default taken until answered)

1. GitLab's ID token claims differ from GitHub's: one rule schema with forge-specific claim names,
   or a claim map per issuer? Default: a claim map per `TrustedIssuer`.
2. Approvals: in weft only, or mirrored from the forge's environment protection? Default: in weft
   only (`weft gate approve`).
3. Runner image: one weft-provided image per forge, or any image the account registers? Default:
   any registered artifact, signed.
4. Cache poisoning across branches: key the cache by ref class (protected / unprotected).
   Default: two caches per key, a protected one written only by protected refs.

## Follow-up (stories, not yet created)

- S1 federation in access (TrustedIssuer, FederationRule, ExchangeFederatedToken).
- S2 deploy gates in pipelines and the `StartRelease` guard in deployments.
- S3 preview environments in formations.
- S4 forge connection and webhooks at the edge.
- S5 runner pools, runner jobs, build caches; reconciliation loops; metering.
- S6 provenance attestations and the provenance gate.
- S7 CLI: `weft deploy` in CI, `weft gate`, `weft runners`.

## Specification (drafted 2026-11-26)

The nouns of stages 1 and 2 are ESS, valid with the whole system
(`ess specify validate --path ess --strict-requires`: `weft v1 — 22 file(s), valid`).

- access (layer 2): `TrustedIssuer` `ess/2-tenancy/access.yaml:319`, `FederationRule` :362,
  `FederatedExchange` :453, command `ExchangeFederatedToken` :1438.
- artifacts (layer 3): `Attestation` `ess/3-resources/artifacts.yaml:106`, command
  `RecordAttestation` :269, view `AttestationsOfArtifact` :341.
- pipelines (new, layer 6), `ess/6-delivery/pipelines.yaml`: `ForgeConnection` :46,
  `RunnerPool` :101, `RunnerJob` :175, `BuildCache` :259, `DeployGate` :321, `GateCheck` :372;
  22 commands from `ConnectForge` :623 to `RejectGateCheck` :1673; 10 views from
  `ForgeConnectionsOfAccount` :1719 to `GateChecksOfPlan` :1857.
- Loops (sketch § 14): edge form `QueueRunnerJob` :825 on the forge webhook; level form, gap view
  `QueuedRunnerJobs` :1791 read by the scheduler; time form `TimeOutRunnerJob` :1035 with view
  `OverdueRunnerJobs` :1802.
- Components (`ess/components.yaml`): weft-api accepts the access and artifacts commands,
  weft-control owns `weft.pipelines`. Actors (`ess/9-actors/actors.yaml`): every new command has
  at least one actor.
- formations (layer 4): `Environment` fields `origin`, `protected`, `source_repository`,
  `pull_request`, `expires_at` `ess/4-applications/formations.yaml:191`; commands
  `CreatePreviewEnvironment` :949 and `ExpirePreviewEnvironment` :998 (time form).

Not yet specified:

- `StartRelease` in deployments does not read `GateCheck` (stage 1.2's guard).
- The sleep policy of a preview (stage 1.3) and the pull-request-closed webhook (no edge domain).
- Meters `runner-seconds`, `cache-gb-hours` (metering) and the catalog product "CI runners".
- Bindings for `RunnerJobQueued`, `RunnerJobEnded`, `BuildCacheEvicted`.
- Conformance scenarios of `weft.pipelines` (`tools/synth-domain.sh pipelines`).

Every `# OPEN:` and `# UNMAPPED:` in these files is listed in `docs/open-questions.md` § CI/CD.
