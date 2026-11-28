//! The adapters of the placement `local`: this host (cloudspec's [`docs/platform-ports.md`](https://github.com/beyond10x/cloudspec/blob/2ed72c2866bbd165da2de75250b2bfcb340a96ae/docs/platform-ports.md) § 8, § 12).
//! [`stub`] is test support: an in-memory adapter of the `workloads` port that reaches no place.

pub mod substrate;

/// Test support: an in-memory adapter of the [`Workloads`](crate::node::Workloads) port. It
/// reaches no place and runs nothing; callers' tests drive it through the port and through its
/// hooks, which make a start fail, a command exit with a code and a stop exceed its grace.
///
/// Each call of [`observe`](crate::node::Workloads::observe) moves a workload at most one step:
/// `starting` to `running`; `restarting` to `running` once the restart's delay passed and
/// `restarting` was answered once, by [`restart`](crate::node::Workloads::restart) or an earlier
/// observe. An exit set with [`StubWorkloads::exit`] is applied at once: the spec's restart rule
/// decides between `exited` and `restarting`. A stop ends at once, within the grace unless
/// [`StubWorkloads::exceed_grace`] was set for the key.
pub mod stub {
    use std::collections::{BTreeMap, BTreeSet};
    use std::sync::{Mutex, MutexGuard, PoisonError};
    use std::time::{Duration, Instant};

    use crate::node::{
        Observation, ProcessExit, StartFailure, StopEnd, StopRule, WorkloadError, WorkloadKey,
        WorkloadSpec, WorkloadState, Workloads,
    };
    use crate::Refusal;

