//! A uQ launcher provider: answers Spotlight queries over D-Bus and exits after 60 s without a
//! call. The session bus starts it again when uQ's launcher opens (`Activatable=true` in the
//! `.provider` file and a D-Bus service file), so nothing runs while it isn't needed.
//!
//! The interface is `org.uq.Launcher1.Provider` at `/org/uq/Launcher1`:
//!
//! - `Search(s query) -> a(sssssd)`: up to 20 rows of (id, title, subtitle, icon, actions as a
//!   JSON list of strings, score 0-1). Keep it fast: uQ waits 300 ms at most.
//! - `Activate(s id, s action)`: the user chose a row (Enter runs the first action).
//!
//! uQ treats every string as untrusted plain text and clamps lengths; the provider still must
//! never run anything taken from a query.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Notify;

/// The bus name in the `.provider` and `.service` files.
const BUS_NAME: &str = "org.example.Provider";
const PATH: &str = "/org/uq/Launcher1";
/// Exit after this long without a call.
const IDLE: Duration = Duration::from_secs(60);

const FRUIT: [&str; 6] = ["apple", "banana", "cherry", "grape", "lemon", "orange"];

type Row = (String, String, String, String, String, f64);

struct Provider {
    /// Poked on every call, so the idle timer starts over.
    touched: Arc<Notify>,
}

#[zbus::interface(name = "org.uq.Launcher1.Provider")]
impl Provider {
    async fn search(&self, query: String) -> Vec<Row> {
        self.touched.notify_one();
        let q = query.trim().to_lowercase();
        FRUIT
            .iter()
            .filter(|f| !q.is_empty() && f.contains(q.as_str()))
            .take(20)
            .map(|f| {
                (
                    (*f).to_string(),
                    (*f).to_string(),
                    "An example result".to_string(),
                    "applications-other".to_string(),
                    r#"["copy"]"#.to_string(),
                    if f.starts_with(q.as_str()) { 0.8 } else { 0.5 },
                )
            })
            .collect()
    }

    async fn activate(&self, id: String, action: String) -> zbus::fdo::Result<()> {
        self.touched.notify_one();
        if !FRUIT.contains(&id.as_str()) {
            return Err(zbus::fdo::Error::InvalidArgs(format!("unknown item {id:?}")));
        }
        match action.as_str() {
            // No shell: the id is one argument to a fixed program.
            "copy" | "" => std::process::Command::new("/usr/bin/wl-copy")
                .arg("--")
                .arg(&id)
                .spawn()
                .map(|_| ())
                .map_err(|e| zbus::fdo::Error::Failed(format!("wl-copy: {e}"))),
            other => Err(zbus::fdo::Error::InvalidArgs(format!(
                "unknown action {other:?}"
            ))),
        }
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> zbus::Result<()> {
    let touched = Arc::new(Notify::new());
    let _conn = zbus::connection::Builder::session()?
        .name(BUS_NAME)?
        .serve_at(
            PATH,
            Provider {
                touched: touched.clone(),
            },
        )?
        .build()
        .await?;
    // Exit once a whole IDLE passes without a call; the bus starts us again when needed.
    while tokio::time::timeout(IDLE, touched.notified()).await.is_ok() {}
    Ok(())
}
