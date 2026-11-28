---
format: aep.planning-md/3
id: decision-blocker:slot-node-isolation-none-family
kind: decision-blocker
status: cleared
title: 'Nobody has decided which substrate family runs a workload of isolation None: gap 1''s confined workload or the host-process family'
relations:
- blocks: epic:slot-node-daemon
revision: 3
transitions:
- {from: "open", to: "cleared", at: "2026-11-28T13:41:13Z", actor: "human:operator", revision: 3, decided_on: {"recorded":{"approval":1}}}
---
# Nobody has decided which substrate family runs a workload of isolation None

## The relation

`weft.workloads.Workload` with `isolation: None` (and `run_as`) → the substrate family the node
runs its process through. Every process of slot (`slot`, `docs/design/cloudspec/slot.architecture.yaml`
on origin/main 97ef38e: `slot-daemon`, `node-daemon`, `sccache`, `toolchain-refresh`) is
`isolation: none`: the node daemon runs as user `build`, binds port 22 on all addresses, runs a
privileged prestart and the builds of the pool with the toolchains of the host.

- The epic names substrate gap 1, `story:host-driver-supervised-workload` (substrate, status
  draft): a workload confined "exactly as an exec" (isolation floor, no egress) from a root
  filesystem of gap 5 (`story:root-filesystem-from-a-digest-named-archive`, draft). It has no
  isolation None and cannot run the node daemon as written.
- substrate already serves unconfined supervised host processes: `epic:host-process-supervision`
  (ADR 0036), stories `host-process-start`, `-cgroup-tree`, `-lease`, `-stop-by-owner`,
  `-listing`, `-logs`, `-deadlines`, `-event-stream` implemented on substrate origin/main
  ffc2301. It restarts nothing, and weft decides restarts itself
  (`ess/5-runtime/node.yaml`, `weft.node.RestartProcess`).

No ess/1 document and no code of weft settles which family serves isolation None.

## Options

- **A (default)**: isolation None runs through the host-process family of substrate; gap 1 serves
  the confined levels only. `story:ports-node-workloads-local` picks the family by the
  isolation level of the spec. Nothing waits on gaps 1 and 5 for slot.
- **B**: everything runs through gap 1; substrate adds isolation None to its workload family
  (a change to `story:host-driver-supervised-workload`, asked of the substrate maintainers).

## What it stops

Epic `slot-node-daemon`, outcome 1 on a real node: which substrate operations the local adapter
of the `workloads` port calls for slot. `story:slot-node-process` is written against the port and
its stub, so it is not stopped.
