.pragma library

// Ranking and merge logic for the launcher. Pure data in, data out; strings are passed
// through untouched (the UI decides how to render them).

// The built-in groups, in the order they show after Apps (a custom order comes first). Other
// providers follow them.
var BUILTIN_ORDER = ["web", "calc", "run", "windows", "settings", "files:folders", "files:documents",
                     "files:images", "files:audio", "files:video", "files:code", "files:archives",
                     "files:other", "files:contents", "files:system"];

function _fuzzy(q, t) {
    // Subsequence match; consecutive runs and early starts score higher. Result in (0, 0.6].
    var ti = 0, first = -1, runs = 0, prev = -2;
    for (var qi = 0; qi < q.length; qi++) {
        var c = q.charAt(qi);
        var at = t.indexOf(c, ti);
        if (at < 0)
            return 0;
        if (first < 0)
            first = at;
        if (at === prev + 1)
            runs++;
        prev = at;
        ti = at + 1;
    }
    var consecutive = q.length > 1 ? runs / (q.length - 1) : 1;
    var early = 1 / (1 + first);
    var span = q.length / (prev - first + 1);
    return 0.6 * (0.1 + 0.4 * consecutive + 0.3 * early + 0.2 * span) * 0.999;
}

function matchScore(query, text) {
    var q = String(query === undefined || query === null ? "" : query).toLowerCase();
    var t = String(text === undefined || text === null ? "" : text).toLowerCase();
    if (q === "" || t === "")
        return 0;
    if (t === q)
        return 1.0;
    if (t.indexOf(q) === 0)
        return 0.9;
    var words = t.split(/[\s\-_.\/]+/);
    for (var i = 1; i < words.length; i++)
        if (words[i].indexOf(q) === 0)
            return 0.75;
    // Scattered letters count only when the first one starts a word ("fire" finds "Fabulous
    // Install Reader Editor", not the f, i, r, e strewn through "LibreOffice Impress").
    var starts = false;
    for (var j = 0; j < words.length && !starts; j++)
        starts = words[j].charAt(0) === q.charAt(0);
    return starts ? _fuzzy(q, t) : 0;
}

// matchScore without the scattered-letter match: the text, or one of its words, starts with the
// query. For keywords and descriptions, where a subsequence matches nearly anything ("report"
// is in "Resource monitor that shows usage and stats for processor…").
function wordScore(query, text) {
    var s = matchScore(query, text);
    return s >= 0.75 ? s : 0;
}

function appScore(query, entry, launchCount) {
    var e = entry || {};
    var nameScore = matchScore(query, e.name);
    var best = nameScore;
    var gn = wordScore(query, e.genericName);
    if (gn > best) best = gn;
    var kws = e.keywords || [];
    for (var i = 0; i < kws.length; i++) {
        var k = wordScore(query, kws[i]);
        if (k > best) best = k;
    }
    var cm = wordScore(query, e.comment) * 0.8;
    if (cm > best) best = cm;
    var n = Math.max(0, Number(launchCount) || 0);
    var kind = nameScore === 1.0 ? "exact" : (nameScore === 0.9 ? "prefix" : "fuzzy");
    var score = best > 0 ? best + 0.15 * Math.log(1 + n) : 0;
    return { score: score, kind: kind };
}

function _tier(kind) {
    return (kind === "exact" || kind === "prefix") ? 1 : 0;
}

function _cmp(a, b) {
    var d = _tier(b.kind) - _tier(a.kind);
    if (d !== 0) return d;
    if (b.score !== a.score) return b.score - a.score;
    var an = String(a.name || ""), bn = String(b.name || "");
    return an < bn ? -1 : (an > bn ? 1 : 0);
}

function searchApps(query, entries, counts, limit) {
    var q = String(query === undefined || query === null ? "" : query).trim();
    if (q === "")
        return [];
    var max = limit === undefined ? 8 : limit;
    var cs = counts || {};
    var out = [];
    var list = entries || [];
    for (var i = 0; i < list.length; i++) {
        var e = list[i];
        if (!e || e.noDisplay)
            continue;
        var r = appScore(q, e, cs[e.id] || 0);
        if (r.score <= 0)
            continue;
        out.push({ id: e.id, name: e.name, icon: e.icon, comment: e.comment,
                   score: r.score, kind: r.kind });
    }
    out.sort(_cmp);
    return out.slice(0, max);
}

