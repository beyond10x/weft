---
format: aep.planning-md/3
id: decision-blocker:cloudspec-source-translator
kind: decision-blocker
status: cleared
title: Nobody has decided which Translator a Cloudspec deployment source names, now that weft writes no translator
relations:
- blocks: epic:cloudspec-submission
revision: 3
transitions:
- {from: "open", to: "cleared", at: "2026-11-28T10:50:09Z", actor: "human:operator", revision: 3, decided_on: {"recorded":{"approval":1}}}
---
# Nobody has decided which Translator a Cloudspec deployment source names, now that weft writes no translator

## The relation

`weft.deployments.DeploymentSource` (kind `Cloudspec`) → `weft.deployments.Translator`: many-to-one,
required. The vendored `weft.deployments.SubmitDeploymentSource` takes a required `translator_id`
(`ess/cloud/11-translation/deployments.yaml:808`, unchanged at cloudspec fdcf7e8) and refuses
`no-translator` and `translator-not-for-source` unless a `Registered` Translator row has
`source_kind == Cloudspec` and `source_format_version == cloudspec-weft-submission/1`; the
Translator row needs an `artifact_digest` that references `weft.artifacts.Artifact`. The
Translation that follows references it too (relation `translator`, same file, line 420).

Against that: R85 = A ("no translator inside weft"), `AGENTS.md` § Ecosystem specs ("weft writes no
translator"), and `docs/domain-sketch.md` § Translation moved to cloudspec, which leaves the
records open: "Their removal, or their reduction to a record of cloudspec's translation, is a later
specification change". No ess/1 document and no decision says what the Translator row of a
Cloudspec source stands for, who registers it, or which artifact it names.

## Options

- **A (default)**: a record, not a translator. The operator registers one Translator row for
  (`Cloudspec`, `cloudspec-weft-submission/1`) whose `translator_version` is the cloudspec commit of
  `CLOUDSPEC_REV` and whose artifact is cloudspec's source at that commit, registered by digest.
  The vendored spec stays as it is; reversible by `RetireTranslator`.
- **B**: cloudspec changes its block so a `Cloudspec` source names no translator (`translator_id`
  optional, or the two refusals skip `Cloudspec`); weft takes it with a later pin move.

## What it stops

Epic `cloudspec-submission`, outcome 2 past the offline checks (weft's upload route and
`SubmitDeploymentSource` with kind `Cloudspec`) and outcome 3 (plan and run of the snippet golden).
No story was drafted for them. `story:cloudspec-submission-reader` (the offline checks) does not
depend on the answer.
