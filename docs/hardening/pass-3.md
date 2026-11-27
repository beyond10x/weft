# weft hardening pass 3: spec diff, mutation, determinism, metamorphic relations (2026-11-27)

Ordered by the owner on 2026-11-27 about 11:48 CET. Techniques of `ess:hardening` not run before,
in the skill's order: 7 (spec diff in the gate), 1 (mutation audit), 4 (determinism), 5
(metamorphic relations). Not run: 3 (caller replay), see the end. "Pass 3" here is the third
document; `pass-2.md` holds sub-passes titled "Pass 3" to "Pass 8", which are part of pass 2.

Inputs:

- `ess 0.99.2` (`ess --version`), the CLI the gate pins.
- weft at `main` `58172a7`. weft-sim (`tools/sim`) at `58172a7` plus the changes of this pass;
  its ESS crates at beyond10x/ess `c0b5f560`. Binary sha256 prefix `e279749b39ed9137` after the
  last change.
- No file under `ess/` changed in this pass.

## Numbers

| technique | ran on | planted defect caught | findings | fixed | open |
| --- | --- | --- | --- | --- | --- |
| 7 spec diff | `ess/` at tag `0.1.0` against the tree; 8 merges of `ess/` since the import | yes, exit 4 | 2 | 1 | 1 |
| 1 mutation | subset `weft.shared` + `weft.infrastructure`, interpreted target | yes, exit 1 | 3 | 0 | 3 |
| 4 determinism | `weft-sim explore`, twice per seed | yes, exit 1 | 2 | 1 (partly) | 2 |
| 5 metamorphic | `weft-sim relate` from `sim/experiments/full-setup.yaml` | yes, both relations | 2 | 0 | 2 |

9 findings; 1 fixed in full, 1 in part (D1, the warning; its decision stays open); 8 open. None
changes `ess/`. `ess specify validate --path ess --strict-requires`: `weft v1 — 25 file(s),
valid`. `slot --timeout 20m task check`: exit 0, 29 s; the spec-diff gate step takes 7.2 s.

## Technique 7: spec diff in the gate

`ess verify diff --from <release>/ess --to ess --fail-on breaking-or-unknown` takes 3.9 s on the
whole specification. weft had no such step: the gate validated `ess/` but did not classify a
change.

### What history shows

Every merge on `main` that changed `ess/`, against its first parent (`ess verify diff
--compatibility --format json`, commits of the rewritten history):

| merge | changes | breaking | unknown | compatible |
| --- | --- | --- | --- | --- |
| #5 harden/stamped-time | 50 | 6 | 33 | 11 |
| #8 spec/components | 132 | 1 | 21 | 110 |
| #9 sim/invariants-in-explore | 7 | 0 | 6 | 1 |
| #10 docs/owner-name | 0 | 0 | 0 | 0 |
| #12 hardening/pass-7 | 61 | 1 | 59 | 1 |
| #13 spec/topology-placement | 12 | 1 | 9 | 2 |
| #14 hardening/isolation-2 | 153 | 29 | 124 | 0 |
| #15 sim/closed-link | 2 | 0 | 2 | 0 |

