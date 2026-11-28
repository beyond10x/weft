---
format: aep.planning-md/3
id: story:observability-event-feed-serve
kind: story
status: draft
title: The control host serves the event feed after a cursor and the placement view keyed by source part
relations:
- decomposes: epic:agent-workloads
- serves: vision:weft
- depends_on: story:observability-event-feed
revision: 1
---
## Epic

Decomposes `epic:agent-workloads`. Serves the event feed and the placement view specified by
`story:observability-event-feed` on the control host.

## Context

- A consumer's deployment plan (relayed by the agentic operator, 2026-11-28, rows R37 and R38)
  asks for a route that serves the event log as a feed resumable from a cursor, and a placement
  view keyed by the part of the source document.
- The control host's routes are generated per view from the specification (`weft-bin/src/lib.rs`);
  this story writes the view handlers over the event log the control host already keeps.
- A cursor is an opaque position in the log; a reader that sends it back gets only later events,
  in log order.

## Scope

- `product/cloud/crates/weft-control/` (feed and `PlacementsBySource` handlers),
  `product/cloud/crates/weft-testkit/`

## Acceptance

A reader takes the feed, keeps its cursor, places one more instance, and reading after the cursor
returns only the events of that placement, in order, each with its component's origin; the
placement view keyed by that origin answers the instance's node.

## Rules for this work

- Builds and tests run through `slot`. No test over 60 s; the test step ends within 120 s; a test
  over 5 s is named with its time. A pool job ends within 10 minutes unless it names its time with
  `slot --timeout` (at most 30m). No Python in anything committed. No network request beyond
  loopback in the test step.
