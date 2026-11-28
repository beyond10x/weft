//! The one integration-test binary of weft-platform.

/// `exec`: one process of an instance runs through the `Exec` port of the local adapter as one
/// exec of the `substrate-daemon` the setup hook `tools/test-setup.d/20-substrate.sh` started for
/// this test run; its socket path comes from the environment (`WEFT_SUBSTRATE_SOCKET`).
mod exec {
    use std::io::ErrorKind;
    use std::net::{SocketAddr, TcpListener};
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use weft_platform::local::substrate::Confine;
    use weft_platform::node::{
        Exec, ExecState, Exited, HostFile, IsolationLevel, NodeFacts, Process,
    };

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

    fn run(process: &Process) -> (Vec<ExecState>, Exited) {
        let mut states = Vec::new();
        let confine = Confine::connect(&socket()).expect("the daemon answers");
        let exec: &dyn Exec = &confine;
        let exited = exec
            .run(process, &mut |state| states.push(state))
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
                    let confine = Confine::connect(&socket())?;
                    barrier.wait();
                    confine.run(&process, &mut |_| {})
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
        let ended = Confine::connect(&socket())
            .expect("the daemon answers")
            .run(&Process::new(["sh", "-c", "echo weft"]), &mut |_| {});
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

    /// The `NodeFacts` port of the local adapter answers what the daemon proves: the level
    /// `Container`, and no egress, which the setup hook checks its daemon proves (the exec
    /// floor); KVM observed either way, no GPU passed through, one CPU at least.
    #[test]
    fn the_node_facts_are_what_the_daemon_proves() {
        let confine = Confine::connect(&socket()).expect("the daemon answers");
        let facts = (&confine as &dyn NodeFacts)
            .observe()
            .expect("the daemon answers its machine facts");
        assert_eq!(facts.isolation.levels, vec![IsolationLevel::Container]);
        assert_eq!(facts.isolation.egress_denied, Some(true), "{facts:?}");
        assert_eq!(facts.isolation.gpu_passthrough, Some(false), "{facts:?}");
        assert!(facts.isolation.kvm_present.is_some(), "{facts:?}");
        assert!(
            facts
                .capacity
                .iter()
                .any(|amount| amount.resource_class == "cpu" && amount.amount >= 1),
            "{facts:?}"
        );
    }
}

/// `boundary` (cloudspec's [`docs/platform-ports.md`](https://github.com/beyond10x/cloudspec/blob/2ed72c2866bbd165da2de75250b2bfcb340a96ae/docs/platform-ports.md) § 12.5, the rules this story needs): substrate's SDK is
/// named only by the local adapter. It reads files under `product/` only, no build.
mod boundary {
    use std::fs;
    use std::path::{Path, PathBuf};

    /// The root of the repository: this crate is `product/cloud/crates/weft-platform`.
    fn root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(4)
            .expect("the crate lies four levels under the repository root")
            .to_path_buf()
    }

    /// Every file under `dir`. It skips build output only: a crate's build directory (`target`
    /// beside a `Cargo.toml`, the workspace's `target/` included), the `node_modules` beside a
    /// `package.json`, and `.git`. A source module named `target` or `node_modules` is read.
    fn files(dir: &Path, out: &mut Vec<PathBuf>) {
        let entries =
            fs::read_dir(dir).unwrap_or_else(|error| panic!("read {}: {error}", dir.display()));
        for entry in entries {
            let path = entry.expect("a directory entry").path();
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");
            if path.is_dir() {
                let output = match name {
                    "target" => dir.join("Cargo.toml").is_file(),
                    "node_modules" => dir.join("package.json").is_file(),
                    ".git" => true,
                    _ => false,
                };
                if !output {
                    files(&path, out);
                }
            } else {
                out.push(path);
            }
        }
    }

