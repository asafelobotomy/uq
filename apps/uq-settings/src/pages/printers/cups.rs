//! Talking to CUPS: IPP over HTTP on CUPS's local socket, as the person running Settings.
//!
//! CUPS knows who is asking from the socket itself (`PeerCred`): no password goes anywhere.
//! Changing printers is allowed to CUPS's administrators (on Arch, the `wheel` group, which is
//! what uQ's "Administrator" account type means); anyone else gets "forbidden", which the page
//! shows as such. Every call has a time limit, and nothing waits on a terminal: the command-line
//! tools (`lpadmin`, `lpoptions`) can ask for a password on one, or wait on a paused printer,
//! which is why the page doesn't use them.

use std::path::PathBuf;
use std::time::Duration;

use futures_util::FutureExt;
use futures_util::future::BoxFuture;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::ipp::{self, Request, Response};

/// Where CUPS listens on Arch (and most systems); `CUPS_SERVER` may name another socket.
pub const SOCKET: &str = "/run/cups/cups.sock";
/// How long a read may take.
pub const READ_TIMEOUT: Duration = Duration::from_secs(5);
/// How long a change may take: adding a driverless printer asks the printer itself.
pub const CHANGE_TIMEOUT: Duration = Duration::from_secs(90);
/// The most a response may be (a printer's driver file is the largest thing read).
const MAX_RESPONSE: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CupsError {
    /// CUPS isn't installed.
    Missing,
    /// CUPS is installed but not running and not set to start.
    Off,
    /// CUPS said no: the person isn't one of its administrators.
    NotAllowed,
    TimedOut,
    /// Anything else, in CUPS's own words where it gave some.
    Failed(String),
}

impl CupsError {
    pub fn text(&self) -> String {
        match self {
            CupsError::Missing => "Printing isn't installed. Install the cups package.".into(),
            CupsError::Off => "Turn it on to see your printers and add new ones.".into(),
            CupsError::NotAllowed => {
                "Only an administrator can change printers, and your account isn't one.".into()
            }
            CupsError::TimedOut => "The printing service didn't answer in time.".into(),
            CupsError::Failed(t) => t.clone(),
        }
    }
}

/// How the page reaches CUPS. A trait so tests can stand in a fake CUPS.
pub trait Transport: Send + Sync + 'static {
    /// POST an IPP request to `path` (`/` or `/admin/`) and return the IPP answer.
    fn ipp(
        &self,
        path: &'static str,
        body: Vec<u8>,
        timeout: Duration,
    ) -> BoxFuture<'static, Result<Vec<u8>, CupsError>>;
    /// GET a file CUPS serves (`/printers/<name>.ppd`).
    fn get(&self, path: String) -> BoxFuture<'static, Result<Vec<u8>, CupsError>>;
}

/// The real CUPS, on its socket.
pub struct Socket {
    pub path: PathBuf,
    /// The login name CUPS checks against the socket's peer (`PeerCred`).
    pub user: String,
    /// Whether CUPS is installed: tells "not installed" from "switched off".
    pub installed: bool,
}

impl Socket {
    pub fn system(user: String) -> Self {
        let path = std::env::var("CUPS_SERVER")
            .ok()
            .filter(|s| s.starts_with('/'))
            .map_or_else(|| PathBuf::from(SOCKET), PathBuf::from);
        let installed = ["/usr/bin/cupsd", "/usr/sbin/cupsd"]
            .iter()
            .any(|p| std::path::Path::new(p).exists());
        Self {
            path,
            user,
            installed,
        }
    }

    fn call(
        &self,
        head: String,
        body: Vec<u8>,
        timeout: Duration,
    ) -> BoxFuture<'static, Result<(u16, Vec<u8>), CupsError>> {
        let path = self.path.clone();
        let installed = self.installed;
        async move {
            let work = async {
                let mut s = match tokio::net::UnixStream::connect(&path).await {
                    Ok(s) => s,
                    Err(e) => {
                        return Err(match e.kind() {
                            std::io::ErrorKind::NotFound
                            | std::io::ErrorKind::ConnectionRefused
                                if !installed =>
                            {
                                CupsError::Missing
                            }
                            std::io::ErrorKind::NotFound
                            | std::io::ErrorKind::ConnectionRefused => CupsError::Off,
                            _ => CupsError::Failed(format!(
                                "The printing service couldn't be reached: {e}"
                            )),
                        });
                    }
                };
                let io = |e: std::io::Error| {
                    CupsError::Failed(format!("The printing service stopped answering: {e}"))
                };
                s.write_all(head.as_bytes()).await.map_err(io)?;
                s.write_all(&body).await.map_err(io)?;
                let mut raw = Vec::new();
                (&mut s)
                    .take(MAX_RESPONSE as u64)
                    .read_to_end(&mut raw)
                    .await
                    .map_err(io)?;
                parse_http(&raw).map_err(CupsError::Failed)
            };
            tokio::time::timeout(timeout, work)
                .await
                .unwrap_or(Err(CupsError::TimedOut))
        }
        .boxed()
    }

    fn auth(&self) -> String {
        if self.user.is_empty() {
            String::new()
        } else {
            format!("Authorization: PeerCred {}\r\n", self.user)
        }
    }
}

