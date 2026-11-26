---
format: aep.planning-md/3
id: design:protocol-ports
kind: design
status: draft
title: Protocol ports from RFCs, adapters as managed products, sessions above
revision: 2
---
## Goal

The owner, 2026-11-26: "there should be infra level ports based and checked against rfcs (http/*,
websocket, etc ..., sip, ...)"; "the cloud must support certain protocols ingresses (... with
different various entrypoints, then loadbalancing, whitelisting, sip-registrar, etc...) -
PBX/Registrar/SIP-proxy <- all managed products of the cloud platform, specified by specs created
from real RFCs | <- this is the port layer"; "when entering the cloud higher layers we already deal
with higher level abstractions like MediaSession (from sip, from rtvbp, from webrtc...)"; "for an
implementation and the sip->mediaSession seam we can deploy sipx-clstr <- this needs a
lot conformance testing or even a more code-generation like approach".

## Before this design

- weft's only ingress is `weft-edge` (design:components): HTTP routes to a formation's endpoints,
  held wake requests, certificates (`ess/5-runtime/edge.yaml` on branch `spec/components`). No
  protocol other than HTTP is named; no entrypoint, transport, load-balancing policy or allowlist.
- Protocol specs in weft are `ess-protospec/1` documents for weft's own channels
  (`protocols/node-channel.yaml`, `protocols/exec-channel.yaml`): participants, messages, channels,
  properties, bounded exploration (`bounds:`).
- sipx-clstr (Rust, public): a clustered SIP proxy and registrar. Its own spec documents
  (`docs/specs/`, 13 documents) carry RFC-derived vector rows; `docs/reference/conformance.md` (checkout
  RG-19, commit 810536c): 218 of 619 rows proved, 16 shape only, 385 deferred. Two nodes share one
  registrar; no affinity tokens, trunks, media control, authentication or operator yet (its README).

## Three layers

1. **Ports** (the layer the owner named): one port per wire protocol. A port is the protocol's behaviour,
   specified from its RFCs, plus how the cloud exposes it.
   - Protocol spec: an `ess-protospec/1` document per protocol family, written from the RFC text,
     each property and message citing RFC and section: HTTP/1.1 (RFC 9110, 9112), HTTP/2 (RFC 9113),
     WebSocket (RFC 6455), TLS 1.3 (RFC 8446, as a transport property), SIP (RFC 3261: transactions
     §17 with timers A-K, dialogs §12, registrar §10, proxy §16; RFC 3263 locating servers; RFC 3581
     rport), SDP offer/answer (RFC 3264, RFC 8866), RTP/RTCP (RFC 3550).
   - RFC vectors: every MUST and SHOULD of a cited section is a row with an expected outcome, like
     sipx-clstr's ledger; a row is proved by a protospec property, a conformance scenario, or a test
     of an implementation.
   - Exposure, a new domain `weft.ingress` (layer 5): `Entrypoint` (protocol, transport UDP/TCP/TLS/
     WS/WSS/QUIC, address and port, certificate), `LoadBalancing` (per protocol: HTTP by route,
     SIP by Call-ID/dialog affinity, RTP by session), `Allowlist` (CIDR and per-protocol identities,
     default deny), rate limits. `weft-edge` becomes the ingress component for every port, not only
     HTTP.
2. **Adapters / managed products** (catalog, `ess/7-services/catalog.yaml`): products that terminate
   a port and speak its protocol for a tenant: SIP registrar, SIP proxy, PBX, later a WebRTC gateway
   and an RTVBP endpoint. Each product declares the ports it uses and is held to the port's RFC
   vectors. sipx-clstr is the first implementation of the registrar and proxy products.
3. **Sessions**: inside the cloud, workloads see protocol-neutral abstractions. `MediaSession`
   (participants, media streams, codecs, state: offered, active, held, ended), created by an adapter
   from SIP+SDP+RTP, RTVBP or WebRTC (ICE, DTLS-SRTP). A workload subscribes to or acts on media
   sessions; it never parses SIP. New domain `weft.media` (layer 7, beside catalog).

## sipx-clstr: conformance first, generation after

- Conformance: map sipx-clstr's vector rows onto the SIP port's RFC vectors; run sipx-clstr in
  weft-sim style deterministic experiments (it already proves behaviour in deterministic simulation,
  README) and against the protospec's explored traces.
- Generation: the transaction and dialog state machines of RFC 3261 (§17, §12) are finite with
  named timers; generate them from the protospec into Rust once the protospec is the reference, and
  let sipx-clstr adopt the generated layer behind its existing tests.

## Steps

1. Probe: write RFC 3261 §17.1.1 (INVITE client transaction, timers A, B, D) as `ess-protospec/1`;
   record what protospec cannot express (timers, retransmission over unreliable transport, message
   loss) as ESS asks.
2. Port documents: SIP (transactions, dialogs, registrar), HTTP/1.1, WebSocket; RFC vector ledger per
   port with a check that every row is proved, shape-only or deferred with a live story (as
   sipx-clstr's `check-vectors`).
3. `weft.ingress`: Entrypoint, LoadBalancing, Allowlist; `weft-edge` owns it.
4. Catalog products: SIP registrar, SIP proxy, PBX, each naming its ports.
5. `weft.media`: MediaSession and the adapter contract (SIP, RTVBP, WebRTC).
6. sipx-clstr conformance against the SIP port; then generation of the transaction layer.

## Decisions (the owner's)

- D1, name of layer 1:
  - A (recommended): "ports", with "adapters" for layer 2 and "sessions" for layer 3 (the ports-and-
    adapters pattern: a port is the protocol boundary, an adapter maps it to the domain).
  - B: "wire layer".
  - C: "protocol edge".
- D2, where the RFC port specs live:
  - A (recommended for now): `weft/protocols/rfc/`, next to weft's own protospecs; moved later if
    another product needs them.
  - B: a repository of their own shared by weft, sipx-clstr and other products (a new repository:
    needs your word).
- D3, sipx-clstr:
  - A (recommended): conformance first, generation of the transaction layer after the SIP port is the
    reference.
  - B: generation first.

## Open questions (default taken until answered)

1. Media relay (RTP) inside weft-edge or as a product? Default: a product (media relay), the port
   spec in layer 1.
2. SIP over WebSocket (RFC 7118) as its own port? Default: a transport of the SIP port.
3. STUN/TURN (RFC 8489, 8656) for WebRTC: part of the WebRTC gateway product. Default: yes.
