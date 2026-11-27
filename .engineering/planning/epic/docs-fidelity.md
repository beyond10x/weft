---
format: aep.planning-md/3
id: epic:docs-fidelity
kind: epic
status: active
title: 'Docs site: checked claims and measured visual fidelity'
relations:
- serves: vision:weft
- derived_from: design:quality-kpis
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-27T10:52:41Z", actor: "human:operator", revision: 2, decided_on: {"recorded":{"approval":1}}}
- {from: "proposed", to: "active", at: "2026-11-27T10:52:41Z", actor: "human:operator", revision: 3, decided_on: {"recorded":{"approval":1}}}
---
## Goal

The owner, 2026-11-27: "later i would like to see the pub website docs be way nicer in fidelity,
etc ... you should look at them too". The public site (https://beyond10x.github.io/weft/) went live
the same day. This epic makes it correct, checked and visibly better, measured in rounds.

## Outcome

1. A checker holds the docs KPIs of `design:quality-kpis` at 0: `docs.status-claims-false`,
   `docs.commands-refused`, `docs.planned-without-banner`, `docs.dead-repo-links` and
   `repo.agents-md-dead-paths`; it runs in the docs build of `pages.yml` and in `task check`.
2. The status page is generated from or checked against the tree (`status.json` items name a file
   or a command that proves them), not only written by hand.
3. The two dead paths in `AGENTS.md` (`surfaces/http/weft-api.yaml`, `surfaces/cli/weft.yaml`) are
   fixed or marked planned.
4. Visual fidelity in measured rounds (the `ui-iteration-loop` skill): a screen-only user bot, layout
   and accessibility checks and a `baseline.json` ratchet in the docs build; each round names what
   it changed and the numbers before and after.
5. The pages carry real figures where they explain more than text: the architecture (control host,
   nodes, substrate), the domain map of `ess/`, and the search animation of
   `epic:search-animation` once it exists.
6. The planned components (`ess/components.yaml`, `design:components`) and managed services
   (`ess/7-services/catalog.yaml`: offering kinds, plans, service instances, bindings, products)
   each have a page, generated from the specification and marked Planned. The owner, 2026-11-27:
   "what i could not find yet at least in the docs online are our planned components/managed
   services".

## Not in this epic

- Content for parts that are not built (they stay marked Planned).

## Rules

No claim on the site that the tree does not prove. No person, host, session or private repository
name on the site.
