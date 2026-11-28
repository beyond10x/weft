---
title: Validate the specification
description: Validate weft's ESS specification with ess, and run the checks of the gate that hold its layers and its stamped time.
---

## With ess

Install `ess` 0.99.2, the version `ess/ess-inputs.yaml` requires
([install ess](https://beyond10x.github.io/ess/docs/start/install)). From the repository root:

```shell-session
$ ess specify validate --path ess --strict-requires
weft v1 — 25 file(s), valid
```

`--strict-requires` refuses, rather than warns, where `ess/ess-inputs.yaml` `requires` an older
`ess` release (the help text of `ess specify validate`). A finding prints its ESS code, the file
and the line.

To see the compiled model as JSON:

```shell-session
$ ess specify compile --path ess --format json > weft.json
```

## The checks beside ess

ESS validates each document and the system as a whole. Three rules of weft go further; each is a
POSIX sh script under `tools/` and a task of the gate.

| Task | Script | Fails when |
|---|---|---|
| `task spec` | `ess specify validate --path ess --strict-requires` | the specification is not valid |
| `task layers` | `tools/check-layers.sh` | a domain refers to a domain of a higher [layer](layers.md) |
| `task stamped-time` | `tools/check-stamped-time.sh`, then with `--negative` | a command with a `weft.shared.StampedAt` input is granted to an actor outside `tools/stamped-time-actors` |

Their output on a valid tree:

```shell-session
$ tools/check-layers.sh
layers: every reference points to its own or a lower layer
$ tools/check-stamped-time.sh
stamped-time: 7 StampedAt input(s) in 7 command(s), granted only to weft.actors.ControlPlane weft.actors.Scheduler weft.actors.ReleaseController
$ tools/check-stamped-time.sh --negative
stamped-time negative: a NodeAgent grant of weft.nodes.Heartbeat is refused
```

### Why stamped time

A time the control plane decides on from its own clock (a receipt, a lease end) is an input of
type `weft.shared.StampedAt`, and only the control plane's actors (`ControlPlane`, `Scheduler`,
`ReleaseController`) may run such a command. A node or a user cannot name it: a node that could
name its own receipt time could postpone its own loss. The negative run grants such a command to
`NodeAgent` in a scratch copy and requires the check to refuse it.

## All of it at once

```shell-session
$ task check
```

runs the checks above, the test of the CI path filter, the docs claims and the tests of the root
workspace. See [the gate](../operate/troubleshoot.md#the-gate).
