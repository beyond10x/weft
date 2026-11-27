//! `weft`: the control host binary.
//!
//! ```text
//! weft --state <path> [--log <path>] [--port <n>] [--listen <address>]
//! ```
//!
//! Serves the generated HTTP surfaces of `weft-api` and `weft-control` on loopback (default
//! `127.0.0.1`, port 0 binds an ephemeral one) over the SQLite state file `--state`, writing the
//! control host log to `--log` (default: the state file's path with `.log` appended). Once it
//! listens it prints one line, `listening <address> callers=<mode>`: `test-header` where the
//! build trusts the `X-Weft-Caller` header (cargo feature `test-callers`), `none` otherwise, in
//! which case every request is answered as no caller. The `test-header` build refuses a
//! `--listen` that is not loopback (exit 2). A state file another control host holds is refused
//! (exit 1); what opening repaired in the log is written to stderr.

use std::io::Write;
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::ExitCode;

/// Whether this build trusts the caller header.
const TRUST_CALLERS: bool = cfg!(feature = "test-callers");

struct Options {
    state: PathBuf,
    log: PathBuf,
    listen: String,
    port: u16,
}

fn options() -> Result<Options, String> {
    let mut state = None;
    let mut log = None;
    let mut listen = "127.0.0.1".to_owned();
    let mut port = 0;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = |name: &str| args.next().ok_or_else(|| format!("{name} needs a value"));
        match arg.as_str() {
            "--state" => state = Some(PathBuf::from(value("--state")?)),
            "--log" => log = Some(PathBuf::from(value("--log")?)),
            "--listen" => listen = value("--listen")?,
            "--port" => {
                let text = value("--port")?;
                port = text
                    .parse()
                    .map_err(|_| format!("--port {text} is not a port"))?;
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    let state = state.ok_or("--state <path> is required")?;
    let log = log.unwrap_or_else(|| {
        let mut path = state.clone().into_os_string();
        path.push(".log");
        PathBuf::from(path)
    });
    Ok(Options {
        state,
        log,
        listen,
        port,
    })
}

fn main() -> ExitCode {
    let options = match options() {
        Ok(options) => options,
        Err(error) => {
            eprintln!("weft: {error}\nusage: weft --state <path> [--log <path>] [--port <n>] [--listen <address>]");
            return ExitCode::from(2);
        }
    };
    let addresses = match weft_bin::listen_addresses(&options.listen, options.port, TRUST_CALLERS) {
        Ok(addresses) => addresses,
        Err(error) => {
            eprintln!("weft: {error}");
            return ExitCode::from(2);
        }
    };
    let host = match weft_bin::Host::open(&options.state, &options.log) {
        Ok(host) => host,
        Err(error) => {
            eprintln!("weft: {error}");
            return ExitCode::FAILURE;
        }
    };
    for note in host.notes() {
        eprintln!("weft: {note}");
    }
    let listener = match TcpListener::bind(&addresses[..]) {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!(
                "weft: cannot listen on {}:{}: {error}",
                options.listen, options.port
            );
            return ExitCode::FAILURE;
        }
    };
    let address = match listener.local_addr() {
        Ok(address) => address,
        Err(error) => {
            eprintln!("weft: {error}");
            return ExitCode::FAILURE;
        }
    };
    let mode = if TRUST_CALLERS { "test-header" } else { "none" };
    let mut stdout = std::io::stdout();
    if writeln!(stdout, "listening {address} callers={mode}")
        .and_then(|()| stdout.flush())
        .is_err()
    {
        return ExitCode::FAILURE;
    }
    host.serve(&listener, TRUST_CALLERS);
    ExitCode::FAILURE
}