    /// The files under `product/` whose name is `name` (or ends in `name` when it starts with
    /// `.`) and whose text holds `needle`, relative to the repository root.
    fn naming(name: &str, needle: &str) -> Vec<String> {
        let root = root();
        let mut all = Vec::new();
        files(&root.join("product"), &mut all);
        let mut found: Vec<String> = all
            .into_iter()
            .filter(|path| {
                let file = path.file_name().and_then(|f| f.to_str()).unwrap_or("");
                if name.starts_with('.') {
                    file.ends_with(name)
                } else {
                    file == name
                }
            })
            .filter(|path| fs::read_to_string(path).is_ok_and(|text| text.contains(needle)))
            .map(|path| {
                path.strip_prefix(&root)
                    .expect("under the root")
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        found.sort();
        found
    }

    const PLATFORM: &str = "product/cloud/crates/weft-platform";

    #[test]
    fn only_the_weft_platform_manifest_names_the_substrate_sdk() {
        let manifest = format!("{PLATFORM}/Cargo.toml");
        let others: Vec<_> = naming("Cargo.toml", "b10x-substrate-sdk")
            .into_iter()
            .filter(|path| *path != manifest)
            .collect();
        assert!(
            others.is_empty(),
            "manifests under product/ other than {manifest} name b10x-substrate-sdk: {others:?}"
        );
    }

    #[test]
    fn only_the_local_adapter_and_its_tests_name_the_substrate_sdk() {
        let allowed = [
            format!("{PLATFORM}/src/local/"),
            format!("{PLATFORM}/tests/"),
        ];
        let outside: Vec<_> = naming(".rs", "b10x_substrate_sdk")
            .into_iter()
            .filter(|path| !allowed.iter().any(|dir| path.starts_with(dir.as_str())))
            .collect();
        assert!(
            outside.is_empty(),
            "source files under product/ outside {allowed:?} name b10x_substrate_sdk: {outside:?}"
        );
    }

    /// The resolved graph: no package of this workspace other than weft-platform reaches a crate
    /// of substrate. `Cargo.lock` names each package by its own name, whatever a manifest
    /// renames it to, so a dependency renamed in `[workspace.dependencies]` (a manifest outside
    /// `product/`) and a path crate that re-exports the SDK both show here.
    #[test]
    fn no_package_of_the_workspace_but_weft_platform_reaches_substrate() {
        let lock = fs::read_to_string(root().join("Cargo.lock")).expect("Cargo.lock is read");
        assert!(
            packages(&lock).iter().any(|package| package.name == ADAPTER
                && package
                    .dependencies
                    .iter()
                    .any(|dependency| dependency.starts_with("b10x-substrate-sdk"))),
            "Cargo.lock does not show {ADAPTER} depending on b10x-substrate-sdk, so this rule reads nothing"
        );
        let reaching = reaching_substrate(&lock);
        assert!(
            reaching.is_empty(),
            "packages of the workspace other than {ADAPTER} reach substrate: {reaching:?}"
        );
    }

    /// The local adapter re-exports nothing of the SDK: no `pub` (of any reach) `use`,
    /// `extern crate` or `type` under `src/local/` or in `src/local.rs` names the SDK or a name
    /// it was imported under.
    #[test]
    fn the_local_adapter_re_exports_nothing_of_the_substrate_sdk() {
        let crate_root = root().join(PLATFORM);
        let mut sources = vec![crate_root.join("src/local.rs")];
        files(&crate_root.join("src/local"), &mut sources);
        let mut found = Vec::new();
        for path in sources
            .iter()
            .filter(|path| path.extension().is_some_and(|e| e == "rs"))
        {
            let text = fs::read_to_string(path)
                .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
            for statement in sdk_reexports(&text) {
                found.push(format!("{}: {statement}", path.display()));
            }
        }
        assert!(
            found.is_empty(),
            "the local adapter re-exports the SDK: {found:?}"
        );
    }

    const ADAPTER: &str = "weft-platform";

    /// One `[[package]]` of a `Cargo.lock`. `source` is none for a package of the workspace or a
    /// path dependency.
    #[derive(Debug, Default)]
    struct Package {
        name: String,
        version: String,
        source: Option<String>,
        /// As the lock lists them: `name`, `name version` or `name version (source)`.
        dependencies: Vec<String>,
    }

    fn unquote(value: &str) -> String {
        value.trim().trim_matches('"').to_owned()
    }

    /// The packages of a `Cargo.lock`.
    fn packages(lock: &str) -> Vec<Package> {
        let mut all: Vec<Package> = Vec::new();
        let mut in_package = false;
        let mut in_dependencies = false;
        for line in lock.lines().map(str::trim) {
            if in_dependencies {
                if line.starts_with(']') {
                    in_dependencies = false;
                } else if let Some(package) = all.last_mut() {
                    package
                        .dependencies
                        .push(unquote(line.trim_end_matches(',')));
                }
                continue;
            }
            if line.starts_with('[') {
                in_package = line == "[[package]]";
                if in_package {
                    all.push(Package::default());
                }
                continue;
            }
            let (true, Some(package)) = (in_package, all.last_mut()) else {
                continue;
            };
            match line.split_once(" = ") {
                Some(("name", value)) => package.name = unquote(value),
                Some(("version", value)) => package.version = unquote(value),
                Some(("source", value)) => package.source = Some(unquote(value)),
                Some(("dependencies", value)) => {
                    let value = value.trim();
                    if value.ends_with(']') {
                        package.dependencies.extend(
                            value
                                .trim_start_matches('[')
                                .trim_end_matches(']')
                                .split(',')
                                .map(unquote)
                                .filter(|dependency| !dependency.is_empty()),
                        );
                    } else {
                        in_dependencies = true;
                    }
                }
                _ => {}
            }
        }
        all
    }

    /// The packages a dependency of the lock names: by name, and by version when it gives one.
    fn named<'a>(all: &'a [Package], reference: &str) -> Vec<&'a Package> {
        let mut words = reference.split_whitespace();
        let name = words.next().unwrap_or("");
        let version = words.next();
        all.iter()
            .filter(|package| {
                package.name == name && version.is_none_or(|version| package.version == version)
            })
            .collect()
    }

    /// A crate of substrate: named `b10x-substrate-*`, or fetched from substrate's repository.
    fn substrate(package: &Package) -> bool {
        package.name.starts_with("b10x-substrate")
            || package
                .source
                .as_deref()
                .is_some_and(|source| source.contains("github.com/beyond10x/substrate"))
    }

    /// Each package of the workspace or a path (no `source`) other than weft-platform that
    /// reaches a crate of substrate in the lock's graph without passing through weft-platform,
    /// as the chain `package -> .. -> crate`.
    fn reaching_substrate(lock: &str) -> Vec<String> {
        let all = packages(lock);
        let mut found = Vec::new();
        for start in all
            .iter()
            .filter(|package| package.source.is_none() && package.name != ADAPTER)
        {
            if substrate(start) {
                found.push(start.name.clone());
                continue;
            }
            let mut seen = vec![(start.name.as_str(), start.version.as_str())];
            let mut queue = std::collections::VecDeque::from([(start, vec![start.name.as_str()])]);
            while let Some((package, chain)) = queue.pop_front() {
                for reference in &package.dependencies {
                    for next in named(&all, reference) {
                        let key = (next.name.as_str(), next.version.as_str());
                        if next.name == ADAPTER || seen.contains(&key) {
                            continue;
                        }
                        seen.push(key);
                        let mut chain = chain.clone();
                        chain.push(next.name.as_str());
                        if substrate(next) {
                            found.push(chain.join(" -> "));
                        } else {
                            queue.push_back((next, chain));
                        }
                    }
                }
            }
        }
        found
    }

    /// The identifiers of `text`.
    fn words(text: &str) -> impl Iterator<Item = &str> {
        text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .filter(|word| !word.is_empty())
    }

    /// The statements of `source` that make the SDK, or a name imported from it, visible outside
    /// their module: a `pub` (of any reach) `use`, `extern crate` or `type` that names
    /// `b10x_substrate_sdk`, an alias of it, or an item imported from it. Line comments are
    /// not read.
    fn sdk_reexports(source: &str) -> Vec<String> {
        const KEYWORDS: [&str; 7] = ["use", "pub", "crate", "extern", "self", "super", "as"];
        let code: Vec<&str> = source
            .lines()
            .map(|line| line.split("//").next().unwrap_or(""))
            .collect();
        let code = code.join("\n");
        let statements: Vec<String> = code
            .split(';')
            .map(|statement| statement.split_whitespace().collect::<Vec<_>>().join(" "))
            .collect();
        let imports = |statement: &str| {
            words(statement).any(|word| word == "use") || statement.contains("extern crate ")
        };
        let mut names = vec!["b10x_substrate_sdk".to_owned()];
        loop {
            let before = names.len();
            for statement in statements.iter().filter(|statement| imports(statement)) {
                if words(statement).any(|word| names.iter().any(|name| name == word)) {
                    for word in words(statement) {
                        if !KEYWORDS.contains(&word) && !names.iter().any(|name| name == word) {
                            names.push(word.to_owned());
                        }
                    }
                }
            }
            if names.len() == before {
                break;
            }
        }
        let mut found = Vec::new();
        for statement in &statements {
            let mut rest = statement.as_str();
            while let Some(at) = rest.find("pub") {
                let before_ok = rest[..at]
                    .chars()
                    .last()
                    .is_none_or(|c| !(c.is_ascii_alphanumeric() || c == '_'));
                let mut after = rest[at + 3..].trim_start();
                rest = &rest[at + 3..];
                if !before_ok {
                    continue;
                }
                if after.starts_with('(') {
                    after = after
                        .split_once(')')
                        .map_or("", |(_, tail)| tail)
                        .trim_start();
                }
                let exported = ["use ", "extern crate ", "type "]
                    .iter()
                    .any(|item| after.starts_with(item));
                if exported && words(after).any(|word| names.iter().any(|name| name == word)) {
                    found.push(statement.clone());
                    break;
                }
            }
        }
        found
    }

