---
format: aep.planning-md/3
id: story:binding-stand-ins-replaced
kind: story
status: draft
title: 'The Job chain''s binding stand-ins give way to the patterns ESS names: related reads, subject fields, per-row events'
relations:
- decomposes: epic:cloudspec-submission
- serves: vision:weft
- depends_on: story:cloudspec-submission-job-exit
revision: 1
---
## Epic

Decomposes `epic:cloudspec-submission`. Stories `cloudspec-submission-job-run` and
`cloudspec-submission-job-exit` declared stand-ins in `ess/components.yaml` (marked `OPEN (ESS)`)
where a binding seemed unable to map an input. ESS answered (2026-11-28, checked on 0.99.9 and
main) with the patterns that replace three of them; two wait on ESS drafts.

## Outcome

1. Inputs from fields of the row an event id names (ESS: refused by design, `docs/design/
   binding-stored-state.md`; story `feature-request-440`): the binding maps the id only, and the
   invoked command reads the row with `{related: {via: input.<id>, field: <f>}}`. The literals 1,
   false and 0 of `r1-rollout-submit-job` go; `SubmitJob` reads parallelism, gang and max_retries
   from the Workload row.
2. Inputs from a field of another row (the conversions `JobId -> WorkloadId` and
   `InstanceId -> JobId`): the triggering command's outcome publishes the field in its event with
   `{subject: <field>}`, and the binding maps it from the event. Both conversions go.
3. Zero-row outcomes (the telemetry bindings on `InstanceExited`): an `instances:` outcome with
   `emits: [{event: <E>, each: changed}]` publishes one event per changed row and none for zero
   rows (ESS since ess/25, 0.60.0). The telemetry bindings are generated again where that removes
   their hand-run delivery.
4. Kept OPEN, with the ESS story named beside each: fan-out over task indexes (ESS story
   `a-specification-may-state-one-run-of-a-process-with-a-command-once-per-member-of-a-set-and-a-command-once-per`,
   undecided), an Integer comparison in `where:` (draft
   `a-binding-condition-may-compare-an-integer-event-field-with-an-integer-literal`), a list literal
   (draft `a-binding-mapping-constant-may-be-a-list-literal-typed-against-the-list-input-it-fills`).

## Scope

`ess/components.yaml`, `ess/4-applications/workloads.yaml` (or the weft file declaring the
commands, never `ess/cloud/`), `product/cloud/crates/weft-impl/src/bindings/`,
`product/cloud/crates/weft-impl/src/behaviours/`, `tools/test-generated.sh`, `tools/synth-input.sh`,
`quality/spec-diff-acknowledgements.json`. If a pattern needs an ess release newer than the one
`ess/ess-inputs.yaml` requires, the `requires:` move is part of this story.

## Acceptance

1. `grep -n "OPEN (ESS)" ess/components.yaml` lists only the three items of outcome 4.
2. The job_run tests of weft-testkit pass unchanged.
3. `slot --timeout 30m sh -c 'tools/test-setup.d/10-generated.sh && task check'` exits 0.