impl Transport for Socket {
    fn ipp(
        &self,
        path: &'static str,
        body: Vec<u8>,
        timeout: Duration,
    ) -> BoxFuture<'static, Result<Vec<u8>, CupsError>> {
        let head = format!(
            "POST {path} HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/ipp\r\n\
             Content-Length: {}\r\nConnection: close\r\n{}\r\n",
            body.len(),
            self.auth()
        );
        let call = self.call(head, body, timeout);
        async move {
            let (status, body) = call.await?;
            match status {
                200 => Ok(body),
                401 | 403 => Err(CupsError::NotAllowed),
                other => Err(CupsError::Failed(format!(
                    "The printing service answered with an error ({other})."
                ))),
            }
        }
        .boxed()
    }

    fn get(&self, path: String) -> BoxFuture<'static, Result<Vec<u8>, CupsError>> {
        let head = format!(
            "GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n{}\r\n",
            self.auth()
        );
        let call = self.call(head, Vec::new(), READ_TIMEOUT);
        async move {
            match call.await? {
                (200, body) => Ok(body),
                (401 | 403, _) => Err(CupsError::NotAllowed),
                (404, _) => Err(CupsError::Failed(
                    "This printer has no driver file to read options from.".into(),
                )),
                (other, _) => Err(CupsError::Failed(format!(
                    "The printing service answered with an error ({other})."
                ))),
            }
        }
        .boxed()
    }
}

/// The status and body of one HTTP/1.1 response read to the end of the connection: by
/// `Content-Length`, chunked, or to the end. A leading `100 Continue` is skipped.
pub fn parse_http(raw: &[u8]) -> Result<(u16, Vec<u8>), String> {
    let mut rest = raw;
    loop {
        let end = rest
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .ok_or("the printing service sent a cut-off answer")?;
        let head = String::from_utf8_lossy(&rest[..end]).into_owned();
        let body = &rest[end + 4..];
        let mut lines = head.split("\r\n");
        let status: u16 = lines
            .next()
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|s| s.parse().ok())
            .ok_or("the printing service sent an unreadable answer")?;
        if status == 100 {
            rest = body;
            continue;
        }
        let header = |name: &str| {
            head.split("\r\n").skip(1).find_map(|l| {
                let (k, v) = l.split_once(':')?;
                k.trim()
                    .eq_ignore_ascii_case(name)
                    .then(|| v.trim().to_string())
            })
        };
        if header("Transfer-Encoding").is_some_and(|v| v.eq_ignore_ascii_case("chunked")) {
            return dechunk(body).map(|b| (status, b));
        }
        if let Some(len) = header("Content-Length").and_then(|v| v.parse::<usize>().ok()) {
            if body.len() < len {
                return Err("the printing service sent a cut-off answer".into());
            }
            return Ok((status, body[..len].to_vec()));
        }
        return Ok((status, body.to_vec()));
    }
}

fn dechunk(mut b: &[u8]) -> Result<Vec<u8>, String> {
    let cut = || "the printing service sent a cut-off answer".to_string();
    let mut out = Vec::new();
    loop {
        let eol = b.windows(2).position(|w| w == b"\r\n").ok_or_else(cut)?;
        let size_text = String::from_utf8_lossy(&b[..eol]);
        let size_text = size_text.split(';').next().unwrap_or("").trim();
        let size = usize::from_str_radix(size_text, 16).map_err(|_| cut())?;
        b = &b[eol + 2..];
        if size == 0 {
            return Ok(out);
        }
        if b.len() < size + 2 {
            return Err(cut());
        }
        out.extend_from_slice(&b[..size]);
        b = &b[size + 2..];
    }
}

/// The page's CUPS: typed calls over a [`Transport`].
pub struct Cups {
    pub t: std::sync::Arc<dyn Transport>,
    pub user: String,
}

/// IPP's "forbidden", "not authenticated" and "not authorized".
fn refused(status: u16) -> bool {
    matches!(status, 0x0401..=0x0403)
}

impl Cups {
    /// The URI CUPS knows a printer by.
    pub fn printer_uri(name: &str) -> String {
        format!("ipp://localhost/printers/{}", uri_escape(name))
    }

    /// A request with `requesting-user-name` set, as every CUPS request should have.
    pub fn request(&self, op: u16) -> Request {
        let mut r = Request::new(op);
        if !self.user.is_empty() {
            r.op_attr(
                "requesting-user-name",
                ipp::Out::Str(ipp::tag::NAME, self.user.clone()),
            );
        }
        r
    }

