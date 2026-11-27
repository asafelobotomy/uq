//! One script per action, run step by step so a failed step is named, and the page-hiding
//! and fade helpers it uses.

use super::*;

pub(crate) fn eval_err(e: CompositorError) -> String {
    match e {
        CompositorError::NotRunning => "Hyprland is not running".into(),
        CompositorError::Rejected(msg) => msg,
        e => e.to_string(),
    }
}

/// What the script raises when a step is rejected: Hyprland replies `error: ` and this, then
/// the step's own `level: message` (the same text a lone `eval` of that step replies with).
pub(crate) const STEP_FAILED: &str = "uq-step:";

/// Lua shared by every script: `fail(r)` turns an `hl.dispatch` result into nil (done) or the
/// `level: message` Hyprland would report for it (`{ok = false, level, error, code}`).
pub(crate) const PRELUDE: &str = r#"local function fail(r)
  if type(r) ~= "table" or (r.ok ~= false and r.error == nil) then return nil end
  return tostring(r.level or "error") .. ": " .. tostring(r.error or "dispatch failed")
end
"#;

/// `stmt` (`return hl.dispatch(…)`) as a step of `run` in `script`: stop at a rejection.
pub(crate) fn step(out: &mut String, stmt: &str) {
    let call = stmt.strip_prefix("return ").unwrap_or(stmt);
    out.push_str(&format!(
        "  do local m = fail({call}); if m then return m end end\n"
    ));
}

/// Hyprland's side of `PageHiding`, kept in its Lua state as `uq_pages` between calls: the
/// table from the shell, the windows hidden now per workspace, and a `window.active` handler
/// that applies the table when focus moves. A window is hidden by setting its opacity (focused,
/// unfocused and fullscreen: a Full column is maximized) to 0 and turning off the blur behind it
/// (drawn whatever its opacity), at once rather than faded: the
/// window's animations are off just around the change (so its moves still slide), and Hyprland's
/// focus fade (fadeSwitch) is kept off while focused and unfocused windows look the same, when it
/// shows nothing else (else it is as configured: `uq_pages_fade`, read by `run`).
pub(crate) const PAGE_HIDING_LUA: &str = r#"  uq_pages = uq_pages or { hidden = {} }
  local P = uq_pages
  -- Hidden (v "0.00") or shown ("1.00"). A hidden window has no blur either: Hyprland draws the
  -- blur behind a window whatever its opacity, which tints the empty space while it moves.
  function P.set(a, v)
    local w = "address:" .. a
    local blur = v == "0.00" and "1" or "unset"
    for _, kv in ipairs({ { "no_anim", "1" }, { "opacity", v }, { "opacity_inactive", v },
                          { "opacity_fullscreen", v }, { "no_blur", blur }, { "no_anim", "unset" } }) do
      pcall(hl.dispatch, hl.dsp.window.set_prop({ window = w, prop = kv[1], value = kv[2] }))
    end
  end
  -- Hyprland fades a window's opacity when focus moves (fadeSwitch), and it does so after this
  -- handler has run, so a window hidden at a focus change would show for a moment as it fades.
  -- That fade only shows anything when focused and unfocused windows differ in opacity: while
  -- they don't, it is kept off; when they do, it is as configured (`uq_pages_fade`, read by
  -- `run`; ignored while it is off by our hand, which is what Hyprland would report).
  if not P.fade_off then P.fade = uq_pages_fade end
  local function opt(k) local ok, v = pcall(hl.get_config, k) return ok and tonumber(v) or nil end
  local a, i = opt("decoration:active_opacity"), opt("decoration:inactive_opacity")
  local same = a ~= nil and a == i and a == opt("decoration:fullscreen_opacity")
  if same and not P.fade_off then
    if pcall(hl.animation, { leaf = "fadeSwitch", enabled = false }) then P.fade_off = true end
  elseif not same and P.fade_off then
    if P.fade then pcall(hl.animation, P.fade) end
    P.fade_off = false
  end
  function P.apply(ws, list)
    local want, cur = {}, P.hidden[ws] or {}
    for _, a in ipairs(list or {}) do want[a] = true end
    for a in pairs(cur) do if not want[a] then P.set(a, "1.00") end end
    for a in pairs(want) do if not cur[a] then P.set(a, "0.00") end end
    P.hidden[ws] = want
  end
  for _, a in ipairs(data.reset) do P.set(a, "1.00") end
  if #data.reset > 0 then P.hidden = {} end
  local map, listed = {}, {}
  for _, w in ipairs(data.ws) do
    listed[w.id] = true
    for a, hide in pairs(w.map) do map[a] = { ws = w.id, hide = hide } end
  end
  for ws in pairs(P.hidden) do if not listed[ws] then P.apply(ws, {}) end end
  P.map = map
  local ok, act = pcall(hl.get_active_window)
  local active = ok and act and act.address or nil
  for _, w in ipairs(data.ws) do
    local e = (active and map[active] and map[active].ws == w.id) and map[active] or map[w.anchor]
    P.apply(w.id, e and e.hide or {})
  end
  -- A reload builds Hyprland's animations afresh (the fade as configured again).
  if not (P.reload_sub and P.reload_sub:is_active()) then
    P.reload_sub = hl.on("config.reloaded", function() uq_pages.fade_off = false end)
  end
  if not (P.sub and P.sub:is_active()) then
    P.sub = hl.on("window.active", function(win)
      if not win or not win.address then return end
      local e = uq_pages.map and uq_pages.map[win.address]
      if e then pcall(uq_pages.apply, e.ws, e.hide) end
    end)
  end
