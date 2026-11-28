---
format: aep.planning-md/3
id: story:cloud-blocks-lock-gate
kind: story
status: draft
title: The gate refuses a vendored cloudspec copy that differs from its lock or its pin, with no network
relations:
- decomposes: epic:cloud-blocks
- serves: vision:weft
revision: 1
---
## Epic

Decomposes `epic:cloud-blocks`: the vendored copy cannot drift from its pin unnoticed.

## Outcome

1. `tools/check-vendor-lock.sh` (POSIX sh, no git, no network) checks `ess/cloud/cloud.lock`: its
   format and `system weft`; its `commit` equals `CLOUDSPEC_REV` in `Taskfile.yml`; every
   `output` digest matches the file; no YAML under `ess/cloud/` that the lock does not list.
2. It runs in `task check` (`tools/check.d/15-vendor-lock.sh`) with a test of each refusal
   (`tools/test-vendor-lock.sh` and a fixture).
3. The slow lane (`tools/slow.d/20-vendor-fresh.sh`) fetches cloudspec at `CLOUDSPEC_REV` into
   the build directory and runs its `scripts/vendor.sh --check ess/cloud`; it runs when
   `ess/cloud/**` or `CLOUDSPEC_REV` changed. CI job `vendor-fresh` in `gate.yml` does the same.
4. `task vendor-update REV=<sha>` fetches, runs `vendor.sh --ref <sha> weft ess/cloud` with the
   excludes of D-a, and sets `CLOUDSPEC_REV`.

## Scope

`tools/check-vendor-lock.sh`, `tools/check.d/15-vendor-lock.sh`, `tools/test-vendor-lock.sh`,
`tools/fixtures/vendor-lock/`, `tools/slow.d/20-vendor-fresh.sh`, `Taskfile.yml`,
`.github/workflows/gate.yml`, `AGENTS.md`.

## Acceptance

1. `tools/test-vendor-lock.sh` passes and shows the check refusing each of: one changed byte in a
   listed file, an unlisted YAML file, a lock `commit` that differs from `CLOUDSPEC_REV`.
2. `tools/check-vendor-lock.sh` exits 0 on the vendored tree with networking off
   (`unshare -rn`).

## Rules for this work

- Merge into the integration branch `move/vendor-cloud-blocks`; `main` gets it once the whole gate
  is green.
- Builds and tests run through `slot --here`. No test over 60 s; the test step ends within 120 s. A
  pool job ends within 10 minutes unless it names its time with `slot --timeout` (at most 30m). No
  Python in anything committed. No network request in the test step.
- One owner per domain file; parallel edits are checked with `tools/check-overlay.sh`.
