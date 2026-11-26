# weft: rules for agent sessions in this repository

These add to `~/beyond10x/AGENTS.md` and do not loosen it.

## What this repository is

weft is a compute cloud a team deploys itself: a control plane and worker nodes that run confined
workloads (code, images, boxes, models, Compose files, Helm charts, ESS systems, Loom agents) with claims on GPUs, disks,
network and secrets, for tenants and their nested accounts. The design is
[`docs/domain-sketch.md`](docs/domain-sketch.md). It is specification first: no code exists, and
none is written before the specification of what it implements.

## Layout

- `ess/`: the ESS domain model. `system.yaml`, `ess-inputs.yaml`, `components.yaml`,
  `<layer>/<name>.yaml` (one bounded context per file, one directory per layer, `docs/layers.md`),
  `scenarios/`. A domain names only domains of its own or a lower layer. Between domains it
  declares the relation (`references`), from the integration pass on (2026-11-26).
- `surfaces/http/weft-api.yaml`: the user API, `ess-http/1`.
- `surfaces/cli/weft.yaml`: the CLI presentation, `ess-cli/2`.
- `protocols/node-channel.yaml`: session between `weft-control` and `weft-node`, `ess-protospec/1`.
- `protocols/exec-channel.yaml`: exec, attach, logs and port forward into an instance,
  `ess-protospec/1`.
- `tools/check-domain.sh <name>`: validates one domain with `weft.shared` only; fails for a domain with relations to other domains (use check-overlay).
- `tools/check-overlay.sh`: validates the whole system from a frozen snapshot with only named
  files from the working tree, for parallel edits (`--freeze` takes the snapshot).
- `tools/check-layers.sh [--graph]`: fails on a reference to a higher layer (`task layers`).
- `tools/check-stamped-time.sh [--inventory | --negative]`: fails when a command with a
  `weft.shared.StampedAt` input is granted outside `tools/stamped-time-actors` (`task stamped-time`).
- `tools/synth-domain.sh [--snapshot] <name> [paths]`: synthesizes the conformance suite of one
  domain on the subset of domains it needs (about 80 s; the whole system takes 451 s) and runs it
  against the interpreted target. Synthesis goes through `slot` with `--jobs 2`, one run at a time
  (2026-11-26: a synthesis held 22 GB and the host ran out of memory).
- `docs/domain-sketch.md`: the design every specification file follows.

## Rules for the specification

- One domain file has one owner at a time. A session that writes `ess/<layer>/<name>.yaml` edits
  no other domain file.
- `ess/0-foundation/shared.yaml` holds every entity identifier and the value types more than one domain
  uses. A domain refers to another domain's entity through its identifier there, as a field, and
  declares the relation (`references`) on its own entity; the target is in its own or a lower
  layer (`tools/check-layers.sh`).
- Actors live only in `ess/9-actors/actors.yaml` (domain `weft.actors`); no other domain file has
  an `actors:` block. Components and bindings (the reconciliation loops, sketch § 14) live only in
  `ess/components.yaml`.
- A domain that needs a new shared identifier or type reports it; the integrator adds it.
- Every command, view and entity names the line of `docs/domain-sketch.md` it comes from in a
  comment. What the sketch does not decide is written as `# OPEN:` with the question.
- Isolation, secrets and identity default to deny: a guard refuses, never a silent fallback.
- A time weft-control decides on from its own clock (a receipt, a decision, its clock plus a
  length) is a command input of type `weft.shared.StampedAt`, and only the actors in
  `tools/stamped-time-actors` (weft-control's: ControlPlane, Scheduler, ReleaseController) are
  granted such a command; a node's or a user's request carries no such time, and an instant a
  user chooses stays `Timestamp`. `tools/check-stamped-time.sh` holds it in `task check` (a node
  that names its own `received_at` postpones its own loss, hardening pass 5).
- `ess specify validate --path ess --strict-requires` is the gate, run as `slot task check` (the
  slot hook refuses a bare `task check`). `tools/check-domain.sh` calls `ess` directly and runs
  without `slot`.
- No Python in anything committed (`~/beyond10x/AGENTS.md`). Tools are POSIX sh or Rust.

## Ecosystem specs

Other specification formats of the ecosystem enter only as deployable workloads, through a
translator of the `deployments` domain: an ESS `system.yaml`, a `loom.agent/1` document, a
Docker Compose file, a Helm chart, Kubernetes manifests, later a ModelSpec program. weft's own
domains reuse none of them.
