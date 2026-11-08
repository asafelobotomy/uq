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

/// A rejected input line. `id` is set when the line was valid JSON carrying a `u64` id, so the
/// bridge can answer that request with an error instead of leaving the caller waiting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rejected {
    pub id: Option<u64>,
    pub error: FrameError,
}

impl Rejected {
    fn new(id: Option<u64>, error: FrameError) -> Self {
        Rejected { id, error }
    }
}

pub type Item = Result<Inbound, Rejected>;

/// `decode_line`, keeping the request id (if any) of a rejected line.
pub fn decode_item(line: &[u8]) -> Item {
    decode_line(line).map_err(|error| {
        let id = serde_json::from_slice::<Value>(line)
            .ok()
            .and_then(|v| v.get("id").and_then(Value::as_u64));
        Rejected::new(id, error)
    })
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
pub async fn read_lines<R: AsyncRead + Unpin>(r: R, tx: mpsc::Sender<Item>) {
    read_lines_inner(r, tx, &mut 0).await
}

/// As `read_lines`, recording the largest line buffer held in `peak`.
async fn read_lines_inner<R: AsyncRead + Unpin>(r: R, tx: mpsc::Sender<Item>, peak: &mut usize) {
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
                Some(Err(Rejected::new(None, FrameError::TooLong)))
            } else if buf.is_empty() {
                None
            } else {
                Some(decode_item(&buf))
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
        Some(Err(Rejected::new(None, FrameError::TooLong)))
    } else if buf.is_empty() {
        None
    } else {
        Some(decode_item(&buf))
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

    /// Hands out data in `chunk`-sized pieces so the reader's own buffering stays tiny.
    struct Chunked {
        data: Vec<u8>,
        pos: usize,
        chunk: usize,
    }
    impl AsyncRead for Chunked {
        fn poll_read(
            mut self: Pin<&mut Self>,
            _: &mut Context<'_>,
            buf: &mut ReadBuf<'_>,
        ) -> Poll<std::io::Result<()>> {
            let n = (self.data.len() - self.pos)
                .min(buf.remaining())
                .min(self.chunk);
            let p = self.pos;
            buf.put_slice(&self.data[p..p + n]);
            self.pos += n;
            Poll::Ready(Ok(()))
        }
    }

    const CHUNKS: [usize; 6] = [1, 7, 4096, MAX_LINE, MAX_LINE + 1, 1 << 20];

    /// A valid request line of exactly `len` bytes (no newline).
    fn request_of_len(id: u64, len: usize) -> Vec<u8> {
        let head = format!("{{\"v\":1,\"id\":{id},\"call\":\"");
        let tail = "\",\"args\":[]}";
        let pad = len - head.len() - tail.len();
        let line = format!("{head}{}{tail}", "a".repeat(pad));
        assert_eq!(line.len(), len);
        line.into_bytes()
    }

    async fn read_all(data: Vec<u8>, chunk: usize) -> (Vec<Item>, usize) {
        let (tx, mut rx) = mpsc::channel(64);
        let mut peak = 0;
        read_lines_inner(
            Chunked {
                data,
                pos: 0,
                chunk,
            },
            tx,
            &mut peak,
        )
        .await;
        let mut items = Vec::new();
        while let Some(i) = rx.recv().await {
            items.push(i);
        }
        (items, peak)
    }

    fn ping(id: u64) -> Vec<u8> {
        format!("{{\"v\":1,\"id\":{id},\"call\":\"Ping\",\"args\":[]}}\n").into_bytes()
    }

    fn too_long() -> Item {
        Err(Rejected::new(None, FrameError::TooLong))
    }

    fn is_ping(i: &Item, id: u64) -> bool {
        matches!(i, Ok(Inbound::Request { id: got, call, .. }) if *got == id && call == "Ping")
    }

    #[tokio::test]
    async fn exact_max_line_accepted_at_any_chunking() {
        for chunk in CHUNKS {
            let mut data = request_of_len(1, MAX_LINE);
            data.push(b'\n');
            data.extend(ping(2));
            let (items, peak) = read_all(data, chunk).await;
            assert_eq!(items.len(), 2, "chunk {chunk}");
            assert!(matches!(items[0], Ok(Inbound::Request { id: 1, .. })));
            assert!(is_ping(&items[1], 2));
            assert!(peak <= MAX_LINE, "peak {peak}");
        }
    }

    #[tokio::test]
    async fn max_line_plus_one_is_too_long_at_any_chunking() {
        for chunk in CHUNKS {
            let mut data = request_of_len(1, MAX_LINE + 1);
            data.push(b'\n');
            data.extend(ping(2));
            let (items, peak) = read_all(data, chunk).await;
            assert_eq!(items.len(), 2, "chunk {chunk}");
            assert_eq!(items[0], too_long(), "chunk {chunk}");
            assert!(is_ping(&items[1], 2));
            assert!(peak <= MAX_LINE, "peak {peak}");
        }
    }

    #[tokio::test]
    async fn overlong_line_dropped_next_ok() {
        let mut data = vec![b'a'; 300 * 1024];
        data.push(b'\n');
        data.extend(ping(9));
        let (items, peak) = read_all(data, 4096).await;
        assert_eq!(items.len(), 2);
        assert_eq!(items[0], too_long());
        assert!(is_ping(&items[1], 9));
        assert!(peak <= MAX_LINE, "peak buffer {peak}");
    }

    #[tokio::test]
    async fn unterminated_lines_at_eof() {
        let (items, _) = read_all(vec![b'a'; MAX_LINE + 10], 4096).await;
        assert_eq!(items, vec![too_long()]);
        let (items, _) = read_all(request_of_len(3, MAX_LINE), 4096).await;
        assert!(matches!(items[..], [Ok(Inbound::Request { id: 3, .. })]));
        let mut data = ping(1);
        data.extend(b"{\"v\":1,\"id\":4,\"call\":\"Ping\"}");
        let (items, _) = read_all(data, 4096).await;
        assert_eq!(items.len(), 2);
        assert!(is_ping(&items[1], 4));
    }

    #[tokio::test]
    async fn empty_lines_skipped() {
        let mut data = b"\n\n".to_vec();
        data.extend(ping(1));
        data.extend(b"\n");
        let (items, _) = read_all(data, 1).await;
        assert_eq!(items.len(), 1);
        assert!(is_ping(&items[0], 1));
    }

    #[tokio::test]
    async fn malformed_lines_keep_id_only_when_valid_u64() {
        let bad = |id| Err(Rejected::new(id, FrameError::BadJson));
        let cases: Vec<(&[u8], Item)> = vec![
            (b"\xff\xfe not utf8\n", bad(None)),
            (b"{\"v\":1,\"id\":\"\xff\",\"call\":\"Ping\"}\n", bad(None)),
            (b"{\"v\":1,\"id\":-1,\"call\":\"Ping\"}\n", bad(None)),
            (
                b"{\"v\":1,\"id\":18446744073709551616,\"call\":\"Ping\"}\n",
                bad(None),
            ),
            (b"{\"v\":1,\"id\":7,\"call\":5}\n", bad(Some(7))),
            (
                b"{\"v\":1,\"id\":7,\"call\":\"P\",\"args\":3}\n",
                bad(Some(7)),
            ),
            (
                b"{\"v\":2,\"id\":8,\"call\":\"Ping\"}\n",
                Err(Rejected::new(Some(8), FrameError::BadVersion)),
            ),
        ];
        for (line, want) in cases {
            let (items, _) = read_all(line.to_vec(), 4096).await;
            assert_eq!(items, vec![want], "{}", String::from_utf8_lossy(line));
        }
        let (items, _) = read_all(
            b"{\"v\":1,\"id\":18446744073709551615,\"call\":\"P\"}\n".to_vec(),
            4096,
        )
        .await;
        assert!(matches!(
            items[..],
            [Ok(Inbound::Request { id: u64::MAX, .. })]
        ));
    }
}
