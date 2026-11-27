# Docs claims hooks

`tools/check-docs.sh` (task `docs-check`, part of `task check`, and the first step of
`.github/workflows/pages.yml`) runs every executable file of this directory in name order
(`tools/run-hooks.sh`) on the repository root; the first that fails ends the run. Each hook holds
one KPI of `design:quality-kpis` at 0: a claim of the docs site or of `AGENTS.md` that the tree
does not prove.

Contract of a hook:

- POSIX sh with grep, sed, awk and jq; no node, no `ess`, no cargo, no network, no Python. It
  runs before anything is installed and ends within a few seconds.
- Takes the root of a tree as its only argument (default: the repository root) and reads only
  that tree.
- Prints one line per offending item, `<file>:<line>` and the link, item or path, then
  `<kpi-id> <count>` as its last line. Exits 0 when the count is 0, 1 when it is above 0, 2 when
  it cannot measure.
- Has two fixture trees, `tools/docs-check-fixture/<hook name without .sh>/pass` and `.../fail`;
  `tools/test-docs-check.sh` expects exit 0 and exit 1 on them, and, when
  `tools/docs-check-fixture/<hook>/pass.out` or `fail.out` exists, exactly that output. A new
  hook adds itself and its trees and edits neither script.
- `epic:kpi-ratchet` reads the `<kpi-id> <count>` lines into `quality/ratchet.yaml`.

Hooks: `10-repo-links.sh` (`docs.dead-repo-links`), `20-planned-banners.sh`
(`docs.planned-without-banner`), `30-agents-md-paths.sh` (`repo.agents-md-dead-paths`).
