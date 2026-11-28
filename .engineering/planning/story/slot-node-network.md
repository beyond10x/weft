---
format: aep.planning-md/3
id: story:slot-node-network
kind: story
status: draft
title: Ports, links, egress, border ingress and unix or FIFO sockets of an architecture run on the node (outline)
relations:
- decomposes: epic:slot-node-daemon
- serves: vision:weft
- depends_on: story:slot-node-process
scope:
- confidence: inferred
  path: product/cloud/crates/weft-agent/src
- confidence: inferred
  path: product/cloud/crates/weft-cloudspec/src
- confidence: inferred
  path: product/cloud/crates/weft-impl/src/behaviours
revision: 3
---
## Epic

Decomposes `epic:slot-node-daemon`, outcome 3 and the third step of its Order (network and
sockets). Outline: to be scoped before it is proposed. cloudspec rules this lets cloudspec map:
`weft_network_unsupported` (10 refusals of slot: ports 7411, 22 and 4226, egress `build-node-ssh`
and `crates.io:443`, four links without data, the border ingress of `slot-daemon`) and
`weft_socket_unsupported` (2: unix `jobs`, fifo `jobserver`): 12 of 44. PlanFromSource is needed
first, through `story:slot-node-process`.

## Substrate

Gap 2 (`substrate` `story:inbound-endpoint-for-a-confined-exec`) is draft, not implemented. A
process with isolation None binds its own port on the host and needs it only if
`decision-blocker:slot-node-isolation-none-family` is answered B; then this story also depends on
`story:ports-node-endpoints-local`. Gap 4 (`story:a-start-delivers-a-per-run-secret-value`, draft)
is not needed: every part of slot has `secrets: []` (row `SECRET`, 0 refusals).

## Outcome

1. A served TCP port (`port`, `bind: loopback|all`, `protocol`) is planned on the workload; the node
   reports the observed address once the process listens.
2. A unix or FIFO port (activation `None`: the process makes the path) is planned with its path and
   observed on the node; socket activation stays refused.
3. A link between parts is admitted when its target port is served and recorded with its carrier;
   an egress destination is recorded on the workload; a border part gets its ingress recorded.

## Scope

Worked out: `product/cloud/crates/weft-cloudspec/src/`, `product/cloud/crates/weft-impl/src/behaviours/`,
`product/cloud/crates/weft-agent/src/`.

## Acceptance

A planned process serving a TCP port, a unix socket and a FIFO reports all three observed on its
node, and a link to its unix port by `to_port` is admitted and recorded.

## Rules for this work

- Builds and tests through slot (`cargo` through its shim). One integration-test binary per crate.
  No test over 60 s (the runner kills it); the test step within 120 s; a test over 5 s is named
  with its time. A pool job ends within 10 minutes unless it names its time with `slot --timeout`
  (at most 30m).
- No network request in the test step. No Python in anything committed.
