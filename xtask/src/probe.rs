//! Bounded loopback readiness probes and port ownership checks.

use std::{
    io::{self, BufRead, BufReader, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

const CONNECT_TIMEOUT: Duration = Duration::from_millis(500);
const IO_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_RESPONSE: usize = 4 * 1024 * 1024;
const MAX_HEAD: usize = 32 * 1024;
const THEATRE_CATALOG_PATH: &str = "/api/v1/theatre/works?limit=20";

/// Where a supervised process is observed: a loopback TCP address, or a private Unix socket
/// (Identity's RPC endpoint has no TCP port).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Endpoint {
    Tcp(SocketAddr),
    Unix(PathBuf),
}

impl std::fmt::Display for Endpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Endpoint::Tcp(addr) => write!(f, "{addr}"),
            Endpoint::Unix(path) => write!(f, "{}", path.display()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Readiness {
    /// The process accepts connections (TCP, or a Unix socket for an [`Endpoint::Unix`]).
    Tcp,
    /// `GET path` answers 200 with a body containing `marker`.
    Http {
        path: &'static str,
        marker: &'static str,
    },
    /// The anonymous Theatre works endpoint answers with a JSON WorkPage envelope.
    TheatreCatalog,
    /// Theatre HTML and its same-origin proxied catalog both answer successfully.
    TheatreWeb,
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

/// True when something accepts a connection on this Unix socket. A connect and an immediate
/// close: no byte is sent, so the peer sees an empty connection and nothing is authenticated.
pub fn accepts_unix(path: &Path) -> bool {
    #[cfg(unix)]
    {
        std::os::unix::net::UnixStream::connect(path).is_ok()
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        false
    }
}

pub fn probe_endpoint(endpoint: &Endpoint, readiness: &Readiness) -> Probe {
    match (endpoint, readiness) {
        (Endpoint::Tcp(addr), readiness) => probe(*addr, readiness),
        (Endpoint::Unix(path), Readiness::Tcp) => {
            if accepts_unix(path) {
                Probe::Ready
            } else {
                Probe::Unavailable
            }
        }
        (Endpoint::Unix(_), other) => {
            Probe::Unexpected(format!("{other:?} readiness needs a TCP endpoint"))
        }
    }
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
        Readiness::TheatreCatalog => theatre_catalog(addr),
        Readiness::TheatreWeb => theatre_web(addr),
    }
}

fn theatre_web(addr: SocketAddr) -> Probe {
    let html = Readiness::Http {
        path: "/",
        marker: "<title>Cantos</title>",
    };
    match probe(addr, &html) {
        Probe::Ready => match theatre_catalog(addr) {
            Probe::Unavailable => {
                Probe::Unexpected("Theatre catalog /api/v1/theatre/works unavailable".into())
            }
            result => result,
        },
        Probe::Unexpected(reason) => Probe::Unexpected(format!("Theatre Web HTML: {reason}")),
        Probe::Unavailable => Probe::Unexpected("Theatre Web HTML unavailable".into()),
    }
}

fn theatre_catalog(addr: SocketAddr) -> Probe {
    let response = match http_get_with_accept(addr, THEATRE_CATALOG_PATH, "application/json") {
        Ok(response) => response,
        Err(_) => return Probe::Unavailable,
    };
    if response.status != 200 {
        return Probe::Unexpected(format!(
            "Theatre catalog /api/v1/theatre/works returned HTTP {}",
            response.status
        ));
    }
    if !response.content_type.as_deref().is_some_and(|value| {
        value
            .split(';')
            .next()
            .is_some_and(|media| media.trim().eq_ignore_ascii_case("application/json"))
    }) {
        return Probe::Unexpected(
            "Theatre catalog /api/v1/theatre/works did not return application/json".into(),
        );
    }
    // Catalog envelope smoke only: the API/client own WorkSummary validation and full DTO
    // conformance. Avoid copying that domain schema or adding a contract-crate dependency here.
    let valid_page =
        serde_json::from_slice::<serde_json::Value>(&response.body).is_ok_and(|page| {
            page.get("items")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|items| items.iter().all(serde_json::Value::is_object))
                && page
                    .get("next_cursor")
                    .is_some_and(|cursor| cursor.is_null() || cursor.is_string())
        });
    if valid_page {
        Probe::Ready
    } else {
        Probe::Unexpected(
            "Theatre catalog /api/v1/theatre/works returned an invalid WorkPage envelope".into(),
        )
    }
}

/// Bounded HTTP/1.1 GET with `Connection: close`; returns the status code and UTF-8 body text.
/// Content-Length and chunked framing are honored without waiting for a persistent peer to close.
pub fn http_get(addr: SocketAddr, path: &str) -> io::Result<(u16, String)> {
    let response = http_get_with_accept(addr, path, "text/html")?;
    let body = String::from_utf8(response.body).map_err(|_| invalid("HTTP body is not UTF-8"))?;
    Ok((response.status, body))
}

pub(crate) struct HttpResponse {
    pub status: u16,
    content_type: Option<String>,
    pub body: Vec<u8>,
}

fn http_get_with_accept(addr: SocketAddr, path: &str, accept: &str) -> io::Result<HttpResponse> {
    http_request(addr, "GET", path, accept, &[], &[])
}

/// Reuses the readiness parser's whole-exchange deadline and framing/size bounds. No proxy,
/// DNS resolution, redirect or retry is performed. Explicit mail fixtures use literal loopback.
pub(crate) fn http_request(
    addr: SocketAddr,
    method: &str,
    path: &str,
    accept: &str,
    headers: &[(&str, &str)],
    body: &[u8],
) -> io::Result<HttpResponse> {
    if !matches!(method, "GET" | "POST")
        || body.len() > 4096
        || accept.bytes().any(|byte| byte.is_ascii_control())
        || headers.iter().any(|(name, value)| {
            name.is_empty()
                || !name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
                || value.bytes().any(|byte| byte.is_ascii_control())
        })
    {
        return Err(invalid("invalid HTTP request"));
    }
    if !path.starts_with('/')
        || path.len() > 4096
        || path
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte == b' ')
    {
        return Err(invalid("invalid HTTP request path"));
    }
    let mut stream = TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT)?;
    let deadline = Instant::now() + IO_TIMEOUT;
    let mut request = format!(
        "{method} {path} HTTP/1.1\r\nHost: {addr}\r\nAccept: {accept}\r\nConnection: close\r\n"
    );
    for (name, value) in headers {
        request.push_str(&format!("{name}: {value}\r\n"));
    }
    if method == "POST" {
        request.push_str(&format!(
            "Content-Type: application/json\r\nContent-Length: {}\r\n",
            body.len()
        ));
    }
    request.push_str("\r\n");
    let mut request = request.into_bytes();
    request.extend_from_slice(body);
    let mut remaining_request = request.as_slice();
    while !remaining_request.is_empty() {
        stream.set_write_timeout(Some(time_left(deadline)?))?;
        let written = stream.write(remaining_request)?;
        if written == 0 {
            return Err(io::Error::new(
                io::ErrorKind::WriteZero,
                "incomplete HTTP request",
            ));
        }
        remaining_request = &remaining_request[written..];
    }
    let mut reader = BufReader::new(BoundedResponse {
        stream,
        deadline,
        received: 0,
    });
    read_response(&mut reader)
}

