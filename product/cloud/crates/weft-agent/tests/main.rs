//! The one integration-test binary of weft-agent.

/// `confine`: one process of an instance runs as one exec of the `substrate-daemon` the setup hook
/// `tools/test-setup.d/20-substrate.sh` started for this test run; its socket path comes from the
/// environment (`WEFT_SUBSTRATE_SOCKET`).
mod confine {
    use std::io::ErrorKind;
    use std::net::{SocketAddr, TcpListener};
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use weft_agent::confine::{Confine, ExecState, HostFile, Process};

    const SOCKET_VARIABLE: &str = "WEFT_SUBSTRATE_SOCKET";
    const PROBE: &str = env!("CARGO_BIN_EXE_weft-probe");
    const PROBE_MOUNT: &str = "/opt/weft/weft-probe";

    fn socket() -> PathBuf {
        let path = std::env::var_os(SOCKET_VARIABLE).unwrap_or_else(|| {
            panic!("{SOCKET_VARIABLE} is unset: the setup hook tools/test-setup.d/20-substrate.sh starts the daemon")
        });
        let path = PathBuf::from(path);
        assert!(
            path.exists(),
            "{SOCKET_VARIABLE}={} does not exist: no substrate-daemon serves it",
            path.display()
        );
        path
    }

    fn block_on<F: std::future::Future>(future: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("a tokio runtime builds")
            .block_on(future)
    }

    /// Accepts every connection waiting on a non-blocking listener and returns how many there were.
    fn accept_waiting(listener: &TcpListener) -> usize {
        let mut accepted = 0;
        loop {
            match listener.accept() {
                Ok(_) => accepted += 1,
                Err(error) if error.kind() == ErrorKind::WouldBlock => return accepted,
                Err(error) => panic!("accept on the probe listener failed: {error}"),
            }
        }
    }

    fn run(process: &Process) -> (Vec<ExecState>, weft_agent::confine::Exited) {
        let mut states = Vec::new();
        let exited = block_on(async {
            let confine = Confine::connect(&socket())
                .await
                .expect("the daemon answers");
            confine.run(process, |state| states.push(state)).await
        })
        .expect("the exec runs to exec.exited");
        (states, exited)
    }

    #[test]
    fn a_snippet_runs_confined_and_its_exit_and_output_are_observed() {
        let (states, exited) = run(&Process::new(["sh", "-c", "echo weft"]));
        assert_eq!(
            states.first(),
            Some(&ExecState::Accepted),
            "states: {states:?}"
        );
        assert_eq!(
            states.last(),
            Some(&ExecState::Exited),
            "states: {states:?}"
        );
        assert!(
            states.len() >= 2,
            "the exec never left accepted: {states:?}"
        );
        assert_eq!(
            exited.code,
            Some(0),
            "stderr: {}",
            String::from_utf8_lossy(&exited.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&exited.stdout), "weft\n");
    }

    #[test]
    fn the_probe_reaches_host_loopback_unconfined_and_not_from_the_sandbox() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback listener binds");
        listener
            .set_nonblocking(true)
            .expect("the listener turns non-blocking");
        let address: SocketAddr = listener.local_addr().expect("the listener has an address");
        let target = address.to_string();

        // Positive control: the same binary and arguments, unconfined, reach the listener.
        let unconfined = Command::new(PROBE)
            .arg(&target)
            .output()
            .expect("the probe starts unconfined");
        let unconfined_report = String::from_utf8_lossy(&unconfined.stdout).into_owned();
        assert!(
            unconfined_report.starts_with("connected "),
            "the probe cannot connect even unconfined, so it proves no confinement: {unconfined_report:?} {:?}",
            String::from_utf8_lossy(&unconfined.stderr)
        );
        assert_eq!(
            accept_waiting(&listener),
            1,
            "the unconfined probe made one connection"
        );

