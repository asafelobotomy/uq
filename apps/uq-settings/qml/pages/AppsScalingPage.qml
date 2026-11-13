// Apps and scaling (spec §6): five switches that help apps look sharp on a scaled screen, the
// older apps open right now that may look blurry (Sharp apps tab), and each app's own fix, how uQ
// starts it (App fixes tab).
// Everything goes through CompatModel. Window titles, app names and the fixes' own settings are
// shown through UqLabel (plain text) only; the fix dialog's title is a fixed phrase.
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
            return JSON.parse(cm.stateJson);
        } catch (e) {
            return {};
        }
    }
    readonly property var blurry: snap.blurry || ({})
    readonly property var fixes: snap.fixes || []
    readonly property var limits: (snap.fields || {}).limits || ({})
    readonly property bool ready: App.uqdUp && snap.error === undefined
    readonly property bool idle: page.ready && !cm.busy
    /// Hyprland's or uQ's message for the last change that wasn't applied, or "".
    readonly property string sectionMessage: page.snap.section && page.snap.section.source === "error" ? String(page.snap.section.message || "") : ""
    /// The last reopen note shown, so each is shown once.
    property int reopenSeen: 0

    /// Read everything again; Main calls it when the window comes back to the front.
    function refresh() {
        cm.refresh();
    }
    function showNews() {
        // The dialog's save has ended (busy goes false before the error and the state arrive).
        if (fixDialog.saving && !cm.busy)
            fixDialog.finishSave(cm.error);
        const reopen = page.snap.reopen;
        if (reopen && reopen.seq > page.reopenSeen) {
            page.reopenSeen = reopen.seq;
            reopenMessage.show([reopenMessage.reopenText(reopen.names, reopen.more)]);
        }
    }
    /// The fix for app `id`, or null.
    function fixFor(id) {
        return page.fixes.find(f => f.id === id) || null;
    }
    /// What uQ did about a fix's launcher file copy, in plain words ("" for nothing to say).
    function overrideText(fix) {
        const origin = fix.origin || {};
        if (origin.source === "error" && fix.message)
            return "";
        switch (fix.override) {
        case "written":
            return "Other launchers use this fix too.";
        case "own_file":
            return "You have your own copy of this app's launcher file, so uQ left it alone.";
        case "no_source":
            return "uQ can't find this app's launcher file.";
        case "error":
            return fix.message ? String(fix.message) : "uQ couldn't save its copy of this app's launcher file.";
        default:
            return "";
        }
    }
    /// "Run on Wayland, size 150 %, 2 extra settings, 1 extra option".
    function fixSummary(fix) {
        const out = [];
        const platform = ((page.snap.fields || {}).platforms || []).find(p => p.key === fix.platform);
        out.push("Run on " + (platform ? platform.label : "Automatic"));
        out.push(fix.scale === null || fix.scale === undefined ? "size Automatic" : "size " + Math.round(fix.scale * 100) + " %");
        const env = (fix.env || []).length;
        if (env > 0)
            out.push(env === 1 ? "1 extra setting" : env + " extra settings");
        const args = (fix.args || []).length;
        if (args > 0)
            out.push(args === 1 ? "1 extra option" : args + " extra options");
        return out.join(", ");
    }
    /// "Fix this app" for a window of the Blurry right now list.
    function fixWindow(w) {
        if (w.app_id)
            fixDialog.start(String(w.app_id), String(w.app_name || w.app_id), page.fixFor(w.app_id));
        else
            fixDialog.startChoosing(String(w.title || w["class"] || ""));
    }

    /// An icon for each switch, by key.
    readonly property var switchIcons: ({
            "sharp_x11": "xorg",
            "electron_wayland": "applications-development",
            "chromium_wayland": "internet-web-browser",
            "qt_wayland": "applications-utilities",
            "steam_scale": "applications-games"
        })

    Component.onCompleted: cm.refresh()

    CompatModel {
        id: cm
        onStateJsonChanged: page.showNews()
    }
    Connections {
        target: App
        function onUqdUpChanged() {
            if (App.uqdUp)
                cm.refresh();
        }
    }

    AppFixDialog {
        id: fixDialog
        fields: page.snap.fields || ({})
        apps: page.snap.installed || []
        existingFix: id => page.fixFor(id)
        preview: json => cm.preview(json)
        canSave: page.idle
        busy: cm.busy
        errorNow: () => cm.error
        onFixAccepted: json => {
            cm.saveFix(json);
            // No request went out (no session bus, or the fix couldn't be read): say so here.
            if (!cm.busy)
                fixDialog.finishSave(cm.error !== "" ? cm.error : "uQ isn't running");
        }
    }

    Kirigami.PromptDialog {
        id: confirmRemove

        property string appId

        title: "Remove this fix?"
        subtitle: "The app starts as it did before. You can undo this from the history."
        standardButtons: Kirigami.Dialog.Cancel
        customFooterActions: [
            Kirigami.Action {
                text: "Remove"
                icon.name: "edit-delete"
                enabled: page.idle
                onTriggered: {
                    cm.removeFix(confirmRemove.appId);
                    confirmRemove.close();
                }
            }
        ]
    }

    banners: [
        ErrorCard {
            objectName: "uqdDown"
            visible: !page.ready
            title: "uQ's settings service isn't running"
            detail: "Start the uQ desktop service to see and change these settings."
            Layout.fillWidth: true
        },
        RuleMessage {
            objectName: "sectionError"
            negative: true
            closable: false
            lines: page.sectionMessage !== "" ? ["The last change here wasn't applied: " + page.sectionMessage] : []
            Layout.fillWidth: true
        },
        RuleMessage {
            objectName: "compatError"
            negative: true
            closable: false
            lines: cm.error !== "" && !(page.sectionMessage !== "" && cm.error.indexOf(page.sectionMessage) >= 0) ? ["That wasn't done: " + cm.error] : []
            Layout.fillWidth: true
        },
        RuleMessage {
            id: reopenMessage
            objectName: "reopenMessage"
            Layout.fillWidth: true
        }
    ]

    SettingsTab {
        title: "Sharp apps"
        iconName: "zoom-in"

        // The five switches, each with its explanation as a tip; under the browsers' one, the
        // flag files.
        SettingsCard {
            title: "Run apps sharp"
            iconName: "zoom-in"
            help: "These change how apps are started, so they apply to apps started after a change."

            Repeater {
                model: page.snap.switches || []
                delegate: ColumnLayout {
                    id: switchBlock
                    required property var modelData
                    spacing: 0
                    Layout.fillWidth: true

                    SwitchRow {
                        settingId: String(switchBlock.modelData.key).replace(/_/g, "-")
                        iconName: page.switchIcons[switchBlock.modelData.key] || "preferences-desktop-display"
                        label: String(switchBlock.modelData.label)
                        help: String(switchBlock.modelData.description || "")
                        value: switchBlock.modelData.on === true
                        source: switchBlock.modelData.origin || null
                        highlight: App.highlight
                        enabled: page.idle
                        Layout.fillWidth: true
                        onChosen: on => cm.setSwitch(String(switchBlock.modelData.key), on)
                    }
                    Repeater {
                        model: switchBlock.modelData.key === "chromium_wayland" ? (page.snap.browsers || []).filter(b => b.message) : []
                        delegate: UqLabel {
                            required property var modelData
                            objectName: "browserMessage"
                            text: String(modelData.message)
                            color: modelData.file === "error" ? Kirigami.Theme.negativeTextColor : Kirigami.Theme.textColor
                            wrapMode: Text.Wrap
                            Layout.fillWidth: true
                            Layout.leftMargin: Kirigami.Units.gridUnit * 2
                        }
                    }
                }
            }
        }

        // Blurry right now: the older apps that are open, each with its icon.
        SettingsCard {
            title: "Blurry right now"
            iconName: "view-preview"
            actions: [
                QQC2.ToolButton {
                    objectName: "refreshWindows"
                    text: "Refresh"
                    icon.name: "view-refresh"
                    display: QQC2.AbstractButton.IconOnly
                    enabled: page.idle
                    Accessible.name: "Refresh the list of open apps"
                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.text: "Refresh"
                    onClicked: cm.refreshWindows()
                }
            ]

            SettingRow {
                objectName: "blurryHeading"
                settingId: "blurry-now"
                iconName: page.blurry.scaled === true && (page.blurry.windows || []).length > 0 ? "dialog-warning" : "dialog-ok"
                label: String(page.blurry.heading || "")
                highlight: App.highlight
                enabled: page.ready
                Layout.fillWidth: true
            }
            UqLabel {
                visible: page.ready && page.blurry.scaled === true && (page.blurry.windows || []).length === 0
                text: "No older apps are open."
                opacity: 0.7
                Layout.leftMargin: Kirigami.Units.largeSpacing
            }
            Repeater {
                model: page.blurry.scaled === true ? (page.blurry.windows || []) : []
                delegate: RowLayout {
                    id: windowRow
                    required property var modelData
                    readonly property string appText: windowRow.modelData.app_name ? String(windowRow.modelData.app_name) : String(windowRow.modelData["class"] || "")
                    spacing: Kirigami.Units.largeSpacing
                    Layout.fillWidth: true
                    Layout.leftMargin: Kirigami.Units.largeSpacing
                    Layout.rightMargin: Kirigami.Units.largeSpacing

                    Kirigami.Icon {
                        objectName: "windowIcon"
                        source: String(windowRow.modelData.app_icon || "") !== "" ? String(windowRow.modelData.app_icon) : "application-x-executable"
                        implicitWidth: Kirigami.Units.iconSizes.medium
                        implicitHeight: Kirigami.Units.iconSizes.medium
                        Accessible.ignored: true
                    }
                    ColumnLayout {
                        spacing: 0
                        Layout.fillWidth: true
                        UqLabel {
                            text: windowRow.appText
                            elide: Text.ElideRight
                            Layout.fillWidth: true
                        }
                        UqLabel {
                            text: String(windowRow.modelData.title || "")
                            elide: Text.ElideRight
                            opacity: 0.7
                            Layout.fillWidth: true
                        }
                    }
                    QQC2.Button {
                        objectName: "fixWindow"
                        text: "Fix this app"
                        icon.name: "tools-wizard"
                        enabled: page.idle
                        Accessible.description: windowRow.appText
                        onClicked: page.fixWindow(windowRow.modelData)
                    }
                }
            }
        }
    }

    SettingsTab {
        title: "App fixes"
        iconName: "tools-wizard"

        SettingsCard {
            title: "App fixes"
            iconName: "tools-wizard"
            help: "How uQ starts one app: on Wayland or X11, at its own size, with extra settings."
            wide: true

            SettingRow {
                settingId: "app-fixes"
                iconName: page.fixes.length === 0 ? "dialog-information" : ""
                label: page.ready && page.fixes.length === 0 ? "No app has a fix yet" : ""
                highlight: App.highlight
                enabled: page.ready
                Layout.fillWidth: true

                QQC2.Button {
                    objectName: "addFix"
                    text: "Add a fix"
                    icon.name: "list-add"
                    enabled: page.idle && page.fixes.length < (page.limits.apps || 100)
                    onClicked: fixDialog.startChoosing("")
                }
            }

            // The fixes as tiles, two or three to a row at full width.
            GridLayout {
                columns: Math.max(1, Math.floor(width / (Kirigami.Units.gridUnit * 26)))
                columnSpacing: Kirigami.Units.largeSpacing
                rowSpacing: Kirigami.Units.largeSpacing
                uniformCellWidths: true
                Layout.fillWidth: true

                Repeater {
                    model: page.fixes
                    delegate: Rectangle {
                        id: fixRow
                        required property var modelData
                        readonly property string appName: String(fixRow.modelData.name || fixRow.modelData.id)
                        readonly property string overrideLine: page.overrideText(fixRow.modelData)
                        radius: Kirigami.Units.cornerRadius
                        color: Kirigami.Theme.backgroundColor
                        border.width: 1
                        border.color: Qt.rgba(Kirigami.Theme.textColor.r, Kirigami.Theme.textColor.g, Kirigami.Theme.textColor.b, 0.1)
                        implicitHeight: fixColumn.implicitHeight + Kirigami.Units.largeSpacing * 2
                        implicitWidth: fixColumn.implicitWidth + Kirigami.Units.largeSpacing * 2
                        Layout.fillWidth: true
                        Layout.alignment: Qt.AlignTop
                        Accessible.role: Accessible.Grouping
                        Accessible.name: fixRow.appName

                        ColumnLayout {
                            id: fixColumn
                            anchors.fill: parent
                            anchors.margins: Kirigami.Units.largeSpacing
                            spacing: Kirigami.Units.smallSpacing

                            RowLayout {
                                spacing: Kirigami.Units.largeSpacing
                                Layout.fillWidth: true

                                Kirigami.Icon {
                                    source: String(fixRow.modelData.icon || "") !== "" ? String(fixRow.modelData.icon) : "application-x-executable"
                                    implicitWidth: Kirigami.Units.iconSizes.large
                                    implicitHeight: Kirigami.Units.iconSizes.large
                                    Accessible.ignored: true
                                }
                                ColumnLayout {
                                    spacing: 0
                                    Layout.fillWidth: true
                                    UqLabel {
                                        objectName: "fixName"
                                        text: fixRow.appName
                                        font.bold: true
                                        elide: Text.ElideRight
                                        Layout.fillWidth: true
                                    }
                                    UqLabel {
                                        text: page.fixSummary(fixRow.modelData)
                                        wrapMode: Text.Wrap
                                        opacity: 0.7
                                        Layout.fillWidth: true
                                    }
                                }
                                QQC2.ToolButton {
                                    text: "Edit"
                                    icon.name: "document-edit"
                                    display: QQC2.AbstractButton.IconOnly
                                    enabled: page.idle
                                    Accessible.description: fixRow.appName
                                    QQC2.ToolTip.visible: hovered
                                    QQC2.ToolTip.text: "Edit"
                                    onClicked: fixDialog.start(String(fixRow.modelData.id), fixRow.appName, fixRow.modelData)
                                }
                                QQC2.ToolButton {
                                    text: "Remove"
                                    icon.name: "edit-delete"
                                    display: QQC2.AbstractButton.IconOnly
                                    enabled: page.idle
                                    Accessible.description: fixRow.appName
                                    QQC2.ToolTip.visible: hovered
                                    QQC2.ToolTip.text: "Remove"
                                    onClicked: {
                                        confirmRemove.appId = String(fixRow.modelData.id);
                                        confirmRemove.open();
                                    }
                                }
                            }
                            QQC2.CheckBox {
                                id: elsewhereBox
                                text: "Also when started from other launchers"
                                checked: fixRow.modelData.elsewhere === true
                                enabled: page.idle
                                Accessible.description: fixRow.appName
                                onToggled: {
                                    cm.setElsewhere(String(fixRow.modelData.id), elsewhereBox.checked);
                                    elsewhereBox.checked = Qt.binding(() => fixRow.modelData.elsewhere === true);
                                }
                            }
                            UqLabel {
                                objectName: "overrideState"
                                visible: fixRow.overrideLine !== ""
                                text: fixRow.overrideLine
                                color: fixRow.modelData.override === "error" ? Kirigami.Theme.negativeTextColor : Kirigami.Theme.textColor
                                wrapMode: Text.Wrap
                                opacity: fixRow.modelData.override === "written" ? 0.7 : 1
                                Layout.fillWidth: true
                            }
                            SourceLine {
                                source: fixRow.modelData.origin || null
                                textName: "fixOrigin"
                                Layout.fillWidth: true
                            }
                        }
                    }
                }
            }
        }
    }
}
