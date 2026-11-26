//! Settings history: the journal, the restore points and the app's own undo and redo.
//! [`HistoryState`] talks to uqd (`org.uq.Desktop1`) and is plain Rust, tested against the fake
//! uqd; [`UndoStack`] is the app-level undo and redo. `crate::history_model` is the thin QML
//! object around them.
//!
//! Undo and redo only ever reach this app's own changes (origin `settings`, made since the app
//! started) and what its own undo and redo wrote. A change from the terminal, the panel or an
//! earlier session is never undone by them; the History page and the "Changed this session" sheet
//! undo such an entry only when the user picks it.

use std::collections::HashSet;
use std::sync::Mutex as StdMutex;

use serde_json::{Value, json};
use tokio::sync::Mutex;
use uq_core::client::DesktopProxy;
use uq_core::history::{AutoReason, Entry, MAX_ENTRIES, PointKind, RestorePoint};

use crate::pages::runner::{ORIGIN, PageLogic, parse_json};

/// The error name uqd's `Undo` answers when the entry's domain changed again since.
const CONFLICT: &str = "org.uq.Desktop1.Error.Conflict";
/// After a restore: how to go back, naming the kind line HistoryPage.qml shows on that point.
const RESTORED_WAY_BACK: &str = "Settings restored. To go back, restore the top restore point, \
     \u{201c}Saved automatically before a restore\u{201d}, on the History page.";

/// A restore point's name is cut to this many characters (by uqd too).
pub const MAX_NAME_CHARS: usize = 100;

/// What the page and the window ask for.
#[derive(Debug, Clone)]
pub enum Op {
    /// Just read everything.
    Refresh,
    /// Undo one entry. `force` overrides a conflict (a newer change to the same domain).
    Undo {
        id: String,
        force: bool,
    },
    /// Undo this app's latest change (Ctrl+Z).
    UndoLast,
    /// Undo the latest undo (Ctrl+Shift+Z).
    RedoLast,
    SaveRestorePoint {
        name: String,
    },
    DeleteRestorePoint {
        id: String,
    },
    Restore {
        id: String,
    },
}

/// The app-level undo and redo, rebuilt from the journal on every read.
#[derive(Debug, Default)]
pub struct UndoStack {
    /// Unix seconds the app started at.
    session_start: i64,
    /// What Ctrl+Z undoes, newest last: this session's `settings` entries and what redo wrote,
    /// less what has been undone.
    pub own: Vec<String>,
    /// What Ctrl+Shift+Z undoes, newest last: the entries this app's undo wrote.
    pub redo: Vec<String>,
    /// Every entry this app's Ctrl+Z and Ctrl+Shift+Z steps wrote (origin `restore`): undos of
    /// `own` entries and redos. An undo of another change, picked from a list, isn't one.
    made: HashSet<String>,
    /// The entries redo wrote: undoable again, like `settings` ones.
    redone: HashSet<String>,
    /// Own entries that have been undone.
    undone: HashSet<String>,
    /// The session's `settings` entries seen so far: a new one clears `redo`.
    seen: HashSet<String>,
}

impl UndoStack {
    pub fn new(session_start: i64) -> Self {
        Self {
            session_start,
            ..Self::default()
        }
    }

    fn is_own_setting(&self, e: &Entry) -> bool {
        e.origin == ORIGIN && e.time >= self.session_start
    }

    /// Whether this app wrote `e`: its own settings change, or a step of its undo or redo.
    fn is_ours(&self, e: &Entry) -> bool {
        self.is_own_setting(e) || self.made.contains(&e.id)
    }

    /// Rebuild `own` from the journal (`newest_first`, as uqd lists it). A `settings` entry not
    /// seen before is a new change, which clears `redo`; undo and redo write `restore` entries,
    /// so they never do.
    pub fn sync(&mut self, newest_first: &[Entry]) {
        let start = self.session_start;
        let mut fresh = false;
        for e in newest_first
            .iter()
            .filter(|e| e.origin == ORIGIN && e.time >= start)
        {
            fresh |= self.seen.insert(e.id.clone());
        }
        if fresh {
            self.redo.clear();
        }
        self.own = newest_first
            .iter()
            .rev()
            .filter(|e| self.is_own_setting(e) || self.redone.contains(&e.id))
            .filter(|e| !self.undone.contains(&e.id))
            .map(|e| e.id.clone())
            .collect();
    }

    /// Whether undoing `id` may skip uqd's conflict check: every newer change to its domain was
    /// written by this app (an earlier undo, say), so nobody else's change is overwritten.
    pub fn only_ours_since(&self, id: &str, newest_first: &[Entry]) -> bool {
        let Some(pos) = newest_first.iter().position(|e| e.id == id) else {
            return false;
        };
        let domain = &newest_first[pos].domain;
        newest_first[..pos]
            .iter()
            .filter(|e| &e.domain == domain)
            .all(|e| self.is_ours(e))
    }

