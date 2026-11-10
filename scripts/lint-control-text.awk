# Control-text lint for apps/uq-settings/qml (run by scripts/lint-settings.sh, one call per file set).
# A stock control (Button, CheckBox, ComboBox, ...) paints its own `text:` through the style, which
# may treat model strings as rich text. So at the control's own depth:
#   - `text:` must be one "..." string literal; text from data goes in `contentItem: UqLabel {}`;
#   - a ComboBox must declare both `contentItem:` and `delegate:`.
# Prints file:line:text for each hit. Nested blocks (contentItem: UqLabel { text: x }) are depth 2
# of the control and are not inspected.
function is_open(prefix) {
    return prefix ~ ("(^|[^A-Za-z0-9_.])(QQC2\\.|Controls\\.|Kirigami\\.)?(" TYPES ")[[:space:]]*$")
}
function hit(n, line) { print FILENAME ":" n ":" line; found = 1 }
function check_seg(k,    seg, rest) {
    seg = SEG[k]
    if (seg ~ /(^|[^A-Za-z0-9_.])contentItem[[:space:]]*:/) HASC[k] = 1
    if (seg ~ /(^|[^A-Za-z0-9_.])delegate[[:space:]]*:/) HASD[k] = 1
    if (match(seg, /(^|[^A-Za-z0-9_.])text[[:space:]]*:/)) {
        rest = substr(seg, RSTART + RLENGTH)
        if (rest !~ /^[[:space:]]*"([^"\\]|\\.)*"[[:space:]]*(;.*)?$/) hit(FNR, $0)
    }
    SEG[k] = ""
}
function pop_ctl(    k) {
    k = top
    check_seg(k)
    if (ISCOMBO[k] && !(HASC[k] && HASD[k])) hit(LINE[k], LINETXT[k])
    top--
}
BEGIN {
    TYPES = "Button|ToolButton|CheckBox|RadioButton|Switch|ItemDelegate|CheckDelegate|SwitchDelegate|RadioDelegate|MenuItem|TabButton|ComboBox|Chip"
}
FNR == 1 { depth = 0; top = 0; pending = ""; inblock = 0 }
{
    line = $0
    n = length(line)
    instr = 0
    # A control type at the end of the previous non-blank line, with `{` opening this line.
    carry = (pending != "" && line ~ /^[[:space:]]*\{/)
    for (i = 1; i <= n; i++) {
        c = substr(line, i, 1)
        if (instr) {
            if (c == "\\") { if (top > 0 && depth == INNER[top]) SEG[top] = SEG[top] c substr(line, i + 1, 1); i++; continue }
            if (c == "\"") instr = 0
            if (top > 0 && depth == INNER[top]) SEG[top] = SEG[top] c
            continue
        }
        if (inblock) {
            if (c == "*" && substr(line, i + 1, 1) == "/") { inblock = 0; i++ }
            continue
        }
        if (c == "/" && substr(line, i + 1, 1) == "/") break
        if (c == "/" && substr(line, i + 1, 1) == "*") { inblock = 1; i++; continue }
        if (c == "\"") {
            instr = 1
            if (top > 0 && depth == INNER[top]) SEG[top] = SEG[top] c
            continue
        }
        if (c == "{") {
            if (top > 0 && depth == INNER[top]) SEG[top] = SEG[top] " "
            prefix = substr(line, 1, i - 1)
            if (is_open(prefix) || (carry && prefix ~ /^[[:space:]]*$/)) {
                top++
                INNER[top] = depth + 1
                SEG[top] = ""; HASC[top] = 0; HASD[top] = 0
                if (carry && prefix ~ /^[[:space:]]*$/) { LINE[top] = pn; LINETXT[top] = pending }
                else { LINE[top] = FNR; LINETXT[top] = line }
                ISCOMBO[top] = (LINETXT[top] ~ /(^|[^A-Za-z0-9_.])(QQC2\.|Controls\.|Kirigami\.)?ComboBox[[:space:]]*\{?[[:space:]]*$/ || LINETXT[top] ~ /(^|[^A-Za-z0-9_.])(QQC2\.|Controls\.|Kirigami\.)?ComboBox[[:space:]]*\{/)
            }
            depth++
            carry = 0
            continue
        }
        if (c == "}") {
            if (top > 0 && depth == INNER[top]) pop_ctl()
            depth--
            continue
        }
        if (top > 0 && depth == INNER[top]) SEG[top] = SEG[top] c
    }
    for (k = 1; k <= top; k++) { if (SEG[k] != "") { SEG[k] = SEG[k] " "; check_seg(k) } }
    if (line !~ /^[[:space:]]*$/) {
        pending = ""
        if (is_open(line)) { pending = line; pn = FNR }
    }
}
END { exit 0 }
