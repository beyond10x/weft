---
title: Getting started
description: Clone weft, validate its ESS specification with ess, build weft-sim and explore the specification in memory.
---

What you can run today is the specification and its simulator. This page validates the
specification, builds weft-sim and runs a short exploration. There is no cloud to start yet
([Run weft](run/control-host-and-node.md) is planned).

## What you need

| Tool | Version | Used for |
|---|---|---|
| `git` | any | cloning the repository |
| [`ess`](https://beyond10x.github.io/ess/docs/start/install) | 0.99.2, the version `ess/ess-inputs.yaml` requires | validating the specification |
| Rust, through `rustup` | 1.98.1, pinned in `tools/sim/rust-toolchain.toml` | building weft-sim |
| [Task](https://taskfile.dev) | 3 | running the gate and the simulator tasks |
| `cargo-nextest` | any recent | the simulator's tests in `task check` |

The ESS install page shows the release archive for any version; set `version=0.99.2`.

## Clone

```shell-session
$ git clone https://github.com/beyond10x/weft.git
$ cd weft
```

## Validate the specification

```shell-session
$ ess specify validate --path ess --strict-requires
weft v1 — 25 file(s), valid
```

25 files: `system.yaml`, `components.yaml` and the 23 domain files listed in
`ess/ess-inputs.yaml`. [Validate the specification](specification/validate.md) covers the other
checks of the gate.

## Build weft-sim

```shell-session
$ task sim-build
```

This runs `cargo build` in `tools/sim`. The first build fetches the ESS crates weft-sim uses from
GitHub and compiles them. The binary is `tools/sim/target/debug/weft-sim`.

List what the specification declares:

```shell-session
$ tools/sim/target/debug/weft-sim describe --spec ess --prefix weft.accounts.CreateTenant
weft: 439 commands, 2525 outcomes, 125 entities, 328 states, 221 transitions, 16 actors, 57 bindings; compiled in 0.37 s

weft.accounts.CreateTenant  [ClusterOperator]
    in  name: weft.accounts.Name
    -> name-taken (related_set, error)
    -> created (otherwise) creates weft.accounts.Tenant emits TenantCreated
```

## Explore

A short exploration: 64 random walks of up to 60 commands each, on 4 threads.

```shell-session
$ tools/sim/target/debug/weft-sim explore --spec ess --seed 1 --walks 64 --steps 60 --threads 4 --out sim/out/first
weft-sim explore: 64 walks, 3865 steps in 0.3 s (12836 steps/s, 212.5 walks/s, 4 threads); outcomes 664/2525, states 76/328, transitions 37/221, abstract states 1145, branch points 26, violations 0; commands never accepted 339 (blocked-by 2)
weft-sim: system checks (closed-link,idempotency,no-double-placement,quota-conservation,tenant-isolation, every 1 steps) 0.1 s summed, 25 duplicates sent; convergence: 0 walks, 0 ticks, 0 requests, 0.0 s summed
weft-sim: wrote sim/out/first/result.json and sim/out/first/index.html
```

Open `sim/out/first/index.html` in a browser. [Read the results](simulator/results.md) explains
each section. Timings and counts depend on the machine and on the revision of the specification.

`sim/out/` is ignored by Git.

## Run the gate

```shell-session
$ task check
```

The same check runs in CI on every pull request. See
[the gate](operate/troubleshoot.md#the-gate) for what each step checks.

## Next

- [Concepts](concepts/tenants-and-accounts.md)
- [Read the specification](specification/read.md)
- [Run weft-sim](simulator/run.md): explore from a full setup, experiments, replay.
