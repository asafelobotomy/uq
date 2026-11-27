//! The `org.uq.Desktop1.Error` names a failed call carries.

use super::*;

#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "org.uq.Desktop1.Error")]
pub enum UqError {
    #[zbus(error)]
    ZBus(zbus::Error),
    UnknownTheme(String),
    InvalidLayout(String),
    EmptyWorkspace(String),
    InvalidSchedule(String),
    InvalidArgument(String),
    InvalidSetting(String),
    ConfigInvalid(String),
    Io(String),
    Conflict(String),
}

impl From<DesktopError> for UqError {
    fn from(e: DesktopError) -> Self {
        let msg = e.to_string();
        match e {
            DesktopError::UnknownTheme(_) => UqError::UnknownTheme(msg),
            DesktopError::InvalidLayout(_) => UqError::InvalidLayout(msg),
            DesktopError::EmptyWorkspace => UqError::EmptyWorkspace(msg),
            DesktopError::InvalidSchedule(_) => UqError::InvalidSchedule(msg),
            DesktopError::InvalidSetting(_) => UqError::InvalidSetting(msg),
            DesktopError::ConfigInvalid(_) => UqError::ConfigInvalid(msg),
            DesktopError::Io(_) => UqError::Io(msg),
            DesktopError::InvalidArgument(_) => UqError::InvalidArgument(msg),
            DesktopError::Conflict(_) => UqError::Conflict(msg),
        }
    }
}
