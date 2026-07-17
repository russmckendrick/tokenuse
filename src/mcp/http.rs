//! Streamable HTTP transport for the MCP server: `POST /mcp` on a loopback
//! listener, fronting the same [`super::McpServer`] dispatcher as stdio.
//! Hand-rolled over `std::net` — still no async runtime, no new
//! dependencies; the only network surface is an opt-in 127.0.0.1 socket.
//!
//! Stateless per the 2025-06-18 revision: no `Mcp-Session-Id` is issued and
//! no server-initiated stream is offered (`GET /mcp` is 405). Every request
//! must carry `Authorization: Bearer <token>`; the token persists in
//! `ConfigPaths::mcp_token_file` (0600 on Unix; Windows relies on the
//! per-user profile ACL) and deleting the file rotates it. Host and Origin
//! headers are pinned to localhost so browsers and DNS-rebinding pages
//! cannot reach the endpoint even from the same machine.

use std::fs;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::Duration;

use color_eyre::Result;

use crate::config::{ConfigPaths, UserConfig};
use crate::copy::{copy, template};

use super::{generate_salt, load_or_create_salt, McpServer};

pub const MCP_HTTP_PATH: &str = "/mcp";
const MAX_BODY_BYTES: usize = 1 << 20;
const MAX_REQUEST_LINE_BYTES: u64 = 8 * 1024;
const MAX_HEADER_BYTES: u64 = 16 * 1024;
const READ_TIMEOUT: Duration = Duration::from_secs(10);

pub struct HttpOptions {
    pub port: u16,
    pub real_names: bool,
}

