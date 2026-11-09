.pragma library
.import "launcher.js" as L
.import "notif.js" as N

// The built-in notification server's rules (spec §2.5): the body sanitiser, timeouts, DnD,
// image references, the popup view and the store caps. Bodies come from any app on the bus and
// are untrusted: sanitize() is the only thing whose output may reach a StyledText
// (notif/builtin/NotifBody.qml), and it emits only <b> <i> <u> <br> and <a href="...">, with
// every opened tag closed in order and all other text escaped.
//
// The parser is one forward pass over at most MAX_BODY_IN chars. Searches for `>`, `-->` and
// `</script` use indexOf from the current position and are cached or consume the input, so the
// work is linear; there are no backtracking regexes.

var MAX_BODY_IN = 8192;
var MAX_BODY_OUT = 2000;
var MAX_KEEP = 50;
var MAX_POPUPS = 3;
var MAX_ACTIONS = 3;
var MAX_ACTION_TEXT = 40;
var DEFAULT_TIMEOUT_MS = 5000;
var MAX_TIMEOUT_MS = 60000;
var MIN_TIMEOUT_MS = 1000;
var MAX_HREF = 512;
var MAX_DEPTH = 16;         // deeper opens are dropped tags (text kept)

var URGENCY_CRITICAL = 2;   // Quickshell NotificationUrgency: Low 0, Normal 1, Critical 2

var NAMED = { amp: "&", lt: "<", gt: ">", quot: "\"", apos: "'" };
var INLINE = { b: true, i: true, u: true };
var SKIP = { script: true, style: true };

function isAlpha(c) {
    return (c >= "a" && c <= "z") || (c >= "A" && c <= "Z");
}

function isNameChar(c) {
    return isAlpha(c) || (c >= "0" && c <= "9") || c === "-" || c === "_" || c === ":";
}

function isSpace(c) {
    return c === " " || c === "\t" || c === "\n" || c === "\r" || c === "\f";
}

function isDigit(c) {
    return c >= "0" && c <= "9";
}

function isHex(c) {
    return isDigit(c) || (c >= "a" && c <= "f") || (c >= "A" && c <= "F");
}

// An entity at s[i] === "&": {text, end} (end is just past the `;`), or null.
function entityAt(s, i) {
    var j = i + 1;
    if (s.charAt(j) === "#") {
        j++;
        var hex = s.charAt(j) === "x" || s.charAt(j) === "X";
        if (hex)
            j++;
        var start = j;
        while (j < s.length && j - start < 8 && (hex ? isHex(s.charAt(j)) : isDigit(s.charAt(j))))
            j++;
        if (j === start || s.charAt(j) !== ";")
            return null;
        var cp = parseInt(s.slice(start, j), hex ? 16 : 10);
        if (!(cp > 0 && cp <= 0x10ffff) || (cp >= 0xd800 && cp <= 0xdfff))
            return null;
        return { text: String.fromCodePoint(cp), end: j + 1 };
    }
    var k = j;
    while (k < s.length && k - j < 5 && isAlpha(s.charAt(k)))
        k++;
    if (s.charAt(k) !== ";" || !Object.prototype.hasOwnProperty.call(NAMED, s.slice(j, k)))
        return null;
    return { text: NAMED[s.slice(j, k)], end: k + 1 };
}

// Entities decoded; anything that isn't one is left as typed.
function decode(s) {
    var out = "";
    var i = 0;
    while (i < s.length) {
        var amp = s.indexOf("&", i);
        if (amp < 0)
            return out + s.slice(i);
        out += s.slice(i, amp);
        var e = entityAt(s, amp);
        if (e) {
            out += e.text;
            i = e.end;
        } else {
            out += "&";
            i = amp + 1;
        }
    }
    return out;
}

function escapeChar(c) {
    if (c === "&")
        return "&amp;";
    if (c === "<")
        return "&lt;";
    if (c === ">")
        return "&gt;";
    if (c === "\"")
        return "&quot;";
    return c;
}

