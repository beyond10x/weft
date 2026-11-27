# weft: rules for agent sessions in this repository

These add to `~/beyond10x/AGENTS.md` and do not loosen it.

## What this repository is

weft is a compute cloud a team deploys itself: a control plane and worker nodes that run confined
workloads (code, images, boxes, models, Compose files, Helm charts, ESS systems, Loom agents) with claims on GPUs, disks,
network and secrets, for tenants and their nested accounts. The design is
[`docs/domain-sketch.md`](docs/domain-sketch.md). It is specification first: no code exists, and
none is written before the specification of what it implements.

## Layout

- `product/<product>/crates/<crate>`: the Rust crates of the root workspace. `product/cloud/` is
  weft's cloud (its operator CLI included); a consumer app, such as the user's management CLI,
  gets `product/<app>/`.
- `target/generated/`: the code ESS synthesizes from `ess/`, not committed; `tools/synth.sh`
  writes `target/generated/`, cached by the digest of its sources. Crates use it through path dependencies, so a cargo
  command in the root workspace fails with "failed to read
  `.../target/generated/crates/<crate>/Cargo.toml`" until `tools/synth.sh` has run once;
  `task test` runs it first.
- `ess/`: the ESS domain model. `system.yaml`, `ess-inputs.yaml`, `components.yaml`,
  `<layer>/<name>.yaml` (one bounded context per file, one directory per layer, `docs/layers.md`).
  A domain names only domains of its own or a lower layer. Between domains it declares the
  relation (`references`), from the integration pass on (2026-11-26).
- `protocols/node-channel.yaml`: session between `weft-control` and `weft-node`, `ess-protospec/1`.
- `protocols/exec-channel.yaml`: exec, attach, logs and port forward into an instance,
  `ess-protospec/1`.
- `tools/check-domain.sh <name>`: validates one domain with `weft.shared` only; fails for a domain with relations to other domains (use check-overlay).
- `tools/check-overlay.sh`: validates the whole system from a frozen snapshot with only named
  files from the working tree, for parallel edits (`--freeze` takes the snapshot).
- `tools/check-layers.sh [--graph]`: fails on a reference to a higher layer (`task layers`).
- `tools/check-stamped-time.sh [--inventory | --negative]`: fails when a command with a
  `weft.shared.StampedAt` input is granted outside `tools/stamped-time-actors` (`task stamped-time`).
- `tools/check-spec-diff.sh [--acknowledge | --negative]`: classifies the change of `ess/` since
  the newest release tag (`ess verify diff --fail-on breaking-or-unknown`) and fails on a breaking
  or unknown change that the acknowledgements file does not name; `--acknowledge` writes
  `quality/spec-diff-acknowledgements.json` (and removes it when there is nothing to
  acknowledge). A change to `ess/` makes that file stale: review the change, then acknowledge
  again. Run by
  `tools/check.d/50-spec-diff` in `task check` (hardening pass 3).
- `tools/sim-relations.sh [spec dir]` (`task sim-relations`): the metamorphic relations of
  hardening pass 3, `weft-sim relate` from the full setup; steps in `sim/relations/`. Measures, so
  it runs without `slot`.
- `tools/kpis.sh` (`task kpis`): prints the KPIs of `design:quality-kpis` measured on this
  machine, one line each with its value, unit and command, from the hooks of `tools/kpis.d/`
  (contract in its `README.md`). Measures, so it runs without `slot`; its hooks put the prefix on
  the builds and tests inside them. Never part of `task check` (test
  `ratchet::check_runs_no_kpi_hook`).
- `quality/ratchet.yaml`: the KPIs as a `ratchet/1` plan, with assessor declarations in
  `quality/assessors/` and accepted values in `ratchet/references/<requirement>.json`
  (`ratchet-reference/1`, written by hand until `ratchet reference adopt` runs). The test
  `ratchet::the_ratchet_file_is_consistent` in `task sim-test` holds the three together; a new KPI
  adds a measurement, its requirement, its record when it is a ratchet, and a test of its own in
  `tools/sim/tests/ratchet/mod.rs`.
- `tools/synth-domain.sh [--snapshot] <name> [paths]`: synthesizes the conformance suite of one
  domain on the subset of domains it needs (about 80 s; the whole system takes 451 s) and runs it
  against the interpreted target. Synthesis goes through `slot` with `--jobs 2`, one run at a time
  (2026-11-26: a synthesis held 22 GB and the host ran out of memory).
- `docs/domain-sketch.md`: the design every specification file follows.
- `website/`: the public documentation site (Docusaurus 3.10.2, `@beyond10x/docs-system`, setup
  copied from spindle), published at `https://beyond10x.github.io/weft/` by
  `.github/workflows/pages.yml` (builds on pull requests and `main`, uploads on `main`) and
  `b10x-docs-site.yml` (deploys). Build: `slot --timeout 15m npm ci` then `slot npm run build`
  inside `website/`. The site is public: it names no person, session, private repository or
  host. A page about something not built carries a `:::caution[Planned]` banner.
  `tools/planning-only.sh` lets a `website/`-only change skip the code gate; `--docs` is the
  docs build's filter, where a `website/` path builds.
- `tools/check-docs.sh` (`task docs-check`, part of `task check`, and the step after the
  checkout in `.github/workflows/pages.yml`, which runs on every pull request): holds
  `docs.dead-repo-links`, `docs.planned-without-banner`, `repo.agents-md-dead-paths` and
  `docs.status-claims-false` at 0 with the hooks of `tools/docs-check.d/` (contract in its
  `README.md`); every hook runs and prints `<kpi-id> <count>` last. Every `shipped` item of
  `website/data/status.json` and every `shipped` feature of `website/product.json` names a file
  or a command that proves it in
  `website/data/status-proofs.json`, which also computes the counts the site states (domains,
  layers, components, specification files); `website/docs/status.md` renders `status.json` and
  has no table of its own. `tools/test-docs-check.sh` runs every hook on its `pass` and
  `fail` trees in `tools/docs-check-fixture/`. A path named in this file is committed (or new
  and not ignored), is written by a tool its bullet names (`tools/synth.sh` writes
  `target/generated/`), or stands under a heading that says Planned.

### Planned: named here, not in the tree yet

- `surfaces/http/weft-api.yaml`: the user API, `ess-http/1`. No document exists yet; the site
  says so on `/docs/specification/read#the-files`.
- `surfaces/cli/weft.yaml`: the CLI presentation, `ess-cli/2`. No document exists yet.
- `ess/scenarios/`: the ESS scenarios of the domain model. None is written
  (`scenarios: []` in `ess/ess-inputs.yaml`).

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

## Gates and bot commits

weft is enrolled in the organization's Gates policy (gates-policy PR 9, baseline `fc11f840`, the
owner's order of 2026-11-26). Every commit after the baseline is a bot commit; the hooks
(`b10x-gates install`) and the `Shared source gates` workflow refuse any other author.

- Commit: `b10x-gates bot --repository beyond10x/weft --policy ~/beyond10x/gates-policy/policy.json -- commit -F <file>`.
- Push: the same with `-- push …`. Pull requests: `b10x-gates api --method POST --path /repos/beyond10x/weft/pulls …`.
- `cherry-pick` and `rebase` re-create commits under another author: use `merge` or a new bot commit.

## Ecosystem specs

Other specification formats of the ecosystem enter only as deployable workloads, through a
translator of the `deployments` domain: an ESS `system.yaml`, a `loom.agent/1` document, a
Docker Compose file, a Helm chart, Kubernetes manifests, later a ModelSpec program. weft's own
domains reuse none of them.
