//! The theme editor's logic, as plain functions (no uqd): the editing session, the live preview
//! and the `.uqtheme` text a save sends. Seven base colours per variant are edited; the rest of
//! the palette is derived (`uq_core::theme::derive`), and everything else is copied from the
//! theme the editor was opened from.

#![cfg_attr(not(test), allow(dead_code, reason = "wired by its model in Task 8"))]

use serde_json::{Value, json};
use uq_core::theme::derive::{self, BaseColors, KEYS};
use uq_core::theme::uqtheme::{self, ImportError};
use uq_core::theme::user;
use uq_core::theme::{Color, Theme};

/// What a colour field that isn't a colour says.
pub const BAD_COLOUR: &str = "Use a colour like #1e2a3b";

const AUTHOR_PROBLEM: &str = "The author's name can't be used.";

/// An open editor: the theme it started from and what is being made of it.
#[derive(Debug, Clone)]
pub struct Session {
    /// The theme the editor was opened from.
    pub base: String,
    /// Editing one of the user's themes (a save replaces it) rather than making a new one.
    pub edit: bool,
    /// `user-<slug>` when editing, empty for a new theme.
    pub id: String,
    pub name: String,
    pub author: String,
    /// The stored variants of the base theme: fonts, icons, sizes and terminal colours come from
    /// them.
    pub dark: Theme,
    pub light: Theme,
    /// The colours the editor opened with.
    pub colors: (BaseColors, BaseColors),
}

impl Session {
    /// A session from the base theme's `.uqtheme` text (`ExportTheme(from, false)`).
    pub fn new(base: &str, edit: bool, parsed: &uqtheme::UqTheme) -> Self {
        let name = if edit {
            parsed.name.clone()
        } else {
            let head: String = parsed.name.chars().take(user::MAX_TEXT_CHARS - 5).collect();
            format!("{} copy", head.trim_end())
        };
        Self {
            base: base.to_string(),
            edit,
            id: if edit {
                format!("{}{}", user::PREFIX, parsed.slug)
            } else {
                String::new()
            },
            name,
            author: if edit {
                parsed.author.clone().unwrap_or_default()
            } else {
                String::new()
            },
            colors: (
                derive::base_of(&parsed.dark),
                derive::base_of(&parsed.light),
            ),
            dark: parsed.dark.clone(),
            light: parsed.light.clone(),
        }
    }

    /// `{base, edit, id, name, author, colors: {dark: {bg…bad}, light: {…}}}`.
    pub fn json(&self) -> Value {
        json!({
            "base": self.base,
            "edit": self.edit,
            "id": self.id,
            "name": self.name,
            "author": self.author,
            "colors": {
                "dark": serde_json::to_value(self.colors.0).unwrap_or(Value::Null),
                "light": serde_json::to_value(self.colors.1).unwrap_or(Value::Null),
            },
        })
    }
}

/// `#rrggbb`, or `#rgb` expanded.
fn parse_hex(text: &str) -> Result<Color, String> {
    let bad = || BAD_COLOUR.to_string();
    let body = text.trim().strip_prefix('#').ok_or_else(bad)?;
    let full: String = match body.chars().count() {
        3 => body.chars().flat_map(|c| [c, c]).collect(),
        6 => body.to_string(),
        _ => return Err(bad()),
    };
    if !full.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(bad());
    }
    format!("#{full}").parse().map_err(|_| bad())
}

fn set(c: &mut BaseColors, key: &str, value: Color) {
    match key {
        "bg" => c.bg = value,
        "surface" => c.surface = value,
        "text" => c.text = value,
        "accent" => c.accent = value,
        "good" => c.good = value,
        "warn" => c.warn = value,
        _ => c.bad = value,
    }
}

