//! The words people read for printer and job states, reasons and connections, and queue names.

/// What a printer is doing, in words: `(state key, status words)`. CUPS drops the `paused`
/// reason when a paused printer's settings change, but keeps its "Paused" message.
pub(crate) fn state_words(
    state: i32,
    reasons: &[String],
    message: &str,
) -> (&'static str, &'static str) {
    let paused_message = message
        .trim()
        .trim_end_matches('.')
        .eq_ignore_ascii_case("paused");
    match state {
        4 => ("printing", "Printing"),
        5 if paused_message || reasons.iter().any(|r| r == "paused") => ("paused", "Paused"),
        5 => ("stopped", "Stopped"),
        _ => ("idle", "Ready"),
    }
}

/// A printer's reason (`media-empty-error`) in words; `None` for ones not worth showing.
pub fn reason_words(reason: &str) -> Option<&'static str> {
    let r = reason
        .strip_suffix("-error")
        .or_else(|| reason.strip_suffix("-warning"))
        .or_else(|| reason.strip_suffix("-report"))
        .unwrap_or(reason);
    Some(match r {
        "offline" | "shutdown" | "timed-out" => "The printer can't be reached",
        "connecting-to-device" => "Connecting to the printer",
        "media-empty" | "media-needed" => "Out of paper",
        "media-low" => "Paper is low",
        "media-jam" => "Paper jam",
        "toner-low" | "marker-supply-low" => "Ink or toner is low",
        "toner-empty" | "marker-supply-empty" => "Out of ink or toner",
        "door-open" | "cover-open" | "interlock-open" => "A cover or door is open",
        "output-area-full" => "The output tray is full",
        "spool-area-full" => "Too many documents are waiting",
        "cups-missing-filter" => "Part of its driver is missing",
        "cups-insecure-filter" => "Part of its driver was refused as unsafe",
        _ => return None,
    })
}

/// How a printer is connected, in words, from its address.
pub fn connection_words(uri: &str) -> &'static str {
    let scheme = uri.split(':').next().unwrap_or("");
    match scheme {
        "usb" | "hp" | "hpfax" => "USB",
        "cups-pdf" => "Saves PDF files",
        "file" => "Saves to a file",
        "ipp" | "ipps" | "http" | "https" | "dnssd" | "socket" | "lpd" | "smb"
        | "implicitclass" => {
            if uri.contains("://localhost:6") {
                // ipp-usb serves USB printers on localhost ports from 60000.
                "USB"
            } else {
                "Network"
            }
        }
        "" => "Not set",
        _ => "Other",
    }
}

/// Whether a printer at `uri` can be added without a driver (IPP Everywhere).
pub fn driverless(uri: &str) -> bool {
    let scheme = uri.split(':').next().unwrap_or("");
    match scheme {
        "ipp" | "ipps" => true,
        "dnssd" => uri.contains("._ipp.") || uri.contains("._ipps."),
        _ => false,
    }
}

/// A queue name for `name`: CUPS takes up to 127 printable characters other than space, `/`,
/// `\`, `#`, `?` and quotes; anything else becomes `_`. Not one of `taken` (case ignored).
pub fn queue_name(name: &str, taken: &[String]) -> String {
    let mut base: String = name
        .trim()
        .chars()
        .map(|c| {
            if c.is_ascii_graphic() && !"/\\#?'\"".contains(c) {
                c
            } else {
                '_'
            }
        })
        .take(100)
        .collect();
    while base.contains("__") {
        base = base.replace("__", "_");
    }
    let base = base.trim_matches('_').to_string();
    let base = if base.is_empty() {
        "Printer".to_string()
    } else {
        base
    };
    let free = |n: &str| !taken.iter().any(|t| t.eq_ignore_ascii_case(n));
    if free(&base) {
        return base;
    }
    (2..)
        .map(|i| format!("{base}_{i}"))
        .find(|n| free(n))
        .expect("an unused name")
}

pub(crate) fn job_state_words(state: i32) -> &'static str {
    match state {
        3 => "Waiting",
        4 => "On hold",
        5 => "Printing",
        6 => "Stopped",
        7 => "Cancelled",
        8 => "Failed",
        9 => "Done",
        _ => "",
    }
}