/// Owns the accept-loop thread. Dropping the handle stops the listener:
/// the shutdown flag is raised and a throwaway self-connect unblocks the
/// blocking `accept()` so the thread can observe it and exit.
pub struct HttpServerHandle {
    addr: SocketAddr,
    shutdown: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl HttpServerHandle {
    pub fn port(&self) -> u16 {
        self.addr.port()
    }

    pub fn endpoint(&self) -> String {
        endpoint_url(self.addr.port())
    }

    /// Stop the listener and wait for the accept loop to exit.
    pub fn shutdown(self) {
        drop(self);
    }

    /// Block until the accept loop ends (foreground CLI mode; the loop only
    /// ends when the process is killed or the handle's flag is raised).
    pub fn join(mut self) {
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for HttpServerHandle {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(self.addr);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub fn endpoint_url(port: u16) -> String {
    format!("http://127.0.0.1:{port}{MCP_HTTP_PATH}")
}

/// Bind 127.0.0.1:`port` (0 picks a free port, reported by `handle.port()`)
/// and serve MCP over HTTP until the handle is dropped. All fallible setup —
/// salt, token, bind — happens synchronously so callers see errors here, not
/// from the background thread.
pub fn serve_http(paths: &ConfigPaths, options: &HttpOptions) -> Result<HttpServerHandle> {
    let salt = load_or_create_salt(paths)?;
    let token = load_or_create_token(paths)?;
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, options.port))?;
    let addr = listener.local_addr()?;

    let server = Arc::new(Mutex::new(McpServer::new(options.real_names, salt)));
    let token = Arc::new(token);
    let shutdown = Arc::new(AtomicBool::new(false));
    let accept_shutdown = Arc::clone(&shutdown);
    let thread = std::thread::spawn(move || {
        for stream in listener.incoming() {
            if accept_shutdown.load(Ordering::SeqCst) {
                break;
            }
            let Ok(stream) = stream else { continue };
            let server = Arc::clone(&server);
            let token = Arc::clone(&token);
            std::thread::spawn(move || handle_connection(&stream, &server, &token));
        }
    });

    Ok(HttpServerHandle {
        addr,
        shutdown,
        thread: Some(thread),
    })
}

/// CLI entry for `tokenuse mcp --http [--port N]`.
pub fn run_foreground(real_names: bool, port_override: Option<u16>) -> Result<()> {
    let paths = ConfigPaths::default();
    let config = UserConfig::load(&paths).unwrap_or_default();
    let port = port_override.unwrap_or(config.mcp.http_port);
    let handle = serve_http(&paths, &HttpOptions { port, real_names })?;
    println!(
        "{}",
        template(
            &copy().cli.mcp_http_listening,
            &[("endpoint", handle.endpoint())]
        )
    );
    handle.join();
    Ok(())
}

/// Read the persisted bearer token, or mint and persist one. Reuses the
/// salt generator: `RandomState` OS entropy is not a CSPRNG, but 64 hex
/// chars is a 256-bit search space and the only attack surface is loopback;
/// swap in a real CSPRNG if this endpoint ever leaves localhost.
pub fn load_or_create_token(paths: &ConfigPaths) -> Result<String> {
    if let Ok(existing) = fs::read_to_string(&paths.mcp_token_file) {
        let trimmed = existing.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
    }
    paths.ensure_dir()?;
    let token = generate_salt();
    write_token_file(&paths.mcp_token_file, &token)?;
    Ok(token)
}

#[cfg(unix)]
fn write_token_file(path: &Path, token: &str) -> Result<()> {
    use std::os::unix::fs::OpenOptionsExt;

    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(token.as_bytes())?;
    Ok(())
}

#[cfg(not(unix))]
fn write_token_file(path: &Path, token: &str) -> Result<()> {
    // Windows: %APPDATA% is already restricted to the user by the profile ACL.
    fs::write(path, token)?;
    Ok(())
}

struct Request {
    method: String,
    path: String,
    host: Option<String>,
    origin: Option<String>,
    authorization: Option<String>,
    content_length: Option<usize>,
    chunked: bool,
}

fn handle_connection(stream: &TcpStream, server: &Mutex<McpServer>, token: &str) {
    let _ = stream.set_read_timeout(Some(READ_TIMEOUT));
    let mut reader = BufReader::new(stream);

    let request = match read_request_head(&mut reader) {
        Ok(Some(request)) => request,
        // Silent EOF: e.g. the shutdown wake-up connect, or a port probe.
        Ok(None) => return,
        Err(_) => return respond(stream, "400 Bad Request", &[], &[]),
    };

    // Security gates before routing: a request that fails them learns
    // nothing about what is served here.
    if !request.host.as_deref().is_some_and(is_local_host) {
        return respond(stream, "403 Forbidden", &[], &[]);
    }
    if let Some(origin) = &request.origin {
        if !is_local_origin(origin) {
            return respond(stream, "403 Forbidden", &[], &[]);
        }
    }
    if request.path != MCP_HTTP_PATH {
        return respond(stream, "404 Not Found", &[], &[]);
    }
    if request.method != "POST" {
        return respond(stream, "405 Method Not Allowed", &[("Allow", "POST")], &[]);
    }
    if !request
        .authorization
        .as_deref()
        .and_then(|value| value.strip_prefix("Bearer "))
        .is_some_and(|provided| token_matches(provided.trim(), token))
    {
        return respond(
            stream,
            "401 Unauthorized",
            &[("WWW-Authenticate", "Bearer")],
            &[],
        );
    }
    let length = match (request.chunked, request.content_length) {
        (true, _) | (false, None) => {
            return respond(stream, "411 Length Required", &[], &[]);
        }
        (false, Some(length)) => length,
    };
    if length > MAX_BODY_BYTES {
        return respond(stream, "413 Content Too Large", &[], &[]);
    }

    let mut body = vec![0u8; length];
    if reader.read_exact(&mut body).is_err() {
        return respond(stream, "400 Bad Request", &[], &[]);
    }
    let Ok(body) = String::from_utf8(body) else {
        return respond(stream, "400 Bad Request", &[], &[]);
    };
    // The 2025-06-18 revision removed JSON-RPC batching.
    if body.trim_start().starts_with('[') {
        return respond(stream, "400 Bad Request", &[], &[]);
    }

    let response = server
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .handle_line(&body);
    match response {
        Some(payload) => {
            let payload = payload.to_string();
            respond(
                stream,
                "200 OK",
                &[("Content-Type", "application/json")],
                payload.as_bytes(),
            );
        }
        None => respond(stream, "202 Accepted", &[], &[]),
    }
}

/// `Ok(None)` = clean EOF before any bytes (not an HTTP request at all);
/// `Err` = malformed or over-limit head.
fn read_request_head(reader: &mut BufReader<&TcpStream>) -> io::Result<Option<Request>> {
    let request_line = match read_line_capped(reader, MAX_REQUEST_LINE_BYTES)? {
        Some(line) => line,
        None => return Ok(None),
    };
    let mut parts = request_line.split_whitespace();
    let (Some(method), Some(target)) = (parts.next(), parts.next()) else {
        return Err(io::ErrorKind::InvalidData.into());
    };
    let path = target.split('?').next().unwrap_or("").to_string();

    let mut request = Request {
        method: method.to_string(),
        path,
        host: None,
        origin: None,
        authorization: None,
        content_length: None,
        chunked: false,
    };

    let mut header_budget = MAX_HEADER_BYTES;
    loop {
        let line = read_line_capped(reader, header_budget)?
            .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidData))?;
        if line.is_empty() {
            return Ok(Some(request));
        }
        header_budget = header_budget.saturating_sub(line.len() as u64 + 2);
        if header_budget == 0 {
            return Err(io::ErrorKind::InvalidData.into());
        }
        let Some((name, value)) = line.split_once(':') else {
            return Err(io::ErrorKind::InvalidData.into());
        };
        let value = value.trim();
        match name.trim().to_ascii_lowercase().as_str() {
            "host" => request.host = Some(value.to_string()),
            "origin" => request.origin = Some(value.to_string()),
            "authorization" => request.authorization = Some(value.to_string()),
            "content-length" => {
                request.content_length = Some(
                    value
                        .parse::<usize>()
                        .map_err(|_| io::Error::from(io::ErrorKind::InvalidData))?,
                );
            }
            "transfer-encoding" => request.chunked = true,
            _ => {}
        }
    }
}