"#;

/// The `PageHiding` table as Lua (`data`), then `PAGE_HIDING_LUA`.
pub(crate) fn page_hiding_lua(workspaces: &[PageWorkspace], reset: &[String]) -> String {
    let list = |v: &[String]| {
        v.iter()
            .map(|a| lua_string(a))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let ws = workspaces
        .iter()
        .map(|(id, anchor, entries)| {
            let map = entries
                .iter()
                .map(|(a, hide)| format!("[{}] = {{ {} }}", lua_string(a), list(hide)))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "{{ id = {id}, anchor = {}, map = {{ {map} }} }}",
                lua_string(anchor)
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "  local data = {{ reset = {{ {} }}, ws = {{ {ws} }} }}\n{PAGE_HIDING_LUA}",
        list(reset)
    )
}

pub(crate) fn script(action: &Action) -> String {
    let keep = matches!(
        action,
        Action::FocusWindow {
            keep_pointer: true,
            ..
        } | Action::Restore { .. }
            | Action::ColumnWidth { .. }
            | Action::Columns { .. }
            | Action::MoveColumn { .. }
            | Action::OwnColumn { .. }
            | Action::Scroll { .. }
    );
    let mut s = String::from(PRELUDE);
    s.push_str("local unwarp, pos, back = false, nil, nil\n");
    // The layout's `move` focuses the window it brings into view: give focus back after.
    if let Action::Columns { .. } | Action::Scroll { .. } = action {
        s.push_str(
            "do local ok, w = pcall(hl.get_active_window)\n  \
             if ok and w and w.address then back = w.address end end\n",
        );
    }
    if let Action::ColumnWidth {
        address,
        full: false,
        keep_focus: true,
    } = action
    {
        s.push_str(&format!(
            "do local ok, w = pcall(hl.get_active_window)\n  \
             if ok and w and w.address and w.address ~= {} then back = w.address end end\n",
            lua_string(address)
        ));
    }
    if keep {
        s.push_str(
            r#"do local got, nw = pcall(hl.get_config, "cursor:no_warps")
  if got and nw == false and pcall(hl.config, { cursor = { no_warps = true } }) then
    unwarp = true
  elseif not (got and nw == true) then
    local ok, p = pcall(hl.get_cursor_pos)
    if ok and type(p) == "table" and tonumber(p.x) and tonumber(p.y) then pos = p end
  end
end
"#,
        );
    }
    s.push_str("local function run()\n");
    for stmt in action.to_lua() {
        step(&mut s, &stmt);
    }
    match action {
        // Scroll the layout so the pinned window's left edge is back where it was.
        Action::Columns {
            pin: Some((address, x)),
            ..
        }
        | Action::Scroll { address, x } => s.push_str(&scroll_lua(address, *x)),
        Action::PageHiding { workspaces, reset } => s.push_str(&page_hiding_lua(workspaces, reset)),
        _ => {}
    }
    if let Action::ColumnWidth {
        address,
        full: true,
        keep_focus: true,
    } = action
    {
        // Not focused: read where it is. Unreadable counts as 0 (not maximized).
        s.push_str(&format!(
            "  local ok, w = pcall(hl.get_window, {})\n  \
             local fs = ok and w and tonumber(w.fullscreen) or 0\n",
            lua_string(&format!("address:{address}"))
        ));
        for (cond, fs) in [("if fs == 1 or fs == 2 then", 1), ("else", 0)] {
            s.push_str(&format!("  {cond}\n"));
            for stmt in column_lua(address, true, fs) {
                s.push_str("  ");
                step(&mut s, &stmt);
            }
        }
        s.push_str("  end\n");
    } else if let Action::ColumnWidth { address, full, .. } = action {
        // Now focused: its state decides what Full or Half takes. Unreadable counts as 0.
        s.push_str(
            "  local ok, w = pcall(hl.get_active_window)\n  \
             local fs = ok and w and tonumber(w.fullscreen) or 0\n",
        );
        for (cond, fs) in [
            ("if fs == 1 then", 1),
            ("elseif fs == 2 then", 2),
            ("else", 0),
        ] {
            s.push_str(&format!("  {cond}\n"));
            for stmt in column_lua(address, *full, fs) {
                s.push_str("  ");
                step(&mut s, &stmt);
            }
        }
        s.push_str("  end\n");
    }
    s.push_str(
        r#"  if back then
    do local m = fail(hl.dispatch(hl.dsp.focus({ window = "address:" .. back }))); if m then return m end end
  end
end
local ran, res = pcall(run)
if unwarp then
  local ok, e = pcall(hl.config, { cursor = { no_warps = false } })
  if not ok then error(tostring(e), 0) end
end
if not ran then error(tostring(res), 0) end
"#,
    );
    s.push_str(&format!(
        "if res then error({} .. res, 0) end\n",
        lua_string(STEP_FAILED)
    ));
    s.push_str(&format!(
        r#"if pos then
  local function round(v) if v < 0 then return -math.floor(-v + 0.5) end return math.floor(v + 0.5) end
  local m = fail(hl.dispatch(hl.dsp.cursor.move({{ x = round(pos.x), y = round(pos.y) }})))
  if m then error({} .. m, 0) end
end
"#,
        lua_string(STEP_FAILED)
    ));
    s
}

/// The error for a rejected script: a rejected step reads as Hyprland's own reply for it.
pub(crate) fn script_err(e: CompositorError) -> String {
    match e {
        CompositorError::Rejected(msg) => match msg.strip_prefix("error: ") {
            Some(m) if m.starts_with(STEP_FAILED) => m[STEP_FAILED.len()..].to_string(),
            _ => msg,
        },
        e => eval_err(e),
    }
}

/// Run `action` in one `eval` (see `script`).
pub async fn run(hypr: &HyprSocket, action: &Action) -> Result<(), String> {
    let mut lua = script(action);
    if let Action::PageHiding { .. } = action {
        // The focus fade as configured, for the handler to put back after hiding (unread: the
        // handler leaves the fade alone).
        let fade = match hypr.request("j/animations").await {
            Ok(r) => fade_switch(&r),
            Err(_) => None,
        };
        lua = format!("{}{lua}", fade_lua(fade.as_ref()));
    }
    hypr.eval(&lua).await.map_err(script_err)
}

/// The focus fade (`fadeSwitch`) as Hyprland has it, from a `j/animations` reply: its own
/// settings when set, else those of the leaf it takes them from (`fade`, then `global`).
/// `(enabled, speed, bezier)`; None when the reply can't be read.
pub(crate) fn fade_switch(reply: &str) -> Option<(bool, f64, String)> {
    let v: Value = serde_json::from_str(reply).ok()?;
    let leaves = v.get(0)?.as_array()?;
    let find = |name: &str| {
        leaves
            .iter()
            .find(|l| l.get("name").and_then(Value::as_str) == Some(name))
    };
    let leaf = ["fadeSwitch", "fade", "global"]
        .into_iter()
        .filter_map(find)
        .find(|l| l.get("overridden").and_then(Value::as_bool) == Some(true))?;
    let enabled = match leaf.get("enabled")? {
        Value::Bool(b) => *b,
        n => n.as_i64()? != 0,
    };
    let speed = leaf.get("speed")?.as_f64()?;
    let bezier = leaf.get("bezier")?.as_str()?;
    (bezier
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        && !bezier.is_empty())
    .then(|| (enabled, speed, bezier.to_string()))
}

/// The focus fade for `PAGE_HIDING_LUA` to restore, as a Lua global it reads (`uq_pages_fade`).
pub(crate) fn fade_lua(fade: Option<&(bool, f64, String)>) -> String {
    match fade {
        Some((enabled, speed, bezier)) => format!(
            "uq_pages_fade = {{ leaf = \"fadeSwitch\", enabled = {enabled}, speed = {speed}, bezier = {} }}\n",
            lua_string(bezier)
        ),
        None => "uq_pages_fade = nil\n".to_string(),
    }
}