    /// What the stub did, in order: what a caller's test reads to see that the port was reached.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum StubEvent {
        /// A start took effect with this spec.
        Started {
            key: WorkloadKey,
            spec: WorkloadSpec,
        },
        /// Prestart step `step` (from 0) ran.
        Prestart {
            key: WorkloadKey,
            step: usize,
            privileged: bool,
        },
        /// The command started.
        Command { key: WorkloadKey },
        /// The command ended on its own.
        Exited { key: WorkloadKey, exit: ProcessExit },
        /// A start again began: by [`Workloads::restart`] or by the spec's restart rule.
        Restarting { key: WorkloadKey },
        /// A stop reached the stub with this rule and ended so.
        Stopped {
            key: WorkloadKey,
            rule: StopRule,
            end: StopEnd,
        },
    }

    struct Entry {
        spec: WorkloadSpec,
        state: WorkloadState,
        last_exit: Option<ProcessExit>,
        restarts: u32,
        stopped: Option<StopEnd>,
        failure: Option<StartFailure>,
        restart_at: Instant,
        /// `restarting` was answered to a caller since the workload entered it.
        restart_answered: bool,
    }

    impl Entry {
        fn observation(&self) -> Observation {
            Observation {
                state: self.state,
                last_exit: self.last_exit,
                restarts: self.restarts,
                stopped: self.stopped,
                failure: self.failure.clone(),
            }
        }
    }

    #[derive(Default)]
    struct Inner {
        entries: BTreeMap<WorkloadKey, Entry>,
        refusals: BTreeMap<WorkloadKey, WorkloadError>,
        failures: BTreeMap<WorkloadKey, StartFailure>,
        past_grace: BTreeSet<WorkloadKey>,
        events: Vec<StubEvent>,
    }

    impl Inner {
        /// Runs the prestart steps and the command of `key`, or fails as a hook says; on success
        /// the workload is in `reached`.
        fn launch(&mut self, key: &WorkloadKey, reached: WorkloadState) {
            let failure = self.failures.remove(key);
            let Some(entry) = self.entries.get_mut(key) else {
                return;
            };
            let failed_step = match &failure {
                Some(StartFailure::Prestart { step, .. }) => {
                    assert!(
                        *step < entry.spec.prestart.len(),
                        "the stub's start of {key} is set to fail prestart step {step}, but the spec has {} steps",
                        entry.spec.prestart.len()
                    );
                    Some(*step)
                }
                _ => None,
            };
            for (step, prestart) in entry.spec.prestart.iter().enumerate() {
                self.events.push(StubEvent::Prestart {
                    key: key.clone(),
                    step,
                    privileged: prestart.privileged,
                });
                if failed_step == Some(step) {
                    break;
                }
            }
            entry.stopped = None;
            if failure.is_some() {
                entry.state = WorkloadState::Failed;
                entry.failure = failure;
                return;
            }
            entry.failure = None;
            entry.state = reached;
            self.events.push(StubEvent::Command { key: key.clone() });
        }

        fn entry(&mut self, key: &WorkloadKey) -> Result<&mut Entry, WorkloadError> {
            self.entries
                .get_mut(key)
                .ok_or_else(|| WorkloadError::UnknownWorkload(key.clone()))
        }

        /// Puts `key` in `restarting`, to run again once `delay` passed; `answered`: the caller
        /// gets this state as its answer.
        fn restarting(&mut self, key: &WorkloadKey, delay: Duration, answered: bool) {
            if let Some(entry) = self.entries.get_mut(key) {
                entry.restarts += 1;
                entry.state = WorkloadState::Restarting;
                entry.stopped = None;
                entry.restart_at = Instant::now() + delay;
                entry.restart_answered = answered;
                self.events.push(StubEvent::Restarting { key: key.clone() });
            }
        }
    }

    /// The in-memory [`Workloads`] adapter. It serves privileged prestart steps unless built
    /// with [`StubWorkloads::without_privileged`].
    pub struct StubWorkloads {
        inner: Mutex<Inner>,
        privileged: bool,
    }

    impl Default for StubWorkloads {
        fn default() -> Self {
            Self::new()
        }
    }

    impl StubWorkloads {
        pub fn new() -> Self {
            Self {
                inner: Mutex::new(Inner::default()),
                privileged: true,
            }
        }

        /// A stub that cannot run a privileged prestart step: such a start is refused
        /// `NotServed` with the operation `prestart.privileged`.
        pub fn without_privileged() -> Self {
            Self {
                privileged: false,
                ..Self::new()
            }
        }

        fn lock(&self) -> MutexGuard<'_, Inner> {
            self.inner.lock().unwrap_or_else(PoisonError::into_inner)
        }

        /// Hook: the next start of `key` is refused with `error`; nothing runs.
        pub fn refuse_start(&self, key: &WorkloadKey, error: WorkloadError) {
            self.lock().refusals.insert(key.clone(), error);
        }

        /// Hook: the next start of `key`, by [`Workloads::start`] or a start again, fails so and
        /// ends in `failed`.
        ///
        /// # Panics
        ///
        /// At that start, when `failure` names a prestart step the spec does not have.
        pub fn fail_start(&self, key: &WorkloadKey, failure: StartFailure) {
            self.lock().failures.insert(key.clone(), failure);
        }

        /// Hook: the command of `key` ends on its own with `exit`; the spec's restart rule
        /// decides between `exited` and `restarting`.
        ///
        /// # Panics
        ///
        /// When `key` is not starting or running.
        pub fn exit(&self, key: &WorkloadKey, exit: ProcessExit) {
            let mut inner = self.lock();
            let entry = inner
                .entries
                .get_mut(key)
                .unwrap_or_else(|| panic!("the stub holds no workload {key}"));
            assert!(
                matches!(
                    entry.state,
                    WorkloadState::Starting | WorkloadState::Running
                ),
                "the command of {key} does not run: {:?}",
                entry.state
            );
            entry.last_exit = Some(exit);
            let rule = entry.spec.restart.clone();
            inner.events.push(StubEvent::Exited {
                key: key.clone(),
                exit,
            });
            if rule.restarts_after(exit) {
                inner.restarting(key, rule.delay, false);
            } else if let Some(entry) = inner.entries.get_mut(key) {
                entry.state = WorkloadState::Exited;
            }
        }

        /// Hook: the next stop of `key` runs out of its grace; the kill reaches the rule's scope.
        pub fn exceed_grace(&self, key: &WorkloadKey) {
            self.lock().past_grace.insert(key.clone());
        }

        /// Everything the stub did, in order.
        pub fn events(&self) -> Vec<StubEvent> {
            self.lock().events.clone()
        }
    }

    impl Workloads for StubWorkloads {
        fn start(
            &self,
            key: &WorkloadKey,
            spec: &WorkloadSpec,
        ) -> Result<Observation, WorkloadError> {
            spec.check()?;
            if !self.privileged && spec.prestart.iter().any(|step| step.privileged) {
                return Err(WorkloadError::Shared(Refusal::NotServed {
                    operation: "prestart.privileged".to_owned(),
                }));
            }
            let mut inner = self.lock();
            if let Some(error) = inner.refusals.remove(key) {
                return Err(error);
            }
            let active = inner.entries.get(key).filter(|entry| {
                matches!(
                    entry.state,
                    WorkloadState::Starting | WorkloadState::Running | WorkloadState::Restarting
                )
            });
            if let Some(entry) = active {
                return Ok(entry.observation());
            }
            let last_exit = inner.entries.get(key).and_then(|entry| entry.last_exit);
            inner.entries.insert(
                key.clone(),
                Entry {
                    spec: spec.clone(),
                    state: WorkloadState::Starting,
                    last_exit,
                    restarts: 0,
                    stopped: None,
                    failure: None,
                    restart_at: Instant::now(),
                    restart_answered: false,
                },
            );
            inner.events.push(StubEvent::Started {
                key: key.clone(),
                spec: spec.clone(),
            });
            inner.launch(key, WorkloadState::Starting);
            Ok(inner.entry(key)?.observation())
        }

        fn stop(&self, key: &WorkloadKey, rule: &StopRule) -> Result<Observation, WorkloadError> {
            let mut inner = self.lock();
            let running = matches!(
                inner.entry(key)?.state,
                WorkloadState::Starting | WorkloadState::Running
            );
            let past_grace = inner.past_grace.remove(key);
            let end = match (running, past_grace) {
                (false, _) => StopEnd::NotRunning,
                (true, false) => StopEnd::WithinGrace,
                (true, true) => StopEnd::Killed(rule.kill),
            };
            let entry = inner.entry(key)?;
            entry.state = WorkloadState::Stopped;
            entry.stopped = Some(end);
            let observation = entry.observation();
            inner.events.push(StubEvent::Stopped {
                key: key.clone(),
                rule: *rule,
                end,
            });
            Ok(observation)
        }

        fn restart(&self, key: &WorkloadKey) -> Result<Observation, WorkloadError> {
            let mut inner = self.lock();
            inner.entry(key)?;
            inner.restarting(key, Duration::ZERO, true);
            Ok(inner.entry(key)?.observation())
        }

        fn observe(&self, key: &WorkloadKey) -> Result<Observation, WorkloadError> {
            let mut inner = self.lock();
            let entry = inner.entry(key)?;
            match entry.state {
                WorkloadState::Starting => entry.state = WorkloadState::Running,
                WorkloadState::Restarting if !entry.restart_answered => {
                    entry.restart_answered = true;
                }
                WorkloadState::Restarting if Instant::now() >= entry.restart_at => {
                    inner.launch(key, WorkloadState::Running);
                }
                _ => {}
            }
            Ok(inner.entry(key)?.observation())
        }
    }
}
