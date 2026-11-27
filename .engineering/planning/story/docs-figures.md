---
format: aep.planning-md/3
id: story:docs-figures
kind: story
status: draft
title: Architecture figure and a domain map of ess/ regenerated from the tree on the docs site
relations:
- decomposes: epic:docs-fidelity
- serves: vision:weft
- depends_on: story:docs-claims-check
scope:
- confidence: inferred
  path: tools/check.d/30-domain-map.sh
- confidence: inferred
  path: tools/gen-domain-map.sh
- confidence: cited
  path: website/docs/concepts/nodes-and-clusters.md
- confidence: cited
  path: website/docs/index.md
- confidence: cited
  path: website/docs/specification/layers.md
revision: 8
---
## Epic

Decomposes `epic:docs-fidelity`, outcome 5 for two of its three figures: the architecture (control
host, nodes, substrate) and the domain map of `ess/`. The third, the search animation, is
`story:docs-figure-search-animation`. Depends on `story:docs-claims-check`: a caption link to
`ess/components.yaml` or `docs/implementation.md` on the repository is held by its
`docs.dead-repo-links` check. This story adds no docs check of its own.

## Context

The site explains the structure in tables and prose (`/docs/` § The planes,
`/docs/concepts/nodes-and-clusters`, `/docs/specification/layers`); its one figure is a Mermaid
flowchart of a formation (`concepts/formations.md`). `@docusaurus/theme-mermaid` is configured.

Domain map, generated, never drawn by hand:

- `tools/gen-domain-map.sh` (POSIX sh) reads the domains and their layers from
  `ess/ess-inputs.yaml` and the references between domains from `tools/check-layers.sh --graph`
  (`ess specify compile`; 0.8 s on 2026-11-27, 23 domains), and writes a Mermaid flowchart with one
  `subgraph` per layer and one node per domain, its id the domain name. The site parses a `.md`
  page as CommonMark (`markdown.format: 'detect'`, `website/docusaurus.config.ts:29`), so a `.md`
  page cannot import a partial: the generator writes the block between `<!-- domain-map -->` and
  `<!-- /domain-map -->` of `website/docs/specification/layers.md` and leaves the rest of the
  page as it is. How edges are drawn so the figure stays readable (one edge per pair of domains,
  or per pair of layers with the domains listed) is the implementer's choice, recorded in the
  generator's header.
- `tools/check.d/30-domain-map.sh`, in `slot task check`, where `ess` is installed; `pages.yml`
  reads the committed page and needs no `ess`. It takes the page from `DOMAIN_MAP_PAGE` (default
  `website/docs/specification/layers.md`) and:
  - regenerates the block into `target/` and compares bytes with the committed block;
  - reads the domain names from `slot ess specify compile --path ess --format json`
    (`jq -r '.domains | keys[]'`), not from the generator, and fails unless each is exactly one
    node of the block, inside the `subgraph` of its layer (the number prefix of the directory of
    the file that declares it, `ess/<n>-<layer>/`);
  - fails unless each pair `tools/check-layers.sh --graph` prints is drawn: as an edge between
    the two domain nodes, or, where the generator draws layer edges, between their two layers.

Architecture figure, hand-written Mermaid, in `/docs/` § The planes and in
`/docs/concepts/nodes-and-clusters` § Node agent and control plane:

- the four components of `ess/components.yaml` (`weft-api`, `weft-control`, `weft-node`,
  `weft-edge`) on their planes, the node channel and the exec channel of `protocols/`, and the
  plan of `docs/implementation.md` (decisions A3 and A6, § Shape): one control host process with
  `weft-api` and `weft-control`, and a node made of its agent and `substrate-daemon`, which
  confines the workload;
- in a fenced `mermaid` block, followed by the caption line
  `*Planned: drawn from ess/components.yaml and docs/implementation.md; neither binary is built.*`
  with both file names linked to the repository, since neither binary is built.
- No gate check of the names in the figure is added (a KPI for them is not among the five docs KPIs
  of `epic:docs-fidelity`); acceptance 5 and 6 name the commands a reviewer runs on the two pages.

## Domain relations

None. The figures draw what `ess/` and `docs/implementation.md` already declare; this story adds
and changes no relation.

## Scope

- `tools/gen-domain-map.sh`, `tools/check.d/30-domain-map.sh`
- `website/docs/specification/layers.md` (the generated block)
- `website/docs/index.md`, `website/docs/concepts/nodes-and-clusters.md`

## Order

`story:docs-commands-check` depends on this story: `specification/layers.md` carries `$ ` lines,
and that story checks them after this story has edited the page.

## Acceptance

1. The domain map holds every domain of `ess/` once, in the `subgraph` of its layer:
   `tools/check.d/30-domain-map.sh` compares the block with the domain names of
   `slot ess specify compile --path ess --format json` and exits 0 on the tree, and exits 1 with
   `DOMAIN_MAP_PAGE` set to a copy of `layers.md` with one domain node line removed.
2. The domain map draws every reference `tools/check-layers.sh --graph` prints: the same hook
   exits 0 on the tree and exits 1 on a copy of `layers.md` with one edge line removed.
3. `slot task check` fails when the committed block differs from one regenerated from `ess/`: the
   hook exits 1 on a copy of `layers.md` with one byte of the block changed, and `slot task check`
   exits 0 on the tree.
4. The docs build passes with the generated block in the page: `slot npm --prefix website run
   build` exits 0.
5. The architecture figure names its parts: for `website/docs/index.md` and
   `website/docs/concepts/nodes-and-clusters.md`, the `mermaid` block holds each of
   `weft-api`, `weft-control`, `weft-node`, `weft-edge` (the keys of
   `slot ess specify compile --path ess --format json | jq -r '.components | keys[]'`), `node
   channel`, `exec channel`, `control host`, `node agent` and `substrate-daemon` (`grep -c` of
   each in the block prints at least 1).
6. In each of those two pages, the line after the closing fence of that block is the caption:
   `grep -n '^\*Planned:.*ess/components.yaml.*docs/implementation.md' <page>` prints exactly one
   line, whose number is one more than the line of the closing fence.

## Rules for this work

- POSIX sh; no Python in anything committed. The generator check ends within 5 s.
- `slot task check` runs the gate; a pool job ends within 10 minutes.
- No claim on the site that the tree does not prove: the figure shows planned parts as planned.
- The repository is public: no person, host, session or private repository name in a figure or a
  caption (`substrate` is public, `docs/implementation.md` § Measured).
