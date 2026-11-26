//! Search entries for the Devices and Connections pages.

use super::*;

pub(crate) static DISPLAYS_SETTINGS: [SearchEntry; 14] = [
    entry(
        "display-arrangement",
        "Display arrangement",
        &["monitor", "screen", "position", "layout", "left", "right"],
    ),
    entry(
        "display-resolution",
        "Resolution",
        &["monitor", "screen", "size", "pixels", "4k"],
    ),
    entry(
        "display-refresh",
        "Refresh rate",
        &["monitor", "screen", "hz", "frame rate", "144"],
    ),
    entry(
        "display-scale",
        "Display scale",
        &["monitor", "screen", "zoom", "hidpi", "bigger", "smaller"],
    ),
    entry(
        "display-rotation",
        "Rotation",
        &[
            "monitor",
            "screen",
            "portrait",
            "landscape",
            "rotate",
            "flip",
        ],
    ),
    entry(
        "display-enabled",
        "Use this display",
        &["monitor", "screen", "turn off", "disable", "on"],
    ),
    entry(
        "display-mirror",
        "Mirror a display",
        &[
            "monitor",
            "screen",
            "duplicate",
            "same picture",
            "projector",
        ],
    ),
    entry(
        "display-vrr",
        "Variable refresh rate",
        &[
            "monitor",
            "screen",
            "vrr",
            "freesync",
            "g-sync",
            "adaptive sync",
            "hz",
        ],
    ),
    entry(
        "display-main",
        "Main display",
        &["main", "primary", "notifications"],
    ),
    entry(
        "display-bar",
        "Show the bar on",
        &["bar", "panel", "screens"],
    ),
    entry(
        "display-arrangements-auto",
        "Switch arrangements when screens change",
        &["profile", "dock", "hotplug", "automatic"],
    ),
    entry(
        "display-remember",
        "Remember for these screens",
        &["save", "profile", "layout"],
    ),
    entry(
        "display-workspaces",
        "Workspaces across screens",
        &["workspace", "monitor", "planner"],
    ),
    entry(
        "display-workspaces-back",
        "Move workspaces back when a screen returns",
        &["workspace", "return", "dock"],
    ),
];

pub(crate) static INPUT_SETTINGS: [SearchEntry; 19] = [
    entry(
        "keyboard-layouts",
        "Keyboard layouts",
        &[
            "keyboard", "language", "layout", "qwerty", "azerty", "dvorak",
        ],
    ),
    entry(
        "layout-switch",
        "Switch layouts with",
        &["keyboard", "language", "shortcut", "alt shift", "toggle"],
    ),
    entry(
        "repeat-delay",
        "Delay before keys repeat",
        &["keyboard", "repeat", "hold", "delay"],
    ),
    entry(
        "repeat-rate",
        "Key repeat speed",
        &["keyboard", "repeat", "rate", "typing"],
    ),
    entry(
        "mouse-speed",
        "Pointer speed",
        &["mouse", "sensitivity", "cursor", "faster", "slower"],
    ),
    entry(
        "mouse-accel",
        "Pointer acceleration",
        &["mouse", "acceleration", "flat", "adaptive", "precision"],
    ),
    entry(
        "mouse-natural-scroll",
        "Natural scrolling",
        &["mouse", "scroll direction", "reverse", "wheel"],
    ),
    entry(
        "mouse-left-handed",
        "Left-handed buttons",
        &["mouse", "swap buttons", "left hand"],
    ),
    entry(
        "mouse-scroll-speed",
        "Scroll speed",
        &["mouse", "wheel", "scrolling", "faster"],
    ),
    entry(
        "touchpad-natural-scroll",
        "Natural scrolling on the touchpad",
        &["touchpad", "trackpad", "scroll direction", "reverse"],
    ),
    entry(
        "tap-to-click",
        "Tap to click",
        &["touchpad", "trackpad", "tap", "click"],
    ),
    entry(
        "disable-while-typing",
        "Turn the touchpad off while typing",
        &["touchpad", "trackpad", "palm", "typing"],
    ),
    entry(
        "touchpad-scroll-speed",
        "Touchpad scroll speed",
        &["touchpad", "trackpad", "scrolling", "two finger"],
    ),
    entry(
        "device-overrides",
        "Settings for one pointing device",
        &["mouse", "touchpad", "trackball", "per device", "device"],
    ),
    entry(
        "gestures",
        "Touchpad gestures",
        &[
            "gesture",
            "swipe",
            "pinch",
            "fingers",
            "three fingers",
            "four fingers",
            "touchpad",
            "trackpad",
        ],
    ),
    entry(
        "gesture-swipe-distance",
        "Swipe distance",
        &["swipe", "gesture", "workspace", "how far", "sensitivity"],
    ),
    entry(
        "gesture-swipe-invert",
        "Reverse swipe direction",
        &[
            "swipe",
            "gesture",
            "natural",
            "invert",
            "reverse",
            "direction",
        ],
    ),
    entry(
        "gesture-swipe-create",
        "Swipe past the last workspace to make a new one",
        &[
            "swipe",
            "gesture",
            "new workspace",
            "create",
            "empty workspace",
        ],
    ),
    entry(
        "gesture-swipe-cancel",
        "How far to swipe before it switches",
        &["swipe", "gesture", "cancel", "threshold", "halfway"],
    ),
];

