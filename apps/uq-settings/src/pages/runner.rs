//! What every page model shares, as plain Rust: run one operation against uqd under a write
//! lock and read the state back ([`run`]), and read-modify-write one section
//! ([`patch_section`]). `crate::model_core` is the QObject side.

use std::future::Future;

use serde_json::{Map, Value, json};
use tokio::sync::Mutex;
use uq_core::client::{self, DesktopProxy};
use uq_core::settings::provenance;

/// The origin uqd's history records for every write the app makes.
pub const ORIGIN: &str = "settings";

/// The result of an operation: the state to show, and uqd's message when it was refused.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    /// The page's state, or `{}` when uqd can't be read.
    pub state: Value,
    pub error: Option<String>,
}

/// The text to show for a failed call: uqd's own message when it sent one.
pub fn message(e: &zbus::Error) -> String {
    match e {
        zbus::Error::MethodError(_, Some(text), _) => text.clone(),
        zbus::Error::FDO(inner) => inner.to_string(),
        other => other.to_string(),
    }
}

/// Parse JSON uqd sent.
pub fn parse_json(text: &str) -> zbus::Result<Value> {
    serde_json::from_str(text).map_err(|e| zbus::Error::Failure(format!("uqd sent bad JSON: {e}")))
}

/// One page's talk with uqd: what it can be asked to do, and the state it shows.
pub trait PageLogic: Send + Sync + 'static {
    type Op: Send + 'static;

    /// The operation that only reads.
    fn refresh() -> Self::Op;
    fn is_refresh(op: &Self::Op) -> bool;
    /// Held while an operation is applied: writes read-modify-write, so one at a time.
    fn write_lock(&self) -> &Mutex<()>;
    fn apply(&self, op: Self::Op) -> impl Future<Output = zbus::Result<()>> + Send;
    /// Everything the page shows, read from uqd.
    fn snapshot(&self) -> impl Future<Output = zbus::Result<Value>> + Send;
}

/// Do `op`, then read the state back. A refusal comes back as `error` with the state as uqd has
/// it, so the controls snap back to the last good value. uqd being down is not an error: the
/// state is `{}` and the page says so.
pub async fn run<L: PageLogic>(l: &L, op: L::Op) -> Outcome {
    let done = {
        let _guard = l.write_lock().lock().await;
        l.apply(op).await
    };
    read_back(l, done).await
}

/// Ask uqd to run `domain`'s render path again (`Reapply`), then read the state back like
/// [`run`]. It writes nothing, but waits for the page's writes so it applies the latest.
pub async fn reapply<L: PageLogic>(l: &L, conn: &zbus::Connection, domain: &str) -> Outcome {
    let done = {
        let _guard = l.write_lock().lock().await;
        match DesktopProxy::new(conn).await {
            Ok(proxy) => proxy.reapply(domain).await,
            Err(e) => Err(e),
        }
    };
    read_back(l, done).await
}

/// The [`Outcome`] of an operation that ended with `done`.
async fn read_back<L: PageLogic>(l: &L, done: zbus::Result<()>) -> Outcome {
    let error = done
        .err()
        .filter(|e| !client::no_owner(e))
        .map(|e| message(&e));
    match l.snapshot().await {
        Ok(state) => Outcome { state, error },
        Err(e) => Outcome {
            state: json!({}),
            error: error.or_else(|| (!client::no_owner(&e)).then(|| message(&e))),
        },
    }
}

/// Read `domain`'s section from uqd, let `edit` change it, and write it back. Call it under
/// [`PageLogic::write_lock`], so the read sees every earlier write.
pub async fn patch_section(
    proxy: &DesktopProxy<'_>,
    domain: &str,
    edit: impl FnOnce(&mut Map<String, Value>) -> zbus::Result<()>,
) -> zbus::Result<()> {
    let mut section = parse_json(&proxy.get_section(domain).await?)?;
    let object = section
        .as_object_mut()
        .ok_or_else(|| zbus::Error::Failure(format!("the {domain} section isn't an object")))?;
    edit(object)?;
    proxy
        .set_section(domain, &section.to_string(), ORIGIN)
        .await
}

/// How much a source says about a row, for a row that shows several keys (a layout's options):
/// the most telling one wins, and the first of equals.
fn rank(source: &str) -> u8 {
    match source {
        "error" => 5,
        "user_lua" => 4,
        "runtime" => 3,
        "uq" => 2,
        "theme" | "default" => 1,
        _ => 0,
    }
}

