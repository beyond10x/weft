---
format: aep.planning-md/3
id: story:cloud-blocks-layers
kind: story
status: draft
title: Layer and domain-map checks treat ess/cloud as one layer below weft's own files
relations:
- decomposes: epic:cloud-blocks
- serves: vision:weft
- depends_on: story:cloud-blocks-vendor
revision: 1
---
## Epic

Decomposes `epic:cloud-blocks`: weft's layer checks and domain map know the vendored layer.

## Outcome

1. `tools/check-layers.sh` and the domain map treat `ess/cloud/` as one layer below weft's files:
   a weft file may reference it, it may not reference weft, and a weft file referencing a higher
   weft layer still fails.
2. `tools/synth-domain.sh`, `tools/check-domain.sh` and `tools/check-overlay.sh` read the
   vendored files.
3. `docs/layers.md`, the docs page and `AGENTS.md` say which files are vendored and that they are
   changed only by `task vendor-update`.

## Scope

`tools/check-layers.sh`, `tools/gen-domain-map.sh`, `tools/check.d/30-domain-map.sh`,
`tools/test-domain-map.sh`, `tools/synth-domain.sh`, `tools/check-domain.sh`,
`tools/check-overlay.sh`, `docs/layers.md`, `website/docs/specification/layers.md`, `AGENTS.md`.

## Acceptance

1. `tools/test-domain-map.sh` passes with a fixture in which a weft file references a higher weft
   layer (refused) and one references `cloud/` (accepted).
2. `task layers` exits 0 on the tree of `story:cloud-blocks-policies`.

## Rules for this work

- Merge into the integration branch `move/vendor-cloud-blocks`; `main` gets it once the whole gate
  is green.
- Builds and tests run through `slot --here`. No test over 60 s; the test step ends within 120 s. A
  pool job ends within 10 minutes unless it names its time with `slot --timeout` (at most 30m). No
  Python in anything committed. No network request in the test step.
- One owner per domain file; parallel edits are checked with `tools/check-overlay.sh`.
