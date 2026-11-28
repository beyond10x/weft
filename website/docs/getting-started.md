---
title: Getting started
description: Clone weft, validate its ESS specification with ess, install cloudspec-sim and explore the specification in memory.
---

What you can run today is the specification and its simulator. This page validates the
specification, installs cloudspec-sim and runs a short exploration. There is no cloud to start yet
([Run weft](run/control-host-and-node.md) is planned).

## What you need

| Tool | Version | Used for |
|---|---|---|
| `git` | any | cloning the repository |
| [`ess`](https://beyond10x.github.io/ess/docs/start/install) | 0.99.2, the version `ess/ess-inputs.yaml` requires | validating the specification |
| Rust, through `rustup` | 1.98.1, pinned in `rust-toolchain.toml` | installing cloudspec-sim |
| [Task](https://taskfile.dev) | 3 | running the gate and the simulator tasks |
| `cargo-nextest` | any recent | the tests in `task check` |

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

## Install cloudspec-sim

```shell-session
$ task sim-install
```

This runs `cargo install` of
[cloudspec-sim](https://github.com/beyond10x/cloudspec/tree/2ed72c2866bbd165da2de75250b2bfcb340a96ae/crates/cloudspec-sim)
at the cloudspec revision weft pins (`CLOUDSPEC_REV` in `Taskfile.yml`). The first install fetches
cloudspec and the ESS crates it uses from GitHub and compiles them. The binary is
`target/cloudspec-sim/bin/cloudspec-sim`; its `--spec` defaults to `blocks`, so pass `--spec ess`.
[Install cloudspec-sim](simulator/install.md) has the details.

List what the specification declares:

```shell-session
$ target/cloudspec-sim/bin/cloudspec-sim describe --spec ess --prefix weft.accounts.CreateTenant
weft: 439 commands, 2533 outcomes, 125 entities, 328 states, 221 transitions, 16 actors, 57 bindings; compiled in 0.38 s

weft.accounts.CreateTenant  [ClusterOperator]
    in  name: weft.accounts.Name
    -> name-taken (related_set, error)
    -> created (otherwise) creates weft.accounts.Tenant emits TenantCreated
```

## Explore

A short exploration: 64 random walks of up to 60 commands each, on 4 threads.

```shell-session
$ target/cloudspec-sim/bin/cloudspec-sim explore --spec ess --seed 1 --walks 64 --steps 60 --threads 4 --out sim/out/first
cloudspec-sim explore: 64 walks, 3860 steps in 0.3 s (12783 steps/s, 212.0 walks/s, 4 threads); outcomes 643/2533, states 70/328, transitions 33/221, abstract states 1146, branch points 29, violations 0; commands never accepted 346 (blocked-by 2)
cloudspec-sim: system checks (closed-link,idempotency,no-double-placement,quota-conservation,tenant-isolation, every 1 steps) 0.1 s summed, 20 duplicates sent; convergence: 0 walks, 0 ticks, 0 requests, 0.0 s summed
cloudspec-sim: wrote sim/out/first/result.json and sim/out/first/index.html
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
- [Run cloudspec-sim](simulator/run.md): explore from a full setup, experiments, replay.
