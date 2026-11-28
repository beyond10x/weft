---
title: The consoles
description: The operator console and the account console of weft, specified as ESS UI documents and drawn by weft's own renderer on fixture data.
---

weft has two web consoles for two different people. Each one is an ESS UI document (`ess-ui/1`)
checked against weft's specification. The same renderer draws both. The renderer and its
components live in [`product/console`](https://github.com/beyond10x/weft/tree/main/product/console).

:::note[Fixture data]
No weft cloud runs yet, so the consoles answer every read from fixture files. A command you send,
such as Drain or Scale, shows what it would send and changes nothing. The names, ids and hosts in
the fixtures are invented.
:::

## Operator console

Actor: `weft.actors.ClusterOperator`, the operator who runs the cloud. Document:
[`product/console/ui/operator.ui.yaml`](https://github.com/beyond10x/weft/blob/main/product/console/ui/operator.ui.yaml).

| Page | Shows | Commands |
|---|---|---|
| Overview | Counts of nodes, node pools, tenants and requests, the clouds with their access mode and control plane, requests that cannot be placed, agent sessions that are not open, preemptions and drains | Create a cloud, then configure its access mode and control plane; scale the control plane, set the cloud policy |
| Nodes, node detail | Every node with its zone, pool, role, capacity, isolation facts and state. On one node: its devices, agent sessions, observations, placements, claim bindings and drains | Provision, cordon, uncordon, drain, remove, reset a device, cancel a drain, issue a join token |
| Node pools | Pools with their desired size drawn between the minimum and the maximum, and their scale requests | Create, edit, scale up and down, delete |
| Zones, join tokens, inventory | Zones, nodes waiting to join, resource classes and free devices | Declare and retire a zone, issue a join token, provision a pool node, define and retire a resource class |
| Requests, placements, priority classes | Scheduling requests grouped by state with the scope that asked, gangs, placement decisions and priority classes | Define and retire a priority class |
| Tenants, tenant detail, quotas | Tenants, their account tree, quotas with a usage bar, roles and principals | Create and close a tenant, bind an owner, grant, raise and lower quotas |
| Platform | Runtimes, translators, trust domains and secret backends | Request a key rotation, retire a runtime or a translator |

## Account console

Actors: `weft.actors.Deployer` on most pages, and `weft.actors.AccountAdmin` on the formations
list and on usage and cost. A tenant's engineer ships and runs workloads there. Document:
[`product/console/ui/account.ui.yaml`](https://github.com/beyond10x/weft/blob/main/product/console/ui/account.ui.yaml).

| Page | Shows | Commands |
|---|---|---|
| Overview | Formations, instances, managed services, spend by meter, alerts and services with their running replicas | |
| Formations, formation detail | Formations as cards; on one formation its revisions, components, workloads and environments, then its network (links only for now: the network page has no border-route and endpoint sections until it is decided how weft-edge is reached) and config (configs, mounts, service bindings) | Create, pin to a cluster (a formation is placed nowhere until it is pinned), retire, commit a revision, remove a component or link, unbind a service |
| Workloads, services, instances | Workloads of a formation, services with a replica bar, instances grouped by state. On one instance: its record, identity documents, log streams with their lines, observations, assignments and exec sessions | Scale a service, retire a workload, pause, resume, snapshot and stop an instance, cancel a job |
| Releases, release detail | Releases of an environment, plans, rollout steps with a ready-replica bar, drift | Apply or discard a plan, roll back |
| Catalog, managed services | Offerings and products as cards; managed service instances with bindings, backups and restores | Provision, back up, restore, deprovision |
| Secrets | Secret references and, under each row, its grants. A secret value is never shown | Create, add a version, grant |
| Usage and cost | Spend by meter for a time window, usage by meter (CSV export), budgets with a spend bar, statements | Set, change and remove a budget |

## Run them locally

The consoles need Node 22 or later and the `ess` CLI on the `PATH`. From the repository root:

```shell-session
$ npm --prefix product/console ci
$ npm --prefix product/console run dev
```

Open `http://127.0.0.1:4416/` and choose a console: `/operator/` or `/account/`. The page
switches between light and dark with the system setting. The account menu at the top right can
fix it to one of them. Add `?latency=0` before the `#` to answer reads without the simulated
delay. The account console signs in with the roles `deployer` and `account_admin`; add
`?actor=deployer` to sign in with `deployer` only. The menu and the pages then leave out what that
role is not granted, such as the formations list and "New formation".

To serve a production build instead:

```shell-session
$ npm --prefix product/console run build
$ npm --prefix product/console run preview
```

The build goes to `target/console/dist`. `preview` serves it on port 4417.

## How the consoles are built

- **The documents.** `ess ui check --model ess` checks each document: every view, command and
  event it names exists in the specification, and each page's actor is granted every command the
  page sends. A part that ESS UI cannot express is a host slot. Examples are the usage bars and the
  log tail. Each slot names its host component and the reason it exists.
- **The renderer.** It is React and TypeScript, built with Vite, and it does not use the renderer
  that ESS ships. At build time it parses both documents and reads the fixtures they name. It also
  takes from the compiled specification the enum variants for choices and the command inputs for
  forms. It then draws each section kind (collection, record, metric, chart, filter bar, form)
  with its own components. The two apps share the renderer and the component library. Each has
  its own accent color from its document's tokens.
- **The signed-in actor.** Its roles name actors of the specification. A page shows when the actor
  holds the page's actor; an action shows when its command is granted. An input the caller owns,
  such as `account_id` or `scope_id`, is always the actor's own. Every read of the account console
  that the specification can scope binds `scope_id` to the scope the actor's account opened. The
  reads it cannot scope yet are listed in `product/console/test/unscoped-reads.txt`, and that list
  may only shrink.
- **The checks.** `task check` runs `tools/check.d/40-console.sh`. That script checks both
  documents and their host components, then runs the type check and the unit tests, builds once,
  and runs Playwright. Playwright loads every page of both apps. It fails on any axe violation
  (WCAG 2.1 A and AA) and on horizontal overflow at 390 px wide. At 1280 px it fails on content
  that is cut off, and on a table that scrolls sideways without a shadow on the hidden side or
  without keyboard focus.
