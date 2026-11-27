---
title: Run weft-sim
description: Explore weft's specification with random command sequences, run experiments with faults and controllers, replay one walk, and describe commands.
---

weft-sim has four commands. Run them from the repository root; `--spec` defaults to `ess`.

| Command | Does |
|---|---|
| `explore` | Random command sequences (walks) over the whole specification, with system invariants checked as it goes. |
| `simulate` | One experiment file: a setup, traffic, faults and controllers, then checks. |
| `replay` | One recorded walk again, step by step, with the invariants checked after every step. |
| `describe` | The commands, inputs, outcomes, actors and bindings the specification declares. |

Each of `explore` and `simulate` writes `result.json` and `index.html` to its `--out` directory
([read the results](results.md)).

## Explore

```shell-session
$ weft-sim explore --spec ess --seed 1 --walks 2048 --steps 300 --threads 8 --out sim/out/explore
```

A walk starts from an empty state and sends up to `--steps` commands. Commands are weighted toward
outcomes not reached yet, and inputs are drawn to name rows the walk already holds. The same
`--seed`, `--walks` and `--steps` give the same walks on any number of threads.

| Option | Default | Meaning |
|---|---|---|
| `--seed N` | 1 | seed of the random draws |
| `--walks W` | 64 | number of walks |
| `--steps S` | 60 | commands per walk at most |
| `--threads T` | 4 | worker threads, 1 to 8 |
| `--max-seconds N` | 280 | start no new batch of walks after N seconds |
| `--from EXPERIMENT` | none | every walk starts from the experiment's setup |
| `--settle N` | 0 | with `--from`: after the setup and after each walk, up to N ticks of the experiment's controllers; a walk whose controllers still work is a `no-convergence` violation |
| `--controller-share P` | 0 | with `--from`: a share P (0 to 1) of the steps is a tick of the controllers |
| `--retry-bound N` | 16 | a binding the specification retries without bound stops after N attempts; 0 keeps the specification as written |
| `--invariants LIST` | all | comma-separated, or `none` |
| `--check-every N` | 1 | check the invariants after every N-th step; 0 never |
| `--patterns FILE` | none | patterns over rows to look for |
| `--serve [ADDR]` | off | serve a live page, default `127.0.0.1:7480`, loopback only |
| `--exit-when-done` | off | with `--serve`: exit when the run ends instead of serving until Ctrl-C |

The invariants:

| Invariant | Holds when |
|---|---|
| `closed-link` | no two active placements of one formation sit in topology groups joined by a link that may not be crossed |
| `idempotency` | a command that an at-least-once binding invokes, sent a second time, changes nothing |
| `no-double-placement` | no instance is active on two nodes |
| `quota-conservation` | a quota's remaining amount stays between 0 and its limit, and its sub-accounts hold no more than its limit |
| `tenant-isolation` | no node runs instances below `MicroVm` of two tenants, which would share its kernel |

The Taskfile has three ready-made runs:

| Task | Runs |
|---|---|
| `task sim-explore` | 2048 walks of 300 steps from an empty state |
| `task sim-explore-from` | 1024 walks from `sim/experiments/full-setup.yaml`, settled by its controllers for up to 12 ticks |
| `task sim-explore-ticks` | 32 walks as above, 1 step in 10 a controller tick |

Pass more options after `--`, for example `task sim-explore -- --serve`.

## Simulate

```shell-session
$ weft-sim simulate --spec ess --experiment sim/experiments/node-loss.yaml --out sim/out/node-loss
weft-sim: exhaustive search over 2 mutation points: 9 combinations (limit 64)
weft-sim simulate node-loss: 222 steps in 0.1 s; checks 87; violations 0
weft-sim: wrote sim/out/node-loss/result.json and sim/out/node-loss/index.html
```

An experiment (`format: weft-sim-experiment/1`, YAML) declares:

| Key | What |
|---|---|
| `setup` | commands in order, each with its actor, input and the outcome it must answer (`expect`); `bind` keeps a value of the answer as a variable, `${name}` reads it |
| `traffic` | requests sent while the experiment runs |
| `faults` | what goes wrong at which tick: `node_loss` (nodes go silent), `duplicate` (a step sent again), `drop` (a controller skipped for some ticks) |
| `controllers` | the reconciliation loops, driven as weft-control would, once a tick |
| `converge_within` | ticks after the last fault in which the controllers must converge (default 6) |
| `invariants` | the checks to run |
| `mutations`, `search` | points to vary and how to search them (`--no-search` turns the search off) |
| `lost_bindings` | bindings whose every delivery is lost, so only a controller repairs their gap |

The experiments in [`sim/experiments/`](https://github.com/beyond10x/weft/tree/main/sim/experiments):

| Experiment | Checks |
|---|---|
| `full-setup.yaml` | a cloud with two zones, two tenants, formations placed, assigned and running; a node goes silent |
| `node-loss.yaml` | two nodes, one goes silent: its session is lost, leases end, the gap views empty within the bound |
| `duplicate-delivery.yaml` | a delivered command arrives twice: no second row, no second transition |
| `stuck-placement.yaml` | every assignment delivery lost: placements reach nodes through the level step, a silent node's placements are released and placed again |
| `closed-link.yaml` | placement across topology groups refuses peers across a closed link |
| `cluster-pool-lifecycle.yaml` | a tenant cluster's dedicated pools created, drained and deleted by the control plane's steps |

`task sim-experiments` runs all of them.

## Replay

A result keeps the trace of up to 64 walks, and of each walk that broke an invariant. Replay one:

```shell-session
$ weft-sim replay --spec ess --trace sim/out/explore/result.json --walk 0
```

It prints each step with the outcome it got, then a summary such as
`walk 0 seed 13898507668257350876: 0 answers differ, 0 system invariant violations`. When a step
starts to break an invariant, replay names it. A walk of `explore --from` runs the experiment's
setup again first.

When explore finds a violation, it prints the replay command for the first walk of each kind.

### Replay a search point

A `simulate` run with a search keeps no trace of its points, but each row of `search` names the
walk its point ran as. Replay reruns that point by its walk number, from the same directory as
the run:

```shell-session
$ weft-sim replay --spec ess --trace sim/out/simulate/result.json --walk 14
```

It loads the experiment the result names (`run.experiment`), applies the row's point, and runs
it once at walk 14 with its timeline recorded. It prints each KPI of the rerun beside the recorded
one, such as `steps: 6 (recorded 6)`, and a summary such as `walk 14: 0 of 20 KPIs differ`. It
exits 0 when every KPI is equal and 2 when one differs. It writes `result.json` and `index.html`
to `--out` (default `sim/out/replay-14`); the page's Experiment timeline shows the setup, traffic,
faults and controller ticks of that point. A walk number that no search row and no trace of the
result holds is refused with exit status 2.

On the page of the run, the player shows the walk of the selected point and this command: the
point of the current frame, or the one whose cell or path mark you click.

## Describe

```shell-session
$ weft-sim describe --spec ess --prefix weft.nodes.
```

lists each command under the prefix with the actors that may run it, its inputs, its response
fields and its outcomes; with no prefix, or `--prefix bindings`, also every binding.

## Exit status

| Status | Meaning |
|---|---|
| 0 | the run ended with no violation; for `replay`, every answer (or every KPI of a search point) as recorded |
| 1 | an invariant was violated |
| 2 | a usage error or a specification that does not load; for `replay`, an answer or a KPI differs from the recording, or no trace and no search row has the walk |
| 3 | a setup step was not accepted as declared (its `expect`, or an accepting outcome without one); with `--from` this stops the run before any walk |
