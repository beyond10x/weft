---
format: aep.planning-md/3
id: story:docs-status-proofs
kind: story
status: active
title: Status page rendered from status.json, every shipped item proved by a file or a command in the docs check
relations:
- decomposes: epic:docs-fidelity
- serves: vision:weft
- depends_on: story:docs-claims-check
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: Taskfile.yml
- confidence: inferred
  path: tools/docs-check-fixture/40-status-claims
- confidence: inferred
  path: tools/docs-check.d/40-status-claims.sh
- confidence: cited
  path: tools/docs-check.d/README.md
- confidence: cited
  path: tools/test-docs-check.sh
- confidence: inferred
  path: website/data/status-proofs.json
- confidence: cited
  path: website/data/status.json
- confidence: cited
  path: website/docs/status.md
- confidence: cited
  path: website/product.json
revision: 12
transitions:
- {from: "draft", to: "proposed", at: "2026-11-27T16:03:33Z", actor: "human:operator", revision: 6, decided_on: {"recorded":{"approval":1,"review_outcome":1}}}
- {from: "proposed", to: "active", at: "2026-11-27T16:03:33Z", actor: "human:operator", revision: 7, decided_on: {"recorded":{"approval":1,"review_outcome":1}}}
---
## Epic

Decomposes `epic:docs-fidelity`, outcome 2 and the KPI `docs.status-claims-false` of outcome 1.
Depends on `story:docs-claims-check` (its hook directory, fixture runner, task and `pages.yml` step).

## Context

`website/docs/status.md` is a hand-written table that repeats `website/data/status.json` row by row,
with a hand-written "Checked against `main` on 2026-11-27". The landing page reads the JSON; the
status page does not. Nothing checks that a `shipped` item holds on the tree
(`design:quality-kpis`: `docs.status-claims-false`, "no checker; the 23-domain and 4-component
claims were checked by hand and hold").

- The status page renders the file: `StatusTable` of `@beyond10x/docs-system` (exported at the
  pinned revision `081e05f`, `src/product.tsx`), and its checked-on date is the `asOf` of the file.
- Proofs live beside the file, not in it: the `b10x-status/1` schema
  (`schema/b10x.status.v1.schema.json` of `@beyond10x/docs-system` at `081e05f`) sets
  `additionalProperties: false` on an item, so a proof field would make `status.json` invalid.
  `website/data/status-proofs.json` maps an item `label` (unique within the file by that schema) to
  its proofs. A proof is `{"file": "<path>"}` (the path exists) or
  `{"command": "<sh>", "expect": "<stdout>"}` (run with `sh -c` from the repository root with a
  10 s limit, exits 0, and prints `expect` when given). Commands use sh, grep, sed, awk, find and jq
  only, so they run in `pages.yml` without `ess` or cargo.
- Counts: the sidecar also names the counts the site states (domains, layers, components, files of
  `ess/ess-inputs.yaml`), each with the command that computes it. Every occurrence of
  `<n> domains`, `<n> layers` and `<n> components` in `website/docs`, `website/product.json` and
  `website/data/status.json` must equal the computed value.
- Hook `tools/docs-check.d/40-status-claims.sh` counts as a false claim: a `shipped` item without a
  proof, a proof that fails, a proof label that names no item, and a stated count that differs from
  the computed one. It prints `docs.status-claims-false <n>`.

## Domain relations

- Status item -> proof: one item has one or more proofs when `shipped`, none required when
  `planned` or `decided`; a proof belongs to one item through its `label` and goes with it.
  Inferable: `epic:docs-fidelity` outcome 2 ("`status.json` items name a file or a command that
  proves them"); the proof lives in a sidecar because the item cannot carry it, inferred from
  `schema/b10x.status.v1.schema.json` (`additionalProperties: false`) of `@beyond10x/docs-system`
  at `081e05f`. No `ess/1` document declares this relation; it is not a domain of weft.

## Scope

- `website/data/status-proofs.json` (new), `website/data/status.json` (`source` names the sidecar
  and the hook)
- `website/docs/status.md` (renders `StatusTable`; no hand-written table or date)
- `tools/docs-check.d/40-status-claims.sh`, `tools/docs-check-fixture/40-status-claims/`

## Acceptance

1. The built `/docs/status` page (`website/build/docs/status/index.html` after
   `slot npm --prefix website run build`) lists every item of `website/data/status.json` with its
   status and detail.
2. The checked-on date the built `/docs/status` page shows equals `jq -r .asOf
   website/data/status.json`.
3. `website/docs/status.md` contains no table of its own (`grep -c '^|' website/docs/status.md`
   prints 0).
4. Every `shipped` item of `website/data/status.json` has at least one proof in
   `website/data/status-proofs.json`, and `tools/check-docs.sh` on the tree prints
   `docs.status-claims-false 0`.
5. `tools/test-docs-check.sh` passes only when `40-status-claims.sh` exits 1 on a fixture with a
   shipped item without a proof, on one with a proof that fails, and on one whose page says
   "24 domains" against 23 computed, and exits 0 on its `pass` fixture.

## Rules for this work

- POSIX sh and JSON; no Python in anything committed.
- The hook and all proofs together end within 10 s; no network.
- No claim on the site that the tree does not prove; no person, host, session or private
  repository name.
- `slot npm --prefix website run build` (with `slot --timeout 15m npm ci` first) proves the page
  renders; a pool job ends within 10 minutes.
