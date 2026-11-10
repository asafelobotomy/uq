//! The Appearance page's logic: [`AppearanceState`] talks to uqd (`org.uq.Desktop1`) and is plain
//! Rust, tested against the fake uqd. `crate::appearance_model` is the thin QML object around it.

use std::path::PathBuf;

use serde_json::{Value, json};
use tokio::sync::Mutex;
use uq_core::client::DesktopProxy;

use crate::pages::runner::{ORIGIN, PageLogic, parse_json, patch_section, sources};
use crate::system_lists;

/// The `appearance` section's fields that [`Op::SetField`] may change.
const FIELDS: [&str; 7] = [
    "ui_font",
    "mono_font",
    "font_size",
    "cursor_theme",
    "cursor_size",
    "icon_theme",
    "kvantum",
];

/// What the page asks for.
#[derive(Debug, Clone)]
pub enum Op {
    /// Just read everything.
    Refresh,
    SetTheme {
        id: String,
        variant: String,
    },
    SetSchedule(String),
    /// Replace one field of the `appearance` section (`null` = use the theme).
    SetField {
        field: String,
        value: Value,
    },
}

pub struct AppearanceState {
    conn: zbus::Connection,
    /// Writes read-modify-write the section, so one at a time.
    write: Mutex<()>,
    icon_dirs: Vec<PathBuf>,
}

impl AppearanceState {
    pub fn new(conn: zbus::Connection, icon_dirs: Vec<PathBuf>) -> Self {
        Self {
            conn,
            write: Mutex::new(()),
            icon_dirs,
        }
    }
}

/// The state is `{themes, theme, variant, schedule, appearance, cursorThemes, iconThemes,
/// sources}`.
impl PageLogic for AppearanceState {
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
        let proxy = DesktopProxy::new(&self.conn).await?;
        match op {
            Op::Refresh => Ok(()),
            Op::SetTheme { id, variant } => proxy.set_theme(&id, &variant, ORIGIN).await,
            Op::SetSchedule(schedule) => proxy.set_schedule(&schedule, ORIGIN).await,
            Op::SetField { field, value } => {
                if !FIELDS.contains(&field.as_str()) {
                    return Err(zbus::Error::Failure(format!("unknown field {field:?}")));
                }
                patch_section(&proxy, "appearance", |section| {
                    section.insert(field, value);
                    Ok(())
                })
                .await
            }
        }
    }

    async fn snapshot(&self) -> zbus::Result<Value> {
        let proxy = DesktopProxy::new(&self.conn).await?;
        let (theme, variant) = proxy.get_theme().await?;
        let themes = proxy.list_themes().await?;
        let schedule = parse_json(&proxy.get_schedule().await?)?;
        let appearance = parse_json(&proxy.get_section("appearance").await?)?;
        let sources = sources(&proxy, &["appearance"]).await;
        Ok(json!({
            "themes": themes,
            "theme": theme,
            "variant": variant,
            "schedule": schedule,
            "appearance": appearance,
            "cursorThemes": system_lists::cursor_themes(&self.icon_dirs),
            "iconThemes": system_lists::icon_themes(&self.icon_dirs),
            "sources": sources,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pages::fake_uqd::{self, Rig};
    use crate::pages::runner::{Outcome, run};
    use uq_core::state::State;
    use uq_core::test_bus::{connect, private_bus};

    async fn rig() -> (Rig, AppearanceState) {
        let mut state = State {
            theme: "gruvbox".into(),
            ..State::default()
        };
        state
            .settings
            .set_section(
                "appearance",
                json!({"ui_font": "Inter", "font_size": 11, "kvantum": true}),
            )
            .unwrap();
        let r = fake_uqd::rig(state).await;
        let page = AppearanceState::new(r.client.clone(), vec![]);
        (r, page)
    }

    fn section(r: &Rig) -> Value {
        r.state
            .lock()
            .unwrap()
            .settings
            .section("appearance")
            .unwrap()
    }

    fn field(field: &str, value: Value) -> Op {
        Op::SetField {
            field: field.into(),
            value,
        }
    }

    #[tokio::test]
    async fn set_field_keeps_other_fields() {
        let (r, page) = rig().await;
        let out = run(&page, field("mono_font", json!("JetBrains Mono"))).await;
        assert_eq!(out.error, None);
        let a = &out.state["appearance"];
        assert_eq!(a["mono_font"], "JetBrains Mono");
        assert_eq!(a["ui_font"], "Inter");
        assert_eq!(a["font_size"], 11);
        assert_eq!(a["kvantum"], true);
        assert_eq!(section(&r), *a);

        // "Use theme": null clears one override and leaves the rest.
        let out = run(&page, field("ui_font", Value::Null)).await;
        assert_eq!(out.state["appearance"]["ui_font"], Value::Null);
        assert_eq!(out.state["appearance"]["mono_font"], "JetBrains Mono");

        let out = run(
            &page,
            Op::SetTheme {
                id: "dracula".into(),
                variant: "light".into(),
            },
        )
        .await;
        assert_eq!(
            (&out.state["theme"], &out.state["variant"]),
            (&json!("dracula"), &json!("light"))
        );
        assert_eq!(out.state["themes"], json!(["gruvbox", "dracula"]));
        assert_eq!(out.state["schedule"]["mode"], "off");
    }

    #[tokio::test]
    async fn concurrent_field_changes_both_land() {
        let (r, page) = rig().await;
        let (a, b) = tokio::join!(
            run(&page, field("mono_font", json!("Fira Code"))),
            run(&page, field("cursor_size", json!(32)))
        );
        assert_eq!((a.error, b.error), (None, None));
        let section = section(&r);
        assert_eq!(section["mono_font"], "Fira Code");
        assert_eq!(section["cursor_size"], 32);
        assert_eq!(section["ui_font"], "Inter");
    }

    #[tokio::test]
    async fn error_restores_state() {
        let (_r, page) = rig().await;
        let before = run(&page, Op::Refresh).await.state;
        let out = run(&page, field("font_size", json!(99))).await;
        let error = out.error.unwrap();
        assert!(error.contains("font_size"), "{error}");
        assert_eq!(
            out.state, before,
            "the controls get the last good values back"
        );
        assert_eq!(out.state["appearance"]["font_size"], 11);

        let out = run(&page, field("nope", json!(1))).await;
        assert!(out.error.unwrap().contains("unknown field"));
    }

    #[tokio::test]
    async fn snapshot_includes_appearance_sources() {
        let (r, page) = rig().await;
        r.explain
            .set("appearance.cursor_size", json!({"source": "runtime"}));
        let out = run(&page, Op::Refresh).await;
        let s = &out.state["sources"];
        assert_eq!(s["ui-font"], json!({"source": "uq", "detail": null}));
        assert_eq!(s["mono-font"], json!({"source": "theme", "detail": null}));
        assert_eq!(
            s["cursor-size"],
            json!({"source": "runtime", "detail": null})
        );
        assert!(s.get("gaps-inner").is_none(), "{s}");
    }

    #[tokio::test]
    async fn uqd_down_gives_empty_state_without_an_error() {
        let bus = private_bus();
        let page = AppearanceState::new(connect(&bus).await, vec![]);
        let out = run(&page, Op::Refresh).await;
        assert_eq!(
            out,
            Outcome {
                state: json!({}),
                error: None
            }
        );
    }
}
