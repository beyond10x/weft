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
}
