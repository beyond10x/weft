---
format: aep.planning-md/3
id: epic:platform-ports
kind: epic
status: active
title: 'Platform ports: weft placed on any substrate through ports and adapters, local adapter first'
relations:
- serves: vision:weft
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-27T23:56:30Z", actor: "human:operator", revision: 2, decided_on: {"recorded":{"approval":1}}}
- {from: "proposed", to: "active", at: "2026-11-27T23:56:30Z", actor: "human:operator", revision: 3, decided_on: {"recorded":{"approval":1}}}
---
# Platform ports: weft placed on any substrate through ports and adapters

## Outcome

weft names every capability it needs from the place it runs as a port, and serves each port with
one adapter per placement (`weft.infrastructure.Placement`: Local, Hosts, Aws, Kubernetes,
`ess/1-infrastructure/infrastructure.yaml:79`). Two layers:

- **Node layer**: what runs on one node (confined execs, long-lived workloads, endpoints into an
  instance, volumes, images). Served by substrate (`beyond10x/substrate`), extended where weft needs
  a resource family substrate's vision names but has not built.
- **Cloud layer**: what spans the installation (machines for node pools, edge and load balancing,
  DNS and certificates, object storage, the secret backend, managed databases). weft's own platform
  ports; adapters reach vendor APIs through connectors (`beyond10x/connectors`).

The default placement `local` runs every port on one Linux host with substrate, with no vendor
account and no network service beyond loopback.

## Order

The owner, 2026-11-28 about 00:55 CET, typed in this session: "let's do the architectural change.
Make the design right now, and then still implement the most cheapest, most local, least
distributed substrate layer or ports and adapter layer possible." Substrate stays the runtime for
now (same session, about 00:50 CET: "substrate is absolutely right for now").

## Scope

- A design document: the port list of both layers, each port's contract, the adapter of each
  placement, what the specification declares for them, and the substrate gaps.
- The ports in code with the `local` adapter only: the cheapest, most local, least distributed one.
- Other adapters (hosts, aws, kubernetes) are designed, not built.

## Acceptance

- The design document names every port with its operations and refusals, and for each placement
  the adapter that serves it or "not served".
- weft's control host and node reach the place they run only through the ports; a test fails when
  code outside an adapter calls substrate's SDK or spawns a node process directly.
- With placement `local`, a node pool's node is made by the machines port and joins on its own.
