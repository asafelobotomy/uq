//! The theme row's logic on the Appearance page: [`ThemesState`] talks to uqd
//! (`org.uq.Desktop1`) and is plain Rust, tested against the fake uqd. `crate::themes_model` is
//! the thin QML object around it.
//!
//! A theme is added from a `.uqtheme` file, saved to one, removed, made from another theme in
//! the editor (`editor`), or switched to. uqd keeps the files and applies the theme; every save,
//! whether picked from a file or made in the editor, goes through its `ImportTheme`, so every
//! theme gets the same checks.

use std::path::PathBuf;
use std::sync::Mutex as StdMutex;

use serde_json::{Value, json};
use tokio::sync::Mutex;
use uq_core::atomic::atomic_write;
use uq_core::client::DesktopProxy;
use uq_core::theme::uqtheme;

use crate::pages::runner::{ORIGIN, PageLogic, parse_json};

pub mod editor;
use editor::Session;

/// The error name uqd's `ImportTheme` answers when the id exists and `replace` is false.
const CONFLICT: &str = "org.uq.Desktop1.Error.Conflict";

#[derive(Debug, Clone)]
pub enum Op {
    Refresh,
    /// Add the theme in the `.uqtheme` file at `path`; `replace` answers a conflict, and
    /// `with_look` also uses the file's window gaps, corners and effects.
    Import {
        path: PathBuf,
        replace: bool,
        with_look: bool,
    },
    Export {
        id: String,
        path: PathBuf,
        with_look: bool,
    },
    Delete {
        id: String,
    },
    /// Open the editor on a copy of `from`, or (`edit`) on the user's theme `from` itself.
    OpenEditor {
        from: String,
        edit: bool,
    },
    /// Save the editor's draft `{name, author, dark: {bg…bad}, light: {…}}`.
    Save {
        draft: Value,
    },
    CloseEditor,
    /// Switch to `id`, keeping the current variant.
    Use {
        id: String,
    },
}

/// The last confirmation to show, numbered so the page shows each once.
#[derive(Debug, Default)]
struct Notice {
    seq: u64,
    text: String,
}

/// An import uqd refused because the theme exists: the page asks to replace it.
#[derive(Debug)]
struct Conflict {
    seq: u64,
    path: String,
    /// uqd's message without its closing "Replace it?": the page's dialog asks the question.
    message: String,
}

#[derive(Debug)]
struct Deleted {
    seq: u64,
    id: String,
    switched: bool,
    profiles: Vec<String>,
}

#[derive(Debug)]
struct Imported {
    seq: u64,
    id: String,
    name: String,
    /// The file had a window look that wasn't used (the box was off).
    unused_look: bool,
}

#[derive(Debug, Default)]
struct Shown {
    seq: u64,
    notice: Notice,
    conflict: Option<Conflict>,
    deleted: Option<Deleted>,
    imported: Option<Imported>,
}

impl Shown {
    fn next(&mut self) -> u64 {
        self.seq += 1;
        self.seq
    }
}

/// uqd's conflict message without the question it ends with.
fn conflict_message(text: &str) -> String {
    let text = text.trim();
    text.strip_suffix("Replace it?")
        .map_or(text, str::trim_end)
        .to_string()
}

fn failure(text: impl Into<String>) -> zbus::Error {
    zbus::Error::Failure(text.into())
}

pub struct ThemesState {
    conn: zbus::Connection,
    /// An import or a delete changes what the next one sees, so one at a time.
    write: Mutex<()>,
    session: StdMutex<Option<Session>>,
    shown: StdMutex<Shown>,
}

impl ThemesState {
    pub fn new(conn: zbus::Connection) -> Self {
        Self {
            conn,
            write: Mutex::new(()),
            session: StdMutex::new(None),
            shown: StdMutex::new(Shown::default()),
        }
    }

    /// The editor's live preview of `draft` (see [`editor::preview`]). Synchronous: it asks uqd
    /// nothing.
    pub fn preview(&self, draft: &Value) -> Value {
        match &*self.session.lock().unwrap() {
            Some(session) => editor::preview(session, draft),
            None => json!({
                "dark": {"tokens": {}, "problems": []},
                "light": {"tokens": {}, "problems": []},
                "ok": false,
                "nameProblem": "",
                "authorProblem": "",
            }),
        }
    }

    fn notice(&self, text: &str) {
        let mut shown = self.shown.lock().unwrap();
        let seq = shown.next();
        shown.notice = Notice {
            seq,
            text: text.to_string(),
        };
    }

    async fn info(proxy: &DesktopProxy<'_>) -> zbus::Result<Value> {
        parse_json(&proxy.list_theme_info().await?)
    }

    /// The ids of the user's themes.
    async fn user_ids(proxy: &DesktopProxy<'_>) -> zbus::Result<Vec<String>> {
        Ok(Self::info(proxy).await?["themes"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|t| t["user"] == true)
            .filter_map(|t| t["id"].as_str().map(str::to_string))
            .collect())
    }

