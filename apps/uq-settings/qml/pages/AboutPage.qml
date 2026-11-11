// About (spec §2.6): the computer's name (changing it asks for an administrator password), the
// system, the hardware and the versions of uQ and what it runs on. Names and versions come from
// files and other programs, so every line is a UqLabel (plain text). Two slots are kept for later
// work: "Integrations" (aboutIntegrations) and the "Show Welcome" button (showWelcome).
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../components"

QQC2.ScrollView {
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

    Accessible.role: Accessible.Pane
    Accessible.name: "About"
    contentWidth: availableWidth

    AboutModel {
        id: am
        onStateJsonChanged: {
            page.showNews();
            if (!hostField.activeFocus)
                hostField.text = String(page.host.name || "");
        }
    }

    ColumnLayout {
        width: Math.min(page.availableWidth - Kirigami.Units.largeSpacing * 2, Kirigami.Units.gridUnit * 44)
        x: Math.max(Kirigami.Units.largeSpacing, (page.availableWidth - width) / 2)
        spacing: Kirigami.Units.smallSpacing

        Item {
            implicitHeight: Kirigami.Units.smallSpacing
        }
        UqLabel {
            text: "About"
            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.5
            font.bold: true
            Accessible.role: Accessible.Heading
        }
        RuleMessage {
            objectName: "aboutError"
            negative: true
            closable: false
            lines: am.error !== "" ? ["That wasn't done: " + am.error] : []
            Layout.fillWidth: true
        }
        RuleMessage {
            id: noticeMessage
            Layout.fillWidth: true
        }

        SettingRow {
            settingId: "hostname"
            label: "Computer name"
            highlight: App.highlight
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.largeSpacing

            QQC2.TextField {
                id: hostField
                objectName: "hostname"
                enabled: page.idle && page.host.available === true
                maximumLength: 64
                Accessible.name: "Computer name"
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
        UqLabel {
            text: "Other computers on the network see this name."
            wrapMode: Text.Wrap
            opacity: 0.7
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing
        }
        ErrorCard {
            objectName: "hostnameMissing"
            visible: page.loaded && page.host.available === false
            title: "The name can't be changed here"
            detail: String(page.host.text || "")
            Layout.fillWidth: true
        }

        GridLayout {
            columns: 2
            columnSpacing: Kirigami.Units.largeSpacing
            rowSpacing: Kirigami.Units.smallSpacing
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.largeSpacing
            Layout.leftMargin: Kirigami.Units.largeSpacing

            UqLabel {
                text: "System"
                opacity: 0.7
            }
            UqLabel {
                objectName: "osName"
                text: page.snap.os ? String(page.snap.os.name) + (page.snap.os.version ? " (" + String(page.snap.os.version) + ")" : "") : ""
                wrapMode: Text.Wrap
                Layout.fillWidth: true
            }
            UqLabel {
                text: "Kernel"
                opacity: 0.7
            }
            UqLabel {
                text: page.orUnknown(page.snap.kernel)
                Layout.fillWidth: true
            }
            UqLabel {
                visible: !!page.host.hardware
                text: "Computer"
                opacity: 0.7
            }
            UqLabel {
                visible: !!page.host.hardware
                text: String(page.host.hardware || "")
                wrapMode: Text.Wrap
                Layout.fillWidth: true
            }
            UqLabel {
                text: "Processor"
                opacity: 0.7
            }
            UqLabel {
                text: page.snap.cpu ? page.orUnknown(page.snap.cpu.model) + (page.snap.cpu.cores > 0 ? " × " + page.snap.cpu.cores : "") : ""
                wrapMode: Text.Wrap
                Layout.fillWidth: true
            }
            UqLabel {
                text: "Graphics"
                opacity: 0.7
            }
            UqLabel {
                objectName: "gpus"
                text: page.snap.gpus && page.snap.gpus.length > 0 ? page.snap.gpus.map(g => String(g)).join("\n") : "Unknown"
                wrapMode: Text.Wrap
                Layout.fillWidth: true
            }
            UqLabel {
                text: "Memory"
                opacity: 0.7
            }
            UqLabel {
                text: page.orUnknown(page.snap.memory)
                Layout.fillWidth: true
            }
        }

        SettingRow {
            settingId: "versions"
            label: "Versions"
            highlight: App.highlight
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.largeSpacing
        }
        GridLayout {
            columns: 2
            columnSpacing: Kirigami.Units.largeSpacing
            rowSpacing: Kirigami.Units.smallSpacing
            Layout.leftMargin: Kirigami.Units.largeSpacing

            UqLabel {
                text: "uQ"
                opacity: 0.7
            }
            UqLabel {
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

        // Kept for Phase 5's Welcome: shown once Welcome exists.
        QQC2.Button {
            id: showWelcome
            objectName: "showWelcome"
            visible: false
            text: "Show Welcome"
            icon.name: "help-about"
            Layout.topMargin: Kirigami.Units.largeSpacing
        }

        // Settings → About → Integrations is added here later; it loads into this slot.
        Loader {
            id: integrationsLoader
            objectName: "aboutIntegrations"
            active: false
            Layout.fillWidth: true
        }
        Item {
            implicitHeight: Kirigami.Units.largeSpacing
        }
    }
}