    /// The boundary checks of the resolved graph and of re-exports, on text that holds one hole
    /// each, and on text that holds none.
    mod checks {
        use super::{reaching_substrate, sdk_reexports};

        const SDK: &str = r#"
[[package]]
name = "b10x-substrate-sdk"
version = "0.29.0"
source = "git+https://github.com/beyond10x/substrate?rev=1a10a30#1a10a30"
dependencies = [
 "b10x-substrate-wire",
]

[[package]]
name = "b10x-substrate-wire"
version = "0.29.0"
source = "git+https://github.com/beyond10x/substrate?rev=1a10a30#1a10a30"

[[package]]
name = "weft-platform"
version = "0.1.0"
dependencies = [
 "b10x-substrate-sdk",
]
"#;

        fn lock(extra: &str) -> String {
            format!("version = 4\n{SDK}{extra}")
        }

        #[test]
        fn a_lock_where_only_weft_platform_reaches_substrate_passes() {
            let text = lock(
                r#"
[[package]]
name = "weft-agent"
version = "0.1.0"
dependencies = [
 "serde_json",
 "weft-platform",
]

[[package]]
name = "serde_json"
version = "1.0.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
"#,
            );
            assert_eq!(reaching_substrate(&text), Vec::<String>::new());
        }

        /// Hole (a): `[workspace.dependencies] substrate = { package = "b10x-substrate-sdk", .. }`
        /// in the root manifest, `substrate.workspace = true` in weft-agent's, and
        /// `use substrate::Client;` in its source: no file under `product/` names
        /// `b10x-substrate-sdk` or `b10x_substrate_sdk`. The lock names the package by its own name.
        #[test]
        fn a_dependency_renamed_in_the_workspace_manifest_is_caught() {
            let text = lock(
                r#"
[[package]]
name = "weft-agent"
version = "0.1.0"
dependencies = [
 "b10x-substrate-sdk",
 "weft-platform",
]
"#,
            );
            let reaching = reaching_substrate(&text);
            assert!(
                reaching
                    .iter()
                    .any(|chain| chain.starts_with("weft-agent ")),
                "{reaching:?}"
            );
        }

        /// Hole (b): a path crate outside `product/` (`tools/sdk-shim`, `pub use
        /// b10x_substrate_sdk::*;`) that weft-agent depends on.
        #[test]
        fn a_path_crate_that_re_exports_the_sdk_is_caught() {
            let text = lock(
                r#"
[[package]]
name = "sdk-shim"
version = "0.1.0"
dependencies = [
 "b10x-substrate-sdk",
]

[[package]]
name = "weft-agent"
version = "0.1.0"
dependencies = [
 "sdk-shim",
 "weft-platform",
]
"#,
            );
            let reaching = reaching_substrate(&text);
            for package in ["sdk-shim", "weft-agent"] {
                assert!(
                    reaching
                        .iter()
                        .any(|chain| chain.starts_with(&format!("{package} "))),
                    "{package} is not reported: {reaching:?}"
                );
            }
        }

        /// The same through a published crate: no path crate, the re-export comes from a
        /// registry, and only substrate's wire crate is reached.
        #[test]
        fn a_registry_crate_that_reaches_substrate_is_caught() {
            let text = lock(
                r#"
[[package]]
name = "substrate-shim"
version = "0.1.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
dependencies = [
 "b10x-substrate-wire",
]

[[package]]
name = "weft-controllers"
version = "0.1.0"
dependencies = [
 "substrate-shim 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)",
]
"#,
            );
            let reaching = reaching_substrate(&text);
            assert!(
                reaching
                    .iter()
                    .any(|chain| chain.starts_with("weft-controllers ")
                        && chain.ends_with("b10x-substrate-wire")),
                "{reaching:?}"
            );
        }

        /// Hole (c): the adapter re-exports the SDK, and code outside it uses the re-export
        /// (`weft_platform::local::substrate::sdk::Client`) without naming `b10x_substrate_sdk`.
        #[test]
        fn a_re_export_of_the_sdk_by_the_adapter_is_caught() {
            let holes = [
                "pub use b10x_substrate_sdk;",
                "pub use b10x_substrate_sdk as sdk;",
                "pub use b10x_substrate_sdk::Client;",
                "pub use b10x_substrate_sdk::{Client, Event};",
                "pub extern crate b10x_substrate_sdk;",
                "pub(crate) use b10x_substrate_sdk as sdk;",
                "pub(super) use b10x_substrate_sdk::Client;",
                "use b10x_substrate_sdk as sdk;\npub use sdk::Client;",
                "use b10x_substrate_sdk as sdk;\npub type Client = sdk::Client;",
                "use b10x_substrate_sdk as sdk;\nuse sdk as inner;\npub use inner::Client;",
                "use b10x_substrate_sdk::{self as sdk, Client};\npub use sdk::Event;",
                "extern crate b10x_substrate_sdk as sdk;\npub use sdk::Client;",
                "mod shim {\n    pub use b10x_substrate_sdk::Client;\n}",
                "#[doc(hidden)]\npub use b10x_substrate_sdk::Client;",
                "pub use\n    b10x_substrate_sdk::Client;",
            ];
            for hole in holes {
                assert!(
                    !sdk_reexports(hole).is_empty(),
                    "not caught as a re-export of the SDK: {hole:?}"
                );
            }
        }

        #[test]
        fn a_private_import_of_the_sdk_is_not_a_re_export() {
            let clean = [
                "use b10x_substrate_sdk as sdk;\nuse b10x_substrate_sdk::{Client, Event};",
                "use b10x_substrate_sdk as sdk;\npub struct Confine {\n    client: sdk::Client,\n}",
                "pub use crate::node::Exec;",
                "// pub use b10x_substrate_sdk::Client;\nuse b10x_substrate_sdk::Client;",
                "use b10x_substrate_sdk as sdk;\npub use crate::sdkish::Thing;",
            ];
            for text in clean {
                assert_eq!(sdk_reexports(text), Vec::<String>::new(), "{text:?}");
            }
        }
    }

