//! Run provider for the Spotlight `>` queries.
//!
//! The command text never leaves the bridge in a result: the shell only sees a per-generation
//! id, and `Launch {run_id}` looks the command up here.

use super::calc::ProviderItem;

const MAX_COMMAND_BYTES: usize = 4096;
const MAX_TITLE_CHARS: usize = 80;

/// The command offered for the newest search generation; older ids are dead.
#[derive(Debug, Default)]
pub(crate) struct RunState {
    generation: u64,
    command: Option<String>,
}

/// The `Run` row for a `>` query, remembering the command for `generation`. Call it once per
/// search: every generation at least as new as the stored one advances the state and drops the
/// old command, even when it yields no item; an older (late) generation changes nothing.
pub(crate) fn run_item(query: &str, generation: u64, state: &mut RunState) -> Option<ProviderItem> {
    if generation < state.generation {
        return None;
    }
    state.generation = generation;
    state.command = None;
    let cmd = query.strip_prefix('>')?.trim();
    if cmd.is_empty() || cmd.len() > MAX_COMMAND_BYTES || cmd.contains('\0') {
        return None;
    }
    state.command = Some(cmd.to_string());
    let mut shown: String = cmd.chars().take(MAX_TITLE_CHARS).collect();
    if cmd.chars().count() > MAX_TITLE_CHARS {
        shown.push('…');
    }
    Some(ProviderItem {
        id: format!("run-{generation}"),
        title: format!("Run \"{shown}\""),
        subtitle: "in a terminal-less scope".into(),
        icon: "utilities-terminal".into(),
        score: 1.0,
        actions: vec!["run".into()],
    })
}

/// The command for `run_id`, only while it is the current generation's id.
pub(crate) fn take_command(state: &RunState, run_id: &str) -> Result<String, String> {
    match &state.command {
        Some(c) if run_id == format!("run-{}", state.generation) => Ok(c.clone()),
        _ => Err("unknown run id".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_item_only_with_prefix() {
        let mut s = RunState::default();
        assert!(run_item("ls", 1, &mut s).is_none());
        assert!(run_item(" > ls", 1, &mut s).is_none());
        let it = run_item(">  ls -la ", 2, &mut s).unwrap();
        assert_eq!(it.id, "run-2");
        assert_eq!(it.title, "Run \"ls -la\"");
        assert_eq!(it.subtitle, "in a terminal-less scope");
        assert_eq!(it.icon, "utilities-terminal");
        assert_eq!(it.score, 1.0);
        assert_eq!(it.actions, vec!["run"]);
        assert_eq!(take_command(&s, "run-2").unwrap(), "ls -la");
    }

    #[test]
    fn empty_command_no_item() {
        let mut s = RunState::default();
        assert!(run_item(">", 1, &mut s).is_none());
        assert!(run_item(">   ", 1, &mut s).is_none());
        assert!(take_command(&s, "run-1").is_err());
    }

    #[test]
    fn run_id_from_old_generation_rejected() {
        let mut s = RunState::default();
        run_item("> ls", 3, &mut s).unwrap();
        run_item("> pwd", 4, &mut s).unwrap();
        assert_eq!(take_command(&s, "run-3").unwrap_err(), "unknown run id");
        assert_eq!(take_command(&s, "run-4").unwrap(), "pwd");
        assert!(take_command(&s, "nope").is_err());
    }

    #[test]
    fn nul_and_oversize_rejected_and_title_truncated() {
        let mut s = RunState::default();
        assert!(run_item("> a\0b", 1, &mut s).is_none());
        assert!(run_item(&format!(">{}", "x".repeat(4097)), 1, &mut s).is_none());
        assert!(run_item(&format!(">{}", "x".repeat(4096)), 1, &mut s).is_some());
        let long = format!("> {}", "é".repeat(90));
        let it = run_item(&long, 2, &mut s).unwrap();
        assert_eq!(it.title, format!("Run \"{}…\"", "é".repeat(80)));
    }

    #[test]
    fn late_older_generation_cannot_overwrite() {
        let mut s = RunState::default();
        run_item("> pwd", 5, &mut s).unwrap();
        assert!(run_item("> ls", 4, &mut s).is_none());
        assert!(take_command(&s, "run-4").is_err());
        assert_eq!(take_command(&s, "run-5").unwrap(), "pwd");
    }

    #[test]
    fn search_without_item_clears_stored_command() {
        let mut s = RunState::default();
        run_item("> ls", 3, &mut s).unwrap();
        assert!(run_item("firefox", 4, &mut s).is_none());
        assert!(take_command(&s, "run-3").is_err());
        assert!(take_command(&s, "run-4").is_err());
        assert!(s.command.is_none());
        assert!(run_item(">", 5, &mut s).is_none());
        assert!(s.command.is_none());
        assert!(take_command(&s, "run-5").is_err());
    }

    #[test]
    fn launch_run_id_wraps_sh_c_in_scope() {
        let argv = ["sh", "-c", "echo hi"].map(String::from);
        assert_eq!(
            crate::bridge::launch::scope_command("run", &argv, 0xff),
            "systemd-run --user --scope --collect --unit=app-uq-run-000000ff -- 'sh' '-c' 'echo hi'"
        );
    }
}
