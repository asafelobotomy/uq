//! Opening a link or a file with the user's default app (`xdg-open`), for news links, typed web
//! addresses and look=see>do_'s files.
//!
//! The opener runs in a scope of its own in `app.slice` (`app-uq-<name>-<rand>.scope`), so the
//! app it starts is an app, not part of the shell's service, which schedulers such as scdr treat
//! as the desktop. Without systemd-run it starts directly. It runs detached, in its own process
//! group, with no stdio, and is reaped whenever it ends.

use std::process::Stdio;

/// Start `opener arg` detached; `scope`: the scope's name, or None to run it as is (tests use a
/// fake opener). Returns once it has started.
pub fn open(opener: &str, arg: &str, scope: Option<&str>) -> Result<(), String> {
    let plain = [opener.to_string(), arg.to_string()];
    let start = |argv: &[String]| {
        tokio::process::Command::new(&argv[0])
            .args(&argv[1..])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()
    };
    let spawned = match scope {
        Some(name) => {
            let scoped = uq_core::desktop_entry::scope_argv(
                name,
                &plain,
                uq_core::desktop_entry::random_suffix(),
            );
            match start(&scoped) {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => start(&plain),
                r => r,
            }
        }
        None => start(&plain),
    };
    let mut child = spawned.map_err(|e| format!("could not start {opener} ({})", e.kind()))?;
    // Reap it whenever it ends, so it never lingers as a zombie. It has already been reported as
    // opened, so a failure after starting (no default app, a broken handler) can only be logged.
    let what = opener.to_string();
    tokio::spawn(async move {
        if let Ok(status) = child.wait().await
            && !status.success()
        {
            eprintln!("uqctl bridge: {what} failed ({status})");
        }
    });
    Ok(())
}