        // The confined run: the same binary, bound read-only into the exec, the same arguments.
        let probe = Path::new(PROBE)
            .canonicalize()
            .expect("the probe binary has a canonical path");
        let process =
            Process::new([PROBE_MOUNT, target.as_str()]).file(HostFile::new(probe, PROBE_MOUNT));
        let (states, exited) = run(&process);
        let confined_report = String::from_utf8_lossy(&exited.stdout).into_owned();
        assert_eq!(
            states.last(),
            Some(&ExecState::Exited),
            "states: {states:?}"
        );
        assert!(
            confined_report.starts_with("refused ") || confined_report.starts_with("unreachable "),
            "the confined probe did not report refused or unreachable: {confined_report:?} (exit {:?}, stderr {:?})",
            exited.code,
            String::from_utf8_lossy(&exited.stderr)
        );
        assert_eq!(
            accept_waiting(&listener),
            0,
            "the sandbox reached the host loopback listener"
        );
    }

    /// Adversary: a node runs more than four processes at once. `Confine::run` holds one event
    /// stream of the daemon per run, and substrate 0.29.0 grants four streams per subject
    /// (`EventStreamPolicy::production`, `streams_per_subject: 4`): the fifth concurrent run is
    /// refused (`event.stream-capacity`) before its process starts.
    #[test]
    fn five_processes_run_at_once_on_one_daemon() {
        const RUNS: usize = 5;
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(RUNS));
        let threads: Vec<_> = (0..RUNS)
            .map(|n| {
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    let script = format!("sleep 2; echo run{n}");
                    let process = Process::new(["sh", "-c", script.as_str()]);
                    block_on(async {
                        let confine = Confine::connect(&socket()).await?;
                        barrier.wait();
                        confine.run(&process, |_| {}).await
                    })
                })
            })
            .collect();
        let ended: Vec<_> = threads
            .into_iter()
            .map(|thread| thread.join().expect("a run thread ends"))
            .collect();
        for (n, end) in ended.iter().enumerate() {
            match end {
                Ok(exited) => {
                    assert_eq!(exited.code, Some(0), "run {n}");
                    assert_eq!(String::from_utf8_lossy(&exited.stdout), format!("run{n}\n"));
                }
                Err(error) => panic!("run {n} of {RUNS} concurrent runs failed: {error}"),
            }
        }
    }

    /// Adversary: a node agent's daemon lives past its first thousand events. `Confine::run`
    /// opens the event stream from the start of the daemon's history (cursor `None`), and
    /// substrate 0.29.0 replays at most 16 pages of 64 events before it ends the stream with
    /// `event.catch-up-limit`; the history holds 10,000 events. Every run after the 1,024th event
    /// fails, though the daemon is healthy. The events here come from empty workspaces created and
    /// destroyed, the cheapest record the daemon writes; a run of processes writes the same kind.
    #[test]
    fn a_process_runs_on_a_daemon_with_more_than_a_thousand_events() {
        use b10x_substrate_sdk::Client;
        use std::time::{Duration, Instant};

        const PAST_CATCH_UP: u64 = 16 * 64 + 64;
        let filled = block_on(async {
            let client = Client::builder()
                .unix_socket(socket())
                .connect()
                .await
                .expect("the daemon answers");
            let started = Instant::now();
            loop {
                let page = client
                    .events(None, 1)
                    .await
                    .expect("the daemon lists its events");
                let first = page.first_retained_sequence.unwrap_or(1);
                let retained = page.through_sequence.saturating_sub(first) + 1;
                if retained >= PAST_CATCH_UP {
                    return retained;
                }
                assert!(
                    started.elapsed() < Duration::from_secs(40),
                    "the daemon recorded only {retained} events in 40 s, the case cannot build its state"
                );
                for _ in 0..16 {
                    let workspace = client
                        .workspace()
                        .empty()
                        .create()
                        .await
                        .expect("an empty workspace is created");
                    workspace
                        .destroy()
                        .await
                        .expect("the workspace is destroyed");
                }
            }
        });
        let ended = block_on(async {
            let confine = Confine::connect(&socket())
                .await
                .expect("the daemon answers");
            confine
                .run(&Process::new(["sh", "-c", "echo weft"]), |_| {})
                .await
        });
        match ended {
            Ok(exited) => {
                assert_eq!(exited.code, Some(0));
                assert_eq!(String::from_utf8_lossy(&exited.stdout), "weft\n");
            }
            Err(error) => panic!("the run after {filled} daemon events failed: {error}"),
        }
    }

    /// Adversary: the states `run` reports belong to its own exec. A second exec starts while the
    /// first runs, so the first run's event stream carries the second's `operation.accepted`
    /// (`exec.start`) and its exec events; none of them may enter the first run's states. The
    /// states of each run follow the lifecycle, each at most once: `accepted`, then `running`,
    /// then `exited`.
    #[test]
    fn the_states_of_a_run_are_its_own_exec_s_while_another_exec_starts() {
        fn rank(state: ExecState) -> u8 {
            match state {
                ExecState::Accepted => 0,
                ExecState::Running => 1,
                _ => 2,
            }
        }
        let long = std::thread::spawn(|| run(&Process::new(["sh", "-c", "sleep 2; echo long"])));
        std::thread::sleep(std::time::Duration::from_millis(700));
        let (short_states, short) = run(&Process::new(["sh", "-c", "echo short"]));
        let (long_states, long) = long.join().expect("the long run ends");
        assert_eq!(String::from_utf8_lossy(&long.stdout), "long\n");
        assert_eq!(String::from_utf8_lossy(&short.stdout), "short\n");
        for states in [&long_states, &short_states] {
            assert_eq!(
                states.first(),
                Some(&ExecState::Accepted),
                "states: {states:?}"
            );
            assert_eq!(
                states.last(),
                Some(&ExecState::Exited),
                "states: {states:?}"
            );
            assert!(
                states.windows(2).all(|pair| rank(pair[0]) < rank(pair[1])),
                "the states are out of lifecycle order, so another exec's event entered them: {states:?}"
            );
        }
        assert!(
            long_states.contains(&ExecState::Running),
            "the long run never reported running: {long_states:?}"
        );
    }
}
