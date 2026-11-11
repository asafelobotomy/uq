//! Running PipeWire's command-line tools for the Sound page: argv only (never a shell), stdout
//! captured, a time limit on every call, and a child that is killed when the page closes.

use std::process::Stdio;
use std::time::Duration;

use futures_util::future::BoxFuture;
use futures_util::stream::BoxStream;
use futures_util::{FutureExt, StreamExt};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

/// How long one call may take.
pub const TIMEOUT: Duration = Duration::from_secs(5);
/// How long the test sound may play before it is stopped.
pub const PLAY_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolError {
    /// The program isn't installed.
    Missing(String),
    /// It ran and failed, with what it said.
    Failed(String),
    TimedOut,
}

/// The tools the page uses. A trait so tests can stand in a fake PipeWire.
pub trait Tools: Send + Sync + 'static {
    /// Run `argv` and return its standard output.
    fn run(&self, argv: Vec<String>) -> BoxFuture<'static, Result<String, ToolError>>;
    /// Run `argv` without waiting for it (the test sound); it is stopped after
    /// [`PLAY_TIMEOUT`].
    fn start(&self, argv: Vec<String>) -> Result<(), ToolError>;
    /// One item each time `argv`'s output gets a line (`pw-dump --monitor`); ends with it.
    fn lines(&self, argv: Vec<String>) -> BoxStream<'static, ()>;
}

/// The real programs.
pub struct System;

fn command(argv: &[String]) -> Command {
    let mut c = Command::new(&argv[0]);
    c.args(&argv[1..])
        .stdin(Stdio::null())
        .kill_on_drop(true)
        // Plain output, without colour codes.
        .env("NO_COLOR", "1");
    c
}

fn spawn_error(program: &str, e: &std::io::Error) -> ToolError {
    if e.kind() == std::io::ErrorKind::NotFound {
        ToolError::Missing(program.to_string())
    } else {
        ToolError::Failed(format!("{program} couldn't start: {e}"))
    }
}

impl Tools for System {
    fn run(&self, argv: Vec<String>) -> BoxFuture<'static, Result<String, ToolError>> {
        async move {
            let program = argv[0].clone();
            let child = command(&argv)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|e| spawn_error(&program, &e))?;
            let out = match tokio::time::timeout(TIMEOUT, child.wait_with_output()).await {
                Ok(Ok(out)) => out,
                Ok(Err(e)) => return Err(ToolError::Failed(format!("{program}: {e}"))),
                // Dropping the child kills it (kill_on_drop).
                Err(_) => return Err(ToolError::TimedOut),
            };
            if out.status.success() {
                Ok(String::from_utf8_lossy(&out.stdout).into_owned())
            } else {
                let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
                Err(ToolError::Failed(if err.is_empty() {
                    format!("{program} failed ({})", out.status)
                } else {
                    err
                }))
            }
        }
        .boxed()
    }

    fn start(&self, argv: Vec<String>) -> Result<(), ToolError> {
        let program = argv[0].clone();
        let mut child = command(&argv)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| spawn_error(&program, &e))?;
        tokio::spawn(async move {
            if tokio::time::timeout(PLAY_TIMEOUT, child.wait())
                .await
                .is_err()
            {
                let _ = child.kill().await;
            }
        });
        Ok(())
    }

    fn lines(&self, argv: Vec<String>) -> BoxStream<'static, ()> {
        let Ok(mut child) = command(&argv)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
        else {
            return futures_util::stream::empty().boxed();
        };
        let Some(out) = child.stdout.take() else {
            return futures_util::stream::empty().boxed();
        };
        let lines = BufReader::new(out).lines();
        futures_util::stream::unfold((lines, child), |(mut lines, child)| async move {
            match lines.next_line().await {
                Ok(Some(_)) => Some(((), (lines, child))),
                _ => None,
            }
        })
        .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[tokio::test]
    async fn runs_without_a_shell_and_reports_failures() {
        let out = System.run(argv(&["echo", "a;b", "$HOME"])).await.unwrap();
        assert_eq!(out, "a;b $HOME\n", "arguments are never shell-expanded");
        assert_eq!(
            System.run(argv(&["uq-no-such-program"])).await,
            Err(ToolError::Missing("uq-no-such-program".into()))
        );
        let e = System
            .run(argv(&["sh", "-c", "echo nope >&2; exit 3"]))
            .await
            .unwrap_err();
        assert_eq!(e, ToolError::Failed("nope".into()));
    }

    #[tokio::test]
    async fn a_hung_tool_times_out() {
        let started = std::time::Instant::now();
        assert_eq!(
            System.run(argv(&["sleep", "30"])).await,
            Err(ToolError::TimedOut)
        );
        assert!(started.elapsed() < Duration::from_secs(10));
    }

    #[tokio::test]
    async fn lines_follow_the_output() {
        let mut s = System.lines(argv(&["printf", "a\\nb\\n"]));
        assert_eq!(s.next().await, Some(()));
        assert_eq!(s.next().await, Some(()));
        assert_eq!(s.next().await, None);
        assert!(
            System
                .lines(argv(&["uq-no-such-program"]))
                .next()
                .await
                .is_none()
        );
    }
}
