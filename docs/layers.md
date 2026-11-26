# weft: layers derived from the references between domains (2026-11-26)

Method: all 14 domain files validated together as one system (`weft v1 — 15 file(s), valid`,
0.24 s, `workloads.yaml` still being written). ESS reports every stored identifier that names an
entity of another domain without a declared relation (`ESS-ENTITY-019`, 133 warnings). Each warning
is one edge `entity -> entity`. Grouped by domain:

```
access      -> accounts (6)
accounts    -> formations (1), resources (1)
catalog     -> formations (4), accounts (3), deployments (1), resources (1)
cluster     -> (none)
deployments -> formations (10), accounts (4), workloads (3), cluster (1)
formations  -> accounts (1), deployments (1), workloads (1)
identity    -> formations (4), accounts (3), cluster (1), workloads (1)
metering    -> accounts (10), formations (7), catalog (4), resources (2), cluster (1), workloads (1)
nodes       -> cluster (7), workloads (5), resources (2), accounts (1), scheduling (1)
resources   -> cluster (7), accounts (5)
scheduling  -> accounts (5), cluster (5), workloads (3), formations (1), resources (1)
secrets     -> formations (2), accounts (1), workloads (1)
workloads   -> accounts (7), formations (4), cluster (1), resources (1), scheduling (1)
```

## Edges against the direction (cycles)

Each closes a cycle. Each has a fix that keeps the meaning:

1. `accounts.Export.endpoint_id -> formations.Endpoint`. Fix: Export moves to formations (an
   account exports one of its endpoints).
2. `accounts.Quota.resource_class -> resources.ResourceClass`. Not a cycle once `resources` splits
   (below): ResourceClass sits in infrastructure, under tenancy.
3. `formations.FormationRevision.translation_id -> deployments.Translation`. Fix: drop it;
   `Translation` already names the revision it produced.
4. `formations.FormationRevision.document_digest -> workloads.Artifact`. Fix: Artifact (a
   content-addressed blob) moves to a low `artifacts` domain.
5. `workloads.Instance.placement_decision_id -> scheduling.PlacementDecision`. Fix: drop it;
   `PlacementDecision` names the instance.

## Splits the graph shows

- `resources`: `ResourceClass` and `Device` name only `cluster` (operator-owned, no tenant).
  `Volume`, `Filesystem`, `Bucket`, `VolumeSnapshot`, `Claim` name an account. Cut: inventory
  (infrastructure) and tenant storage and claims (above tenancy).
- `workloads`: `Workload` names a formation; `Instance` names a node and a placement. Cut, to check
  once the file is done: definitions (application layer) and instances (runtime layer).

## Layers (after the fixes)

Each layer names only layers below it.

0. foundation: `shared` (identifiers, value types)
1. infrastructure: `infrastructure` (clouds, zones, pools, nodes; `cluster` until 2026-11-26), `inventory` (resource classes, devices)
2. tenancy: `accounts`, `access`
3. tenant resources: `resources` (volumes, filesystems, buckets, snapshots), `artifacts` (an artifact is owned by an account), `cluster` (tenant clusters, design:tenant-clusters)
4. applications: `formations`, workload definitions
5. runtime: `claims` (claims, bindings), `scheduling`, `nodes`, `identity`, `secrets`, `observability`
6. delivery: `deployments`
7. services: `catalog` (managed services, products)
8. economy: `metering`
9. actors: `actors` (every actor; names commands of all layers)

## Applied (2026-11-26)

- Export moved to formations; FormationRevision.translation_id and Instance.placement_decision_id dropped.
- ResourceClass and Device moved to `1-infrastructure/inventory.yaml`; Artifact to `3-resources/artifacts.yaml`.
- `tools/check-layers.sh` (`task layers`) holds the rule: 0 upward references.
- workloads stays whole in layer 4: once Instance.placement_decision_id was dropped, no reference of it points upward.
- Claim and Binding moved from `resources` (layer 3) to `5-runtime/claims.yaml`: a binding names a node and an instance. `Volume.writer_binding_id` dropped; DeleteVolume, DeleteFilesystem, DeleteBucket refuse `UnitInUse` as `external` (layer 3 cannot read claims).
