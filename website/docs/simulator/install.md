---
title: Install cloudspec-sim
description: Install cloudspec-sim, the in-memory explorer and simulator weft uses on its specification, at the revision weft pins.
---

cloudspec-sim explores and simulates weft's specification in memory. It compiles the specification
with the ESS compiler and runs commands against ESS's interpreted target (crate
`ess-conformance`), so no part of weft needs to exist to run it. It is a crate of cloudspec:
[`crates/cloudspec-sim`](https://github.com/beyond10x/cloudspec/tree/2ed72c2866bbd165da2de75250b2bfcb340a96ae/crates/cloudspec-sim).
Until 2026-11-28 it was weft-sim, in weft's own repository.

There is no release binary. weft pins one cloudspec revision, `CLOUDSPEC_REV` in `Taskfile.yml`,
and installs cloudspec-sim from it.

## Install

You need Rust through `rustup`; weft's `rust-toolchain.toml` pins the toolchain and `rustup`
installs it on the first build. The ESS crates are Git dependencies of cloudspec-sim, fetched from
GitHub on the first install.

```shell-session
$ task sim-install
```

runs `cargo install` with the revision of `CLOUDSPEC_REV` in place of `<rev>`:

```shell-session
$ cargo install --git https://github.com/beyond10x/cloudspec --rev <rev> cloudspec-sim --locked --root target/cloudspec-sim
```

The binary is `target/cloudspec-sim/bin/cloudspec-sim`. The tasks `sim-explore`, `sim-explore-from`,
`sim-explore-ticks`, `sim-experiments` and `sim-relations` install it first; a second install of the
same revision does nothing.

## Check it

```shell-session
$ target/cloudspec-sim/bin/cloudspec-sim help
```

prints the usage of its commands, among them `explore`, `simulate`, `replay` and `describe`.

## Run it on weft

cloudspec-sim's `--spec` defaults to `blocks`, cloudspec's own specification. On weft, pass
`--spec ess`; the `sim-*` tasks do.

## Its tests

cloudspec tests cloudspec-sim in its own repository. weft's gate, `task check`, does not install
or test it: the gate makes no network request.

## The ESS revision

cloudspec-sim depends on an ESS revision from `main` rather than a tag: the run record of each
report names it (`run.ess` in `result.json`). The validation of the gate uses the released `ess`
0.99.2.
