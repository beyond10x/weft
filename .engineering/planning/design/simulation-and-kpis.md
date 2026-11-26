---
format: aep.planning-md/3
id: design:simulation-and-kpis
kind: design
status: draft
title: 'Simulation on the in-memory ESS runtime: experiments, fault injection, system invariants, KPIs (ratchet), state-space hill climbing'
revision: 3
---
## Goal

The owner, 2026-11-26: define KPIs (ratchet) to evaluate the cluster; run simulations as experiments
(topologies, fault injection); connected parts of the infrastructure hold invariants such as
idempotent behaviour; the simulations run on an in-memory ESS runtime usable from a build and
callable through the `ess` CLI, many explorations at once; measure the state-space size and reduce it
by adding conditions and invariants where things branch (hill climbing).

## What exists (checked 2026-11-26)

- In-memory runtime: `Interpreted::for_model` in the `ess-conformance` crate
  (`crates/verify/ess-conformance/src/interpret.rs:115`, beyond10x/ess). It executes commands
  (outcome selection, transitions, `sets:`, events and payloads, `invariants:` at rest), projects
  views, dispatches bindings, and observes explicit delivery facts and relative time. It reads no
  store: a run is in memory and deterministic. What it cannot derive answers `Unsupported`, never a
  guess.
- Reached from the CLI only through a suite: `ess verify conform run --target interpreted`. No CLI
  sends a free command sequence to it; `ess verify conform serve` serves only `billing` and
  `oracle-fixture`.
- Bounded exploration exists for protocol models only: `ess verify protocol explore` (bounded
  schedules, finite typed input witnesses), `crates/verify/ess-conformance/src/protocol/`.
- `ess verify conform check-history`: linearizability of a recorded concurrent history.
- `ess verify conform mutate`: specification mutants replayed against a reference target.
- `faulty.rs`: implementations wrong in one way, to audit a suite. It injects implementation faults,
  not infrastructure faults.
- Ratchet (`~/beyond10x/ratchet`): format `ratchet/1` (requirements bound to an assessor, a condition
  and a gate; pass, fail or insufficient evidence; a reference that only tightens). Presets include
  `conformance-cases@1`, `conformance-report@1`, `timing-report@1`, `case-list@1`. The `ratchet`
  binary answers "unavailable" to its 16 commands (wave 2 delivered 2026-11-25); `ratchet validate`
  waits on an ESS release (ROADMAP.md, wave 4).
- Prototype running: a random walk over the interpreted model that reports reached states,
  transitions and outcomes as one HTML page (scratch `explorer/`, 2026-11-26).

## Answer to "is that possible?"

Yes, with one ESS epic. The runtime is there and is in memory; what is missing is a way to drive it:

1. A library entry point to open a model, send a command with inputs, read the outcome, the rows and
   the events, and fork the state (for branching exploration). Today `Interpreted` is driven
   through `ConformanceTarget` with scenario steps.
2. A CLI over it: `ess verify explore --path <tree> --experiment <file> --seed <n> --steps <n>
   --out <dir>`, writing a machine-readable result (states, transitions, outcomes, invariant
   violations, the walk that reached each) and exiting non-zero on a violation.
3. Fault injection at the delivery seam the interpreter already observes: drop, duplicate, delay and
   reorder a bound event; a binding's `on_failure` (retry, drop, escalate) then runs as declared.
4. System invariants over many rows and domains, checked after every step (idempotency, conservation,
   isolation, convergence below), beside the entity `invariants:` that exist.
5. State abstraction and counting: a state is the tuple of each row's lifecycle state plus the
   fields the guards read; the run counts distinct abstract states and the branch points (a step
   where more than one outcome of the same command is reachable).

Runs are independent and deterministic per seed, so many run at once (one process per seed, or
threads in one process). Each run must stay small: the host's slot pool kills a job at 10 min.

## Experiments

An experiment file (one per directory `sim/experiments/`):

- topology: clouds, zones, node pools and nodes with capacity and isolation facts; tenant clusters
  and their dedicated pools (design:tenant-clusters);
- tenants: tenants, accounts, quotas, principals;
- workload mix: formations, workload shapes (Service, Job, Box, Invocation), replica counts,
  claims;
- fault schedule: node gone, node partitioned (stale heartbeat), event dropped, duplicated,
  reordered, delayed; binding failure; clock steps; a quota lowered under load;
- seed and step budget.

## Invariants of connected parts (checked after every step)

- Idempotency: a duplicated event changes nothing a second time (no second row, no second
  transition).
- Convergence: after the fault schedule ends, within N steps, desired equals observed for every
  service (replicas, placement), drift is repaired or escalated.
- No double placement: an instance is assigned to one node at a time; a claim bound once.
- Conservation: quota drawn <= quota allocated, at every account level; metering usage >= 0.
- Isolation: instances of different tenants never share a kernel below `microvm`; a secret is
  delivered only within its scope; a tenant cluster's dedicated pool runs only that account.
