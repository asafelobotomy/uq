.pragma library

// Formatting for the data islands (stocks, news, system monitor, world clock) and the marquee's
// timing. Pure: no Quickshell imports. Every string from a feed or a quote source is untrusted;
// these only shape it, and it reaches the screen through UqText (plain text).

function _num(v) {
    return typeof v === "number" && isFinite(v);
}

// "354.88", "0.0123" (small prices keep 4 significant digits), "12,345.60".
function price(v) {
    if (!_num(v))
        return "–";
    var abs = Math.abs(v);
    var digits = abs >= 1 ? 2 : (abs === 0 ? 2 : Math.min(8, 1 - Math.floor(Math.log(abs) / Math.LN10) + 2));
    var s = v.toFixed(digits);
    var parts = s.split(".");
    var neg = parts[0].charAt(0) === "-";
    var int = neg ? parts[0].slice(1) : parts[0];
    int = int.replace(/\B(?=(\d{3})+(?!\d))/g, ",");
    return (neg ? "-" : "") + int + (parts.length > 1 ? "." + parts[1] : "");
}

// -1, 0 or 1 for a change, so colour is never the only cue (the glyph carries it too).
function direction(change) {
    if (!_num(change) || Math.abs(change) < 1e-9)
        return 0;
    return change > 0 ? 1 : -1;
}

function glyph(change) {
    var d = direction(change);
    return d > 0 ? "▲" : (d < 0 ? "▼" : "■");
}

// "▲1.24%", "▼0.10%", "■0.00%"; "" without a number.
function pct(changePct) {
    if (!_num(changePct))
        return "";
    return glyph(changePct) + Math.abs(changePct).toFixed(2) + "%";
}

// The stocks event as clean rows; untrusted shapes are dropped.
function quotes(ev) {
    var list = ev && ev.quotes && typeof ev.quotes.length === "number" ? ev.quotes : [];
    var out = [];
    for (var i = 0; i < list.length && out.length < 12; i++) {
        var q = list[i];
        if (!q || typeof q.symbol !== "string" || q.symbol.length === 0 || q.symbol.length > 16)
            continue;
        out.push({
            symbol: q.symbol,
            name: typeof q.name === "string" ? q.name.slice(0, 80) : "",
            price: _num(q.price) ? q.price : null,
            change: _num(q.change) ? q.change : null,
            changePct: _num(q.change_pct) ? q.change_pct : null,
            currency: typeof q.currency === "string" ? q.currency.slice(0, 8) : "",
            high: _num(q.day_high) ? q.day_high : null,
            low: _num(q.day_low) ? q.day_low : null,
            ok: q.ok !== false && _num(q.price)
        });
    }
    return out;
}

// One ticker entry: "AAPL 354.88 ▼0.14%".
function quoteText(q) {
    if (!q.ok && q.price === null)
        return q.symbol + " –";
    return q.symbol + " " + price(q.price) + (q.changePct !== null ? " " + pct(q.changePct) : "");
}

