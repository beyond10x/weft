//! `weft-agent`: the node agent binary.
//!
//! ```text
//! weft-agent --control http://<host>:<port> --node-id <id> --join-token <token> --substrate <socket> --report <path>
//! ```
//!
//! Joins its node, reports what it observed and opens its session on the node channel of the
//! control host ([`weft_agent::agent`]), writing each answer to its `hello` to the report file
//! ([`weft_agent::report`]). Exit 2 on bad arguments, 1 on a failure.

use std::process::ExitCode;

use weft_agent::agent::{self, Options};

fn main() -> ExitCode {
    let options = match Options::parse(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("weft-agent: {error}\n{}", agent::USAGE);
            return ExitCode::from(2);
        }
    };
    match agent::run(&options) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("weft-agent: {error}");
            ExitCode::FAILURE
        }
    }
}
