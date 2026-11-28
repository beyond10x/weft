---
format: aep.planning-md/3
id: story:weft-run-ephemeral
kind: story
status: draft
title: '`weft run`: a control plane and one node in the foreground on placement local, torn down on exit'
relations:
- decomposes: epic:platform-ports
- serves: vision:weft
- depends_on: story:ports-local-machines
- depends_on: story:ports-machines-remove
- depends_on: story:ports-cloud-edge-local
- depends_on: story:skeleton-end-to-end
revision: 1
---
## Epic

Decomposes `epic:platform-ports`: the placement `local` used as one command. Depends on
`story:ports-local-machines` (a node is a `weft-agent` process the machines port makes),
`story:ports-machines-remove` (a machine without a node is removed), `story:ports-cloud-edge-local`
(weft-edge on a loopback listener) and `story:skeleton-end-to-end` (a placed snippet runs and its
instance reads Running).

## Context

- The owner wants a one-command form: `weft run <cloudspec architecture | workload ...>`; later
  `weft run architecture.yaml --binding local.yaml`, which has cloudspec translate the source,
  admits it and runs it (cloudspec epic `translation`; `story:deployments-admit`).
- Placement `local`: "one process is control plane and worker; single-user access"
  (`docs/domain-sketch.md:713`); "In the `local` placement all four server components run in one
  process" (`docs/domain-sketch.md:80`).
- In the foreground: the control host and one node, every port on its `local` adapter
  (`docs/platform-ports.md` § 8, column `local`), port bindings on loopback only, state in a
  temporary directory, a join token the command issues to itself. While up it serves the same HTTP
  views on loopback. On exit (end of the workload, or SIGINT/SIGTERM) it tears down every machine
  through the machines port and removes the state directory.
- First form: `weft run <workload ...>` with a snippet; the architecture form follows cloudspec's
  translator.

## Scope

- `product/cloud/crates/weft-bin/` (the `run` subcommand), `product/cloud/crates/weft-testkit/`
  (a test that runs it as a child process)

## Acceptance

1. One command `weft run --snippet 'sh -c "echo weft; sleep 30"' --port 8080` brings its instance
   to `Running` in `weft.workloads.Instances`, answered on its loopback HTTP views.
2. A request on its loopback port binding reaches the instance.
3. After SIGTERM to the command, no `weft` or `weft-agent` process of the run remains and its
   state directory does not exist; the same holds after the workload ends on its own.

## Rules for this work

- Builds and tests run through `slot`. No test over 60 s; the test step ends within 120 s; a test
  over 5 s is named with its time. A pool job ends within 10 minutes unless it names its time with
  `slot --timeout` (at most 30m). No Python in anything committed. No network request beyond
  loopback in the test step.
