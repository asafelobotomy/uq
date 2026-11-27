use super::*;
use crate::integration::manifest::ArgSpec;
use std::time::Instant;

/// The shell and `env` live in /bin or /usr/bin depending on the system; tests allow both.
fn runner() -> Runner {
    Runner::system().with_prefixes(vec!["/bin".into(), "/usr/bin".into()])
}

fn sh() -> String {
    if std::path::Path::new("/usr/bin/sh").exists() {
        "/usr/bin/sh".into()
    } else {
        "/bin/sh".into()
    }
}

fn spec(script: &str, output: Output, timeout_ms: u64) -> CommandSpec {
    CommandSpec {
        id: "t".into(),
        argv: vec![sh(), "-c".into(), script.into(), "sh".into()],
        output,
        timeout_ms,
        args: Vec::new(),
        confirm: None,
        preview: None,
        watch: None,
    }
}

fn with_args(mut s: CommandSpec, args: &[(&str, &str)]) -> CommandSpec {
    s.args = args
        .iter()
        .map(|(n, p)| ArgSpec::new(n, p).unwrap())
        .collect();
    s
}

fn strings(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

/// Whether `pid` still runs (a zombie waiting for its reaper counts as gone).
fn alive(pid: &str) -> bool {
    match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
        Ok(stat) => !stat
            .rsplit_once(')')
            .is_some_and(|(_, rest)| rest.trim_start().starts_with('Z')),
        Err(_) => false,
    }
}

async fn wait_gone(pid: &str) -> bool {
    for _ in 0..50 {
        if !alive(pid) {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    false
}

mod run;
mod stream;
