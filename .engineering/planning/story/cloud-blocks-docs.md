---
format: aep.planning-md/3
id: story:cloud-blocks-docs
kind: story
status: proposed
title: Docs pages and the spec-diff acknowledgements follow the renamed domains
relations:
- decomposes: epic:cloud-blocks
- serves: vision:weft
- depends_on: story:cloud-blocks-rust
- depends_on: story:cloud-blocks-consoles
- depends_on: story:cloud-blocks-sim-pin
- depends_on: story:cloud-blocks-layers
scope:
- confidence: cited
  path: quality/spec-diff-acknowledgements.json
- confidence: cited
  path: tools/docs-spec-pages.sh
- confidence: cited
  path: website/data/status-proofs.json
- confidence: cited
  path: website/docs/concepts
- confidence: cited
  path: website/docs/run/consoles.md
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-28T08:38:23Z", actor: "human:operator", revision: 6, decided_on: {"recorded":{"approval":1}}}
---
## Epic

Decomposes `epic:cloud-blocks`: the docs and the spec diff follow the renamed domains, and the
whole gate passes.

## Outcome

1. `tools/docs-spec-pages.sh` generates pages for the vendored domains; its check and test follow.
2. `website/docs/concepts/`, `website/docs/run/consoles.md`, `website/docs/concepts/managed-services.md`
   and `website/data/status-proofs.json` (23 -> 28 domains) name the renamed domains.
3. `quality/spec-diff-acknowledgements.json` acknowledges each breaking change of the move
   (`check-spec-diff.sh --acknowledge`, then reviewed).

## Scope

`tools/docs-spec-pages.sh`, its check and test, `website/docs/concepts/`,
`website/docs/run/consoles.md`, `website/docs/concepts/managed-services.md`,
`website/data/status-proofs.json`, `quality/spec-diff-acknowledgements.json`.

## Acceptance

1. `task docs-check` exits 0 on the final tree.
2. `tools/check.d/50-spec-diff` exits 0 on the final tree.
3. `slot --here task check` exits 0 on the integration branch with `cloud.lock` at
   `CLOUDSPEC_REV`.

## Rules for this work

- Merge into the integration branch `move/vendor-cloud-blocks`; `main` gets it once the whole gate
  is green.
- Builds and tests run through `slot --here`. No test over 60 s; the test step ends within 120 s. A
  pool job ends within 10 minutes unless it names its time with `slot --timeout` (at most 30m). No
  Python in anything committed. No network request in the test step.
- One owner per domain file; parallel edits are checked with `tools/check-overlay.sh`.
