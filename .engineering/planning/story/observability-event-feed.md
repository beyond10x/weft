---
format: aep.planning-md/3
id: story:observability-event-feed
kind: story
status: draft
title: 'Spec: an event feed with a cursor and a placement view keyed by source part'
relations:
- decomposes: epic:agent-workloads
- serves: vision:weft
- depends_on: story:agent-architecture-units
revision: 1
---
## Epic

Decomposes `epic:agent-workloads`: a specification change. weft fulfils cloudspec's observation
contract (cloudspec `story:observation-contract`, agenticspec request C5: per unit release state,
instances, health, failures and drift, and a resumable feed of a workload's own records, each
labelled with its source; agenticspec `design/deployment.md`, branch `design/ingress` at
`a47e0c9`, K15). Depends on `story:agent-architecture-units` (the component `origin`).

## Context

- weft has the pieces but no feed: instance health (`ess/4-applications/workloads.yaml:224-231`),
  log streams (`weft.observability.InstanceLogStreams`), drift (`weft.deployments.OpenDrifts`). No
  view is keyed by the source part an instance came from, and no route hands out events after a
  cursor.

## The specification change

- `ess/5-runtime/observability.yaml`: an event feed per formation: a view
  `weft.observability.EventsAfter` with params `formation_id` and `after` (a cursor, the sequence
  of the last event read) answering instance, release and drift events in order, each with the
  component's `origin`; and a feed of one instance's own records (an agent runtime's run records,
  appended by the instance through its workload identity) with the same cursor.
- `ess/4-applications/workloads.yaml` (or `ess/5-runtime/scheduling.yaml`): a view
  `PlacementsBySource` keyed by `origin.object` (the source part), answering desired and ready
  instances, health and the last failure per part.

## Scope

- `ess/5-runtime/observability.yaml`, `ess/4-applications/workloads.yaml`,
  `ess/9-actors/actors.yaml`

## Acceptance

`ess specify validate --path ess --strict-requires` passes with the feed view and
`PlacementsBySource`; a scenario reads the feed after a cursor and gets only the later events, in
order, each carrying its component's origin.
