---
title: Workloads
description: A weft workload is one component - its package, its shape and its claims - and runs as instances on nodes.
status: decided
---

A **workload** is one component of a formation. It is described on three independent axes:
**package** (what the bytes are), **shape** (how long it lives and how it starts) and **claims**
(which resources it holds). Domain: `weft.workloads`
([`ess/4-applications/workloads.yaml`](https://github.com/beyond10x/weft/blob/main/ess/4-applications/workloads.yaml)).

:::info[Specified, not built]
This page describes the specification. No running weft starts a workload yet.
:::

## Package

Plain kinds run as they are. Each package is an **artifact**: content-addressed, signed, and
verified by the node before it starts.

| Kind | What |
|---|---|
| `Snippet` | source text and a language |
| `Bundle` | a directory of code with an entry point and a dependency lock |
| `Wasm` | a WebAssembly module or component |
| `Image` | an OCI image by digest |
| `Process` | a command and arguments in a given root filesystem |
| `Machine` | a full VM image: kernel and root disk |
| `Model` | model weights and a serving runtime, for inference |
| `Browser` | a headless browser session driven by a protocol |
| `Build` | a recipe whose output is another artifact |

Spec kinds are translated into plain components before they run. The translation is cloudspec's:
an inbound translator turns the source into a cloudspec architecture, which weft runs; weft has
no translator of its own.

| Kind | Becomes |
|---|---|
| `EssSystem` | an API service with its state store, its commands and views, and its conformance suite as health check |
| `Agent` | an agent runtime instance with its tools, its model access and a sandbox for tool effects |
| `Graph` | a pipeline of other workloads |
| `Training` | later: a job on GPUs that writes a model artifact |

## Shape

| Shape | Lives |
|---|---|
| `Invocation` | one call, one result, a deadline |
| `Job` | runs to completion, retries, writes outputs; `parallelism` for N tasks, `gang` for all-or-none start |
| `Service` | long-running, N replicas, scales between bounds; may sleep at zero replicas |
| `Box` | one virtual machine the user owns: start, stop, suspend, exec, resize; its volume stays across stops |
| `Session` | interactive and attached: a terminal, a notebook, an agent sandbox |
| `Daemon` | one instance per matching node |
| `Schedule` | starts an invocation or job on a calendar rule |

## Instances

An **instance** is one running copy of a workload on one node. Its lifecycle:

| Transition | From | To |
|---|---|---|
| schedule | `Pending` | `Scheduled` |
| start | `Scheduled` | `Starting` |
| run | `Starting` | `Running` |
| pause, resume | `Running`, `Paused` | `Paused`, `Running` |
| exit | `Starting`, `Running`, `Paused` | `Exited` |
| withdraw | `Pending` | `Exited` |
| fail | `Scheduled`, `Starting`, `Running`, `Paused` | `Failed` |
| evict | `Scheduled`, `Starting`, `Running`, `Paused` | `Evicted` |

An instance runs one or more processes (`main`, `init`, `sidecar`) that share its network and
volumes. Health checks per process (`startup`, `ready`, `alive`) gate traffic and restarts; a
restart policy with backoff applies.

## Sleep and wake

A service with a minimum of zero replicas has a sleep policy: after an idle time its instances
stop, or are snapshotted and suspended. The first request wakes it: `weft-edge` holds the request,
an instance is scheduled, and the request is forwarded when the instance is ready. Past the wake
deadline the request fails with a named answer.

## Isolation

The scheduler places an instance at its minimum isolation level or stronger. Levels, strongest
first: `MicroVm`, `UserKernel`, `Container`, `ProcessSandbox`. Spec kinds and untrusted code
default to `MicroVm`. Instances of different tenants never share a kernel, and a GPU is wiped
between tenants.