    async fn import(
        &self,
        proxy: &DesktopProxy<'_>,
        path: PathBuf,
        replace: bool,
        with_look: bool,
    ) -> zbus::Result<()> {
        // The size and kind of the file are checked here, before uqd is asked anything.
        let text = uqtheme::read_file(&path).map_err(|e| failure(e.to_string()))?;
        match proxy.import_theme(&text, replace, with_look, ORIGIN).await {
            Ok(id) => {
                let parsed = uqtheme::parse(&text).ok();
                let mut shown = self.shown.lock().unwrap();
                let seq = shown.next();
                shown.imported = Some(Imported {
                    seq,
                    id,
                    name: parsed.as_ref().map(|t| t.name.clone()).unwrap_or_default(),
                    unused_look: !with_look && parsed.is_some_and(|t| t.look.is_some()),
                });
                Ok(())
            }
            Err(zbus::Error::MethodError(name, text, _)) if name.as_str() == CONFLICT => {
                let mut shown = self.shown.lock().unwrap();
                let seq = shown.next();
                shown.conflict = Some(Conflict {
                    seq,
                    path: path.display().to_string(),
                    message: conflict_message(&text.unwrap_or_default()),
                });
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    async fn open_editor(
        &self,
        proxy: &DesktopProxy<'_>,
        from: &str,
        edit: bool,
    ) -> zbus::Result<()> {
        if edit && !Self::user_ids(proxy).await?.iter().any(|id| id == from) {
            return Err(failure("Only your own themes can be edited."));
        }
        let text = proxy.export_theme(from, false).await?;
        let parsed = uqtheme::parse(&text).map_err(|e| failure(e.to_string()))?;
        *self.session.lock().unwrap() = Some(Session::new(from, edit, &parsed));
        Ok(())
    }

    async fn save(&self, proxy: &DesktopProxy<'_>, draft: &Value) -> zbus::Result<()> {
        let session = self.session.lock().unwrap().clone();
        let Some(session) = session else {
            return Err(failure("There's no theme to save."));
        };
        let taken = Self::user_ids(proxy).await?;
        let (text, replace) = editor::to_file(&session, draft, &taken).map_err(failure)?;
        let id = proxy.import_theme(&text, replace, false, ORIGIN).await?;
        *self.session.lock().unwrap() = None;
        if replace {
            self.notice("Saved your changes.");
        } else {
            let name = uqtheme::parse(&text).map(|t| t.name).unwrap_or_default();
            let mut shown = self.shown.lock().unwrap();
            let seq = shown.next();
            shown.imported = Some(Imported {
                seq,
                id,
                name,
                unused_look: false,
            });
        }
        Ok(())
    }
}

/// The state is `{themes: [{id, name, author, user, active, missing}], skipped: [{folder,
/// reason}], rootIsLink, theme, variant, editor, notice: {seq, text}, conflict: {seq, path,
/// message} | null, deleted: {seq, id, switched, profiles} | null, imported: {seq, id, name,
/// unusedLook} | null}`. `editor` is `null` or the open session (see [`Session::json`]).
impl PageLogic for ThemesState {
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
        {
            // What an earlier action said goes away when the person does something else.
            let mut shown = self.shown.lock().unwrap();
            shown.conflict = None;
            shown.deleted = None;
            shown.imported = None;
        }
        // Closing needs no uqd.
        if matches!(op, Op::CloseEditor) {
            *self.session.lock().unwrap() = None;
            return Ok(());
        }
        let proxy = DesktopProxy::new(&self.conn).await?;
        match op {
            Op::Refresh | Op::CloseEditor => Ok(()),
            Op::Import {
                path,
                replace,
                with_look,
            } => self.import(&proxy, path, replace, with_look).await,
            Op::Export {
                id,
                path,
                with_look,
            } => {
                let text = proxy.export_theme(&id, with_look).await?;
                atomic_write(&path, &text)
                    .map_err(|e| failure(format!("Can't save the file: {e}")))?;
                Ok(())
            }
            Op::Delete { id } => {
                let report = parse_json(&proxy.delete_theme(&id, ORIGIN).await?)?;
                let profiles = report["profiles"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|p| p.as_str().map(str::to_string))
                    .collect();
                let mut shown = self.shown.lock().unwrap();
                let seq = shown.next();
                shown.deleted = Some(Deleted {
                    seq,
                    id,
                    switched: report["switched"] == true,
                    profiles,
                });
                Ok(())
            }
            Op::OpenEditor { from, edit } => self.open_editor(&proxy, &from, edit).await,
            Op::Save { draft } => self.save(&proxy, &draft).await,
            Op::Use { id } => {
                let (_, variant) = proxy.get_theme().await?;
                proxy.set_theme(&id, &variant, ORIGIN).await
            }
        }
    }

    async fn snapshot(&self) -> zbus::Result<Value> {
        let proxy = DesktopProxy::new(&self.conn).await?;
        let info = Self::info(&proxy).await?;
        let (theme, variant) = proxy.get_theme().await?;
        let themes: Vec<Value> = info["themes"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|t| {
                json!({
                    "id": t["id"],
                    "name": t["name"],
                    "author": t["author"].as_str().unwrap_or(""),
                    "user": t["user"] == true,
                    "active": t["active"] == true,
                    "missing": t["missing"] == true,
                })
            })
            .collect();
        let skipped: Vec<Value> = info["skipped"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|s| {
                json!({
                    "folder": s["folder"].as_str().unwrap_or(""),
                    "reason": s["reason"].as_str().unwrap_or(""),
                })
            })
            .collect();
        let editor = self.session.lock().unwrap().as_ref().map(Session::json);
        let shown = self.shown.lock().unwrap();
        Ok(json!({
            "themes": themes,
            "skipped": skipped,
            "rootIsLink": info["root_is_link"] == true,
            "theme": theme,
            "variant": variant,
            "editor": editor,
            "notice": {"seq": shown.notice.seq, "text": shown.notice.text},
            "conflict": shown.conflict.as_ref().map(|c| json!({
                "seq": c.seq,
                "path": c.path,
                "message": c.message,
            })),
            "deleted": shown.deleted.as_ref().map(|d| json!({
                "seq": d.seq,
                "id": d.id,
                "switched": d.switched,
                "profiles": d.profiles,
            })),
            "imported": shown.imported.as_ref().map(|i| json!({
                "seq": i.seq,
                "id": i.id,
                "name": i.name,
                "unusedLook": i.unused_look,
            })),
        }))
    }
}

#[cfg(test)]
mod tests;
