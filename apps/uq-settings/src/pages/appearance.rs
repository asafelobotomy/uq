//! The Appearance page's logic: [`AppearanceState`] talks to uqd (`org.uq.Desktop1`) and is plain
//! Rust, tested against a fake uqd. `crate::appearance_model` is the thin QML object around it.

use std::path::PathBuf;

use serde_json::{Value, json};
use tokio::sync::Mutex;
use uq_core::client::{self, DesktopProxy};

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

/// The result of an [`Op`]: the state to show, and uqd's message when it was refused.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    /// `{themes, theme, variant, schedule, appearance, cursorThemes, iconThemes}`, or `{}` when
    /// uqd can't be read.
    pub state: Value,
    pub error: Option<String>,
}

pub struct AppearanceState {
    conn: zbus::Connection,
    /// Writes read-modify-write the section, so one at a time.
    write: Mutex<()>,
    icon_dirs: Vec<PathBuf>,
}

/// The text to show for a failed call: uqd's own message when it sent one.
fn message(e: &zbus::Error) -> String {
    match e {
        zbus::Error::MethodError(_, Some(text), _) => text.clone(),
        zbus::Error::FDO(inner) => inner.to_string(),
        other => other.to_string(),
    }
}

impl AppearanceState {
    pub fn new(conn: zbus::Connection, icon_dirs: Vec<PathBuf>) -> Self {
        Self {
            conn,
            write: Mutex::new(()),
            icon_dirs,
        }
    }

    /// Do `op`, then read the state back. A refusal comes back as `error` with the state as uqd
    /// has it, so the controls snap back to the last good value.
    pub async fn run(&self, op: Op) -> Outcome {
        let done = {
            let _guard = self.write.lock().await;
            self.apply(op).await
        };
        let error = done
            .err()
            .filter(|e| !client::no_owner(e))
            .map(|e| message(&e));
        match self.snapshot().await {
            Ok(state) => Outcome { state, error },
            Err(e) => Outcome {
                state: json!({}),
                error: error.or_else(|| (!client::no_owner(&e)).then(|| message(&e))),
            },
        }
    }

    async fn apply(&self, op: Op) -> zbus::Result<()> {
        let proxy = DesktopProxy::new(&self.conn).await?;
        match op {
            Op::Refresh => Ok(()),
            Op::SetTheme { id, variant } => proxy.set_theme(&id, &variant).await,
            Op::SetSchedule(schedule) => proxy.set_schedule(&schedule).await,
            Op::SetField { field, value } => {
                if !FIELDS.contains(&field.as_str()) {
                    return Err(zbus::Error::Failure(format!("unknown field {field:?}")));
                }
                let mut section: Value =
                    serde_json::from_str(&proxy.get_section("appearance").await?)
                        .map_err(|e| zbus::Error::Failure(format!("uqd sent bad JSON: {e}")))?;
                let object = section.as_object_mut().ok_or_else(|| {
                    zbus::Error::Failure("the appearance section isn't an object".into())
                })?;
                object.insert(field, value);
                proxy.set_section("appearance", &section.to_string()).await
            }
        }
    }

