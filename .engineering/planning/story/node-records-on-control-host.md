---
format: aep.planning-md/3
id: story:node-records-on-control-host
kind: story
status: active
title: The control host serves the weft.node records the node agent sends (D-N1 = A)
relations:
- decomposes: epic:slot-node-daemon
- serves: vision:weft
- depends_on: story:slot-node-process
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-28T15:38:26Z", actor: "human:operator", revision: 2, decided_on: {"recorded":{"approval":1}}}
- {from: "proposed", to: "active", at: "2026-11-28T15:38:26Z", actor: "human:operator", revision: 3, decided_on: {"recorded":{"approval":1}}}
---
## Epic

Decomposes `epic:slot-node-daemon`. The owner's pick D-N1 = A (2026-11-28, relayed by the
operator): the control host serves the `weft.node` records like every other record the node agent
sends; `weft-node` moves to the control-host component; one store, no relay.

`story:slot-node-process` (integration/2026-11-28-9, 00904ee) built the records behind a port,
`weft_agent::records::NodeRecords`. `ess/components.yaml` places `weft-node` in the agent process,
so the control host serves no route for `StartProcess`, `RestartProcess` and `StopProcess`; the
agent got 404, and without records each call is refused and logged while the process still runs.
Tests use `LocalRecords` over weft-impl's `weft.node` behaviours.

## Outcome

1. `ess/components.yaml`: the commands and views of `weft.node` are accepted by the control-host
   component; no component runs in the agent process for them.
2. The control host serves `StartProcess`, `RestartProcess`, `StopProcess` and the view
   `NodeInstanceRuns` like the agent's other records (the same transport and caller rules as
   `RecordInstanceRunning`).
3. The node agent's production `NodeRecords` adapter sends them to the control host; `agent::run_with`
   passes it in production. `LocalRecords` stays a test helper only if a test still needs it.
4. A `ProcessRestarted` (and every other event of these commands) is in the control host's log; no
   relay exists.

## Scope

`ess/components.yaml`, `ess/9-actors/actors.yaml` (grants if needed),
`product/cloud/crates/weft-agent/src/records.rs`, `product/cloud/crates/weft-agent/src/agent.rs`,
`product/cloud/crates/weft-bin/src/` (routes, if records are not served generically),
`product/cloud/crates/weft-agent/tests/process.rs`, `product/cloud/crates/weft-testkit/tests/`,
`quality/spec-diff-acknowledgements.json`, `website/docs/concepts/components.md` (counts).

## Acceptance

1. A testkit test with the control host and the skeleton node agent runs a process part through
   the Workloads stub, makes it exit 1 under restart Always, and reads `ProcessRestarted` in the
   control host's log and the restart in its rows; no 404 and no refused record line in the agent's
   report.
2. `ess specify validate --path ess --strict-requires` exits 0; `tools/check-spec-diff.sh
   --acknowledge` re-run after the last ess change.
3. `slot --timeout 30m sh -c 'tools/test-setup.d/10-generated.sh && task check'` exits 0.
