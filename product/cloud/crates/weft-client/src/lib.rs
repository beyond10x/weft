//! The HTTP client of the control host, shared by the tests, the node agent and the conformance
//! adapter: send a command as an actor with its caller, read a view, read the control host log
//! after a `seq`.
//!
//! One request per connection, as the generated surface answers (`Connection: close`). The
//! caller travels in the header `X-Weft-Caller`, `{"actor": "<qualified actor>", "caller":
//! {...}}`, which only a test build of the control host reads (cargo feature `test-callers` of
//! `weft-bin`); authentication proper is a later epic.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::time::Duration;

use serde_json::Value;

/// The header that names the caller of a request.
pub const CALLER_HEADER: &str = "X-Weft-Caller";

/// How long one request may take before the client gives it up.
const TIMEOUT: Duration = Duration::from_secs(20);

/// A client of one control host.
#[derive(Debug, Clone, Copy)]
pub struct Client {
    address: SocketAddr,
}

/// One answer: the status and the JSON body.
#[derive(Debug, Clone, PartialEq)]
pub struct Answer {
    /// The HTTP status.
    pub status: u16,
    /// The body, parsed; a body that is not JSON is kept as a JSON string.
    pub body: Value,
}

impl Client {
    /// A client of the control host listening on `address`.
    pub fn new(address: SocketAddr) -> Self {
        Self { address }
    }

    /// The address this client sends to.
    pub fn address(&self) -> SocketAddr {
        self.address
    }

    /// Sends the command `command` (a qualified name, `weft.accounts.CreateTenant`) as `actor`
    /// (`weft.actors.ClusterOperator`) with the caller attributes `caller` (an object, such as
    /// `{"tenant_id": "..."}`) and the input `input`.
    ///
    /// # Errors
    ///
    /// What the connection refuses, or an answer that is not HTTP.
    pub fn command(
        &self,
        command: &str,
        actor: &str,
        caller: Value,
        input: Value,
    ) -> io::Result<Answer> {
        let path = format!("/weft/commands/{}", short_name(command));
        let header = caller_header(actor, caller);
        self.send("POST", &path, Some(&header), input.to_string().as_bytes())
    }

    /// Reads the view `view` (a qualified name, `weft.accounts.Tenants`) with the parameters
    /// `params`, as no caller.
    ///
    /// # Errors
    ///
    /// What the connection refuses, or an answer that is not HTTP.
    pub fn view(&self, view: &str, params: &[(&str, &str)]) -> io::Result<Answer> {
        self.send("GET", &view_target(view, params), None, &[])
    }

    /// Reads the view `view` as `actor` with the caller attributes `caller`, for a view an
    /// actor's grant names.
    ///
    /// # Errors
    ///
    /// What the connection refuses, or an answer that is not HTTP.
    pub fn view_as(
        &self,
        view: &str,
        params: &[(&str, &str)],
        actor: &str,
        caller: Value,
    ) -> io::Result<Answer> {
        let header = caller_header(actor, caller);
        self.send("GET", &view_target(view, params), Some(&header), &[])
    }

    /// Posts the JSON `body` to `path` as no caller: a frame of the node channel (`/node/...`).
    ///
    /// # Errors
    ///
    /// What the connection refuses, or an answer that is not HTTP.
    pub fn post(&self, path: &str, body: &Value) -> io::Result<Answer> {
        self.send("POST", path, None, body.to_string().as_bytes())
    }

    fn send(
        &self,
        method: &str,
        target: &str,
        caller: Option<&str>,
        body: &[u8],
    ) -> io::Result<Answer> {
        let mut stream = TcpStream::connect_timeout(&self.address, TIMEOUT)?;
        stream.set_read_timeout(Some(TIMEOUT))?;
        stream.set_write_timeout(Some(TIMEOUT))?;
        let mut head = format!(
            "{method} {target} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n",
            self.address,
            body.len()
        );
        if let Some(caller) = caller {
            head.push_str(&format!("{CALLER_HEADER}: {caller}\r\n"));
        }
        head.push_str("\r\n");
        stream.write_all(head.as_bytes())?;
        stream.write_all(body)?;
        stream.flush()?;
        read_answer(stream)
    }
}

/// The value of the caller header for `actor` with the attributes `caller`.
pub fn caller_header(actor: &str, caller: Value) -> String {
    serde_json::json!({"actor": actor, "caller": caller}).to_string()
}

/// The last segment of a qualified name: the route name of a command or view.
fn short_name(name: &str) -> &str {
    name.rsplit('.').next().unwrap_or(name)
}

fn view_target(view: &str, params: &[(&str, &str)]) -> String {
    let mut target = format!("/weft/views/{}", short_name(view));
    for (index, (key, value)) in params.iter().enumerate() {
        target.push(if index == 0 { '?' } else { '&' });
        target.push_str(&encode(key));
        target.push('=');
        target.push_str(&encode(value));
    }
    target
}

/// Percent-encodes every byte outside the unreserved set of RFC 3986.
fn encode(text: &str) -> String {
    let mut out = String::new();
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

fn read_answer(stream: TcpStream) -> io::Result<Answer> {
    let mut reader = BufReader::new(stream);
    let mut status_line = String::new();
    reader.read_line(&mut status_line)?;
    let status = status_line
        .split(' ')
        .nth(1)
        .and_then(|code| code.parse::<u16>().ok())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("not an HTTP status line: {status_line:?}"),
            )
        })?;
    let mut length = None;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            if name.trim().eq_ignore_ascii_case("content-length") {
                length = value.trim().parse::<usize>().ok();
            }
        }
    }
    let mut body = Vec::new();
    match length {
        Some(length) => {
            body.resize(length, 0);
            reader.read_exact(&mut body)?;
        }
        None => {
            reader.read_to_end(&mut body)?;
        }
    }
    let text = String::from_utf8_lossy(&body).into_owned();
    let body = serde_json::from_str(&text).unwrap_or(Value::String(text));
    Ok(Answer { status, body })
}

/// The expectation a test writes on an answer.
pub trait Expect {
    /// The body of an answer with status `status`; panics, naming the answer, on any other
    /// status or on a transport failure.
    fn ok_body(self, status: u16) -> Value;
}

impl Expect for io::Result<Answer> {
    fn ok_body(self, status: u16) -> Value {
        match self {
            Ok(answer) if answer.status == status => answer.body,
            Ok(answer) => panic!(
                "expected status {status}, the control host answered {} {}",
                answer.status, answer.body
            ),
            Err(error) => panic!("the control host did not answer: {error}"),
        }
    }
}

/// The lines of the control host log at `path` whose `seq` is above `seq`, in file order. A log
/// that does not exist yet has no lines.
///
/// # Errors
///
/// What reading the file refuses, or a line that is not JSON with a `seq`.
pub fn log_after(path: &Path, seq: u64) -> io::Result<Vec<Value>> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut lines = Vec::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let value: Value = serde_json::from_str(line).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("log line {line:?}: {error}"),
            )
        })?;
        let line_seq = value["seq"].as_u64().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("log line without seq: {line}"),
            )
        })?;
        if line_seq > seq {
            lines.push(value);
        }
    }
    Ok(lines)
}

/// The highest `seq` of the control host log at `path`, `0` for an empty or absent log.
///
/// # Errors
///
/// As [`log_after`].
pub fn last_seq(path: &Path) -> io::Result<u64> {
    Ok(log_after(path, 0)?
        .last()
        .and_then(|line| line["seq"].as_u64())
        .unwrap_or(0))
}