pub(crate) static SOUND_SETTINGS: [SearchEntry; 4] = [
    entry(
        "sound-output",
        "Play sound through",
        &[
            "speakers",
            "headphones",
            "volume",
            "mute",
            "output",
            "test sound",
        ],
    ),
    entry(
        "sound-input",
        "Record sound from",
        &["microphone", "mic", "input", "recording", "volume"],
    ),
    entry(
        "sound-apps",
        "App volume",
        &["app volume", "mixer", "per app", "playing"],
    ),
    entry(
        "sound-profiles",
        "Sound card modes",
        &["profile", "sound card", "hdmi", "duplex"],
    ),
];

pub(crate) static BLUETOOTH_SETTINGS: [SearchEntry; 4] = [
    entry(
        "bluetooth-power",
        "Bluetooth",
        &["bluetooth", "wireless", "turn on", "turn off"],
    ),
    entry(
        "bluetooth-visible",
        "Let other devices find this computer",
        &["discoverable", "visible", "find", "bluetooth name"],
    ),
    entry(
        "bluetooth-devices",
        "Your Bluetooth devices",
        &[
            "paired",
            "headphones",
            "connect",
            "battery",
            "forget",
            "trust",
        ],
    ),
    entry(
        "bluetooth-pair",
        "Pair a new device",
        &["pair", "add device", "scan", "keyboard", "mouse", "speaker"],
    ),
];

pub(crate) static PRINTERS_SETTINGS: [SearchEntry; 4] = [
    entry(
        "printers-list",
        "Your printers",
        &[
            "printer",
            "printing",
            "default printer",
            "pause",
            "remove printer",
            "cups",
        ],
    ),
    entry(
        "printers-queue",
        "Documents waiting to print",
        &["print queue", "jobs", "cancel printing", "stuck"],
    ),
    entry(
        "printers-test",
        "Print a test page",
        &["test page", "check printer"],
    ),
    entry(
        "printers-add",
        "Add a printer",
        &[
            "add printer",
            "network printer",
            "usb printer",
            "driver",
            "scan",
            "pdf printer",
        ],
    ),
];

pub(crate) static NETWORK_SETTINGS: [SearchEntry; 8] = [
    entry(
        "wifi",
        "Wi-Fi",
        &["wireless", "wlan", "internet", "join", "password"],
    ),
    entry(
        "hidden-network",
        "Join a hidden network",
        &["hidden", "ssid", "not listed"],
    ),
    entry(
        "saved-connections",
        "Saved connections",
        &["forget", "known networks", "connect automatically"],
    ),
    entry("wired", "Wired network", &["ethernet", "cable", "lan"]),
    entry(
        "vpn",
        "VPN",
        &[
            "wireguard",
            "openvpn",
            "tunnel",
            "import",
            "private network",
        ],
    ),
    entry(
        "hotspot",
        "Hotspot",
        &["share connection", "tethering", "access point"],
    ),
    entry(
        "ip-settings",
        "Addresses and DNS",
        &["ip", "dns", "static", "manual address", "gateway", "ipv6"],
    ),
    entry(
        "metered",
        "Metered connection",
        &["data limit", "mobile data", "data plan"],
    ),
];
