---
format: aep.planning-md/3
id: story:docs-commands-check
kind: story
status: draft
title: Command lines of the docs checked against committed help of ess, weft-sim and the Taskfile
relations:
- decomposes: epic:docs-fidelity
- serves: vision:weft
- depends_on: story:docs-claims-check
- depends_on: story:anim-point-replay
- depends_on: story:docs-figures
- depends_on: story:docs-figure-search-animation
scope:
- confidence: inferred
  path: tools/check.d/20-cli-snapshots.sh
- confidence: inferred
  path: tools/docs-check-fixture/50-commands
- confidence: inferred
  path: tools/docs-check.d/50-commands.sh
- confidence: inferred
  path: tools/test-cli-snapshots.sh
- confidence: inferred
  path: website/data/cli
- confidence: cited
  path: website/docs/getting-started.md
- confidence: cited
  path: website/docs/simulator/install.md
- confidence: cited
  path: website/docs/simulator/run.md
- confidence: cited
  path: website/docs/specification/layers.md
- confidence: cited
  path: website/docs/specification/read.md
- confidence: cited
  path: website/docs/specification/validate.md
revision: 6
---
## Epic

Decomposes `epic:docs-fidelity`, the KPI `docs.commands-refused` of outcome 1. Depends on
`story:docs-claims-check` (its hook directory, fixture runner, task and `pages.yml` step).

Order:

- `story:anim-point-replay`: it edits the one `USAGE` text of `tools/sim/src/main.rs` (the replay
  of a search point) that `website/data/cli/weft-sim.txt` mirrors, and `run.md`; this story
  writes the snapshot after it.
- `story:docs-figures` (`specification/layers.md`) and `story:docs-figure-search-animation`
  (`simulator/run.md`) edit pages with `$ ` lines before this story; this story checks them as
  they stand and fixes a refused line after them.
- `story:docs-ux-rounds` depends on this story: its rounds may change any page of
  `website/docs/`, after the refused lines are fixed.

## Context

`design:quality-kpis`: `docs.commands-refused`, a command line in the docs that the named binary
refuses (unknown flag, unknown subcommand); "needs a checker over code blocks". On 2026-11-27 the
`$ ` lines of `website/docs` call `ess` (3), `weft-sim` (5 on `PATH`, 3 as
`tools/sim/target/debug/weft-sim`), `task` (5), `tools/check-layers.sh` (2),
`tools/check-stamped-time.sh` (2), and `git`, `cd`, `cargo` once or twice each.

The docs build of `pages.yml` has neither `ess` nor a weft-sim binary, and building weft-sim there
fetches and compiles the ESS crates. So the check reads committed help text, and the gate, which
has both binaries, holds that text equal to the binaries:

- `website/data/cli/ess.txt`: the `--help` output of every `ess` subcommand path the docs use
  (`ess specify validate`, ...), headed by `ess --version`. `website/data/cli/weft-sim.txt`: the
  output of `weft-sim --help` (one `USAGE` text in `tools/sim/src/main.rs`).
- Hook `tools/docs-check.d/50-commands.sh` reads every `$ ` line of `shell-session`, `console`,
  `shell`, `sh` and `bash` blocks in `website/docs`, and refuses: an `ess` or `weft-sim`
  subcommand or `--flag` the help text does not list; a `task <name>` that `Taskfile.yml` does not
  define (read from the file, `task` is not installed in `pages.yml`); a `tools/...` script that
  does not exist or is not executable, or a flag that its usage line does not list. Lines of other
  programs (`git`, `cd`, `cargo`) are counted as unchecked and printed, not refused. It prints
  `docs.commands-refused <n>`.
- Hook `tools/check.d/20-cli-snapshots.sh` (run by `slot task check`): fails unless `ess --version`
  is the version `ess/ess-inputs.yaml` requires; builds weft-sim with `cargo build` in `tools/sim`
  when the binary is absent (shimmed through slot inside the gate job); regenerates both help files
  under `target/` and compares bytes with the committed ones. A weft-sim change that drops a flag
  the docs use thus fails the gate until the help file and the page follow.

- `tools/test-cli-snapshots.sh` (new, POSIX sh, run by `slot task check` through the hook): the
  hook reads its help files from `CLI_SNAPSHOT_DIR` (default `website/data/cli`); the self-test
  copies the committed files under `target/`, changes one line of the `weft-sim.txt` copy, and
  passes only when the hook exits 1 on that copy and 0 on an unchanged copy.

A refused line found on the tree is fixed in its page in this story, and the page is named in the
report. The pages with `$ ` lines on 2026-11-27 (`grep -rln '^\$ ' website/docs`):
`getting-started.md`, `simulator/install.md`, `simulator/run.md`, `specification/read.md`,
`specification/layers.md`, `specification/validate.md`. These six are the pages this story may
edit, and only at a refused line.

## Domain relations

None. This story touches no entity of `ess/`.

## Scope

- `website/data/cli/` (`ess.txt`, `weft-sim.txt`)
- `tools/docs-check.d/50-commands.sh`, `tools/docs-check-fixture/50-commands/`
- `tools/check.d/20-cli-snapshots.sh`, `tools/test-cli-snapshots.sh`
- the pages with `$ ` lines, at a refused line only: `website/docs/getting-started.md`,
  `website/docs/simulator/install.md`, `website/docs/simulator/run.md`,
  `website/docs/specification/read.md`, `website/docs/specification/layers.md`,
  `website/docs/specification/validate.md`

## Acceptance

1. `tools/check-docs.sh` on the tree prints `docs.commands-refused 0`.
2. The docs job of `pages.yml` installs no `ess` and builds no weft-sim:
   `grep -nE 'ess-[0-9]|beyond10x/ess|cargo (build|install)|tools/sim' .github/workflows/pages.yml`
   prints nothing.
3. The log of the docs job of `pages.yml` on this story's pull request shows the line
   `docs.commands-refused 0`.
4. `tools/test-docs-check.sh` passes only when `50-commands.sh` exits 1 on a fixture page with
   `weft-sim explore --no-such-flag`, one with `task no-such-task` and one with
   `ess specify no-such-subcommand`, and exits 0 on its `pass` fixture.
5. `tools/check.d/20-cli-snapshots.sh` exits 0 on the tree under `slot task check`.
6. `tools/test-cli-snapshots.sh` passes: the hook exits 1 on a copy of the help files with one
   line of `weft-sim.txt` changed and 0 on an unchanged copy (`CLI_SNAPSHOT_DIR=<copy>`); it runs
   in `slot task check`.

## Rules for this work

- POSIX sh; no Python in anything committed.
- `slot task check` runs the gate; a pool job ends within 10 minutes (`SLOT_TIMEOUT` for a longer
  shimmed cargo call, at most 30m). No test over 60 s; the test step stays within 120 s (this
  story adds no test to it).
- The docs hook ends within 10 s; no network.
