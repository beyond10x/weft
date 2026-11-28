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
revision: 3
---
## Epic

Decomposes `epic:cloudspec-submission`, outcome 5: the docs say how a cloudspec architecture reaches
weft, on one page linked from the deploy page. The site has no page named deployments;
`website/docs/run/deploy-a-workload.md` (sidebar group `run`) is the deploy page.

## Outcome

1. A page `website/docs/run/cloudspec-submission.md`: cloudspec writes the submission
   (`cloudspec generate weft --from <bundle|architecture> [--binding f] --out submission.json`, linked
   to cloudspec's `docs/layers.md` § 11 at commit fdcf7e8); its five members and the canonical-JSON
   digests; what weft checks offline and the name of each refusal (`story:cloudspec-submission-reader`);
   the three goldens in `tests/fixtures/cloudspec/`; the one workload kind weft runs (`snippet`) and
   that every other construct arrives refused by name in the coverage.
2. Submitting and running a submission is not built: that section carries a `:::caution[Planned]`
   banner and names no command that does not exist.
3. `deploy-a-workload.md` links the page, and the sidebar lists it under `run`.

## Scope

`website/docs/run/cloudspec-submission.md` (new), `website/docs/run/deploy-a-workload.md`,
`website/sidebars.ts`.

## Acceptance

The built site serves the page from the `run` group and from a link on the deploy page, and
`tools/check-docs.sh` reports every count at 0.

Checked by:

1. `slot npm run build` in `website/` succeeds and the output holds
   `run/cloudspec-submission/index.html`.
2. `tools/check-docs.sh` prints `docs.dead-repo-links 0`, `docs.planned-without-banner 0` and
   `docs.status-claims-false 0`.
3. Every refusal name on the page is a variant of `Refusal` in
   `product/cloud/crates/weft-cloudspec`.

## Rules for this work

- The site is public: no person, session, private repository or host named.
- Node work runs through slot (`slot --timeout 15m npm ci`, `slot npm run build`); no test over
  60 s, the test step within 120 s, a pool job within 10 minutes unless it names its time.
- No Python in anything committed.
