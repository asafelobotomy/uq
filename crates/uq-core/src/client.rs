//! Typed zbus clients: uqd's `org.uq.Desktop1` and the Settings app's `org.uq.Settings1`.
//! The signatures mirror `crates/uqd/src/dbus.rs`.

use std::collections::HashMap;

#[zbus::proxy(
    interface = "org.uq.Desktop1",
    default_service = "org.uq.Desktop1",
    default_path = "/org/uq/Desktop1"
)]
pub trait Desktop {
    fn get_theme(&self) -> zbus::Result<(String, String)>;
    fn set_theme(&self, name: &str, variant: &str) -> zbus::Result<()>;
    fn list_themes(&self) -> zbus::Result<Vec<String>>;
    fn get_schedule(&self) -> zbus::Result<String>;
    fn set_schedule(&self, json: &str) -> zbus::Result<()>;
    fn get_settings(&self) -> zbus::Result<String>;
    fn get_section(&self, domain: &str) -> zbus::Result<String>;
    fn set_section(&self, domain: &str, json: &str) -> zbus::Result<()>;
    fn get_layouts(&self) -> zbus::Result<HashMap<String, String>>;

    #[zbus(signal)]
    fn section_changed(&self, domain: &str, json: &str) -> zbus::Result<()>;
    #[zbus(signal)]
    fn theme_changed(&self, name: &str, variant: &str, tokens_path: &str) -> zbus::Result<()>;
    #[zbus(signal)]
    fn settings_changed(&self, json: &str) -> zbus::Result<()>;
}

/// The Settings app's single-instance interface. An empty `page_id` means "just raise".
#[zbus::proxy(
    interface = "org.uq.Settings1",
    default_service = "org.uq.Settings1",
    default_path = "/org/uq/Settings1"
)]
pub trait SettingsApp {
    fn show_page(&self, page_id: &str, setting_id: &str) -> zbus::Result<()>;
}
