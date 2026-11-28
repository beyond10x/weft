//! weft-probe <host:port>: opens one TCP connection to the address and prints what happened, one
//! line: `connected <addr>`, `refused <addr>: <error>`, `unreachable <addr>: <error>`,
//! `timed-out <addr>: <error>` or `failed <addr>: <error>`. Exits 0 only when it connected, 1
//! when it did not, 2 on a usage error. The confinement test of weft-platform runs it unconfined
//! and confined with the same arguments.

use std::io::ErrorKind;
use std::net::{SocketAddr, TcpStream};
use std::process::ExitCode;
use std::time::Duration;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let (Some(target), None) = (args.next(), args.next()) else {
        eprintln!("usage: weft-probe <host:port>");
        return ExitCode::from(2);
    };
    let address: SocketAddr = match target.parse() {
        Ok(address) => address,
        Err(error) => {
            eprintln!("weft-probe: {target} is no socket address: {error}");
            return ExitCode::from(2);
        }
    };
    match TcpStream::connect_timeout(&address, Duration::from_secs(3)) {
        Ok(_) => {
            println!("connected {address}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            let outcome = match error.kind() {
                ErrorKind::ConnectionRefused => "refused",
                ErrorKind::HostUnreachable | ErrorKind::NetworkUnreachable => "unreachable",
                ErrorKind::TimedOut => "timed-out",
                _ => "failed",
            };
            println!("{outcome} {address}: {error}");
            ExitCode::FAILURE
        }
    }
}
