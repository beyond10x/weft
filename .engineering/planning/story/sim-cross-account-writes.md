---
format: aep.planning-md/3
id: story:sim-cross-account-writes
kind: story
status: draft
title: weft-sim finds a command that writes another account's row
relations:
- decomposes: epic:kpi-ratchet
- serves: vision:weft
scope:
- confidence: cited
  path: quality/ratchet.yaml
- confidence: cited
  path: tools/sim/src/explore.rs
- confidence: cited
  path: tools/sim/src/simulate.rs
- confidence: cited
  path: tools/sim/tests/main.rs
revision: 2
---
## Context

`story:skeleton-workload-definitions` found that `weft.formations.AddComponent` and
`weft.formations.CommitRevision` let a Deployer of one account write to another account's
formation by its id (fixed in weft#21; `docs/hardening/pass-2.md:900` had listed both as left
untied). weft-sim did not find it:

- the KPI `cross_tenant_reach` is fed only by the `tenant-isolation` check
  (`tools/sim/src/simulate.rs:383-418`), which reads the assignment and placement views for two
  tenants sharing a kernel on one node; it never asks whether a command's caller account owns
  the row the command wrote;
- `caller_for` (`tools/sim/src/explore.rs:106`) copies the caller's `account_id` from the
  input's `account_id`, so exploration seldom sends a caller of another account.

The sweep in weft#21 lists the commands still without a caller check (formations: AddToBorder,
AddLink, AddEndpoint, AddRoute, CreateConfig, MountConfig, SetConfigOverride; workloads:
StopInstance, WakeService, Invoke; five blocked by ESS-COMMAND-009) and one possible gap,
AddTopologyGroup's `parent_group_id` (`ess/1-infrastructure/cluster.yaml`).

## Scope

- `tools/sim/src/simulate.rs`, `tools/sim/src/explore.rs`
- `tools/sim/tests/main.rs`
- `quality/ratchet.yaml` (the new KPI's merge requirement)

## Acceptance

1. weft-sim has an invariant that fails when an accepted command of a tenant actor writes or
   moves a row owned by an account other than the caller's; its count is a KPI held by
   `quality/ratchet.yaml`.
2. Exploration sends commands as a caller of another account than the one the input names, on
   every command granted to a tenant actor.
3. Against the specification of main before weft#21, a run finds AddComponent's cross-account
   write (shown with one red run on a fixture copy).
4. On main, the run names each remaining command of the sweep above, or the story says why one is
   not reached.

## Rules for this work

- Builds and tests run through `slot`; a pool job ends within 10 minutes unless it names its
  time (at most 30 minutes). No test over 60 s; the test step within 120 s.
- The specification is synthesized once per run, not per test.
- No Python in anything committed.
