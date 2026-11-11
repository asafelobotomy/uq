//! "Rules from apps" (Phase 4 spec §3.3): window rules that installed apps ship, each off until
//! turned on here. uqd lists them (`ListIntegrationRules`) and owns the check: a rule that is
//! missing or broken is refused, and the switch shows the list as uqd has it again.

use serde_json::{Map, Value, json};
use uq_core::client::{self, DesktopProxy};

use crate::pages::runner::parse_json;

use super::failure;

/// `{rules, problems, missing}` from uqd; all empty when uqd is too old to list them.
pub(super) async fn read(proxy: &DesktopProxy<'_>) -> zbus::Result<Value> {
    let empty = || json!({"rules": [], "problems": [], "missing": []});
    match proxy.list_integration_rules().await {
        Ok(text) => {
            let v = parse_json(&text)?;
            Ok(if v.is_object() { v } else { empty() })
        }
        Err(e) if client::no_owner(&e) => Err(e),
        Err(_) => Ok(empty()),
    }
}

/// Turn `id` on or off in the `integration_rules` section (`{"enabled": [...]}`).
pub(super) fn set(section: &mut Map<String, Value>, id: &str, on: bool) -> zbus::Result<()> {
    let list = section
        .entry("enabled")
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .ok_or_else(|| failure("the rules from apps aren't a list"))?;
    let here = list.iter().any(|v| v == id);
    if on && !here {
        list.push(json!(id));
    } else if !on {
        list.retain(|v| v != id);
    }
    Ok(())
}
