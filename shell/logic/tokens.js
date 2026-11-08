.pragma library

// Defaults match Graphite dark, so a missing or malformed key never leaves the shell unthemed.
function defaults() {
    return {
        bg: "#1f1f22", surface: "#2b2b2e", surfaceAlt: "#353538", overlay: "#464649",
        text: "#e4e4e7", textMuted: "#a9a9ac", accent: "#8ab4f8", accentText: "#1f1f22",
        border: "#5a5a5d", success: "#81c995", warning: "#fdd663", error: "#f28b82",
        radius: 10, gapIn: 5, gapOut: 12, borderWidth: 2,
        fontUi: "Inter", fontMono: "JetBrains Mono", fontSize: 11,
        variant: "dark"
    };
}

// Returns the parsed object, or null for anything that is not a JSON object.
function parse(text) {
    if (typeof text !== "string" || text.length === 0)
        return null;
    try {
        var o = JSON.parse(text);
        return (o !== null && typeof o === "object" && !Array.isArray(o)) ? o : null;
    } catch (e) {
        return null;
    }
}

function color(v, d) {
    return (typeof v === "string" && /^#([0-9a-fA-F]{6}|[0-9a-fA-F]{8})$/.test(v)) ? v : d;
}
function int(v, d, min) {
    return (typeof v === "number" && isFinite(v) && v >= min) ? Math.round(v) : d;
}
function str(v, d) {
    return (typeof v === "string" && v.length > 0 && v.length < 128) ? v : d;
}
function sub(o, k) {
    var v = o ? o[k] : undefined;
    return (v !== null && typeof v === "object") ? v : {};
}

// Flattens a tokens object to the Theme property set, per-key falling back to the defaults.
function toTheme(o) {
    var d = defaults();
    var c = sub(o, "colors"), u = sub(o, "ui"), f = sub(o, "fonts");
    return {
        bg: color(c.bg, d.bg), surface: color(c.surface, d.surface),
        surfaceAlt: color(c.surface_alt, d.surfaceAlt), overlay: color(c.overlay, d.overlay),
        text: color(c.text, d.text), textMuted: color(c.text_muted, d.textMuted),
        accent: color(c.accent, d.accent), accentText: color(c.accent_text, d.accentText),
        border: color(c.border, d.border), success: color(c.success, d.success),
        warning: color(c.warning, d.warning), error: color(c.error, d.error),
        radius: int(u.radius, d.radius, 0), gapIn: int(u.gap_in, d.gapIn, 0),
        gapOut: int(u.gap_out, d.gapOut, 0), borderWidth: int(u.border_width, d.borderWidth, 0),
        fontUi: str(f.ui, d.fontUi), fontMono: str(f.mono, d.fontMono),
        fontSize: int(f.size, d.fontSize, 1),
        variant: (o && (o.variant === "dark" || o.variant === "light")) ? o.variant : d.variant
    };
}