/// One CRLF/LF-terminated line, at most `cap` bytes. `Ok(None)` = EOF before
/// any bytes; a line truncated by EOF or the cap is `Err`.
fn read_line_capped(reader: &mut BufReader<&TcpStream>, cap: u64) -> io::Result<Option<String>> {
    let mut buf = Vec::new();
    let read = reader.by_ref().take(cap).read_until(b'\n', &mut buf)?;
    if read == 0 {
        return Ok(None);
    }
    if buf.pop() != Some(b'\n') {
        return Err(io::ErrorKind::InvalidData.into());
    }
    if buf.last() == Some(&b'\r') {
        buf.pop();
    }
    String::from_utf8(buf)
        .map(Some)
        .map_err(|_| io::ErrorKind::InvalidData.into())
}

fn respond(stream: &TcpStream, status: &str, extra_headers: &[(&str, &str)], body: &[u8]) {
    let mut head = format!("HTTP/1.1 {status}\r\n");
    for (name, value) in extra_headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str(&format!(
        "Content-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    ));
    let mut out = stream;
    let _ = out.write_all(head.as_bytes());
    let _ = out.write_all(body);
    let _ = out.flush();
}

/// Accepts `localhost`, `127.0.0.1`, and `[::1]`, with or without a port.
fn is_local_host(value: &str) -> bool {
    let host = value.trim().to_ascii_lowercase();
    if let Some(rest) = host.strip_prefix('[') {
        return rest
            .split_once(']')
            .is_some_and(|(inner, suffix)| inner == "::1" && suffix_is_port(suffix));
    }
    let (bare, port) = match host.rsplit_once(':') {
        Some((bare, port)) => (bare, Some(port)),
        None => (host.as_str(), None),
    };
    (bare == "localhost" || bare == "127.0.0.1")
        && port.is_none_or(|port| !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()))
}

fn suffix_is_port(suffix: &str) -> bool {
    match suffix.strip_prefix(':') {
        None => suffix.is_empty(),
        Some(port) => !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()),
    }
}

/// A browser Origin is only tolerated when it points at this machine;
/// non-browser MCP clients simply omit the header.
fn is_local_origin(origin: &str) -> bool {
    let trimmed = origin.trim();
    trimmed
        .strip_prefix("http://")
        .or_else(|| trimmed.strip_prefix("https://"))
        .is_some_and(is_local_host)
}

