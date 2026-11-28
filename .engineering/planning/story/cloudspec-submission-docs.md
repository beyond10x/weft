---
format: aep.planning-md/3
id: story:cloudspec-submission-docs
kind: story
status: draft
title: One docs page says how a cloudspec architecture reaches weft, linked from the deploy page
relations:
- decomposes: epic:cloudspec-submission
- serves: vision:weft
- depends_on: story:cloudspec-submission-reader
scope:
- confidence: cited
  path: website/docs/run/cloudspec-submission.md
- confidence: cited
  path: website/docs/run/deploy-a-workload.md
- confidence: cited
  path: website/sidebars.ts
revision: 4
---
## Epic

Decomposes `epic:cloudspec-submission`, outcome 5: the docs say how a cloudspec architecture reaches
weft, on one page linked from the deploy page. The site has no page named deployments;
`website/docs/run/deploy-a-workload.md` (sidebar category `Run weft (Planned)` in
`website/sidebars.ts`) is the deploy page.

## Outcome

1. A page `website/docs/run/cloudspec-submission.md` (doc id `run/cloudspec-submission`):
   cloudspec writes the submission (`cloudspec generate weft --from <bundle|architecture>
   [--binding f] --out submission.json`, linked to cloudspec's `docs/layers.md` § 11 at commit
   fdcf7e8); its five members and the canonical-JSON digests; what weft checks offline and the name
   of each refusal (`story:cloudspec-submission-reader`), one table row per refusal under the
   heading `## What weft refuses`, the name first in backticks (`` | `NotASubmission` | … | ``);
   the three goldens in `tests/fixtures/cloudspec/`; the one workload kind weft runs (`snippet`) and
   that every other construct arrives refused by name in the coverage.
2. Submitting and running a submission is not built: that section carries a `:::caution[Planned]`
   banner and names no command that does not exist.
3. `deploy-a-workload.md` links the page, and `website/sidebars.ts` lists `run/cloudspec-submission`
   in the category labelled `Run weft (Planned)`, after `run/deploy-a-workload`.

## Scope

`website/docs/run/cloudspec-submission.md` (new), `website/docs/run/deploy-a-workload.md`,
`website/sidebars.ts`.

Overlap with a draft outside this epic: `story:anim-docs-published-run` also lands on
`website/sidebars.ts` (the `cloudspec-sim` category); a wave keeps the two apart.

## Acceptance

The built site serves the page from the `Run weft (Planned)` category and from a link on the deploy
page, and `tools/check-docs.sh` reports every count at 0.

Checked by:

1. `slot npm run build` in `website/` succeeds and the output holds
   `website/build/docs/run/cloudspec-submission.html` (`website/docusaurus.config.ts`:
   `routeBasePath: 'docs'` and `trailingSlash: false`, so a doc is written as `<path>.html`, as
   `build/docs/run/deploy-a-workload.html` is today; served at `/weft/docs/run/cloudspec-submission`
   under `baseUrl: '/weft/'`).
2. `tools/check-docs.sh` prints `docs.dead-repo-links 0`, `docs.planned-without-banner 0` and
   `docs.status-claims-false 0`.
3. The refusal names on the page are exactly the variants of `enum Refusal` in
   `product/cloud/crates/weft-cloudspec/src/`: run from the repository root, this prints nothing
   and exits 0:

   ```sh
   mkdir -p target/docs-refusals
   sed -n '/^pub enum Refusal {/,/^}/s/^    \([A-Z][A-Za-z0-9]*\).*/\1/p' \
     product/cloud/crates/weft-cloudspec/src/*.rs | sort -u > target/docs-refusals/code
   sed -n '/^## What weft refuses/,/^## /s/^| `\([A-Z][A-Za-z0-9]*\)` |.*/\1/p' \
     website/docs/run/cloudspec-submission.md | sort -u > target/docs-refusals/page
   test -s target/docs-refusals/code && diff target/docs-refusals/code target/docs-refusals/page
   ```

## Rules for this work

- The site is public: no person, session, private repository or host named.
- Node work runs through slot (`slot --timeout 15m npm ci`, `slot npm run build`); no test over
  60 s, the test step within 120 s, a pool job within 10 minutes unless it names its time.
- No Python in anything committed.
