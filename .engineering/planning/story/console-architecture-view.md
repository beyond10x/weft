---
format: aep.planning-md/3
id: story:console-architecture-view
kind: story
status: draft
title: 'Operator console: the cloud''s architecture drawn in agenticspec''s style, live from views'
relations:
- decomposes: epic:consoles
- serves: vision:weft
revision: 1
---
## Epic

Decomposes `epic:consoles` (operator console). Builds on `story:console-ui`
(`product/console/ui/operator.ui.yaml`, actor `weft.actors.ClusterOperator`, 14 pages; weft's own
renderer with host slots in `product/console/src/hosts/`). Starts after `story:console-ui` is
merged: that tree is in active work.

## Context

- The owner, 2026-11-28 about 02:08 CET, typed in a session: "what we do need in the control plane
  is a nice rendering, similar to what agenticspec does for the architecture, for the cluster
  operator."
- The style to follow: agenticspec's architecture page (`agenticspec sim render`,
  `crates/agenticspec-render/src/lib.rs` and `graph.rs`): parts as cards in rows by layer, a colour
  per family with a legend, links as arrows labelled by what they carry, a border mark, light and
  dark, and the list of links as text after the graph for screen readers. weft draws its own cloud
  with its own host component in that style; agenticspec's renderer reads only
  `agenticspec-architecture/1` documents and is not used here.
- The static topology comes from the specification at build time: components and the domains each
  owns from `ess/components.yaml:78` (`weft-api`, `weft-control`, `weft-node`, `weft-edge`), planes
  from `docs/domain-sketch.md:61-78`, the channels from `protocols/node-channel.yaml` and
  `protocols/exec-channel.yaml`. Live state comes from views.

## What the page draws, and what each element reads

| element | drawn as | reads |
|---|---|---|
| cloud, placement | header facts | `weft.infrastructure.Clouds` (`placement`, `access_mode`, `control_plane.replicas`, `state`) |
| planes | rows: management, control, worker, data | build-time, `docs/domain-sketch.md:61-70` |
| `weft-api`, `weft-control` | cards in the management and control rows | build-time (owned domains, `ess/components.yaml`); desired replicas from `Clouds.control_plane` |
| scheduler queue on `weft-control` | badges | `weft.scheduling.PendingRequests`, `UnschedulableRequests`, `Preemptions`, `ScaleRequests` (counts) |
| zones | columns of the worker row | `weft.infrastructure.Zones` |
| node pools | groups in a zone | `weft.infrastructure.NodePools` (`zone`, `min_size`, `desired_size`, `max_size`, `state`) |
| nodes (`weft-node`) | cards in a pool or a zone | `weft.infrastructure.Nodes` (`zone`, `node_pool_id`, `role`, `capacity`, `observed_at`, `state`) |
| node load | a meter per node card | `weft.scheduling.PlacementsOnNode`, `weft.nodes.AssignmentsOfNode`, `weft.inventory.NodeDevices` (each per `node_id`) |
| drains | a mark on a node or pool | `weft.nodes.NodeDrains`, `weft.infrastructure.DrainingNodePools` |
| flow control <-> node (node channel) | an arrow per node, tone by session state | `weft.nodes.NodeSessions` (`state`: open, lost, superseded) |
| flow exec channel | arrow count | `weft.nodes.ExecSessions` |
| `weft-edge` | card in the data row | `weft.edge.Backends`, `weft.edge.RouteBackends`, `weft.edge.Certificates`, `weft.edge.HeldRequests` |
| flow edge -> instances | arrows to the nodes holding backends | `weft.edge.RouteBackends` joined to `weft.workloads.Instances` (`node_id`) |
| flow CLI and CI -> `weft-api` | a static arrow | none |

Every element opens the existing page of its kind (`nodes.detail`, `node_pools`, `zones`,
`scheduling`, `placements`).

## View gaps

1. No observed state of the server components: nothing reports whether `weft-api`, `weft-control`
   and `weft-edge` run, how many replicas run against `ControlPlaneSpec.replicas`, or their version.
   In the `local` placement all four run in one process (`docs/domain-sketch.md:80`).
2. No flow volume: no view gives requests per second into `weft-api`, messages on the node channel,
   or traffic through `weft-edge`; views count rows only. `weft.observability.NodeMetricSeries`
   indexes node series, not their values.
3. No cloud-wide load per node: `PlacementsOnNode`, `AssignmentsOfNode` and `NodeDevices` take one
   `node_id`, so the page makes one read per node; a view of used against `capacity` per node is
   missing.
4. No view of the placement's ports and adapters (`epic:platform-ports`): which adapter serves each
   port and whether it answers.
5. Grants: `story:console-ui` found that `ClusterOperator` reads none of the control-plane queue
   views (commit `e1d44e4`), for example `weft.nodes.SilentNodeSessions` and
   `weft.nodes.ExpiredLeases` (granted to ControlPlane) and `weft.scheduling.UnassignedPlacements`
   (granted to Scheduler); the page shows them once they are granted.
6. No view of the weft-api to weft-control flow: it is in-process or internal; drawn static.

Each gap is a row the page draws as "no data" with the gap's number, until a specification story
closes it.

## Scope

- `product/console/ui/operator.ui.yaml` (a page `architecture`, first in the menu),
  `product/console/ui/operator.hosts`, `product/console/ui/fixtures/operator/`
- `product/console/src/hosts/` (host component `CloudArchitecture`), its build-time input from
  `ess/components.yaml`

## Acceptance

1. `ess ui check --model ess` reports no error on `operator.ui.yaml`.
2. On fixtures with 2 zones, 2 node pools and 5 nodes (one with a lost session), the page draws the
   four components, every zone, pool and node, one node-channel arrow per node with the lost one in
   its warning tone, and a text list of every flow; light and dark, no axe violation, no horizontal
   overflow at 390 px.
3. Every element whose data has no view shows its gap number.
