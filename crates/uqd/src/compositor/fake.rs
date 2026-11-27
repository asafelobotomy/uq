use super::*;
use std::sync::Mutex;

#[derive(Debug, Default)]
pub struct FakeCompositor {
    pub running: Mutex<bool>,
    /// `Some(msg)` makes `verify` return `Rejected(msg)`.
    pub verify_error: Mutex<Option<String>>,
    pub calls: Mutex<Vec<String>>,
    /// (main config path, its contents) for every `verify` call.
    pub verified: Mutex<Vec<(std::path::PathBuf, String)>>,
    /// What `monitors` returns (`monitors all -j`); empty means `[]`.
    pub monitors_json: Mutex<String>,
    /// What `devices` returns (`devices -j`); empty means `{}`.
    pub devices_json: Mutex<String>,
    /// What `clients` returns (`clients -j`); empty means `[]`.
    pub clients_json: Mutex<String>,
    /// What `cursor_pos` returns (`cursorpos -j`); empty means `{"x":0,"y":0}`.
    pub cursor_json: Mutex<String>,
    /// What `workspaces` returns (`workspaces -j`); empty means `[]`.
    pub workspaces_json: Mutex<String>,
    /// What `binds` returns (`binds -j`); empty means `[]`.
    pub binds_json: Mutex<String>,
    /// `Some(msg)` makes the next `eval` return `Rejected(msg)`, once.
    pub eval_error: Mutex<Option<String>>,
    /// The next this many `eval`s return `Timeout`.
    pub eval_timeouts: Mutex<u32>,
    /// `Some(msg)` makes the next `reload` return `Rejected(msg)`, once.
    pub reload_error: Mutex<Option<String>>,
    /// What `get_option` answers per option path; a missing path is `Rejected`.
    pub options: Mutex<std::collections::HashMap<String, serde_json::Value>>,
    /// What `config_errors` returns.
    pub config_errors: Mutex<Vec<String>>,
}

impl FakeCompositor {
    pub fn running() -> Self {
        FakeCompositor {
            running: Mutex::new(true),
            ..Default::default()
        }
    }
    pub fn set_option(&self, path: &str, value: serde_json::Value) {
        self.options.lock().unwrap().insert(path.to_string(), value);
    }
    pub fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
    fn record(&self, call: String) -> Result<(), CompositorError> {
        self.calls.lock().unwrap().push(call);
        if *self.running.lock().unwrap() {
            Ok(())
        } else {
            Err(CompositorError::NotRunning)
        }
    }
}

#[async_trait]
impl Compositor for FakeCompositor {
    async fn verify(&self, main_config: &Path) -> Result<(), CompositorError> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("verify {}", main_config.display()));
        let src = std::fs::read_to_string(main_config).unwrap_or_default();
        self.verified
            .lock()
            .unwrap()
            .push((main_config.to_path_buf(), src));
        match self.verify_error.lock().unwrap().clone() {
            Some(msg) => Err(CompositorError::Rejected(msg)),
            None => Ok(()),
        }
    }
    async fn eval(&self, lua: &str) -> Result<(), CompositorError> {
        self.record(format!("eval {lua}"))?;
        {
            let mut n = self.eval_timeouts.lock().unwrap();
            if *n > 0 {
                *n -= 1;
                return Err(CompositorError::Timeout("eval".into()));
            }
        }
        match self.eval_error.lock().unwrap().take() {
            Some(msg) => Err(CompositorError::Rejected(msg)),
            None => Ok(()),
        }
    }
    async fn reload(&self) -> Result<(), CompositorError> {
        self.record("reload".into())?;
        match self.reload_error.lock().unwrap().take() {
            Some(msg) => Err(CompositorError::Rejected(msg)),
            None => Ok(()),
        }
    }
    async fn set_cursor(&self, theme: &str, size: u8) -> Result<(), CompositorError> {
        self.record(format!("setcursor {theme} {size}"))
    }
    async fn config_errors(&self) -> Result<Vec<String>, CompositorError> {
        self.record("configerrors".into())?;
        Ok(self.config_errors.lock().unwrap().clone())
    }
    async fn monitors(&self) -> Result<String, CompositorError> {
        self.record("monitors".into())?;
        let j = self.monitors_json.lock().unwrap().clone();
        Ok(if j.is_empty() { "[]".into() } else { j })
    }
    async fn devices(&self) -> Result<String, CompositorError> {
        self.record("devices".into())?;
        let j = self.devices_json.lock().unwrap().clone();
        Ok(if j.is_empty() { "{}".into() } else { j })
    }
    async fn clients(&self) -> Result<String, CompositorError> {
        self.record("clients".into())?;
        let j = self.clients_json.lock().unwrap().clone();
        Ok(if j.is_empty() { "[]".into() } else { j })
    }
    async fn cursor_pos(&self) -> Result<String, CompositorError> {
        self.record("cursorpos".into())?;
        let j = self.cursor_json.lock().unwrap().clone();
        Ok(if j.is_empty() {
            r#"{"x":0,"y":0}"#.into()
        } else {
            j
        })
    }
    async fn workspaces(&self) -> Result<String, CompositorError> {
        self.record("workspaces".into())?;
        let j = self.workspaces_json.lock().unwrap().clone();
        Ok(if j.is_empty() { "[]".into() } else { j })
    }
    async fn binds(&self) -> Result<String, CompositorError> {
        self.record("binds".into())?;
        let j = self.binds_json.lock().unwrap().clone();
        Ok(if j.is_empty() { "[]".into() } else { j })
    }
    async fn get_option(&self, path: &str) -> Result<serde_json::Value, CompositorError> {
        if !*self.running.lock().unwrap() {
            return Err(CompositorError::NotRunning);
        }
        self.options
            .lock()
            .unwrap()
            .get(path)
            .cloned()
            .ok_or_else(|| CompositorError::Rejected("no such option".into()))
    }
}
