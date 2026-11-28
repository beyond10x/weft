---
format: aep.planning-md/3
id: decision-blocker:slot-node-per-node-instance-lifecycle
kind: decision-blocker
status: cleared
title: Nobody has decided what happens to a per-node instance when its node leaves the pool
relations:
- blocks: epic:slot-node-daemon
revision: 4
transitions:
- {from: "open", to: "cleared", at: "2026-11-28T13:41:14Z", actor: "human:operator", revision: 4, decided_on: {"recorded":{"approval":1}}}
---
# Nobody has decided what happens to a per-node instance when its node leaves the pool

## The relation

`weft.workloads.Instance` of a workload in a scope placed on a pool (one instance per node), and
of a workload with `per: <scope>` (one instance per instance of that scope) → `weft.infrastructure.Node`.
Cardinality one instance per (workload, node): inferable (inferred from cloudspec
`59129eb:crates/cloudspec/src/generate/systemd.rs:25-26`, "one instance per instance of that
scope", and the comment "a Daemon runs one instance per node" in
cloudspec `59129eb:blocks/5-workloads/workloads.yaml:4513-4514`; no relations: entry declares it).

Lifecycle coupling is not settled: the systemd generator writes "which instances run is not
written: whoever installs the unit starts or enables one per instance" (`systemd.rs:968`), and
the workloads block leaves Daemon counts OPEN (`blocks/5-workloads/workloads.yaml:2886` at 59129eb). The repair step of weft
(`product/cloud/crates/weft-controllers/src/repair.rs`) places an instance of a lost node again,
which for a per-node instance would put a second one on a node that has one.

## Options

- **A (default)**: the instance belongs to its node. It is stopped when the node leaves `Active`
  (drain, lease end, gone) and is never placed on another node; a node that joins gets its own.
- **B**: it is placed again like a replica, and the one-per-node rule is checked at placement.

## What it stops

Epic `slot-node-daemon`, outcome 2 for a node that leaves: the drain and lease end slot needs
(`slot` design § 4 A). `story:slot-node-pool-placement` covers nodes that join and run, not one
that leaves.
