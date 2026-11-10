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
        let shown = self.shown.lock().unwrap();
        Ok(json!({
            "entries": entries,
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
mod tests {
    use super::*;
    use crate::pages::fake_uqd::{self, Rig, now};
    use crate::pages::runner::{Outcome, run};
    use uq_core::state::State;
    use uq_core::test_bus::{connect, private_bus};

    /// A rig whose app started a minute ago.
    async fn rig() -> (Rig, HistoryState) {
        let r = fake_uqd::rig(State::default()).await;
        let page = HistoryState::new(r.client.clone(), now() - 60);
        (r, page)
    }

    /// Write `domain`'s `field` the way `origin` would.
    async fn write(r: &Rig, origin: &str, domain: &str, field: &str, value: Value) -> String {
        let proxy = DesktopProxy::new(&r.client).await.unwrap();
        let mut section = parse_json(&proxy.get_section(domain).await.unwrap()).unwrap();
        section[field] = value;
        proxy
            .set_section(domain, &section.to_string(), origin)
            .await
            .unwrap();
        r.history.last_id()
    }

    fn section(r: &Rig, domain: &str) -> Value {
        r.state.lock().unwrap().section(domain).unwrap()
    }

    #[tokio::test]
    async fn undo_last_undoes_only_own_entries() {
        let (r, page) = rig().await;
        // Before this session, from the app itself: not ours to undo any more.
        r.history.seed(Entry::new(
            now() - 3600,
            "settings",
            "wallpaper",
            "Wallpaper changed",
            json!({}),
            json!({}),
        ));
        write(&r, "uqctl", "look", "rounding", json!(4)).await;
        let own = write(&r, ORIGIN, "appearance", "font_size", json!(12)).await;
        write(&r, "shell", "shell", "clock_24h", json!(true)).await;

        let out = run(&page, Op::UndoLast).await;
        assert_eq!(out.error, None);
        assert_eq!(out.state["notice"]["text"], "Change undone");
        assert_eq!(section(&r, "appearance")["font_size"], Value::Null);
        assert_eq!(
            section(&r, "look")["rounding"],
            4,
            "the terminal's change stays"
        );
        assert_eq!(
            section(&r, "shell")["clock_24h"],
            true,
            "the panel's change stays"
        );
        let undo = r.history.entries.lock().unwrap().last().cloned().unwrap();
        assert_eq!(
            (undo.origin.as_str(), undo.domain.as_str()),
            ("restore", "appearance")
        );
        assert_eq!(undo.before["font_size"], 12, "it undid {own}");

        // Nothing of ours is left: Ctrl+Z does nothing, however many foreign entries remain.
        let journal = r.history.origins().len();
        let out = run(&page, Op::UndoLast).await;
        assert_eq!(out.error, None);
        assert_eq!(out.state["notice"]["text"], "Nothing to undo");
        assert_eq!(
            r.history.origins().len(),
            journal,
            "no further undo was made"
        );
        assert_eq!(section(&r, "look")["rounding"], 4);
        assert_eq!(section(&r, "shell")["clock_24h"], true);
        assert_eq!(out.state["canUndo"], false);
        assert_eq!(out.state["canRedo"], true);
    }

    #[tokio::test]
    async fn undo_last_steps_back_through_own_changes_to_one_domain() {
        let (r, page) = rig().await;
        write(&r, ORIGIN, "look", "rounding", json!(4)).await;
        write(&r, ORIGIN, "look", "rounding", json!(8)).await;
        assert_eq!(run(&page, Op::UndoLast).await.error, None);
        assert_eq!(section(&r, "look")["rounding"], 4);
        // The undo just written changed `look` again, but it was ours: no conflict to ask about.
        let out = run(&page, Op::UndoLast).await;
        assert_eq!((out.error, &out.state["conflict"]), (None, &Value::Null));
        assert_eq!(section(&r, "look")["rounding"], Value::Null);
    }

    #[tokio::test]
    async fn redo_reapplies_after_undo() {
        let (r, page) = rig().await;
        write(&r, ORIGIN, "look", "rounding", json!(4)).await;
        write(&r, ORIGIN, "look", "rounding", json!(8)).await;
        run(&page, Op::UndoLast).await;
        run(&page, Op::UndoLast).await;
        assert_eq!(section(&r, "look")["rounding"], Value::Null);

        let out = run(&page, Op::RedoLast).await;
        assert_eq!(out.error, None);
        assert_eq!(out.state["notice"]["text"], "Change redone");
        assert_eq!(section(&r, "look")["rounding"], 4);
        let out = run(&page, Op::RedoLast).await;
        assert_eq!((out.error, &out.state["conflict"]), (None, &Value::Null));
        assert_eq!(section(&r, "look")["rounding"], 8);
        assert_eq!(out.state["canRedo"], false);
        assert_eq!(
            run(&page, Op::RedoLast).await.state["notice"]["text"],
            "Nothing to redo"
        );

        // A redone change is undoable again.
        run(&page, Op::UndoLast).await;
        assert_eq!(section(&r, "look")["rounding"], 4);
        run(&page, Op::RedoLast).await;
        assert_eq!(section(&r, "look")["rounding"], 8);
    }

    #[tokio::test]
    async fn new_change_clears_redo() {
        let (r, page) = rig().await;
        write(&r, ORIGIN, "look", "rounding", json!(4)).await;
        let out = run(&page, Op::UndoLast).await;
        assert_eq!(out.state["canRedo"], true);

        // Another app's change keeps redo; a new change of the app's own drops it.
        write(&r, "uqctl", "appearance", "ui_font", json!("Inter")).await;
        assert_eq!(run(&page, Op::Refresh).await.state["canRedo"], true);
        write(&r, ORIGIN, "appearance", "font_size", json!(12)).await;
        let out = run(&page, Op::RedoLast).await;
        assert_eq!(out.state["notice"]["text"], "Nothing to redo");
        assert_eq!(out.state["canRedo"], false);
        assert_eq!(section(&r, "look")["rounding"], Value::Null, "not redone");
        // Ctrl+Z now undoes the new change.
        run(&page, Op::UndoLast).await;
        assert_eq!(section(&r, "appearance")["font_size"], Value::Null);
    }

    #[tokio::test]
    async fn conflict_surfaces_as_error_then_force_applies() {
        let (r, page) = rig().await;
        let own = write(&r, ORIGIN, "look", "rounding", json!(4)).await;
        write(&r, "uqctl", "look", "border_width", json!(3)).await;

        let out = run(&page, Op::UndoLast).await;
        assert_eq!(
            out.error, None,
            "the window asks instead of showing an error"
        );
        let conflict = &out.state["conflict"];
        assert_eq!(conflict["id"], own.as_str());
        assert_eq!(conflict["redo"], false);
        // The window asks the question; uqd's own "Undo anyway?" would ask it twice.
        assert_eq!(conflict["message"], "look was changed again after this.");
        assert_eq!(section(&r, "look")["rounding"], 4, "nothing undone yet");

        // "Undo anyway".
        let out = run(
            &page,
            Op::Undo {
                id: own,
                force: true,
            },
        )
        .await;
        assert_eq!((out.error, &out.state["conflict"]), (None, &Value::Null));
        assert_eq!(out.state["notice"]["text"], "Change undone");
        let look = section(&r, "look");
        assert_eq!(look["rounding"], Value::Null);
        assert_eq!(
            look["border_width"], 3,
            "only the undone key goes back, as uqd does; the other change stays"
        );
        assert_eq!(out.state["canRedo"], true, "a forced undo can be redone");
    }

    #[tokio::test]
    async fn redo_conflict_says_redo() {
        let (r, page) = rig().await;
        write(&r, ORIGIN, "look", "rounding", json!(4)).await;
        let undo = {
            run(&page, Op::UndoLast).await;
            r.history.last_id()
        };
        write(&r, "uqctl", "look", "border_width", json!(3)).await;
        let out = run(&page, Op::RedoLast).await;
        let conflict = &out.state["conflict"];
        assert_eq!(conflict["id"], undo.as_str());
        assert_eq!(conflict["redo"], true);
        assert_eq!(conflict["message"], "look was changed again after this.");
        assert_eq!(
            section(&r, "look")["rounding"],
            Value::Null,
            "not redone yet"
        );
    }

    #[tokio::test]
    async fn picked_undo_of_another_change_is_not_ours() {
        let (r, page) = rig().await;
        let other = write(&r, "uqctl", "look", "gaps_in", json!(6)).await;
        let own = write(&r, ORIGIN, "look", "rounding", json!(4)).await;
        // The user picks the terminal's change from the list and forces its undo.
        let out = run(
            &page,
            Op::Undo {
                id: other,
                force: true,
            },
        )
        .await;
        assert_eq!(out.error, None);
        // That undo wrote `look` after the app's own change, and it was not a Ctrl+Z step: Ctrl+Z
        // must ask, not silently write back a section that still holds the terminal's change.
        let out = run(&page, Op::UndoLast).await;
        assert_eq!(out.state["conflict"]["id"], own.as_str());
        assert_eq!(out.state["conflict"]["redo"], false);
    }

    #[tokio::test]
    async fn picked_undo_keeps_uqds_check_and_joins_the_stack() {
        let (r, page) = rig().await;
        let first = write(&r, ORIGIN, "look", "rounding", json!(4)).await;
        let second = write(&r, ORIGIN, "look", "gaps_in", json!(6)).await;

        // Picking the older one: the newer own change to `look` is still live, so uqd asks.
        let out = run(
            &page,
            Op::Undo {
                id: first,
                force: false,
            },
        )
        .await;
        assert_eq!(
            out.state["conflict"]["message"],
            "look was changed again after this."
        );
        assert_eq!(section(&r, "look")["gaps_in"], 6);

        // Picking the newest from the list counts like Ctrl+Z: it can be redone, and Ctrl+Z
        // goes on to the one before.
        let out = run(
            &page,
            Op::Undo {
                id: second,
                force: false,
            },
        )
        .await;
        assert_eq!((out.error, &out.state["conflict"]), (None, &Value::Null));
        assert_eq!(out.state["canRedo"], true);
        assert_eq!(section(&r, "look")["gaps_in"], Value::Null);
        run(&page, Op::UndoLast).await;
        assert_eq!(section(&r, "look")["rounding"], Value::Null);
        run(&page, Op::RedoLast).await;
        run(&page, Op::RedoLast).await;
        let look = section(&r, "look");
        assert_eq!(
            (&look["rounding"], &look["gaps_in"]),
            (&json!(4), &json!(6))
        );
    }

    #[tokio::test]
    async fn restore_requires_existing_id() {
        let (r, page) = rig().await;
        let saved = State {
            theme: "dracula".into(),
            ..State::default()
        };
        let auto = r.history.add_point(
            "Before today's changes",
            PointKind::Auto(uq_core::history::AutoReason::Daily),
            saved,
        );

        let out = run(
            &page,
            Op::Restore {
                id: "1700000000-abcdef".into(),
            },
        )
        .await;
        assert_eq!(
            out.error.as_deref(),
            Some("no restore point 1700000000-abcdef")
        );
        assert!(r.history.origins().is_empty(), "nothing was restored");

        let out = run(&page, Op::Restore { id: auto.clone() }).await;
        assert_eq!(out.error, None);
        // The way back is the point uqd saved first: newest, so at the top of the list.
        assert_eq!(
            out.state["notice"]["text"],
            "Settings restored. To go back, restore the top restore point, \u{201c}Saved \
             automatically before a restore\u{201d}, on the History page."
        );
        assert_eq!(
            out.state["points"][0]["kind"],
            json!({"auto": "before_restore"})
        );
        assert_eq!(r.state.lock().unwrap().theme, "dracula");
        assert_eq!(r.history.origins(), ["restore"]);

        // Restoring is not one of the app's own changes: Ctrl+Z leaves it.
        let out = run(&page, Op::UndoLast).await;
        assert_eq!(out.state["notice"]["text"], "Nothing to undo");
        assert_eq!(r.state.lock().unwrap().theme, "dracula");
    }

    #[tokio::test]
    async fn restore_points_save_and_delete_manual_only() {
        let (r, page) = rig().await;
        let auto = r.history.add_point(
            "0.9.0",
            PointKind::Auto(uq_core::history::AutoReason::NewVersion),
            State::default(),
        );
        let out = run(
            &page,
            Op::SaveRestorePoint {
                name: "  Before the party  ".into(),
            },
        )
        .await;
        assert_eq!(out.error, None);
        assert_eq!(out.state["notice"]["text"], "Restore point saved");
        let points = out.state["points"].as_array().unwrap().clone();
        assert_eq!(points.len(), 2);
        assert_eq!(points[0]["name"], "Before the party");
        assert_eq!(points[0]["kind"], "manual");

        let out = run(&page, Op::SaveRestorePoint { name: " ".into() }).await;
        assert_eq!(out.error.as_deref(), Some("give the restore point a name"));
        let long = "x".repeat(150);
        run(&page, Op::SaveRestorePoint { name: long }).await;

        let out = run(&page, Op::DeleteRestorePoint { id: auto.clone() }).await;
        assert_eq!(
            out.error.as_deref(),
            Some("automatic restore points are removed by uQ itself")
        );
        let manual = points[0]["id"].as_str().unwrap().to_string();
        let out = run(&page, Op::DeleteRestorePoint { id: manual }).await;
        assert_eq!(out.error, None);
        assert_eq!(out.state["notice"]["text"], "Restore point deleted");
        let names: Vec<_> = out.state["points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["name"].as_str().unwrap().chars().count())
            .collect();
        assert_eq!(
            names,
            [100, 5],
            "the long name is cut; the auto point stays"
        );
    }

    #[tokio::test]
    async fn snapshot_lists_entries_and_session_start() {
        let (r, page) = rig().await;
        let id = write(&r, "uqctl", "look", "rounding", json!(4)).await;
        let out = run(&page, Op::Refresh).await;
        assert_eq!(out.state["session_start"], page.session_start);
        assert_eq!(out.state["entries"][0]["id"], id.as_str());
        assert_eq!(out.state["entries"][0]["origin"], "uqctl");
        assert_eq!(out.state["entries"][0]["domain"], "look");
        assert_eq!(out.state["notice"], json!({"seq": 0, "text": ""}));
        assert_eq!(out.state["canUndo"], false);
    }

    #[tokio::test]
    async fn uqd_down_gives_empty_state_without_an_error() {
        let bus = private_bus();
        let page = HistoryState::new(connect(&bus).await, 0);
        assert_eq!(
            run(&page, Op::UndoLast).await,
            Outcome {
                state: json!({}),
                error: None
            }
        );
    }
}
