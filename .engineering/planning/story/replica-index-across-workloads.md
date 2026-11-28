---
format: aep.planning-md/3
id: story:replica-index-across-workloads
kind: story
status: draft
title: The replica level step picks an index free across the whole service
relations:
- serves: vision:weft
scope:
- confidence: cited
  path: product/cloud/crates/weft-controllers/src
- confidence: cited
  path: product/cloud/crates/weft-controllers/tests/main.rs
revision: 3
---
## Found

Pre-existing defect, found by `story:cloud-blocks-rust` while an adversary fixture repointed a
service at a new workload. The replica level step picks the lowest free index from the instances
of the service's current workload, while `weft.workloads.CreateInstance` guard `replica-live`
checks across the whole service. When a service's workload changes while replica 0 of the old
workload is live, the step requests index 0 again, binding `r1-create-replica` drops the request
as `replica-live`, and `running_replicas` was already raised: the service waits for a replica
that is never created.

## Outcome

The replica level step picks the lowest index free across every live instance of the service, so
a workload change never requests an index that is live.

## Scope

`product/cloud/crates/weft-controllers/src/` (the replica level step),
`product/cloud/crates/weft-controllers/tests/main.rs`.

## Acceptance

1. A controller test: a service with replica 0 live on workload A is repointed at workload B; the
   step requests index 1, and `CreateInstance` answers created (red before the fix).
2. `slot --here task test` exits 0 within 120 s.
