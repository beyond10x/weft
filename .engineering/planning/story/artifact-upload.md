---
format: aep.planning-md/3
id: story:artifact-upload
kind: story
status: draft
title: 'Upload a gzip archive through the API: weft stores the bytes by digest and registers the artifact, no container daemon on the client'
relations:
- decomposes: epic:platform-ports
- serves: vision:weft
- depends_on: story:ports-cloud-objects-local
- depends_on: story:artifact-sign-local
revision: 1
---
## Epic

Decomposes `epic:platform-ports`: the bytes of an artifact enter weft through its API.

## Order

The operator, 2026-11-28 (relayed by the agentic operator as requirement R81): "with weft running,
we could create that image inside weft via api by uploading gz - OR weft also has a registry
deployed for its system-tools/helpers - and we can push to an adress, registries being a 1st class
domain object in wefts apis/domains". This story is the first route; `story:container-registry` is
the second.

## Context

- `weft.artifacts.RegisterArtifact` records a digest, a kind, a size and a signature and carries no
  bytes (`ess/3-resources/artifacts.yaml:196`); the design leaves ingestion open: "none: the bytes
  are stored before the command" (cloudspec@2ed72c2:docs/platform-ports.md § 4).
- C5 `objects` holds the artifact store; its local adapter keeps it in a directory
  (`story:ports-cloud-objects-local`).
- First consumer: one UI unit of static files (a Vite build) beside one long-running server unit.

## Outcome

1. An upload operation of the user API takes a gzip archive in the request body, with its kind and
   the account, and needs no container daemon on the client.
2. weft computes the digest of the bytes, stores them through C5 `objects` under that digest, and
   registers the artifact with `RegisterArtifact` (signed with the account's local key on the local
   placement, `story:artifact-sign-local`). A second upload of the same bytes answers the same
   digest and stores nothing new.
3. Two archive forms: a root filesystem (kind `Image`; runs once N6 `images` serves it,
   `story:ports-node-images-local`, substrate gap 5) and a directory of static files (kind `Site`,
   served over HTTP by the edge, `story:static-site-serve`).
4. A size limit per account, and refusals named by cause: not gzip, over the limit, unknown kind,
   unsafe path in the archive (absolute, `..`, a link leaving the root).

## Scope

- `ess/3-resources/artifacts.yaml` (`UploadArtifact`, kind `Site`), `surfaces/http/weft-api.yaml`,
  `product/cloud/crates/weft-bin/` (the upload route), `product/cloud/crates/weft-platform/src/local/`
  (objects on local), `product/cloud/crates/weft-testkit/`

## Acceptance

1. `ess specify validate --path ess --strict-requires` passes with `UploadArtifact` and kind `Site`.
2. On the local placement, a POST of a gzip archive of two files answers the digest of its bytes,
   and `weft.artifacts.Artifacts` lists that digest with kind `Site`.
3. A second POST of the same bytes answers the same digest, and the objects directory holds one
   copy.
4. An archive with an entry `../x` is refused with `unsafe-path`, and nothing is stored.

## Rules for this work

- Builds and tests run through `slot`. No test over 60 s; the test step ends within 120 s. A pool
  job ends within 10 minutes unless it names its time with `slot --timeout` (at most 30m). No
  Python in anything committed. No network request beyond loopback in the test step.
