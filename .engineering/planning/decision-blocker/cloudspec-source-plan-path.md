---
format: aep.planning-md/3
id: decision-blocker:cloudspec-source-plan-path
kind: decision-blocker
status: cleared
title: Nobody has decided whether a Cloudspec source reaches its plan through a Translation record or without one
relations:
- blocks: epic:cloudspec-submission
revision: 3
transitions:
- {from: "open", to: "cleared", at: "2026-11-28T10:50:09Z", actor: "human:operator", revision: 3, decided_on: {"recorded":{"approval":1}}}
---
# Nobody has decided whether a Cloudspec source reaches its plan through a Translation record or without one

## The relation

`weft.deployments.Plan` → `weft.deployments.Translation` for a source of kind `Cloudspec`. In the
vendored spec every plan names one: `CreatePlan` takes a required `translation_id`
(`ess/cloud/11-translation/deployments.yaml:1240`), and its `no-such-translation` refusal reads it;
`Plan` declares the relation `translation` (line 527), and `Translation` declares
`formation_revision` and owns its `CoverageEntry` rows (relation `coverage`).

For a Cloudspec source weft builds the revision from the architecture's snippet parts itself (R85
= A), and the submission already carries cloudspec's coverage. Undecided: whether that coverage is
recorded as a Translation of weft (one per accepted submission) that the plan names, or whether a
Cloudspec plan names no translation. `docs/domain-sketch.md` § Translation moved to cloudspec
leaves it open ("removed, or reduced to a record of cloudspec's translation that a plan names",
`epic:agent-workloads` § Translation moves to cloudspec).

## Options

- **A (default)**: through a Translation. `TranslateSource` opens it, one `RecordCoverageEntry` per
  entry of the submission's `generate weft` coverage step, `CompleteTranslation` names the revision
  weft built from the snippet parts, and `CreatePlan` names the translation. The vendored spec stays
  as it is; the coverage is queryable in `weft.deployments.CoverageReport`.
- **B**: no Translation. weft builds the revision with the formation commands and plans it without
  one; needs `translation_id` optional in cloudspec's block and a later pin move. The coverage stays
  only in the stored submission.

## What it stops

Epic `cloudspec-submission`, outcome 3: an accepted submission becomes a plan whose instances come
from the snippet workloads, and the walking-skeleton path runs the `snippet` golden to a finished
job. No story was drafted for it.