/// Length check first, then a fold over every byte so equal-length
/// comparisons take the same time regardless of where they diverge.
fn token_matches(provided: &str, expected: &str) -> bool {
    let (provided, expected) = (provided.as_bytes(), expected.as_bytes());
    if provided.len() != expected.len() {
        return false;
    }
    provided
        .iter()
        .zip(expected)
        .fold(0u8, |acc, (a, b)| acc | (a ^ b))
        == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_paths(name: &str) -> ConfigPaths {
        let dir = std::env::temp_dir().join(format!(
            "tokenuse-mcp-http-{}-{}",
            crate::tools::paths::test_run_id(),
            name
        ));
        ConfigPaths::new(dir)
    }

    fn serve(paths: &ConfigPaths) -> HttpServerHandle {
        serve_http(
            paths,
            &HttpOptions {
                port: 0,
                real_names: false,
            },
        )
        .expect("bind loopback")
    }

    fn roundtrip(port: u16, raw: &str) -> String {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
        stream.write_all(raw.as_bytes()).expect("send request");
        let mut response = String::new();
        stream.read_to_string(&mut response).expect("read response");
        response
    }

    fn post(port: u16, headers: &[(&str, &str)], body: &str) -> String {
        let mut raw = format!("POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n");
        for (name, value) in headers {
            raw.push_str(&format!("{name}: {value}\r\n"));
        }
        raw.push_str(&format!("Content-Length: {}\r\n\r\n{body}", body.len()));
        roundtrip(port, &raw)
    }

    fn authed_post(port: u16, token: &str, body: &str) -> String {
        post(port, &[("Authorization", &format!("Bearer {token}"))], body)
    }

    fn body_json(response: &str) -> serde_json::Value {
        let body = response.split_once("\r\n\r\n").expect("head/body split").1;
        serde_json::from_str(body).expect("JSON body")
    }

    #[test]
    fn initialize_ping_and_tools_list_respond_over_http() {
        let paths = temp_paths("requests");
        let handle = serve(&paths);
        let token = load_or_create_token(&paths).expect("token");
        let port = handle.port();

        let init = authed_post(
            port,
            &token,
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}"#,
        );
        assert!(init.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(init.contains("Content-Type: application/json"));
        assert_eq!(body_json(&init)["result"]["protocolVersion"], "2025-06-18");

        let ping = authed_post(port, &token, r#"{"jsonrpc":"2.0","id":2,"method":"ping"}"#);
        assert_eq!(body_json(&ping)["result"], serde_json::json!({}));

        let tools = authed_post(
            port,
            &token,
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/list"}"#,
        );
        assert_eq!(
            body_json(&tools)["result"]["tools"]
                .as_array()
                .map(Vec::len),
            Some(4)
        );

        handle.shutdown();
        std::fs::remove_dir_all(&paths.dir).ok();
    }

    #[test]
    fn notifications_return_202_with_an_empty_body() {
        let paths = temp_paths("notify");
        let handle = serve(&paths);
        let token = load_or_create_token(&paths).expect("token");

        let response = authed_post(
            handle.port(),
            &token,
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        );
        assert!(response.starts_with("HTTP/1.1 202 Accepted\r\n"));
        assert!(response.ends_with("\r\n\r\n"));

        handle.shutdown();
        std::fs::remove_dir_all(&paths.dir).ok();
    }

    #[test]
    fn missing_or_wrong_bearer_token_is_unauthorized() {
        let paths = temp_paths("auth");
        let handle = serve(&paths);
        let port = handle.port();

        let missing = post(port, &[], "{}");
        assert!(missing.starts_with("HTTP/1.1 401 Unauthorized\r\n"));
        assert!(missing.contains("WWW-Authenticate: Bearer"));

        let wrong = post(port, &[("Authorization", "Bearer nope")], "{}");
        assert!(wrong.starts_with("HTTP/1.1 401 Unauthorized\r\n"));

        handle.shutdown();
        std::fs::remove_dir_all(&paths.dir).ok();
    }

    #[test]
    fn foreign_host_and_origin_headers_are_forbidden() {
        let paths = temp_paths("rebind");
        let handle = serve(&paths);
        let token = load_or_create_token(&paths).expect("token");
        let port = handle.port();

        let rebound = roundtrip(
            port,
            &format!(
                "POST /mcp HTTP/1.1\r\nHost: evil.example\r\nAuthorization: Bearer {token}\r\nContent-Length: 2\r\n\r\n{{}}"
            ),
        );
        assert!(rebound.starts_with("HTTP/1.1 403 Forbidden\r\n"));

        let cross_origin = post(
            port,
            &[
                ("Authorization", &format!("Bearer {token}")),
                ("Origin", "https://evil.example"),
            ],
            "{}",
        );
        assert!(cross_origin.starts_with("HTTP/1.1 403 Forbidden\r\n"));

        let local_origin = post(
            port,
            &[
                ("Authorization", &format!("Bearer {token}")),
                ("Origin", "http://localhost:5173"),
            ],
            r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#,
        );
        assert!(local_origin.starts_with("HTTP/1.1 200 OK\r\n"));

        handle.shutdown();
        std::fs::remove_dir_all(&paths.dir).ok();
    }

    #[test]
    fn wrong_method_and_path_are_rejected() {
        let paths = temp_paths("routing");
        let handle = serve(&paths);
        let token = load_or_create_token(&paths).expect("token");
        let port = handle.port();

        let get = roundtrip(
            port,
            &format!(
                "GET /mcp HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {token}\r\n\r\n"
            ),
        );
        assert!(get.starts_with("HTTP/1.1 405 Method Not Allowed\r\n"));
        assert!(get.contains("Allow: POST"));

        let elsewhere = roundtrip(
            port,
            &format!(
                "POST /other HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {token}\r\nContent-Length: 2\r\n\r\n{{}}"
            ),
        );
        assert!(elsewhere.starts_with("HTTP/1.1 404 Not Found\r\n"));

        handle.shutdown();
        std::fs::remove_dir_all(&paths.dir).ok();
    }

    #[test]
    fn body_framing_violations_map_to_http_errors() {
        let paths = temp_paths("framing");
        let handle = serve(&paths);
        let token = load_or_create_token(&paths).expect("token");
        let port = handle.port();
        let auth = format!("Authorization: Bearer {token}");

        let chunked = roundtrip(
            port,
            &format!(
                "POST /mcp HTTP/1.1\r\nHost: localhost\r\n{auth}\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n"
            ),
        );
        assert!(chunked.starts_with("HTTP/1.1 411 Length Required\r\n"));

        let unsized_body = roundtrip(
            port,
            &format!("POST /mcp HTTP/1.1\r\nHost: localhost\r\n{auth}\r\n\r\n"),
        );
        assert!(unsized_body.starts_with("HTTP/1.1 411 Length Required\r\n"));

        let oversized = roundtrip(
            port,
            &format!(
                "POST /mcp HTTP/1.1\r\nHost: localhost\r\n{auth}\r\nContent-Length: {}\r\n\r\n",
                MAX_BODY_BYTES + 1
            ),
        );
        assert!(oversized.starts_with("HTTP/1.1 413 Content Too Large\r\n"));

        let batch = authed_post(
            port,
            &token,
            r#"[{"jsonrpc":"2.0","id":1,"method":"ping"}]"#,
        );
        assert!(batch.starts_with("HTTP/1.1 400 Bad Request\r\n"));

        handle.shutdown();
        std::fs::remove_dir_all(&paths.dir).ok();
    }

    #[test]
    fn shutdown_stops_the_listener_and_frees_the_port() {
        let paths = temp_paths("shutdown");
        let handle = serve(&paths);
        let port = handle.port();
        handle.shutdown();

        assert!(
            TcpListener::bind(("127.0.0.1", port)).is_ok(),
            "port should be rebindable after shutdown"
        );
        std::fs::remove_dir_all(&paths.dir).ok();
    }

    #[test]
    fn token_round_trips_and_is_owner_readable_only() {
        let paths = temp_paths("token");
        let first = load_or_create_token(&paths).expect("create token");
        let second = load_or_create_token(&paths).expect("reload token");
        assert_eq!(first, second);
        assert_eq!(first.len(), 64);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&paths.mcp_token_file)
                .expect("token metadata")
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }

        std::fs::remove_dir_all(&paths.dir).ok();
    }

    #[test]
    fn host_and_origin_allowlists_cover_port_and_bracket_forms() {
        assert!(is_local_host("localhost"));
        assert!(is_local_host("127.0.0.1:20151"));
        assert!(is_local_host("[::1]:20151"));
        assert!(!is_local_host("evil.example"));
        assert!(!is_local_host("127.0.0.1.evil.example"));
        assert!(!is_local_host("localhost:port"));
        assert!(is_local_origin("http://localhost:5173"));
        assert!(!is_local_origin("null"));
        assert!(!is_local_origin("file://localhost"));
    }
}
