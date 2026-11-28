---
title: Read the specification
description: Where weft's ESS specification lives, how a domain file is laid out, and how to read an entity, a command, an actor and a binding.
---

weft's specification is a set of [ESS](https://beyond10x.github.io/ess/) documents in the
repository. It is the source every planned implementation is held to.

## The files

| Path | What |
|---|---|
| `ess/ess-inputs.yaml` | The list of files that form the system, and the `ess` version it requires (0.99.2). |
| `ess/system.yaml` | The system `weft`, version `v1`, and its 23 domains. |
| `ess/<n>-<layer>/<domain>.yaml` | One domain per file, one directory per [layer](layers.md). |
| `ess/9-actors/actors.yaml` | Every actor and the commands and views it may use. No other file declares actors. |
| `ess/components.yaml` | The four components, which domains each owns, and the reconciliation bindings. |
| `protocols/node-channel.yaml` | The session between `weft-control` and `weft-node` (`ess-protospec/1`). |
| `protocols/exec-channel.yaml` | Exec, attach, logs and port forward into an instance (`ess-protospec/1`). |
| `surfaces/http/`, `surfaces/cli/` | Planned: the user API (`ess-http/1`) and the CLI (`ess-cli/2`). Not written yet. |

The design every file follows is
[`docs/domain-sketch.md`](https://github.com/beyond10x/weft/blob/main/docs/domain-sketch.md).

## A domain file

A domain file declares, in this order: `domain`, `summary`, `naming`, `types`, `entities`,
`errors`, `events`, `commands`, `views` (a domain leaves out what it has none of). Comments name the line of the domain sketch each
declaration comes from. A question the sketch does not decide is written as `# OPEN:` with the
answer taken for now:

```yaml
# OPEN: the sketch gives a tenant no lifecycle. Taken: Active, then Closed once no account of
# it is open. Suspending a tenant is left out.
```

`grep -rn 'OPEN:' ess` lists every open question.

## An entity

An entity has an identity, fields, relations and a lifecycle. From `ess/2-tenancy/accounts.yaml`:

```yaml
- name: weft.accounts.Tenant
  naming:
    summary: A top-level customer of the cloud. It owns everything below it.
  identity:
    name: tenant_id
    type: weft.shared.TenantId
  fields:
    - name: name
      type: weft.accounts.Name
  relations:
    - name: accounts
      kind: owns
      target: weft.accounts.Account
      cardinality: many
      via: tenant_id
  lifecycle:
    initial: Active
    states: [Active, Closed]
    terminal: [Closed]
    transitions:
      - name: close
        from: [Active]
        to: Closed
```

Every entity identifier, such as `weft.shared.TenantId`, is declared in
`ess/0-foundation/shared.yaml`. A domain refers to another domain's entity through that identifier
and declares the relation on its own entity.

## A command

A command has inputs and outcomes. Each outcome is one answer: a refusal with an error, or an
accepting outcome with its effect and the events it emits.

```yaml
- name: weft.accounts.CreateTenant
  naming:
    display: Create a tenant
  input:
    - {name: name, type: weft.accounts.Name}
  outcomes:
    - name: name-taken
      when_related:
        entity: weft.accounts.Tenant
        where: name == input.name
        exists: true
      error: weft.accounts.TenantNameTaken
    - name: created
      creates: weft.accounts.Tenant
      instance: tenant_id
      sets:
        name: input.name
      emits: [weft.accounts.TenantCreated]
```

`name-taken` refuses when a tenant of that name exists; otherwise `created` creates the tenant
and emits `TenantCreated`. Which outcome answers a request is fixed by ESS: see the ESS guide
[guards and predicates](https://beyond10x.github.io/ess/docs/guides/specify/guards-and-predicates),
section "Which branch answers".

To see a command's inputs, outcomes and actors without reading YAML, use
`cloudspec-sim describe` ([install cloudspec-sim](../simulator/install.md)):

```shell-session
$ cloudspec-sim describe --spec ess --prefix weft.accounts.CreateTenant
```

## Actors

`ess/9-actors/actors.yaml` lists each actor with `may:`, the commands it may run and the views it
may read. A command no actor may run cannot be called. Isolation, secrets and identity default to
deny: a guard refuses, never a silent fallback.

## Components and bindings

`ess/components.yaml` places each domain in exactly one component, which accepts all its commands
and publishes all its events:

| Component | Domains |
|---|---|
| `weft-api` | `access`, `accounts`, `artifacts`, `formations` |
| `weft-control` | `infrastructure`, `inventory`, `cluster`, `resources`, `workloads`, `claims`, `scheduling`, `nodes`, `identity`, `secrets`, `observability`, `deployments`, `pipelines`, `catalog`, `metering` |
| `weft-node` | `node` |
| `weft-edge` | `edge` |

`weft.shared` (types only) and `weft.actors` (actors only) belong to no component. `weft-api`
forwards the user commands of `weft-control`'s domains, so a user sees one API.

The `bindings:` section holds the reconciliation loops: an event or a view invokes a command, with
a delivery mode and what happens on failure (`retry`, `drop`, or `escalate`). See
[nodes and clusters](../concepts/nodes-and-clusters.md#reconciliation).
