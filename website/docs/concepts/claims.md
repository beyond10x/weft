---
title: Claims
description: How a weft workload claims GPUs, disks, network addresses and secrets through one generic resource model - resource classes, claims and bindings.
status: decided
---

A workload holds resources through **claims**. Every resource, from a GPU to a public address, is
a **resource class** with the same five attributes, and the scheduler reads only those. Domains:
`weft.inventory` (resource classes and devices, operator-owned), `weft.resources` (volumes,
filesystems, buckets and snapshots of an account) and `weft.claims` (claims and bindings).

:::info[Specified, not built]
This page describes the specification. No running weft binds a claim yet.
:::

## Resource classes

| Attribute | Values |
|---|---|
| unit | `Count`, `Quantity`, `Attachment` |
| divisible | whole units only, or slices |
| sharing | `Exclusive`, `SharedReadOnly`, `SharedReadWrite` |
| scope | `NodeLocal`, `Zone`, `Cluster`: where a unit can move |
| lifetime | `Instance` (dies with the instance) or `Independent` |

Classes the design names:

| Group | Classes |
|---|---|
| Compute | `cpu`, `memory`, `gpu`, `gpu-slice`, `accelerator`, `interconnect` |
| Storage | `scratch`, `volume`, `filesystem`, `bucket`, `snapshot`, `cache` |
| Network | `endpoint`, `domain`, `private-network`, `public-address`, `egress`, `bandwidth` |
| Security | `secret`, `workload-identity`, `key` |
| Limits | `deadline`, `budget`, `concurrency`, `rate` |

## Claim and binding

A **claim** asks for an amount of one resource class with one sharing mode. A **binding** is the
concrete units on a node that hold it.

| Entity | States |
|---|---|
| Claim | `Pending` (declared, not bound), `Bound` (held by one active binding), `Released` |
| Binding | `Reserved` (placed by the scheduler, no unit taken), `Active` (the claim holds the units), `Released` (units given back; exclusive devices are wiped) |

An exclusive unit is held by at most one active binding. A component lists its claims; the
formation adds the implicit ones, its private network and its workload identity.

## Secrets and identity

Secrets and workload identities are claimed like other resources, with rules of their own
(`weft.secrets`, `weft.identity`):

- a secret value is written once through the API and never returned by it;
- a value reaches an instance only at start and only through a grant to the component's
  workload identity;
- every delivery is recorded; a value never appears in logs, plans, simulations or events;
- every instance gets a short-lived identity document, renewed by the node agent, and no
  long-lived key.
