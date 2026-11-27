---
title: Read the results
description: What weft-sim writes - the summary line, result.json and the index.html page - and how to turn a gap into a line of the specification.
---

Each `explore` and `simulate` run writes two files to its `--out` directory:

- `result.json`: every number of the run, the gaps, the violations and the traces of up to 64
  walks;
- `index.html`: one static page with the same data inlined. Open it in a browser; it needs no
  server.

With `--serve`, the same page is served live while the run goes on.

## The summary line

```text
weft-sim explore: 64 walks, 3865 steps in 0.3 s (12836 steps/s, 212.5 walks/s, 4 threads); outcomes 664/2525, states 76/328, transitions 37/221, abstract states 1145, branch points 26, violations 0; commands never accepted 339 (blocked-by 2)
```

| Field | Meaning |
|---|---|
| outcomes 664/2525 | outcomes some step answered, of all outcomes declared |
| states, transitions | lifecycle states held and transitions taken, of all declared |
| abstract states | distinct model states seen, counting per entity, state and the fields guards read whether there is one row or many |
| branch points | a command at an abstract state where it answered two or more different outcomes |
| violations | invariant violations; any is exit status 1 |
| commands never accepted | commands that never answered an accepting outcome |
| blocked-by | of those, commands a guard on stored rows refused every time (below) |

A short run reaches a small part of the model. `task sim-explore-from` starts every walk from a
full setup and reaches far more.

## result.json

The top-level keys used most:

| Key | Holds |
|---|---|
| `walks`, `steps`, `elapsed_s` | size and time of the run |
| `outcomes_reached`, `outcomes_declared` | the outcome count of the summary |
| `violation_count`, `violations`, `violations_by_kind` | each violation with its walk, seed, step and the last command |
| `gaps` | what was not reached, each with the file and line of its declaration |
| `commands` | per command, how often it was accepted and which outcomes it reached |
| `errors` | interpreter answers that are not an outcome, such as `unsupported` and `undeclared` |
| `traces` | the recorded walks that `replay` reads |
| `run` | the command line, the ESS revision, the seed and the options |

For `simulate`, `run.result` holds the experiment's checks and KPIs.

### Search rows

A `simulate` run whose experiment has a `search` block writes one row to `search` per point it
evaluated, in evaluation order. The live snapshot of `--serve` holds the same rows as they come.

| Field | Meaning |
|---|---|
| `point` | the value of each mutation, by name |
| `kpis`, `fitness`, `feasible` | the point's KPIs, its weighted fitness, and whether every gate KPI is 0 |
| `error` | only when the point's run failed: why |
| `seq` | the row's position, from 1 |
| `round` | the evaluation batch the point was part of. `fit`: the start is round 1, then each round of neighbours and each restart is a round of its own. `exhaustive`: every point is round 1 |
| `index` | per mutation axis, in the order of `info.search.axes`, the index of the point's value in that axis's `values` |
| `parent` | the `seq` of the point this one is a neighbour of; `null` for the start, a restart and every `exhaustive` point |
| `decision` | `fit`: `start` for the first point, `accepted` for the neighbour that became the current point of its round, `rejected` for every other neighbour, `restart` for a restart. `exhaustive`: `start` for the first point, then `accepted` when the point raised the best fitness so far and `rejected` otherwise |
| `best` | the best fitness of the feasible rows up to and including this one; `null` while none is feasible |
| `walk` | the walk number the point was simulated with, unique within the run. Walk 0 is the best point, run again with its timeline recorded |

A `fit` search climbs from the point with index 0 on every axis to the best feasible neighbour
that improves on the current point. After a round of neighbours with no improvement, or at a
point with no neighbour left to visit, it restarts at a random point not visited yet. The
restart is evaluated in a round of its own, counts against `budget` and becomes the current
point. The search ends when the budget is spent or every point is visited.

`info.search` describes the search. It is written before the first point is evaluated, so the
live page has it from the start. The variable `WEFT_SIM_SEARCH_HOLD` is for tests only: naming a
file, it makes the search wait after writing `info.search` until that file exists (at most 30 s);
empty or unset, it does nothing.

| Field | Meaning |
|---|---|
| `mode` | `fit` or `exhaustive` |
| `seed` | the experiment's seed; it also draws the restarts |
| `budget` | the most points a `fit` search evaluates, at least 1; the experiment is refused at load with a `fit` budget of 0 |
| `limit` | the most combinations an `exhaustive` search accepts |
| `weights` | the weight of each KPI in the fitness, each a finite number; the experiment is refused at load with a weight that is not |
| `gates` | the KPIs that must be 0 for a point to be feasible |
| `axes` | each mutation's `name`, `kind`, `point` and its `values` in order |

## Gaps

`gaps.top` lists the most useful gaps first. Each names the place in the specification:

```json
{"at": "ess/5-runtime/identity.yaml:635", "kind": "command-blocked-by",
 "what": "weft.identity.FinishKeyRotation is always refused `wrong-state` (12 requests naming live rows)"}
```

| Kind | Means |
|---|---|
| `violation` | an invariant broke; replay the walk |
| `unsupported` | the interpreter could not evaluate a guard |
| `unbounded-retry` | a binding retried without bound |
| `undeclared-refusal` | a request got a refusal no declared outcome covers |
| `command-blocked-by` | every request naming live rows got the same refusal from a guard that reads stored rows: the guard holds on every reachable row, an inferred gap of the specification |
| `entity-never-held` | no walk created a row of the entity |
| `command-never-accepted` | the command never answered an accepting outcome |

A command never accepted for other reasons is `rare`: too few requests named live rows, it was
never sent, or the refusal depends on the input only. Its gap is the drawing or an upstream
command, not its own guard.

The full lists are in `gaps.commands_never_accepted`, `gaps.entities_never_held`,
`gaps.unreached_outcomes`, `gaps.unreached_states` and `gaps.unreached_transitions`.

## The page

`index.html` has these sections:

| Section | Shows |
|---|---|
| Growth | distinct abstract states and outcomes reached, over steps |
| Live feed | the last steps, while the run goes on |
| Experiment timeline | for `simulate`: setup, traffic, faults and controller ticks |
| Search, Pattern matches | the mutation search and the patterns found |
| Violations | each violation and the walk to replay |
| Top gaps | `gaps.top`, each with the file and line of the specification |
| Commands never accepted | with their refusals and class |
| Domains | lifecycles and outcomes per domain, reached or not |
| Branch points | where commands fanned out, largest first |
| Interpreter answers that are not an outcome | `unsupported` and `undeclared` answers |
| Notes | notes the run recorded |

## From a gap to a fix

1. Open the file and line the gap names.
2. For a violation, replay the walk ([run weft-sim](run.md#replay)) to see the step where it
   starts.
3. Change the specification, validate it, and run the same seed again: the gap is gone or has
   moved.

The hardening passes in
[`docs/hardening/`](https://github.com/beyond10x/weft/tree/main/docs/hardening) record each finding
of this kind and what changed.