// Open windows whose title, class or app name match: rows for the "windows" group, best first,
// below an app's own name match (at most 0.85). windows: [{address, title, cls, app, icon,
// minimised, workspace}] (address already normalised); at most `limit` (5).
function searchWindows(query, windows, limit) {
    var q = String(query === undefined || query === null ? "" : query).trim();
    if (q.length < 2)
        return [];
    var max = limit === undefined ? 5 : limit;
    var out = [];
    var list = windows || [];
    for (var i = 0; i < list.length; i++) {
        var w = list[i];
        if (!w || typeof w.address !== "string" || w.address === "")
            continue;
        var best = Math.max(matchScore(q, w.title), matchScore(q, w.app), matchScore(q, w.cls));
        if (best <= 0)
            continue;
        var where = w.minimised ? "Minimised" : (w.workspace ? "Workspace " + w.workspace : "");
        out.push({ id: w.address, title: String(w.title || w.app || w.cls || ""),
                   subtitle: [String(w.app || w.cls || ""), where].filter(function (s) { return s !== ""; }).join(" · "),
                   icon: String(w.icon || ""), score: best * 0.85, minimised: !!w.minimised,
                   toWs: Number(w.toWs) || 0 });
    }
    out.sort(function (a, b) {
        if (b.score !== a.score) return b.score - a.score;
        return a.title < b.title ? -1 : (a.title > b.title ? 1 : 0);
    });
    return out.slice(0, max);
}

function merge(apps, providerGroups, generation, order) {
    var appList = apps || [];
    var groups = [];
    if (appList.length > 0)
        groups.push({ provider: "apps", providerName: "Apps", items: appList });

    var all = providerGroups || {};
    var ids = Object.keys(all);
    var live = [];
    for (var i = 0; i < ids.length; i++) {
        var g = all[ids[i]];
        if (!g || g.generation !== generation || !g.items || g.items.length === 0)
            continue;
        live.push({ provider: g.provider || ids[i], providerName: g.provider_name || g.providerName || "",
                    items: g.items });
    }
    var custom = Array.isArray(order) ? order : [];
    var rank = function (p) {
        var c = custom.indexOf(p.provider);
        if (c >= 0)
            return c;
        var b = BUILTIN_ORDER.indexOf(p.provider);
        return custom.length + (b >= 0 ? b : BUILTIN_ORDER.length);
    };
    live.sort(function (a, b) {
        var d = rank(a) - rank(b);
        if (d !== 0) return d;
        if (a.providerName !== b.providerName) return a.providerName < b.providerName ? -1 : 1;
        return a.provider < b.provider ? -1 : (a.provider > b.provider ? 1 : 0);
    });
    for (var j = 0; j < live.length; j++)
        groups.push(live[j]);

    var topHit = null;
    // A clear web address (score 1) is what was meant: it leads, even over an app's name.
    for (var w = 0; w < groups.length && topHit === null; w++)
        if (groups[w].provider === "web" && groups[w].items.length > 0 && Number(groups[w].items[0].score) >= 1)
            topHit = { provider: "web", providerName: groups[w].providerName, item: groups[w].items[0] };
    if (topHit !== null) {
        // Chosen above.
    } else if (appList.length > 0 && _tier(appList[0].kind) === 1) {
        topHit = { provider: "apps", providerName: "Apps", item: appList[0] };
    } else {
        var bestScore = -1;
        for (var k = 0; k < groups.length; k++) {
            var its = groups[k].items;
            for (var m = 0; m < its.length; m++) {
                var s = Number(its[m].score) || 0;
                if (s > bestScore) {
                    bestScore = s;
                    topHit = { provider: groups[k].provider, providerName: groups[k].providerName,
                               item: its[m] };
                }
            }
        }
    }
    return { topHit: topHit, groups: groups };
}
