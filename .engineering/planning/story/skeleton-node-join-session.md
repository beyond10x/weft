---
format: aep.planning-md/3
id: story:skeleton-node-join-session
kind: story
status: active
title: weft-agent joins with its token and opens its session through the node-channel endpoint of the control host
relations:
- decomposes: epic:walking-skeleton
- depends_on: story:skeleton-node-confine
- serves: vision:weft
- depends_on: story:skeleton-control-host
scope:
- confidence: inferred
  path: Cargo.lock
- confidence: inferred
  path: product/cloud/crates/weft-agent/Cargo.toml
- confidence: cited
  path: product/cloud/crates/weft-agent/src/agent.rs
- confidence: cited
  path: product/cloud/crates/weft-agent/src/main.rs
- confidence: cited
  path: product/cloud/crates/weft-agent/src/report.rs
- confidence: cited
  path: product/cloud/crates/weft-agent/tests/main.rs
- confidence: cited
  path: product/cloud/crates/weft-bin/src/main.rs
- confidence: cited
  path: product/cloud/crates/weft-bin/src/node_channel.rs
- confidence: cited
  path: product/cloud/crates/weft-impl/src/behaviours/open_node_session.rs
- confidence: cited
  path: product/cloud/crates/weft-impl/src/behaviours/record_node_observation.rs
- confidence: cited
  path: product/cloud/crates/weft-impl/src/behaviours/supersede_node_session.rs
- confidence: cited
  path: product/cloud/crates/weft-testkit/src
revision: 27
transitions:
- {from: "draft", to: "proposed", at: "2026-11-27T17:18:35Z", actor: "human:operator", revision: 24, decided_on: {"recorded":{"approval":1}}}
- {from: "proposed", to: "active", at: "2026-11-27T17:18:35Z", actor: "human:operator", revision: 25, decided_on: {"recorded":{"approval":1}}}
---
## Epic

Decomposes `epic:walking-skeleton`, outcome 5 (join and session) and outcome 3 (the obligations of
the node part of the path up to the session). Depends on `story:skeleton-control-host` (`weft-bin`,
its log, `weft-client`, `weft-testkit`) and `story:skeleton-node-confine` (the `weft-agent` crate).
Blocked by no ESS gap: no E1-E3 transformation touches these commands.

## Context

Decision A5 (`docs/implementation.md`): the node dials the control host over HTTP on loopback with
the join token `ProvisionPoolNode` issued, no inbound port on the node, mTLS a later story.

- Join: `weft.infrastructure.JoinNode` (generated) as NodeAgent with node id and join token, then
  `weft.infrastructure.RecordNodeObservation` as NodeAgent with its capacity and isolation facts;
  the levels it reports are those substrate confinement gives (`Container`), so `PlaceInstance`
  can choose this node.
- Session: no frame of the node carries a time (`protocols/node-channel.yaml` header, hardening
  pass 5). `OpenNodeSession` takes `received_at` (`weft.shared.StampedAt`) and is granted to
  ControlPlane alone (`ess/9-actors/actors.yaml` line 1050). So `weft-bin` gets a node-channel
  endpoint, `product/cloud/crates/weft-bin/src/node_channel.rs`, served on the same listener under `/node/`: the
  node sends a `hello` frame; the endpoint sends `weft.nodes.OpenNodeSession` in-process as
  ControlPlane, with `received_at` from the context clock on receipt, and answers `hello` with the
  session id or the refusal. As the protocol model says (`protocols/node-channel.yaml`): a `hello`
  for a node whose session is still `Open` runs `weft.nodes.SupersedeNodeSession` on it first
  (`replace-open-session`, `ess/5-runtime/nodes.yaml`), so a restarted agent gets a new session;
  a refused `hello` ends the agent (`session-refused`), and only a lost connection is redialled
  after the redial delay (3000 ms). Every answered request on `/node/`, a refusal of the endpoint
  itself included, writes its request line to the control host log.
- Obligations implemented here, each in its own file under `product/cloud/crates/weft-impl/src/behaviours/`:
  `weft.infrastructure.RecordNodeObservation`, `weft.nodes.OpenNodeSession` and
  `weft.nodes.SupersedeNodeSession` (command behaviours on the skeleton path).
- Not here: heartbeats (`weft.nodes.Heartbeat`), lease renewal and the R3 loss of a silent
  session. The skeleton path needs none of them; they come with the story that runs R3.
- `weft-agent` gets its binary: arguments control URL, node id, join token, substrate socket,
  report file.
- Report file: `weft-agent --report <path>` writes one JSON line per event the tests need, to a
  file the testkit names. This story writes `{"hello": "opened", "session_id": ...}` or
  `{"hello": "refused", "status": <http status>}` for each answer to `hello`;
  `story:skeleton-node-run-assignment` adds the lines of finished processes and work-list passes.
- Control host log, delta read: the test reads the highest `seq` of the control host log before it
  starts `weft-agent`, and after the first `hello` line of the report file reads only the log lines
  after that `seq`. The test uses a `ControlHost` of its own.
- `weft-testkit` starts `weft-agent` as a child process beside `ControlHost`.

## Domain relations

- `weft.nodes.NodeSession` -> `weft.infrastructure.Node`, many-to-one, references; the node
  outlives every session — inferable: `ess/5-runtime/nodes.yaml`, entity `weft.nodes.NodeSession`,
  relation `node`.
- `weft.infrastructure.Node` -> `weft.infrastructure.NodePool`, many-to-one, references; the node
  joins the pool it was provisioned in — inferable: `ess/1-infrastructure/infrastructure.yaml`,
  entity `weft.infrastructure.Node`, relation `pool`.

## Scope

- `product/cloud/crates/weft-agent/` (`src/main.rs`, `src/agent.rs`, `src/report.rs`, `tests/main.rs` module
  `join`)
- `product/cloud/crates/weft-bin/` (`src/node_channel.rs`, routing in `src/main.rs`)
- `product/cloud/crates/weft-impl/` (`src/behaviours/`: `record_node_observation.rs`,
  `open_node_session.rs`, `supersede_node_session.rs`)
- `product/cloud/crates/weft-testkit/` (start `weft-agent` as a child process)

## Acceptance

1. Against a running control host, the node provisioned by `ProvisionPoolNode` reads `Provisioning`
   in `weft.infrastructure.Nodes` before `weft-agent` starts, and `Ready` after `weft-agent` started
   with that node id and join token.
2. Its first `hello` gets the report line `{"hello": "opened", ...}`, and `weft.nodes.NodeSessions`
   holds an `Open` row for the node with that `session_id` and an `opened_at` between the wall-clock
   reads taken before the agent started and after that report line.
3. The control host log after the `seq` read before the agent started holds a request line for
   `weft.nodes.OpenNodeSession` with actor `weft.actors.ControlPlane` and a 2xx status.
4. With the number of rows of `weft.nodes.NodeSessions` read first, `weft.nodes.OpenNodeSession`
   sent over HTTP as `weft.actors.NodeAgent` is answered `403` `not granted`, and the number of rows
   is the same afterwards, the negative side of statement 2.
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