    async fn snapshot(&self) -> zbus::Result<Value> {
        let proxy = DesktopProxy::new(&self.conn).await?;
        let (theme, variant) = proxy.get_theme().await?;
        let themes = proxy.list_themes().await?;
        let json_of = |text: String| {
            serde_json::from_str::<Value>(&text)
                .map_err(|e| zbus::Error::Failure(format!("uqd sent bad JSON: {e}")))
        };
        let schedule = json_of(proxy.get_schedule().await?)?;
        let appearance = json_of(proxy.get_section("appearance").await?)?;
        Ok(json!({
            "themes": themes,
            "theme": theme,
            "variant": variant,
            "schedule": schedule,
            "appearance": appearance,
            "cursorThemes": system_lists::cursor_themes(&self.icon_dirs),
            "iconThemes": system_lists::icon_themes(&self.icon_dirs),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_bus::{connect, private_bus};
    use std::sync::Arc;
    use std::sync::Mutex as StdMutex;
    use uq_core::client::DESKTOP_NAME;

    #[derive(Debug, zbus::DBusError)]
    #[zbus(prefix = "org.uq.Desktop1.Error")]
    enum FakeError {
        #[zbus(error)]
        ZBus(zbus::Error),
        InvalidSetting(String),
    }

    struct Fake {
        section: Arc<StdMutex<Value>>,
        theme: Arc<StdMutex<(String, String)>>,
    }

    #[zbus::interface(name = "org.uq.Desktop1")]
    impl Fake {
        fn get_theme(&self) -> (String, String) {
            self.theme.lock().unwrap().clone()
        }
        fn set_theme(&self, name: &str, variant: &str) {
            *self.theme.lock().unwrap() = (name.into(), variant.into());
        }
        fn list_themes(&self) -> Vec<String> {
            vec!["gruvbox".into(), "dracula".into()]
        }
        fn get_schedule(&self) -> String {
            r#"{"mode":"off"}"#.into()
        }
        fn get_section(&self, _domain: &str) -> String {
            self.section.lock().unwrap().to_string()
        }
        /// Like uqd: font_size must be 6..=32.
        fn set_section(&self, _domain: &str, json: &str) -> Result<(), FakeError> {
            let v: Value =
                serde_json::from_str(json).map_err(|e| FakeError::InvalidSetting(e.to_string()))?;
            if let Some(size) = v["font_size"].as_i64()
                && !(6..=32).contains(&size)
            {
                return Err(FakeError::InvalidSetting(format!(
                    "font_size {size} is outside 6 to 32"
                )));
            }
            *self.section.lock().unwrap() = v;
            Ok(())
        }
    }

    struct Rig {
        _bus: crate::test_bus::Bus,
        _uqd: zbus::Connection,
        state: AppearanceState,
        section: Arc<StdMutex<Value>>,
    }

    async fn rig() -> Rig {
        let bus = private_bus();
        let section = Arc::new(StdMutex::new(json!({
            "ui_font": "Inter", "mono_font": null, "font_size": 11,
            "cursor_theme": null, "cursor_size": null, "icon_theme": null, "kvantum": true,
        })));
        let fake = Fake {
            section: section.clone(),
            theme: Arc::new(StdMutex::new(("gruvbox".into(), "dark".into()))),
        };
        let uqd = connect(&bus).await;
        uqd.object_server()
            .at("/org/uq/Desktop1", fake)
            .await
            .unwrap();
        uqd.request_name(DESKTOP_NAME).await.unwrap();
        let state = AppearanceState::new(connect(&bus).await, vec![]);
        Rig {
            _bus: bus,
            _uqd: uqd,
            state,
            section,
        }
    }

    fn field(field: &str, value: Value) -> Op {
        Op::SetField {
            field: field.into(),
            value,
        }
    }

    #[tokio::test]
    async fn set_field_keeps_other_fields() {
        let r = rig().await;
        let out = r
            .state
            .run(field("mono_font", json!("JetBrains Mono")))
            .await;
        assert_eq!(out.error, None);
        let a = &out.state["appearance"];
        assert_eq!(a["mono_font"], "JetBrains Mono");
        assert_eq!(a["ui_font"], "Inter");
        assert_eq!(a["font_size"], 11);
        assert_eq!(a["kvantum"], true);
        assert_eq!(*r.section.lock().unwrap(), *a);

        // "Use theme": null clears one override and leaves the rest.
        let out = r.state.run(field("ui_font", Value::Null)).await;
        assert_eq!(out.state["appearance"]["ui_font"], Value::Null);
        assert_eq!(out.state["appearance"]["mono_font"], "JetBrains Mono");

        let out = r
            .state
            .run(Op::SetTheme {
                id: "dracula".into(),
                variant: "light".into(),
            })
            .await;
        assert_eq!(
            (&out.state["theme"], &out.state["variant"]),
            (&json!("dracula"), &json!("light"))
        );
        assert_eq!(out.state["themes"], json!(["gruvbox", "dracula"]));
        assert_eq!(out.state["schedule"]["mode"], "off");
    }

    #[tokio::test]
    async fn error_restores_state() {
        let r = rig().await;
        let before = r.state.run(Op::Refresh).await.state;
        let out = r.state.run(field("font_size", json!(99))).await;
        assert_eq!(
            out.error.as_deref(),
            Some("font_size 99 is outside 6 to 32")
        );
        assert_eq!(
            out.state, before,
            "the controls get the last good values back"
        );
        assert_eq!(out.state["appearance"]["font_size"], 11);

        let out = r.state.run(field("nope", json!(1))).await;
        assert!(out.error.unwrap().contains("unknown field"));
    }

    #[tokio::test]
    async fn uqd_down_gives_empty_state_without_an_error() {
        let bus = private_bus();
        let state = AppearanceState::new(connect(&bus).await, vec![]);
        let out = state.run(Op::Refresh).await;
        assert_eq!(
            out,
            Outcome {
                state: json!({}),
                error: None
            }
        );
    }
}
