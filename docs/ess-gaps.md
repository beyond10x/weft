# ESS gaps found while specifying weft (ess 0.99.2)

Each entry: what weft needed, what ESS does, where it shows. Input for stories in the ESS planning
store (`~/beyond10x/ess/.engineering/planning/`). Newest last.

## Language

1. A component must own a domain (ESS-COMPONENT-007). weft-node and weft-edge accept commands of
   protocols but own no domain, so they are not components. `ess/components.yaml`.
2. Only the owning component accepts a domain's commands (ESS-COMPONENT-004): a user API that fronts
   commands of control-plane domains cannot be declared. Sketch § 14.
3. A binding invokes one command per event; no fan-out. weft uses level-form gap views instead.
4. No timer or time trigger: time-driven steps are actor commands with `now` guards. `now` is
   refused in an `affects:` filter, so they run once per row.
   Level and time steps (design:components, 2026-11-26) are asked as ESS story (store `053bcb1333`, third in the ess order) `story:a-component-declares-level-and-time-steps-that-validate-checks-and-the-interpreter-runs`: component `steps:` lowered to binding causes, a level step runs once per answered row, a loop has a named bound.
5. No set effect beside a create, no `{increment:}` on a set effect, no `affects:` beside
   `instances:` (all ESS-COMMAND-009): a counter on a parent cannot be kept by the child's create.
6. Row-set count compares only with a literal; guards cannot compare enum order or a `Duration`;
   no `==` over a Map; no power function (backoff `n`-th value).
7. `system: true` actors cannot be invoked by bindings.
8. A view reads one entity and one reference hop: no walk up a tree (effective roles over the
   account tree).

## Synthesis

