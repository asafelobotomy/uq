//! Typed proxies for the system services the System pages use. Calls that can raise a polkit
//! prompt allow interactive authorization; the connection's long timeout
//! ([`super::sysbus::CALL_TIMEOUT`]) covers the time the prompt is up.

use std::collections::HashMap;

use zbus::proxy::CacheProperties;
use zbus::zvariant::{OwnedObjectPath, OwnedValue};

#[zbus::proxy(
    interface = "org.freedesktop.Accounts",
    default_service = "org.freedesktop.Accounts",
    default_path = "/org/freedesktop/Accounts"
)]
pub trait Accounts {
    fn list_cached_users(&self) -> zbus::Result<Vec<OwnedObjectPath>>;
    #[zbus(allow_interactive_auth)]
    fn create_user(
        &self,
        name: &str,
        fullname: &str,
        account_type: i32,
    ) -> zbus::Result<OwnedObjectPath>;
    #[zbus(allow_interactive_auth)]
    fn delete_user(&self, id: i64, remove_files: bool) -> zbus::Result<()>;
}

#[zbus::proxy(
    interface = "org.freedesktop.Accounts.User",
    default_service = "org.freedesktop.Accounts"
)]
pub trait AccountsUser {
    #[zbus(property)]
    fn uid(&self) -> zbus::Result<u64>;
    #[zbus(property)]
    fn user_name(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn real_name(&self) -> zbus::Result<String>;
    /// 0 standard, 1 administrator.
    #[zbus(property)]
    fn account_type(&self) -> zbus::Result<i32>;
    #[zbus(property)]
    fn icon_file(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn locked(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn system_account(&self) -> zbus::Result<bool>;

    #[zbus(allow_interactive_auth)]
    fn set_real_name(&self, name: &str) -> zbus::Result<()>;
    #[zbus(allow_interactive_auth)]
    fn set_account_type(&self, account_type: i32) -> zbus::Result<()>;
    /// `password` is already crypted (crypt(3)); AccountsService never sees the typed text.
    #[zbus(allow_interactive_auth)]
    fn set_password(&self, password: &str, hint: &str) -> zbus::Result<()>;
    #[zbus(allow_interactive_auth)]
    fn set_icon_file(&self, filename: &str) -> zbus::Result<()>;
}

#[zbus::proxy(
    interface = "org.freedesktop.timedate1",
    default_service = "org.freedesktop.timedate1",
    default_path = "/org/freedesktop/timedate1"
)]
pub trait Timedate {
    #[zbus(property)]
    fn timezone(&self) -> zbus::Result<String>;
    #[zbus(property, name = "NTP")]
    fn ntp(&self) -> zbus::Result<bool>;
    #[zbus(property, name = "CanNTP")]
    fn can_ntp(&self) -> zbus::Result<bool>;
    #[zbus(property, name = "NTPSynchronized")]
    fn ntp_synchronized(&self) -> zbus::Result<bool>;
    #[zbus(property, name = "TimeUSec")]
    fn time_usec(&self) -> zbus::Result<u64>;

    fn list_timezones(&self) -> zbus::Result<Vec<String>>;
    #[zbus(allow_interactive_auth)]
    fn set_timezone(&self, timezone: &str, interactive: bool) -> zbus::Result<()>;
    #[zbus(name = "SetNTP", allow_interactive_auth)]
    fn set_ntp(&self, use_ntp: bool, interactive: bool) -> zbus::Result<()>;
    #[zbus(allow_interactive_auth)]
    fn set_time(&self, usec_utc: i64, relative: bool, interactive: bool) -> zbus::Result<()>;
}

#[zbus::proxy(
    interface = "org.freedesktop.locale1",
    default_service = "org.freedesktop.locale1",
    default_path = "/org/freedesktop/locale1"
)]
pub trait Locale {
    #[zbus(property)]
    fn locale(&self) -> zbus::Result<Vec<String>>;
    #[zbus(property, name = "VConsoleKeymap")]
    fn vconsole_keymap(&self) -> zbus::Result<String>;
    #[zbus(property, name = "VConsoleKeymapToggle")]
    fn vconsole_keymap_toggle(&self) -> zbus::Result<String>;

    #[zbus(allow_interactive_auth)]
    fn set_locale(&self, locale: &[&str], interactive: bool) -> zbus::Result<()>;
    #[zbus(name = "SetVConsoleKeyboard", allow_interactive_auth)]
    fn set_vconsole_keyboard(
        &self,
        keymap: &str,
        keymap_toggle: &str,
        convert: bool,
        interactive: bool,
    ) -> zbus::Result<()>;
}

