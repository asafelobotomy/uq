// About (spec §2.6): the computer's name (changing it asks for an administrator password), the
// system, the hardware and the versions of uQ and what it runs on. Names and versions come from
// files and other programs, so every line is a UqLabel (plain text). "Show Welcome" (showWelcome)
// opens Welcome again.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../components"

SettingsPage {
    id: page

    readonly property var snap: {
        try {
            return JSON.parse(am.stateJson);
        } catch (e) {
            return {};
        }
    }
    readonly property bool loaded: snap.hostname !== undefined
    readonly property var host: snap.hostname || ({})
    readonly property bool idle: page.loaded && !am.busy
    readonly property var versions: snap.versions || ({})
    property int noticeSeen: 0

    function refresh() {
        am.refresh();
    }
    function showNews() {
        const notice = page.snap.notice;
        if (notice && notice.seq > page.noticeSeen) {
            page.noticeSeen = notice.seq;
            noticeMessage.show(notice.text !== "" ? [notice.text] : []);
        }
    }
    function orUnknown(s) {
        return s && String(s) !== "" ? String(s) : "Unknown";
    }
    readonly property string osText: page.snap.os ? String(page.snap.os.name) : ""
    /// The hardware, one `{icon, name, text, id}` per line.
    readonly property var hardware: {
        const out = [];
        if (page.host.hardware && String(page.host.hardware).trim() !== "")
            out.push({ id: "model", icon: "computer", name: "Computer", text: String(page.host.hardware) });
        const cpu = page.snap.cpu;
        out.push({ id: "cpu", icon: "cpu", name: "Processor",
                   text: cpu ? page.orUnknown(cpu.model) + (cpu.cores > 0 ? " × " + cpu.cores : "") : "Unknown" });
        out.push({ id: "gpus", icon: "video-display", name: "Graphics",
                   text: page.snap.gpus && page.snap.gpus.length > 0 ? page.snap.gpus.map(g => String(g)).join("\n") : "Unknown" });
        out.push({ id: "memory", icon: "memory", name: "Memory", text: page.orUnknown(page.snap.memory) });
        return out;
    }

    AboutModel {
        id: am
        onStateJsonChanged: {
            page.showNews();
            if (!hostField.activeFocus)
                hostField.text = String(page.host.name || "");
        }
    }

    banners: [
        RuleMessage {
            objectName: "aboutError"
            negative: true
            closable: false
            lines: am.error !== "" ? ["That wasn't done: " + am.error] : []
            Layout.fillWidth: true
        },
        RuleMessage {
            id: noticeMessage
            Layout.fillWidth: true
        }
    ]

    SettingsTab {
        title: "About"
        iconName: "help-about"

        SettingsCard {
            title: "This computer"
            iconName: "computer"
            actions: [
                // Welcome again from its first step, with every step (Phase 5 spec §5.1).
                QQC2.Button {
                    id: showWelcome
                    objectName: "showWelcome"
                    text: "Show Welcome"
                    icon.name: "help-about"
                    flat: true
                    Accessible.description: "The first-run tour: look, wallpaper, layouts, apps and the keys to know"
                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                    QQC2.ToolTip.text: "The first-run tour again"
                    onClicked: App.showWelcome()
                }
            ]

            // The system, large.
            RowLayout {
                spacing: Kirigami.Units.largeSpacing
                Layout.fillWidth: true
                Layout.leftMargin: Kirigami.Units.largeSpacing
                Layout.topMargin: Kirigami.Units.smallSpacing
                Layout.bottomMargin: Kirigami.Units.smallSpacing

                Kirigami.Icon {
                    source: "computer"
                    implicitWidth: Kirigami.Units.iconSizes.huge
                    implicitHeight: Kirigami.Units.iconSizes.huge
                    Accessible.ignored: true
                }
                ColumnLayout {
                    spacing: 0
                    Layout.fillWidth: true
                    UqLabel {
                        text: String(page.host.name || "")
                        font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.6
                        font.bold: true
                        elide: Text.ElideRight
                        Layout.fillWidth: true
                    }
                    UqLabel {
                        objectName: "osName"
                        text: page.osText + (page.snap.os && page.snap.os.version ? " (" + String(page.snap.os.version) + ")" : "")
                        wrapMode: Text.Wrap
                        Layout.fillWidth: true
                    }
                    UqLabel {
                        text: "Kernel " + page.orUnknown(page.snap.kernel)
                        opacity: 0.7
                        Layout.fillWidth: true
                    }
                }
            }
            SettingRow {
                settingId: "hostname"
                iconName: "network-workgroup"
                label: "Name"
                help: "Other computers on the network see this name. Changing it asks for an administrator password."
                highlight: App.highlight
                Layout.fillWidth: true

                QQC2.TextField {
                    id: hostField
                    objectName: "hostname"
                    enabled: page.idle && page.host.available === true
                    maximumLength: 64
                    Accessible.name: "Computer name"
                    Layout.preferredWidth: Kirigami.Units.gridUnit * 12
                    onAccepted: saveHost.clicked()
                }
                QQC2.Button {
                    id: saveHost
                    text: "Save"
                    icon.name: "document-save"
                    enabled: page.idle && page.host.available === true && hostField.text.trim() !== "" && hostField.text.trim() !== String(page.host.name || "")
                    onClicked: am.setHostname(hostField.text.trim())
                }
            }
            ErrorCard {
                objectName: "hostnameMissing"
                visible: page.loaded && page.host.available === false
                title: "The name can't be changed here"
                detail: String(page.host.text || "")
                Layout.fillWidth: true
            }
            SettingRow {
                settingId: "versions"
                iconName: "dialog-information"
                label: "Versions"
                highlight: App.highlight
                Layout.fillWidth: true
            }
            GridLayout {
                columns: 2
                columnSpacing: Kirigami.Units.largeSpacing * 2
                rowSpacing: Kirigami.Units.smallSpacing
                Layout.leftMargin: Kirigami.Units.largeSpacing * 2 + Kirigami.Units.iconSizes.smallMedium
                Layout.bottomMargin: Kirigami.Units.smallSpacing

                UqLabel {
                    text: "uQ"
                    opacity: 0.7
                }
                UqLabel {
                    objectName: "uqVersion"
                    text: page.orUnknown(page.versions.uq)
                }
                UqLabel {
                    text: "Hyprland"
                    opacity: 0.7
                }
                UqLabel {
                    text: page.versions.hyprland ? String(page.versions.hyprland) : "Not found"
                }
                UqLabel {
                    text: "Quickshell"
                    opacity: 0.7
                }
                UqLabel {
                    text: page.versions.quickshell ? String(page.versions.quickshell) : "Not found"
                }
            }
        }

        // The hardware, an icon for each part.
        SettingsCard {
            title: "Hardware"
            iconName: "hwinfo"

            Repeater {
                model: page.hardware
                delegate: RowLayout {
                    required property var modelData
                    spacing: Kirigami.Units.largeSpacing
                    Layout.fillWidth: true
                    Layout.leftMargin: Kirigami.Units.largeSpacing
                    Layout.rightMargin: Kirigami.Units.largeSpacing
                    Layout.topMargin: Kirigami.Units.smallSpacing
                    Accessible.role: Accessible.StaticText
                    Accessible.name: modelData.name + ": " + modelData.text

                    Kirigami.Icon {
                        source: modelData.icon
                        implicitWidth: Kirigami.Units.iconSizes.medium
                        implicitHeight: Kirigami.Units.iconSizes.medium
                        Layout.alignment: Qt.AlignTop
                        Accessible.ignored: true
                    }
                    ColumnLayout {
                        spacing: 0
                        Layout.fillWidth: true
                        UqLabel {
                            text: modelData.name
                            font.pointSize: Kirigami.Theme.smallFont.pointSize
                            opacity: 0.7
                        }
                        UqLabel {
                            objectName: modelData.id
                            text: modelData.text
                            wrapMode: Text.Wrap
                            Layout.fillWidth: true
                        }
                    }
                }
            }
        }
    }
}
