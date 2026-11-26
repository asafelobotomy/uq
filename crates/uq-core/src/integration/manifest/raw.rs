//! The raw `[requires]`, `[dbus]` and `[[command]]` tables, and their checks into specs.

use super::*;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawRequires {
    pub(crate) bus_name: Option<String>,
    #[serde(default)]
    pub(crate) bins: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawDbus {
    pub(crate) name: String,
    pub(crate) path: String,
    pub(crate) interface: String,
    #[serde(default)]
    pub(crate) methods: Vec<String>,
    #[serde(default)]
    pub(crate) properties: Vec<String>,
    #[serde(default)]
    pub(crate) signals: Vec<String>,
    #[serde(default)]
    pub(crate) bus: Bus,
    #[serde(default)]
    pub(crate) timeout_ms: BTreeMap<String, u64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawArg {
    pub(crate) name: String,
    pub(crate) pattern: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawCommand {
    pub(crate) id: String,
    pub(crate) argv: Vec<String>,
    pub(crate) output: Output,
    pub(crate) timeout_ms: Option<u64>,
    #[serde(default)]
    pub(crate) args: Vec<RawArg>,
    pub(crate) confirm: Option<String>,
    pub(crate) preview: Option<String>,
    pub(crate) watch: Option<RawWatch>,
}

pub(crate) fn requires_from(raw: Option<RawRequires>) -> Result<Requires, ManifestError> {
    let Some(raw) = raw else {
        return Ok(Requires::default());
    };
    if let Some(n) = &raw.bus_name
        && !valid_bus_name(n)
    {
        return Err(invalid(format!(
            "requires.bus_name {n:?} is not a valid bus name"
        )));
    }
    if raw.bins.len() > MAX_BINS {
        return Err(invalid(format!("requires.bins: at most {MAX_BINS}")));
    }
    if let Some(b) = raw.bins.iter().find(|b| !valid_bin_name(b)) {
        return Err(invalid(format!(
            "requires.bins: {b:?} must be a program name in /usr/bin, not a path"
        )));
    }
    Ok(Requires {
        bus_name: raw.bus_name,
        bins: raw.bins,
    })
}

pub(crate) fn members(kind: &str, list: Vec<String>) -> Result<Vec<String>, ManifestError> {
    if list.len() > MAX_MEMBERS {
        return Err(invalid(format!("dbus.{kind}: at most {MAX_MEMBERS}")));
    }
    for (i, m) in list.iter().enumerate() {
        if MemberName::try_from(m.as_str()).is_err() {
            return Err(invalid(format!("dbus.{kind}: {m:?} is not a member name")));
        }
        if list[..i].contains(m) {
            return Err(invalid(format!("dbus.{kind}: {m:?} is listed twice")));
        }
    }
    Ok(list)
}

pub(crate) fn dbus_from(raw: Option<RawDbus>) -> Result<Option<DbusSpec>, ManifestError> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    if !valid_bus_name(&raw.name) {
        return Err(invalid(format!(
            "dbus.name {:?} is not a valid bus name",
            raw.name
        )));
    }
    if ObjectPath::try_from(raw.path.as_str()).is_err() {
        return Err(invalid(format!(
            "dbus.path {:?} is not an object path",
            raw.path
        )));
    }
    if InterfaceName::try_from(raw.interface.as_str()).is_err() {
        return Err(invalid(format!(
            "dbus.interface {:?} is not an interface name",
            raw.interface
        )));
    }
    let methods = members("methods", raw.methods)?;
    for (method, ms) in &raw.timeout_ms {
        if !methods.contains(method) {
            return Err(invalid(format!(
                "dbus.timeout_ms: {method:?} is not listed in dbus.methods"
            )));
        }
        if !(1..=MAX_DBUS_TIMEOUT_MS).contains(ms) {
            return Err(invalid(format!(
                "dbus.timeout_ms: {method:?} must be 1 to {MAX_DBUS_TIMEOUT_MS}, not {ms}"
            )));
        }
    }
    Ok(Some(DbusSpec {
        methods,
        properties: members("properties", raw.properties)?,
        signals: members("signals", raw.signals)?,
        name: raw.name,
        path: raw.path,
        interface: raw.interface,
        bus: raw.bus,
        timeout_ms: raw.timeout_ms,
    }))
}

/// Validate one `[[command]]` table against the allowed `argv[0]` prefixes.
pub(crate) fn command_from(raw: RawCommand, prefixes: &[PathBuf]) -> Result<CommandSpec, String> {
    let id = raw.id;
    if !valid_command_id(&id) {
        return Err(format!("command id {id:?} must match [a-z0-9_-]{{1,40}}"));
    }
    let what = |m: &str| format!("command {id:?}: {m}");
    if raw.argv.is_empty() || raw.argv.len() > MAX_ARGV {
        return Err(what(&format!("argv must have 1 to {MAX_ARGV} entries")));
    }
    if raw
        .argv
        .iter()
        .any(|a| a.len() > MAX_ARG_BYTES || a.contains('\0'))
    {
        return Err(what("an argv entry is too long or holds a NUL"));
    }
    if !path_under(&raw.argv[0], prefixes) {
        return Err(what(&format!(
            "argv[0] {:?} must be an absolute path under {}",
            raw.argv[0],
            prefixes
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(" or ")
        )));
    }
    let timeout_ms = raw.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS);
    if !(1..=MAX_TIMEOUT_MS).contains(&timeout_ms) {
        return Err(what(&format!(
            "timeout_ms must be 1 to {MAX_TIMEOUT_MS}, not {timeout_ms}"
        )));
    }
    if raw.args.len() > MAX_DECLARED_ARGS {
        return Err(what(&format!("at most {MAX_DECLARED_ARGS} args")));
    }
    let mut args: Vec<ArgSpec> = Vec::new();
    for a in raw.args {
        if args.iter().any(|o| o.name == a.name) {
            return Err(what(&format!("argument {:?} is declared twice", a.name)));
        }
        args.push(ArgSpec::new(&a.name, &a.pattern).map_err(|e| what(&e))?);
    }
    if let Some(text) = &raw.confirm {
        if !valid_text(text, MAX_CONFIRM_CHARS) {
            return Err(what(&format!(
                "confirm must be 1-{MAX_CONFIRM_CHARS} characters without control characters"
            )));
        }
        let placeholders = text.matches("%s").count();
        if placeholders > 1 || text.replacen("%s", "", 1).contains('%') {
            return Err(what(
                "confirm may hold one %s (the first argument) and no other %",
            ));
        }
        if placeholders == 1 && args.is_empty() {
            return Err(what("confirm has %s but the command declares no args"));
        }
    }
    if let Some(p) = &raw.preview {
        if !valid_command_id(p) {
            return Err(what(&format!(
                "preview {p:?} must match [a-z0-9_-]{{1,40}}"
            )));
        }
        if *p == id {
            return Err(what("preview can't be the command itself"));
        }
        if raw.confirm.is_none() {
            return Err(what("preview needs a confirm question to show it under"));
        }
    }
    let watch = match raw.watch {
        None => None,
        Some(w) => {
            Some(watch_from(w, !args.is_empty(), raw.confirm.is_some()).map_err(|e| what(&e))?)
        }
    };
    Ok(CommandSpec {
        id,
        argv: raw.argv,
        output: raw.output,
        timeout_ms,
        args,
        confirm: raw.confirm,
        preview: raw.preview,
        watch,
    })
}

pub(crate) fn commands_from(raw: Vec<RawCommand>) -> Result<Vec<CommandSpec>, ManifestError> {
    if raw.len() > MAX_COMMANDS {
        return Err(invalid(format!(
            "at most {MAX_COMMANDS} [[command]] tables"
        )));
    }
    let prefixes = command_prefixes();
    let mut out: Vec<CommandSpec> = Vec::new();
    for c in raw {
        let c = command_from(c, &prefixes).map_err(invalid)?;
        if out.iter().any(|o| o.id == c.id) {
            return Err(invalid(format!("duplicate command id {:?}", c.id)));
        }
        out.push(c);
    }
    check_previews(&out).map_err(invalid)?;
    Ok(out)
}

/// Each `preview` names another command of the page that takes the same arguments, prints JSON,
/// asks nothing and previews nothing itself.
pub(crate) fn check_previews(commands: &[CommandSpec]) -> Result<(), String> {
    for c in commands {
        let Some(pid) = &c.preview else { continue };
        let what = |m: &str| format!("command {:?}: preview {pid:?} {m}", c.id);
        let Some(p) = commands.iter().find(|o| &o.id == pid) else {
            return Err(what("is not a declared command"));
        };
        if p.args != c.args {
            return Err(what("must declare the same args"));
        }
        if p.output != Output::Json {
            return Err(what("must have output = \"json\""));
        }
        if p.confirm.is_some() || p.preview.is_some() {
            return Err(what("can't have confirm or preview of its own"));
        }
    }
    Ok(())
}

/// A page below [`STREAM_API`] that uses streamed output or a previewed confirmation: an older
/// uQ would run it wrongly, so it is refused with a reason instead.
pub(crate) fn check_stream_api(commands: &[CommandSpec], api: u32) -> Result<(), ManifestError> {
    if api >= STREAM_API {
        return Ok(());
    }
    for c in commands {
        if c.output == Output::Ndjson {
            return Err(invalid(format!(
                "command {:?}: output = \"ndjson\" needs uq_api = {STREAM_API} (page.toml says {api})",
                c.id
            )));
        }
        if c.preview.is_some() {
            return Err(invalid(format!(
                "command {:?}: preview needs uq_api = {STREAM_API} (page.toml says {api})",
                c.id
            )));
        }
    }
    Ok(())
}

/// Check `uq_api` before anything else. `required`: the key must be present (widgets); pages
/// predate it and default to 1.
pub(crate) fn check_api(file: &str, text: &str, required: bool) -> Result<(), ManifestError> {
    let table: toml::Table =
        toml::from_str(text).map_err(|e| invalid(format!("{file}: {}", e.message().trim())))?;
    match table.get("uq_api") {
        None if required => Err(invalid(format!(
            "{file}: uq_api is missing (add uq_api = {UQ_API})"
        ))),
        None => Ok(()),
        Some(toml::Value::Integer(n)) if *n > i64::from(UQ_API) => {
            Err(ManifestError::NeedsNewerUq(*n))
        }
        Some(toml::Value::Integer(n)) if *n >= 1 => Ok(()),
        Some(other) => Err(invalid(format!(
            "{file}: uq_api must be a whole number from 1, not {other}"
        ))),
    }
}

pub(crate) fn strict<'de, T: Deserialize<'de>>(
    file: &str,
    text: &'de str,
) -> Result<T, ManifestError> {
    toml::from_str(text).map_err(|e| invalid(format!("{file}: {}", e.message().trim())))
}

pub(crate) fn check_name(name: &str) -> Result<(), ManifestError> {
    if valid_text(name, 100) {
        Ok(())
    } else {
        Err(invalid(
            "name must be 1-100 characters without control characters",
        ))
    }
}

pub(crate) fn check_qml(qml: &str) -> Result<(), ManifestError> {
    if valid_qml_name(qml) {
        Ok(())
    } else {
        Err(invalid(format!(
            "qml {qml:?} must be a bare file name ending .qml"
        )))
    }
}

/// `requires.bus_name` is watched on the `[dbus]` table's bus, which must be the same name.
pub(crate) fn check_requires_bus(
    requires: &Requires,
    dbus: Option<&DbusSpec>,
) -> Result<(), ManifestError> {
    match (&requires.bus_name, dbus) {
        (Some(r), Some(d)) if d.bus == Bus::System && *r != d.name => Err(invalid(
            "requires.bus_name on the system bus must be dbus.name",
        )),
        _ => Ok(()),
    }
}
