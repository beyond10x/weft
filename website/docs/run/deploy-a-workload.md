---
title: Deploy a workload (Planned)
sidebar_label: Deploy a workload (Planned)
description: Planned - there is no weft CLI or running cloud yet. The intended formation document and deploy flow, as the design sketch describes them.
status: planned
---

:::caution[Planned]
Nothing on this page can be run today. There is no `weft` CLI and no running cloud to deploy to.
The document format and the commands below are the intended design from
[`docs/domain-sketch.md`](https://github.com/beyond10x/weft/blob/main/docs/domain-sketch.md);
they may change before they are built.
:::

## The formation document

A deploy hands weft a formation: its components, the links allowed between them, and its border.
The intended format:

```yaml
formation: shop
components:
  gateway: { package: { kind: image, ref: ghcr.io/acme/gw@sha256:... }, shape: service }
  orders:  { package: { kind: ess-system, ref: ./orders/system.yaml }, shape: service }
  agent:   { package: { kind: agent, ref: ./support-agent.yaml }, shape: service }
  llm:     { package: { kind: model, ref: qwen3-32b }, shape: service,
             claims: { gpu: { model: nvidia-h100, count: 1 } } }
links:
  - gateway -> orders
  - gateway -> agent
  - agent -> orders
  - agent -> llm
border: [gateway]
```

| Part | Says |
|---|---|
| `components` | each component's [package](../concepts/workloads.md#package), [shape](../concepts/workloads.md#shape) and [claims](../concepts/claims.md) |
| `links` | which component may call which; without a link, a component cannot reach another |
| `border` | which components the outside can reach; all others are internal |

`orders` is an ESS system; the plan is to run it with Spindle. `llm` claims one GPU.

## The intended CLI

```console
weft login                      # federated clusters only
weft context use acme/research
weft deploy shop.yaml --env dev # plan, simulate, ask, apply
weft deploy system.yaml         # a single spec becomes a one-component formation
weft plan shop.yaml --env prod
weft invoke shop/orders ...
weft secret set shop/dev STRIPE_KEY
weft logs shop/agent
weft account create acme/research/gpu-lab --quota gpu=4
```

## What a deploy does (specified)

The steps behind `weft deploy` are specified in `weft.deployments` and `weft.formations` and can be
explored with weft-sim today:

1. The document is a **deployment source**. A translator turns it into a **formation revision**
   and a coverage report: every object mapped, mapped with loss, or refused with a reason. A
   refused object stops the deploy.
2. A **plan** shows the difference between the live release and the new revision, with a
   **simulation**: does every instance fit capacity and quota, does every link, egress and secret
   grant pass the account's policy, which component can reach which.
3. A **release** applies the revision to an environment, in ordered steps (pre-install job,
   rollout, post-install job), with a rollout strategy: replace, rolling or blue-green.
4. The scheduler places the instances, the node agents start them, and the release goes live once
   its rollout steps saw ready instances.
5. **Drift** between the live release and what the nodes report is recorded and re-applied or
   escalated.

Besides weft's own document, the specified source kinds are a Docker Compose file, a Helm chart,
Kubernetes manifests, an ESS system and an agent document
([formations](../concepts/formations.md#deployment-sources)).
