---
format: aep.planning-md/3
id: story:docs-claims-check
kind: story
status: implemented
title: 'Docs claims check: repository links, Planned banners and AGENTS.md paths held at 0 in pages.yml and task check'
relations:
- decomposes: epic:docs-fidelity
- serves: vision:weft
- depends_on: story:kpi-repository
scope:
- confidence: cited
  path: .github/workflows/pages.yml
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: Taskfile.yml
- confidence: inferred
  path: tools/check-docs.sh
- confidence: inferred
  path: tools/docs-check-fixture
- confidence: inferred
  path: tools/docs-check.d
- confidence: inferred
  path: tools/test-docs-check.sh
revision: 12
transitions:
- {from: "draft", to: "proposed", at: "2026-11-27T15:01:10Z", actor: "human:operator", revision: 8, decided_on: {"recorded":{"approval":1,"review_outcome":3}}}
- {from: "proposed", to: "active", at: "2026-11-27T15:01:10Z", actor: "human:operator", revision: 9, decided_on: {"recorded":{"approval":1,"review_outcome":3}}}
- {from: "active", to: "implemented", at: "2026-11-27T16:03:31Z", actor: "human:operator", revision: 12, decided_on: {"recorded":{"test_result":1,"approval":1,"review_outcome":3}}}
---
## Epic

Decomposes `epic:docs-fidelity`, outcome 1 (three of its five KPIs) and outcome 3. First story of
the epic: `story:docs-status-proofs`, `story:docs-commands-check` and `story:docs-figures` add their
checks as hooks of the directory this story creates, without editing its scripts.

Order: depends on `story:kpi-repository`, which edits `Taskfile.yml` (task `kpis`) and `AGENTS.md`
before it. `story:docs-ux-harness` depends on this story and edits `.github/workflows/pages.yml`
after it; `story:anim-docs-published-run` depends on this story and edits `Taskfile.yml` and
`pages.yml` after it.

## Context

`design:quality-kpis` § Docs site and § Repository define three KPIs that only a hand probe
measures today:

- `docs.dead-repo-links`: links to `github.com/beyond10x/weft/(tree|blob)/main/<path>` whose path
  does not exist (probe: 0; 17 distinct links on 2026-11-27 in `website/docs`, `website/product.json`
  and `website/docusaurus.config.ts`).
- `docs.planned-without-banner`: a `planned` item of `website/data/status.json` whose page neither
  carries `:::caution[Planned]` nor says "Planned" at the anchor of its `href` (probe: 0).
- `repo.agents-md-dead-paths`: repository paths named in backticks in `AGENTS.md` that do not exist
  (probe: 2, `surfaces/http/weft-api.yaml` and `surfaces/cli/weft.yaml`, both planned on the site).

Shape:

- `tools/docs-check.d/`: one POSIX sh check per KPI, run by `tools/run-hooks.sh`. Each takes the
  root of a tree as its only argument (default: the repository root), prints one line per offending
  item (`file:line` and the link, item or path), prints `<kpi-id> <count>` as its last line, and
  exits 1 when the count is above 0. It uses sh, grep, sed, awk and jq only: no node, no `ess`, no
  cargo, no network, so it runs before anything is installed and ends within a few seconds.
- `tools/check-docs.sh`: runs the hooks of `tools/docs-check.d` on the repository root.
- `tools/test-docs-check.sh`: for every `tools/docs-check-fixture/<check>/pass` and
  `.../<check>/fail` it runs the hook `<check>` on that tree and expects exit 0 and exit 1. A later
  story adds a hook and its two fixture trees and edits neither script.
- `Taskfile.yml`: task `docs-check` (the self-test, then `tools/check-docs.sh`), called by `check`.
- `.github/workflows/pages.yml`: a step that runs the self-test and `tools/check-docs.sh` before the
  planning-only decision, so it runs on every pull request. An `AGENTS.md`-only change is
  planning-only and skips the build, and without this it would skip the check that holds
  `repo.agents-md-dead-paths`.

What counts as a path in `AGENTS.md`: the first word of a backticked span that contains `/`, with
these excluded: format names (`ess-http/1`, `loom.agent/1`), package names (`@scope/name`), paths
outside the repository (`~/...`, `/...`, URLs), templates (`<...>`), and a name that the bullet it
stands in places under a directory (`scenarios/` in the `ess/` bullet is `ess/scenarios/`). On
`AGENTS.md` as of this story's base the hook reports exactly the two surface paths; the probe of
2026-11-27 with the plain rule reported 8, six of them false.

The fix of outcome 3: the two surface bullets move under a heading of their own that says Planned
(the documents do not exist; the site says so on `/docs/specification/read#the-files`); the hook
skips paths under a heading that contains "Planned".

Each hook prints its KPI value so that `epic:kpi-ratchet` can read it into `quality/ratchet.yaml`;
the ratchet assessment itself is that epic's, not this story's.

## Domain relations

None. This story touches no entity of `ess/`.

## Scope

- `tools/docs-check.d/` (`10-repo-links.sh`, `20-planned-banners.sh`, `30-agents-md-paths.sh`)
- `tools/check-docs.sh`, `tools/test-docs-check.sh`, `tools/docs-check-fixture/`
- `Taskfile.yml` (task `docs-check` in `check`)
- `.github/workflows/pages.yml` (the check step before the planning-only decision)
- `AGENTS.md` (the two surface paths under a Planned heading)

## Acceptance

1. `tools/check-docs.sh` on the tree prints `docs.dead-repo-links 0`, `docs.planned-without-banner 0`
   and `repo.agents-md-dead-paths 0` and exits 0.
2. `slot task check` runs `tools/check-docs.sh`: its output holds the same three lines
   (`task check` calls the task `docs-check`).
3. In `.github/workflows/pages.yml` the step that runs `tools/test-docs-check.sh` and
   `tools/check-docs.sh` stands before the planning-only decision step (`grep -n -e
   'tools/check-docs.sh' -e 'The changed paths' .github/workflows/pages.yml` prints the
   `check-docs.sh` line first) and carries no `if:` key (the lines of that step, from its `- name:`
   to the next `- name:`, hold no `if:`).
4. The log of that step in the `pages.yml` run on this story's pull request shows
   `docs.dead-repo-links 0`, `docs.planned-without-banner 0` and `repo.agents-md-dead-paths 0`.
5. `tools/test-docs-check.sh` passes only when each of the three hooks exits 1 on its `fail`
   fixture (a link to a missing path, a planned item whose page has no banner, a backticked path
   that does not exist) and 0 on its `pass` fixture.
6. On `AGENTS.md` as it was before this story, `30-agents-md-paths.sh` reports exactly five
   paths in a clean checkout: `target/generated/`, `ess/scenarios/`, `surfaces/http/weft-api.yaml`,
   `surfaces/cli/weft.yaml` and `quality/spec-diff-acknowledgements.json` (`ess/scenarios/` is not
   committed; the base text does not say that a tool writes the first and the last).
7. After this story, `AGENTS.md` lists the planned paths under a Planned heading, says which tool
   writes each generated path, under a heading that says Planned and
   `30-agents-md-paths.sh` on the tree reports `repo.agents-md-dead-paths 0`.

## Rules for this work

- POSIX sh; no Python in anything committed. The hooks run without the `slot` prefix inside
  `pages.yml`; locally they run inside `slot task check`.
- The self-test and the check end within 10 s together; no network.
- The repository is public: no person, host, session or private repository name in a hook, a
  fixture or its output.
