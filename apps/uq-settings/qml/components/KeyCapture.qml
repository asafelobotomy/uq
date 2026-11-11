// "Press the keys…": a field that listens for one key combination. While it listens the page
// pauses uQ's shortcuts (the editor connects started() and ended() to KeybindsModel), so a combo
// that is already a shortcut, such as Super + Space, reaches the field instead of opening
// something. A modifier on its own keeps waiting; Escape on its own, losing focus or 15 s
// passing ends it. What was pressed is turned into a shortcut by `comboFrom`
// (KeybindsModel.comboFromKey).
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import "."

FocusScope {
    id: capture

    /// `(key, modifiers) => "SUPER + Q"`, "" for a modifier on its own or an unknown key.
    property var comboFrom: (key, modifiers) => ""
    /// Whether pausing the shortcuts worked. False hides the field: the pickers still work.
    property bool available: true
    /// Whether a request can be sent now.
    property bool canStart: true
    readonly property bool listening: pressing
    property bool pressing: false
    /// How long uqd keeps shortcuts paused, ms.
    property int timeout: 15000

    /// A shortcut was pressed: `combo` as `SUPER + Q`.
    signal captured(string combo)
    /// Listening began (pause the shortcuts) and ended (put them back).
    signal started()
    signal ended()

    function start() {
        if (!capture.available || !capture.canStart || capture.pressing)
            return;
        capture.pressing = true;
        limit.restart();
        capture.forceActiveFocus();
        capture.started();
    }
    function stop() {
        if (!capture.pressing)
            return;
        capture.pressing = false;
        limit.stop();
        capture.ended();
    }

    visible: available
    implicitWidth: row.implicitWidth
    implicitHeight: row.implicitHeight
    Accessible.role: Accessible.Pane
    Accessible.name: "Press the keys"

    Keys.onPressed: event => {
        if (!capture.pressing)
            return;
        event.accepted = true;
        if (event.key === Qt.Key_Escape && event.modifiers === Qt.NoModifier) {
            capture.stop();
            return;
        }
        const combo = String(capture.comboFrom(event.key, event.modifiers));
        if (combo === "")
            return;
        capture.stop();
        capture.captured(combo);
    }
    // While listening, no window or Kirigami shortcut (Ctrl+Z, Ctrl+Shift+Z...) may take the keys.
    Keys.onShortcutOverride: event => {
        event.accepted = capture.pressing;
    }
    onActiveFocusChanged: if (!activeFocus) capture.stop()

    Timer {
        id: limit
        interval: capture.timeout
        onTriggered: capture.stop()
    }

    ColumnLayout {
        id: row
        anchors.fill: parent
        spacing: Kirigami.Units.smallSpacing

        QQC2.Button {
            objectName: "captureButton"
            // The scope holds the focus, so Space and Return are keys to capture, not clicks.
            focusPolicy: Qt.NoFocus
            enabled: capture.canStart || capture.pressing
            text: "Press the keys…"
            icon.name: "input-keyboard"
            highlighted: capture.pressing
            Accessible.description: "Then press the shortcut you want"
            onClicked: capture.pressing ? capture.stop() : capture.start()
        }
        UqLabel {
            objectName: "captureNote"
            visible: capture.pressing
            text: "Your shortcuts are paused while you press keys."
            wrapMode: Text.Wrap
            opacity: 0.7
            Layout.fillWidth: true
        }
    }
}
