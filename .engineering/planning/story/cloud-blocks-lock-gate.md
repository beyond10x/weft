---
format: aep.planning-md/3
id: story:cloud-blocks-lock-gate
kind: story
status: active
title: The gate refuses a vendored cloudspec copy that differs from its lock or its pin, with no network
relations:
- decomposes: epic:cloud-blocks
- serves: vision:weft
- depends_on: story:cloud-blocks-vendor
scope:
- confidence: cited
  path: .github/workflows/gate.yml
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: Taskfile.yml
- confidence: cited
  path: tools/check-vendor-lock.sh
- confidence: cited
  path: tools/check.d/15-vendor-lock.sh
- confidence: cited
  path: tools/fixtures/vendor-lock
- confidence: cited
  path: tools/slow.d/20-vendor-fresh.sh
- confidence: cited
  path: tools/test-vendor-lock.sh
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-28T08:38:21Z", actor: "human:operator", revision: 4, decided_on: {"recorded":{"approval":1}}}
- {from: "proposed", to: "active", at: "2026-11-28T08:40:12Z", actor: "human:operator", revision: 5, decided_on: {"recorded":{"approval":1}}}
---
## Epic

Decomposes `epic:cloud-blocks`: the vendored copy cannot drift from its pin unnoticed.

## Outcome

1. `tools/check-vendor-lock.sh` (POSIX sh, no git, no network) checks `ess/cloud/cloud.lock`: its
   format is `cloudspec-vendor/2` (with `exclude` lines) and its system `weft`; its `commit`
   equals `CLOUDSPEC_REV` in `Taskfile.yml`; every `output` digest matches the file; no YAML
   under `ess/cloud/` that the lock does not list; every `output` is listed in
   `ess/ess-inputs.yaml`.
2. It runs in `task check` (`tools/check.d/15-vendor-lock.sh`) with a test of each refusal
   (`tools/test-vendor-lock.sh` and a fixture).
3. The slow lane (`tools/slow.d/20-vendor-fresh.sh`) fetches cloudspec at `CLOUDSPEC_REV` into
   the build directory and runs its `scripts/vendor.sh --check ess/cloud`; it runs when
   `ess/cloud/**` or `CLOUDSPEC_REV` changed. CI job `vendor-fresh` in `gate.yml` does the same.
4. `task vendor-update REV=<sha>` fetches, runs `vendor.sh --ref <sha> weft ess/cloud` with the
   excludes of D-a, and sets `CLOUDSPEC_REV`. This story adds the task; it does not change the
   pin (`story:cloud-blocks-vendor` sets it).

## Scope

`tools/check-vendor-lock.sh`, `tools/check.d/15-vendor-lock.sh`, `tools/test-vendor-lock.sh`,
`tools/fixtures/vendor-lock/`, `tools/slow.d/20-vendor-fresh.sh`, `Taskfile.yml` (task
`vendor-update` only), `.github/workflows/gate.yml`, `AGENTS.md` (a new section "Vendored
blocks"; `story:cloud-blocks-layers` edits the layer section after this story).

## Acceptance

1. `tools/test-vendor-lock.sh` exits 0 and shows the check exiting 1 on each of: a format other
   than `cloudspec-vendor/2`, a system other than `weft`, a lock `commit` that differs from
   `CLOUDSPEC_REV`, one changed byte in a listed file, an unlisted YAML file, an `output` missing
   from `ess-inputs.yaml`.
2. `tools/check-vendor-lock.sh` exits 0 on the tree of `story:cloud-blocks-vendor` with
   networking off (`unshare -rn`).
3. `tools/slow.d/20-vendor-fresh.sh` exits 0 on that tree, and 1 after one byte of an
   `ess/cloud/` file changes.
4. `task vendor-update REV=<CLOUDSPEC_REV>` exits 0 and leaves `git diff --exit-code ess/cloud
   Taskfile.yml` clean.

## Rules for this work

- Merge into the integration branch `move/vendor-cloud-blocks`; `main` gets it once the whole gate
  is green.
- Builds and tests run through `slot --here`. No test over 60 s; the test step ends within 120 s. A
  pool job ends within 10 minutes unless it names its time with `slot --timeout` (at most 30m). No
  Python in anything committed. No network request in the test step.
- One owner per domain file; parallel edits are checked with `tools/check-overlay.sh`.