// Qt's StyledText reads attribute values literally (no entity decoding) and ends a quoted value
// at either quote character, so an href is percent-encoded instead of entity-escaped: nothing
// in it can end the value or the tag. `&` stays raw, as Qt shows it.
function hrefChar(c) {
    if (isControl(c) || c === "\n" || c === "\t" || c === "\r")
        return " ";
    if (c === "\"")
        return "%22";
    if (c === "'")
        return "%27";
    if (c === "<")
        return "%3C";
    if (c === ">")
        return "%3E";
    return c;
}

// Control characters (C0 other than \n and \t, DEL, C1), line/paragraph separators and bidi
// overrides become spaces, as in cleanText.
function isControl(c) {
    var n = c.charCodeAt(0);
    return (n < 0x20 && c !== "\n" && c !== "\t") || (n >= 0x7f && n <= 0x9f)
        || n === 0x2028 || n === 0x2029
        || (n >= 0x202a && n <= 0x202e) || (n >= 0x2066 && n <= 0x2069);
}

// The href of an `a` tag's attribute text (what follows the name, up to `>`), decoded, or "".
function hrefOf(attrs) {
    var i = 0;
    var n = attrs.length;
    while (i < n) {
        while (i < n && (isSpace(attrs.charAt(i)) || attrs.charAt(i) === "/"))
            i++;
        var ns = i;
        while (i < n && !isSpace(attrs.charAt(i)) && attrs.charAt(i) !== "=" && attrs.charAt(i) !== "/")
            i++;
        var name = attrs.slice(ns, i).toLowerCase();
        while (i < n && isSpace(attrs.charAt(i)))
            i++;
        var value = "";
        if (attrs.charAt(i) === "=") {
            i++;
            while (i < n && isSpace(attrs.charAt(i)))
                i++;
            var q = attrs.charAt(i);
            if (q === "\"" || q === "'") {
                var close = attrs.indexOf(q, i + 1);
                if (close < 0)
                    close = n;
                value = attrs.slice(i + 1, close);
                i = close + 1;
            } else {
                var vs = i;
                while (i < n && !isSpace(attrs.charAt(i)))
                    i++;
                value = attrs.slice(vs, i);
            }
        }
        if (name === "href")
            return decode(value);
        if (name === "" && i === ns)
            i++;
    }
    return "";
}

// One pass over the body. `plain` emits decoded text only (br and \n become "\n"); otherwise
// the StyledText subset.
function walk(body, plain) {
    if (typeof body !== "string" || body === "")
        return "";
    var s = body.length > MAX_BODY_IN ? body.slice(0, MAX_BODY_IN) : body;
    var lower = s.toLowerCase();
    var out = [];
    var stack = [];         // open allowed tags, innermost last; at most MAX_DEPTH
    var aOpen = false;      // an `a` is on the stack
    var visible = 0;
    var truncated = false;
    var nextGt = -2;        // cached index of the next `>` at or after the cursor (-1: none)
    var i = 0;
    var n = s.length;

    function text(c) {
        if (c === "\r")
            return;
        // Never split a surrogate pair at the cap.
        var high = c.charCodeAt(0) >= 0xd800 && c.charCodeAt(0) <= 0xdbff;
        if (visible >= MAX_BODY_OUT || (high && visible + 1 >= MAX_BODY_OUT)) {
            truncated = true;
            return;
        }
        visible++;
        if (c === "\n") {
            out.push(plain ? "\n" : "<br>");
            return;
        }
        if (isControl(c))
            c = " ";
        out.push(plain ? c : escapeChar(c));
    }

    function closeTo(depth) {
        while (stack.length > depth) {
            var t = stack.pop();
            if (t === "a")
                aOpen = false;
            if (!plain)
                out.push("</" + t + ">");
        }
    }

    function findGt(from) {
        if (nextGt === -1 || (nextGt >= from))
            return nextGt;
        nextGt = s.indexOf(">", from);
        return nextGt;
    }

    while (i < n && !truncated) {
        var c = s.charAt(i);
        if (c === "&") {
            var e = entityAt(s, i);
            if (e) {
                for (var k = 0; k < e.text.length; k++)
                    text(e.text.charAt(k));
                i = e.end;
            } else {
                text("&");
                i++;
            }
            continue;
        }
        if (c !== "<") {
            text(c);
            i++;
            continue;
        }

        // Comments and declarations (<!...>, <?...>) are dropped.
        var c1 = s.charAt(i + 1);
        if (c1 === "!" || c1 === "?") {
            var end;
            if (s.substr(i, 4) === "<!--") {
                var cm = s.indexOf("-->", i + 4);
                end = cm < 0 ? n : cm + 3;
            } else {
                var g = findGt(i);
                end = g < 0 ? n : g + 1;
            }
            i = end;
            continue;
        }

        var closing = c1 === "/";
        var ns = i + (closing ? 2 : 1);
        var gt = isAlpha(s.charAt(ns)) ? findGt(ns) : -1;
        if (gt < 0) {
            text("<");
            i++;
            continue;
        }
        var ne = ns;
        while (ne < gt && isNameChar(s.charAt(ne)))
            ne++;
        var name = lower.slice(ns, ne);
        var attrs = s.slice(ne, gt);
        var selfClosing = attrs.length > 0 && attrs.charAt(attrs.length - 1) === "/";
        i = gt + 1;

        if (closing) {
            for (var d = stack.length - 1; d >= 0; d--) {
                if (stack[d] === name) {
                    closeTo(d);
                    break;
                }
            }
            continue;
        }
        if (name === "br") {
            text("\n");
            continue;
        }
        if (SKIP[name] === true) {
            if (selfClosing)
                continue;
            var sc = lower.indexOf("</" + name, i);
            i = sc < 0 ? n : sc;
            continue;
        }
        if (selfClosing)
            continue;
        if (stack.length >= MAX_DEPTH)
            continue;
        if (INLINE[name] === true) {
            stack.push(name);
            if (!plain)
                out.push("<" + name + ">");
            continue;
        }
        if (name === "a" && !aOpen) {
            var href = hrefOf(attrs);
            if (href !== "") {
                href = href.slice(0, MAX_HREF);
                var h = "";
                for (var x = 0; x < href.length; x++)
                    h += hrefChar(href.charAt(x));
                stack.push("a");
                aOpen = true;
                if (!plain)
                    out.push("<a href=\"" + h + "\">");
            }
        }
        // Anything else: the tag is dropped and its text kept.
    }
    if (truncated)
        out.push("…");
    closeTo(0);
    return out.join("");
}