7 of 8 would have failed `--fail-on breaking-or-unknown`; 38 breaking changes (31
`input-added` of a required input, 6 `grant-removed`, 1 `variant-added`) merged with no
acknowledgement. Of the 254 unknown changes, 83 are `outcome-added` (a new refusal) and 29
`outcome-sets-changed`. `ess/` at tag `0.1.0` and at `58172a7` mean the same system ("no semantic
change").

### What changed

- `tools/check-spec-diff.sh`: compares `ess/` at the newest release tag
  (`<major>.<minor>.<patch>`, `git archive` into `.check/spec-diff/`) with the tree and exits with
  the status of `ess`: 0 passed, 4 an unacknowledged breaking or unknown change, 1 a refused or
  stale acknowledgement. `--acknowledge` writes `quality/spec-diff-acknowledgements.json`
  (`ess-diff-acknowledgements/1`, both digests, every id that is not compatible) or removes it
  when nothing is left; the reviewer reads that file's diff. `--negative` drops the grant of
  `weft.infrastructure.CreateCloud` to `ClusterOperator` in a copy of the release and requires
  exit 4 with `grant-removed`.
- `tools/check.d/50-spec-diff` runs both in `task check` (locally and in CI, which checks out
  with `fetch-depth: 0` and so has the tags).
- `AGENTS.md` names the tool.

### Planted defect and green run

Planted in the tree: `weft.infrastructure.Placement` loses `Kubernetes`
(`ess/1-infrastructure/infrastructure.yaml:25`); the specification stays valid.

```console
$ tools/check-spec-diff.sh
1 change(s): 0 widening, 1 narrowing, 0 other
compatibility: 1 breaking, 0 unknown, 0 compatible
fails --fail-on breaking-or-unknown: type/weft.infrastructure.Placement/variant-removed/Kubernetes
gate --fail-on breaking-or-unknown: 1 unacknowledged change(s) fail
check-spec-diff: ess/ against 0.1.0, exit 4
$ tools/check-spec-diff.sh --acknowledge
check-spec-diff: 1 breaking or unknown change(s) since 0.1.0 acknowledged in quality/spec-diff-acknowledgements.json
$ tools/check-spec-diff.sh
gate --fail-on breaking-or-unknown: passed (1 acknowledged)
check-spec-diff: ess/ against 0.1.0, exit 0
```

Restored the file; the acknowledgement is now stale and refuses:

```console
$ tools/check-spec-diff.sh
refused: quality/spec-diff-acknowledgements.json: 2 validation errors:
  - [conflicting_declaration] acknowledgements.after: the acknowledgements are for `after` 0afbc1f5…, and this comparison's `after` is ee379930… (…)
  - [undeclared_reference] acknowledgements.acknowledged[0]: `type/weft.infrastructure.Placement/variant-removed/Kubernetes` is not a change of this delta
check-spec-diff: ess/ against 0.1.0, exit 1
$ tools/check-spec-diff.sh --acknowledge
check-spec-diff: nothing breaking or unknown since 0.1.0; quality/spec-diff-acknowledgements.json removed
$ tools/check-spec-diff.sh; tools/check-spec-diff.sh --negative
no semantic change: these two revisions mean the same system
check-spec-diff: ess/ against 0.1.0, exit 0
check-spec-diff --negative: a dropped grant of CreateCloud is refused (exit 4, grant-removed)
```

### Findings

- S1 (fixed): no gate step classified a change of `ess/`; 38 breaking changes since the import
  merged unacknowledged. The step above holds it from now on.
- S2 (open, `ess` limit): a refusal added to a command whose earlier outcomes use `when_related`
  or `when_subject` is `unknown`, not `breaking` or `compatible` (83 of 292 changes that were not
  compatible; spec-diff.md: the rating is decided only over plain `when:` branches). Nearly every
  weft command has such guards, so the gate cannot tell a narrowing from a harmless addition and
  every change to `ess/` needs an acknowledgement. Decided by: an issue on beyond10x/ess
  asking for refusal ratings over row guards.

## Technique 1: mutation audit

Target: ESS's own interpreted target (`ess verify conform mutate --target interpreted`), not
weft-sim. weft has no implementation of its own yet; the generated Rust is being built
elsewhere.

Scope: `weft.shared` + `weft.infrastructure` (22 commands, 95 outcomes, 5 entities, 9 views), the
only domain besides `shared` that validates alone (`tools/check-domain.sh infrastructure`;
`inventory` and `artifacts` refuse with `undeclared_reference`). Its synthesis takes 5.4 s (266
scenarios, 346 refusals); the baseline passes 266 of 266 on the interpreted target. The whole
system was not audited: one synthesis of it takes 451 s and held 22 GB (goal.md), and the audit
synthesizes one suite per mutant.

```console
$ slot --timeout 30m ess verify conform mutate --path .check/h3/infra --target interpreted --report-out .check/h3/mut-all.json
mutation audit of weft v1 against one-time-protected-target: 203 mutant(s), 109 killed, 4 survived, 0 inconclusive, 25 stillborn, 58 unwitnessed, 7 equivalent (baseline: 266 scenario(s), 346 refusal(s)); 113 of 301 site(s) scored; 23 unavailable
exit 1, 550 s
```

- Killed, by class: error-swap 56, guard-negate 26, guard-boundary 12, from-drop 4,
  sets-retarget 4, guard-connective 3, transition-to 2, emit-drop 1, precedence-swap 1.
- Unwitnessed 58: 43 on an outcome the baseline's synthesis refuses (ESS-SYNTH-004 35,
  ESS-SYNTH-003 7, ESS-SYNTH-001 1), 4 that add a refusal (ESS-SYNTH-004, -008, -025), 11 whose
  suite does not change.
