---
format: aep.planning-md/3
id: story:agenticspec-render-contract
kind: story
status: draft
title: 'Hand-over to agenticspec: a stable render contract with embed mode and state overlay, and the bundle gaps'
relations:
- decomposes: epic:agent-workloads
- serves: vision:weft
revision: 3
---
## Epic

Decomposes `epic:agent-workloads`, outcome 3 and § The bundle: a hand-over to the agenticspec
maintainers (`beyond10x/agenticspec`) of what weft needs from agenticspec. Nothing is built in weft;
the story is done when each item below is filed in agenticspec's planning store or as an issue
there, and each filing is named here.

## Context

- The owner, 2026-11-28 about 02:08 CET: "the rendering is provided from agenticspec" (epic §
  Order).
- What exists: `agenticspec_render::page(architecture, result)` makes one self-contained HTML page
  (`crates/agenticspec-render/src/lib.rs`), parts as `<g data-part=...>`, links as
  `<path data-from=... data-to=...>`, with a header, a theme switch and a footer of its own; the
  crate is not published (`publish.workspace`), and agenticspec says field names and formats may
  change between 0.x releases (`README.md:13`). The `embed.*` pages of `ui/agenticspec.ui.yaml`
  embed one session, not an architecture (`ui/README.md:14-25`).
- weft frames that page in its account console (`story:console-agent-architecture`) and marks each
  part with live state from its own views.

## Render contract asked of agenticspec

1. A stable, versioned render output: a format name (for example `agenticspec-render/1`) printed in
   the page, kept across 0.x releases or changed only with a new name.
2. An embed mode (`sim render --embed`, or a `page` option): no header, no theme switch, no footer;
   the theme taken from the host (`prefers-color-scheme` or a `data-theme` the host sets on the
   frame's root).
3. Stable ids: `data-part` equals the part's name in the architecture, `data-from` and `data-to`
   the link ends, for parts of nested architectures the nesting path; documented as part of the
   contract.
4. A state overlay input: a JSON document beside the architecture (for example
   `agenticspec-part-states/1`: per part `{state, ready, desired, note}`, with `state` one of a
   small fixed set) drawn as a badge and a tone per part; or a documented `postMessage` with the
   same document, so a host updates states without rendering again.
5. A release artifact weft can pin: the `agenticspec` binary of a release (with `SHA256SUMS`) or a
   published crate; weft pins its version and digest.

## Bundle gaps

What `agenticspec-deployment/1` still lacks for weft to run its units (agenticspec
`design/deployment.md`, branch `design/ingress` at `a47e0c9`, K7; `epic:agent-workloads` § The
bundle): disk and volume size in a unit's `claims` (it names CPU, memory, accelerators), and the
mapping of a unit's `isolation` to weft's level names (`weft.shared.IsolationLevel`). Checks,
secrets by name, egress destinations and runtimes by digest are in K7 at that commit. Filed for
agenticspec and copied to cloudspec's translation of the bundle, which has to carry them through.

## Scope

- No file of weft. The filings in agenticspec; their links recorded in this story's body.

## Acceptance

Each of the five render items and the two bundle gaps has a filing in agenticspec (an artifact id in
its planning store or an issue number), named in this story's body.