    /// Adversary (weft#29): what the walk of the boundary rules does not read. It runs the walk
    /// [`files`] the two rules above use on a fixture tree under `CARGO_TARGET_TMPDIR`.
    mod holes {
        use std::fs;
        use std::path::PathBuf;

        /// The walk skips a crate's build directory (`target` beside its `Cargo.toml`) and reads
        /// every source module, whatever its name: `target`, `node_modules`.
        #[test]
        fn the_walk_skips_only_a_crate_s_build_directory() {
            let fixture = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
                .join(format!("boundary-walk-{}", std::process::id()));
            let krate = fixture.join("product/cloud/crates/weft-agent");
            let build = krate.join("target/debug/build.rs");
            let modules = [
                krate.join("src/target/mod.rs"),
                krate.join("src/node_modules/mod.rs"),
                krate.join("src/deploy/target/mod.rs"),
            ];
            for file in modules.iter().chain([&build]) {
                fs::create_dir_all(file.parent().expect("a parent"))
                    .expect("the fixture directory");
                fs::write(file, "use b10x_substrate_sdk::Client;\n").expect("the fixture file");
            }
            fs::write(
                krate.join("Cargo.toml"),
                "[package]\nname = \"weft-agent\"\n",
            )
            .expect("the fixture manifest");
            let mut walked = Vec::new();
            super::files(&fixture.join("product"), &mut walked);
            let _ = fs::remove_dir_all(&fixture);
            for module in &modules {
                assert!(
                    walked.contains(module),
                    "the walk never reads {}: it read {walked:?}",
                    module.display()
                );
            }
            assert!(
                !walked.contains(&build),
                "the walk reads the crate's build directory: {walked:?}"
            );
        }

        /// A crate whose source has a module named `target` (`mod target;` -> `src/target/`,
        /// a deployment target, say). The walk skips every directory named `target`, not only
        /// the build directory at a crate's root, so that module's files are never read, and a
        /// `use b10x_substrate_sdk::Client;` there passes both rules.
        #[test]
        fn the_walk_reads_a_source_module_named_target() {
            let fixture = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
                .join(format!("boundary-holes-{}", std::process::id()));
            let module = fixture.join("product/cloud/crates/weft-agent/src/target");
            fs::create_dir_all(&module).expect("the fixture directory is made");
            let leak = module.join("mod.rs");
            fs::write(&leak, "use b10x_substrate_sdk::Client;\n").expect("the fixture file");
            let mut walked = Vec::new();
            super::files(&fixture.join("product"), &mut walked);
            let _ = fs::remove_dir_all(&fixture);
            assert!(
                walked.contains(&leak),
                "the boundary walk never reads {}, a source file that names the SDK: it read {walked:?}",
                leak.display()
            );
        }
    }
}

/// Adversary cases of story `ports-node-exec-facts` (weft#29).
mod adversary_ports {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::path::{Path, PathBuf};

    use serde_json::{json, Value};
    use weft_platform::local::substrate::Confine;
    use weft_platform::node::{
        Exec, ExecError, ExecState, IsolationLevel, Limits, NodeFacts, Process,
    };
    use weft_platform::Refusal;

    pub(super) fn socket() -> PathBuf {
        let path = PathBuf::from(
            std::env::var_os("WEFT_SUBSTRATE_SOCKET").expect("WEFT_SUBSTRATE_SOCKET is set"),
        );
        assert!(
            path.exists(),
            "no substrate-daemon serves {}",
            path.display()
        );
        path
    }

    pub(super) fn panic_text(payload: &(dyn std::any::Any + Send)) -> String {
        payload
            .downcast_ref::<&str>()
            .map(|text| (*text).to_owned())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_default()
    }

    /// One answer of the real daemon at `path`: its headers and its JSON body.
    pub(super) fn daemon_get(path: &str) -> (Vec<(String, String)>, Value) {
        let mut stream = UnixStream::connect(socket()).expect("the daemon's socket connects");
        write!(
            stream,
            "GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
        )
        .expect("the request is written");
        let mut text = String::new();
        stream
            .read_to_string(&mut text)
            .expect("the answer is read");
        let (head, body) = text.split_once("\r\n\r\n").expect("a head and a body");
        let headers = head
            .lines()
            .skip(1)
            .filter_map(|line| line.split_once(':'))
            .map(|(name, value)| (name.trim().to_owned(), value.trim().to_owned()))
            .collect();
        (headers, serde_json::from_str(body).expect("a JSON body"))
    }

    /// A stand-in daemon on a Unix socket of its own: each request's path gets `answer(path)`,
    /// with the contract headers the real daemon sends.
    pub(super) fn stand_in(
        name: &str,
        answer: impl Fn(&str) -> (u16, Value) + Send + 'static,
    ) -> PathBuf {
        let (headers, _) = daemon_get("/v1/machine");
        let contract: Vec<(String, String)> = headers
            .into_iter()
            .filter(|(name, _)| name.starts_with("x-b10x-contract"))
            .collect();
        let path = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("{name}-{}.sock", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let listener = UnixListener::bind(&path).expect("the stand-in's socket binds");
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { continue };
                let mut reader = BufReader::new(stream);
                let mut first = String::new();
                let mut length = 0;
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                        break;
                    }
                    if first.is_empty() {
                        first = line.clone();
                    }
                    if let Some((name, value)) = line.split_once(':') {
                        if name.eq_ignore_ascii_case("content-length") {
                            length = value.trim().parse().unwrap_or(0);
                        }
                    }
                }
                let mut body = vec![0; length];
                let _ = reader.read_exact(&mut body);
                let target = first.split_whitespace().nth(1).unwrap_or("").to_owned();
                let (status, body) = answer(&target);
                let body = body.to_string();
                let mut stream = reader.into_inner();
                let mut head = format!("HTTP/1.1 {status} X\r\ncontent-type: application/json\r\n");
                for (name, value) in &contract {
                    head.push_str(&format!("{name}: {value}\r\n"));
                }
                let _ = write!(
                    stream,
                    "{head}content-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        path
    }

    pub(super) fn empty_events() -> Value {
        json!({"api_version": "v1", "request_id": "req_adversary", "result": {
            "source_scope": "scope_adversary", "generation": 1, "items": [],
            "next_cursor": "ev2.scope_adversary.1.0", "through_seq": 0, "first_retained_seq": null}})
    }