- Stillborn 25 (emit-drop 22: ESS-COMMAND-007 20, ESS-COMMAND-001 2; from-drop 2:
  ESS-ENTITY-011; guard-negate 1: ESS-COMMAND-012); unmutated 75 (sets-drop 49, sets-retarget
  18, transition-to 8); emit-swap unavailable 23 (no event of the same fields).

### The 4 survivors are killed by the baseline suite

`mutate` scores a mutant by the suite synthesized for it, run against the unchanged target. Each
survivor's suite chose inputs at which mutant and specification agree. Run the other way, the
baseline suite against the mutant's interpreter (a copy of the subset with the one edit; the
suite's `provenance.spec_digest` set to the mutant's, which `run` otherwise refuses):

| survivor | the baseline suite on the mutant |
| --- | --- |
| `sets-drop/ScaleControlPlane/scaled/control_plane` (`:619-620` removed) | exit 1, 3 failed: `ScaleControlPlane/outcome/scaled` and two disclosure copies |
| `guard-boundary/IssueJoinToken/expiry-not-future` (`:1051` `<= now` to `< now`) | exit 1: `IssueJoinToken/outcome/expiry-not-future/held/join_token_expires_at/0` |
| `guard-boundary/ProvisionNode/expiry-not-future` (`:949`) | exit 1: `ProvisionNode/outcome/expiry-not-future/held/join_token_expires_at/0` |
| `guard-boundary/ProvisionPoolNode/expiry-too-far` (`:1003` `>` to `>=`) | exit 1: `ProvisionPoolNode/outcome/unknown-pool/held/join_token_expires_at/86400` |

The baseline scaled 3 replicas to 1 and expected 1; the mutant's suite created the cloud with 1
replica and scaled it to 1. The boundary mutants' suites drop the `held/…/0` scenario that pins
`now` exactly.

### Planted defect and green run

The audit plants its own defects; the harness check is a mutant a scenario must kill, by hand:
`control_plane: input.control_plane` deleted from `ScaleControlPlane/scaled` in a copy of the
subset.

```console
$ slot ess verify conform run --target interpreted --path .check/h3/mut-scp --suite .check/h3/infra-suite-scp.json --report-format 2 --report-out …
  failed weft.infrastructure.ScaleControlPlane/outcome/scaled
  266 scenarios: 263 passed, 3 failed, 0 error, 0 unsupported
exit 1
$ slot ess verify conform run --target interpreted --path .check/h3/infra --suite .check/h3/infra-suite.json --report-format 2 --report-out …
  266 scenarios: 266 passed, 0 failed, 0 error, 0 unsupported
exit 0
```

### Findings

- M1 (open, `ess` limit, false positive of the audit): all 4 survivors are pinned by the
  baseline suite (table above). Decided by: an issue on beyond10x/ess, to score a mutant also by
  the baseline suite on the mutant's interpreter. Until then a survivor on weft is checked by
  hand as above.
- M2 (open, `ess` limit): synthesis gives inputs of one type the same literal (`DefinePriorityClass`
  `value` = `grace_seconds` = 2, `preempts` = `preemptible` = false; `CreateNodePool` `min_size` =
  `max_size` = 1), and an update the value already stored (`SetCloudPolicy` sets `Container` over
  `Container`). 6 sets-retarget mutants and the `SetCloudPolicy` sets-drop mutant are
  unwitnessed for it, and those rules are not pinned at all: `value: input.grace_seconds`
  in `DefinePriorityClass` (`ess/1-infrastructure/infrastructure.yaml:1285`) passes the baseline
  suite, 266 of 266, exit 0. Decided by: an issue on beyond10x/ess (distinct literals for sibling
  inputs), or authored scenarios run with `--scenarios` against the interpreter.
- M3 (open): 188 of 301 sites have no scored mutant, 43 of them because synthesis refuses the
  outcome on this subset (ESS-SYNTH-004 35). The rest of the system is not audited. Decided by:
  synthesis fast enough to run once per mutant (`docs/ess-gaps.md` gap 24), then `--component`
  per component.