9. A creating command with a `when_related: {via: input.…}` guard beside a uniqueness row set over
   the entity it creates makes that entity unreachable for every later command ("which no input
   reaches"). Every child creator in formations has this shape; accounts' `CreateAccount/created`
   too. Reproduced on a small spec by the formations soundness pass (2026-11-26). Root cause (accounts
   pass): `selects` in `crates/verify/ess-conformance/src/synthesize/related_guard.rs:2026` never
   evaluates a `when_related` row-set guard. Blocks Account, Quota, RoleBinding and everything above.
10. A forced-external scenario is emitted for a row synthesis could not arrange; the target answers
    not-found instead of the external refusal. Worked around with `before_row: true`.
11. Whole-system synthesis of weft takes 451 s (4118 scenarios, 88 MB, `--jobs 4` at about one
    core); `--component` on one domain was no faster (8.5 min). weft synthesizes per domain on the
    subset of domains it needs (`tools/synth-domain.sh`, 80-130 s).
12. A row whose fields a command copies from another row (`SnapshotVolume` copies size and zone from the volume) cannot be arranged; with fixed values it can. Reproduced by the resources soundness pass (2026-11-26).
13. A scenario that opens with a send of a command synthesis has no expected outcome for marks every row untracked, so a later one-row-per-key guard is refused (`not-own-node` in nodes). Probe in job scratch nodes-probe (2026-11-26).
14. A timestamp the system stamps itself (`last_seen_at`, `lease_expires_at`, generated) cannot be arranged; only a time from an input can. Taking it from the input would let a node supply its own clock.
15. An input-only guard (negative amount) still needs the subject row before synthesis emits its scenario.
16. A timestamp pair arranged as 2020-01-01 and now-1s produced a wrong expected answer (`IssueStatement/empty-period`); worked around with `period_start > now - 1s`. A uniqueness check on two Timestamp fields is "Unknown". A list of minimum length 1 is arranged only empty.
17. A creating command is never used to set up its own repeat call (`JoinNode/token-used`); a test command that sets the field made it reachable (cluster pass).
18. An entity rule `signals.count >= 1` stops synthesis from trying the empty list the guard refuses (`ConfigureTelemetrySink/no-signals`); confirmed on a two-file model (observability pass).

Fix for gap 9 (`via:` form): beyond10x/ess PR 835, story `a-creator-beside-a-named-row-and-its-own-row-set-is-a-route`. Not covered: the `where:` lookup form, the optional-reference form (gap9b), story `a-creator-under-a-row-set-and-when-is-arranged`. Binary for measurement: scratch `bin-fix/ess`.
19. A row set or a move picked by membership in a list (`exists: {in: subject.device_ids}`) is refused; only equality to an input or subject field pins a row set. `each:` takes only input lists (claims pass).
20. A row set keyed on a field that holds a generated ID is never decided (claims `CommitVolumeWriter`). A list guard may not sit beside other guards.
- 2026-11-26 measurement with the PR 835 binary: accounts 1624 -> 1895 scenarios, own refusals 63 -> 51, but failed 16 -> 50; scheduling 1749 -> 2078 scenarios, refusals 56 -> 56, failed 4 -> 83. New failures: "no declared outcome was reached" on formations no-such-* and cluster disclosure cells. Sent back to the fix agent.
21. Reading an artifact row in `DefineWorkload` (4 guards) makes synthesis run without end (40 min, 16 GB on a test copy). Three inputs that must change together are not found. A list-membership guard on a related row limits how many created rows later commands get (workloads pass).
- 2026-11-26 PR 835 round 2: accounts subset 1868 scenarios, 4 failed (the 4 also fail on main; `RecordSnapshotTaken/no-artifact`, fixed in the live tree since). Scratch `bin-fix/ess` updated.
- 2026-11-26 secrets subset with the PR 835 binary: 2199 scenarios, 1 failed, synthesis 681 s (local slot). `accounts.CreateAccount/created` now has a scenario; `secrets.CreateSecret/created` still none.
22. A synthesized variant that leaves out an optional input keeps the expected outcome of the full send, though an earlier `when` guard on that input answers first (`secrets.CreateSecret/no-such-environment`, PR 835 binary). Sent to the PR 835 branch with gap 9b (absent optional reference treated as a required row).
- 2026-11-26 PR 835 round 3 (pushed fa93a1bae4): gap 22 and gap 9b fixed (the interpreter skips an absent Optional reference, `interpret/execute.rs:1634`; synthesis now leaves them out too). Secrets subset: 2264 scenarios, 4 failed, 39 unsupported, synthesis 817 s; `CreateSecret/created` reached.
23. A row set beside a chained read (`[workload_id, formation_id]`) gives a wrong expected payload: the chained read re-points to another formation (`workloads.CreateInstance/created`; also on the binary before round 3). Needs its own ESS story.
24. Synthesis with caller values runs the ordinary path once with every command sent by `first`, and again per command that reads the caller (`docs/design/caller-values.md` § Synthesis; `synthesize/caller.rs`). After hardening pass 1 (about 90 caller-reading commands in the workloads subset), workloads synthesis ran past 30 min and was killed (exit 124, 2026-11-26); comparable subsets took 442-583 s before. Inferred cause, not yet measured per command. Needs an ESS story: synthesize the `second` assignment only for the branches that read the caller.
    Measured in the ess repository (2026-11-26): the workloads subset with one caller-reading command still runs over 30 min (slot jobs 69368, 69474); with none it takes 444-1130 s. The cost is a pass per command granted caller attributes (`caller/readings.rs:42-58`, `caller.rs:828-840`), not the number of reads. A 1 h budget is not enough. ESS story (beyond10x/ess store `053bcb1333`, first in the ess order): `story:a-synthesis-with-one-command-that-reads-the-caller-costs-at-most-a-constant-factor-of-the-same-model-without-the-read`.
25. A comparison over an Optional field a row holds absent (`{cleared: true}`) is Unknown in a row-set selector, a related predicate, a subject predicate and an `affects:` filter, though `defined()` on the same field reads False; `ess specify validate --strict-requires` accepts the guard, and the interpreter answers `Unsupported` for every input that meets such a row. weft writes `defined(x)` first (hardening pass 3, 12,587 answers on seed 1, 1024 walks). Repro, 3 commands on the weft spec before that pass: `CreateTenant`, `CreateAccount {name: prod}` (top-level, `parent_account_id` cleared), `CreateSubAccount {parent_account_id: <that account>, name: prod}` -> unsupported at `name-taken` (`where: {all: [parent_account_id == input.parent_account_id, name == input.name, state == Open]}`). Wanted: either the comparison reads False against an observed absence, or validate names the guard.
    ESS story (store `053bcb1333`, second in the ess order): `story:a-comparison-over-an-optional-field-a-row-holds-absent-is-decided-or-named-by-validate-and-not-left-unknown-for-the-interpreter-to-refuse`. Decided by ESS: validate names the guard and prints `defined(f) and f == x`; the meaning does not change.
