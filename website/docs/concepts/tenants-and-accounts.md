---
title: Tenants and accounts
description: Tenants, accounts nested to any depth, principals, roles, quotas, exports and how a user reaches the management API, as weft's specification defines them.
status: decided
---

Everything a user owns in weft sits in an account, and every account sits in a tenant. Domains:
`weft.accounts` and `weft.access`
([`ess/2-tenancy/`](https://github.com/beyond10x/weft/tree/main/ess/2-tenancy)).

:::info[Specified, not built]
This page describes the specification. No running weft enforces it yet.
:::

## Tenants

A **tenant** is a top-level customer of the cloud. It owns every account, principal, role
binding and quota below it. States: `Active`, `Closed`. A tenant closes only when none of its
accounts is open.

## Accounts

An **account** lives in a tenant or in another account, nested to any depth:

```text
acme                     tenant
acme/research            account
acme/research/gpu-lab    sub-account
```

A tenant or account name is a DNS label: lower-case letters, digits and `-`, 1 to 63 characters.
States: `Open`, `Closed`. Nothing crosses from one account to another unless an **export** names
it: one endpoint of an account shared with one other account of the same tenant (`weft.formations`).

## Principals and roles

A **principal** is a user, a service account or a workload identity. A **role binding** gives a
principal a role on an account and everything below it. The roles are a fixed set:

| Role | May |
|---|---|
| `viewer` | read state, logs and usage |
| `deployer` | deploy formations, set secrets, scale, exec |
| `admin` | manage one account and its sub-accounts |
| `billing` | read usage, quotas, prices and statements of its account |
| `owner` | the tenant owner: create accounts, grant roles, set quotas |

## Quotas

A **quota** is how much of one resource class an account may hold, together with its
sub-accounts. A sub-account draws its quota from its parent's remaining quota, so a child never
holds more than its parent has left. Usage rolls up the account tree and is checked at every level.

## Actors

The specification names who may run each command (`ess/9-actors/actors.yaml`).

| Actor | Who |
|---|---|
| `ClusterOperator` | runs the cloud: clouds, zones, node pools, nodes, price lists. Sees no tenant's secrets or workload data. |
| `TenantOwner` | creates accounts, grants roles, sets quotas in a tenant |
| `AccountAdmin` | manages one account and its sub-accounts |
| `Deployer` | deploys formations, sets secrets, scales, execs |
| `Viewer` | reads state, logs and usage |
| `Billing` | reads usage, quotas, prices and statements of its account |
| `Automation` | an API token acting with one role in one account, for CI |
| `Anonymous` | a CLI before login; may only start a device login |

System actors are parts of weft acting on their own: `NodeAgent`, `Scheduler`, `Translator`,
`Instance`, `EdgeGateway`, `ManagementApi`, `ReleaseController`, `ControlPlane`.

## Access

A cloud has one access mode, set when it is created:

- `SingleUser`: one implicit admin; the API listens on a local socket only its owner can read. No
  login.
- `Federated`: the CLI logs in through the browser (a device flow) with tokens from an external
  identity provider.

An **API token** is scoped to one account and one role and expires; CI uses it. Every management
call is authorized against role bindings and recorded.
