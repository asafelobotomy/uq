//! Hidden copies of system entries, apps' own hidden flags, and staggered start delays.

use super::*;

/// The seconds each start waits when apps are `gap` seconds apart: in order of their own delay
/// (ties in list order), each waits for its own delay or `gap` after the one before, whichever
/// is later. With no gap they are the delays as they are.
pub fn staggered(delays: &[u32], gap: u32) -> Vec<u32> {
    let mut order: Vec<usize> = (0..delays.len()).collect();
    order.sort_by_key(|i| (delays[*i], *i));
    let mut out = delays.to_vec();
    let mut last: Option<u32> = None;
    for i in order {
        let at = last.map_or(delays[i], |l| delays[i].max(l.saturating_add(gap)));
        out[i] = at;
        last = Some(at);
    }
    out
}

/// Spread the apps that will run (`Plan::Run`) by the state's gap. The page's own `delay`
/// stays what the person set.
pub(crate) fn apply_stagger(rows: &mut [Resolved], gap: u32) {
    if gap == 0 {
        return;
    }
    let runs: Vec<usize> = (0..rows.len())
        .filter(|i| matches!(rows[*i].plan, Plan::Run { .. }))
        .collect();
    let own: Vec<u32> = runs.iter().map(|i| rows[*i].delay).collect();
    for (i, at) in runs.into_iter().zip(staggered(&own, gap)) {
        if let Plan::Run { delay, .. } = &mut rows[i].plan {
            *delay = at;
        }
    }
}

/// Every found id and every added id.
pub fn known_ids(found: &[Found], state: &Autostart) -> BTreeSet<String> {
    found
        .iter()
        .map(|f| f.id.clone())
        .chain(state.added.iter().map(|a| a.id().to_string()))
        .collect()
}

/// uQ's copy of a system entry that turns it off: the file line for line, with `Hidden=true`
/// and the marker set in `[Desktop Entry]`, under a header.
pub fn hidden_copy(system_text: &str) -> String {
    const ENTRY: &str = "Desktop Entry";
    let text = set_key(system_text, ENTRY, "Hidden", "true");
    let text = set_key(&text, ENTRY, "X-uQ-Autostart", "true");
    format!("{HIDDEN_COPY_HEADER}\n{text}")
}

/// The hidden copies the user folder should hold, as `(id, text)`, and uQ's copies to delete.
/// A system entry that is off in state, with no file of the user's own in its place, gets a
/// copy; a marked copy that isn't wanted (the entry is on again, or its system file is gone) is
/// deleted. A file without the marker is never listed.
pub fn wanted_copies(found: &[Found], state: &Autostart) -> (Vec<(String, String)>, Vec<PathBuf>) {
    let mut write = Vec::new();
    let mut delete = Vec::new();
    for f in found {
        let marked = f.user.as_ref().filter(|(_, t)| is_marked(t));
        if f.user.is_some() && marked.is_none() {
            continue;
        }
        let off = state.choice(&f.id).and_then(|c| c.enabled) == Some(false);
        match (&f.system, marked) {
            (Some((_, text)), _) if off => write.push((f.id.clone(), hidden_copy(text))),
            (_, Some((path, _))) => delete.push(path.clone()),
            _ => {}
        }
    }
    (write, delete)
}

/// Programs that can start hidden on their own: the program's file name and the option that
/// does it. These are the apps people most often start at login; the window watcher still
/// covers whatever the option leaves open.
pub(crate) const OWN_HIDDEN_FLAG: &[(&str, &str)] = &[
    ("steam", "-silent"),
    ("discord", "--start-minimized"),
    ("telegram-desktop", "-startintray"),
    ("signal-desktop", "--start-in-tray"),
    ("element-desktop", "--hidden"),
    ("slack", "--startup"),
    ("nextcloud", "--background"),
    ("syncthing-gtk", "--minimized"),
    ("1password", "--silent"),
];

/// The option the program (by file name, with no path) starts hidden with.
pub fn own_hidden_flag(program: &str) -> Option<&'static str> {
    let name = program.rsplit('/').next().unwrap_or(program);
    OWN_HIDDEN_FLAG
        .iter()
        .find(|(p, _)| *p == name)
        .map(|(_, f)| *f)
}

/// `argv` with the program's own start-hidden option right after the program, unless it is
/// already there. A program that isn't known is left as it is.
pub fn with_hidden_flag(mut argv: Vec<String>) -> Vec<String> {
    if let Some(flag) = argv.first().and_then(|p| own_hidden_flag(p))
        && !argv.iter().any(|a| a == flag)
    {
        argv.insert(1, flag.to_string());
    }
    argv
}
