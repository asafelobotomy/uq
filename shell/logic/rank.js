.pragma library

// Ranking and merge logic for the launcher. Pure data in, data out; strings are passed
// through untouched (the UI decides how to render them).

var BUILTIN_ORDER = ["calc", "run"];

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
    return _fuzzy(q, t);
}

function appScore(query, entry, launchCount) {
    var e = entry || {};
    var nameScore = matchScore(query, e.name);
    var best = nameScore;
    var gn = matchScore(query, e.genericName);
    if (gn > best) best = gn;
    var kws = e.keywords || [];
    for (var i = 0; i < kws.length; i++) {
        var k = matchScore(query, kws[i]);
        if (k > best) best = k;
    }
    var cm = matchScore(query, e.comment) * 0.8;
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
    if (appList.length > 0 && _tier(appList[0].kind) === 1) {
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