    /// `target` was undone and uqd journalled `result` ("" when nothing changed). Undoing a redo
    /// entry is a redo: its result is undoable again. Undoing an own entry moves it to redo.
    pub fn record(&mut self, target: &str, result: &str) {
        let has_result = !result.is_empty();
        if let Some(i) = self.redo.iter().position(|r| r == target) {
            self.redo.remove(i);
            if has_result {
                self.made.insert(result.to_string());
                self.redone.insert(result.to_string());
                self.own.push(result.to_string());
            }
        } else if let Some(i) = self.own.iter().position(|o| o == target) {
            self.own.remove(i);
            self.undone.insert(target.to_string());
            if has_result {
                self.made.insert(result.to_string());
                self.redo.push(result.to_string());
            }
        }
    }
}

/// The last confirmation to show, numbered so the page shows each once.
#[derive(Debug, Default, Clone)]
struct Notice {
    seq: u64,
    text: String,
}

/// An undo uqd refused because the domain changed again since: the window asks to force it.
#[derive(Debug, Clone)]
struct Conflict {
    seq: u64,
    id: String,
    /// Whether it was a redo (undoing an undo), so the window says "Redo".
    redo: bool,
    /// uqd's message without its closing "Undo anyway?": the window asks the question.
    message: String,
}

/// uqd's conflict message without the question it ends with.
fn conflict_message(text: &str) -> String {
    let text = text.trim();
    text.strip_suffix("Undo anyway?")
        .map_or(text, str::trim_end)
        .to_string()
}

#[derive(Debug, Default)]
struct Shown {
    seq: u64,
    notice: Notice,
    conflict: Option<Conflict>,
}

impl Shown {
    fn notice(&mut self, text: &str) {
        self.seq += 1;
        self.notice = Notice {
            seq: self.seq,
            text: text.to_string(),
        };
    }
}

pub struct HistoryState {
    conn: zbus::Connection,
    session_start: i64,
    /// One operation at a time, so undo and redo see each other's results.
    write: Mutex<()>,
    stack: StdMutex<UndoStack>,
    shown: StdMutex<Shown>,
}

impl HistoryState {
    pub fn new(conn: zbus::Connection, session_start: i64) -> Self {
        Self {
            conn,
            session_start,
            write: Mutex::new(()),
            stack: StdMutex::new(UndoStack::new(session_start)),
            shown: StdMutex::new(Shown::default()),
        }
    }

    /// `{"<profile id>": "<name>"}`, so an entry from `profile:<id>` can say which profile. A
    /// profile list that can't be read gives `{}`: entries then name the id.
    async fn profile_names(proxy: &DesktopProxy<'_>) -> Value {
        let list = match proxy.list_profiles().await.and_then(|t| parse_json(&t)) {
            Ok(Value::Array(list)) => list,
            _ => return json!({}),
        };
        let names = list
            .iter()
            .filter_map(|p| Some((p["id"].as_str()?, p["name"].as_str()?)));
        Value::Object(
            names
                .map(|(id, name)| (id.to_string(), json!(name)))
                .collect(),
        )
    }

    async fn entries(proxy: &DesktopProxy<'_>) -> zbus::Result<Vec<Entry>> {
        let text = proxy.list_history(MAX_ENTRIES as u32).await?;
        serde_json::from_str(&text).map_err(|e| {
            zbus::Error::Failure(format!(
                "uQ's settings service sent unreadable history: {e}"
            ))
        })
    }

    async fn points(proxy: &DesktopProxy<'_>) -> zbus::Result<Vec<RestorePoint>> {
        let text = proxy.list_restore_points().await?;
        serde_json::from_str(&text).map_err(|e| {
            zbus::Error::Failure(format!(
                "uQ's settings service sent unreadable restore points: {e}"
            ))
        })
    }

    async fn point(proxy: &DesktopProxy<'_>, id: &str) -> zbus::Result<RestorePoint> {
        Self::points(proxy)
            .await?
            .into_iter()
            .find(|p| p.id == id)
            .ok_or_else(|| zbus::Error::Failure(format!("no restore point {id}")))
    }

    /// Undo `target` and record the result in the stack. A conflict is kept for the window to ask
    /// about, not returned as an error.
    async fn undo(
        &self,
        proxy: &DesktopProxy<'_>,
        target: &str,
        force: bool,
        done: &str,
    ) -> zbus::Result<()> {
        match proxy.undo(target, force).await {
            Ok(result) => {
                self.stack.lock().unwrap().record(target, &result);
                self.shown.lock().unwrap().notice(done);
                Ok(())
            }
            Err(zbus::Error::MethodError(name, text, _)) if name.as_str() == CONFLICT => {
                let redo = self.stack.lock().unwrap().redo.iter().any(|r| r == target);
                let mut shown = self.shown.lock().unwrap();
                shown.seq += 1;
                shown.conflict = Some(Conflict {
                    seq: shown.seq,
                    id: target.to_string(),
                    redo,
                    message: conflict_message(&text.unwrap_or_default()),
                });
                Ok(())
            }
            Err(e) => Err(e),
        }
    }
}

