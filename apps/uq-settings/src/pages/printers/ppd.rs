//! A printer's options from its driver file (PPD), and setting their defaults the way `lpadmin
//! -o` does: rewrite the `*Default…` lines and give CUPS the file back. Driverless printers have
//! one too (CUPS writes it from what the printer says it can do), so every printer is handled the
//! same way. The file is edited as bytes, line by line: only `*Default` lines change, so text in
//! another encoding passes through untouched.

use std::collections::BTreeMap;

use serde_json::{Value, json};

/// The options most people look for, in the order they are shown; the rest go under "More".
const COMMON: [&str; 7] = [
    "PageSize",
    "Duplex",
    "ColorModel",
    "print-color-mode",
    "cupsPrintQuality",
    "MediaType",
    "InputSlot",
];
/// Options that repeat another one (`PageRegion` is the page size for a different PostScript
/// call) and aren't shown.
const HIDDEN: [&str; 1] = ["PageRegion"];
/// When the page size changes, these follow it (as `lpadmin` does).
const PAGE_SIZE_FOLLOWERS: [&str; 3] = ["PageRegion", "PaperDimension", "ImageableArea"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    pub key: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PpdOption {
    pub key: String,
    pub label: String,
    pub default: String,
    pub choices: Vec<Choice>,
}

impl PpdOption {
    pub fn common(&self) -> bool {
        COMMON.contains(&self.key.as_str())
    }

    pub fn to_json(&self) -> Value {
        json!({
            "key": self.key,
            "label": self.label,
            "default": self.default,
            "common": self.common(),
            "choices": self.choices.iter().map(|c| json!({"key": c.key, "label": c.label})).collect::<Vec<_>>(),
        })
    }
}

/// `keyword/label` → (keyword, label); the label is `None` when the file gives none.
fn split_label(s: &str) -> (String, Option<String>) {
    match s.split_once('/') {
        Some((k, l)) if !l.trim().is_empty() => (k.trim().to_string(), Some(l.trim().to_string())),
        Some((k, _)) => (k.trim().to_string(), None),
        None => (s.trim().to_string(), None),
    }
}

/// Words for options some drivers leave unlabelled (CUPS's own driverless files among them).
fn option_words(key: &str) -> String {
    match key {
        "PageSize" => "Paper size",
        "Duplex" => "Two-sided",
        "ColorModel" | "print-color-mode" => "Color",
        "cupsPrintQuality" => "Print quality",
        "MediaType" => "Paper type",
        "InputSlot" => "Paper source",
        "OutputBin" => "Output tray",
        "cupsFinishingTemplate" => "Finishing",
        _ => key,
    }
    .to_string()
}

/// Words for choices some drivers leave unlabelled.
fn choice_words(key: &str) -> String {
    match key {
        "None" | "False" | "none" => "Off",
        "True" => "On",
        "DuplexNoTumble" => "Both sides, long edge",
        "DuplexTumble" => "Both sides, short edge",
        "Gray" | "Grayscale" => "Black and white",
        "RGB" | "CMYK" => "Color",
        "Auto" => "Automatic",
        "Stationery" => "Plain paper",
        "StationeryLetterhead" => "Letterhead",
        "Main" => "Main tray",
        "Manual" => "Manual feed",
        "ByPassTray" => "Bypass tray",
        "FaceDown" => "Face down",
        "FaceUp" => "Face up",
        _ => key,
    }
    .to_string()
}

/// `en_US.Translation`, `en.PageSize`: an English translation line's (language, rest).
fn english(rest: &str) -> Option<&str> {
    let (lang, after) = rest.split_once('.')?;
    (lang == "en" || lang.starts_with("en_")).then_some(after)
}

/// Every pick-one (or on/off) option the file offers, in file order, with its current default.
pub fn options(ppd: &[u8]) -> Vec<PpdOption> {
    let text = String::from_utf8_lossy(ppd);
    let mut out: Vec<PpdOption> = Vec::new();
    let mut defaults: BTreeMap<String, String> = BTreeMap::new();
    // English translations (`*en_US.Translation Key/Label`, `*en.Key Choice/Label`).
    let mut option_labels: BTreeMap<String, String> = BTreeMap::new();
    let mut choice_labels: BTreeMap<(String, String), String> = BTreeMap::new();
    let mut open: Option<PpdOption> = None;
    for line in text.lines() {
        let line = line.trim_end();
        let Some(rest) = line.strip_prefix('*') else {
            continue;
        };
        if let Some(ui) = rest
            .strip_prefix("OpenUI ")
            .or_else(|| rest.strip_prefix("JCLOpenUI "))
        {
            // `*OpenUI *PageSize/Media Size: PickOne`
            let Some((name, kind)) = ui.rsplit_once(':') else {
                continue;
            };
            let kind = kind.trim();
            if kind != "PickOne" && kind != "Boolean" {
                continue;
            }
            let (key, label) = split_label(name.trim().trim_start_matches('*'));
            open = Some(PpdOption {
                label: label.unwrap_or_default(),
                key,
                default: String::new(),
                choices: Vec::new(),
            });
            continue;
        }
        if rest.starts_with("CloseUI") || rest.starts_with("JCLCloseUI") {
            if let Some(o) = open.take()
                && !HIDDEN.contains(&o.key.as_str())
                && !o.choices.is_empty()
            {
                out.push(o);
            }
            continue;
        }
        if let Some(d) = rest.strip_prefix("Default") {
            if let Some((k, v)) = d.split_once(':') {
                defaults.insert(k.trim().to_string(), v.trim().to_string());
            }
            continue;
        }
        if let Some(t) = english(rest)
            && let Some((head, _)) = t.split_once(':')
        {
            if let Some(name) = head.strip_prefix("Translation ") {
                if let (key, Some(label)) = split_label(name) {
                    option_labels.insert(key, label);
                }
            } else if let Some((key, choice)) = head.split_once(' ')
                && let (choice, Some(label)) = split_label(choice)
            {
                choice_labels.insert((key.to_string(), choice), label);
            }
            continue;
        }
        // A choice: `*PageSize A4/A4: "…"`.
        if let Some(o) = open.as_mut()
            && let Some(after) = rest.strip_prefix(o.key.as_str())
            && let Some(after) = after.strip_prefix(' ')
            && let Some((choice, _)) = after.split_once(':')
        {
            let (key, label) = split_label(choice);
            if !key.is_empty() && !o.choices.iter().any(|c| c.key == key) {
                o.choices.push(Choice {
                    label: label.unwrap_or_default(),
                    key,
                });
            }
        }
    }
    for o in &mut out {
        o.default = defaults.get(&o.key).cloned().unwrap_or_default();
        if o.label.is_empty() {
            o.label = option_labels
                .get(&o.key)
                .cloned()
                .unwrap_or_else(|| option_words(&o.key));
        }
        for c in &mut o.choices {
            if c.label.is_empty() {
                c.label = choice_labels
                    .get(&(o.key.clone(), c.key.clone()))
                    .cloned()
                    .unwrap_or_else(|| choice_words(&c.key));
            }
        }
    }
    // Common ones first, in COMMON's order; the rest as the file has them.
    out.sort_by_key(|o| {
        COMMON
            .iter()
            .position(|c| *c == o.key)
            .unwrap_or(COMMON.len())
    });
    out
}

/// The file with each option of `set` defaulting to its value. An option the file doesn't have,
/// or a value it doesn't offer, is an error naming it.
pub fn with_defaults(ppd: &[u8], set: &BTreeMap<String, String>) -> Result<Vec<u8>, String> {
    let known = options(ppd);
    let mut wanted: BTreeMap<String, String> = BTreeMap::new();
    for (k, v) in set {
        let o = known
            .iter()
            .find(|o| &o.key == k)
            .ok_or_else(|| format!("This printer has no option {k}."))?;
        if !o.choices.iter().any(|c| &c.key == v) {
            return Err(format!("{} can't be set to {v} on this printer.", o.label));
        }
        wanted.insert(k.clone(), v.clone());
        if k == "PageSize" {
            for f in PAGE_SIZE_FOLLOWERS {
                wanted.entry(f.to_string()).or_insert_with(|| v.clone());
            }
        }
    }
    let mut out = Vec::with_capacity(ppd.len());
    for line in ppd.split_inclusive(|b| *b == b'\n') {
        let replaced = line.strip_prefix(b"*Default").and_then(|rest| {
            let colon = rest.iter().position(|b| *b == b':')?;
            let key = std::str::from_utf8(&rest[..colon]).ok()?.trim();
            let value = wanted.get(key)?;
            let ending: &[u8] = if line.ends_with(b"\r\n") {
                b"\r\n"
            } else if line.ends_with(b"\n") {
                b"\n"
            } else {
                b""
            };
            Some([b"*Default", key.as_bytes(), b": ", value.as_bytes(), ending].concat())
        });
        match replaced {
            Some(l) => out.extend_from_slice(&l),
            None => out.extend_from_slice(line),
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PPD: &str = "*PPD-Adobe: \"4.3\"\n\
*% A comment\n\
*OpenGroup: General/General\n\
*OpenUI *PageSize/Media Size: PickOne\n\
*OrderDependency: 10 AnySetup *PageSize\n\
*DefaultPageSize: Letter\n\
*PageSize Letter/US Letter: \"<</PageSize[612 792]>>setpagedevice\"\n\
*PageSize A4/A4: \"<</PageSize[595 842]\n\
>>setpagedevice\"\n\
*End\n\
*CloseUI: *PageSize\n\
*OpenUI *PageRegion: PickOne\n\
*DefaultPageRegion: Letter\n\
*PageRegion Letter/US Letter: \"\"\n\
*PageRegion A4/A4: \"\"\n\
*CloseUI: *PageRegion\n\
*DefaultImageableArea: Letter\r\n\
*DefaultPaperDimension: Letter\n\
*OpenUI *Resolution/Output Resolution: PickOne\n\
*DefaultResolution: 300dpi\n\
*Resolution 300dpi: \"\"\n\
*Resolution 600dpi/Fine: \"\"\n\
*CloseUI: *Resolution\n\
*OpenUI *Duplex/2-Sided Printing: PickOne\n\
*DefaultDuplex: None\n\
*Duplex None/Off: \"\"\n\
*Duplex DuplexNoTumble: \"\"\n\
*CloseUI: *Duplex\n\
*OpenUI *StapleLocation/Staple: PickMany\n\
*StapleLocation Top: \"\"\n\
*CloseUI: *StapleLocation\n\
*CloseGroup: General\n";

    #[test]
    fn reads_options_with_labels_and_defaults() {
        let o = options(PPD.as_bytes());
        let keys: Vec<_> = o.iter().map(|o| o.key.as_str()).collect();
        assert_eq!(
            keys,
            ["PageSize", "Duplex", "Resolution"],
            "common first; PickMany and PageRegion left out"
        );
        assert_eq!(o[0].label, "Media Size");
        assert_eq!(o[0].default, "Letter");
        assert_eq!(
            o[0].choices,
            [
                Choice {
                    key: "Letter".into(),
                    label: "US Letter".into()
                },
                Choice {
                    key: "A4".into(),
                    label: "A4".into()
                }
            ]
        );
        assert_eq!(o[1].choices[1].label, "Both sides, long edge");
        assert_eq!(o[2].choices[0].label, "300dpi");
        assert_eq!(o[1].choices[0].label, "Off");
        assert!(o[0].common() && !o[2].common());
        assert_eq!(o[2].to_json()["choices"][1]["label"], "Fine");
    }

    #[test]
    fn labels_from_translations_or_plain_words() {
        // The way CUPS writes a driverless printer's file.
        let ppd = "*OpenUI *ColorModel: PickOne\n\
*en_US.Translation ColorModel/Color Mode: \"\"\n\
*ColorModel Gray: \"\"\n\
*ColorModel RGB: \"\"\n\
*en.ColorModel RGB/Full color: \"\"\n\
*DefaultColorModel: Gray\n\
*CloseUI: *ColorModel\n\
*OpenUI *MediaType: PickOne\n\
*MediaType Stationery: \"\"\n\
*MediaType Glossy: \"\"\n\
*CloseUI: *MediaType\n\
*fr.Translation MediaType/Papier: \"\"\n";
        let o = options(ppd.as_bytes());
        assert_eq!(o[0].label, "Color Mode");
        assert_eq!(o[0].choices[0].label, "Black and white");
        assert_eq!(o[0].choices[1].label, "Full color");
        assert_eq!(o[1].label, "Paper type", "not the French translation");
        assert_eq!(o[1].choices[1].label, "Glossy");
    }

    #[test]
    fn sets_defaults_and_page_size_followers() {
        let set = BTreeMap::from([
            ("PageSize".to_string(), "A4".to_string()),
            ("Duplex".to_string(), "DuplexNoTumble".to_string()),
        ]);
        let out = String::from_utf8(with_defaults(PPD.as_bytes(), &set).unwrap()).unwrap();
        assert!(out.contains("*DefaultPageSize: A4\n"));
        assert!(out.contains("*DefaultPageRegion: A4\n"));
        assert!(
            out.contains("*DefaultImageableArea: A4\r\n"),
            "line endings kept"
        );
        assert!(out.contains("*DefaultPaperDimension: A4\n"));
        assert!(out.contains("*DefaultDuplex: DuplexNoTumble\n"));
        assert!(
            out.contains("*DefaultResolution: 300dpi\n"),
            "others untouched"
        );
        // Four "Letter"s became "A4" (4 bytes shorter each); "None" became "DuplexNoTumble".
        assert_eq!(out.len(), PPD.len() - 16 + 10);
        let again = options(out.as_bytes());
        assert_eq!(again[0].default, "A4");
    }

    #[test]
    fn refuses_unknown_options_and_values() {
        let bad = BTreeMap::from([("PageSize".to_string(), "A0".to_string())]);
        assert_eq!(
            with_defaults(PPD.as_bytes(), &bad).unwrap_err(),
            "Media Size can't be set to A0 on this printer."
        );
        let bad = BTreeMap::from([("Nope".to_string(), "x".to_string())]);
        assert!(with_defaults(PPD.as_bytes(), &bad).is_err());
    }

    #[test]
    fn other_encodings_pass_through() {
        let mut ppd = PPD.as_bytes().to_vec();
        ppd.extend_from_slice(b"*% caf\xe9\n");
        let set = BTreeMap::from([("Resolution".to_string(), "600dpi".to_string())]);
        let out = with_defaults(&ppd, &set).unwrap();
        assert!(out.ends_with(b"*% caf\xe9\n"));
    }
}