## Technique 4: determinism

Run: `weft-sim explore --spec ess --seed 1 …` twice, then the two `result.json` compared with a
scratch script (`jq`): (1) the result without wall-clock fields (`elapsed_s`, `*_seconds`,
`steps_per_s`, …) and without the fields that record completion order (`first_reached`,
`traces`, `branch`, `feed`, `growth`) must be byte-equal; (2) every walk kept as a trace in both
runs must be equal step by step, and the first differing step is named.

| run | aggregate | traces |
| --- | --- | --- |
| 128 walks x 100 steps, 8 threads, twice | equal, 741,910 bytes | 63 walks kept in both, equal |
| the same, 1 thread against 8 | equal | 63, equal |
| `--from full-setup.yaml --settle 12`, 64 walks x 300 steps (23,116 steps, outcomes 1386 / 2525), twice | equal, 590,614 bytes | 64, equal |

Source scan of `tools/sim/src`: no randomness besides the seeded `Rng`; `HashMap`s are read by
key only; every `Instant::now` measures time, except the `--max-seconds` cap (D1).

### Planted defect and green run

Planted in `input_for` (`tools/sim/src/explore.rs:157`): `weft.accounts.CreateTenant` takes its
`name` from the wall clock's nanoseconds. Two runs of the planted binary:

```console
aggregate: equal (737291 bytes)
traces: 63 walk(s) kept in both
traces: first divergent step {"walk":0,"step":0,"command":"weft.accounts.CreateTenant","a":"created","b":"created","input_equal":false}
exit 1
```

The aggregate alone would have passed: a tenant's name changes no count. Restored and rebuilt:
`aggregate: equal (741910 bytes)`, `traces: equal step by step`, exit 0.

### Findings

- D1 (fixed in part, open): explore stops at `--max-seconds` (default 280 s) between epochs, so
  the number of walks, and every count after it, depends on the host's load. Same seed, a 4 s
  cap: 8 threads 736 walks, outcomes 1004 / 2525; 2 threads 224 walks, outcomes 902. Pass 2
  recorded 640 of 1024 walks in the cap at a load of 45-60. The run exited 0 and said so only in
  `notes`. Changed: it now prints `weft-sim: stopped after N s (--max-seconds) at W walks of
  1024: the result depends on the host's load, not only on --seed`. Open: whether a capped run
  fails (`task sim-explore-from` feeds the ratchet's `no-lost-outcomes`; the ratchet's
  `executed: {min: 1024}` would catch it once `ratchet validate` runs). Decided by: the owner.
- D2 (open, note): `first_reached` (the walk named to replay an outcome), the kept `traces`
  (63 of 64 the same), `branch`, `feed` and `growth` follow thread completion order, not the seed.
  Every kept walk is reproducible; only the choice of which walk is recorded varies. Decided by:
  whether replay locators must be stable across runs (then record the lowest walk index).

## Technique 5: metamorphic relations

Tool (new): `weft-sim relate` runs the setup of an experiment (and its controllers, `--settle`)
twice, the base and a derived run (the base without the steps whose label holds a `--drop` word,
then `--derived-append` steps), and compares the rows of every parameterless read-your-writes
view, and of every one-parameter view read with each bound variable. A value bound to a variable
reads `$name`; a minted id, a minted timestamp (a day of January 2020) and a minted text
(`generated-<n>`) are counter values of the interpreter, which move with every request sent, and
read `$minted`, `$minted-time`, `$minted-text`. `--observe` compares only rows that name an
observed variable, `--ignore` leaves named fields out. `tools/sim-relations.sh` (`task
sim-relations`) runs the two relations below, steps in `sim/relations/`; 0.4-0.7 s each.

Relations, from the design:

- Non-interference (`docs/domain-sketch.md` § 10 "Instances of different tenants never share a
  kernel"; `design:simulation-and-kpis` "Isolation"; AGENTS.md "Isolation ... default to deny").
  Base: the full setup plus globex's Deployer defining a workload in acme's formation `shop` with
  globex's account (`sim/relations/globex-defines-in-shop.yaml`, refused by `formation-foreign`,
  pass 1 W1). Derived: without globex's 11 steps. Every row naming one of acme's 13 variables is
  the same.