    /// § 3.4: `Invalid { field, reason }` is "the input breaks a bound the place states";
    /// `Unavailable` is "the place did not answer; transient, the step tries again on its next
    /// tick". A process whose limits break substrate's bounds (here 0 processes; equally 2 MiB of
    /// output, over substrate's 1 MiB `MAX_IO_BYTES`) is refused by the SDK's policy builder
    /// with `SdkError::Protocol`, which `refused` maps to `Unavailable`: a caller that follows
    /// the contract retries an input that can never run. The `Builder` arm that yields
    /// `Invalid` is not reachable: `run_async` sets every field.
    #[test]
    fn limits_outside_the_place_s_bounds_are_refused_invalid_not_unavailable() {
        let confine = Confine::connect(&socket()).expect("the daemon answers");
        let process = Process::new(["sh", "-c", "echo weft"]).limits(Limits {
            processes: 0,
            ..Limits::default()
        });
        let ended = confine.run(&process, &mut |_| {});
        assert!(
            matches!(ended, Err(ExecError::Shared(Refusal::Invalid { .. }))),
            "limits with 0 processes break a bound the place states, so the refusal is Invalid; it was {ended:?}"
        );
    }

    /// The traits are synchronous and `Confine` keeps its own runtime: `connect` (and `run`,
    /// `observe`) called on a thread that is inside a tokio runtime panics ("Cannot start a
    /// runtime from within a runtime") instead of answering. The async `connect` of
    /// `confine.rs` before the move was callable there.
    #[test]
    fn connect_inside_a_tokio_runtime_answers_and_does_not_panic() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("a runtime builds");
        let caught = runtime.block_on(async {
            std::panic::catch_unwind(|| {
                Confine::connect(Path::new("/nonexistent/weft-adversary.sock")).err()
            })
        });
        match caught {
            Ok(Some(ExecError::Shared(Refusal::Unavailable { .. }))) => {}
            Ok(other) => panic!("no daemon answers there, yet connect answered {other:?}"),
            Err(payload) => panic!(
                "Confine::connect panicked inside a tokio runtime: {}",
                panic_text(payload.as_ref())
            ),
        }
    }

    /// The agent passes one adapter as both ports (`main.rs`: `agent::run(&options, &local,
    /// &local)`). `on_state` runs inside the adapter's `block_on`, so a callback that asks the
    /// other port of the same adapter panics ("Cannot start a runtime from within a
    /// runtime"); `Exec::run`'s contract (`node.rs:167-181`) does not forbid it.
    #[test]
    fn on_state_may_ask_the_node_facts_port_of_the_same_adapter() {
        let confine = Confine::connect(&socket()).expect("the daemon answers");
        let mut observed = Vec::new();
        let ended = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            confine.run(&Process::new(["sh", "-c", "echo weft"]), &mut |state| {
                if state == ExecState::Accepted {
                    observed.push(confine.observe().is_ok());
                }
            })
        }));
        match ended {
            Ok(Ok(exited)) => {
                assert_eq!(exited.code, Some(0));
                assert_eq!(observed, vec![true]);
            }
            Ok(Err(error)) => panic!("the run failed: {error}"),
            Err(payload) => panic!(
                "a NodeFacts::observe from on_state panicked: {}",
                panic_text(payload.as_ref())
            ),
        }
    }

    /// § 3.2 "Observed, not claimed", § 3.6 default deny, § 5 N2 "a fact the daemon withholds
    /// is absent". A daemon whose start-up probe withheld the exec floor (no `exec.namespaces`,
    /// no `exec.no-egress`, no `exec.cgroup-limits`: bubblewrap missing, say) runs no confined
    /// exec. The adapter keeps the withheld Booleans absent, and still reports the level
    /// `Container`, which `observe` writes as a constant (`substrate.rs:251`).
    #[test]
    fn a_daemon_that_withholds_the_exec_floor_is_not_reported_container() {
        let (_, mut machine) = daemon_get("/v1/machine");
        let facts = machine["result"]["facts"]
            .as_object_mut()
            .expect("the machine has facts");
        for withheld in ["exec.namespaces", "exec.no-egress", "exec.cgroup-limits"] {
            facts.remove(withheld);
        }
        let path = stand_in("floor", move |target| {
            if target.starts_with("/v1/machine") {
                (200, machine.clone())
            } else if target.starts_with("/v1/events") {
                (200, empty_events())
            } else {
                (404, json!({}))
            }
        });
        let confine = Confine::connect(&path).expect("the stand-in answers");
        let facts = confine.observe().expect("the stand-in answers its machine");
        let _ = std::fs::remove_file(&path);
        assert_eq!(facts.isolation.egress_denied, None, "{facts:?}");
        assert_eq!(facts.isolation.cgroups_delegated, None, "{facts:?}");
        assert!(
            !facts.isolation.levels.contains(&IsolationLevel::Container),
            "the daemon proves no namespaces, yet the node reports the level Container: {facts:?}"
        );
    }
}

/// The classes of the adversary's findings on weft#29, each member a case.
mod ports_classes {
    use std::time::Duration;

    use b10x_substrate_sdk::{
        MAX_EXEC_DURATION, MAX_EXEC_MEMORY_BYTES, MAX_EXEC_PROCESSES, MAX_IO_BYTES,
        MIN_EXEC_MEMORY_BYTES,
    };
    use serde_json::{json, Value};
    use weft_platform::local::substrate::Confine;
    use weft_platform::node::{
        Exec, ExecError, IsolationLevel, Limits, NodeFacts, Process, RefusalClass,
    };
    use weft_platform::Refusal;

    use super::adversary_ports::{daemon_get, empty_events, socket, stand_in};

    /// Every limit outside the bounds substrate states is refused `Invalid` on field `limits`,
    /// before anything reaches the daemon.
    #[test]
    fn every_limit_outside_the_place_s_bounds_is_refused_invalid() {
        let base = Limits::default();
        let outside = [
            (
                "timeout 0",
                Limits {
                    timeout: Duration::ZERO,
                    ..base
                },
            ),
            (
                "timeout over the most",
                Limits {
                    timeout: MAX_EXEC_DURATION + Duration::from_secs(1),
                    ..base
                },
            ),
            (
                "cpu_time 0",
                Limits {
                    cpu_time: Duration::ZERO,
                    ..base
                },
            ),
            (
                "cpu_time over the most",
                Limits {
                    cpu_time: MAX_EXEC_DURATION + Duration::from_secs(1),
                    ..base
                },
            ),
            (
                "memory under the least",
                Limits {
                    memory_bytes: MIN_EXEC_MEMORY_BYTES - 1,
                    ..base
                },
            ),
            (
                "memory over the most",
                Limits {
                    memory_bytes: MAX_EXEC_MEMORY_BYTES + 1,
                    ..base
                },
            ),
            (
                "processes 0",
                Limits {
                    processes: 0,
                    ..base
                },
            ),
            (
                "processes over the most",
                Limits {
                    processes: MAX_EXEC_PROCESSES + 1,
                    ..base
                },
            ),
            (
                "output 0",
                Limits {
                    output_bytes: 0,
                    ..base
                },
            ),
            (
                "output over the most",
                Limits {
                    output_bytes: MAX_IO_BYTES + 1,
                    ..base
                },
            ),
        ];
        let confine = Confine::connect(&socket()).expect("the daemon answers");
        for (name, limits) in outside {
            let ended = confine.run(
                &Process::new(["sh", "-c", "echo weft"]).limits(limits),
                &mut |_| {},
            );
            assert!(
                matches!(&ended, Err(ExecError::Shared(Refusal::Invalid { field, .. })) if field == "limits"),
                "{name}: the refusal is Invalid on limits; it was {ended:?}"
            );
        }
    }

