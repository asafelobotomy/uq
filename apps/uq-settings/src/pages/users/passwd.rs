//! Changing your own password through `/usr/bin/passwd`, the way GNOME Settings does: passwd
//! runs as you, PAM checks the current password and the password rules, and no administrator
//! is asked. Passwords go to passwd's standard input one at a time, each only once passwd has
//! asked for it; they are never in its arguments or environment, and the bytes written are
//! zeroed afterwards. passwd's prompts are read in the C locale.

use std::ffi::OsString;
use std::process::Stdio;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use tokio::sync::mpsc;

/// The real program; tests use a stub run by `/bin/sh`.
pub const PASSWD: &str = "/usr/bin/passwd";
/// A whole change (passwd answers at once; PAM modules may take a moment).
pub const TIMEOUT: Duration = Duration::from_secs(30);
/// The most passwd output kept for messages.
const MAX_TRANSCRIPT: usize = 16 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Prompt {
    Current,
    New,
    Retype,
}

/// What a line ending in `password:` asks for, if it is a password prompt at all.
fn prompt_kind(line: &str) -> Option<Prompt> {
    let l = line.trim_end().to_ascii_lowercase();
    if !l.ends_with(':') || !l.contains("password") {
        return None;
    }
    Some(
        if l.contains("retype") || l.contains("again") || l.contains("repeat") {
            Prompt::Retype
        } else if l.contains("current") || l.contains("old") {
            Prompt::Current
        } else if l.contains("new") {
            Prompt::New
        } else {
            // A bare "Password:" before any new one is the current password.
            Prompt::Current
        },
    )
}

/// A line of passwd's output fit to show: plain, short, without passwd's own prefix.
fn shown(line: &str) -> String {
    let t: String = line
        .trim()
        .trim_start_matches("passwd:")
        .trim()
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(200)
        .collect();
    t
}

/// Write `secret` and a newline to passwd, then zero the copy.
async fn answer(stdin: &mut tokio::process::ChildStdin, secret: &str) -> std::io::Result<()> {
    let mut buf = Vec::with_capacity(secret.len() + 1);
    buf.extend_from_slice(secret.as_bytes());
    buf.push(b'\n');
    let r = async {
        stdin.write_all(&buf).await?;
        stdin.flush().await
    }
    .await;
    for b in buf.iter_mut() {
        // SAFETY: a valid, owned byte.
        unsafe { std::ptr::write_volatile(b, 0) };
    }
    r
}

/// Why the change didn't happen, from what passwd said.
fn failure_text(lines: &[String], answered_current: bool, asked_new: bool) -> String {
    let said = |needle: &str| {
        lines
            .iter()
            .any(|l| l.to_ascii_lowercase().contains(needle))
    };
    if let Some(bad) = lines.iter().rev().find(|l| l.starts_with("BAD PASSWORD")) {
        let why = bad.trim_start_matches("BAD PASSWORD:").trim();
        return format!("That password can't be used: {why}");
    }
    if answered_current
        && !asked_new
        && (said("authentication") || said("incorrect") || said("token manipulation"))
    {
        return "The current password is wrong.".into();
    }
    match lines
        .iter()
        .rev()
        .find(|l| !l.is_empty() && prompt_kind(l).is_none())
    {
        Some(l) => format!("The password wasn't changed: {l}"),
        None => "The password wasn't changed.".into(),
    }
}