/// Both bounds apply to the entire exchange; a peer cannot extend the deadline by dripping bytes.
struct BoundedResponse {
    stream: TcpStream,
    deadline: Instant,
    received: usize,
}

impl Read for BoundedResponse {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        self.stream
            .set_read_timeout(Some(time_left(self.deadline)?))?;
        let limit = buffer
            .len()
            .min(MAX_RESPONSE.saturating_sub(self.received) + 1);
        let count = self.stream.read(&mut buffer[..limit])?;
        self.received += count;
        if self.received > MAX_RESPONSE {
            return Err(invalid("HTTP response exceeds size limit"));
        }
        Ok(count)
    }
}

fn time_left(deadline: Instant) -> io::Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "HTTP exchange deadline exceeded"))
}

#[derive(Clone, Copy)]
enum Framing {
    Close,
    Length(usize),
    Chunked,
}

fn read_response(reader: &mut impl BufRead) -> io::Result<HttpResponse> {
    let mut head_bytes = 0;
    let status_line = read_head_line(reader, &mut head_bytes)?;
    let status_text =
        std::str::from_utf8(&status_line).map_err(|_| invalid("malformed HTTP status line"))?;
    let mut status_parts = status_text.splitn(3, ' ');
    let version = status_parts.next();
    let code = status_parts.next().unwrap_or_default();
    if !matches!(version, Some("HTTP/1.0" | "HTTP/1.1"))
        || code.len() != 3
        || !code.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(invalid("malformed HTTP status line"));
    }
    let status = code
        .parse()
        .map_err(|_| invalid("malformed HTTP status line"))?;
    let mut content_type = None;
    let mut content_length = None;
    let mut chunked = false;
    loop {
        let line = read_head_line(reader, &mut head_bytes)?;
        if line.is_empty() {
            break;
        }
        let (name, value) = parse_header(&line)?;
        if name.eq_ignore_ascii_case("content-type") {
            if content_type.replace(value.to_owned()).is_some() {
                return Err(invalid("duplicate HTTP content type"));
            }
        } else if name.eq_ignore_ascii_case("content-length") {
            if content_length.is_some()
                || value.is_empty()
                || !value.bytes().all(|byte| byte.is_ascii_digit())
            {
                return Err(invalid("invalid HTTP content length"));
            }
            let length = value
                .parse::<usize>()
                .map_err(|_| invalid("invalid HTTP content length"))?;
            if length > MAX_RESPONSE {
                return Err(invalid("HTTP response exceeds size limit"));
            }
            content_length = Some(length);
        } else if name.eq_ignore_ascii_case("transfer-encoding") {
            if chunked || !value.eq_ignore_ascii_case("chunked") {
                return Err(invalid("unsupported HTTP transfer encoding"));
            }
            chunked = true;
        }
    }
    let framing = match (content_length, chunked) {
        (Some(_), true) => return Err(invalid("ambiguous HTTP response framing")),
        (_, true) => Framing::Chunked,
        (Some(length), false) => Framing::Length(length),
        (None, false) => Framing::Close,
    };
    let body = match framing {
        Framing::Length(length) => {
            let mut body = vec![0; length];
            reader.read_exact(&mut body)?;
            body
        }
        Framing::Chunked => read_chunked(reader)?,
        Framing::Close => {
            let mut body = Vec::new();
            reader
                .take(MAX_RESPONSE as u64 + 1)
                .read_to_end(&mut body)?;
            if body.len() > MAX_RESPONSE {
                return Err(invalid("HTTP response exceeds size limit"));
            }
            body
        }
    };
    Ok(HttpResponse {
        status,
        content_type,
        body,
    })
}

