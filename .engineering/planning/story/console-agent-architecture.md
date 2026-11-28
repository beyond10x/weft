---
format: aep.planning-md/3
id: story:console-agent-architecture
kind: story
status: draft
title: 'Account console: a deployed agent architecture drawn by agenticspec render, and one common agent card'
relations:
- decomposes: epic:agent-workloads
- serves: vision:weft
- depends_on: story:agent-architecture-units
- depends_on: story:agenticspec-render-contract
revision: 1
---
## Epic

Decomposes `epic:agent-workloads`, outcome 3: the account console shows a deployed agent
architecture as agenticspec draws it, and every agent workload as one common agent card. Builds on
`story:console-ui` (epic `consoles`: `product/console/ui/account.ui.yaml`, weft's own React
renderer with host slots in `product/console/src/hosts/`). Depends on
`story:agent-architecture-units` (component origin, revision architecture) and
`story:agenticspec-render-contract` (the render contract it embeds). Starts after `story:console-ui`
is merged: that tree is in active work.

## Context

- The owner, 2026-11-28 about 02:08 CET: "If we deploy a thing from agenticspec, we can also render
  it; the rendering is provided from agenticspec. There is probably also some common rendering for
  the agent itself" (epic § Order).
- agenticspec offers two things that render:
  - `agenticspec sim render <architecture.yaml> --out page.html` (agenticspec `README.md:69-78`),
    the function `agenticspec_render::page(architecture, result)`
    (`crates/agenticspec-render/src/lib.rs`): one self-contained HTML page with inline CSS, script
    and SVG, no network, output a function of its inputs only; each part is a `<g data-part=...>`,
    each link a `<path data-from=... data-to=...>`.
  - The `embed.*` pages of `ui/agenticspec.ui.yaml` (`embed.session`, `embed.timeline`,
    `embed.events`; `ui/README.md:14-25`): one session of agenticspec's session store, served by
    `agenticspec serve` from `sessions.db` through the `session-view` component.

## Decision: the architecture page embeds the render output

The architecture tab frames the page that `agenticspec_render::page` makes from the revision's
architecture document (`weft.formations.FormationRevision.architecture`), in an
`<iframe sandbox srcdoc>` host component. Why: the drawing is of the architecture, and the render
output is a pure function of the document weft already holds by digest; it needs no server, no
store and no network, so it works on every placement. The `embed.*` pages draw one session of an
agent run from agenticspec's session store, which weft does not run; they are the later per-run
view, once an agent runtime records its runs (agenticspec design story `agent-run-lift`), not the
architecture.

Live state over the drawing: the host component reads weft's views and marks each `data-part` by
the component that `weft.formations.FormationComponents.origin.object` names: state, ready and
desired replicas, last Job result. This needs the overlay contract of
`story:agenticspec-render-contract` (stable `data-part` ids, an embed mode, a state input); until it
exists the page is shown without the overlay.

## The common agent card

A host component `AgentCard` (`product/console/src/hosts/`), one per component whose package was
`Agent`, single or from an architecture. It shows: component and origin (unit, parts), shape, state,
ready and desired replicas (Service), last and running Jobs with result and duration (Job), last
fired (Schedule), model routes, isolation. Reads:

| field | view |
|---|---|
| component, origin | `weft.formations.FormationComponents` |
| workload of the component | `weft.workloads.WorkloadList` (per formation, field `component`) |
| shape, settings, isolation | `weft.workloads.WorkloadDetail` |
| replicas, state (Service) | `weft.workloads.ServiceDetail` (`replicas`, `running_replicas`), `weft.workloads.InstanceList` |
| Jobs | `weft.workloads.JobList` |
| last fired (Schedule) | `weft.workloads.ScheduleList` (`last_fired_at`) |

Gaps: a workload's `package_kind` is the plain kind its agent became (Image or Process), so the
card finds agents by the component's `origin` or by a field this story asks of
`story:agent-package-settings` (the `AgentSettings` kept on the workload); `ScheduleList` has no
next firing time; `JobList` has no start or end time.

## Scope

- `product/console/ui/account.ui.yaml` (a page `formations.architecture` and the card on the
  formation page), `product/console/ui/account.hosts`, `product/console/ui/fixtures/account/`
- `product/console/src/hosts/` (`AgentArchitecture`, `AgentCard`)
- the pinned render output for the fixtures: a page made by `agenticspec sim render` of the
  vendored fixture of `story:agent-architecture-run`, committed with the agenticspec version and the
  architecture digest, checked by digest in the console tests

## Acceptance

1. `ess ui check --model ess` reports no error on `account.ui.yaml`.
2. On fixtures, the architecture page of the `scheduled-loops` fixture shows the framed render with
   one element per part, and the `sweeper` and `triager` agent cards, in light and dark, with no
   axe violation and no horizontal overflow at 390 px.
3. The frame loads nothing from the network (a Playwright route that fails every request outside
   the console's origin is not hit).
