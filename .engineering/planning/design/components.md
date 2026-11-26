---
format: aep.planning-md/3
id: design:components
kind: design
status: draft
title: 'weft components: api, control, node, edge and their contracts'
revision: 4
---
## Goal

The owner, 2026-11-26 (relayed by the host operator, about 12:05 CET): "we also need to start defining the
components - this seems to be gotten lost somehow". Every running part of weft named by
`docs/domain-sketch.md` § Structure "Components" (lines 72-78) gets a definition in
`ess/components.yaml` with a contract: the domain it owns, the commands it accepts, the events it
publishes and consumes, the actors it acts as, its channels, and the loop steps it runs.

## Before this design (2026-11-26, commit f0b72781)

- `ess/components.yaml` declares 2 of the 5 components the sketch names: `weft-api` (owns access,
  accounts, artifacts, formations) and `weft-control` (owns every other domain).
- 38 bindings carry the edge steps of loops R1-R8 and the cluster pool loop (sketch § 14).
- The level and time steps (gap view -> command -> actor) exist only as a comment table
  (`ess/components.yaml` about :988-1030). weft-sim runs them from hand-written controllers in
  each experiment, so the controllers are not checked against the spec.
- `weft-node` and `weft-edge` are not declared (`ess/components.yaml:32`, UNMAPPED). The CLI is a
  surface (`surfaces/cli/weft.yaml`), not a component. The control plane's parts (scheduler,
  translators, identity issuer, secret delivery) are actors inside `weft-control`.

## Where it got lost

- Integration wave 3 (`docs/open-questions.md:259`): ESS refuses a component that owns no domain
  (ESS-COMPONENT-007, `docs/ess-gaps.md` gap 1). No domain belonged to the node agent or the
  gateway, so both became "protocol peers" of `protocols/node-channel.yaml` and
  `protocols/exec-channel.yaml`, and the work to give them a domain was not scheduled.
- Two placement OPENs stayed undecided (`ess/components.yaml:18-30`): who serves the user commands
  of control-owned domains (DefineWorkload, CreateSecret, ...), and whether `secrets` and
  `deployments` split between the two server components.

## The components and their contracts

### weft-api (management plane, sketch line 72)

- Owns: access, accounts, artifacts, formations (unchanged).
- Accepts: every user command of those domains; reached by network (`surfaces/http/weft-api.yaml`).
- Acts as: `ManagementApi`; authorizes every management call (caller attributes, pass 1).
- Forwards (D1, the owner 2026-11-26): user commands of control-owned domains to `weft-control`.

### weft-control (control plane, sketch line 73)

- Owns: infrastructure, inventory, cluster, resources, workloads, claims, identity, nodes,
  observability, scheduling, secrets, deployments, pipelines, catalog, metering.
- Acts as: `Scheduler`, `ReleaseController`, `ControlPlane`, `Translator`.
- Runs: every edge step as an ESS binding (as today), and every level and time step as a declared
  step `{form: level|time, view, command, actor}`, replacing the comment table. Where ESS cannot hold
  a declared step, it is the ESS ask below.
- Channels: terminates the node session (`protocols/node-channel.yaml`); stamps the receive time on
  node frames (the stamped-time rule, `harden/stamped-time`).

### weft-node (worker plane, sketch line 74): new domain `weft.node`

- Owns a node-local domain in layer 5 (`ess/5-runtime/node.yaml`), state the control plane sees
  through observations only:
  - `LocalProcess`: a process of an instance on this node, its restarts and backoff (loop R9,
    sketch § 14 line 593, today a comment at `ess/components.yaml` about :1027).
  - `LocalEndpointSession`: an instance's call on its local endpoint for its identity document and
    secrets (sketch line 100), checked against the assignment the node holds.
- Accepts: commands of `NodeAgent` about its own processes, and of `Instance` on its local endpoint.
- Publishes: process observations (`ProcessRestarted`, `ProcessBackingOff`, `AliveCheckFailed`);
  edge bindings carry them to `weft-control` (`nodes`/`workloads` `Record*`), which stamps them.
- Consumes: assignments and their ends from `nodes` (R4, R2, R3) over the node channel.
- Contract test: `protocols/node-channel.yaml` is the wire; the domain is the node's state machine.

### weft-edge (data plane, sketch line 75): new domain `weft.edge`

