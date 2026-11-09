pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Wayland
import Quickshell.Hyprland
import Quickshell.Services.Polkit as Polkit
import "../core"
import "../logic/notif.js" as N

// The polkit authentication agent (spec §2.13): a dialog on the focused monitor, above normal
// windows, for each request polkit sends while this agent is registered. The first agent to
// register for the session wins, so with another agent (KDE's, GNOME's, lxpolkit) running this
// one never sees a request.
// Everything shown comes from the requesting process via polkit and is untrusted: the message,
// action id, prompts and identity names are plain text through cleanText and capped. The
// response field uses the lock screen's hints when the response is hidden. Nothing is logged.
// While the screen is locked the dialog stays hidden (it can't be answered there) and the request
// stays pending; it shows again on unlock.
PanelWindow {
    id: root

    readonly property var flow: agent.flow
    readonly property bool shown: agent.isActive && flow !== null && !Bridge.locked
    // The focused monitor when the dialog opened, so it doesn't follow the cursor.
    property var openedOn: null

    readonly property string message: flow ? N.cleanText(flow.message, 500) : ""
    readonly property string actionId: flow ? N.cleanText(flow.actionId, 200) : ""
    readonly property string supplementary: flow ? N.cleanText(flow.supplementaryMessage, 300) : ""
    readonly property string prompt: flow ? N.cleanText(flow.inputPrompt, 100) : ""
    readonly property var identities: flow && flow.identities ? flow.identities : []

    visible: shown
    screen: openedOn ? openedOn : screenFor(Hyprland.focusedMonitor)
    color: "transparent"
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.namespace: "uq-polkit"
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive
    exclusionMode: ExclusionMode.Ignore
    // No anchors: the compositor centres the layer surface on its output.
    implicitWidth: 420
    implicitHeight: layout.implicitHeight + Theme.gapOut * 2

    function screenFor(monitor) {
        if (!monitor)
            return null;
        var list = Quickshell.screens;
        for (var i = 0; i < list.length; i++)
            if (list[i].name === monitor.name)
                return list[i];
        return null;
    }

    function identityName(identity) {
        if (!identity)
            return "";
        var name = N.cleanText(identity.displayName, 64);
        return name !== "" ? name : N.cleanText(identity.name, 64);
    }

    function submit() {
        if (!root.flow || !root.shown || !root.flow.isResponseRequired)
            return;
        var value = field.text;
        field.clear();
        root.flow.submit(value);
    }

    function cancel() {
        field.clear();
        if (root.flow)
            root.flow.cancelAuthenticationRequest();
    }

    onShownChanged: {
        openedOn = shown ? screenFor(Hyprland.focusedMonitor) : null;
        field.clear();
        if (shown)
            Qt.callLater(field.forceActiveFocus);
    }
    onFlowChanged: field.clear()

    // Qualified: this file is itself named PolkitAgent.
    Polkit.PolkitAgent {
        id: agent
    }

    Rectangle {
        id: content
        anchors.fill: parent
        radius: Theme.radius
        color: Theme.bg
        border.color: Theme.border
        border.width: Theme.borderWidth
        Accessible.role: Accessible.Dialog
        Accessible.name: "Authentication required"
        Keys.onEscapePressed: root.cancel()

        ColumnLayout {
            id: layout
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top
            anchors.margins: Theme.gapOut
            spacing: Theme.gapIn * 2

            UqText {
                Layout.fillWidth: true
                text: "Authentication required"
                font.bold: true
                font.pixelSize: Theme.fontSize + 2
            }
            UqText {
                Layout.fillWidth: true
                text: root.message
                wrapMode: Text.Wrap
                elide: Text.ElideNone
            }
            UqText {
                Layout.fillWidth: true
                visible: root.actionId !== ""
                text: root.actionId
                color: Theme.textMuted
                font.family: Theme.fontMono
            }

            // --- who to authenticate as, when polkit offers more than one identity ---
            Flow {
                Layout.fillWidth: true
                visible: root.identities.length > 1
                spacing: Theme.gapIn

                Repeater {
                    model: root.identities.length > 1 ? root.identities : []
                    delegate: Rectangle {
                        id: idBtn
                        required property var modelData
                        readonly property bool selected: root.flow !== null
                            && root.flow.selectedIdentity === modelData
                        width: idText.implicitWidth + Theme.gapOut * 2
                        height: 26
                        radius: Theme.radius - 2
                        color: selected ? Theme.accent
                            : (idMouse.containsMouse || activeFocus ? Theme.overlay : Theme.surfaceAlt)
                        border.color: Theme.accent
                        border.width: activeFocus ? 1 : 0
                        activeFocusOnTab: true
                        Accessible.role: Accessible.RadioButton
                        Accessible.name: idText.text
                        Accessible.checked: selected
                        Accessible.onPressAction: idBtn.pick()
                        Keys.onReturnPressed: idBtn.pick()
                        Keys.onEnterPressed: idBtn.pick()
                        Keys.onSpacePressed: idBtn.pick()
                        Keys.onEscapePressed: root.cancel()

                        function pick() {
                            if (root.flow && !selected) {
                                field.clear();
                                root.flow.selectedIdentity = modelData;
                            }
                        }

                        UqText {
                            id: idText
                            anchors.centerIn: parent
                            text: root.identityName(idBtn.modelData)
                            color: idBtn.selected ? Theme.accentText : Theme.text
                        }
                        MouseArea {
                            id: idMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            onClicked: idBtn.pick()
                        }
                    }
                }
            }

            UqText {
                Layout.fillWidth: true
                visible: root.prompt !== ""
                text: root.prompt
            }
            UqInput {
                id: field
                Layout.fillWidth: true
                enabled: root.flow !== null && root.flow.isResponseRequired
                // Hidden responses (passwords) get the lock field's echo and hints.
                secret: root.flow === null || !root.flow.responseVisible
                Accessible.name: root.prompt !== "" ? root.prompt : "Password"
                Keys.onEscapePressed: root.cancel()
                onAccepted: root.submit()
                // Re-take focus when polkit asks again (after a failed try or an identity change).
                onEnabledChanged: if (enabled && root.shown) field.forceActiveFocus()
            }
            UqText {
                Layout.fillWidth: true
                visible: root.supplementary !== ""
                text: root.supplementary
                color: root.flow && root.flow.supplementaryIsError ? Theme.error : Theme.textMuted
                wrapMode: Text.Wrap
                elide: Text.ElideNone
            }

            RowLayout {
                Layout.alignment: Qt.AlignRight
                spacing: Theme.gapIn * 2

                Rectangle {
                    id: cancelButton
                    Layout.preferredWidth: cancelText.implicitWidth + Theme.gapOut * 2
                    Layout.preferredHeight: 32
                    radius: Theme.radius - 2
                    color: cancelMouse.containsMouse || activeFocus ? Theme.overlay : Theme.surfaceAlt
                    border.color: Theme.accent
                    border.width: activeFocus ? 1 : 0
                    activeFocusOnTab: true
                    Accessible.role: Accessible.Button
                    Accessible.name: "Cancel"
                    Accessible.onPressAction: root.cancel()
                    Keys.onReturnPressed: root.cancel()
                    Keys.onEnterPressed: root.cancel()
                    Keys.onSpacePressed: root.cancel()
                    Keys.onEscapePressed: root.cancel()

                    UqText {
                        id: cancelText
                        anchors.centerIn: parent
                        text: "Cancel"
                    }
                    MouseArea {
                        id: cancelMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        onClicked: root.cancel()
                    }
                }

                Rectangle {
                    id: okButton
                    Layout.preferredWidth: okText.implicitWidth + Theme.gapOut * 2
                    Layout.preferredHeight: 32
                    radius: Theme.radius - 2
                    color: Theme.accent
                    opacity: field.enabled ? 1 : 0.5
                    border.color: Theme.text
                    border.width: activeFocus ? 1 : 0
                    activeFocusOnTab: true
                    Accessible.role: Accessible.Button
                    Accessible.name: "Authenticate"
                    Accessible.onPressAction: root.submit()
                    Keys.onReturnPressed: root.submit()
                    Keys.onEnterPressed: root.submit()
                    Keys.onSpacePressed: root.submit()
                    Keys.onEscapePressed: root.cancel()

                    UqText {
                        id: okText
                        anchors.centerIn: parent
                        text: "Authenticate"
                        color: Theme.accentText
                    }
                    MouseArea {
                        anchors.fill: parent
                        onClicked: root.submit()
                    }
                }
            }
        }
    }
}
