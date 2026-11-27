---
format: aep.planning-md/3
id: story:docs-components-services
kind: story
status: implemented
title: Components and managed services pages generated from ess/, marked Planned
relations:
- decomposes: epic:docs-fidelity
- serves: vision:weft
- depends_on: story:docs-claims-check
- depends_on: story:docs-ux-harness
- depends_on: story:docs-status-proofs
scope:
- confidence: cited
  path: tools/check.d/30-docs-spec-pages.sh
- confidence: cited
  path: tools/docs-spec-pages.sh
- confidence: inferred
  path: tools/test-docs-spec-pages.sh
- confidence: cited
  path: website/docs/concepts/components.md
- confidence: cited
  path: website/docs/concepts/managed-services.md
- confidence: cited
  path: website/docusaurus.config.ts
- confidence: cited
  path: website/sidebars.ts
- confidence: inferred
  path: website/ux/baseline.json
revision: 15
transitions:
- {from: "draft", to: "proposed", at: "2026-11-27T19:03:23Z", actor: "human:operator", revision: 12, decided_on: {"recorded":{"approval":1,"review_outcome":7}}}
- {from: "proposed", to: "active", at: "2026-11-27T19:03:23Z", actor: "human:operator", revision: 13, decided_on: {"recorded":{"approval":1,"review_outcome":7}}}
- {from: "active", to: "implemented", at: "2026-11-27T20:35:46Z", actor: "human:operator", revision: 15, decided_on: {"recorded":{"test_result":1,"approval":1,"review_outcome":7}}}
---
## Epic

Decomposes `epic:docs-fidelity`, outcome 6. The owner, 2026-11-27: "what i could not find yet at
least in the docs online are our planned components/managed services".

## Context

The specification declares both; the public site shows neither (checked 2026-11-27 on main
90d59218: no page under `website/docs/` names `weft-api`, `weft-control`, `weft-node`,
`weft-edge`, an offering kind or a product).

- Components: `ess/components.yaml` declares 4 components (`weft-api` line 80, `weft-control`
  line 245, `weft-node` line 1001, `weft-edge` line 1039), each with the domains it owns, the
  commands it accepts and its bindings; `design:components` adds the weft CLI (§ weft CLI) and the
  plane of each (management, control, worker, data). Neither the weft CLI nor the planes are
  fields of `components.yaml`, so that part of the page is written by hand.
- Managed services: `ess/7-services/catalog.yaml` (domain `weft.catalog`) declares the offering
  kinds in `weft.catalog.OfferingKind` (queue, stream, cache, kv, sql, object-store, search,
  vector, scheduler, model-endpoint, sso), with `Offering`, `OfferingPlan` (a size tier: its
  limits, durability and replicas), `Engine`, `ServiceInstance` (lifecycle), `ServiceBinding`
  (modes), backups and restores, and products (`Product`, `ProductVersion`, `Installation`), each
  entity with a one-line `naming.summary`. `docs/domain-sketch.md` § 13 (lines 515-565) says what
  each kind is for and that each offering's own API is a future ESS system of its own.
- The owner's layout of 2026-11-27: products live under `product/`; the cloud under
  `product/cloud/`, consumer apps (a user management CLI) beside it, the operator CLI under
  `product/cloud/`. The pages name that layout where they name where code will live.

Shape (taken here):

- The site parses a `.md` page as CommonMark (`markdown.format: 'detect'`,
  `website/docusaurus.config.ts:29`). `tools/docs-spec-pages.sh` (POSIX sh, `jq`) reads
  `slot ess specify compile --path ess --format json` and writes the generated blocks of both
  pages, each between `<!-- generated: <name> -->` and `<!-- /generated: <name> -->`; the rest of
  each page is written by hand.
- Generated: the components table (name, summary, owned domains, number of accepted commands); the
  binding modes (`.types["weft.catalog.BindingMode"].body.variants`); the `ServiceInstance` states
  (`.entities["weft.catalog.ServiceInstance"].lifecycle.states`); the plans section (the
  `naming.summary` of `OfferingPlan` and the names of its fields, which say what a plan fixes);
  the products section (`Product`, `ProductVersion`, `Installation`, one line each, their
  `naming.summary`).
- Written by hand: the planes and the weft CLI (from `design:components`); the table of offering
  kinds with one line each of what it is for (from `docs/domain-sketch.md` § 13; the enum
  variants carry no text); the difference between an offering and a product.