    /// `Limits::LONGEST`, what `weft-agent` gives a `Service` replica (`story:skeleton-node-run-
    /// assignment`), is substrate's most, and an exec with it as its `timeout` and `cpu_time`
    /// runs to `exited`.
    #[test]
    fn the_longest_limits_are_the_place_s_most_and_are_accepted() {
        assert_eq!(Limits::LONGEST, MAX_EXEC_DURATION);
        let confine = Confine::connect(&socket()).expect("the daemon answers");
        let ended = confine.run(
            &Process::new(["sh", "-c", "echo weft"]).limits(Limits {
                timeout: Limits::LONGEST,
                cpu_time: Limits::LONGEST,
                ..Limits::default()
            }),
            &mut |_| {},
        );
        let exited = ended.expect("an exec with the longest limits runs");
        assert_eq!(
            (exited.code, exited.stdout.as_slice()),
            (Some(0), b"weft\n".as_slice())
        );
    }

    /// Each fact of the exec floor, withheld alone, takes the level `Container` away; the
    /// machine of the real daemon, answered unchanged by the stand-in, keeps it.
    #[test]
    fn each_fact_of_the_exec_floor_withheld_takes_container_away() {
        let (_, machine) = daemon_get("/v1/machine");
        let levels = |change: &dyn Fn(&mut serde_json::Map<String, Value>)| {
            let mut machine = machine.clone();
            change(
                machine["result"]["facts"]
                    .as_object_mut()
                    .expect("the machine has facts"),
            );
            let path = stand_in("floor-each", move |target| {
                if target.starts_with("/v1/machine") {
                    (200, machine.clone())
                } else if target.starts_with("/v1/events") {
                    (200, empty_events())
                } else {
                    (404, json!({}))
                }
            });
            let confine = Confine::connect(&path).expect("the stand-in answers");
            let facts = confine.observe().expect("the stand-in answers its machine");
            let _ = std::fs::remove_file(&path);
            facts.isolation.levels
        };
        assert_eq!(
            levels(&|_| {}),
            vec![IsolationLevel::Container],
            "the real daemon's machine, unchanged"
        );
        for fact in [
            "exec.argv-only",
            "exec.namespaces",
            "exec.no-egress",
            "exec.cgroup-limits",
            "exec.cgroup-kill",
        ] {
            assert_eq!(
                levels(&|facts| {
                    facts.remove(fact);
                }),
                Vec::<IsolationLevel>::new(),
                "{fact} withheld"
            );
        }
        for namespace in ["user", "mount", "pid", "ipc", "uts", "network"] {
            assert_eq!(
                levels(&|facts| {
                    facts["exec.namespaces"][namespace] = json!(false);
                }),
                Vec::<IsolationLevel>::new(),
                "exec.namespaces.{namespace} false"
            );
        }
        for controller in ["cpu", "memory", "processes"] {
            assert_eq!(
                levels(&|facts| {
                    facts["exec.cgroup-limits"][controller] = json!(false);
                }),
                Vec::<IsolationLevel>::new(),
                "exec.cgroup-limits.{controller} false"
            );
        }
    }

    /// Each entry of the adapter (`connect`, `run`, `observe`, and its drop) answers on a thread
    /// inside a tokio runtime, current-thread and multi-thread, and does not panic.
    #[test]
    fn every_entry_of_the_adapter_answers_inside_a_tokio_runtime() {
        let runtimes = [
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("a runtime builds"),
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .build()
                .expect("a runtime builds"),
        ];
        for runtime in runtimes {
            let caught = runtime.block_on(async {
                std::panic::catch_unwind(|| {
                    let confine = Confine::connect(&socket()).expect("the daemon answers");
                    let facts = confine.observe().expect("the daemon answers its machine");
                    let exited = confine
                        .run(&Process::new(["sh", "-c", "echo weft"]), &mut |_| {})
                        .expect("the exec runs");
                    drop(confine);
                    (facts.isolation.levels, exited.stdout)
                })
            });
            match caught {
                Ok((levels, stdout)) => {
                    assert_eq!(levels, vec![IsolationLevel::Container]);
                    assert_eq!(String::from_utf8_lossy(&stdout), "weft\n");
                }
                Err(payload) => panic!(
                    "the adapter panicked inside a tokio runtime: {}",
                    super::adversary_ports::panic_text(payload.as_ref())
                ),
            }
        }
    }

    /// § 3.4: a substrate refusal is carried with its code, its class, its message and whether
    /// it is retriable. A stand-in daemon refuses the workspace of the run with each class in
    /// turn, retriable and not.
    #[test]
    fn a_refusal_of_the_place_carries_its_class_and_whether_it_is_retriable() {
        let (_, machine) = daemon_get("/v1/machine");
        let classes = [
            ("refused", RefusalClass::Refused, false),
            ("conflict", RefusalClass::Conflict, true),
            ("unserved", RefusalClass::Unserved, false),
            ("exhausted", RefusalClass::Exhausted, true),
            ("failed", RefusalClass::Failed, false),
        ];
        for (wire, class, retriable) in classes {
            let machine = machine.clone();
            let path = stand_in("refusal", move |target| {
                if target.starts_with("/v1/machine") {
                    (200, machine.clone())
                } else if target.starts_with("/v1/events") {
                    (200, empty_events())
                } else {
                    (
                        409,
                        json!({"api_version": "v1", "request_id": "req_adversary", "error": {
                            "class": wire, "code": "workspace.adversary",
                            "message": "the stand-in refuses", "retriable": retriable}}),
                    )
                }
            });
            let confine = Confine::connect(&path).expect("the stand-in answers");
            let ended = confine.run(&Process::new(["sh", "-c", "echo weft"]), &mut |_| {});
            let _ = std::fs::remove_file(&path);
            assert_eq!(
                ended,
                Err(ExecError::Refused {
                    class,
                    code: "workspace.adversary".to_owned(),
                    message: "the stand-in refuses".to_owned(),
                    retriable,
                }),
                "class {wire}"
            );
        }
    }
}

