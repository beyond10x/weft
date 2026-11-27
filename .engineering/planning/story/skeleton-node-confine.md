---
format: aep.planning-md/3
id: story:skeleton-node-confine
kind: story
status: draft
title: weft-agent runs a snippet confined by substrate-daemon and observes its exit
relations:
- decomposes: epic:walking-skeleton
- depends_on: story:skeleton-workspace-gate
- serves: vision:weft
- depends_on: story:skeleton-generated-code
scope:
- confidence: inferred
  path: Cargo.lock
- confidence: cited
  path: crates/weft-agent
- confidence: cited
  path: tools/test-setup.d/20-substrate.sh
- confidence: cited
  path: tools/test-teardown.d/20-substrate.sh
revision: 11
---
## Epic

Decomposes `epic:walking-skeleton`, outcome 5 (the confined start). Depends on
`story:skeleton-workspace-gate` and `story:skeleton-generated-code`. Blocked by no ESS gap: it
touches neither `ess/` nor the generated code. It lands after `story:skeleton-generated-code` and
before `story:skeleton-control-host`, which depends on it, so that the `Cargo.lock` edits of the
three come one after the other.

## Context

Decision A6 of `docs/implementation.md`: `substrate-daemon` on the node, no root
(bubblewrap, user and other namespaces, delegated cgroup v2, seccomp), reached over its Unix socket
with `b10x-substrate-sdk` from the public repository `beyond10x/substrate` at an exact git rev.
Exec states `accepted | running | exited | cancelled | expired | unknown`, event `exec.exited`.
Ordinary execution has no egress: every run is under `--unshare-net` (substrate `README.md`).

This story creates `crates/weft-agent` (the node agent; the package name `weft-node` is the
generated component crate's, `generated/crates/weft-node`) with the module `confine`: given one process of an instance
(command, environment), it starts one exec through the SDK, reports its state changes, and on
`exec.exited` returns the exit code and the captured output. Nothing here talks to the control host.

- One `substrate-daemon` per test run, never one per test, through the setup hooks of
  `story:skeleton-workspace-gate`: `tools/test-setup.d/20-substrate.sh` (POSIX sh) installs the
  pinned substrate-daemon release (version and SHA-256) when the host lacks that version, prepares a CI
  runner for an unprivileged bubblewrap (user namespaces, delegated cgroup v2) where it runs in CI,
  and starts the daemon on a socket under `target/`, waiting on its socket file, not `pgrep -f`;
  `tools/test-teardown.d/20-substrate.sh` stops it by its PID. Tests read the socket path from the
  environment. What the CI runner needs is written in that hook, so the local gate equals the
  forge. This story edits neither the `test` task nor the workflow step list.
- Confinement probe: the test opens a TCP listener on host loopback and counts the connections it
  accepts. The probe is a small helper binary of this crate that connects to an address given as
  its argument and prints whether the connection succeeded. Positive control first: the same probe
  binary with the same arguments, run unconfined as a child of the test, connects, and the listener
  has accepted exactly one connection. Only then is the confined run counted: the same binary and
  arguments, passed into the exec workspace and started through `confine`, must report the
  connection refused or unreachable, and the listener count must stay at one (0 connections from
  the confined run). A sandbox that reaches the host network fails the test; a probe that cannot
  connect even unconfined fails it too, instead of passing as confinement.

## Domain relations

- A process of an instance on its node -> one confined exec: one exec per
  `weft.node.LocalProcess`, which is "one row per process of one instance on this node" and
  references its instance and assignment — inferable for the LocalProcess side:
  `ess/5-runtime/node.yaml`, entity `weft.node.LocalProcess`, relations `instance` and `assignment`.
  The substrate exec is no `ess/1` entity; it is the running form of that row on the host, not a
  domain relation.

## Scope

- `crates/weft-agent/` (`Cargo.toml` with `b10x-substrate-sdk` at an exact `rev`, `src/lib.rs`,
  `src/confine.rs`, the probe helper, `tests/main.rs` module `confine`)
- `tools/test-setup.d/20-substrate.sh`, `tools/test-teardown.d/20-substrate.sh`

## Acceptance

1. A test in `crates/weft-agent/tests/main.rs` starts `sh -c "echo weft"` through `substrate-daemon`
   with the `confine` module and observes the exec leave `accepted` and end in `exited` with code 0
   and output `weft`, in `slot task check` locally and in the CI Gate job.
2. In the same test binary, the probe binary run unconfined connects to a TCP listener the test
   opened on host loopback and the listener accepts exactly one connection; after that, the same
   binary with the same arguments started through `confine` reports the connection refused or
   unreachable, and the listener accepts no further connection.
## Rules for this work

- Builds and tests run through `slot`: `cargo` is shimmed; anything else that compiles or tests
  carries the `slot` prefix (`slot task check`). A script that only measures, probes or waits runs
  without it.
- No test runs over 60 s (nextest kills it); the test step ends within 120 s. A test over 5 s is
  named with its time in the report.
- A pool job is killed after 10 minutes unless it names its time with `slot --timeout` (at most
  30m; `SLOT_TIMEOUT=30m cargo ...` for a shimmed call).
- No Python in anything committed: Rust or POSIX sh (`yq` v4 may be called from sh).
- No load tests, no VMs, no GPU workloads.
- The test step makes no request that leaves the host: the control host, the node agent and
  substrate-daemon run on loopback and a Unix socket; seeded rows or fixtures stand in for
  anything else.
