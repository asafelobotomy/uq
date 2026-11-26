use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Serves `body` (200, JSON) to every connection; returns the base URL and the request lines.
async fn fake(body: &'static str) -> (String, Arc<std::sync::Mutex<Vec<String>>>) {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
    let s2 = seen.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = l.accept().await else {
                return;
            };
            let s3 = s2.clone();
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
                s3.lock()
                    .unwrap()
                    .push(text.lines().next().unwrap().to_string());
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = sock.write_all(resp.as_bytes()).await;
            });
        }
    });
    (base, seen)
}

fn settings(loc: Option<(&str, f64)>, ip: bool) -> Option<Settings> {
    let mut s = Settings {
        weather_location: loc.map(|(n, lat)| Location {
            name: n.into(),
            lat,
            lon: 0.5,
        }),
        ..Settings::default()
    };
    s.privacy.public_ip_lookup = ip;
    if s.weather_location.is_some()
        && let Some(b) = s.shell.bar.as_mut()
    {
        b.end.insert(0, uq_core::settings::BarItem::new("weather"));
    }
    Some(s)
}

async fn event(rx: &mut mpsc::Receiver<Outbound>) -> (String, Value) {
    match tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .unwrap()
        .unwrap()
    {
        Outbound::Event { event, data } => (event, data),
        _ => panic!("expected event"),
    }
}

const WX: &str = r#"{"current":{"temperature_2m":7.0,"weather_code":61,"is_day":0}}"#;

/// Serves WX (200) except for the 1-based request numbers in `fail` (500); records the time
/// of each request.
async fn scripted(fail: &'static [usize]) -> (String, Arc<std::sync::Mutex<Vec<Instant>>>) {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    let times = Arc::new(std::sync::Mutex::new(Vec::new()));
    let t2 = times.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = l.accept().await else {
                return;
            };
            let n = {
                let mut t = t2.lock().unwrap();
                t.push(Instant::now());
                t.len()
            };
            tokio::spawn(async move {
                let mut buf = Vec::new();
                let mut chunk = [0u8; 1024];
                while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
                    match sock.read(&mut chunk).await {
                        Ok(0) | Err(_) => return,
                        Ok(k) => buf.extend_from_slice(&chunk[..k]),
                    }
                }
                let resp = if fail.contains(&n) {
                    "HTTP/1.1 500 Err\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string()
                } else {
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{WX}",
                        WX.len()
                    )
                };
                let _ = sock.write_all(resp.as_bytes()).await;
            });
        }
    });
    (base, times)
}

fn ms(a: Instant, b: Instant) -> u128 {
    b.duration_since(a).as_millis()
}

mod fetch;
mod loops;