/// Change the running user's password with `cmd` (`[PASSWD]` normally): answer its current,
/// new and retype prompts. `Err` is a message for the person.
pub async fn change_own(
    cmd: &[OsString],
    current: &str,
    new: &str,
    timeout: Duration,
) -> Result<(), String> {
    let (program, args) = cmd.split_first().ok_or("No passwd program.")?;
    let mut child = Command::new(program)
        .args(args)
        .env("LC_ALL", "C")
        .env("LANGUAGE", "C")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("The password couldn't be changed: passwd didn't start ({e})."))?;
    let mut stdin = child.stdin.take().ok_or("passwd has no input")?;
    // Both streams feed one channel: PAM prompts go to stderr, passwd's notes to stdout.
    let (tx, mut rx) = mpsc::channel::<Vec<u8>>(16);
    for stream in [
        child
            .stdout
            .take()
            .map(|s| Box::new(s) as Box<dyn tokio::io::AsyncRead + Unpin + Send>),
        child
            .stderr
            .take()
            .map(|s| Box::new(s) as Box<dyn tokio::io::AsyncRead + Unpin + Send>),
    ]
    .into_iter()
    .flatten()
    {
        let tx = tx.clone();
        tokio::spawn(async move {
            let mut stream = stream;
            let mut buf = [0u8; 1024];
            while let Ok(n) = stream.read(&mut buf).await {
                if n == 0 || tx.send(buf[..n].to_vec()).await.is_err() {
                    break;
                }
            }
        });
    }
    drop(tx);

    let work = async {
        let mut pending = String::new();
        let mut lines: Vec<String> = Vec::new();
        let mut kept = 0usize;
        let (mut answered_current, mut news, mut retypes) = (false, 0u32, 0u32);
        while let Some(chunk) = rx.recv().await {
            if kept < MAX_TRANSCRIPT {
                kept += chunk.len();
                pending.push_str(&String::from_utf8_lossy(&chunk));
            }
            // Whole lines are messages; a trailing unfinished one may be a prompt.
            while let Some(i) = pending.find('\n') {
                let line: String = pending.drain(..=i).collect();
                lines.push(shown(&line));
            }
            let Some(kind) = prompt_kind(&pending) else {
                continue;
            };
            lines.push(shown(&pending));
            pending.clear();
            let secret = match kind {
                Prompt::Current if !answered_current && news == 0 => {
                    answered_current = true;
                    current
                }
                Prompt::New if news == 0 => {
                    news += 1;
                    new
                }
                Prompt::Retype if news == 1 && retypes == 0 => {
                    retypes += 1;
                    new
                }
                // Asked again: the password was refused (rules, mismatch) or something else
                // wants an answer. Stop rather than guess.
                _ => return Err(failure_text(&lines, answered_current, news > 0)),
            };
            if answer(&mut stdin, secret).await.is_err() {
                break;
            }
        }
        if !pending.is_empty() {
            lines.push(shown(&pending));
        }
        let status = child.wait().await.map_err(|e| e.to_string())?;
        if status.success() {
            Ok(())
        } else {
            Err(failure_text(&lines, answered_current, news > 0))
        }
    };
    let r = match tokio::time::timeout(timeout, work).await {
        Ok(r) => r,
        Err(_) => Err("Changing the password took too long, so it was stopped.".into()),
    };
    // On a refusal or timeout passwd may still be waiting for input.
    let _ = child.start_kill();
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompts_are_recognised() {
        assert_eq!(prompt_kind("Current password: "), Some(Prompt::Current));
        assert_eq!(
            prompt_kind("(current) UNIX password:"),
            Some(Prompt::Current)
        );
        assert_eq!(prompt_kind("Password: "), Some(Prompt::Current));
        assert_eq!(prompt_kind("New password: "), Some(Prompt::New));
        assert_eq!(prompt_kind("Retype new password: "), Some(Prompt::Retype));
        assert_eq!(
            prompt_kind("Enter new password again:"),
            Some(Prompt::Retype)
        );
        assert_eq!(prompt_kind("Changing password for alice."), None);
        assert_eq!(prompt_kind("passwd: password updated successfully"), None);
    }

    #[test]
    fn failures_read_plainly() {
        let l = |v: &[&str]| v.iter().map(|s| shown(s)).collect::<Vec<_>>();
        assert_eq!(
            failure_text(
                &l(&[
                    "Current password:",
                    "passwd: Authentication token manipulation error"
                ]),
                true,
                false
            ),
            "The current password is wrong."
        );
        assert_eq!(
            failure_text(
                &l(&[
                    "New password:",
                    "BAD PASSWORD: The password is shorter than 8 characters"
                ]),
                true,
                true
            ),
            "That password can't be used: The password is shorter than 8 characters"
        );
        assert_eq!(
            failure_text(
                &l(&["You must wait longer to change your password"]),
                false,
                false
            ),
            "The password wasn't changed: You must wait longer to change your password"
        );
        assert_eq!(
            failure_text(&[], false, false),
            "The password wasn't changed."
        );
    }
}