- Monotonicity: revisions, release steps and versions only move forward.

## KPIs (ratchet/1, file `quality/ratchet.yaml`)

Specification health (lower is better unless marked):

- `synthesis-refusals` per domain (ESS-SYNTH-001/003/004).
- `failed-scenarios` on the interpreted target: 0, a gate.
- `outcomes-unreached` (synthesis and explorer together).
- `invariant-violations` found by the explorer: 0, a gate.
- `abstract-states` per experiment: the state-space size the hill climb reduces.
- `branch-points-without-guard`: steps where a command's outcome depends on something no guard or
  invariant names.
- `outcomes-reached` (higher is better): the floor that keeps a reduction from removing behaviour.
- `synthesis-seconds` per domain: at most 120 s (test budget, `~/beyond10x/AGENTS.md`).

Cluster behaviour, per experiment:

- `steps-to-converge` after a fault; `instances-lost`; `unschedulable-requests`.
- `cross-tenant-reach`: 0, a gate.
- `duplicate-effects` under duplicated delivery: 0, a gate.
- `node-hours` per workload (metering), as a cost figure.

## Use case: virtual full setup, traffic, mutation points, search

The owner, 2026-11-26: "use the specs to construct a virtual full setup, simulate traffic, mark partial
mutation points <- fit function apply or full space exploration for gap finding or pattern matches".

1. Virtual setup: an experiment's `setup:` is turned into commands and run on the in-memory model: a
   cloud, zones, node pools and nodes, tenant clusters with their topology (design:tenant-clusters
   step 2b), tenants, accounts, quotas, formations, workloads, claims, secrets. The result is one
   complete weft instance as rows; no machine, no network.
2. Traffic: `traffic:` gives per formation link a `TrafficPattern` and a rate. Each tick the simulator
   turns traffic into what the model can see: invocations, autoscale inputs (`AutoscaleService`),
   usage records (metering), node observations (load), and counts the links each message crosses in
   the cluster topology.
3. Mutation points: `mutations:` names places that may vary, each with its values:
   - a setup value (topology kind, replica count, pool size, zone count, isolation level);
   - a traffic value (pattern, rate);
   - a fault (which node is lost, when; which event is duplicated or dropped);
   - a specification variant (a guard present or absent, a threshold): applied to a copy of the
     spec, the way `ess verify conform mutate` makes specification mutants.
4. Search over the mutation points, `search.mode`:
   - `fit`: a fitness function over the KPIs (weights, gates as hard constraints); hill climbing or
     a small evolutionary loop picks the next point from the best so far;
   - `exhaustive`: every combination within the bound; the product size is printed before the run
     and refused over a limit;
   - both write every point visited with its KPIs, so the result is a table and a front.
5. Goals of a run, `search.find`:
   - gaps: invariant violations, outcomes or states not reached, branch points no guard decides;
   - pattern matches: a state pattern over rows (for example "two tenants' instances on one node
     below `microvm`", "a release Live with a gate not Passed", "a quota drawn above its
     allocation"); a match stops with the walk that reached it, replayable by seed.
6. Live UI: the served page shows the search: points visited, the fitness of each, the best so far,
   matches found with their walk.

## Hill climbing

1. Run the experiment set (seeds x experiments) in parallel; collect `abstract-states`,
   `branch-points-without-guard`, `outcomes-reached`, violations.
2. Pick the branch point with the largest fan-out that no guard or invariant decides.
3. Add the condition or invariant that decides it (in the domain file that owns the command).
4. Re-run. Accept when `abstract-states` drops, `outcomes-reached` does not drop, violations stay 0
   and conformance stays green. Ratchet tightens the reference.
5. Repeat. A step that only shrinks states by making declared outcomes unreachable is rejected by the
   floor KPI.

## Stages

1. Prototype explorer on the interpreted model (running), HTML gap page. Measure steps per second.
2. ESS epic (beyond10x/ess): library driving API, `ess verify explore`, delivery faults, system
   invariants, state counting. Story drafts filed in the ESS planning store after the prototype
   reports what is missing.
3. weft: `sim/experiments/`, `quality/ratchet.yaml`, a `sim` task that runs the set in parallel
   outside slot (it measures), each `ess` call through slot.
4. Hill-climbing loop as an agent procedure in `AGENTS.md`.

## Open questions (default taken until answered)

1. Abstract state: lifecycle states only, or also the guard-read fields? Default: both, with numbers
   bucketed (0, 1, many).
2. Where do system invariants live: an ESS construct (`system_invariants:`), or weft-side checks over
   the explorer's row dump? Default: weft-side first (no ESS change needed), propose the construct
   with the evidence.
3. Ratchet before its 0.1.0: write `quality/ratchet.yaml` now in `ratchet/1` and check it when
   `ratchet validate` ships. Default: yes.
