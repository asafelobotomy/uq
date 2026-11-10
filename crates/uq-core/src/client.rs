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
    fn set_setting(&self, key: &str, json: &str) -> zbus::Result<()>;
    fn get_launch_counts(&self) -> zbus::Result<HashMap<String, u32>>;
    fn clear_launch_counts(&self) -> zbus::Result<()>;
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

/// uqd's bus name (the default service of [`DesktopProxy`]).
pub const DESKTOP_NAME: &str = "org.uq.Desktop1";
/// The Settings app's single-instance bus name (the default service of [`SettingsAppProxy`]).
pub const SETTINGS_NAME: &str = "org.uq.Settings1";
/// The object path the Settings app serves `org.uq.Settings1` at.
pub const SETTINGS_PATH: &str = "/org/uq/Settings1";

/// Whether a call failed because nobody owns the destination name.
pub fn no_owner(err: &zbus::Error) -> bool {
    const NAMES: [&str; 2] = [
        "org.freedesktop.DBus.Error.ServiceUnknown",
        "org.freedesktop.DBus.Error.NameHasNoOwner",
    ];
    match err {
        zbus::Error::MethodError(name, _, _) => NAMES.contains(&name.as_str()),
        zbus::Error::FDO(e) => matches!(
            **e,
            zbus::fdo::Error::ServiceUnknown(_) | zbus::fdo::Error::NameHasNoOwner(_)
        ),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_match_the_proxies() {
        use zbus::proxy::Defaults;
        let name = |n: &Option<zbus::names::BusName<'static>>| n.as_ref().unwrap().to_string();
        assert_eq!(DESKTOP_NAME, name(DesktopProxy::DESTINATION));
        assert_eq!(SETTINGS_NAME, name(SettingsAppProxy::DESTINATION));
        let path = SettingsAppProxy::PATH.as_ref().unwrap();
        assert_eq!(SETTINGS_PATH, path.as_str());
    }
}
