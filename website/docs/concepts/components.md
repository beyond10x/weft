---
title: Components
description: The three planned server components of weft (weft-api, weft-control, weft-edge), the node agent of the worker plane, the plane each runs in, the domains each owns, and the weft CLI.
status: planned
---

weft runs as three server components, a node agent on every worker, and one command-line client.
The specification declares the components in
[`ess/components.yaml`](https://github.com/beyond10x/weft/blob/main/ess/components.yaml): each
owns a set of domains and accepts their commands. That file places the domains of cloudspec's
blocks, vendored under [`ess/cloud/`](https://github.com/beyond10x/weft/tree/main/ess/cloud),
the same way as weft's own.

:::caution[Planned]
Specified, not built. The components and their contracts are part of the specification, which
`task check` validates. No release of weft runs them.
:::

Runs today: none of the three components serves a user, no offering provisions, and no release
ships them. The walking skeleton under `product/cloud/` builds a control host binary named
`weft` (not the CLI of the same name below) that serves the generated HTTP surfaces of
`weft-api` and `weft-control` and the node channel, and a node agent, `weft-agent`, that joins
it. There is no login yet: built without its test feature, the control host answers every
request as one without a caller. The gate runs a test build of both in its tests. `weft-edge`
does not run.

## The components

Generated from the specification by `tools/docs-spec-pages.sh`; `task check` fails when the
table no longer matches it.

<!-- generated: components -->
| Component | What it does | Domains it owns | Commands it accepts |
|---|---|---|---|
| `weft-api` | The user API: logins, API tokens and the authorization of every management call; tenants, accounts and principals, and the scope each account opens; formations; the artifacts an account registers. | `weft.access`, `weft.accounts`, `weft.architecture`, `weft.artifacts`, `weft.formations`, `weft.scope` | 74 |
| `weft-control` | The control plane: the cluster and its inventory, tenant storage, workloads and their instances, claims, scheduling, the record of each node session and of the processes each node runs, workload identity, secret custody and delivery, telemetry, deployments and releases, the catalog and metering. | `weft.catalog`, `weft.claims`, `weft.cluster`, `weft.deployments`, `weft.identity`, `weft.infrastructure`, `weft.inventory`, `weft.limits`, `weft.metering`, `weft.node`, `weft.nodes`, `weft.observability`, `weft.pipelines`, `weft.resources`, `weft.scheduling`, `weft.secrets`, `weft.services`, `weft.topology`, `weft.workloads` | 359 |
| `weft-edge` | The edge gateway: the ready instances behind each route of a formation, the requests held while a sleeping service wakes, and the certificate of each public endpoint. | `weft.edge` | 15 |
<!-- /generated: components -->

Where a domain sits, from the same specification:

<!-- generated: placement -->
- Each domain with commands sits with exactly one component, which accepts every command of the domain.
- Domains without commands sit with no component: `weft.actors`, `weft.shared`.
- A component accepts no command of a domain it does not own.
<!-- /generated: placement -->

## Planes

Each component runs in one plane of the design
([`docs/domain-sketch.md`](https://github.com/beyond10x/weft/blob/main/docs/domain-sketch.md),
§ Structure).

| Plane | Component | What the plane does |
|---|---|---|
| Management | `weft-api` | The user API: tenants, accounts, formations, secrets, deploys. Called by the CLI and by CI. |
| Control | `weft-control` | State store, scheduler, identity issuer; translators are cloudspec's. Decides; runs no workload. |
| Worker | node agent (`weft-agent`) | One node agent per node: starts, observes and stops instances. Holds no state the control plane does not know: it sends its records, the processes it runs and their restarts included (`weft.node`), to `weft-control`. It is no component of the specification. |
| Data | `weft-edge` | Traffic to endpoints: the edge gateway, links between components, egress through the node's filter. |

How they talk:

- The CLI reaches the user API over HTTP, authenticated per access mode.
- A node dials out to the control plane: mutual TLS, one long-lived session per node
  ([`protocols/node-channel.yaml`](https://github.com/beyond10x/weft/blob/main/protocols/node-channel.yaml)).
  A worker opens no inbound port.
- Exec, attach, logs and port forwards run from the CLI through the user API and the node's
  session into an instance
  ([`protocols/exec-channel.yaml`](https://github.com/beyond10x/weft/blob/main/protocols/exec-channel.yaml)),
  never over a direct connection to the node.
- An instance asks its node's local endpoint for its identity document and its secrets; only that
  instance can reach it.

In the `local` placement, one machine for trying weft, the three server components run in one
process.

## weft CLI

The planned `weft` CLI is the command-line client of the user API: login, contexts, deploy,
plan, logs and secrets. It is no component: it holds no state and no power of its own beyond the
caller's, and every call goes through `weft-api`. Its commands will be specified as an
`ess-cli/2` surface of the specification; that document is not written yet. The CLI is not the
control host binary that the walking skeleton builds under the same name today: that binary
serves the user API and holds the state.

Where the code will live: the cloud, with the server components and the operator's CLI, under
`product/cloud/`; a consumer app such as the user's `weft` CLI in a product directory of its own
beside it, `product/<app>/`.

## Related

- [Managed services](managed-services.md): the catalog, which `weft-control` owns.
- [Nodes and clusters](nodes-and-clusters.md): the machines the node agent runs on.
- [Read the specification](../specification/read.md): the files of the specification.
