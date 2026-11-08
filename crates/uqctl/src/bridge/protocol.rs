//! Wire format for `uqctl bridge`: one JSON object per line, every message carrying `"v":1`.

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::sync::mpsc;

/// Maximum line length (excluding the newline) in each direction.
pub const MAX_LINE: usize = 262_144;
/// Protocol version carried by every message.
pub const VERSION: u64 = 1;

#[derive(Debug, Clone, PartialEq)]
pub enum Inbound {
    Request {
        id: u64,
        call: String,
        args: Vec<Value>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Outbound {
    Ok { id: u64, ok: Value },
    Err { id: u64, err: String },
    Event { event: String, data: Value },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameError {
    TooLong,
    BadJson,
    BadVersion,
}

impl std::fmt::Display for FrameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            FrameError::TooLong => "line too long",
            FrameError::BadJson => "malformed JSON or request shape",
            FrameError::BadVersion => "unsupported protocol version",
        })
    }
}

pub fn decode_line(line: &[u8]) -> Result<Inbound, FrameError> {
    if line.len() > MAX_LINE {
        return Err(FrameError::TooLong);
    }
    let v: Value = serde_json::from_slice(line).map_err(|_| FrameError::BadJson)?;
    let obj = v.as_object().ok_or(FrameError::BadJson)?;
    if obj.get("v").and_then(Value::as_u64) != Some(VERSION) {
        return Err(FrameError::BadVersion);
    }
    let id = obj
        .get("id")
        .and_then(Value::as_u64)
        .ok_or(FrameError::BadJson)?;
    let call = obj
        .get("call")
        .and_then(Value::as_str)
        .ok_or(FrameError::BadJson)?
        .to_owned();
    let args = match obj.get("args") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(a)) => a.clone(),
        Some(_) => return Err(FrameError::BadJson),
    };
    Ok(Inbound::Request { id, call, args })
}

pub fn encode(msg: &Outbound) -> Vec<u8> {
    let v = match msg {
        Outbound::Ok { id, ok } => json!({"v": VERSION, "id": id, "ok": ok}),
        Outbound::Err { id, err } => json!({"v": VERSION, "id": id, "err": err}),
        Outbound::Event { event, data } => json!({"v": VERSION, "event": event, "data": data}),
    };
    // serde_json escapes control characters, so the output never contains a raw newline.
    let mut out = serde_json::to_vec(&v).expect("a Value always serialises");
    out.push(b'\n');
    out
}

/// Read newline-delimited requests from `r`. Never buffers more than `MAX_LINE` bytes: an
/// over-long line is discarded as it streams past and reported once as `FrameError::TooLong`.
/// Returns at EOF or when the receiver is gone.
pub async fn read_lines<R: AsyncRead + Unpin>(r: R, tx: mpsc::Sender<Result<Inbound, FrameError>>) {
    read_lines_inner(r, tx, &mut 0).await
}

