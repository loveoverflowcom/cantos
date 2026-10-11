//! Loopback readiness probes and port ownership checks, using only the standard library.

use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    time::Duration,
};

const CONNECT_TIMEOUT: Duration = Duration::from_millis(500);
const IO_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_RESPONSE: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Readiness {
    /// The process accepts TCP connections.
    Tcp,
    /// `GET path` answers 200 with a body containing `marker`.
    Http {
        path: &'static str,
        marker: &'static str,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Probe {
    Ready,
    /// Something answered, but not (yet) the expected application.
    Unexpected(String),
    Unavailable,
}

/// True when nothing listens on `addr`. A port that is taken belongs to someone else unless
/// xtask started it in this invocation; xtask never stops such a process.
pub fn port_is_free(addr: SocketAddr) -> bool {
    TcpListener::bind(addr).is_ok()
}

pub fn accepts_connections(addr: SocketAddr) -> bool {
    TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT).is_ok()
}

pub fn probe(addr: SocketAddr, readiness: &Readiness) -> Probe {
    match readiness {
        Readiness::Tcp => {
            if accepts_connections(addr) {
                Probe::Ready
            } else {
                Probe::Unavailable
            }
        }
        Readiness::Http { path, marker } => match http_get(addr, path) {
            Ok((200, body)) if body.contains(marker) => Probe::Ready,
            Ok((200, _)) => Probe::Unexpected(format!("HTTP 200 without `{marker}`")),
            Ok((status, _)) => Probe::Unexpected(format!("HTTP {status}")),
            Err(_) => Probe::Unavailable,
        },
    }
}

/// Minimal HTTP/1.1 GET with `Connection: close`; returns the status code and body text.
pub fn http_get(addr: SocketAddr, path: &str) -> std::io::Result<(u16, String)> {
    let mut stream = TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT)?;
    stream.set_read_timeout(Some(IO_TIMEOUT))?;
    stream.set_write_timeout(Some(IO_TIMEOUT))?;
    let request = format!(
        "GET {path} HTTP/1.1\r\nHost: {addr}\r\nAccept: text/html\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(request.as_bytes())?;
    let mut raw = Vec::new();
    stream.take(MAX_RESPONSE).read_to_end(&mut raw)?;
    let text = String::from_utf8_lossy(&raw);
    let (head, body) = text.split_once("\r\n\r\n").unwrap_or((&text, ""));
    let status = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse().ok())
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "malformed HTTP status line",
            )
        })?;
    Ok((status, body.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    fn serve_once(response: &'static str) -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut byte = [0_u8; 1];
            while !request.ends_with(b"\r\n\r\n") && stream.read(&mut byte).unwrap() == 1 {
                request.push(byte[0]);
            }
            let _ = stream.write_all(response.as_bytes());
        });
        addr
    }

    const HTML: Readiness = Readiness::Http {
        path: "/",
        marker: "<title>Cantos Studio</title>",
    };

    #[test]
    fn http_probe_distinguishes_ready_wrong_app_and_unavailable() {
        let ready = serve_once(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n<title>Cantos Studio</title>",
        );
        assert_eq!(probe(ready, &HTML), Probe::Ready);

        let other = serve_once("HTTP/1.1 200 OK\r\n\r\n<title>Something else</title>");
        assert!(matches!(probe(other, &HTML), Probe::Unexpected(_)));

        let missing = serve_once("HTTP/1.1 404 Not Found\r\n\r\n");
        assert_eq!(probe(missing, &HTML), Probe::Unexpected("HTTP 404".into()));

        let closed = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap();
        assert_eq!(probe(closed, &HTML), Probe::Unavailable);
        assert_eq!(probe(closed, &Readiness::Tcp), Probe::Unavailable);
    }

    #[test]
    fn port_ownership_reflects_an_existing_listener() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        assert!(!port_is_free(addr));
        drop(listener);
        assert!(port_is_free(addr));
        let accepting = serve_once("HTTP/1.1 204 No Content\r\n\r\n");
        assert_eq!(probe(accepting, &Readiness::Tcp), Probe::Ready);
    }
}
