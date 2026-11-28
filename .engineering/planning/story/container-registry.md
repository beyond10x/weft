---
format: aep.planning-md/3
id: story:container-registry
kind: story
status: draft
title: Every cloud serves an OCI registry backed by its objects port; a push registers a signed Image artifact by digest; nodes pull by digest
relations:
- decomposes: epic:platform-ports
- serves: vision:weft
- depends_on: story:contract-providers
revision: 1
---
## Epic

Decomposes `epic:platform-ports`: the images a cloud runs come from a registry the cloud itself
serves.

## Order

The owner, 2026-11-28 about 03:36 CET, typed in this session: "we also need a concept of a
container registry", because formations are submitted, and managed services are delivered by a
deployment that pulls images, which may be custom, the operator's own; "a deployment of Weft would
basically spin this up and provide these mechanisms". The generic part is defined in a layer of
cloudspec (requested from the cloudspec session the same minute as a contract,
`container_registry`, beside `artifact_store`); weft provides it.

## Context

- An artifact is content-addressed and signed; `RegisterArtifact` refuses unsigned bytes
  (`ess/3-resources/artifacts.yaml`); kind `Image` is "an OCI image by digest"
  (`docs/domain-sketch.md:228`).
- N6 `images` prepares a root filesystem from an artifact's bytes in C5 `objects`, digest checked
  on the node (`docs/platform-ports.md` § N6); images "are recorded by observed digest even when
  given as a tag" (substrate's contract).
- Nothing in weft serves the OCI distribution protocol: an operator or a pipeline has no place to
  push an image to, and a managed-service engine's image (`weft.catalog.Engine`) has no source.

## Outcome

1. Every cloud serves one OCI registry (distribution protocol: blobs, manifests, tags), backed by
   C5 `objects`; its address is part of the cloud's endpoints.
2. A push of a manifest registers an `Image` artifact by digest under the pusher's account, signed
   (`story:artifact-sign-local` on local); a tag resolves to a digest and is recorded as a name,
   never as the identity.
3. Access is scoped per account (read, push) and denies by default; a node pulls only digests its
   assignments name.
4. N6 `images` takes an `Image` artifact from this registry by digest.
5. At cloud creation the images of the engines the cloud offers are copied into the registry by
   digest from a declared source, so a managed service never pulls from outside the cloud at start.
6. The registry fulfils cloudspec's `container_registry` contract and is named as such on its
   `Engine` (`story:contract-providers`).

## Scope

- `ess/3-resources/artifacts.yaml` (registry, repository and tag), `ess/7-services/catalog.yaml`
  (the registry engine), `docs/platform-ports.md` § N6 and § C5, `docs/domain-sketch.md`
  § Artifacts

## Acceptance

1. `ess specify validate --path ess --strict-requires` passes with the registry, its repositories
   and tags.
2. A scenario pushes a manifest and reads one `Image` artifact registered by its digest.
3. A scenario moves a tag to a second digest; the first artifact is unchanged and still pulls by
   digest.
4. A scenario refuses a push from an account without push rights on the repository, naming the
   right.

## Dependencies outside weft

- cloudspec story contract-container-registry: contract `container_registry` in layer 1, beside
  `artifact_store`; a workload image names a need on it plus a digest; a binding picks the registry.
  Cite at the merge sha of cloudspec#4.

## Rules for this work

- Builds and tests run through `slot`. No test over 60 s; the test step ends within 120 s. A pool
  job ends within 10 minutes unless it names its time with `slot --timeout` (at most 30m). No
  Python in anything committed. No network request beyond loopback in the test step.
