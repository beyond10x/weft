---
format: aep.planning-md/3
id: epic:consoles
kind: epic
status: active
title: 'Consoles: the cloud operator''s and the tenant''s UI, specified in ess-ui/1'
relations:
- serves: vision:weft
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-28T00:44:54Z", actor: "human:operator", revision: 2, decided_on: {"recorded":{"approval":1}}}
- {from: "proposed", to: "active", at: "2026-11-28T00:44:54Z", actor: "human:operator", revision: 3, decided_on: {"recorded":{"approval":1}}}
---
# Consoles: the cloud operator's and the tenant's UI, specified in ess-ui/1

## Outcome

Two apps of weft, each specified as an `ess-ui/1` document checked against weft's model and drawn
by weft's own renderer: the operator console for the actor that runs the cloud
(`weft.actors.ClusterOperator`) and the account console for the people who deploy to it.

## Order

The owner, 2026-11-28 about 00:45 CET, typed in a session: use ESS UI spec to model the control
plane of the operator, and in an entirely different app the end-user UI, also in ESS UI spec; do
not use ESS's renderer, build an own renderer with good-looking components.

## Scope

- `product/console/ui/`: the two documents and their fixtures.
- `product/console/`: the renderer, the component library, two entry points.
- A gate hook that checks both documents against the model and runs the renderer's tests.

## Acceptance

- `ess ui check --model ess` reports no error on either document.
- Every page of both apps loads with no axe violation and no horizontal overflow at 390 px, in
  light and dark.
- No page reads a view or sends a command its actor is not granted.
