---
format: aep.planning-md/3
id: story:contract-providers
kind: story
status: draft
title: 'weft provides cloudspec''s managed-service contracts: engines name the contracts they fulfil, a runtime binding picks one, memory binds all'
relations:
- decomposes: epic:agent-workloads
- serves: vision:weft
- depends_on: story:deployments-admit
revision: 2
---
## Epic

Decomposes `epic:agent-workloads`: an agent architecture that needs a store gets one from weft.

## Order

The owner, 2026-11-28 about 03:33 CET, typed in this session: a part that needs a key-value store
(or a queue, pub/sub, any managed service) becomes, in cloudspec, a need for that managed service.
It points at a service that is not there yet: unmapped at first. cloudspec keeps it at the
specification level ("I need a key-value store; not Redis; NATS yes") and does not turn it into an
engine while translating. Which engine serves it (Valkey, NATS key-value, ...) is the configuration
of the runtime that runs the architecture, weft: a binding. weft can hold several engines per
contract. "During testing it would just bind it to [...] memory."

## Context

- cloudspec owns the contracts (D6 A): `key_value_store`, `record_store`, `queue`,
  `artifact_store`, `semantic_store`, `graph_store`, `sip_registrar`, `sip_outbound_proxy`, each
  with operations, guarantees and KPIs (cloudspec `docs/layers.md` layer 1). A binding (layer 10)
  maps a role or contract need to a runtime or service, with admission.
- weft already models the provider side: `weft.catalog.Engine` ("a versioned implementation behind
  one or more offering kinds", `ess/7-services/catalog.yaml:95`), `weft.catalog.Offering` and
  `OfferingPlan`, and service instances and service bindings. `OfferingKind` (`:50`) has `kv`,
  `queue`, `stream`, `sql`, `vector` and others, named by weft, not by cloudspec's contracts.
- Placement `memory` (`story:ports-memory-placement`) serves every port in one process.

## Outcome

1. Each `Engine` declares the cloudspec contracts it fulfils and the guarantees it meets for each
   (durability, consistency, ordering, delivery), by contract name and cloudspec version.
2. A runtime binding (weft configuration, per deployment or per cloud) maps each contract need of
   an architecture to an offering; the architecture's engine constraints (an engine named as
   required or refused) and its guarantee floor are checked against the engine.
3. Admission (`story:deployments-admit`) reports each contract need with no binding as Refused
   naming the contract and the part that needs it; nothing starts.
4. On placement `memory` every contract need binds to an in-process provider by default, so a test
   or a simulation needs no binding file.

## Scope

- `ess/7-services/catalog.yaml` (contracts on `Engine`), `ess/6-delivery/deployments.yaml` (the
  binding and its admission refusals), `docs/domain-sketch.md` § Services

## Acceptance

1. `ess specify validate --path ess --strict-requires` passes with an `Engine` that names the
   contract `key_value_store` it fulfils.
2. A scenario admits a formation with one `key_value_store` need bound to an offering whose engine
   fulfils it.
3. A scenario refuses the same formation with no binding, naming `key_value_store` and the part.
4. A scenario refuses a binding to an engine the formation's constraints refuse, naming the engine.

## Dependencies outside weft

- cloudspec stories contract-needs (constraints and guarantee floor), admission-contract-needs
  (unbound needs, refused engines and floors, by name), memory-binds-contracts and contract-pubsub;
  contracts to cite: record_store, key_value_store, artifact_store, semantic_store, graph_store,
  queue, pubsub, sip_registrar, sip_outbound_proxy (cloudspec `docs/layers.md` layer 1), at the
  merge sha of cloudspec#4. Earlier request:
  layer 10 binding); requested from the cloudspec session on 2026-11-28.

## Rules for this work

- Builds and tests run through `slot`. No test over 60 s; the test step ends within 120 s. A pool
  job ends within 10 minutes unless it names its time with `slot --timeout` (at most 30m). No
  Python in anything committed.
