//! Each island's options, and the bar's options as a whole.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DateStyle {
    None,
    #[default]
    Short,
    Long,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ClockOptions {
    pub seconds: bool,
    pub date: DateStyle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct StocksOptions {
    /// Ticker symbols as the quote source spells them (`AAPL`, `^GSPC`, `BTC-USD`, `VOD.L`).
    pub symbols: Vec<String>,
}

impl Default for StocksOptions {
    fn default() -> Self {
        Self {
            symbols: vec!["^GSPC".into(), "AAPL".into(), "MSFT".into()],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct NewsOptions {
    /// RSS or Atom feed URLs, https only. Empty until the user picks some.
    pub feeds: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WorldClockOptions {
    /// IANA time zone names (`America/New_York`).
    pub zones: Vec<String>,
}

/// How the Open windows island orders its pills.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WindowSort {
    /// Where the windows sit on screen: left to right, as the scrolling layout's columns run.
    /// Minimised windows keep their place. One order (`descending` is ignored).
    #[default]
    Screen,
    /// When each window opened (oldest first unless `descending`).
    Opened,
    /// The last notification from each window's app.
    Activity,
    /// Pinned apps in pin order, then the rest by when they opened.
    Pin,
    /// Processor and memory use of each window's process.
    Usage,
    /// The app's name, then the title.
    Name,
}

/// What a tab's close button (X) does to an open window. A minimised window's X always closes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CloseButton {
    /// Ask ("Minimize to Tab?"), where the answer can become `minimize` or `close`.
    #[default]
    Ask,
    /// Minimise the window to its tab.
    Minimize,
    /// Close the window.
    Close,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WorkOptions {
    pub sort: WindowSort,
    /// Reverse the order (newest, most active, busiest or Z first). Ignored for `screen` and
    /// `pin`.
    pub descending: bool,
    /// What a tab's X does to an open window.
    pub close: CloseButton,
}

/// Which readings the System monitor island shows (its popup shows them all).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SysMonOptions {
    pub cpu: bool,
    pub mem: bool,
    /// Graphics card use. Off, the bar never asks the graphics driver.
    pub gpu: bool,
    pub temp: bool,
}

impl Default for SysMonOptions {
    fn default() -> Self {
        Self {
            cpu: true,
            mem: true,
            gpu: true,
            temp: true,
        }
    }
}

/// The most the Storage cleanup island's amber mark can wait for, in GB.
pub const MAX_WARN_GB: u32 = 500;

/// The Storage cleanup island (crumBS): when the space that can be reclaimed counts as a lot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CrumbsOptions {
    /// The island turns amber at this much reclaimable space, in GB; 0 never turns amber.
    pub warn_gb: u32,
}

impl Default for CrumbsOptions {
    fn default() -> Self {
        Self { warn_gb: 5 }
    }
}

/// Every island at once.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AllOptions {
    /// The text style of every island that has none of its own (`text` on its item); the
    /// theme's by default.
    #[serde(skip_serializing_if = "TextStyle::is_default")]
    pub text: TextStyle,
}

/// The Security island (oysterAV).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SecurityOptions {
    /// Send a notification when the state gets worse.
    pub notify: bool,
}

impl Default for SecurityOptions {
    fn default() -> Self {
        Self { notify: true }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BarOptions {
    pub all: AllOptions,
    pub clock: ClockOptions,
    pub stocks: StocksOptions,
    pub news: NewsOptions,
    pub worldclock: WorldClockOptions,
    pub work: WorkOptions,
    pub sysmon: SysMonOptions,
    pub crumbs: CrumbsOptions,
    pub security: SecurityOptions,
}

impl BarOptions {
    pub const FIELDS: [&str; 9] = [
        "all",
        "clock",
        "stocks",
        "news",
        "worldclock",
        "work",
        "sysmon",
        "crumbs",
        "security",
    ];
}
