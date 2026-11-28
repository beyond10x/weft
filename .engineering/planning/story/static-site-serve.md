---
format: aep.planning-md/3
id: story:static-site-serve
kind: story
status: draft
title: 'A Site workload: the edge serves an uploaded directory of static files on a route, beside its server unit'
relations:
- decomposes: epic:platform-ports
- serves: vision:weft
- depends_on: story:artifact-upload
- depends_on: story:ports-cloud-edge-local
revision: 1
---
## Epic

Decomposes `epic:platform-ports`: a UI built from specifications runs on weft without a container
image.

## Order

Requirement R81 of the agentic operator (2026-11-28): an uploaded directory of static files plus
the statement "serve these over HTTP" is a workload weft runs. First consumer: one Vite build beside
one long-running server unit.

## Context

- A `Site` artifact is a directory of static files registered by `story:artifact-upload`.
- `weft-edge` is the gateway in front of public endpoints (`docs/domain-sketch.md:77`); the `edge`
  port's local adapter is weft-edge on a loopback listener (`story:ports-cloud-edge-local`).
- Serving files needs no process and no root filesystem, so this path does not wait for substrate
  gap 5 (a root filesystem from an archive without a container daemon), which a `Image` artifact
  does.

## Outcome

1. A workload of package kind `Site` names a `Site` artifact by digest and a route; it has no
   process.
2. The edge serves the artifact's files from C5 `objects` on that route, with content types by
   extension, `index.html` for a directory, and an optional fallback file for client-side routing.
3. A route to a server unit beside it (an API path prefix) is part of the same formation, so the UI
   and its server share one origin.
4. A new digest on the workload switches the route at once; the old files are not served after the
   switch.

## Scope

- `ess/4-applications/workloads.yaml` (package kind `Site`), `ess/5-runtime/edge.yaml`,
  the edge adapter of `story:ports-cloud-edge-local` under `product/cloud/crates/` (no `weft-edge`
  crate exists yet),
  `product/cloud/crates/weft-testkit/`

## Acceptance

1. `ess specify validate --path ess --strict-requires` passes with package kind `Site`.
2. On the local placement, after a `Site` workload names an uploaded archive with `index.html`, a
   GET of its route on the loopback listener answers that file's bytes with `text/html`.
3. A GET of an unknown path under the route answers the fallback file when one is declared, and 404
   when none is.
4. After the workload names a second digest, a GET answers the second archive's `index.html`.

## Dependencies

- `story:artifact-upload`, `story:ports-cloud-edge-local`, `story:ports-cloud-objects-local`.
- cloudspec: a unit whose code is a directory of static files (requirement R79 of the agentic
  operator, sent to the cloudspec session).

## Rules for this work

- Builds and tests run through `slot`. No test over 60 s; the test step ends within 120 s. A pool
  job ends within 10 minutes unless it names its time with `slot --timeout` (at most 30m). No
  Python in anything committed. No network request beyond loopback in the test step.
