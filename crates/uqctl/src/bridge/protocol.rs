//! Wire format for `uqctl bridge`: one JSON object per line, every message carrying `"v":1`.

use serde::Serialize;
use serde_json::Value;
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
    let mut v: Value = serde_json::from_slice(line).map_err(|_| FrameError::BadJson)?;
    let obj = v.as_object_mut().ok_or(FrameError::BadJson)?;
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
    // Taken out of the parsed line, not copied: arguments can be large (a bar layout).
    let args = match obj.remove("args") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(a)) => a,
        Some(_) => return Err(FrameError::BadJson),
    };
    Ok(Inbound::Request { id, call, args })
}

/// One outgoing line, serialised straight from the borrowed message (no copy of the payload).
/// Fields are in the order `json!` gave them (serde_json's map is sorted), so the bytes on the
/// wire are unchanged.
#[derive(Serialize)]
#[serde(untagged)]
enum Wire<'a> {
    Ok {
        id: u64,
        ok: &'a Value,
        v: u64,
    },
    Err {
        err: &'a str,
        id: u64,
        v: u64,
    },
    Event {
        data: &'a Value,
        event: &'a str,
        v: u64,
    },
}

pub fn encode(msg: &Outbound) -> Vec<u8> {
    let v = VERSION;
    let w = match msg {
        Outbound::Ok { id, ok } => Wire::Ok { id: *id, ok, v },
        Outbound::Err { id, err } => Wire::Err { err, id: *id, v },
        Outbound::Event { event, data } => Wire::Event { data, event, v },
    };
    // serde_json escapes control characters, so the output never contains a raw newline.
    let mut out = serde_json::to_vec(&w).expect("a Value always serialises");
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
mod tests;
