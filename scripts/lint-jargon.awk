# usage: awk -f lint-jargon.awk scripts/settings-jargon.txt FILE...
# Prints file:line: word for each jargon word found inside a quoted string. A line ending with
# `// jargon-ok: <reason>` and components/AdvancedDetails.qml are exempt.
function isword(c) { return c ~ /[A-Za-z0-9_]/ }
function check(str, lw, i, p, n, before, after) {
    str = tolower(str)
    for (i = 1; i <= nwords; i++) {
        lw = words[i]; n = length(lw); p = 0
        while ((q = index(substr(str, p + 1), lw)) > 0) {
            p += q
            before = p > 1 ? substr(str, p - 1, 1) : ""
            after = substr(str, p + n, 1)
            if (!isword(before) && !isword(after)) { print FILENAME ":" FNR ": " words[i]; return }
            p += 1
        }
    }
}
FNR == NR {
    if ($0 ~ /^[[:space:]]*(#|$)/) next
    gsub(/^[[:space:]]+|[[:space:]]+$/, "")
    words[++nwords] = tolower($0)
    next
}
FILENAME ~ /(^|\/)components\/AdvancedDetails\.qml$/ { next }
{
    line = $0; n = length(line); quote = ""; cur = ""; strs = ""
    for (i = 1; i <= n; i++) {
        c = substr(line, i, 1)
        if (quote != "") {
            if (c == "\\") { cur = cur substr(line, i + 1, 1); i++ }
            else if (c == quote) { strs = strs "\n" cur; quote = ""; cur = "" }
            else cur = cur c
        } else if (c == "\"" || c == "'" || c == "`") quote = c
        else if (c == "/" && substr(line, i + 1, 1) == "/") {
            if (substr(line, i) ~ /^\/\/[[:space:]]*jargon-ok:/) { strs = "" }
            break
        }
    }
    if (quote != "") strs = strs "\n" cur
    if (strs != "") check(strs)
}
