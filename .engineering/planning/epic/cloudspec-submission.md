---
format: aep.planning-md/3
id: epic:cloudspec-submission
kind: epic
status: draft
title: weft runs a cloudspec architecture submitted as cloudspec-weft-submission/1
relations:
- serves: vision:weft
- depends_on: epic:cloud-blocks
revision: 1
---
## Why

Owner's decision R85 = A (2026-11-28 11:08 CET, relayed by the host operator): weft reads
cloudspec's architecture document unchanged, plus a binding and the coverage, as one submission
(`cloudspec-weft-submission/1`). There is no weft formation document for this path and no
translator inside weft. "Build it."

cloudspec delivered its side in cloudspec#12, merged as fdcf7e8286a79bc3bbcdb54bf9fe1e332d17884c:

- `cloudspec generate weft --from <bundle|arch> [--binding f] --out submission.json` prints the
  file's sha256 and exits 1 (file written) when anything is refused;
- format `cloudspec-weft-submission/1`: `{format, architecture, binding (null when none),
  coverage, digests{architecture, binding}}`; member digests over canonical JSON (sorted keys, no
  whitespace);
- `SourceKind::Cloudspec` in `blocks/11-translation/deployments.yaml`;
- goldens `fixtures/weft/{snippet,workstation,host-daemon}.submission.json`;
- the refusal table `crates/cloudspec/src/generate/weft/runs.rs` (cloudspec `docs/layers.md` § 11);
- workload kind `snippet` {command, environment, shape: job} is the only mapped kind.

## Outcome

1. weft's vendored blocks are pinned at a cloudspec revision that holds `SourceKind::Cloudspec`
   (fdcf7e8 or later; one pin move together with the cloudspec-sim view-name fix).
2. `weft.deployments.SubmitDeploymentSource` accepts a `cloudspec-weft-submission/1` with
   `SourceKind::Cloudspec`: weft checks the format, recomputes both member digests and refuses a
   mismatch, an unknown format version or a binding that names no part of the architecture, each
   with a named error and nothing changed.
3. An accepted submission becomes a plan whose instances come from the architecture's snippet
   workloads; the walking-skeleton path runs the `snippet` golden to a finished job on the local
   node.
4. The three goldens are copied into weft's fixtures with their digests; a test step check reads
   them with no network and no cloudspec binary.
5. The docs say how a cloudspec architecture reaches weft (one page, linked from deployments).

## Out of scope

- Workload kinds other than `snippet` (cloudspec maps no other kind yet).
- A weft formation document or a translator for this source kind.
- cloudspec's layer refactor (a later pin move with its rename file).
