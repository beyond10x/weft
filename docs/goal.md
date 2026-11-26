# weft: the current goal (the owner, 2026-11-26)

"Spec fully validating and being sound, and several hardening passes."

Measured as:

1. `ess specify validate --path ess --strict-requires`: valid, 0 warnings.
2. `tools/check-layers.sh`: 0 references to a higher layer.
3. Every command, view and event is owned by a component (`ess/components.yaml`) and callable by at
   least one actor (`weft.actors`), except where a comment names why not.
4. `ess verify conform synthesize` writes the suite; the run against the interpreted target is
   green; unscenarioed and unanswered counts are recorded and each remaining one has a reason.
5. `protocols/*.yaml`: `ess specify protocol validate` valid, `ess verify protocol explore` passed.
6. At least three hardening passes (skill `ess:hardening`), each recorded in
   `docs/hardening/pass-<n>.md` with its findings and what changed.

Progress is recorded below, newest first.

## Progress

- 2026-11-26: CI/CD design (`design:ci-cd-support`) specified: pipelines domain (6 entities, 22 commands), federation in access, attestations in artifacts, preview environments in formations; 22 files valid, every command has a component and an actor. Soundness: every measured domain has 0 failed scenarios of its own; deployments and pipelines reach only their first outcomes (root: Account, gap 9). PR 835 binary makes `CreateAccount/created` reachable; round 3 (gaps 9b, 22) running. Hardening pass 1 (isolation guards) running.
- 2026-11-26: first synthesis on the 17-domain snapshot (before the claims move): 451 s with `--jobs 4`, 4118 scenarios, 88 MB suite. Outcomes with no scenario: 305 `ESS-SYNTH-001`, 358 `ESS-SYNTH-003`. Skipped view reads: 5268 `ESS-SYNTH-005`, 3508 of them on `cluster_id == param.cluster_id`, 1729 on `principal_id == param.principal_id`. Synthesis exceeds the 120 s test step, so the suite is a committed generated file with a digest of its sources (`~/beyond10x/AGENTS.md` § Tests stay fast), regenerated only in the `slow` lane; per component with `--component` once `ess/components.yaml` exists.
- 2026-11-26: Claim and Binding moved to `5-runtime/claims.yaml`; `weft v1 — 20 file(s), valid`, layers green. Sketch §§ 14-16 added (reconciliation, health, observability); wave 3 running: observability, workload health, actors.
- 2026-11-26: layers derived and applied; `tools/check-layers.sh` green; 54 entity identifiers and
  7 value types moved to `weft.shared`; per-domain integration pass running (relations, shared types).