/// `workloads`: the N3 port, a long-lived supervised workload, driven through the in-memory stub
/// adapter of `local::stub`. No place is reached: these cases need no daemon.
mod workloads {
    use std::time::Duration;

    use weft_platform::local::stub::{StubEvent, StubWorkloads};
    use weft_platform::node::{
        KillScope, Observation, PrestartStep, ProcessExit, RestartMode, RestartRule, StartFailure,
        StopEnd, StopRule, WorkloadError, WorkloadKey, WorkloadSpec, WorkloadState, Workloads,
    };
    use weft_platform::Refusal;

    fn key() -> WorkloadKey {
        WorkloadKey::new("assignment-1", "main")
    }

    fn observe(port: &dyn Workloads) -> Observation {
        port.observe(&key()).expect("the stub knows the workload")
    }

    /// The acceptance of the port: start, running, an exit with code 3, a restart, and a stop
    /// whose grace runs out so the kill scope is applied, each state read through the port.
    #[test]
    fn a_workload_starts_runs_exits_3_restarts_and_a_stop_past_its_grace_kills_its_scope() {
        let stub = StubWorkloads::new();
        let port: &dyn Workloads = &stub;
        let spec = WorkloadSpec::new(["/usr/local/bin/slot", "serve"])
            .env("HOME", "/var/lib/slot-build")
            .working_dir("/srv/slot")
            .run_as("build")
            .prestart(PrestartStep::new(["/bin/true"]))
            .stop(StopRule::new(Duration::from_secs(2), KillScope::Group));

        let started = port.start(&key(), &spec).expect("the start is accepted");
        assert_eq!(started.state, WorkloadState::Starting, "{started:?}");
        let running = observe(port);
        assert_eq!(running.state, WorkloadState::Running, "{running:?}");
        assert_eq!(running.restarts, 0, "{running:?}");

        stub.exit(&key(), ProcessExit::Code(3));
        let exited = observe(port);
        assert_eq!(exited.state, WorkloadState::Exited, "{exited:?}");
        assert_eq!(exited.last_exit, Some(ProcessExit::Code(3)), "{exited:?}");

        let restarting = port.restart(&key()).expect("the restart is accepted");
        assert_eq!(
            restarting.state,
            WorkloadState::Restarting,
            "{restarting:?}"
        );
        let running = observe(port);
        assert_eq!(running.state, WorkloadState::Running, "{running:?}");
        assert_eq!(running.restarts, 1, "{running:?}");

        stub.exceed_grace(&key());
        let rule = StopRule::new(Duration::from_secs(2), KillScope::Group);
        let stopped = port.stop(&key(), &rule).expect("the stop is accepted");
        assert_eq!(stopped.state, WorkloadState::Stopped, "{stopped:?}");
        assert_eq!(
            stopped.stopped,
            Some(StopEnd::Killed(KillScope::Group)),
            "{stopped:?}"
        );
        assert_eq!(observe(port), stopped);

        let commands = stub
            .events()
            .into_iter()
            .filter(|event| matches!(event, StubEvent::Command { .. }))
            .count();
        assert_eq!(
            commands, 2,
            "the command ran once and once more after the restart"
        );
        assert!(
            stub.events().contains(&StubEvent::Stopped {
                key: key(),
                rule,
                end: StopEnd::Killed(KillScope::Group),
            }),
            "the stop reached the stub with its grace and kill scope: {:?}",
            stub.events()
        );
    }

    /// A stop that the process answers within its grace ends without the kill.
    #[test]
    fn a_stop_within_the_grace_applies_no_kill() {
        let stub = StubWorkloads::new();
        let port: &dyn Workloads = &stub;
        port.start(&key(), &WorkloadSpec::new(["sleep", "infinity"]))
            .expect("the start is accepted");
        let rule = StopRule::new(Duration::from_secs(5), KillScope::Mixed);
        let stopped = port.stop(&key(), &rule).expect("the stop is accepted");
        assert_eq!(stopped.state, WorkloadState::Stopped, "{stopped:?}");
        assert_eq!(stopped.stopped, Some(StopEnd::WithinGrace), "{stopped:?}");
    }

    /// The spec's restart rule: `OnFailure` restarts after exit 3, not after exit 0; `Always`
    /// keeps the process down on an exit code it names in `keep_down_on_exit`.
    #[test]
    fn the_restart_rule_of_the_spec_restarts_by_policy_and_keeps_down_on_a_named_code() {
        let stub = StubWorkloads::new();
        let port: &dyn Workloads = &stub;
        let on_failure = WorkloadSpec::new(["daemon"])
            .restart(RestartRule::new(RestartMode::OnFailure, Duration::ZERO));
        port.start(&key(), &on_failure)
            .expect("the start is accepted");
        observe(port);
        stub.exit(&key(), ProcessExit::Code(3));
        let restarting = observe(port);
        assert_eq!(
            restarting.state,
            WorkloadState::Restarting,
            "{restarting:?}"
        );
        assert_eq!(
            restarting.last_exit,
            Some(ProcessExit::Code(3)),
            "{restarting:?}"
        );
        let running = observe(port);
        assert_eq!(running.state, WorkloadState::Running, "{running:?}");
        assert_eq!(running.restarts, 1, "{running:?}");
        stub.exit(&key(), ProcessExit::Code(0));
        assert_eq!(observe(port).state, WorkloadState::Exited);

        let other = WorkloadKey::new("assignment-1", "refresh");
        let always = WorkloadSpec::new(["refresh"])
            .restart(RestartRule::new(RestartMode::Always, Duration::ZERO).keep_down_on_exit([2]));
        port.start(&other, &always).expect("the start is accepted");
        port.observe(&other).expect("known");
        stub.exit(&other, ProcessExit::Code(2));
        let down = port.observe(&other).expect("known");
        assert_eq!(down.state, WorkloadState::Exited, "{down:?}");
        assert_eq!(down.last_exit, Some(ProcessExit::Code(2)), "{down:?}");
    }

