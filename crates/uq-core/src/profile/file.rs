//! A profile's TOML file: parsing, writing and reading it.

use super::*;

/// The text of a profile file, read at most `MAX_FILE_BYTES` deep.
pub fn read_file(path: &Path) -> Result<String, ProfileError> {
    let unreadable = |e: std::io::Error| ProfileError::Unreadable(e.to_string());
    let file = std::fs::File::open(path).map_err(unreadable)?;
    if file.metadata().map_err(unreadable)?.len() > MAX_FILE_BYTES {
        return Err(ProfileError::TooBig);
    }
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(unreadable)?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(ProfileError::TooBig);
    }
    String::from_utf8(bytes).map_err(|_| ProfileError::NotAProfile)
}

pub(crate) fn toml_to_json(v: &toml::Value) -> Option<Value> {
    Some(match v {
        toml::Value::String(s) => Value::String(s.clone()),
        toml::Value::Integer(n) => Value::from(*n),
        toml::Value::Float(f) => Value::Number(serde_json::Number::from_f64(*f)?),
        toml::Value::Boolean(b) => Value::Bool(*b),
        toml::Value::Datetime(_) => return None,
        toml::Value::Array(a) => Value::Array(a.iter().map(toml_to_json).collect::<Option<_>>()?),
        toml::Value::Table(t) => Value::Object(
            t.iter()
                .map(|(k, v)| Some((k.clone(), toml_to_json(v)?)))
                .collect::<Option<_>>()?,
        ),
    })
}

pub(crate) fn json_to_toml(v: &Value) -> Option<toml::Value> {
    Some(match v {
        Value::Null => return None,
        Value::Bool(b) => toml::Value::Boolean(*b),
        Value::Number(n) => match n.as_i64() {
            Some(i) => toml::Value::Integer(i),
            None => toml::Value::Float(n.as_f64()?),
        },
        Value::String(s) => toml::Value::String(s.clone()),
        Value::Array(a) => toml::Value::Array(a.iter().filter_map(json_to_toml).collect()),
        Value::Object(m) => toml::Value::Table(
            m.iter()
                .filter_map(|(k, v)| Some((k.clone(), json_to_toml(v)?)))
                .collect(),
        ),
    })
}

impl Profile {
    /// Parse and fully validate profile text.
    pub fn parse(text: &str) -> Result<Profile, ProfileError> {
        if text.len() as u64 > MAX_FILE_BYTES {
            return Err(ProfileError::TooBig);
        }
        let table: toml::Table = text.parse().map_err(|_| ProfileError::NotAProfile)?;
        match table.get("format") {
            Some(toml::Value::Integer(n)) if *n > FORMAT => {
                return Err(ProfileError::NewerFormat(*n));
            }
            Some(toml::Value::Integer(n)) if *n == FORMAT => {}
            _ => return Err(ProfileError::NotAProfile),
        }
        let text_of = |key: &str| match table.get(key) {
            Some(toml::Value::String(s)) => Some(s.clone()),
            _ => None,
        };
        let id = text_of("id").ok_or(ProfileError::BadId)?;
        let name = text_of("name").ok_or(ProfileError::BadName)?;
        let icon = match table.get("icon") {
            None => None,
            Some(toml::Value::String(s)) => Some(s.clone()),
            Some(_) => return Err(ProfileError::BadIcon),
        };
        let mut values = BTreeMap::new();
        for (key, v) in &table {
            if matches!(key.as_str(), "format" | "id" | "name" | "icon") {
                continue;
            }
            let json = toml_to_json(v).ok_or_else(|| invalid(key.clone(), "can't be used"))?;
            if json.as_object().is_some_and(Map::is_empty) {
                continue;
            }
            values.insert(key.clone(), json);
        }
        // The launcher's shortcut was `uq.spotlight` before it became look=see>do_.
        if let Some(Value::Object(kb)) = values.get_mut("keybinds")
            && let Some(on) = kb.remove("uq.spotlight")
        {
            kb.entry("uq.lookseedo").or_insert(on);
        }
        let profile = Profile {
            id,
            name,
            icon,
            values,
        };
        profile.validate()?;
        Ok(profile)
    }

    /// The file text: `HEADER`, `format`, `id`, `name`, `icon`, then the domains in order.
    pub fn to_toml(&self) -> String {
        let mut out = String::from(HEADER);
        let mut scalar = |key: &str, v: toml::Value| {
            let mut t = toml::Table::new();
            t.insert(key.into(), v);
            out.push_str(&toml::to_string(&t).expect("a scalar serialises"));
        };
        scalar("format", toml::Value::Integer(FORMAT));
        scalar("id", toml::Value::String(self.id.clone()));
        scalar("name", toml::Value::String(self.name.clone()));
        if let Some(icon) = &self.icon {
            scalar("icon", toml::Value::String(icon.clone()));
        }
        for (domain, _) in PROFILE_DOMAINS {
            let Some(tree) = self.values.get(domain).and_then(json_to_toml) else {
                continue;
            };
            let mut t = toml::Table::new();
            t.insert(domain.into(), tree);
            out.push('\n');
            out.push_str(&toml::to_string(&t).expect("a table serialises"));
        }
        out
    }
}