#[zbus::proxy(
    interface = "org.freedesktop.hostname1",
    default_service = "org.freedesktop.hostname1",
    default_path = "/org/freedesktop/hostname1"
)]
pub trait Hostname {
    #[zbus(property)]
    fn hostname(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn static_hostname(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn kernel_release(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn operating_system_pretty_name(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn hardware_vendor(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn hardware_model(&self) -> zbus::Result<String>;

    #[zbus(allow_interactive_auth)]
    fn set_static_hostname(&self, hostname: &str, interactive: bool) -> zbus::Result<()>;
}

#[zbus::proxy(
    interface = "org.freedesktop.UPower",
    default_service = "org.freedesktop.UPower",
    default_path = "/org/freedesktop/UPower"
)]
pub trait UPower {
    #[zbus(property)]
    fn on_battery(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn lid_is_present(&self) -> zbus::Result<bool>;
    fn enumerate_devices(&self) -> zbus::Result<Vec<OwnedObjectPath>>;
}

#[zbus::proxy(
    interface = "org.freedesktop.UPower.Device",
    default_service = "org.freedesktop.UPower"
)]
pub trait UPowerDevice {
    /// 2 is a battery.
    #[zbus(property, name = "Type")]
    fn kind(&self) -> zbus::Result<u32>;
    #[zbus(property)]
    fn power_supply(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn is_present(&self) -> zbus::Result<bool>;
    /// 1 charging, 2 discharging, 3 empty, 4 full, 5 not charging, 6 not discharging.
    #[zbus(property)]
    fn state(&self) -> zbus::Result<u32>;
    #[zbus(property)]
    fn percentage(&self) -> zbus::Result<f64>;
    #[zbus(property)]
    fn time_to_empty(&self) -> zbus::Result<i64>;
    #[zbus(property)]
    fn time_to_full(&self) -> zbus::Result<i64>;
    /// Health: how much the battery holds now, as a percentage of its design capacity.
    #[zbus(property)]
    fn capacity(&self) -> zbus::Result<f64>;
    #[zbus(property)]
    fn vendor(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn model(&self) -> zbus::Result<String>;
}

#[zbus::proxy(
    interface = "org.freedesktop.login1.Manager",
    default_service = "org.freedesktop.login1",
    default_path = "/org/freedesktop/login1"
)]
pub trait Login1 {
    #[zbus(property)]
    fn handle_lid_switch(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn handle_lid_switch_external_power(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn handle_power_key(&self) -> zbus::Result<String>;
}

#[zbus::proxy(
    interface = "org.uq.SettingsHelper1",
    default_service = "org.uq.SettingsHelper1",
    default_path = "/org/uq/SettingsHelper1"
)]
pub trait Helper {
    fn set_logind_power_actions(
        &self,
        lid: &str,
        lid_on_ac: &str,
        power_key: &str,
    ) -> zbus::Result<()>;
    fn set_greeter_appearance(
        &self,
        theme: &str,
        variant: &str,
        hide_users: bool,
        reduce_motion: bool,
    ) -> zbus::Result<()>;
    fn set_greeter_wallpaper(&self, fd: zbus::zvariant::Fd<'_>) -> zbus::Result<()>;
    fn clear_greeter_wallpaper(&self) -> zbus::Result<()>;
    fn set_greeter_monitors(&self, json: &str) -> zbus::Result<()>;
    fn set_auto_login(&self, user: &str) -> zbus::Result<()>;
}

pub const HELPER_NAME: &str = "org.uq.SettingsHelper1";

/// power-profiles-daemon: the current name, then the one older releases use.
pub const POWER_PROFILES_NAMES: [(&str, &str, &str); 2] = [
    (
        "org.freedesktop.UPower.PowerProfiles",
        "/org/freedesktop/UPower/PowerProfiles",
        "org.freedesktop.UPower.PowerProfiles",
    ),
    (
        "net.hadess.PowerProfiles",
        "/net/hadess/PowerProfiles",
        "net.hadess.PowerProfiles",
    ),
];

/// A proxy that reads every property afresh (the pages read state once per refresh).
pub async fn fresh<'a, P>(conn: &zbus::Connection) -> zbus::Result<P>
where
    P: From<zbus::Proxy<'a>> + zbus::proxy::Defaults,
{
    zbus::proxy::Builder::<P>::new(conn)
        .cache_properties(CacheProperties::No)
        .build()
        .await
}

/// [`fresh`] at an object path (a user, a battery).
pub async fn fresh_at<'a, P>(conn: &zbus::Connection, path: OwnedObjectPath) -> zbus::Result<P>
where
    P: From<zbus::Proxy<'a>> + zbus::proxy::Defaults,
{
    zbus::proxy::Builder::<P>::new(conn)
        .path(path)?
        .cache_properties(CacheProperties::No)
        .build()
        .await
}

/// One power profile as power-profiles-daemon lists it.
pub type ProfileDict = HashMap<String, OwnedValue>;
