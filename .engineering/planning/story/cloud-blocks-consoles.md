---
format: aep.planning-md/3
id: story:cloud-blocks-consoles
kind: story
status: implemented
title: The account and operator consoles drive the vendored commands and views
relations:
- decomposes: epic:cloud-blocks
- serves: vision:weft
- depends_on: story:cloud-blocks-policies
- depends_on: story:cloud-blocks-cluster-pinning
- depends_on: story:cloud-blocks-runtime-fields
scope:
- confidence: cited
  path: product/console/src/hosts/index.tsx
- confidence: cited
  path: product/console/src/renderer
- confidence: cited
  path: product/console/test
- confidence: cited
  path: product/console/ui/account.ui.yaml
- confidence: cited
  path: product/console/ui/fixtures
- confidence: cited
  path: product/console/ui/operator.ui.yaml
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-28T08:38:23Z", actor: "human:operator", revision: 5, decided_on: {"recorded":{"approval":1}}}
- {from: "proposed", to: "active", at: "2026-11-28T10:10:15Z", actor: "human:operator", revision: 6, decided_on: {"recorded":{"approval":1}}}
- {from: "active", to: "implemented", at: "2026-11-28T13:47:54Z", actor: "human:operator", revision: 7, decided_on: {"recorded":{"test_result":1,"approval":1}}}
---
## Epic

Decomposes `epic:cloud-blocks`: the account and operator consoles use the renamed domains.

## Outcome

1. `account.ui.yaml` and `operator.ui.yaml` name the vendored commands and views.
2. The 11 fixtures, the renderer (`grants.ts`, `nodes/Form.tsx`, `runtime.tsx`) and the hosts
   follow.

## Scope

`product/console/ui/account.ui.yaml`, `product/console/ui/operator.ui.yaml`,
`product/console/ui/fixtures/`, `product/console/src/renderer/`, `product/console/src/hosts/index.tsx`,
`product/console/test/`.

## Acceptance

1. `tools/check.d/40-console.sh` exits 0 (`ess ui check`, vitest, build, Playwright).

## Rules for this work

- Merge into the integration branch `move/vendor-cloud-blocks`; `main` gets it once the whole gate
  is green.
- Builds and tests run through `slot --here`. No test over 60 s; the test step ends within 120 s. A
  pool job ends within 10 minutes unless it names its time with `slot --timeout` (at most 30m). No
  Python in anything committed. No network request in the test step.
- One owner per domain file; parallel edits are checked with `tools/check-overlay.sh`.