- Cordon round trip (sketch line 451, "cordoned" back to "ready"; spec comment
  `infrastructure.yaml:1188`). Derived: the base plus `CordonNode` and `UncordonNode` of a node of
  the shared pool. Every row is the same.

Green, on the unchanged specification (settle 12):

```console
weft-sim relate: base full-setup: 156 requests, 114 views, 130 rows, 59 out of scope
weft-sim relate: derived full-setup (derived): 139 requests, 114 views, 121 rows, 50 out of scope
weft-sim relate: 0 row difference(s) in 0.42 s (compile 0.39 s)
sim-relations: non-interference exit 0
weft-sim relate: base full-setup: 155 requests, 228 views, 252 rows, 0 out of scope
weft-sim relate: derived full-setup (derived): 157 requests, 228 views, 252 rows, 0 out of scope
weft-sim relate: 0 row difference(s) in 0.69 s (compile 0.37 s)
sim-relations: cordon-round-trip exit 0
```

### Planted defects

Non-interference: `formation-foreign` deleted from `DefineWorkload`
(`ess/4-applications/workloads.yaml:2171-2173`) in a copy of `ess/`.

```console
$ tools/sim-relations.sh .check/h3/plant/mr1
  base: not accepted as declared: globex-defines-workload-in-shop (weft.workloads.DefineWorkload) answered defined, declared formation-foreign
weft-sim relate: 1 row difference(s) in 0.43 s (compile 0.38 s)
weft.workloads.WorkloadList(formation_id=$formation_shop): only in base: {"account_id":"$account_b","component":"$shop_c_2","environment_id":"$env_shop","formation_id":"$formation_shop",…}
sim-relations: non-interference exit 1
```

The first version compared parameterless views only and passed this plant (0 differences: no
parameterless view lists workloads); reading the one-parameter views was added because of it.

Cordon round trip: `uncordon` goes `to: Cordoned` (`ess/1-infrastructure/infrastructure.yaml:261`)
in a copy.

```console
$ tools/sim-relations.sh .check/h3/plant/mr2
weft-sim relate: 20 row difference(s) in 0.68 s (compile 0.39 s)
weft.infrastructure.CloudNodes(cloud_id=$cloud_id): only in derived: {… "node_id":"$node_s_0", … "state":"Cordoned"}
…
sim-relations: cordon-round-trip exit 1
```

