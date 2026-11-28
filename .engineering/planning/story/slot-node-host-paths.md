---
format: aep.planning-md/3
id: story:slot-node-host-paths
kind: story
status: active
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
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-28T16:06:47Z", actor: "human:operator", revision: 5, decided_on: {"recorded":{"approval":1}}}
- {from: "proposed", to: "active", at: "2026-11-28T16:06:47Z", actor: "human:operator", revision: 6, decided_on: {"recorded":{"approval":1}}}
---
## Epic

Decomposes `epic:slot-node-daemon`, outcome 3 and the second step of its Order (host paths and
data). cloudspec rules this lets cloudspec map in `generate weft` (cloudspec
`59129eb:crates/cloudspec/src/generate/weft/runs.rs`), with the items of slot's architecture
(`beyond10x/slot` `docs/design/cloudspec/slot.architecture.yaml`, run 2026-11-28):

- `weft_host_path_unsupported`, 4: `slot-daemon.volumes[0]` (`${home}/`), `slot-daemon.volumes[1]`
  (`${home}/.local/state/slot`), `node-daemon.volumes[0]` (`/var/lib/slot-build`),
  `node-daemon.volumes[1]` (`/srv/slot`); each `mount: same`, `mode: rw`.
- `weft_deployer_path_unsupported`, 8: `slot-daemon.command[0]` (`${home}/.local/bin/slot`),
  `slot-daemon.environment.PATH` (four `${home}/` entries), `slot-daemon.volumes[0].host_path`,
  `slot-daemon.volumes[1].host_path`, `slot-daemon.ports[1].unix` (`${runtime_dir}/slot.sock`),
  `slot-daemon.ports[2].fifo` (`${home}/.local/state/slot/jobserver.fifo`),
  `toolchain-refresh.command[1]` (`${home}/.local/share/slot/toolchains-refresh.sh`),
  `toolchain-refresh.environment_files[0].path` (`${home}/.config/slot/node/toolchains-refresh.env`).
- `weft_data_link_unsupported`, 2: `links[3]` (agent-session → node-daemon, carries `tree`,
  `protocol: ssh`, `data: push`), `links[4]` (node-daemon → agent-session, carries `products`,
  `data: pull`).

14 of 44. The socket and FIFO ports themselves stay `weft_socket_unsupported` (network story); this
story resolves their paths only. `environment_files` stays its own single rule; this story resolves
its path only.

## Dependencies

- `story:slot-node-process` (merged): the process workload, `run_as`, the Workloads port.
- `story:slot-node-pool-placement` (merged): the node an instance is placed on, and the
  instance-creation step where `${instance}` is written; the deployer words are written at the same
  step, for the same node.
- Substrate gap 6 (`story:read-write-volume-kept-across-runs`) is not needed: every host path of
  slot is `mount: same` on a process of isolation None, which sees the host without a mount
  (`decision-blocker:slot-node-isolation-none-family`, D-S1 = A).

## Outcome

1. A `volumes` entry `{host_path, mount: same, mode}` of a process part is planned on its workload
   (`weft.workloads.Volume`, kind host path) and checked on the node before the start: the
   directory exists, or is made with the process user as owner, and the process user can read it,
   and write it under `rw`. A check that fails stops the start, like a failing prestart step, and
   names the path. `mount` other than `same` on isolation None is refused by name at apply.
2. `run_as: {user: deployer}` resolves, on the node the instance is placed on, to the host user that
   node reports as its deployer (a node fact beside its capacity: user name, home, runtime
   directory). `${home}` and `${runtime_dir}` in command words, environment values, `host_path`,
   socket and FIFO paths and environment-file paths are written with that user's home and runtime
   directory at instance creation, as `${instance}` is. A node that reports no deployer gets no
   instance of a workload that names one, and the instance names why.
3. A link with `data: push|pull` and `protocol: ssh` is recorded on the formation with its
   direction, its carrier and its two ends; nothing moves data in this story (the transport is the
   network story).

## Domain relations

- `weft.workloads.Volume` (host path) → `weft.workloads.Workload`, many-to-one, the workload owns
  it: inferable from `ess/cloud/5-workloads/workloads.yaml` (volumes of a workload).
- node deployer fact → `weft.infrastructure.Node`, one-to-one: requires a field or fact the vendored
  `Node` does not have; inferable as a node fact (`weft_platform::node::Facts`) until cloudspec
  models it.
- data link → `weft.architecture.Link` (or the formation's link rows), with `direction`
  `push|pull`: requires-stakeholder-input if the vendored link has no direction field (cloudspec).

## Scope

`product/cloud/crates/weft-bin/src/cloudspec_apply.rs` (volumes, the deployer words, the links),
`product/cloud/crates/weft-impl/src/behaviours/create_instance.rs` and
`product/cloud/crates/weft-impl/src/placement.rs` (word resolution at instance creation),
`product/cloud/crates/weft-platform/src/node.rs` (the deployer fact; the host path check before a
start), `product/cloud/crates/weft-agent/src/follow/supervise.rs` (the check before the start),
`product/cloud/crates/weft-testkit/`.

## Acceptance

A submission with one placed `kind: process` part with `run_as: {user: deployer}`, command word
`${home}/bin/tool`, environment `PATH: ${home}/bin:/usr/bin`, a unix port path
`${runtime_dir}/x.sock` and two volumes `{host_path: ${home}/state, mount: same, mode: rw}` and
`{host_path: /srv/x, mount: same, mode: ro}`, on a node reporting deployer `u` with home `/var/lib/u`
and runtime directory `/run/user/1000`, is applied and its instance carries `/var/lib/u/bin/tool`,
`PATH=/var/lib/u/bin:/usr/bin`, `/run/user/1000/x.sock`, `/var/lib/u/state` (rw) and `/srv/x` (ro); it
reaches Running through the stub Workloads port after both paths were checked; a missing `rw`
directory is made, an unwritable one stops the start with its path named; a node with no deployer
gets no instance. The formation of slot's architecture records its two data links with `push` and
`pull`. A `mount: /elsewhere` on isolation None is refused at apply by name.

## Rules for this work

- Builds and tests through slot (`cargo` through its shim). One integration-test binary per crate.
  No test over 60 s (the runner kills it); the test step within 120 s; a test over 5 s is named
  with its time. A pool job ends within 10 minutes unless it names its time with `slot --timeout`
  (at most 30m).
- The test step makes no network request, makes no machine and starts no `substrate-daemon`: the
  Workloads port is the stub; the path checks run against a temporary directory.
- No Python in anything committed.
