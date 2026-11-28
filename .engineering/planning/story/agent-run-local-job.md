---
format: aep.planning-md/3
id: story:agent-run-local-job
kind: story
status: draft
title: An Agent package runs as a Job, and on a Schedule, on the local placement
relations:
- decomposes: epic:agent-workloads
- serves: vision:weft
- depends_on: story:agent-package-settings
- depends_on: story:skeleton-node-run-assignment
- depends_on: story:ports-node-exec-facts
revision: 2
---
## Epic

Decomposes `epic:agent-workloads`, outcome 1 and acceptance 2: one `Agent` package runs as a Job
on the `local` placement. Depends on `story:agent-package-settings` (the settings it runs from),
`story:skeleton-node-run-assignment` (weft-agent follows an assignment and records the instance
running) and `story:ports-node-exec-facts` (the `exec` port a process that ends runs through,
`cloudspec@2ed72c2:docs/platform-ports.md` § 8). A persistent agent (shape Service) needs a process that runs until
stopped: it waits for `story:ports-node-workloads-local` and is not part of this story.

## Context

- The plain component of an `Agent` package (`story:agent-package-settings`, item 4): a process
  from the `runtime` artifact, the document mounted read-only, the task and bounds as arguments,
  shape Job.
- Job lifecycle `Pending -> Running -> Succeeded | Failed | Cancelled`
  (`ess/4-applications/workloads.yaml:931-1008`), read in `weft.workloads.Jobs`.
- The test step makes no model call and no network request (`~/beyond10x/AGENTS.md` § Tests stay
  fast): the runtime in the test is a fixture: a POSIX sh script under the crate's tests that reads
  the document and the task, answers from a recorded model reply in a file, and writes one output
  line. A real agent runtime is a later artifact.
- Schedule: once a Job runs, a Schedule with `@every` starts the same Job; the test drives the
  clock through the control host's time input, not by waiting.

## Scope

- `product/cloud/crates/weft-agent/` (start the process of an agent Job; report its exit)
- `product/cloud/crates/weft-impl/` (the behaviours of `weft.workloads` Job start and end that the
  path needs, each in its own file under `src/behaviours/`)
- `product/cloud/crates/weft-testkit/` (seeded agent workload, fixture runtime)

## Acceptance

1. With a seeded `Agent` workload of shape Job (document and fixture runtime as artifacts, task
   `say done`), weft-agent on placement `local` starts its process, the process exits 0, the
   report file holds its output line, and its Job reads `Succeeded` in `weft.workloads.Jobs`.
2. With `max_duration` 1 s and a fixture runtime that sleeps 5 s, the Job reads `Failed`.
3. A Schedule of that workload with `@every 1m`, after the control host's clock is moved by 1 min,
   has `last_fired_at` set in `weft.workloads.ScheduleList` and one more `Succeeded` Job.

## Rules for this work

- Builds and tests run through `slot` (`cargo` is shimmed; anything else that compiles or tests
  carries the prefix). No test over 60 s; the test step ends within 120 s; a test over 5 s is named
  with its time.
- A pool job ends within 10 minutes unless it names its time with `slot --timeout` (at most 30m).
- No Python in anything committed.
- No model call, no network request in the test step.
