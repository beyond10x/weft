---
format: aep.planning-md/3
id: story:docs-ux-harness
kind: story
status: implemented
title: Docs site layout, accessibility and screen-only user checks with a baseline.json ratchet in the docs build
relations:
- decomposes: epic:docs-fidelity
- serves: vision:weft
- depends_on: story:docs-claims-check
scope:
- confidence: cited
  path: .github/workflows/pages.yml
- confidence: cited
  path: website/package-lock.json
- confidence: cited
  path: website/package.json
- confidence: inferred
  path: website/playwright.config.ts
- confidence: cited
  path: website/src/css/custom.css
- confidence: inferred
  path: website/src/theme
- confidence: cited
  path: website/tsconfig.json
- confidence: inferred
  path: website/ux
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-11-27T16:03:35Z", actor: "human:operator", revision: 6, decided_on: {"recorded":{"approval":1,"review_outcome":1}}}
- {from: "proposed", to: "active", at: "2026-11-27T16:03:36Z", actor: "human:operator", revision: 7, decided_on: {"recorded":{"approval":1,"review_outcome":1}}}
- {from: "active", to: "implemented", at: "2026-11-27T19:03:22Z", actor: "human:operator", revision: 8, decided_on: {"recorded":{"test_result":1,"approval":1,"review_outcome":1}}}
---
## Epic

Decomposes `epic:docs-fidelity`, outcome 4, its fast loop: the checks every docs build runs. The
measured rounds that move the numbers are `story:docs-ux-rounds`, which depends on this one.

Order: depends on `story:docs-claims-check`, which edits `.github/workflows/pages.yml` (the claims
check step) before it. `story:anim-search-player` (its player spec in `website/ux/`),
`story:anim-docs-published-run` and `story:docs-components-services` (their pages' values in
`website/ux/baseline.json`) depend on this story.

## Context

The `ui-iteration-loop` skill (uxui plugin) gives the method: an adapter (§ 1), the layout contract
L2 and the screen-only user L3 on every merge (§ 2), hard gates (§ 4), a lower-is-better
`baseline.json` ratchet (§ 6) and a deterministic screen-only user (§ 7). Built on Playwright and
axe-core, specs in TypeScript inside `website/`, which already builds TypeScript. Nothing of the
kind exists for the site today.

Adapter, `website/ux/adapter.ts`:

- pages: every route of the built `sitemap.xml` (the landing page and the docs pages);
- gating viewports 1280x800 and 390x844; measured-only 1440x900, 1024x768, 768x1024;
- themes light and dark;
- always-visible: the navbar, the search control and the theme toggle at every size, the sidebar at
  1280x800;
- scenarios for the screen-only user, each with its truth (the target route and a text on it),
  started on the landing page: find the command that validates the specification; find the `ess`
  version the specification requires; find how to replay a walk; find which parts are planned; find
  what the border of a formation is. Two of them are the anchors of `story:docs-ux-rounds`;
- no guidance cue and no protected focus (skill § 1: L0 and gate 6 dropped); completion is the
  screen-only user arriving at the truth.

Hard gates on every build (skill § 4, docs-site reading):

1. no horizontal page scroll and no element past its box at the gating viewports, both themes;
2. the always-visible elements are fully inside the viewport and not covered at their centre;
3. and 4. the screen-only user reaches the truth of every scenario using only visible controls
   found by role and name;
5. axe-core reports 0 serious or critical violations (`wcag2a, wcag2aa, wcag21a, wcag21aa,
   wcag22aa`) on every page in both themes, and the search is reachable by keyboard alone.

Ratchet `website/ux/baseline.json` (`{about, scenarios, tolerances}`, skill § 6): per scenario
`screen_only.actions`, `screen_only.looks`, `layout_shift`; per page the count of moderate and minor
axe violations. Tolerances: counts 0, `layout_shift` 0.01. The ratchet fails on a value worse than
the baseline, on an improvement not written into the baseline in the same change, and on a missing
value.

Runs:

- `website/playwright.config.ts`: `workers: 2`, a test killed at 60 s, a `webServer` that serves
  the built `website/build/`; the run prints its ten slowest tests; `npm run ux` in
  `website/package.json` wraps the run in `timeout 120`, so the step fails at 120 s.
- `.github/workflows/pages.yml`: after "Build the Docusaurus site", install Chromium for Playwright
  and run `npm --prefix website run ux`; the job fails on a hard gate or the ratchet.
- Locally: `slot npm --prefix website run build`, then `slot npm --prefix website run ux --
  --workers=2`.

A hard-gate failure the site has today is fixed in this story (`website/src/css/custom.css` or a
component under `website/src/theme/`), each named in the report with its count before. A failure
whose cause sits in `@beyond10x/docs-system` and cannot be overridden in `website/src` stops the
story and is reported; a gate is not waived.

Negative fixtures: two static HTML pages under `website/ux/fixtures/` (not part of the site), one
with an element 200 px wider than the viewport and one with a serious axe violation, on which the
layout and axe checks must fail.

## Domain relations

None. This story touches no entity of `ess/`.

## Scope

- `website/ux/` (`adapter.ts`, scenarios, specs, `baseline.json`, the ratchet, `fixtures/`)
- `website/playwright.config.ts`, `website/package.json`, `website/package-lock.json`,
  `website/tsconfig.json`
- `website/src/css/custom.css`, `website/src/theme/` (fixes of hard-gate failures found today)
- `.github/workflows/pages.yml` (the run after the build)

## Acceptance

1. The docs build job of `pages.yml` runs the layout, accessibility and screen-only checks against
   the built site and fails when a hard gate fails or a value of `website/ux/baseline.json` gets
   worse.
2. On the tree every hard gate passes on every sitemap page at both gating viewports in both
   themes, and `website/ux/baseline.json` holds a value for every key of every scenario and page.
3. The docs build job of `pages.yml` and the local run call the same script: the step in
   `pages.yml` runs `npm --prefix website run ux`, and `slot npm --prefix website run ux --
   --workers=2` after a local build exits 0 on the tree.
4. `website/playwright.config.ts` sets the per-test timeout to 60 000 ms and `workers: 2`
   (`npx playwright test --list` loads it; `grep -n timeout website/playwright.config.ts` shows
   the value).
5. The `ux` script of `website/package.json` wraps the run in `timeout 120`, so the step exits 124
   when it runs over 120 s (`jq -r .scripts.ux website/package.json` shows it).
6. The output of `slot npm --prefix website run ux -- --workers=2` ends with a list of its ten
   slowest tests, each with its time.
7. The layout check fails on the fixture page with the overflowing element.
8. The accessibility check fails on the fixture page with the serious violation.
9. The ratchet exits 1 on a value worse than its baseline, on an improved value not written into
   `baseline.json`, and on a missing value (one case each in the harness self-test).

## Rules for this work

- Playwright runs with `--workers=2` through `slot` locally; no test over 60 s; the step under
  120 s. A test over 5 s is named with its time in the report.
- A pool job ends within 10 minutes; `npm ci` names its time (`slot --timeout 15m npm ci`).
- No Python in anything committed: TypeScript in `website/`, POSIX sh elsewhere.
- No model call, no paid API, no network beyond the served build (skill levels L4 and L5 are not
  part of this).
- The repository is public: no person, host, session or private repository name in a spec, a
  fixture, a screenshot or a report.
