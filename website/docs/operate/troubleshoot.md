---
title: Operate and troubleshoot
description: What the gate and weft-sim tell you today, how to read a failure of each, and what operating a running weft will look like (Planned).
---

There is no running weft to operate yet. What you can operate today is the specification: the
gate and weft-sim. Operating a cloud is [planned](#operating-a-cloud-planned).

## The gate

`task check` is the gate, locally and in CI. It runs these steps in order and stops at the first
failure:

| Step | Runs | Fails when |
|---|---|---|
| `spec` | `ess specify validate --path ess --strict-requires` | the specification is not valid ESS |
| `layers` | `tools/check-layers.sh` | a domain refers to a domain of a higher layer |
| `stamped-time` | `tools/check-stamped-time.sh`, then `--negative` | a command with a `StampedAt` input is granted outside the control plane's actors, or the check stops refusing that |
| `ci-paths` | `tools/test-planning-only.sh` | the CI path filter classifies one of its test cases wrongly |
| `sim-test` | `cargo nextest run` in `tools/sim` | a test of weft-sim fails, a test runs over 60 s, or the run over 120 s |

In CI the workflow `Gate` runs the same `task check` on every pull request and on `main`. A pull
request that changes only planning or prose files passes without building.

### When a step fails

| Failing step | Look at |
|---|---|
| `spec` | the ESS code and the file and line `ess` prints; the [ESS documentation](https://beyond10x.github.io/ess/) explains each code |
| `layers` | the reference it names; move the entity or the field to the right layer ([layers](../specification/layers.md)) |
| `stamped-time` | the command and actor it names; a time the control plane decides on is granted only to `ControlPlane`, `Scheduler` or `ReleaseController` ([validate](../specification/validate.md#why-stamped-time)) |
| `ci-paths` | the case it prints as `FAIL: want … got …` |
| `sim-test` | the failing test's output; `task sim-test` alone reruns only this step |

## weft-sim findings

| What you see | What it means | Next |
|---|---|---|
| exit status 1, `violations by kind` | an invariant broke in a walk | run the `replay` command it prints ([replay](../simulator/run.md#replay)) |
| exit status 3, `setup steps not accepted as declared` | an experiment's setup step got another outcome than its `expect` | the step it names; the specification or the experiment changed |
| exit status 2 | a usage error or a specification that does not load | the message; validate with `ess` first |
| `command-blocked-by` in the gaps | a guard on stored rows refuses every reachable request | the guard at the line it names ([gaps](../simulator/results.md#gaps)) |
| `no-convergence` | after `--settle N` ticks the controllers still had work | the busy controllers it names |

## Operating a cloud (Planned)

:::caution[Planned]
Nothing in this section exists yet. It lists what the specification provides for operators, to be
built with the control host and the node agent.
:::

- **Nodes**: cordon, uncordon and drain a node; a node with lost heartbeats loses its session and
  its instances move when their leases end.
- **Logs**: stdout and stderr of each process and weft's own events about each instance
  (scheduled, started, check failed, restarted, evicted); `weft logs` reads a stream live.
- **Metrics**: per instance and node (CPU, memory, disk I/O, network, restarts, check results), at
  the edge and links (requests, errors, latency); alert rules that fire to a webhook, email or a
  queue.
- **Traces**: OTLP spans from instances through a node-local endpoint.
- **Health**: `startup`, `ready` and `alive` checks per process; past N restarts in a window an
  instance is `backing-off` and the event is named.
- **Isolation of telemetry**: an account reads only its own telemetry; the cluster operator reads
  node metrics and weft's own components, never tenant logs or spans.