/// Where the live value of each setting of `domains` comes from, as
/// `{"<setting id>": {source, detail}}`, from uqd's `Explain`. uqd keys its answer by the path
/// inside the domain; [`provenance::OPTIONS`] gives each path's setting id. A domain whose
/// `Explain` fails adds nothing, so its rows show no source.
pub async fn sources(proxy: &DesktopProxy<'_>, domains: &[&str]) -> Value {
    let mut out = Map::new();
    for &domain in domains {
        let explained = match proxy.explain(domain).await.and_then(|t| parse_json(&t)) {
            Ok(v) => v,
            Err(e) => {
                if !client::no_owner(&e) {
                    eprintln!("uq-settings: can't explain {domain}: {}", message(&e));
                }
                continue;
            }
        };
        for row in provenance::options_for(domain) {
            let Some(entry) = explained.get(row.json_key()) else {
                continue;
            };
            let Some(source) = entry.get("source").and_then(Value::as_str) else {
                continue;
            };
            let shown = out
                .get(row.setting_id)
                .and_then(|o| o["source"].as_str())
                .map_or(0, rank);
            if out.contains_key(row.setting_id) && shown >= rank(source) {
                continue;
            }
            let detail = entry.get("detail").cloned().unwrap_or(Value::Null);
            out.insert(
                row.setting_id.to_string(),
                json!({ "source": source, "detail": detail }),
            );
        }
    }
    Value::Object(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pages::fake_uqd;
    use uq_core::state::State;
    use uq_core::test_bus::{connect, private_bus};

    /// The smallest page: sets one field of the `appearance` section; its state is that section.
    struct Tiny {
        conn: zbus::Connection,
        write: Mutex<()>,
    }

    impl PageLogic for Tiny {
        type Op = Option<(String, serde_json::Value)>;

        fn refresh() -> Self::Op {
            None
        }

        fn is_refresh(op: &Self::Op) -> bool {
            op.is_none()
        }

        fn write_lock(&self) -> &Mutex<()> {
            &self.write
        }

        async fn apply(&self, op: Self::Op) -> zbus::Result<()> {
            let Some((field, value)) = op else {
                return Ok(());
            };
            let proxy = DesktopProxy::new(&self.conn).await?;
            patch_section(&proxy, "appearance", |o| {
                o.insert(field, value);
                Ok(())
            })
            .await
        }

        async fn snapshot(&self) -> zbus::Result<serde_json::Value> {
            let proxy = DesktopProxy::new(&self.conn).await?;
            parse_json(&proxy.get_section("appearance").await?)
        }
    }

    fn tiny(conn: zbus::Connection) -> Tiny {
        Tiny {
            conn,
            write: Mutex::new(()),
        }
    }

    fn set(field: &str, value: serde_json::Value) -> Option<(String, serde_json::Value)> {
        Some((field.into(), value))
    }

    #[tokio::test]
    async fn refusal_returns_uqd_state_and_message() {
        let r = fake_uqd::rig(State::default()).await;
        let t = tiny(r.client.clone());
        assert_eq!(run(&t, set("font_size", json!(12))).await.error, None);

        let out = run(&t, set("font_size", json!(99))).await;
        let error = out.error.unwrap();
        assert!(error.contains("font_size"), "{error}");
        assert_eq!(out.state["font_size"], 12, "the last good value comes back");

        // A config Hyprland refuses: its text, and the section as uqd still has it.
        *r.verify_error.lock().unwrap() = Some("bad option".into());
        let out = run(&t, set("font_size", json!(14))).await;
        assert_eq!(
            out.error.as_deref(),
            Some("Hyprland rejected the new config: bad option")
        );
        assert_eq!(out.state["font_size"], 12);
        assert_eq!(
            r.state.lock().unwrap().settings.appearance.font_size,
            Some(12)
        );
    }

    #[tokio::test]
    async fn uqd_down_is_empty_without_error() {
        let bus = private_bus();
        let t = tiny(connect(&bus).await);
        assert_eq!(
            run(&t, Tiny::refresh()).await,
            Outcome {
                state: json!({}),
                error: None
            }
        );
        assert_eq!(
            run(&t, set("font_size", json!(12))).await,
            Outcome {
                state: json!({}),
                error: None
            },
            "a write with uqd down is not an error either: the page shows uqd as down"
        );
    }

    #[tokio::test]
    async fn writes_are_serialised() {
        let r = fake_uqd::rig(State::default()).await;
        let t = tiny(r.client.clone());
        let (a, b, c) = tokio::join!(
            run(&t, set("ui_font", json!("Inter"))),
            run(&t, set("mono_font", json!("Fira Code"))),
            run(&t, set("cursor_size", json!(32)))
        );
        assert_eq!((a.error, b.error, c.error), (None, None, None));
        let a = r.state.lock().unwrap().settings.appearance.clone();
        assert_eq!(a.ui_font.as_deref(), Some("Inter"));
        assert_eq!(a.mono_font.as_deref(), Some("Fira Code"));
        assert_eq!(a.cursor_size, Some(32));
    }

    #[test]
    fn bad_json_is_named() {
        let e = parse_json("{").unwrap_err();
        assert!(message(&e).starts_with("uqd sent bad JSON: "), "{e}");
    }
}
