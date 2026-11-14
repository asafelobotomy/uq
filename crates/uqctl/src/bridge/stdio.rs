//! The bridge's real stdin and stdout, each owned by one dedicated thread.
//!
//! tokio's `stdin()`/`stdout()` run every read and every flush as a `spawn_blocking` job, so each
//! line in or out is a hand-off to the blocking pool (and keeps pool threads around). Here one
//! plain thread blocks on fd 0 and another on fd 1; the async side talks to them over bounded
//! channels. The fds stay blocking (no `O_NONBLOCK` on pipes shared with Quickshell), and the
//! threads are never joined, so a peer that stops reading can't hold up the process exit.

use std::future::Future;
use std::io::{Read, Write};
use std::pin::Pin;
use std::task::{Context, Poll, ready};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::PollSender;

/// Chunks read ahead of the bridge: the reader thread blocks once these are queued.
const IN_CHUNKS: usize = 4;
/// Lines queued for the writer thread before `flush` waits (backpressure, as a blocked write had).
const OUT_LINES: usize = 32;
/// One `read` from fd 0 at most.
const CHUNK: usize = 8192;

/// fd 0, read by its own thread. Ends at EOF; a read error is passed on once.
pub struct ThreadStdin {
    rx: mpsc::Receiver<std::io::Result<Vec<u8>>>,
    chunk: Vec<u8>,
    at: usize,
}

impl ThreadStdin {
    pub fn spawn() -> std::io::Result<ThreadStdin> {
        ThreadStdin::spawn_from(std::io::stdin())
    }

    fn spawn_from<R: Read + Send + 'static>(mut r: R) -> std::io::Result<ThreadStdin> {
        let (tx, rx) = mpsc::channel(IN_CHUNKS);
        std::thread::Builder::new()
            .name("uqctl-stdin".into())
            .spawn(move || {
                let mut buf = vec![0u8; CHUNK];
                loop {
                    let item = match r.read(&mut buf) {
                        Ok(0) => return,
                        Ok(n) => Ok(buf[..n].to_vec()),
                        Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                        Err(e) => Err(e),
                    };
                    let last = item.is_err();
                    // The bridge has gone (reader aborted at shutdown): nothing left to do.
                    if tx.blocking_send(item).is_err() || last {
                        return;
                    }
                }
            })?;
        Ok(ThreadStdin {
            rx,
            chunk: Vec::new(),
            at: 0,
        })
    }
}

impl AsyncRead for ThreadStdin {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        out: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let me = &mut *self;
        if me.at == me.chunk.len() {
            match ready!(me.rx.poll_recv(cx)) {
                Some(Ok(c)) => {
                    me.chunk = c;
                    me.at = 0;
                }
                Some(Err(e)) => return Poll::Ready(Err(e)),
                // EOF (or the thread is gone).
                None => return Poll::Ready(Ok(())),
            }
        }
        let n = out.remaining().min(me.chunk.len() - me.at);
        out.put_slice(&me.chunk[me.at..me.at + n]);
        me.at += n;
        Poll::Ready(Ok(()))
    }
}

/// fd 1, written by its own thread. `write` collects bytes; `flush` hands them over as one
/// line, which the thread writes and flushes in order. `shutdown` waits until the thread has
/// written everything handed over (the bridge's bounded drain at exit).
pub struct ThreadStdout {
    pending: Vec<u8>,
    tx: PollSender<Vec<u8>>,
    done: oneshot::Receiver<()>,
}

impl ThreadStdout {
    pub fn spawn() -> std::io::Result<ThreadStdout> {
        ThreadStdout::spawn_to(std::io::stdout())
    }

    fn spawn_to<W: Write + Send + 'static>(mut w: W) -> std::io::Result<ThreadStdout> {
        let (tx, mut rx) = mpsc::channel::<Vec<u8>>(OUT_LINES);
        let (done_tx, done) = oneshot::channel();
        std::thread::Builder::new()
            .name("uqctl-stdout".into())
            .spawn(move || {
                while let Some(line) = rx.blocking_recv() {
                    // Closed: the next `flush` sees the channel gone and reports it.
                    if w.write_all(&line).and_then(|()| w.flush()).is_err() {
                        break;
                    }
                }
                let _ = done_tx.send(());
            })?;
        Ok(ThreadStdout {
            pending: Vec::new(),
            tx: PollSender::new(tx),
            done,
        })
    }
}

fn closed() -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::BrokenPipe, "stdout closed")
}

impl AsyncWrite for ThreadStdout {
    fn poll_write(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        self.pending.extend_from_slice(buf);
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        let me = &mut *self;
        if me.pending.is_empty() {
            return Poll::Ready(Ok(()));
        }
        ready!(me.tx.poll_reserve(cx)).map_err(|_| closed())?;
        me.tx
            .send_item(std::mem::take(&mut me.pending))
            .map_err(|_| closed())?;
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        ready!(self.as_mut().poll_flush(cx))?;
        self.tx.close();
        // Ok or Err (thread gone): either way it has stopped writing.
        let _ = ready!(Pin::new(&mut self.done).poll(cx));
        Poll::Ready(Ok(()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[derive(Clone, Default)]
    struct Sink(Arc<Mutex<Vec<u8>>>, Arc<Mutex<usize>>);

    impl Write for Sink {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            *self.1.lock().unwrap() += 1;
            Ok(())
        }
    }

    #[tokio::test]
    async fn stdout_lines_in_order_flushed_each() {
        let sink = Sink::default();
        let mut out = ThreadStdout::spawn_to(sink.clone()).unwrap();
        for i in 0..100 {
            out.write_all(format!("line {i}\n").as_bytes()).await.unwrap();
            out.flush().await.unwrap();
        }
        out.shutdown().await.unwrap();
        let want: String = (0..100).map(|i| format!("line {i}\n")).collect();
        assert_eq!(String::from_utf8(sink.0.lock().unwrap().clone()).unwrap(), want);
        assert_eq!(*sink.1.lock().unwrap(), 100, "one flush per line");
    }

    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::ErrorKind::BrokenPipe.into())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn stdout_closed_is_reported() {
        let mut out = ThreadStdout::spawn_to(Broken).unwrap();
        let mut failed = false;
        for _ in 0..(OUT_LINES * 4) {
            if out.write_all(b"x\n").await.is_err() || out.flush().await.is_err() {
                failed = true;
                break;
            }
        }
        assert!(failed, "a closed stdout surfaces as a write error");
        // Still returns: the thread has stopped.
        let _ = out.shutdown().await;
    }

    #[tokio::test]
    async fn stdin_chunks_then_eof() {
        let data: Vec<u8> = (0..50_000u32).map(|i| (i % 251) as u8).collect();
        let mut inp = ThreadStdin::spawn_from(std::io::Cursor::new(data.clone())).unwrap();
        let mut got = Vec::new();
        inp.read_to_end(&mut got).await.unwrap();
        assert_eq!(got, data);
    }

    struct Fails;
    impl Read for Fails {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("boom"))
        }
    }

    #[tokio::test]
    async fn stdin_error_is_passed_on() {
        let mut inp = ThreadStdin::spawn_from(Fails).unwrap();
        let mut b = [0u8; 4];
        assert_eq!(inp.read(&mut b).await.unwrap_err().to_string(), "boom");
    }
}
