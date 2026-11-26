//! A local fake HTTP server for the bridge's fetch tests.
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// One canned response: status line, extra headers, body.
#[derive(Clone)]
pub struct Reply {
    pub status: &'static str,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Reply {
    pub fn ok(body: impl Into<Vec<u8>>) -> Reply {
        Reply {
            status: "200 OK",
            headers: Vec::new(),
            body: body.into(),
        }
    }
    pub fn status(status: &'static str) -> Reply {
        Reply {
            status,
            headers: Vec::new(),
            body: Vec::new(),
        }
    }
    pub fn redirect(to: &str) -> Reply {
        Reply {
            status: "302 Found",
            headers: vec![("Location".into(), to.into())],
            body: Vec::new(),
        }
    }
}

pub type Seen = Arc<Mutex<Vec<(Instant, String, String)>>>;

/// Serves `routes` by request path (falling back to `"*"`, else 404); `fail` is consulted
/// with the 1-based request number and may override the reply. Returns the base URL and
/// (time, request line, user agent) of every request.
pub async fn serve(
    routes: HashMap<String, Reply>,
    fail: impl Fn(usize) -> Option<Reply> + Send + Sync + 'static,
) -> (String, Seen) {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    let seen: Seen = Arc::new(Mutex::new(Vec::new()));
    let s2 = seen.clone();
    let routes = Arc::new(routes);
    let fail = Arc::new(fail);
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = l.accept().await else {
                return;
            };
            let (s3, routes, fail) = (s2.clone(), routes.clone(), fail.clone());
            tokio::spawn(async move {
                let mut buf = Vec::new();
                let mut chunk = [0u8; 1024];
                while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
                    match sock.read(&mut chunk).await {
                        Ok(0) | Err(_) => return,
                        Ok(n) => buf.extend_from_slice(&chunk[..n]),
                    }
                }
                let text = String::from_utf8_lossy(&buf).to_string();
                let line = text.lines().next().unwrap_or("").to_string();
                let ua = text
                    .lines()
                    .find_map(|l| {
                        l.to_ascii_lowercase()
                            .strip_prefix("user-agent: ")
                            .map(str::to_owned)
                    })
                    .unwrap_or_default();
                let n = {
                    let mut s = s3.lock().unwrap();
                    s.push((Instant::now(), line.clone(), ua));
                    s.len()
                };
                let path = line.split(' ').nth(1).unwrap_or("/").to_string();
                let path_only = path.split('?').next().unwrap_or("").to_string();
                let reply = fail(n).unwrap_or_else(|| {
                    routes
                        .get(&path)
                        .or_else(|| routes.get(&path_only))
                        .or_else(|| routes.get("*"))
                        .cloned()
                        .unwrap_or_else(|| Reply::status("404 Not Found"))
                });
                let mut head = format!(
                    "HTTP/1.1 {}\r\nContent-Length: {}\r\nConnection: close\r\n",
                    reply.status,
                    reply.body.len()
                );
                for (k, v) in &reply.headers {
                    head.push_str(&format!("{k}: {v}\r\n"));
                }
                head.push_str("\r\n");
                let _ = sock.write_all(head.as_bytes()).await;
                let _ = sock.write_all(&reply.body).await;
            });
        }
    });
    (base, seen)
}

pub fn routes(list: &[(&str, Reply)]) -> HashMap<String, Reply> {
    list.iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect()
}
