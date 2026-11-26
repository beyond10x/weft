---
format: aep.planning-md/3
id: design:tenant-clusters
kind: design
status: draft
title: 'Clusters as tenant resources: rename the installation to Cloud, account-owned clusters, workloads deploy into a cluster'
revision: 4
---
## Goal

The owner, 2026-11-26: "domain 'cluster' be an entity a user could create, destroy, etc, and there would
be a default one == the current design ... understood as 1st class resource a user could create and
then deploy different workloads into it". Decision C (same day): shared control plane first (A), a
dedicated control plane as a later stage (B).

## Before this design

- `weft.cluster.Cluster` was one weft installation: placement, access mode, control plane, with
  zones, node pools and nodes under it; only the `ClusterOperator` created it
  (`ess/1-infrastructure/cluster.yaml:67` before the rename).
- 12 domains referenced it (`tools/check-layers.sh --graph`), every one meaning the installation:
  access settings, identity trust domains, nodes, scheduling, resources and claims (zones).
- An account-owned cluster cannot live in layer 1: it would name `weft.accounts` (layer 2).

## Step 1, done 2026-11-26: the installation is a `Cloud`

- Domain `weft.cluster` renamed `weft.infrastructure`, file `ess/1-infrastructure/infrastructure.yaml`.
- `Cluster` -> `Cloud`, `ClusterId` -> `CloudId`, `cluster_id` -> `cloud_id`, `CreateCluster` ->
  `CreateCloud`, `ClusterPolicy` -> `CloudPolicy`, `access.ClusterAccess` -> `access.CloudAccess`.
  (`InstallationId` was taken: catalog product installations.)
- Zones, node pools, nodes and priority classes stay in `weft.infrastructure`, under a cloud.
- The actor `ClusterOperator` keeps its name (sketch § Actors): it runs the cloud.

## Step 2: `weft.cluster`, a tenant resource (layer 3)

- `Cluster`: name, owning account (absent for the default cluster), `control_plane: Shared |
  Dedicated` (`Dedicated` refused until stage B), minimum isolation level, zones it may use, state
  `Active`, `Draining`, `Deleted`.
- The default cluster: one per cloud, created by the `ClusterOperator` (`CreateDefaultCluster`),
  shared, runs on the cloud's shared node pools; every account may deploy into it. This is today's
  behaviour.
- `ClusterNodePool`: a dedicated pool of an account cluster (machine type or resource class, min,
  max, zone). Requesting one emits an event; a binding makes weft-control create the
  `infrastructure.NodePool` (edge form, sketch § 14), so tenants never call infrastructure commands.
- Commands: `CreateCluster`, `DeleteCluster` (refused while a formation runs in it; or drain first),
  `AddClusterNodePool`, `RemoveClusterNodePool`, `SetClusterPolicy`; views `ClustersOfAccount`,
  `Clusters`.
- Caller ties as in hardening pass 1: `account_id == caller.account_id`.

## Step 2b: topology of a cluster's nodes

The owner, 2026-11-26: "'topology' is also a concept in that domain space ... its defining the structure
of child nodes (of multiple kinds: ring, star, etc)"; "this could then optimized for certain traffic
patterns". Distinct from a formation's topology (components and links, sketch § "Topology as data").

- `Topology` (one per cluster, in `weft.cluster`): `kind` (`Star`, `Ring`, `Mesh`, `Tree`, `Line`,
  `Custom`), its child groups and the links between them.
- `TopologyGroup`: a child of the topology: a set of nodes from one or more of the cluster's node
  pools (a rack, a zone, a GPU island), with its role in the kind (the hub of a star, a position in a
  ring, a level and parent in a tree).
- `TopologyLink`: two groups, bandwidth, latency, whether traffic may cross it. Derived from the kind
  for `Star`/`Ring`/`Mesh`/`Tree`/`Line`; declared for `Custom`.
- `TrafficPattern` (a value on a formation or a link of a formation): `AllToAll`, `FanIn`
  (clients to one service), `FanOut`/`Broadcast`, `Pipeline` (stage to stage), `RingAllReduce`
  (ML training), `RequestResponse`.
- Scheduling reads both: it places a formation's components so their traffic pattern crosses the
  fewest and cheapest links (a pipeline along a line or ring, all-reduce on a ring, fan-in near the
  hub of a star). A placement that crosses a link traffic may not cross is refused.
- Simulation (design:simulation-and-kpis): experiments vary topology kind x traffic pattern; KPIs
  `link-crossings`, `hops-per-message`, `cross-link-bytes` per experiment, lower is better; the hill
  climb picks the topology and placement rules that lower them.

## Step 3: workloads deploy into a cluster

- `formations.Formation` gets `cluster_id` (required; the CLI fills the default cluster).
  `CreateFormation` refuses an unknown or deleted cluster, and an account cluster of another account.
- `scheduling`: a request carries the formation's cluster; `PlaceInstance` refuses a node whose pool
  is not the cluster's (the default cluster: the cloud's shared pools).
- Isolation: a cluster's minimum isolation raises a workload's minimum, never lowers it.

## Specified (2026-11-26)

- Step 1: `ess/1-infrastructure/infrastructure.yaml` (domain `weft.infrastructure`, entity `Cloud`).
- Step 2 and 2b: `ess/3-resources/cluster.yaml`: 5 entities, 13 commands, 6 views, `TrafficPattern`; 28 OPENs. Actors, component `weft-control`, bindings `cluster-create-node-pool`, `cluster-delete-node-pool` (`ess/components.yaml`).
- Step 3: `Formation.cluster_id`, `CreateFormation` refuses with `NoSuchCluster`; `SchedulingRequest.cluster_id` copied from the formation; `PlaceInstance` refuses another cluster's pool, a cluster's weaker isolation and zones outside the cluster (`NodeOutsideCluster`, `IsolationTooWeak`, `ZoneMismatch`). An account cluster with a dedicated pool runs only on its own pools.
- Not run by synthesis: it exceeds 30 min since hardening pass 1 (`docs/ess-gaps.md` gap 24).

## Stage B (later): dedicated control plane

`control_plane: Dedicated` makes weft-control create a `Cloud` for the cluster
(`infrastructure.CreateCloud` on placement `hosts` or `kubernetes`), managed from the first cloud.
Needs fleet management and cross-cloud identity. Decide after step 3 runs.

## Open questions (default taken until answered)

1. Does a cluster draw from the account's quota, or carry its own? Default: the account's quota; a
   dedicated pool's nodes count against it.
2. Are sub-accounts allowed to deploy into a parent account's cluster? Default: yes (D1 of hardening
   pass 1: the request acts in the cluster's account).
3. Metering of dedicated pools: node-hours to the owning account. Default: yes, metering stage after
   step 3.
4. Deleting a cluster with formations: refuse or drain? Default: refuse.
