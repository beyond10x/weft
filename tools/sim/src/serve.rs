//! `--serve <addr>`: the page over HTTP while a run goes, progress pushed with Server-Sent Events.
//! Loopback only.

use std::io::Read;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::Value;
use tiny_http::{Header, Response, Server, StatusCode};

use crate::catalog::Catalog;
use crate::coverage::Coverage;
use crate::report;

pub struct Live {
    pub cat: Arc<Catalog>,
    pub cov: Arc<Coverage>,
    pub model: Value,
    pub last: Mutex<Option<Value>>,
}

impl Live {
    fn snapshot(&self) -> Value {
        if let Some(done) = self.last.lock().ok().and_then(|l| l.clone()) {
            return done;
        }
        self.cov.snapshot(&self.cat, false)
    }
}

pub fn check_addr(addr: &str) -> Result<SocketAddr, String> {
    let parsed: SocketAddr = addr
        .parse()
        .map_err(|e| format!("--serve {addr}: not an address: {e}"))?;
    if !parsed.ip().is_loopback() {
        return Err(format!("--serve {addr}: refused, not a loopback address"));
    }
    Ok(parsed)
}

struct Sse {
    live: Arc<Live>,
    buf: Vec<u8>,
    pos: usize,
    first: bool,
    finished: bool,
}

impl Read for Sse {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        if self.pos >= self.buf.len() {
            if self.finished {
                return Ok(0);
            }
            if !self.first {
                std::thread::sleep(Duration::from_millis(600));
            }
            self.first = false;
            let snap = self.live.snapshot();
            if snap["running"] == Value::Bool(false) && self.live.last.lock().map(|l| l.is_some()).unwrap_or(false) {
                self.finished = true;
            }
            self.buf = format!("data: {snap}\n\n").into_bytes();
            self.pos = 0;
        }
        let n = (self.buf.len() - self.pos).min(out.len());
        out[..n].copy_from_slice(&self.buf[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}

fn header(name: &str, value: &str) -> Header {
    Header::from_bytes(name.as_bytes(), value.as_bytes()).expect("header")
}

/// Starts the server on its own thread; each request gets a thread of its own, so a held event
/// stream blocks nothing.
pub fn start(addr: SocketAddr, live: Arc<Live>) -> Result<(), String> {
    let server = Server::http(addr).map_err(|e| format!("--serve {addr}: {e}"))?;
    std::thread::spawn(move || {
        for request in server.incoming_requests() {
            let live = live.clone();
            std::thread::spawn(move || {
                let path = request.url().split('?').next().unwrap_or("/").to_owned();
                let result = match path.as_str() {
                    "/" | "/index.html" => {
                        let page = report::page(&live.model, &Value::Null, true);
                        request.respond(
                            Response::from_string(page)
                                .with_header(header("Content-Type", "text/html; charset=utf-8")),
                        )
                    }
                    "/snapshot" => request.respond(
                        Response::from_string(live.snapshot().to_string())
                            .with_header(header("Content-Type", "application/json")),
                    ),
                    "/model" => request.respond(
                        Response::from_string(live.model.to_string())
                            .with_header(header("Content-Type", "application/json")),
                    ),
                    "/events" => {
                        let reader = Sse { live, buf: Vec::new(), pos: 0, first: true, finished: false };
                        request.respond(Response::new(
                            StatusCode(200),
                            vec![
                                header("Content-Type", "text/event-stream"),
                                header("Cache-Control", "no-cache"),
                            ],
                            reader,
                            None,
                            None,
                        ))
                    }
                    _ => request.respond(Response::from_string("not found").with_status_code(404)),
                };
                let _ = result;
            });
        }
    });
    Ok(())
}