/// A variant's colours from the draft; a field that can't be read keeps its value from `keep`
/// and adds `(label, reason)`.
fn read_variant(v: &Value, keep: &BaseColors) -> (BaseColors, Vec<(&'static str, String)>) {
    let mut out = *keep;
    let mut problems = Vec::new();
    for (key, label) in KEYS {
        let parsed = match v.get(key).and_then(Value::as_str) {
            Some(text) => parse_hex(text),
            None => Err(BAD_COLOUR.to_string()),
        };
        match parsed {
            Ok(c) => set(&mut out, key, c),
            Err(reason) => problems.push((label, reason)),
        }
    }
    (out, problems)
}

/// The two variants' base colours from a draft `{dark: {bg…bad}, light: {…}}`. Each value must
/// be `#rrggbb` (`#rgb` is expanded).
pub fn draft_colors(draft: &Value) -> Result<(BaseColors, BaseColors), String> {
    let read = |name: &str, keep: &BaseColors| {
        let (colors, problems) = read_variant(&draft[name], keep);
        match problems.into_iter().next() {
            Some((_, reason)) => Err(reason),
            None => Ok(colors),
        }
    };
    // The values to keep are never used when every field reads.
    let placeholder = BaseColors {
        bg: Color([0; 3]),
        surface: Color([0; 3]),
        text: Color([0; 3]),
        accent: Color([0; 3]),
        good: Color([0; 3]),
        warn: Color([0; 3]),
        bad: Color([0; 3]),
    };
    Ok((read("dark", &placeholder)?, read("light", &placeholder)?))
}

fn draft_name(draft: &Value) -> String {
    draft["name"].as_str().unwrap_or("").trim().to_string()
}

fn draft_author(draft: &Value) -> String {
    draft["author"].as_str().unwrap_or("").trim().to_string()
}

fn name_problem(name: &str) -> String {
    if user::valid_display_text(name) {
        String::new()
    } else {
        ImportError::BadName.to_string()
    }
}

fn author_problem(author: &str) -> String {
    if author.is_empty() || user::valid_display_text(author) {
        String::new()
    } else {
        AUTHOR_PROBLEM.to_string()
    }
}

/// The variants as the draft would make them, and each one's problems.
fn derived(session: &Session, draft: &Value) -> [(Theme, Vec<String>); 2] {
    let name = draft_name(draft);
    [
        ("dark", &session.dark, &session.colors.0),
        ("light", &session.light, &session.colors.1),
    ]
    .map(|(key, base, keep)| {
        let (colors, unread) = read_variant(&draft[key], keep);
        let theme = derive::derive(base, &colors, &name);
        let problems = if unread.is_empty() {
            derive::problems(&theme)
        } else {
            unread
                .into_iter()
                .map(|(label, reason)| format!("{label}: {reason}"))
                .collect()
        };
        (theme, problems)
    })
}

/// `{dark: {tokens, problems}, light: {…}, ok, nameProblem, authorProblem}` for the draft
/// `{name, author, dark: {bg…bad}, light: {…}}`. `tokens` are the derived palette as `#rrggbb`,
/// for the swatches. `ok` is false while anything would stop a save.
pub fn preview(session: &Session, draft: &Value) -> Value {
    let name_problem = name_problem(&draft_name(draft));
    let author_problem = author_problem(&draft_author(draft));
    let [dark, light] = derived(session, draft);
    let ok = name_problem.is_empty()
        && author_problem.is_empty()
        && dark.1.is_empty()
        && light.1.is_empty();
    let side = |(theme, problems): (Theme, Vec<String>)| {
        json!({
            "tokens": serde_json::to_value(&theme.colors).unwrap_or(Value::Null),
            "problems": problems,
        })
    };
    json!({
        "dark": side(dark),
        "light": side(light),
        "ok": ok,
        "nameProblem": name_problem,
        "authorProblem": author_problem,
    })
}

/// The `.uqtheme` text a save sends, and whether it replaces a theme (an edit). A new theme
/// gets a free slug from its name (`taken` holds the user's theme ids, `user-…` or bare slugs);
/// an edit keeps its own. The first problem refuses.
pub fn to_file(
    session: &Session,
    draft: &Value,
    taken: &[String],
) -> Result<(String, bool), String> {
    let name = draft_name(draft);
    let author = draft_author(draft);
    let name_problem = name_problem(&name);
    if !name_problem.is_empty() {
        return Err(name_problem);
    }
    let author_problem = author_problem(&author);
    if !author_problem.is_empty() {
        return Err(author_problem);
    }
    let [(mut dark, dark_problems), (mut light, light_problems)] = derived(session, draft);
    if let Some(first) = dark_problems.into_iter().chain(light_problems).next() {
        return Err(first);
    }
    let slug = if session.edit {
        session
            .id
            .strip_prefix(user::PREFIX)
            .unwrap_or(&session.id)
            .to_string()
    } else {
        let slugs: Vec<String> = taken
            .iter()
            .map(|t| t.strip_prefix(user::PREFIX).unwrap_or(t).to_string())
            .collect();
        user::slug_for(&name, &slugs)
    };
    let id = format!("{}{slug}", user::PREFIX);
    dark.id.clone_from(&id);
    light.id = id;
    let author = (!author.is_empty()).then_some(author.as_str());
    let text = uqtheme::to_text(&uqtheme::from_pair(&dark, &light, author, None));
    // The same checks uqd makes, so a refusal here reads as uqd's would.
    uqtheme::parse(&text).map_err(|e| e.to_string())?;
    Ok((text, session.edit))
}
