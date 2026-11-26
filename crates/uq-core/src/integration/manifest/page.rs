//! `page.toml`: a Settings page's manifest.

use super::*;

/// A searchable `[[setting]]` row of a page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageSetting {
    pub id: String,
    pub label: String,
    pub keywords: Vec<String>,
}

/// A valid `page.toml` (Phase 3 §2.5 keys, plus `uq_api`, `[requires]`, `[dbus]` and
/// `[[command]]` from Phase 4 §2.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageManifest {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub qml: String,
    /// Phase 3's `dbus_name`: the one bus name the page's `dbus` helper reaches. Set from
    /// `[dbus].name` when only the table is given.
    pub dbus_name: Option<String>,
    pub keywords: Vec<String>,
    pub settings: Vec<PageSetting>,
    pub uq_api: u32,
    pub requires: Requires,
    pub dbus: Option<DbusSpec>,
    pub commands: Vec<CommandSpec>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawPage {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) icon: Option<String>,
    pub(crate) category: Option<String>,
    pub(crate) qml: String,
    pub(crate) dbus_name: Option<String>,
    #[serde(default)]
    pub(crate) keywords: Vec<String>,
    #[serde(default)]
    pub(crate) setting: Vec<RawSetting>,
    pub(crate) uq_api: Option<u32>,
    pub(crate) requires: Option<RawRequires>,
    pub(crate) dbus: Option<RawDbus>,
    #[serde(default)]
    pub(crate) command: Vec<RawCommand>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawSetting {
    pub(crate) id: String,
    pub(crate) label: String,
    #[serde(default)]
    pub(crate) keywords: Vec<String>,
}

/// Parse and validate a `page.toml` (the folder name is checked by [`load_page`]).
pub fn parse_page(text: &str) -> Result<PageManifest, ManifestError> {
    const FILE: &str = "page.toml";
    check_api(FILE, text, false)?;
    let m: RawPage = strict(FILE, text)?;
    if !valid_id(&m.id) {
        return Err(invalid(format!(
            "id {:?} must match [a-z0-9-]{{1,40}}",
            m.id
        )));
    }
    if let Some(c) = &m.category
        && c != "apps"
    {
        return Err(invalid(format!("category must be \"apps\", not {c:?}")));
    }
    check_name(&m.name)?;
    if let Some(icon) = &m.icon
        && !valid_icon_name(icon)
    {
        return Err(invalid(format!(
            "icon {icon:?} must be an icon name matching [A-Za-z0-9._-]{{1,64}}"
        )));
    }
    check_qml(&m.qml)?;
    if let Some(n) = &m.dbus_name
        && !valid_bus_name(n)
    {
        return Err(invalid(format!("dbus_name {n:?} is not a valid bus name")));
    }
    if !valid_keywords(&m.keywords) {
        return Err(invalid("keywords: at most 32, each 1-64 characters"));
    }
    if m.setting.len() > MAX_SETTINGS {
        return Err(invalid(format!(
            "at most {MAX_SETTINGS} [[setting]] entries"
        )));
    }
    let mut settings: Vec<PageSetting> = Vec::new();
    for s in m.setting {
        if !valid_setting_id(&s.id) {
            return Err(invalid(format!(
                "setting id {:?} must match [A-Za-z0-9._-]{{1,64}}",
                s.id
            )));
        }
        if settings.iter().any(|o| o.id == s.id) {
            return Err(invalid(format!("duplicate setting id {:?}", s.id)));
        }
        if !valid_text(&s.label, 200) || !valid_keywords(&s.keywords) {
            return Err(invalid(format!(
                "setting {:?}: bad label or keywords",
                s.id
            )));
        }
        settings.push(PageSetting {
            id: s.id,
            label: s.label,
            keywords: s.keywords,
        });
    }
    let requires = requires_from(m.requires)?;
    let dbus = dbus_from(m.dbus)?;
    check_requires_bus(&requires, dbus.as_ref())?;
    let dbus_name = match (m.dbus_name, &dbus) {
        (Some(n), Some(d)) if n != d.name => {
            return Err(invalid(format!(
                "dbus_name {n:?} and dbus.name {:?} differ",
                d.name
            )));
        }
        (n, d) => n.or_else(|| d.as_ref().map(|d| d.name.clone())),
    };
    let commands = commands_from(m.command)?;
    check_stream_api(&commands, m.uq_api.unwrap_or(PAGE_DEFAULT_API))?;
    Ok(PageManifest {
        commands,
        id: m.id,
        name: m.name,
        icon: m.icon,
        qml: m.qml,
        dbus_name,
        keywords: m.keywords,
        settings,
        uq_api: m.uq_api.unwrap_or(PAGE_DEFAULT_API),
        requires,
        dbus,
    })
}