function sanitize(body) {
    return walk(body, false);
}

function plainBody(body) {
    return walk(body, true);
}

function isCritical(urgency) {
    return urgency === URGENCY_CRITICAL;
}

function timeoutMs(expireTimeout, urgency) {
    if (isCritical(urgency))
        return 0;
    if (typeof expireTimeout !== "number" || !isFinite(expireTimeout) || expireTimeout < 0)
        return DEFAULT_TIMEOUT_MS;
    if (expireTimeout === 0)
        return 0;
    return Math.min(MAX_TIMEOUT_MS, Math.max(MIN_TIMEOUT_MS, Math.round(expireTimeout)));
}

function popupAllowed(dnd, urgency) {
    return !dnd || isCritical(urgency);
}

// image:// (Quickshell's image-data provider) or a local file; never the network.
function imageRef(image) {
    if (typeof image !== "string" || image === "")
        return "";
    if (image.indexOf("image://") === 0)
        return image;
    if (image.indexOf("file:///") === 0)
        return image;
    if (image.charAt(0) === "/") {
        var r = L.iconRef(image);
        return r.path ? r.path : "";
    }
    return "";
}

function popupView(n, sharing) {
    var o = n || {};
    var actions = [];
    var list = o.actions;
    if (!sharing && list) {
        for (var i = 0; i < list.length && actions.length < MAX_ACTIONS; i++) {
            var a = list[i];
            if (!a || a.identifier === "default")
                continue;
            actions.push({ identifier: String(a.identifier), text: N.cleanText(a.text, MAX_ACTION_TEXT) });
        }
    }
    return {
        appName: N.cleanText(o.appName, N.MAX_APP),
        appIcon: typeof o.appIcon === "string" ? o.appIcon : "",
        summary: sharing ? "" : N.cleanText(o.summary, N.MAX_SUMMARY),
        // raw: only NotifBody may render this (it sanitises it)
        body: sharing || typeof o.body !== "string" ? "" : o.body,
        actions: actions
    };
}

function trimKept(count) {
    if (typeof count !== "number" || !isFinite(count))
        return 0;
    return Math.max(0, Math.floor(count) - MAX_KEEP);
}
