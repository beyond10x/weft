---
format: aep.planning-md/3
id: story:release-assignment-closes-sessions
kind: story
status: draft
title: 'ReleaseAssignment answers without an unmet obligation: CloseAssignmentEndpointSessions has a behaviour'
relations:
- decomposes: epic:slot-node-daemon
- serves: vision:weft
revision: 1
---
## Epic

Decomposes `epic:slot-node-daemon`. Seen while building `story:slot-node-process`
(integration/2026-11-28-9): on the control host, `weft.nodes.ReleaseAssignment` answers
`committed: true` with "unmet obligation: command behaviour `CloseAssignmentEndpointSessions`".
A generated binding on the release reaches a command nothing implements, as the three exit bindings
did before `story:cloudspec-submission-job-exit`. The released-assignment test in weft-agent accepts
both answers, with a comment.

## Outcome

1. `ReleaseAssignment` answers 2xx with no unmet obligation: `CloseAssignmentEndpointSessions` has a
   behaviour, and its binding invokes nothing for an assignment with no open endpoint session.
2. The released-assignment test accepts only the 2xx answer.

## Scope

`product/cloud/crates/weft-impl/src/behaviours/` (one behaviour), `product/cloud/crates/weft-impl/src/bindings/`
(the binding delivered by hand if a zero-row case needs it), `tools/test-generated.sh`,
`tools/synth-input.sh`, `product/cloud/crates/weft-agent/tests/`.

## Acceptance

1. A testkit or agent test releases an assignment with no endpoint session and reads a 2xx answer
   with no `unmet obligation` text.
2. `slot --timeout 30m sh -c 'tools/test-setup.d/10-generated.sh && task check'` exits 0.
