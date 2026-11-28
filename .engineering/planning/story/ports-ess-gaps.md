---
format: aep.planning-md/3
id: story:ports-ess-gaps
kind: story
status: draft
title: ESS gaps 26-29 filed with the ESS maintainers
relations:
- decomposes: epic:platform-ports
- serves: vision:weft
scope:
- confidence: cited
  path: docs/ess-gaps.md
revision: 2
---
## Epic

Decomposes `epic:platform-ports`, epic scope "what the specification declares for them". Design:
`docs/platform-ports.md` § 9, "ESS gaps", gaps 26-29. Depends on none.

## Context

- Gap 26: a component cannot declare a port (wanted: a component key `uses:` with operations,
  checked by validate, emitted by synthesis as a trait beside `Context`, answered by conformance with
  a double).
- Gap 27: an `external:` branch cannot name what answers it (wanted: `external: {port, operation,
  text}`); S4 (`story:ports-spec-port-names`) is the workaround.
- Gap 28: a requested effect is not linked to its observation (wanted: `requests:` and `answers:`);
  S5 (`story:ports-spec-step-block`) is the workaround.
- Gap 29: no adapter per placement (wanted: a realization keyed by a component setting's value).
- Each gap is filed as a story in ESS's planning store through its maintainers, and
  `docs/ess-gaps.md` continues at 26 with that id.
- Ownership: cloudspec, a new repository being created, will later own the port definitions; weft
  keeps the wiring of the adapters.

## Domain relations

- None: a document change.

## Scope

- `docs/ess-gaps.md`

## Acceptance

`docs/ess-gaps.md` lists gaps 26-29, each moved from proposed (design § 9) to filed by carrying the
id of its story in ESS's planning store.

