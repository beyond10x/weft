---
format: aep.planning-md/3
id: epic:agent-workloads
kind: epic
status: draft
title: 'Agent workloads: one agent or an agent architecture, run and rendered by weft'
summary: Agent package with Job, Schedule and Service shapes; the agent units of a cloudspec architecture run on weft; agenticspec renders the architecture in the account console
relations:
- serves: vision:weft
revision: 5
---
# Agent workloads: one agent or an agent architecture, run and rendered by weft

## Order

The owner, 2026-11-28 about 02:08 CET, typed in a session (condensed): weft will be able to run
agents. It might run a whole agent architecture: an architecture from agenticspec is just a
workload kind, a whole spec coming from agenticspec deployed. One agent can be deployed too: a job
as a workload that is an agent, a scheduled agent, a persistent agent. A thing deployed from
agenticspec can also be rendered, and the rendering is provided by agenticspec; there is probably
also a common rendering for the agent itself.

The owner, 2026-11-28 about 02:14 CET (decision D5): cloudspec is the lowest common point, the
intermediate representation. Every source translates into cloudspec, every target is generated out
of it; weft runs only cloudspec specifications.

## Outcome

- One agent is a workload: package kind `Agent` (`ess/4-applications/workloads.yaml:35`, a spec
  kind, `docs/domain-sketch.md:241-243`) with the shapes Job (runs a task to its end), Schedule
  (starts such a Job on a calendar rule) and Service (persistent, N replicas).
- weft runs the agent units of a cloudspec architecture. An agenticspec deployment bundle
  (`agenticspec-deployment/1`, below) reaches weft only as the cloudspec architecture cloudspec
  translated it into; until cloudspec has an architecture format, a weft formation stands in for it.
- The account console shows a deployed agent architecture as agenticspec draws it, and every agent
  workload, single or part of an architecture, as one common agent card with its live state.

## Translation moves to cloudspec

weft has no translator of its own after D5. The translator concept of
`docs/domain-sketch.md:354-366` (registered per source kind and version, deterministic, a coverage
report of mapped, mapped with loss and refused, no grant beyond the source) moves to cloudspec's
epic `translation`; weft's documents say so (`docs/domain-sketch.md`, last section; `README.md`;
`website/docs/concepts/formations.md`, `workloads.md`, `components.md`, `index.md`,
`website/docs/run/deploy-a-workload.md`, `control-host-and-node.md`; `AGENTS.md` § Ecosystem specs).
This epic adds no translator and no source kind to weft.

Specification change for later (not in this epic; `ess/` is not edited now):

- `ess/6-delivery/deployments.yaml`: `SourceKind` (:26-35), `Translator` (:297), `DeploymentSource`
  (:331), `Translation` (:388), `CoverageEntry` (:467) and the commands on them
  (`SubmitDeploymentSource`, `TranslateSource`, `RecordCoverageEntry`, `CompleteTranslation`,
  `RefuseTranslation`, `FailTranslation`): removed, or reduced to a record of cloudspec's
  translation that a plan names.
- `ess/9-actors/actors.yaml:100-101`: `RegisterTranslator`, `RetireTranslator` of
  `ClusterOperator`; the system actor `Translator`.
- `ess/4-applications/workloads.yaml:22-38`: the spec kinds (`EssSystem`, `Agent`, `Training`,
  `Graph`) and the `spec-kind` refusal (:2099), once weft receives only plain components from
  cloudspec.

## The bundle

agenticspec's deployment artifact is `agenticspec-deployment/1` (agenticspec `design/deployment.md`,
branch `design/ingress` at `a47e0c9`, K6): one YAML file with every member inline, each keyed by its
digest: the architecture and nested architectures, the contract, the binding, the packing, the
`units`, ingress and exports, requirements (KPI targets), the agent documents and the runtimes by
OCI digest, and the bundle digest. Unit kinds: `core`, `agent`, `service`, `job`, `scheduled`,
`store`, `model`, `external` (K3; default packing `single`). cloudspec translates it (its epic
`translation`, K13); weft runs the result.

What the bundle carried for weft, checked against K7 at `a47e0c9`, where the earlier draft
(`7d489f0`) lacked it:

| need of weft | at `a47e0c9` |
|---|---|
| resource claims per unit | `claims`: CPU, memory, accelerators. Missing: disk and volume size |
| isolation level per unit | `isolation`: "the least isolation level required". Missing: the mapping to `weft.shared.IsolationLevel` values |
| health checks | `checks`: `startup`, `ready`, `alive`, each with kind and path or command |
| named secrets | `secrets`: every secret by name, never a value |
| egress destinations | `egress`: every destination by name; nothing undeclared granted |
| runtime artifact by digest | `code` and `runtimes`: each runtime by OCI digest |

The two that remain (disk claims; isolation level names) go to agenticspec with
`story:agenticspec-render-contract` and to cloudspec's translator, which has to carry them through.

## Scope

- Specification (planned in stories; `ess/` is written when a story is implemented): the settings
  of the `Agent` package per shape; what weft keeps of an agent architecture (the origin of each
  component, so the console can join the drawing to live state); an event feed with a cursor and
  a view keyed by source part, for cloudspec's observation contract.
- One `Agent` package run as a Job on the `local` placement; Schedule on top of it. The Service
  shape (persistent agent) waits for `story:ports-node-workloads-local`.
- An agent architecture's units run on `local` from a formation that stands in for cloudspec's
  translation of a vendored fixture bundle.
- The account console: the architecture page from agenticspec's renderer and the common agent card.
- A hand-over story to the agenticspec maintainers: a stable render contract and the bundle gaps.
- weft's documents say that translation is cloudspec's (§ Translation moves to cloudspec).

Not in scope: any translator (cloudspec's); agenticspec's packing, admission, contract or
simulation (agenticspec's, design K9); an agent runtime of weft's own (the runtime comes as an
artifact); the specification change of § Translation moves to cloudspec.

## Acceptance

- `ess specify validate --path ess --strict-requires` passes with the `Agent` settings and the
  component origin in `ess/`.
- On the `local` placement an `Agent` package with shape Job runs to its end and its Job reads
  `Succeeded` in `weft.workloads.Jobs`.
- The stand-in formation of a vendored fixture bundle deploys on `local`, and each of its Job and
  Schedule components runs.
- The account console shows the architecture page of that deployed bundle and one agent card per
  agent component, in light and dark, with no axe violation.