- Owns a domain in layer 5 (`ess/5-runtime/edge.yaml`):
  - `RouteBackends`: the ready instances behind each route of a formation (sketch § 15: readiness
    gates traffic).
  - `HeldRequest`: a request held while a sleeping service wakes (loop R8, sketch § 4 "Sleep and wake").
  - `EdgeCertificate`: the certificate of a public endpoint (sketch § 14 OPEN).
- Consumes: formations route and endpoint events; workloads readiness events.
- Publishes: `WakeRequested` (R8), `BackendSetChanged`.
- Accepts: no user command; its rows change only through bindings and its own actor `EdgeGateway`.

### weft CLI (sketch line 76)

- Stays a client surface (`surfaces/cli/weft.yaml`, `ess-cli/2`) of `weft-api`; no component.

## Steps

1. Declare `weft.node` and `weft.edge` with the entities above (one domain file each, layer 5,
   `tools/check-layers.sh`), the two components, their bindings, and the actors `EdgeGateway` (new)
   and `NodeAgent`/`Instance` grants moved to the new domain.
2. Replace the level/time comment table with declared steps (needs the ESS ask, or one interim form
   ESS 0.99.2 accepts).
3. weft-sim reads the declared steps instead of each experiment's hand-written controllers; an
   experiment may still override one for a fault.
4. Settle D1 and D2 in `ess/components.yaml` and `surfaces/http/weft-api.yaml`.

## ESS ask (one line, to the host operator)

A component declares level and time steps (`steps: [{form: level|time, view, command, actor, every}]`)
that `validate` checks (view and command exist, the actor may run the command and read the view) and
the interpreter can run.

## Decisions (the owner, 2026-11-26 about 12:20 CET, relayed by the host operator)

- D1, user commands of control-owned domains (DefineWorkload, CreateSecret, SubmitDeploymentSource):
  The owner picked "A: weft-api forwards". `weft-api` forwards them to `weft-control` over an internal
  HTTP binding; the user sees one API, one owner per domain stays. Not taken: B, move those domains
  to `weft-api` (node reports would land on `weft-api`, against sketch line 96).
- D2, `secrets` and `deployments`: the owner picked "A: whole in weft-control". No domain split; with D1
  the user reaches them through `weft-api`. Not taken: B, split custody and sources (weft-api) from
  delivery, translation and releases (weft-control).

## Open questions (default taken until answered)

1. Does `weft-node` own the local endpoint's secret delivery, or only relay `secrets.DeliverSecret`?
   Default: relay; `weft-control` stays the owner of `secrets`.
2. Does `weft-edge` keep its own copy of route desired state? Default: no; it reads formations events.
3. In the `local` placement all four server components run in one process (sketch line 78): one
   deployment unit, four components in the model. Default: yes.

## Specified (2026-11-26)

- Step 1: `ess/5-runtime/node.yaml` (domain `weft.node`, component `weft-node`): `LocalProcess` and
  `LocalEndpointSession`, 9 commands, 9 events, 4 views; R9 as edge `r9-mark-restart-due`
  (`workloads.AliveCheckFailed` -> `MarkRestartDue`), node level step `RestartsDue` ->
  `RestartProcess` (NodeAgent) and edge `r9-record-process-restarted` (-> `workloads.RecordProcessRestarted`,
  no longer granted to NodeAgent). Assignment ends stop processes and close endpoint sessions (4
  bindings). No time input from the node.
- Step 1: `ess/5-runtime/edge.yaml` (domain `weft.edge`, component `weft-edge`): `RouteBackend` (view
  `RouteBackends`: the ready instances of a route), `HeldRequest`, `EdgeCertificate`; 13 commands, 10
  events, 5 views; actor `EdgeGateway`. 13 bindings: workloads readiness events keep the backends,
  `r8-edge-wake` (`WakeRequested` -> `workloads.WakeService`, source `EdgeRequest`), `r8-edge-forward`
  (`BackendSetChanged`), formations route and endpoint events; edge time step
  `HeldRequestsPastDeadline` -> `MissHeldRequest`.
- Step 4: D1 and D2 in the header of `ess/components.yaml`; D1 is `weft-control` `reached_by:
  network` plus an OPEN naming what ESS lacks (a component that forwards commands another component
  accepts). No `ess-http/1` document: `surfaces/http/` does not exist yet.
- Gate: `slot task check` green; weft-sim experiments all exit 0; explore from the full setup (256
  walks): 0 violations, no Unsupported answer in the new domains.
- Not run by synthesis (`docs/ess-gaps.md` gap 24).
