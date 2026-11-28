---
format: aep.planning-md/3
id: story:console-ui
kind: story
status: active
title: Operator and account consoles from ess-ui/1 documents, drawn by weft's own renderer on fixtures
relations:
- decomposes: epic:consoles
- serves: vision:weft
scope:
- confidence: cited
  path: product/console/
- confidence: cited
  path: tools/check.d/40-console.sh
- confidence: cited
  path: website/docs/run/consoles.md
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-28T00:44:54Z", actor: "human:operator", revision: 3, decided_on: {"recorded":{"approval":1}}}
- {from: "proposed", to: "active", at: "2026-11-28T00:44:54Z", actor: "human:operator", revision: 4, decided_on: {"recorded":{"approval":1}}}
---
# Operator and account consoles from ess-ui/1 documents, fixtures only

## Context

epic:consoles. The two documents run on fixtures; reading the control host's HTTP surface is a
later story.

## Acceptance

Running `task console` checks `product/console/ui/operator.ui.yaml` and
`product/console/ui/account.ui.yaml` against `ess/` with no error finding, builds both apps, and
passes the renderer's unit tests and the Playwright run (every page loads, no axe violation, no
horizontal overflow at 390 px, light and dark).

## Scope

- product/console/
- tools/check.d/40-console.sh
- website/docs/run/consoles.md