- `tools/check.d/30-docs-spec-pages.sh` (in `slot task check`, where `ess` is installed; it takes
  the `ess/` tree and the two pages from `DOCS_SPEC_ESS` and `DOCS_SPEC_PAGES`, defaults the
  tree's) fails when a generated block differs in bytes from a regenerated one, and, reading the
  IR with its own `jq` queries and not through the generator, when the first cells of the
  components table differ from `.components | keys`, or the first cells of the hand-written kinds
  table differ from the variants of `weft.catalog.OfferingKind`. `tools/test-docs-spec-pages.sh`
  (run by the hook first) copies `ess/` and the pages under `target/`, adds a component to the
  copy of `components.yaml` and a variant to the copy's `OfferingKind`, and passes only when the
  hook exits 1 on each copy and 0 on the tree.

## Acceptance

1. `/docs/concepts/components` has one row per component in its generated table: the hook
   compares the first cells with `slot ess specify compile --path ess --format json | jq -r
   '.components | keys[]'` and exits 0 on the tree; each row holds the owned domains and the
   number of accepted commands.
2. A component added to a copy of `ess/components.yaml` fails the check:
   `tools/test-docs-spec-pages.sh` passes (`slot task check`).
3. `/docs/concepts/components` has the hand-written sections from `design:components`:
   `grep -n -e '^## Planes' -e '^## weft CLI' website/docs/concepts/components.md` prints both
   headings, and the planes section names each of the four components once.
4. `/docs/concepts/managed-services` has one row per variant of `weft.catalog.OfferingKind` in
   its kinds table, with one line of what it is for: the hook compares the first cells with
   `.types["weft.catalog.OfferingKind"].body.variants` of the IR and exits 0 on the tree; a
   variant added to a copy of `catalog.yaml` makes it exit 1 (`tools/test-docs-spec-pages.sh`).
5. `/docs/concepts/managed-services` lists the binding modes of `weft.catalog.BindingMode` and the
   lifecycle states of `ServiceInstance` in generated blocks: the hook's byte compare exits 0 on
   the tree and 1 on a copy of the page with one mode removed from the block.
6. `/docs/concepts/managed-services` has a plans section: a generated block with the
   `naming.summary` of `weft.catalog.OfferingPlan` and the names of its fields (what a plan
   fixes); the hook's byte compare exits 0 on the tree, and `grep -n '^## Plans'` on the page
   prints the heading.
7. `/docs/concepts/managed-services` has a products section: a generated block with one line each
   for `Product`, `ProductVersion` and `Installation`, each their `naming.summary`; the hook's
   byte compare exits 0 on the tree, and `grep -n '^## Products'` on the page prints the heading.
8. The products section says how an offering differs from a product in one hand-written paragraph
   under the heading `### Offering or product` (`grep -n` prints it).
9. Both pages carry the Planned banner: `grep -c ':::caution\[Planned\]'` prints 1 for each page,
   and the banner says "Specified, not built".
10. Both pages say which parts run today: each holds a line starting `Runs today:` that says no
    component serves a user and no offering provisions; the components page also names what the walking skeleton
    under `product/cloud/` builds (the control host binary and the node agent)
    (`grep -n '^Runs today:'` prints it).
11. `tools/check-docs.sh` on the tree with both pages prints `docs.planned-without-banner 0`.
12. `tools/check-docs.sh` on the tree with both pages prints `docs.status-claims-false 0` (the
    hook of `story:docs-status-proofs`, on which this story depends).
13. The top navigation and the concepts index link both pages: the built
    `website/build/docs/concepts/index.html` and the navbar of every built page hold a link to
    `/docs/concepts/components` and to `/docs/concepts/managed-services`.
14. The docs build passes with both pages: `slot npm --prefix website run build` exits 0.
15. The docs build job of `pages.yml` on this story's pull request is green.
16. The UX checks of `story:docs-ux-harness` pass on both pages: `slot npm --prefix website run ux
    -- --workers=2` after the build exits 0 with both routes in the sitemap, and
    `website/ux/baseline.json` holds a value for each key of both pages.

## Scope

- `website/docs/concepts/components.md` (new)
- `website/docs/concepts/managed-services.md` (new)
- `website/sidebars.ts`
- `website/docusaurus.config.ts`
- `tools/docs-spec-pages.sh` (new)
- `tools/check.d/30-docs-spec-pages.sh` (new)
- `tools/test-docs-spec-pages.sh` (new)
- `website/ux/baseline.json` (the values of the two new pages)

## Order

This story lands early, at the owner's request that the components and managed-services pages
come first. It depends on `story:docs-claims-check` (`docs.planned-without-banner` and its hook),
on `story:docs-status-proofs` (the hook `40-status-claims.sh` that counts
`docs.status-claims-false`, read by acceptance 12) and on `story:docs-ux-harness` (the UX checks of
acceptance 16 and `website/ux/baseline.json`, which must hold a value for every page). It does not
wait for the weft-sim chain: `story:anim-docs-published-run` depends on this story and edits
`website/sidebars.ts` and `website/ux/baseline.json` after it.

## Rules

No claim the tree does not prove. No person, host, session or private repository name. No Python.
No test over 60 s; the generator runs in `check.d`, not in the test step. A pool job ends within
10 minutes.
