use super::*;
use serde_json::json;
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
fn encode_bytes_match_a_json_value() {
    // The same bytes as serialising the whole message as one `Value` (sorted keys).
    let data = json!({"b": [1, 2.5, "x\n"], "a": {"z": null, "y": true}});
    let cases = [
        (
            Outbound::Ok {
                id: 3,
                ok: data.clone(),
            },
            json!({"v": VERSION, "id": 3, "ok": data}),
        ),
        (
            Outbound::Err {
                id: 4,
                err: "bad \"thing\"".into(),
            },
            json!({"v": VERSION, "id": 4, "err": "bad \"thing\""}),
        ),
        (
            Outbound::Event {
                event: "Hello".into(),
                data: data.clone(),
            },
            json!({"v": VERSION, "event": "Hello", "data": data}),
        ),
    ];
    for (msg, v) in cases {
        let mut want = serde_json::to_vec(&v).unwrap();
        want.push(b'\n');
        assert_eq!(encode(&msg), want);
    }
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
