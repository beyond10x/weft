---
title: Managed services
description: The planned catalog of weft - managed services by name (queues, streams, caches, databases, object stores, search, single sign-on), their plans, service instances and bindings, and ready-made products.
status: planned
---

weft offers two levels of use. You build and deploy your own images as
[workloads](workloads.md), or you take a ready service from weft by name: a queue, a database, an
object store. The services weft operates are the **catalog**, domain `weft.catalog`
([`ess/7-services/catalog.yaml`](https://github.com/beyond10x/weft/blob/main/ess/7-services/catalog.yaml)),
owned by the component `weft-control` ([components](components.md)).

:::caution[Planned]
Specified, not built. The catalog is part of the specification, which `task check` validates
and weft-sim explores in memory. No release of weft provisions a service.
:::

Runs today: none of the components serves a user, no offering provisions and no product
installs. No engine runs behind any kind below; the catalog exists as specification only.

## Offering kinds

An **offering** is a kind of managed service weft provides by name, with a simplified API of its
own. Each kind's API is meant to become an ESS system of its own, with SDKs generated from it.

| Kind | What it is for |
|---|---|
| `queue` | Messages with acknowledgement, retry and a dead letter. |
| `stream` | An ordered, replayable event log with consumers. |
| `cache` | Key-value with expiry and no durability promise. |
| `kv` | Durable key-value with versions. |
| `sql` | A relational database. |
| `object-store` | Buckets of objects. |
| `search` | A text index. |
| `vector` | A vector index. |
| `scheduler` | Timed delivery of messages. |
| `model-endpoint` | An inference endpoint for a model package. |
| `sso` | Single sign-on a customer configures for its own users: identity provider connections, applications, a user directory. |

An **engine** is the versioned implementation behind one or more kinds. It is internal: a tenant
sees the offering's API, never the engine's, so an engine can be swapped without a change of that
API.

## Plans

<!-- generated: plans -->
`OfferingPlan`: A size tier of an offering - its limits, durability and replicas.

A plan fixes:

- `offering_id`: `weft.shared.OfferingId`
- `name`: `string`
- `limits`: `list of weft.shared.ResourceAmount`
- `durability`: `weft.catalog.Durability`
- `replicas`: `integer`
<!-- /generated: plans -->

## Service instances

A **service instance** is one provisioned offering in an account or a formation. weft runs on
weft: an instance is translated into components of a formation the system owns, on the same
nodes, with the same isolation and metering as any workload.

<!-- generated: instance-states -->
A service instance starts in `Provisioning` and ends in `Gone`. Its states: `Degraded`, `Deprovisioning`, `Gone`, `Provisioning`, `Ready`, `Updating`.

| Transition | From | To |
|---|---|---|
| `become_ready` | `Degraded`, `Provisioning`, `Updating` | `Ready` |
| `degrade` | `Provisioning`, `Ready`, `Updating` | `Degraded` |
| `update` | `Degraded`, `Ready` | `Updating` |
| `deprovision` | `Degraded`, `Provisioning`, `Ready`, `Updating` | `Deprovisioning` |
| `remove` | `Deprovisioning` | `Gone` |
<!-- /generated: instance-states -->

A durable instance can be backed up on request, and a completed backup restored into the
instance it was taken from.

## Bindings

A **service binding** gives one component of a formation access to a service instance, in one
mode. The component authenticates with its workload identity; there are no static credentials.
A component without a binding cannot reach the instance, the same default deny as for links
between components. The modes:

<!-- generated: binding-modes -->
- `publish`
- `consume`
- `read`
- `write`
- `admin`
<!-- /generated: binding-modes -->

## Products

A **product** is a ready-made application in the catalog, for example a build server with its
workers, a cache volume and an endpoint for build clients.

<!-- generated: products -->
- `Product`: A packaged application, published as versioned formation templates, run as the customer's own formation.
- `ProductVersion`: One immutable version of a product - its deployment source, parameters and needs.
- `Installation`: A product version enabled in an account with parameter values, run as a formation of the account.
<!-- /generated: products -->

### Offering or product

weft operates an offering itself, behind the offering's own API: the tenant sees a queue or a
database, never the formation that runs it. A product runs as the customer's own formation,
which the customer can see, scale and pay for per use; an upgrade moves the installation to
another version of its product, and an uninstall removes the formation. The specification gives
versions no order yet: whether an upgrade must go to a newer version, and whether a move to an
older one (a rollback) is allowed, is an open question
([`ess/7-services/catalog.yaml`](https://github.com/beyond10x/weft/blob/main/ess/7-services/catalog.yaml),
`UpgradeInstallation`).

## Related

- [Components](components.md): `weft-control`, which owns the catalog.
- [Formations](formations.md): what an installation and a service instance run as.
- [Status](../status.md): what exists today.