    /// A restart waits the rule's delay: `Restarting` until it passed.
    #[test]
    fn a_restart_by_the_rule_waits_its_delay() {
        let stub = StubWorkloads::new();
        let port: &dyn Workloads = &stub;
        let delay = Duration::from_millis(200);
        let spec =
            WorkloadSpec::new(["daemon"]).restart(RestartRule::new(RestartMode::Always, delay));
        port.start(&key(), &spec).expect("the start is accepted");
        observe(port);
        stub.exit(&key(), ProcessExit::Code(1));
        assert_eq!(observe(port).state, WorkloadState::Restarting);
        assert_eq!(observe(port).state, WorkloadState::Restarting);
        std::thread::sleep(delay);
        assert_eq!(observe(port).state, WorkloadState::Running);
    }

    /// Prestart steps run in list order before the command; a step that exits non-zero ends the
    /// start: the command never runs and the workload does not reach `Running`.
    #[test]
    fn a_prestart_step_that_fails_keeps_the_workload_from_running() {
        let stub = StubWorkloads::new();
        let port: &dyn Workloads = &stub;
        let spec = WorkloadSpec::new(["daemon"])
            .prestart(PrestartStep::new(["first"]))
            .prestart(PrestartStep::new(["second"]))
            .prestart(PrestartStep::new(["third"]));
        let failure = StartFailure::Prestart {
            step: 1,
            exit: ProcessExit::Code(4),
        };
        stub.fail_start(&key(), failure.clone());
        port.start(&key(), &spec).expect("the start is accepted");
        let failed = observe(port);
        assert_eq!(failed.state, WorkloadState::Failed, "{failed:?}");
        assert_eq!(failed.failure, Some(failure), "{failed:?}");
        assert_eq!(observe(port).state, WorkloadState::Failed);
        let ran: Vec<_> = stub
            .events()
            .into_iter()
            .filter(|event| {
                matches!(
                    event,
                    StubEvent::Prestart { .. } | StubEvent::Command { .. }
                )
            })
            .collect();
        assert_eq!(
            ran,
            vec![
                StubEvent::Prestart {
                    key: key(),
                    step: 0,
                    privileged: false,
                },
                StubEvent::Prestart {
                    key: key(),
                    step: 1,
                    privileged: false,
                },
            ]
        );
    }

    /// The hooks that make a start fail: a refusal of the place, and a command that does not
    /// start.
    #[test]
    fn a_start_can_be_refused_or_fail_to_start_its_command() {
        let stub = StubWorkloads::new();
        let port: &dyn Workloads = &stub;
        let refusal = WorkloadError::Shared(Refusal::Unavailable {
            detail: "the supervisor did not answer".to_owned(),
        });
        stub.refuse_start(&key(), refusal.clone());
        assert_eq!(
            port.start(&key(), &WorkloadSpec::new(["daemon"])),
            Err(refusal)
        );
        assert_eq!(
            port.observe(&key()),
            Err(WorkloadError::UnknownWorkload(key()))
        );

        let failure = StartFailure::Command {
            reason: "no such file: daemon".to_owned(),
        };
        stub.fail_start(&key(), failure.clone());
        port.start(&key(), &WorkloadSpec::new(["daemon"]))
            .expect("the start is accepted");
        let failed = observe(port);
        assert_eq!(failed.state, WorkloadState::Failed, "{failed:?}");
        assert_eq!(failed.failure, Some(failure), "{failed:?}");
    }

    /// A privileged prestart step is passed as such; an adapter that cannot run it refuses
    /// `NotServed` by name, before anything runs.
    #[test]
    fn a_privileged_prestart_step_is_refused_not_served_where_the_adapter_cannot_run_it() {
        let step = PrestartStep::new(["/bin/sh", "provision.sh", "start-pre"]).privileged(true);
        let spec = WorkloadSpec::new(["daemon"]).prestart(step);

        let serving = StubWorkloads::new();
        serving.start(&key(), &spec).expect("the start is accepted");
        assert!(serving.events().contains(&StubEvent::Prestart {
            key: key(),
            step: 0,
            privileged: true,
        }));

        let unprivileged = StubWorkloads::without_privileged();
        assert_eq!(
            unprivileged.start(&key(), &spec),
            Err(WorkloadError::Shared(Refusal::NotServed {
                operation: "prestart.privileged".to_owned(),
            }))
        );
        assert!(
            unprivileged.events().is_empty(),
            "{:?}",
            unprivileged.events()
        );
    }

    /// A second start of a running key answers its observation and starts nothing.
    #[test]
    fn a_second_start_of_a_running_key_answers_its_observation() {
        let stub = StubWorkloads::new();
        let port: &dyn Workloads = &stub;
        let spec = WorkloadSpec::new(["daemon"]);
        port.start(&key(), &spec).expect("the start is accepted");
        let running = observe(port);
        assert_eq!(port.start(&key(), &spec), Ok(running));
        let commands = stub
            .events()
            .into_iter()
            .filter(|event| matches!(event, StubEvent::Command { .. }))
            .count();
        assert_eq!(commands, 1);
    }

    /// A spec without a command, or with a prestart step without one, is refused `Invalid`.
    #[test]
    fn a_spec_without_a_command_is_refused_invalid() {
        let stub = StubWorkloads::new();
        let empty: [&str; 0] = [];
        assert!(matches!(
            stub.start(&key(), &WorkloadSpec::new(empty)),
            Err(WorkloadError::Shared(Refusal::Invalid { field, .. })) if field == "argv"
        ));
        let spec = WorkloadSpec::new(["daemon"]).prestart(PrestartStep::new(empty));
        assert!(matches!(
            stub.start(&key(), &spec),
            Err(WorkloadError::Shared(Refusal::Invalid { field, .. })) if field == "prestart[0].argv"
        ));
        assert!(stub.events().is_empty(), "{:?}", stub.events());
    }

    /// `RestartRule::restarts_after`, the rule an adapter or the agent applies after an exit.
    #[test]
    fn the_restart_rule_decides_each_exit() {
        let never = RestartRule::default();
        let on_failure = RestartRule::new(RestartMode::OnFailure, Duration::ZERO);
        let always = RestartRule::new(RestartMode::Always, Duration::ZERO).keep_down_on_exit([2]);
        let cases = [
            (&never, ProcessExit::Code(1), false),
            (&never, ProcessExit::Signal, false),
            (&on_failure, ProcessExit::Code(0), false),
            (&on_failure, ProcessExit::Code(1), true),
            (&on_failure, ProcessExit::Signal, true),
            (&always, ProcessExit::Code(0), true),
            (&always, ProcessExit::Code(1), true),
            (&always, ProcessExit::Code(2), false),
            (&always, ProcessExit::Signal, true),
        ];
        for (rule, exit, restarts) in cases {
            assert_eq!(
                rule.restarts_after(exit),
                restarts,
                "{rule:?} after {exit:?}"
            );
        }
        assert_eq!(never.policy, RestartMode::Never);
    }
}
