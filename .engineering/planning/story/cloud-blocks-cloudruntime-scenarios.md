---
format: aep.planning-md/3
id: story:cloud-blocks-cloudruntime-scenarios
kind: story
status: draft
title: 'Synthesized scenarios reach CloudRuntime: ConfigureCloud again, ScaleControlPlane, ConfigureAccess'
relations:
- decomposes: epic:cloud-blocks
- serves: vision:weft
- depends_on: story:cloud-blocks-synth-domain-time
scope:
- confidence: cited
  path: ess/1-infrastructure/infrastructure.yaml
- confidence: cited
  path: quality/spec-diff-acknowledgements.json
revision: 3
---
## Epic

Decomposes `epic:cloud-blocks`. Found by `story:cloud-blocks-synth-domain-time`.

## Found

`weft.infrastructure.CloudRuntime` (story `cloud-blocks-runtime-fields`) shares the Cloud's id, and
ess 0.99.2 cannot address the row again by input in a generated scenario:

- `ConfigureCloud/already-configured` fails: the second call sends a literal id and gets
  `unknown-cloud`.
- `ScaleControlPlane/scaled` has no scenario (ESS-SYNTH-008: needs a scenario written by hand).
- `ConfigureAccess/configured` has no scenario (ESS-SYNTH-003: no CloudRuntime row could be
  arranged).

`ConfigureAccess` no longer sets `access_mode`; `ConfigureCloud` does, and
`ConfigureCloud/configured` passes. Acceptance 1 of `story:cloud-blocks-synth-domain-time` names
the old command.

## Outcome

The infrastructure and access subsets of `tools/synth-domain.sh` hold passing scenarios for
`ConfigureCloud/already-configured`, `ScaleControlPlane/scaled` and `ConfigureAccess/configured`:
either CloudRuntime gets an identity of its own that the scenarios capture, or the three are
written by hand as scenarios the suite carries.

## Scope

`ess/1-infrastructure/infrastructure.yaml`, `quality/spec-diff-acknowledgements.json`.

## Acceptance

1. `slot --timeout 30m tools/synth-domain.sh infrastructure` and `... access` report the three
   scenarios as passed.
2. `ess specify validate --path ess --strict-requires` exits 0.
