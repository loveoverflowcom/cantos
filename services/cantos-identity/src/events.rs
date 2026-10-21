//! Operational events. Only `'static` text can be emitted, so a request value, secret, address
//! or password cannot reach a log line by construction.

use std::io::Write;

pub fn emit(event: &'static str, detail: &'static str) {
    let line = format!("{{\"event\":\"{event}\",\"detail\":\"{detail}\"}}\n");
    // A closed stderr must never fail a request.
    let _ = std::io::stderr().write_all(line.as_bytes());
}
