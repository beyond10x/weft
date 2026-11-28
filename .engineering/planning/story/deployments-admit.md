---
format: aep.planning-md/3
id: story:deployments-admit
kind: story
status: draft
title: 'Spec: a dry-run admit in deployments answering a plan or every violation and missing port'
relations:
- decomposes: epic:platform-ports
- serves: vision:weft
revision: 2
---
## Epic

Decomposes `epic:platform-ports`: a specification change. A target admits a deployment before it
runs it: agenticspec's deployment design asks the target (weft) to admit quota, placement
feasibility (isolation level, accelerators), the account's secret and egress policy, refused as
`TargetRefused` with the named violations (agenticspec `design/deployment.md`, branch
`design/ingress` at `a47e0c9`, K14). `weft run` uses it (`story:weft-run-ephemeral`).

## Context

- Today a check without running exists only inside a plan: `weft.deployments.CreatePlan`
  (`ess/6-delivery/deployments.yaml:1567`) needs a translation, and its `Simulation` (lines
  273-280) checks placement, policy and reachability with the violations of `ViolationName`
  (lines 233-245). It does not check the ports the placement serves.
- The ports a placement serves: `cloudspec@2ed72c2:docs/platform-ports.md` § 8.

## The specification change

- `ess/6-delivery/deployments.yaml`: a command `weft.deployments.AdmitArchitecture` (dry run, no
  row written) with input the formation document's digest, the account and the environment, and
  outcomes: `admitted` answering a plan preview (the simulation's reachability and an empty
  violation list), or `refused` answering every violation and the list of missing ports.
- Checks, each from its existing record: capacity (`weft.infrastructure.Nodes.capacity`), isolation
  facts (`Nodes.isolation_facts`), the quota chain (`weft.accounts` quotas up the account tree),
  catalog offerings (`weft.catalog` offerings for each managed-service binding), and the ports the
  cloud's placement serves (`weft.infrastructure.Clouds.placement` against the port table).
- `ViolationName` gains `PortNotServed` and `OfferingMissing`; `Violation.subject` names the port
  or the offering.
- The actor: `Deployer` and `Automation` (`ess/9-actors/actors.yaml`).

## Scope

- `ess/6-delivery/deployments.yaml`, `ess/9-actors/actors.yaml`, `docs/domain-sketch.md` § 5

## Acceptance

`ess specify validate --path ess --strict-requires` passes with `AdmitArchitecture`, and
`tools/check-spec-diff.sh --acknowledge` records the change; a scenario admits a formation that
fits and refuses one that claims an accelerator no node has, with `NoMatchingResource`.