fn read_head_line(reader: &mut impl BufRead, total: &mut usize) -> io::Result<Vec<u8>> {
    let line = read_crlf_line(reader, MAX_HEAD.saturating_sub(*total))?;
    *total += line.len() + 2;
    Ok(line)
}

fn read_crlf_line(reader: &mut impl BufRead, limit: usize) -> io::Result<Vec<u8>> {
    let mut line = Vec::new();
    reader.take(limit as u64 + 1).read_until(b'\n', &mut line)?;
    if line.len() > limit || !line.ends_with(b"\r\n") {
        return Err(invalid("malformed or oversized HTTP line"));
    }
    line.truncate(line.len() - 2);
    Ok(line)
}

fn parse_header(line: &[u8]) -> io::Result<(&str, &str)> {
    let line = std::str::from_utf8(line).map_err(|_| invalid("malformed HTTP header"))?;
    let (name, value) = line
        .split_once(':')
        .ok_or_else(|| invalid("malformed HTTP header"))?;
    if name.is_empty()
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte))
        || value
            .bytes()
            .any(|byte| byte.is_ascii_control() && byte != b'\t')
    {
        return Err(invalid("malformed HTTP header"));
    }
    Ok((name, value.trim()))
}

fn read_chunked(reader: &mut impl BufRead) -> io::Result<Vec<u8>> {
    let mut body = Vec::new();
    loop {
        let line = read_crlf_line(reader, 1024)?;
        let text = std::str::from_utf8(&line).map_err(|_| invalid("malformed HTTP chunk size"))?;
        let size = text.split(';').next().unwrap_or_default();
        if size.is_empty() || !size.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(invalid("malformed HTTP chunk size"));
        }
        let size =
            usize::from_str_radix(size, 16).map_err(|_| invalid("malformed HTTP chunk size"))?;
        if size == 0 {
            let mut trailer_bytes = 0;
            loop {
                let trailer = read_head_line(reader, &mut trailer_bytes)?;
                if trailer.is_empty() {
                    return Ok(body);
                }
                parse_header(&trailer)?;
            }
        }
        let length = body
            .len()
            .checked_add(size)
            .filter(|length| *length <= MAX_RESPONSE)
            .ok_or_else(|| invalid("HTTP response exceeds size limit"))?;
        let start = body.len();
        body.resize(length, 0);
        reader.read_exact(&mut body[start..])?;
        let mut ending = [0; 2];
        reader.read_exact(&mut ending)?;
        if ending != *b"\r\n" {
            return Err(invalid("malformed HTTP chunk ending"));
        }
    }
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::mpsc, thread};

    fn serve_responses(responses: Vec<Vec<u8>>) -> (SocketAddr, mpsc::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let (sender, requests) = mpsc::channel();
        thread::spawn(move || {
            for response in responses {
                let (mut stream, _) = listener.accept().unwrap();
                stream.set_read_timeout(Some(IO_TIMEOUT)).unwrap();
                let mut request = Vec::new();
                let mut byte = [0_u8; 1];
                while !request.ends_with(b"\r\n\r\n") && stream.read(&mut byte).unwrap() == 1 {
                    request.push(byte[0]);
                }
                let _ = sender.send(String::from_utf8(request).unwrap());
                let _ = stream.write_all(&response);
            }
        });
        (addr, requests)
    }

    fn serve_once(response: &str) -> SocketAddr {
        serve_responses(vec![response.as_bytes().to_vec()]).0
    }

    fn json_response(body: &str) -> Vec<u8> {
        format!("HTTP/1.1 200 OK\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\n\r\n{body}", body.len()).into_bytes()
    }

    const HTML: Readiness = Readiness::Http {
        path: "/",
        marker: "<title>Cantos Studio</title>",
    };

    const WORK_PAGE: &str = include_str!(
        "../../services/cantos-server/contracts/theatre/discovery/v1/fixtures/works.json"
    );

    #[test]
    fn http_probe_distinguishes_ready_wrong_app_and_unavailable() {
        let ready = serve_once(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n<title>Cantos Studio</title>",
        );
        assert_eq!(probe(ready, &HTML), Probe::Ready);

        let other = serve_once("HTTP/1.1 200 OK\r\n\r\n<title>Something else</title>");
        assert_eq!(
            probe(other, &HTML),
            Probe::Unexpected("HTTP 200 without `<title>Cantos Studio</title>`".into())
        );

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
    fn catalogue_requires_anonymous_exact_path_and_json_accept() {
        let (addr, requests) =
            serve_responses(vec![json_response(r#"{"items":[],"next_cursor":null}"#)]);
        assert_eq!(probe(addr, &Readiness::TheatreCatalog), Probe::Ready);
        assert_eq!(requests.recv().unwrap(), format!("GET /api/v1/theatre/works?limit=20 HTTP/1.1\r\nHost: {addr}\r\nAccept: application/json\r\nConnection: close\r\n\r\n"));
    }

    #[test]
    fn catalogue_accepts_empty_nonempty_and_intermediate_pages() {
        for body in [
            r#"{"items":[],"next_cursor":null}"#,
            WORK_PAGE,
            r#"{"items":[],"next_cursor":"lc1_next"}"#,
        ] {
            let (addr, _) = serve_responses(vec![json_response(body)]);
            assert_eq!(
                probe(addr, &Readiness::TheatreCatalog),
                Probe::Ready,
                "{body}"
            );
        }
    }

    #[test]
    fn catalogue_envelope_rejects_non_object_items() {
        for body in [
            r#"{"items":[null],"next_cursor":null}"#,
            r#"{"items":[42],"next_cursor":null}"#,
            r#"{"items":["work"],"next_cursor":null}"#,
            r#"{"items":[[]],"next_cursor":null}"#,
        ] {
            let (addr, _) = serve_responses(vec![json_response(body)]);
            assert_eq!(
                probe(addr, &Readiness::TheatreCatalog),
                Probe::Unexpected(
                    "Theatre catalog /api/v1/theatre/works returned an invalid WorkPage envelope"
                        .into()
                )
            );
        }
    }

    #[test]
    fn catalogue_rejects_other_statuses_including_studio_session_success() {
        for status in [201, 204, 301, 401, 403, 404, 500, 503] {
            let response = format!("HTTP/1.1 {status} Result\r\nContent-Type: application/json\r\n\r\n{{\"items\":[],\"next_cursor\":null}}");
            assert_eq!(
                probe(serve_once(&response), &Readiness::TheatreCatalog),
                Probe::Unexpected(format!(
                    "Theatre catalog /api/v1/theatre/works returned HTTP {status}"
                ))
            );
        }
        let (addr, requests) = serve_responses(vec![json_response(
            r#"{"authenticated":true,"user_id":"private-user"}"#,
        )]);
        assert_eq!(
            probe(addr, &Readiness::TheatreCatalog),
            Probe::Unexpected(
                "Theatre catalog /api/v1/theatre/works returned an invalid WorkPage envelope"
                    .into()
            )
        );
        assert!(!requests.recv().unwrap().contains("/session"));
    }

    #[test]
    fn catalogue_requires_json_media_type_and_valid_envelope() {
        for content_type in [
            "",
            "Content-Type: text/html\r\n",
            "Content-Type: application/msgpack\r\n",
            "Content-Type: application/jsonp\r\n",
        ] {
            let response = format!(
                "HTTP/1.1 200 OK\r\n{content_type}\r\n{{\"items\":[],\"next_cursor\":null}}"
            );
            assert_eq!(
                probe(serve_once(&response), &Readiness::TheatreCatalog),
                Probe::Unexpected(
                    "Theatre catalog /api/v1/theatre/works did not return application/json".into()
                )
            );
        }
        for body in [
            "not JSON",
            "[]",
            r#"{"items":[]}"#,
            r#"{"items":null,"next_cursor":null}"#,
            r#"{"items":[],"next_cursor":42}"#,
            r#"{"items":[],"next_cursor":null} trailing"#,
        ] {
            let (addr, _) = serve_responses(vec![json_response(body)]);
            assert_eq!(
                probe(addr, &Readiness::TheatreCatalog),
                Probe::Unexpected(
                    "Theatre catalog /api/v1/theatre/works returned an invalid WorkPage envelope"
                        .into()
                )
            );
        }
    }

    #[test]
    fn catalogue_decodes_chunked_json_with_extensions_and_trailers() {
        let body = r#"{"items":[],"next_cursor":null}"#;
        let response = format!("HTTP/1.1 200 OK\r\nContent-Type: Application/JSON\r\nTransfer-Encoding: chunked\r\n\r\n{:x};local=yes\r\n{}\r\n{:x}\r\n{}\r\n0\r\nX-Local: done\r\n\r\n", 11, &body[..11], body.len() - 11, &body[11..]);
        assert_eq!(
            probe(serve_once(&response), &Readiness::TheatreCatalog),
            Probe::Ready
        );
    }

    #[test]
    fn http_framing_refuses_truncated_ambiguous_and_oversize_responses() {
        for response in [
            "HTTP/1.1 200 OK\r\nContent-Length: 50\r\n\r\nshort".to_owned(),
            "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nshort\r\n".to_owned(),
            "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nshortxx0\r\n\r\n".to_owned(),
            "HTTP/1.1 200 OK\r\nContent-Length: 1\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n"
                .to_owned(),
            "HTTP/1.1 200 OK\r\nContent-Length: 1\r\nContent-Length: 1\r\n\r\nx".to_owned(),
            format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n",
                MAX_RESPONSE + 1
            ),
            format!(
                "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\n",
                MAX_RESPONSE + 1
            ),
            format!(
                "HTTP/1.1 200 OK\r\nX-Large: {}\r\n\r\n",
                "x".repeat(MAX_HEAD)
            ),
            format!("HTTP/1.1 200 OK\r\n\r\n{}", "x".repeat(MAX_RESPONSE)),
        ] {
            assert_eq!(
                probe(serve_once(&response), &Readiness::TheatreCatalog),
                Probe::Unavailable
            );
        }
    }

    #[test]
    fn content_length_finishes_without_waiting_for_connection_close() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let (done, wait) = mpsc::channel();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\nbody")
                .unwrap();
            wait.recv_timeout(Duration::from_secs(2)).unwrap();
        });
        assert_eq!(http_get(addr, "/").unwrap(), (200, "body".into()));
        done.send(()).unwrap();
        server.join().unwrap();
    }

    #[test]
    fn theatre_web_checks_html_then_same_origin_proxy_and_sanitizes_failure() {
        let (addr, requests) = serve_responses(vec![
            b"HTTP/1.1 200 OK\r\n\r\n<title>Cantos</title>".to_vec(),
            json_response(r#"{"items":[],"next_cursor":null}"#),
        ]);
        assert_eq!(probe(addr, &Readiness::TheatreWeb), Probe::Ready);
        assert!(requests.recv().unwrap().starts_with("GET / HTTP/1.1\r\n"));
        assert!(requests
            .recv()
            .unwrap()
            .starts_with("GET /api/v1/theatre/works?limit=20 HTTP/1.1\r\n"));

        let (addr, _) = serve_responses(vec![
            b"HTTP/1.1 200 OK\r\n\r\n<title>Cantos</title>".to_vec(),
            b"HTTP/1.1 503 Unavailable\r\nX-Secret: credential\r\n\r\nprivate-body".to_vec(),
        ]);
        assert_eq!(
            probe(addr, &Readiness::TheatreWeb),
            Probe::Unexpected("Theatre catalog /api/v1/theatre/works returned HTTP 503".into())
        );
        assert_eq!(
            probe(
                serve_once("HTTP/1.1 200 OK\r\n\r\n<title>Cantos Studio</title>"),
                &Readiness::TheatreWeb
            ),
            Probe::Unexpected("Theatre Web HTML: HTTP 200 without `<title>Cantos</title>`".into())
        );
    }

    #[test]
    fn response_deadline_does_not_restart_per_read() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let _server = thread::spawn(move || {
            let _ = listener.accept();
        });
        let stream = TcpStream::connect(addr).unwrap();
        let mut reader = BoundedResponse {
            stream,
            deadline: Instant::now() - Duration::from_millis(1),
            received: 0,
        };
        assert_eq!(
            reader.read(&mut [0; 1]).unwrap_err().kind(),
            io::ErrorKind::TimedOut
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_unix_endpoint_is_ready_only_while_something_accepts_on_the_socket() {
        let dir = std::env::temp_dir().join(format!("xtask-probe-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("s.sock");
        let endpoint = Endpoint::Unix(path.clone());
        assert_eq!(
            probe_endpoint(&endpoint, &Readiness::Tcp),
            Probe::Unavailable
        );
        let listener = std::os::unix::net::UnixListener::bind(&path).unwrap();
        assert_eq!(probe_endpoint(&endpoint, &Readiness::Tcp), Probe::Ready);
        assert!(matches!(
            probe_endpoint(&endpoint, &Readiness::TheatreCatalog),
            Probe::Unexpected(_)
        ));
        // Drain the probe connection before closing: on macOS an unaccepted backlog can
        // keep the socket connectable briefly after the listening handle is dropped.
        drop(listener.accept().unwrap());
        drop(listener);
        // The inode survives the listener, but nothing answers on it any more.
        assert!(path.exists());
        // Darwin can publish close asynchronously even after the accepted connection drained.
        // Observe the real endpoint with a deadline; keep the final refusal assertion.
        let deadline = std::time::Instant::now() + Duration::from_secs(1);
        while probe_endpoint(&endpoint, &Readiness::Tcp) == Probe::Ready
            && std::time::Instant::now() < deadline
        {
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(
            probe_endpoint(&endpoint, &Readiness::Tcp),
            Probe::Unavailable
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn port_ownership_reflects_an_existing_listener() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        assert!(!port_is_free(addr));
        drop(listener);
        // Closing a socket does not promise that the kernel releases its bind synchronously.
        let deadline = std::time::Instant::now() + Duration::from_secs(1);
        while !port_is_free(addr) && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(port_is_free(addr));
        let accepting = serve_once("HTTP/1.1 204 No Content\r\n\r\n");
        assert_eq!(probe(accepting, &Readiness::Tcp), Probe::Ready);
    }
}