(The node stays cordoned, and globex's instance is placed on the other shared node.)

### Findings

- R1 (open, spec gap): `FormationRevision.number` is `{generated: true}`
  (`ess/4-applications/formations.yaml:903`, payload `:910`); "1 for the first revision of a
  formation, one more for each next" is a comment (`:155-156`), and the only declared rule is
  `number >= 1`. Non-interference without `--ignore number`: acme's revisions read `number` 73
  and 78 with globex present, 65 and 70 without. An implementation that numbers revisions from
  one counter for all tenants passes every scenario, and each tenant reads the others' activity
  from its own revision numbers. `tools/sim-relations.sh` ignores `number` and names this
  finding. Decided by: whether ESS can write a per-formation sequence (a counter on `Formation`
  that `CommitRevision` reads and raises), or an issue on beyond10x/ess.
- R2 (open, spec gap): the first relation runs differed in `NodeSessions.opened_at` and
  `SchedulingRequests.requested_at`, generated timestamps; nothing decides on those two, so
  `relate` now names minted times. The same form decides elsewhere: an expiry computed as "issued
  plus a lifetime" is `{generated: true}` and compared with `now` later:
  `weft.access.StartLogin` sets `expires_at` (`ess/2-tenancy/access.yaml:804`) that `ApproveLogin`
  compares (`:838`); `ExchangeFederatedToken` (`:1655`); `weft.identity.IssueIdentityDocument` and
  `RenewIdentityDocument` (`ess/5-runtime/identity.yaml:787`, `:827`) read by `:818` and
  `ExchangeToken` (`:1044`, `:1120`). AGENTS.md says a time weft-control decides on from "its
  clock plus a length" is a `weft.shared.StampedAt` input; `tools/check-stamped-time.sh` checks
  inputs only, so these 6 sets are outside it, and the interpreter's minted 2020 date makes them
  expired at once. Decided by: who stamps them (StartLogin is a user's command; a StampedAt input
  may be granted only to weft-control's actors), then the domain owners of `access` and
  `identity`.

## Not run

- 3, caller replay: weft has no caller yet. The implementation is being generated from `ess/`
  now; the first recorded caller traffic (the walking skeleton's CLI or tests) is its input.
- Mutation beyond `weft.infrastructure` (M3) and a mutation audit of weft-sim's own rules: cost
  above value until synthesis is faster.
- A third relation, the order of independent commands ("acme's and globex's steps swapped"): not
  built; the non-interference relation covers the same rows, and a plant for order dependence
  was not found cheaply.
- Techniques 2, 6 and 8 ran in passes 1 and 2.

## Files

- New: `tools/check-spec-diff.sh`, `tools/check.d/50-spec-diff`, `tools/sim/src/relate.rs`,
  `tools/sim-relations.sh`, `sim/relations/globex-defines-in-shop.yaml`,
  `sim/relations/cordon-uncordon.yaml`, this file.
- Changed: `tools/sim/src/main.rs` (`relate`), `tools/sim/src/simulate.rs` (`setup_rows`),
  `tools/sim/src/lib.rs`, `tools/sim/src/explore.rs` (D1 warning), `Taskfile.yml`
  (`sim-relations`), `AGENTS.md`.
- Scratch, not committed: the subset, mutant copies and probes under `.check/h3/`.

```findings
- file: tools/check.d/50-spec-diff
  category: spec-gap
  severity: warning
  verdict: CONFIRMED
  origin: pre-existing
  message: no gate step classified a change of ess/, and 38 breaking changes since the import merged unacknowledged; fixed by tools/check-spec-diff.sh in task check
- file: ess/4-applications/workloads.yaml
  category: ess-limit
  severity: note
  verdict: CONFIRMED
  origin: pre-existing
  message: ess verify diff rates a refusal added beside when_related or when_subject guards unknown, so 83 of 292 changes need an acknowledgement whether they narrow or not
- file: ess/1-infrastructure/infrastructure.yaml
  line: 619
  category: false-positive
  severity: note
  verdict: CONFIRMED
  origin: pre-existing
  message: the 4 mutation survivors (ScaleControlPlane sets-drop, three join-token now boundaries) are killed by the baseline suite on the mutant's interpreter; mutate scores only the mutant's own suite
- file: ess/1-infrastructure/infrastructure.yaml
  line: 1285
  category: ess-limit
  severity: warning
  verdict: CONFIRMED
  origin: pre-existing
  message: synthesis gives sibling inputs of one type the same literal, so DefinePriorityClass value set from grace_seconds passes the baseline suite and 7 set mutants are unwitnessed
- file: ess/1-infrastructure/infrastructure.yaml
  category: missing-scenario
  severity: note
  verdict: CONFIRMED
  origin: pre-existing
  message: 188 of 301 mutation sites of weft.infrastructure have no scored mutant, 43 because synthesis refuses the outcome, and no other domain was audited
- file: tools/sim/src/explore.rs
  line: 693
  category: spec-gap
  severity: warning
  verdict: CONFIRMED
  origin: pre-existing
  message: the --max-seconds cap makes the walk count and every explore count depend on host load while the run exits 0; now warned on stderr, whether it fails stays open
- file: tools/sim/src/coverage.rs
  category: spec-gap
  severity: note
  verdict: CONFIRMED
  origin: pre-existing
  message: first_reached, the kept traces, branch, feed and growth follow thread completion order, so replay locators differ between runs of one seed
- file: ess/4-applications/formations.yaml
  line: 903
  category: spec-gap
  severity: warning
  verdict: CONFIRMED
  origin: pre-existing
  message: FormationRevision.number is generated and per-formation numbering is only a comment, so a global counter passes and leaks other tenants' activity into a tenant's revision numbers
- file: ess/2-tenancy/access.yaml
  line: 804
  category: spec-gap
  severity: warning
  verdict: CONFIRMED
  origin: pre-existing
  message: expiries computed from weft-control's clock plus a lifetime are generated, not StampedAt inputs, in StartLogin, ExchangeFederatedToken, IssueIdentityDocument, RenewIdentityDocument and ExchangeToken, outside the stamped-time check
```
