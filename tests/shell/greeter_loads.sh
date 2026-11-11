#!/usr/bin/env bash
# Headless check under real Quickshell (offscreen, no Hyprland, no greetd socket): greeter/Auth.qml
# connects to Quickshell.Services.Greetd's Greetd singleton (so every signal it handles exists with
# that name), and the login form (Greeter.qml with its user list, session picker and power buttons)
# compiles and instantiates. The windows themselves (PanelWindow) need Wayland and aren't loaded.
set -u
root=$(cd "$(dirname "$0")/../.." && pwd)
command -v qs >/dev/null || { echo "greeter_loads skipped: qs not found"; exit 0; }
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
cat > "$tmp/shell.qml" <<QML
import QtQuick
import Quickshell
import Quickshell.Services.Greetd
import "file:$root/greeter"
ShellRoot {
    Auth { id: conv; greetd: Greetd }
    FloatingWindow {
        implicitWidth: 1000; implicitHeight: 700
        Greeter {
            id: form
            anchors.fill: parent
            auth: conv
            sessions: [{ id: "uq.desktop", name: "uQ", command: ["/usr/bin/uq-session"] }]
            Component.onCompleted: {
                start("");
                console.log("LOADED", conv.mode, form.session.name, Greetd.available);
            }
        }
    }
    Timer { interval: 1500; running: true; onTriggered: Qt.quit() }
}
QML
out=$(env -u GREETD_SOCK -u HYPRLAND_INSTANCE_SIGNATURE -u WAYLAND_DISPLAY QT_QPA_PLATFORM=offscreen \
    timeout 30 qs -p "$tmp/shell.qml" --no-duplicate 2>&1)
echo "$out" | grep -q 'LOADED password uQ false' || { echo "FAIL: the greeter form did not load"; echo "$out"; exit 1; }
if echo "$out" | grep -E 'TypeError|ReferenceError|is not a type|Cannot assign|no signal of the target matches|Detected function|qml: .*[Ee]rror|Unable to assign|is not a valid import|module .* is not installed|File not found|Failed to load configuration|Cannot read property|is not a function|Syntax[Ee]rror|[A-Za-z]+\.qml:[0-9]+:[0-9]*:? '; then
    echo "FAIL: QML errors"; echo "$out"; exit 1
fi
echo "greeter_loads ok"