// The news event as clean rows (max 40); links must be http(s).
function headlines(ev) {
    var list = ev && ev.items && typeof ev.items.length === "number" ? ev.items : [];
    var out = [];
    for (var i = 0; i < list.length && out.length < 40; i++) {
        var it = list[i];
        if (!it || typeof it.title !== "string" || it.title.length === 0
                || typeof it.link !== "string" || !/^https?:\/\//.test(it.link))
            continue;
        out.push({
            title: it.title.slice(0, 300),
            link: it.link,
            source: typeof it.source === "string" ? it.source.slice(0, 80) : "",
            published: _num(it.published) ? it.published : null
        });
    }
    return out;
}

// "now", "5 min", "3 h", "2 d" since unix seconds `t`.
function age(t, nowMs) {
    if (!_num(t))
        return "";
    var s = Math.max(0, Math.round(nowMs / 1000 - t));
    if (s < 60)
        return "now";
    if (s < 3600)
        return Math.floor(s / 60) + " min";
    if (s < 86400)
        return Math.floor(s / 3600) + " h";
    return Math.floor(s / 86400) + " d";
}

// What the island says about the data: "" (fresh), "Updating…", "Offline · 20 min old", …
// `status` is the bridge's ok | partial | stale | error.
function state(ev, nowMs) {
    if (ev === null || ev === undefined)
        return "Loading…";
    var st = typeof ev.status === "string" ? ev.status : "ok";
    var a = age(ev.fetched_at, nowMs);
    if (st === "error")
        return a !== "" ? "Can't update · " + a + " old" : "Can't reach the source";
    if (st === "stale")
        return a !== "" ? "Updating… · " + a + " old" : "Updating…";
    return "";
}

// "45%" from a number.
function percent(v) {
    return _num(v) ? Math.round(v) + "%" : "–";
}

// "12.3 GiB" from bytes.
function bytes(n) {
    if (!_num(n) || n < 0)
        return "–";
    var units = ["B", "KiB", "MiB", "GiB", "TiB"], i = 0;
    while (n >= 1024 && i < units.length - 1) {
        n /= 1024;
        i++;
    }
    return (i === 0 ? n.toFixed(0) : n.toFixed(1)) + " " + units[i];
}

// "54°" from Celsius, "" without one.
function temp(c) {
    if (!_num(c))
        return "";
    var r = Math.round(c);
    return (r === 0 ? 0 : r) + "°";
}

// The world clock event as clean rows.
function zones(ev) {
    var list = ev && typeof ev.length === "number" ? ev : [];
    var out = [];
    for (var i = 0; i < list.length && out.length < 4; i++) {
        var z = list[i];
        if (!z || typeof z.zone !== "string" || !_num(z.offset_s) || Math.abs(z.offset_s) > 18 * 3600)
            continue;
        out.push({
            zone: z.zone,
            label: typeof z.label === "string" && z.label.length > 0 ? z.label.slice(0, 40) : z.zone,
            offset: z.offset_s,
            abbrev: typeof z.abbrev === "string" ? z.abbrev.slice(0, 8) : ""
        });
    }
    return out;
}

function _pad(n) {
    return n < 10 ? "0" + n : String(n);
}

// The wall-clock time in a zone `offsetS` seconds from UTC, at `nowMs`: "09:41" or "9:41 AM".
function zoneTime(nowMs, offsetS, h24) {
    var d = new Date(nowMs + offsetS * 1000);
    var h = d.getUTCHours(), m = d.getUTCMinutes();
    if (h24)
        return _pad(h) + ":" + _pad(m);
    var ap = h < 12 ? "AM" : "PM";
    var h12 = h % 12 === 0 ? 12 : h % 12;
    return h12 + ":" + _pad(m) + " " + ap;
}

// "+1 day", "−1 day" or "" against the local date, for a zone's time.
function dayShift(nowMs, offsetS, localOffsetMin) {
    var there = new Date(nowMs + offsetS * 1000);
    var here = new Date(nowMs - localOffsetMin * 60000);
    var a = Date.UTC(there.getUTCFullYear(), there.getUTCMonth(), there.getUTCDate());
    var b = Date.UTC(here.getUTCFullYear(), here.getUTCMonth(), here.getUTCDate());
    var d = Math.round((a - b) / 86400000);
    return d > 0 ? "+1 day" : (d < 0 ? "−1 day" : "");
}

// Marquee: milliseconds to scroll `distance` pixels at `pxPerSec`.
function scrollMs(distance, pxPerSec) {
    if (!(distance > 0) || !(pxPerSec > 0))
        return 0;
    return Math.round(distance / pxPerSec * 1000);
}

// Marquee: should it move now?
function moving(overflows, reduceMotion, hovered, focused, sharing) {
    return overflows && !reduceMotion && !hovered && !focused && !sharing;
}

// Feeds the popup offers before the user types their own.
var PRESET_FEEDS = [
    { name: "BBC News – World", url: "https://feeds.bbci.co.uk/news/world/rss.xml" },
    { name: "The Guardian – World", url: "https://www.theguardian.com/world/rss" },
    { name: "NPR News", url: "https://feeds.npr.org/1001/rss.xml" },
    { name: "Al Jazeera", url: "https://www.aljazeera.com/xml/rss/all.xml" },
    { name: "Hacker News", url: "https://hnrss.org/frontpage" },
    { name: "Ars Technica", url: "https://feeds.arstechnica.com/arstechnica/index" }
];

// `^[A-Z0-9^.=-]{1,16}$` after upper-casing, as uqd checks; null when not a symbol.
function cleanSymbol(s) {
    var t = String(s).trim().toUpperCase();
    return /^[A-Z0-9^.=\-]{1,16}$/.test(t) && /[A-Z0-9]/.test(t) ? t : null;
}

// https only, a host, no spaces; as uqd checks (uqd has the last word).
function validFeed(u) {
    var s = String(u).trim();
    return s.length <= 512 && /^https:\/\/[A-Za-z0-9.\-:]+(\/|\?|#|$)/.test(s) && !/\s/.test(s);
}
