---
title: Install weft-sim
description: Build weft-sim, the in-memory explorer and simulator of weft's specification, from the repository.
---

weft-sim explores and simulates weft's specification in memory. It compiles the specification
with the ESS compiler and runs commands against ESS's interpreted target (crate
`ess-conformance`), so no part of weft needs to exist to run it. Source:
[`tools/sim/`](https://github.com/beyond10x/weft/tree/main/tools/sim).

weft-sim is built from the repository; there is no release binary.

## Build

You need Rust through `rustup`; `tools/sim/rust-toolchain.toml` pins 1.98.1 and `rustup` installs
it on the first build. The ESS crates are Git dependencies pinned to one ESS revision in
`tools/sim/Cargo.toml`, fetched from GitHub on the first build.

```shell-session
$ task sim-build
```

or, without Task:

```shell-session
$ cd tools/sim
$ cargo build
```

The binary is `tools/sim/target/debug/weft-sim`. The tasks `sim-explore`, `sim-explore-from`,
`sim-explore-ticks` and `sim-experiments` build it first.

## Check it

```shell-session
$ tools/sim/target/debug/weft-sim help
```

prints the usage of the four commands: `explore`, `simulate`, `replay` and `describe`.

## Test it

```shell-session
$ task sim-test
```

builds the test binary, then runs the tests with `cargo nextest`. A test over 60 s is killed and
counts as failed, and the whole test run fails after 120 s. Install
[cargo-nextest](https://nexte.st) first.

## The ESS revision

weft-sim depends on an ESS revision from `main` rather than a tag: the run record of each report
names it (`run.ess` in `result.json`). The validation of the gate uses the released `ess` 0.99.2.
