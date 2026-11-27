---
format: aep.planning-md/3
id: story:docs-ux-budget
kind: story
status: draft
title: The docs UX step stays within its budget as pages are added
relations:
- decomposes: epic:docs-fidelity
- serves: vision:weft
- depends_on: story:docs-ux-harness
scope:
- confidence: cited
  path: website/playwright.config.ts
- confidence: cited
  path: website/ux/page.ts
- confidence: cited
  path: website/ux/probe.ts
- confidence: cited
  path: website/ux/site.spec.ts
revision: 2
---
## Context

The `npm run ux` step (`story:docs-ux-harness`, `website/ux/`) costs about 1-3 s per page and
theme, and every new docs page adds 2 tests. Measured on this host (`--workers=2`, load from
`/proc/loadavg`):

- weft#17 merged: 87 tests, 53.8 s at load 12-16; CI 66.3 s;
- `story:anim-search-player`: 108 tests, 54.4-88.4 s at load 14-28;
- `story:docs-components-services`: 93 tests, 75-104 s at load 16-30.

The step's budget is 120 s (`website/package.json` `timeout 120`, `globalTimeout` 105 s). With
both stories merged and the site growing, it reaches the budget under the host's usual load.
A test step that grows with the number of pages is the cost multiplied per test that the
repository rules name.

## Scope

- `website/ux/site.spec.ts`, `website/ux/page.ts`, `website/ux/probe.ts`
- `website/playwright.config.ts`

## Acceptance

1. With every page of the site at the time of the change, `npm run ux` ends within 60 s at host
   load 30 or higher (wall time and `/proc/loadavg` named, 3 runs).
2. The cost per added page is measured and named: the step's wall time with the sitemap as is
   and with 10 more pages of a fixture docs tree.
3. No gate is weakened: every hard gate still runs at 1280x800 and 390x844 in both themes, and
   the harness self-tests (`harness.spec.ts`) pass unchanged.
4. Raising a timeout, adding workers or shards, or dropping pages is not a fix.

## Rules for this work

- npm, Playwright and the docs build run through `slot --here` with `--workers=2`.
- No test over 60 s; the step under 120 s.
- No Python in anything committed.