/// The state is `{entries, points, session_start, canUndo, canRedo, notice: {seq, text},
/// conflict: {seq, id, redo, message} | null}`; entries and points newest first, as uqd lists them.
impl PageLogic for HistoryState {
    type Op = Op;

    fn refresh() -> Op {
        Op::Refresh
    }

    fn is_refresh(op: &Op) -> bool {
        matches!(op, Op::Refresh)
    }

    fn write_lock(&self) -> &Mutex<()> {
        &self.write
    }

    async fn apply(&self, op: Op) -> zbus::Result<()> {
        if matches!(op, Op::Refresh) {
            return Ok(());
        }
        self.shown.lock().unwrap().conflict = None;
        let proxy = DesktopProxy::new(&self.conn).await?;
        match op {
            Op::Refresh => Ok(()),
            Op::Undo { .. } | Op::UndoLast | Op::RedoLast => {
                // Read the journal first: a change made a moment ago may not be in `own` yet.
                let entries = Self::entries(&proxy).await?;
                let next = {
                    let mut stack = self.stack.lock().unwrap();
                    stack.sync(&entries);
                    // Ctrl+Z and Ctrl+Shift+Z step through the app's own changes, so their
                    // own earlier undos and redos of the domain don't count as a conflict. A
                    // pick from the list gets uqd's check unless the user forces it.
                    let stepped = |id: Option<&String>| {
                        id.map(|id| (id.clone(), stack.only_ours_since(id, &entries)))
                    };
                    match &op {
                        Op::Undo { id, force } => Some((id.clone(), *force)),
                        Op::RedoLast => stepped(stack.redo.last()),
                        _ => stepped(stack.own.last()),
                    }
                    .map(|(id, force)| {
                        let done = if stack.redo.contains(&id) {
                            "Change redone"
                        } else {
                            "Change undone"
                        };
                        (id, force, done)
                    })
                };
                let Some((id, force, done)) = next else {
                    let nothing = if matches!(op, Op::RedoLast) {
                        "Nothing to redo"
                    } else {
                        "Nothing to undo"
                    };
                    self.shown.lock().unwrap().notice(nothing);
                    return Ok(());
                };
                self.undo(&proxy, &id, force, done).await
            }
            Op::SaveRestorePoint { name } => {
                let name: String = name.trim().chars().take(MAX_NAME_CHARS).collect();
                if name.is_empty() {
                    return Err(zbus::Error::Failure("give the restore point a name".into()));
                }
                proxy.save_restore_point(&name).await?;
                self.shown.lock().unwrap().notice("Restore point saved");
                Ok(())
            }
            Op::DeleteRestorePoint { id } => {
                let point = Self::point(&proxy, &id).await?;
                if point.kind != PointKind::Manual {
                    return Err(zbus::Error::Failure(
                        "automatic restore points are removed by uQ itself".into(),
                    ));
                }
                proxy.delete_restore_point(&id).await?;
                self.shown.lock().unwrap().notice("Restore point deleted");
                Ok(())
            }
            Op::Restore { id } => {
                Self::point(&proxy, &id).await?;
                proxy.restore(&id).await?;
                // A restore is undone by restoring the point uqd saved just before it, the
                // newest one; Ctrl+Z doesn't reach it. Point there only if uqd saved it. The
                // toast isn't a UqLabel, so it says the fixed kind line the page shows, never
                // the point's name.
                let saved = Self::points(&proxy).await.ok().is_some_and(|points| {
                    points
                        .first()
                        .is_some_and(|p| p.kind == PointKind::Auto(AutoReason::BeforeRestore))
                });
                self.shown.lock().unwrap().notice(if saved {
                    RESTORED_WAY_BACK
                } else {
                    "Settings restored"
                });
                Ok(())
            }
        }
    }

    async fn snapshot(&self) -> zbus::Result<Value> {
        let proxy = DesktopProxy::new(&self.conn).await?;
        let entries = Self::entries(&proxy).await?;
        let points = parse_json(&proxy.list_restore_points().await?)?;
        let (can_undo, can_redo) = {
            let mut stack = self.stack.lock().unwrap();
            stack.sync(&entries);
            (!stack.own.is_empty(), !stack.redo.is_empty())
        };
        let profiles = Self::profile_names(&proxy).await;
        let shown = self.shown.lock().unwrap();
        Ok(json!({
            "entries": entries,
            "profiles": profiles,
            "points": points,
            "session_start": self.session_start,
            "canUndo": can_undo,
            "canRedo": can_redo,
            "notice": {"seq": shown.notice.seq, "text": shown.notice.text},
            "conflict": shown.conflict.as_ref().map(|c| json!({
                "seq": c.seq,
                "id": c.id,
                "redo": c.redo,
                "message": c.message,
            })),
        }))
    }
}

#[cfg(test)]
mod tests;
