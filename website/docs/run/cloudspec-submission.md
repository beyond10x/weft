---
title: Submit a cloudspec architecture
sidebar_label: Submit a cloudspec architecture
description: How a cloudspec architecture reaches weft as one submission file, what weft checks offline and refuses by name, and the routes that plan, apply and run its snippet parts as Jobs.
---

A cloudspec architecture reaches weft as one file, a **submission**. weft reads it as it is and
translates nothing. Runs today, in the control host's test build: weft checks the file offline,
records it as a deployment source, plans it for an environment, defines one Job workload per
`snippet` part and runs that Job on a node to `Succeeded`. The test
[`cloudspec_snippet.rs`](https://github.com/beyond10x/weft/blob/main/product/cloud/crates/weft-testkit/tests/cloudspec_snippet.rs)
runs this path end to end.

## Write the submission

cloudspec writes the file:

```console
cloudspec generate weft --from <bundle|architecture> [--binding <file>] --out submission.json
```

`--from` takes a `cloudspec-architecture/1` document or a bundle that cloudspec translates first.
The format is specified in cloudspec's
[`docs/layers.md` § 11, Weft submission](https://github.com/beyond10x/cloudspec/blob/59129ebfcdbef65bbde498b749fb91c6ff5563b2/docs/layers.md#11-weft-submission),
at the commit `CLOUDSPEC_REV` of `Taskfile.yml` that weft pins.

The file is one JSON object, format `cloudspec-weft-submission/1`, with exactly five members:

| Member | Holds |
|---|---|
| `format` | `cloudspec-weft-submission/1` |
| `architecture` | the `cloudspec-architecture/1` document, as JSON |
| `binding` | the binding document as JSON (`--binding`), or `null` |
| `coverage` | the `cloudspec-coverage/1` report: one step per translation, the last one `generate weft` |
| `digests` | `{architecture, binding}`: the digest of each of those two members |

A digest is `sha256:<hex>` of the member's **canonical JSON**: object keys sorted by their UTF-8
bytes, no white space, strings and numbers as `serde_json` writes them. A `null` binding has a
digest too, the digest of `null`. The file is named by a third digest, `sha256:<hex>` of its
bytes: the source digest.

## What weft checks offline

The reader is the crate
[`weft-cloudspec`](https://github.com/beyond10x/weft/blob/main/product/cloud/crates/weft-cloudspec/src/lib.rs).
It reads the bytes of the file, opens no network connection, starts no process and needs no
cloudspec binary. It runs these checks in this order and answers the first refusal:

1. the shape: JSON, an object, the five members and no other, `digests` with two strings;
2. the format;
3. the architecture digest, then the binding digest;
4. when there is a binding: the architecture it names, then its roles.

The reader does not judge the coverage. A file whose coverage refuses objects is accepted; the
apply route skips the parts it refuses (see below).

## What weft refuses

Each refusal is named by its variant of `enum Refusal`. The submission route answers it as `422`
with the body `{"refusal": "<name>"}` and writes nothing.

| Refusal | The file |
|---|---|
| `NotASubmission` | is not JSON, not an object with exactly the five members, or its `digests` is not an object with a string `architecture` and a string `binding` |
| `UnknownFormat` | has a `format` other than `cloudspec-weft-submission/1`, another version included |
| `DigestMismatch` | declares a digest in `digests` that differs from the digest of the member's canonical JSON |
| `BindingForOtherArchitecture` | has a binding whose `architecture` is not the architecture's `architecture` |
| `BindingRoleNamesNoPart` | has a binding with a key of `roles` that no part names as `package.runtime.role` |

## What weft runs

weft runs one workload kind of a cloudspec architecture: `snippet`, one bounded command
(`{kind: snippet, command: [<argv>...], environment?}`) run to its end. A scope and an external
part map with nothing to run. Every other construct arrives in the coverage of the
`generate weft` step with the status `refused`, a rule that names it (such as
`weft_service_unsupported`, `weft_process_unsupported` or `weft_network_unsupported`) and the
note "weft does not run this yet". The full table of rules is in cloudspec's
[`docs/layers.md` § 11](https://github.com/beyond10x/cloudspec/blob/59129ebfcdbef65bbde498b749fb91c6ff5563b2/docs/layers.md#11-weft-submission).
When the coverage refuses an object, `cloudspec generate weft` exits 1 and still writes the file.

## The goldens

[`tests/fixtures/cloudspec/`](https://github.com/beyond10x/weft/tree/main/tests/fixtures/cloudspec)
holds three submissions written by cloudspec at the pinned commit. `goldens.lock` names that
commit and the sha256 of each file; a test fails when a file differs from its line or when the
lock names another commit than `CLOUDSPEC_REV`.

| File | Architecture | `generate weft` step | What weft does with it |
|---|---|---|---|
| `snippet.submission.json` | `hello`: one scope and one snippet part, no binding | 4 mapped, 0 refused | accepts it; runs the snippet `hello` to a `Succeeded` Job |
| `workstation.submission.json` | `workstation`, translated from an agenticspec deployment, with a binding | 3 mapped, 23 refused (services, agents, network paths, secrets, a volume) | accepts it |
| `host-daemon.submission.json` | `host-daemon`: process workloads on named hosts, no binding | 4 mapped, 28 refused (processes, host placement, host directories, sockets, data links) | accepts it |

## How a submission runs

The control host serves two routes under `/cloudspec/`, on the listener of its HTTP surfaces,
beside the command routes `/weft/commands/<name>`. Each request is sent by a caller acting in a
scope.

1. **Submit.** `POST /cloudspec/submissions?formation_id=<id>` with the bytes of the file as the
   body. The reader checks the file ([what weft refuses](#what-weft-refuses)). An accepted file
   runs `weft.deployments.SubmitDeploymentSource` in the caller's scope with kind `Cloudspec`,
   format version `cloudspec-weft-submission/1`, the source digest and the two member digests,
   and no translator. When the command answers `submitted`, the file's bytes are kept under the
   source digest in the same transaction. The route answers what the command answers; its
   event carries the `source_id`.
2. **Plan.** `weft.deployments.PlanFromSource` with `{source_id, scope_id, environment_id}`
   reads the kept file, checks it again with the reader, and records a new revision of the
   source's formation whose document digest is the architecture digest, and a plan that names the
   source and no translation. It answers `planned` with the `plan_id`.
3. **Record the plan's result.** `weft.deployments.RecordPlanResult`, sent as
   `weft.actors.ReleaseController` with the plan's diff and simulation, makes a plan without
   violations `Ready`. Nothing in the control host simulates a plan yet; the caller records the
   result.
4. **Apply.** `POST /cloudspec/plans/<plan_id>/apply` with `{"strategy": "Replace"}` (any rollout
   strategy). In one transaction the route reads the kept file, runs `weft.deployments.ApplyPlan`
   and, for each `snippet` part the coverage does not refuse, in key order: a component named
   after the part (an existing one of that name is reused), one Job workload
   (`weft.workloads.DefineWorkload`), its assignment to the plan's revision and environment, and
   a rollout step with 1 replica. Then it starts the release and its first step. It answers
   `202` `{"outcome": "applied", "release_id", "parts": [...]}`.
5. **Run.** The first step submits its Job. The scheduler places the Job's one instance on a
   node, the node agent runs the part's command with its environment, and when it exits 0 the
   task and the Job read `Succeeded`.

Every workload of a snippet part gets one fixed profile: shape `Job` (parallelism 1, no retries),
restart mode `Never`, 1 cpu, priority class `standard`, package kind `Snippet` on the registered
runtime named `snippet`, and one `Main` process that runs the part's `command` and
`environment`. Its identity is `<scope>/<formation>/<environment>/<part>`.

The routes' own answers:

| Status | Route | When |
|---|---|---|
| `400` | submit | no `formation_id` in the query |
| `400` | apply | the body names no `strategy` |
| `403` | both | the caller acts in no scope |
| `404` | both | a path under `/cloudspec/` that is neither route |
| `405` | both | a method other than `POST` |
| `422` | submit | a refused file, `{"refusal": "<name>"}` |
| `422` | apply | a plan that names no source, `{"refusal": "PlanNotFromCloudspec"}` |

A command that refuses inside the apply (such as `plan-not-ready`, or `no-runtime` when no
runtime named `snippet` is registered) makes the transaction commit nothing, and the route
answers that refusal.

Limits of what runs today:

- Only the control host's test build reads the caller of a request. A build without its test
  feature has no login and answers every request as one without a caller: it still refuses a
  malformed file by name, and answers `403` to an accepted one.
- A release with snippet parts starts only its first step; nothing completes a rollout step of a
  Job yet, so steps after position 1 stay `Pending`. A submission with no snippet part to run
  leaves its release `Planned`.

## Not built yet

:::caution[Planned]
Process parts (`kind: process`) do not run on a node yet. The apply route defines a supervised
`Daemon` workload with isolation `None` for a process part that names `run_as` (and refuses one
that does not), but no adapter of the node agent's workloads port exists yet, and the coverage at
the pinned cloudspec revision still refuses process parts (`weft_process_unsupported`). Every
other construct the coverage refuses arrives refused by name in the same way.
:::
