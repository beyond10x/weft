---
format: aep.planning-md/3
id: story:agent-package-settings
kind: story
status: draft
title: 'Spec: the settings of an Agent package for the Job, Schedule and Service shapes'
relations:
- decomposes: epic:agent-workloads
- serves: vision:weft
revision: 1
---
## Epic

Decomposes `epic:agent-workloads`, outcome 1 (one agent is a workload with the shapes Job,
Schedule and Service). The specification change only; `ess/` and `docs/domain-sketch.md` are
written when this story is implemented.

## Context

- The owner, 2026-11-28 about 02:08 CET: "a job as a workload that is an agent; a scheduled agent,
  a persistent agent" (epic § Order).
- `Agent` is a spec kind (`ess/4-applications/workloads.yaml:35`, `docs/domain-sketch.md:241-243`:
  "becomes an agent runtime instance, its tools, its model access and its sandbox for tool
  effects"). A `Workload` holds plain kinds only (`workloads.yaml:415-418`); `DefineWorkload`
  refuses a spec kind with `SpecKindNotRunnable` (line 2099). The agent's settings therefore
  describe the plain component it becomes.
- Shapes today: `ShapeKind` (line 43), `JobSettings` (52-63), `ServiceSettings` (68-72),
  `ScheduleSettings` (109-122, calendar syntax `# OPEN:` cron or RRULE, line 112), `ScheduledShape`
  (105-107). An agenticspec schedule has a fixed interval (`every`, `next_fire_at`) and an overlap
  policy (`skip`, `queue`, `parallel` with `max_parallel`; agenticspec
  `architectures/scheduled-loops.yaml`), which `ScheduleSettings` cannot say.
- `SessionKind::AgentSandbox` (line 84) is an interactive sandbox a person attaches to; it stays
  apart from this package.

## The specification change

1. `ess/4-applications/workloads.yaml`, a new struct `weft.workloads.AgentSettings` (after line
   122):
   - `document: weft.shared.ArtifactDigest`: the agent document, an artifact.
   - `runtime: weft.shared.ArtifactDigest`: the agent runtime that runs the document (an image or a
     process root filesystem); weft ships none of its own.
   - `task: Optional<String>`: the task a Job run starts with (Job and Schedule); absent for a
     Service, which takes its input over its links and endpoints.
   - `model_routes: List<String>`: the model components or egress destinations the agent may call;
     the plain component gets links or egress for these and no other.
   - `max_model_calls: Optional<Integer>`, `max_duration: Optional<Duration>`: the run's bounds; a
     run over either ends `Failed`.
2. Shapes an `Agent` package takes: `Job`, `Schedule` (starts `Job`), `Service`. Any other shape is
   refused with a new error `weft.workloads.AgentShapeNotAdmitted` (Invocation, Box, Session,
   Daemon).
3. `weft.workloads.ScheduleSettings`: a field `overlap: weft.workloads.ScheduleOverlap`, enum
   `[Skip, Queue, Parallel]`, and `max_parallel: Integer` (at least 1; 1 unless `Parallel`). The
   `# OPEN:` of line 112 is decided: `calendar` is a cron expression or `@every <duration>`, so a
   fixed interval maps without loss.
4. Plain component of an `Agent` package (written into `docs/domain-sketch.md:241-243`): package
   `Image` or `Process` from `runtime`; the document mounted read-only as a `Config`; `task` and the
   bounds as process arguments; the same shape and `ShapeSettings`; for `Service`, `stateful`
   gives each replica a volume for the agent's own state, and a `SleepPolicy` lets an idle agent
   sleep until a call wakes it (`docs/domain-sketch.md` "Sleep and wake").
5. `weft.workloads.Workload` (line 394): a field `agent: Optional<weft.workloads.AgentSettings>`,
   set when the workload is the plain component of an `Agent` package, shown in
   `weft.workloads.WorkloadDetail` and `WorkloadList`, so a console finds agents without parsing
   processes (`story:console-agent-architecture`).

## Scope

- `ess/4-applications/workloads.yaml`, `docs/domain-sketch.md`

## Acceptance

`ess specify validate --path ess --strict-requires` passes with `weft.workloads.AgentSettings`,
`weft.workloads.ScheduleOverlap`, `weft.workloads.AgentShapeNotAdmitted` and the fields
`overlap` and `max_parallel` of `weft.workloads.ScheduleSettings` in `ess/`, and no `# OPEN:` is
left on `ScheduleSettings.calendar`.
