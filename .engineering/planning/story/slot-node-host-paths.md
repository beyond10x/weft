---
format: aep.planning-md/3
id: story:slot-node-host-paths
kind: story
status: draft
title: Host paths, deployer paths and data links of an architecture run on the node (outline)
relations:
- decomposes: epic:slot-node-daemon
- serves: vision:weft
- depends_on: story:slot-node-process
- depends_on: story:slot-node-pool-placement
scope:
- confidence: inferred
  path: product/cloud/crates/weft-agent/src
- confidence: inferred
  path: product/cloud/crates/weft-cloudspec/src
- confidence: inferred
  path: product/cloud/crates/weft-impl/src/behaviours
revision: 3
---
## Epic

Decomposes `epic:slot-node-daemon`, outcome 3 and the second step of its Order (host paths and
data). Outline: to be scoped before it is proposed. cloudspec rules this lets cloudspec map:
`weft_host_path_unsupported` (4 refusals of slot), `weft_deployer_path_unsupported` (8),
`weft_data_link_unsupported` (2): 14 of 44. PlanFromSource is needed first, through
`story:slot-node-process` (`story:cloudspec-submission-plan`).

## Substrate

Gap 6 (`substrate` `story:read-write-volume-kept-across-runs`) is draft, not implemented. slot's
architecture needs none of it: it has no `resources` volume part (row `VOLUME`, 0 refusals); its
four volumes are host directories `mount: same` of processes with isolation None, which see the
host without a mount. Gap 6 is needed only if `decision-blocker:slot-node-isolation-none-family`
is answered B.

## Outcome

1. A `volumes` entry `{host_path, mount: same, mode}` is planned on the workload and checked on the
   node: the directory exists (or is made) and the process user can read it, and write it under `rw`.
2. `run_as: {user: deployer}` resolves to the user who deployed, and `${home}/`, `${runtime_dir}/`
   in command words, environment values, socket paths, `host_path` and `mount` resolve to that
   user's home and runtime directory on the node the instance is placed on
   (`story:slot-node-pool-placement`).
3. A link with `data: push|pull` (`protocol: ssh`) is recorded with its direction and carrier; its
   transport is reached as in `story:slot-node-network`.

## Scope

Worked out: `product/cloud/crates/weft-cloudspec/src/`, `product/cloud/crates/weft-impl/src/behaviours/`,
`product/cloud/crates/weft-agent/src/`.

## Acceptance

A planned process with `run_as: deployer`, a `${home}/` command word and two `mount: same` host
paths runs on its node with each path resolved to the deployer's home there and written under
`rw`, and the two data links of slot are recorded with their directions.

## Rules for this work

- Builds and tests through slot (`cargo` through its shim). One integration-test binary per crate.
  No test over 60 s (the runner kills it); the test step within 120 s; a test over 5 s is named
  with its time. A pool job ends within 10 minutes unless it names its time with `slot --timeout`
  (at most 30m).
- No network request in the test step. No Python in anything committed.