    /// Send `r` to `path`; a refusal or an error status is an `Err`. `not_found` is what a
    /// "not found" status means for this call (`None`: it's not an error, e.g. no default
    /// printer).
    pub async fn send(
        &self,
        path: &'static str,
        r: &Request,
        timeout: Duration,
    ) -> Result<Response, CupsError> {
        let raw = self.t.ipp(path, r.encode(), timeout).await?;
        let resp = ipp::decode(&raw).map_err(CupsError::Failed)?;
        if ipp::is_success(resp.status) {
            return Ok(resp);
        }
        if refused(resp.status) {
            return Err(CupsError::NotAllowed);
        }
        Err(CupsError::Failed(match resp.message() {
            Some(m) if !m.is_empty() => sentence(m),
            _ => format!(
                "The printing service refused that (error {:#06x}).",
                resp.status
            ),
        }))
    }
}

/// CUPS's messages start lower-case and end without a full stop; make one a sentence.
pub fn sentence(m: &str) -> String {
    let m = m.trim();
    let mut c = m.chars();
    let mut s = match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect::<String>(),
        None => return String::new(),
    };
    if !s.ends_with(['.', '!', '?']) {
        s.push('.');
    }
    s
}

/// Escape a printer name for a URI path (CUPS names may hold almost anything but `/`, `#`,
/// spaces and quotes; escape everything outside the unreserved set to be safe).
pub fn uri_escape(name: &str) -> String {
    let mut out = String::new();
    for b in name.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_bodies() {
        let plain = b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\nabcEXTRA";
        assert_eq!(parse_http(plain).unwrap(), (200, b"abc".to_vec()));
        let chunked = b"HTTP/1.1 100 Continue\r\n\r\nHTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\n3\r\nabc\r\n2;x=y\r\nde\r\n0\r\n\r\n";
        assert_eq!(parse_http(chunked).unwrap(), (200, b"abcde".to_vec()));
        let to_end = b"HTTP/1.0 403 Forbidden\r\n\r\nno";
        assert_eq!(parse_http(to_end).unwrap(), (403, b"no".to_vec()));
        assert!(parse_http(b"HTTP/1.1 200 OK\r\nContent-Length: 9\r\n\r\nabc").is_err());
        assert!(
            parse_http(b"HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\n9\r\nab").is_err()
        );
        assert!(parse_http(b"garbage").is_err());
    }

    #[test]
    fn escapes_names_and_makes_sentences() {
        assert_eq!(uri_escape("Hall_Laser-2"), "Hall_Laser-2");
        assert_eq!(uri_escape("a b/é"), "a%20b%2F%C3%A9");
        assert_eq!(
            sentence("printer \"x\" does not exist"),
            "Printer \"x\" does not exist."
        );
        assert_eq!(sentence("Done."), "Done.");
        assert_eq!(sentence("  "), "");
    }

    #[tokio::test]
    async fn socket_errors_say_what_is_wrong() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = Socket {
            path: dir.path().join("cups.sock"),
            user: "me".into(),
            installed: false,
        };
        let r = Request::new(ipp::op::CUPS_GET_DEFAULT).encode();
        assert_eq!(
            s.ipp("/", r.clone(), READ_TIMEOUT).await,
            Err(CupsError::Missing)
        );
        s.installed = true;
        assert_eq!(s.ipp("/", r, READ_TIMEOUT).await, Err(CupsError::Off));
    }

    #[tokio::test]
    async fn socket_sends_peercred_and_reads_the_answer() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cups.sock");
        let listener = tokio::net::UnixListener::bind(&path).unwrap();
        let server = tokio::spawn(async move {
            let (mut c, _) = listener.accept().await.unwrap();
            let mut got = vec![0u8; 4096];
            let n = c.read(&mut got).await.unwrap();
            got.truncate(n);
            let answer = ipp::Request::new(0).encode(); // status 0: successful-ok
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n",
                answer.len()
            );
            c.write_all(head.as_bytes()).await.unwrap();
            c.write_all(&answer).await.unwrap();
            String::from_utf8_lossy(&got).into_owned()
        });
        let s = Socket {
            path,
            user: "me".into(),
            installed: true,
        };
        let cups = Cups {
            t: std::sync::Arc::new(s),
            user: "me".into(),
        };
        let r = cups.request(ipp::op::CUPS_GET_DEFAULT);
        let resp = cups.send("/admin/", &r, READ_TIMEOUT).await.unwrap();
        assert_eq!(resp.status, 0);
        let sent = server.await.unwrap();
        assert!(sent.starts_with("POST /admin/ HTTP/1.1\r\n"), "{sent}");
        assert!(sent.contains("Authorization: PeerCred me\r\n"), "{sent}");
        assert!(sent.contains("requesting-user-name"), "{sent}");
    }
}
