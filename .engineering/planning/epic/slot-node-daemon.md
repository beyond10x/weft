---
format: aep.planning-md/3
id: epic:slot-node-daemon
kind: epic
status: draft
title: weft runs slot's node daemon from slot's cloudspec architecture on a build node of a pool
relations:
- serves: vision:weft
- depends_on: epic:cloudspec-submission
revision: 1
---
## Why

A consumer, slot, has an exact acceptance test: `cloudspec generate weft --from
docs/design/cloudspec/slot.architecture.yaml --out x.json` (slot's architecture, cloudspec main
59129eb or later) exits 0, and weft then runs slot's node daemon on a build node of a pool. On
2026-11-28 the command exits 1 with 44 refusals under 17 rules of cloudspec's weft generator
(`crates/cloudspec/src/generate/weft/runs.rs`): network 10, deployer path 8, process 4, host path
4, stop 2, socket 2, process environment 2, placement 2, data link 2, and one each for restart,
priority, prestart, per-instance, parity, limits, instance and environment files. The same file
generates systemd units equal to slot's own, so the architecture is complete; the refusals name
what weft does not run yet.

## Outcome

1. weft runs a long-lived supervised process (workload kind beyond `snippet`) on a node, through
   the substrate host driver (substrate gap 1), with its environment, prestart, stop and restart.
2. A workload is placed on a node of a named pool (placement), one instance per node where asked.
3. Host paths, a read-write volume kept across runs (substrate gap 6), a root filesystem from a
   digest-named archive (gap 5), sockets and the network an architecture names.
4. For each weft feature, cloudspec's generator maps the matching rule; the acceptance command
   exits 0 and weft runs slot's node daemon on a build node of a pool.

## Order

Process and placement first (substrate gaps 1 and 5), then host paths and data (gap 6), then
network and sockets, then the remaining single rules.

## Out of scope

Changing slot's architecture file; cloudspec's generator code (cloudspec maps a rule once weft
runs it).
