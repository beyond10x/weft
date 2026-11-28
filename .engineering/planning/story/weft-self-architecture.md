---
format: aep.planning-md/3
id: story:weft-self-architecture
kind: story
status: draft
title: weft's own deployment is a cloudspec architecture with control-plane, user-space and shared-services scopes, generated to a chart by cloudspec
relations:
- decomposes: epic:platform-ports
- serves: vision:weft
- depends_on: story:container-registry
revision: 1
---
## Epic

Decomposes `epic:platform-ports`: how weft itself is deployed.

## Order

The owner, 2026-11-28 about 03:49 CET, typed in this session: "The own deployment for Weft is
of course itself a cloud spec thing." The cloudspec architecture is turned into the target
environment by cloudspec's own transformation, a Helm chart for example. That chart holds three
namespaces: the operator or control plane; the user space; and shared services, such as an image
registry ("this is where we're putting the image to") or global seeding. "Weft itself, the
deployment of itself being the multi-tenant compute cloud, is having a cloud spec."

## Context

- Today weft's own deployment is four lines, `docs/domain-sketch.md` § Lowering (`:711-718`); only
  `local` is answered ("one process is control plane and worker"). The Kubernetes line maps
  tenant workloads, not weft (boundary review, 2026-11-28, `self-deploy.md`).
- The components: `weft-api`, `weft-control` (with the controllers), `weft-node`, `weft-edge`
  (`docs/domain-sketch.md:72-80`), the consoles (`product/console/`), and the registry of
  `story:container-registry`.
- Generators are cloudspec's (D5 A, 2026-11-28); a Helm output is cloudspec's generator, not
  weft code.

## Outcome

1. `deploy/weft.cloudspec.yaml` (path held until cloudspec's architecture format lands): weft's own
   architecture with three scopes:
   - `control-plane`: weft-api, weft-control and its controllers, the console server;
   - `user-space`: where tenant formations are placed; on Kubernetes the nodes' workloads;
   - `shared-services`: the container registry, seeding of engines' images and the catalog.
2. Each component is a workload of that architecture with its ports, its contract needs (the
   control plane's record store, the registry's artifact store) and its health checks.
3. cloudspec's Kubernetes or Helm generator turns it into a chart with one namespace per scope;
   `local` stays one process (the same architecture, bound to the memory or local placement).
4. Not self-hosted: the control plane is started by its target (Kubernetes, systemd), never by
   weft itself (review finding: a control plane cannot restart itself after a crash).

## Scope

- `deploy/` (new), `docs/domain-sketch.md` § Lowering, `website/docs/run/`

## Acceptance

1. The architecture file validates with cloudspec's checker at a pinned cloudspec commit.
2. cloudspec's generator renders it to a chart with three namespaces, `control-plane`,
   `user-space` and `shared-services`, and a coverage report with no refused element.
3. `docs/domain-sketch.md` § Lowering names the file and drops the four-line sketch.

## Dependencies outside weft

- cloudspec: the architecture format and the Kubernetes or Helm generator
  (`story:generate-out-kubernetes`; scopes as namespaces).

## Rules for this work

- Builds and tests run through `slot`. No test over 60 s; the test step ends within 120 s. A pool
  job ends within 10 minutes unless it names its time with `slot --timeout` (at most 30m). No
  Python in anything committed. No network request beyond loopback in the test step.
