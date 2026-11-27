//! `uqctl reset` and `uqctl backup`: every setting at once. A reset puts each setting back to
//! uQ's defaults; a backup is a file of the current settings that `backup restore` brings back.
//! Both save a restore point of the settings first (`uqctl restore list`), so either can be
//! undone. Settings › Restore runs these.

use crate::profile_cmd::describe;
use clap::Subcommand;
use std::io::{BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};
use uq_core::atomic::atomic_write;

#[derive(Subcommand, Debug)]
pub enum BackupCmd {
    /// Save the current settings to FILE.
    Save { file: PathBuf },
    /// Make FILE's settings the current ones (a restore point of these is saved first).
    Restore { file: PathBuf },
}

/// What `uqctl reset` says before it asks.
pub const RESET_WARNING: &str = "Every uQ setting goes back to how uQ comes: theme, bar, shortcuts, \
gestures, window rules, startup apps and the rest. Your files, apps and wallpapers stay, and so does \
your own user.lua. A restore point of your current settings is saved first: `uqctl restore list` \
shows it, marked auto:before_reset.";

/// The largest file `backup restore` reads; uqd refuses anything bigger too.
const MAX_BYTES: u64 = 1024 * 1024;

pub async fn reset(proxy: &zbus::Proxy<'_>, yes: bool) -> Result<(), String> {
    if !yes {
        if !std::io::stdin().is_terminal() {
            return Err(
                "resetting every setting needs --yes when there's no terminal to ask on".into(),
            );
        }
        println!("{RESET_WARNING}");
        print!("Reset all settings? [y/N] ");
        let _ = std::io::stdout().flush();
        let mut answer = String::new();
        std::io::stdin()
            .lock()
            .read_line(&mut answer)
            .map_err(|e| e.to_string())?;
        if !matches!(answer.trim().to_lowercase().as_str(), "y" | "yes") {
            return Err("nothing was changed".into());
        }
    }
    proxy
        .call::<_, _, ()>("ResetAll", &())
        .await
        .map_err(describe)?;
    println!(
        "Every setting is back to uQ's defaults. To go back: uqctl restore list, then uqctl restore <id>."
    );
    Ok(())
}

pub async fn backup(proxy: &zbus::Proxy<'_>, cmd: BackupCmd) -> Result<(), String> {
    match cmd {
        BackupCmd::Save { file } => {
            let text: String = proxy.call("ExportSettings", &()).await.map_err(describe)?;
            write(&file, &text)?;
            println!("Settings saved to {}", file.display());
        }
        BackupCmd::Restore { file } => {
            let text = read(&file)?;
            proxy
                .call::<_, _, ()>("ImportSettings", &(text,))
                .await
                .map_err(describe)?;
            println!(
                "Settings from {} are the current ones. To go back: uqctl restore list, then uqctl restore <id>.",
                file.display()
            );
        }
    }
    Ok(())
}

fn write(file: &Path, text: &str) -> Result<(), String> {
    if let Some(dir) = file.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    atomic_write(file, text).map_err(|e| format!("{}: {e}", file.display()))
}

fn read(file: &Path) -> Result<String, String> {
    let meta = std::fs::metadata(file).map_err(|e| format!("{}: {e}", file.display()))?;
    if meta.len() > MAX_BYTES {
        return Err(format!(
            "{} is too big to be a uQ settings backup",
            file.display()
        ));
    }
    std::fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_backup_file_round_trips_and_big_files_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("sub/backup.toml");
        write(&f, "# uQ settings backup\n").unwrap();
        assert_eq!(read(&f).unwrap(), "# uQ settings backup\n");
        std::fs::write(&f, vec![b'a'; MAX_BYTES as usize + 1]).unwrap();
        assert!(read(&f).unwrap_err().contains("too big"));
        assert!(read(&dir.path().join("missing")).is_err());
    }
}