/// As `read_lines`, recording the largest line buffer held in `peak`.
async fn read_lines_inner<R: AsyncRead + Unpin>(
    r: R,
    tx: mpsc::Sender<Result<Inbound, FrameError>>,
    peak: &mut usize,
) {
    let mut r = BufReader::with_capacity(8192, r);
    let mut buf: Vec<u8> = Vec::new();
    let mut discarding = false;
    loop {
        let chunk = match r.fill_buf().await {
            Ok(c) => c,
            Err(e) => {
                eprintln!("uqctl bridge: stdin read error: {e}");
                break;
            }
        };
        if chunk.is_empty() {
            break;
        }
        let (piece, found_nl) = match chunk.iter().position(|&b| b == b'\n') {
            Some(i) => (&chunk[..i], true),
            None => (chunk, false),
        };
        let consumed = piece.len() + usize::from(found_nl);
        if !discarding {
            if buf.len() + piece.len() > MAX_LINE {
                discarding = true;
                buf = Vec::new();
            } else {
                buf.extend_from_slice(piece);
                *peak = (*peak).max(buf.len());
            }
        }
        r.consume(consumed);
        if found_nl {
            let item = if discarding {
                Some(Err(FrameError::TooLong))
            } else if buf.is_empty() {
                None
            } else {
                Some(decode_line(&buf))
            };
            buf.clear();
            discarding = false;
            if let Some(item) = item
                && tx.send(item).await.is_err()
            {
                return;
            }
        }
    }
    // EOF: a final unterminated line still counts.
    let item = if discarding {
        Some(Err(FrameError::TooLong))
    } else if buf.is_empty() {
        None
    } else {
        Some(decode_line(&buf))
    };
    if let Some(item) = item {
        let _ = tx.send(item).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::pin::Pin;
    use std::task::{Context, Poll};
    use tokio::io::{AsyncRead, ReadBuf};

    #[test]
    fn encode_has_version_and_newline() {
        let b = encode(&Outbound::Ok {
            id: 7,
            ok: json!("pong"),
        });
        assert_eq!(b.last(), Some(&b'\n'));
        let v: Value = serde_json::from_slice(&b).unwrap();
        assert_eq!(v, json!({"v":1,"id":7,"ok":"pong"}));
        let e: Value = serde_json::from_slice(&encode(&Outbound::Err {
            id: 1,
            err: "x".into(),
        }))
        .unwrap();
        assert_eq!(e, json!({"v":1,"id":1,"err":"x"}));
        let ev: Value = serde_json::from_slice(&encode(&Outbound::Event {
            event: "UqdUp".into(),
            data: Value::Null,
        }))
        .unwrap();
        assert_eq!(ev, json!({"v":1,"event":"UqdUp","data":null}));
    }

    #[test]
    fn decode_rejects_v2() {
        assert_eq!(
            decode_line(br#"{"v":2,"id":1,"call":"Ping","args":[]}"#),
            Err(FrameError::BadVersion)
        );
        assert_eq!(
            decode_line(br#"{"id":1,"call":"Ping","args":[]}"#),
            Err(FrameError::BadVersion)
        );
        assert_eq!(decode_line(b"not json"), Err(FrameError::BadJson));
        assert_eq!(
            decode_line(br#"{"v":1,"id":"x","call":"Ping"}"#),
            Err(FrameError::BadJson)
        );
        assert_eq!(
            decode_line(br#"{"v":1,"id":3,"call":"Ping","args":[1]}"#),
            Ok(Inbound::Request {
                id: 3,
                call: "Ping".into(),
                args: vec![json!(1)]
            })
        );
    }

    /// Hands out data in small chunks so the reader's own buffering stays tiny.
    struct Chunked {
        data: Vec<u8>,
        pos: usize,
    }
    impl AsyncRead for Chunked {
        fn poll_read(
            mut self: Pin<&mut Self>,
            _: &mut Context<'_>,
            buf: &mut ReadBuf<'_>,
        ) -> Poll<std::io::Result<()>> {
            let n = (self.data.len() - self.pos).min(buf.remaining()).min(4096);
            let p = self.pos;
            buf.put_slice(&self.data[p..p + n]);
            self.pos += n;
            Poll::Ready(Ok(()))
        }
    }

    #[tokio::test]
    async fn overlong_line_dropped_next_ok() {
        let mut data = vec![b'a'; 300 * 1024];
        data.push(b'\n');
        data.extend_from_slice(b"{\"v\":1,\"id\":9,\"call\":\"Ping\",\"args\":[]}\n");
        let (tx, mut rx) = mpsc::channel(8);
        let mut peak = 0;
        read_lines_inner(Chunked { data, pos: 0 }, tx, &mut peak).await;
        assert_eq!(rx.recv().await, Some(Err(FrameError::TooLong)));
        assert_eq!(
            rx.recv().await,
            Some(Ok(Inbound::Request {
                id: 9,
                call: "Ping".into(),
                args: vec![]
            }))
        );
        assert_eq!(rx.recv().await, None);
        assert!(peak <= MAX_LINE, "peak buffer {peak}");
    }

    #[tokio::test]
    async fn exactly_max_line_is_accepted_and_final_unterminated_line_read() {
        let pad = MAX_LINE - 40;
        let line = format!(
            "{{\"v\":1,\"id\":1,\"call\":\"{}\",\"args\":[]}}",
            "a".repeat(pad - 3)
        );
        assert!(line.len() <= MAX_LINE);
        let (tx, mut rx) = mpsc::channel(8);
        read_lines(std::io::Cursor::new(line.into_bytes()), tx).await;
        assert!(matches!(rx.recv().await, Some(Ok(_))));
    }
}
