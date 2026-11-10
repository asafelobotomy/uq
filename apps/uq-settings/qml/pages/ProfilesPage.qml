// Profiles: named sets of settings that are turned on together (spec §4), one card each with
// Turn on or Back to normal, the settings it holds (removable), Add settings…, Rename…, Export…
// and Delete; a New profile dialog that takes the settings changed lately or the current
// settings of some pages; and Import…. Everything goes through ProfilesModel. Profile names and
// settings come from the user or from imported files, so they are shown through UqLabel only.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Dialogs
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../components"

QQC2.ScrollView {
    id: page

    readonly property var snap: {
        try {
            return JSON.parse(pm.stateJson);
        } catch (e) {
            return {};
        }
    }
    readonly property var profiles: snap.profiles || []
    readonly property string active: snap.active || ""
    /// The current value of every setting a profile can hold: `[{domain, path, label, text}]`.
    readonly property var available: snap.available || []
    /// The pages a profile can take the current settings of: `[{id, name}]`.
    readonly property var pages: snap.pages || []
    readonly property var icons: snap.icons || []
    readonly property bool ready: App.uqdUp
    readonly property bool idle: page.ready && !pm.busy
    /// The last notice, report and conflict shown, so each is shown once.
    property int noticeSeen: 0
    property int reportSeen: 0
    property int conflictSeen: 0
    /// The "changed in" choices of the New profile dialog.
    readonly property var periods: ["the last hour", "today", "the last 7 days", "the last 30 days"]
    /// The names read out for the offered icons.
    readonly property var iconNames: ({
        "preferences-desktop": "Desktop",
        "input-gaming": "Game controller",
        "video-display": "Screen",
        "battery": "Battery",
        "weather-clear-night": "Night",
        "applications-office": "Office",
        "media-playback-start": "Play"
    })

    /// Unix seconds where the "changed in" choice `index` starts.
    function sinceOf(index) {
        const now = new Date();
        const secs = Math.floor(now.getTime() / 1000);
        switch (index) {
        case 0:
            return secs - 3600;
        case 1:
            return Math.floor(new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime() / 1000);
        case 2:
            return secs - 7 * 86400;
        default:
            return secs - 30 * 86400;
        }
    }
    function keyId(k) {
        return k.domain + "\u0000" + JSON.stringify(k.path);
    }
    function settingsCount(n) {
        return n === 1 ? "1 setting" : n + " settings";
    }
    /// Read everything again; Main calls it when the window comes back to the front.
    function refresh() {
        pm.refresh();
    }
    /// Show what the last request said: a notice, an apply's report, or a conflict to ask about.
    function showNews() {
        let snap = {};
        try {
            snap = JSON.parse(pm.stateJson);
        } catch (e) {
            return;
        }
        const notice = snap.notice;
        if (notice && notice.seq > page.noticeSeen) {
            page.noticeSeen = notice.seq;
            // A fixed phrase from pages::profiles.
            toast.text = notice.text;
            toast.visible = notice.text !== "";
            toastTimer.restart();
        }
        const report = snap.report;
        if (report && report.seq > page.reportSeen) {
            page.reportSeen = report.seq;
            // Plain setting labels and uqd's fixed notices; no profile names.
            const lines = [];
            if ((report.kept || []).length > 0)
                lines.push("Kept your own value: " + report.kept.join(", "));
            for (const n of report.notices || [])
                lines.push(n);
            reportMessage.text = lines.join("\n");
            reportMessage.visible = lines.length > 0;
        }
        const conflict = snap.conflict;
        if (conflict && conflict.seq > page.conflictSeen) {
            page.conflictSeen = conflict.seq;
            conflictDialog.path = conflict.path;
            conflictMessage.text = conflict.message;
            conflictDialog.open();
        }
    }

    Component.onCompleted: pm.refresh()
    Accessible.role: Accessible.Pane
    Accessible.name: "Profiles"
    contentWidth: availableWidth

    ProfilesModel {
        id: pm
        onStateJsonChanged: page.showNews()
    }

    Connections {
        target: App
        function onUqdUpChanged() {
            if (App.uqdUp)
                pm.refresh();
        }
    }

    // A row of the offered icons; `chosen` is the picked icon name. An inline component can't see
    // the page's ids, so it takes the icons and their names as properties.
    component IconChoice: Flow {
        id: choice

        property var icons: []
        property var names: ({})
        property string chosen

        spacing: Kirigami.Units.smallSpacing

        Repeater {
            model: choice.icons
            delegate: QQC2.ToolButton {
                required property string modelData

                icon.name: modelData
                display: QQC2.AbstractButton.IconOnly
                checkable: true
                checked: choice.chosen === modelData
                Accessible.name: choice.names[modelData] || modelData
                QQC2.ToolTip.text: choice.names[modelData] || modelData
                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                onClicked: choice.chosen = modelData
            }
        }
    }

    Kirigami.Dialog {
        id: newDialog

        /// The ids of the pages ticked under "The current settings of these pages".
        property var pickedPages: []
        readonly property bool fromPages: fromPagesButton.checked
        readonly property bool canCreate: page.idle && newName.text.trim() !== "" && (!fromPages || pickedPages.length > 0)

        function start() {
            newName.clear();
            newIcon.chosen = page.icons.length > 0 ? page.icons[0] : "";
            fromHistoryButton.checked = true;
            period.currentIndex = 1;
            newDialog.pickedPages = [];
            newDialog.open();
            newName.forceActiveFocus();
        }
        function create() {
            if (!newDialog.canCreate)
                return;
            if (newDialog.fromPages)
                pm.createFromPages(newName.text, newIcon.chosen, JSON.stringify(newDialog.pickedPages));
            else
                pm.createFromHistory(newName.text, newIcon.chosen, page.sinceOf(period.currentIndex));
            newDialog.close();
        }

        title: "New profile"
        standardButtons: Kirigami.Dialog.Cancel
        preferredWidth: Kirigami.Units.gridUnit * 26
        customFooterActions: [
            Kirigami.Action {
                text: "Create"
                icon.name: "list-add"
                enabled: newDialog.canCreate
                onTriggered: newDialog.create()
            }
        ]

        ColumnLayout {
            spacing: Kirigami.Units.smallSpacing

            UqLabel {
                text: "Name"
            }
            QQC2.TextField {
                id: newName
                objectName: "newProfileName"
                maximumLength: 100
                placeholderText: "Such as Gaming or Presentation"
                Accessible.name: "Name of the new profile"
                Layout.fillWidth: true
                onAccepted: newDialog.create()
            }
            UqLabel {
                text: "Icon"
                Layout.topMargin: Kirigami.Units.smallSpacing
            }
            IconChoice {
                id: newIcon
                icons: page.icons
                names: page.iconNames
                Layout.fillWidth: true
            }
            UqLabel {
                text: "Start with"
                Layout.topMargin: Kirigami.Units.smallSpacing
            }
            QQC2.ButtonGroup {
                id: sourceGroup
            }
            RowLayout {
                QQC2.RadioButton {
                    id: fromHistoryButton
                    text: "Settings I changed"
                    checked: true
                    QQC2.ButtonGroup.group: sourceGroup
                }
                UqComboBox {
                    id: period
                    model: page.periods
                    currentIndex: 1
                    enabled: fromHistoryButton.checked
                    Accessible.name: "Changed in"
                }
            }
            QQC2.RadioButton {
                id: fromPagesButton
                text: "The current settings of these pages"
                QQC2.ButtonGroup.group: sourceGroup
            }
            Repeater {
                model: page.pages
                delegate: RowLayout {
                    id: pageRow

                    required property var modelData
                    readonly property bool picked: newDialog.pickedPages.indexOf(modelData.id) >= 0

                    enabled: newDialog.fromPages
                    Layout.leftMargin: Kirigami.Units.largeSpacing * 2

                    QQC2.CheckBox {
                        id: pageBox
                        checked: pageRow.picked
                        Accessible.name: pageRow.modelData.name
                        onToggled: {
                            const others = newDialog.pickedPages.filter(id => id !== pageRow.modelData.id);
                            newDialog.pickedPages = pageBox.checked ? others.concat([pageRow.modelData.id]) : others;
                            pageBox.checked = Qt.binding(() => pageRow.picked);
                        }
                    }
                    UqLabel {
                        text: pageRow.modelData.name
                        MouseArea {
                            anchors.fill: parent
                            enabled: pageBox.enabled
                            onClicked: {
                                pageBox.toggle();
                                pageBox.toggled();
                            }
                        }
                    }
                }
            }
        }
    }

    Kirigami.Dialog {
        id: renameDialog

        property string profileId

        function start(p) {
            renameDialog.profileId = p.id;
            renameName.text = p.name;
            renameIcon.chosen = p.icon;
            renameDialog.open();
            renameName.forceActiveFocus();
        }
        function save() {
            if (!page.idle || renameName.text.trim() === "")
                return;
            pm.rename(renameDialog.profileId, renameName.text, renameIcon.chosen);
            renameDialog.close();
        }

        title: "Rename profile"
        standardButtons: Kirigami.Dialog.Cancel
        preferredWidth: Kirigami.Units.gridUnit * 24
        customFooterActions: [
            Kirigami.Action {
                text: "Save"
                icon.name: "document-save"
                enabled: page.idle && renameName.text.trim() !== ""
                onTriggered: renameDialog.save()
            }
        ]

        ColumnLayout {
            spacing: Kirigami.Units.smallSpacing

            UqLabel {
                text: "Name"
            }
            QQC2.TextField {
                id: renameName
                maximumLength: 100
                Accessible.name: "Name of the profile"
                Layout.fillWidth: true
                onAccepted: renameDialog.save()
            }
            UqLabel {
                text: "Icon"
                Layout.topMargin: Kirigami.Units.smallSpacing
            }
            IconChoice {
                id: renameIcon
                icons: page.icons
                names: page.iconNames
                Layout.fillWidth: true
            }
        }
    }

    // The settings a profile can hold that it doesn't yet, each with its current value.
    Kirigami.Dialog {
        id: addDialog

        property string profileId
        /// `keyId`s of the profile's own settings, which aren't offered.
        property var held: []
        /// The ticked settings, by `keyId`.
        property var picked: []
        readonly property var offered: page.available.filter(k => addDialog.held.indexOf(page.keyId(k)) < 0)

        function start(p) {
            addDialog.profileId = p.id;
            addDialog.held = (p.keys || []).map(k => page.keyId(k));
            addDialog.picked = [];
            addDialog.open();
        }
        function add() {
            const keys = addDialog.offered.filter(k => addDialog.picked.indexOf(page.keyId(k)) >= 0).map(k => [k.domain, k.path]);
            if (!page.idle || keys.length === 0)
                return;
            pm.addKeys(addDialog.profileId, JSON.stringify(keys));
            addDialog.close();
        }

        title: "Add settings"
        standardButtons: Kirigami.Dialog.Cancel
        preferredWidth: Kirigami.Units.gridUnit * 28
        customFooterActions: [
            Kirigami.Action {
                text: "Add"
                icon.name: "list-add"
                enabled: page.idle && addDialog.picked.length > 0
                onTriggered: addDialog.add()
            }
        ]

        ColumnLayout {
            spacing: Kirigami.Units.smallSpacing

            UqLabel {
                text: "Each setting is added with the value it has now."
                wrapMode: Text.Wrap
                opacity: 0.7
                Layout.fillWidth: true
            }
            UqLabel {
                text: "This profile already holds every setting it can."
                visible: addDialog.offered.length === 0
                wrapMode: Text.Wrap
                Layout.fillWidth: true
            }
            Repeater {
                model: addDialog.offered
                delegate: RowLayout {
                    id: keyRow

                    required property var modelData
                    readonly property string kid: page.keyId(modelData)
                    readonly property bool picked: addDialog.picked.indexOf(kid) >= 0

                    Layout.fillWidth: true

                    QQC2.CheckBox {
                        id: keyBox
                        checked: keyRow.picked
                        Accessible.name: keyRow.modelData.label
                        Accessible.description: keyRow.modelData.text
                        onToggled: {
                            const others = addDialog.picked.filter(id => id !== keyRow.kid);
                            addDialog.picked = keyBox.checked ? others.concat([keyRow.kid]) : others;
                            keyBox.checked = Qt.binding(() => keyRow.picked);
                        }
                    }
                    UqLabel {
                        text: keyRow.modelData.label
                        elide: Text.ElideRight
                        Layout.fillWidth: true
                        MouseArea {
                            anchors.fill: parent
                            onClicked: {
                                keyBox.toggle();
                                keyBox.toggled();
                            }
                        }
                    }
                    UqLabel {
                        text: keyRow.modelData.text
                        opacity: 0.7
                        elide: Text.ElideRight
                        Layout.maximumWidth: Kirigami.Units.gridUnit * 10
                    }
                }
            }
        }
    }

    Kirigami.PromptDialog {
        id: confirmDelete

        property string profileId

        title: "Delete this profile?"
        subtitle: "Your settings stay as they are now. This can't be undone."
        standardButtons: Kirigami.Dialog.Cancel
        customFooterActions: [
            Kirigami.Action {
                text: "Delete"
                icon.name: "edit-delete"
                onTriggered: {
                    pm.deleteProfile(confirmDelete.profileId);
                    confirmDelete.close();
                }
            }
        ]
    }

    // An imported file has the id of a profile that exists. The message (uqd's, without its
    // question) names it; the title and the button ask.
    Kirigami.PromptDialog {
        id: conflictDialog

        property string path

        title: "Replace this profile?"
        standardButtons: Kirigami.Dialog.Cancel
        customFooterActions: [
            Kirigami.Action {
                text: "Replace"
                icon.name: "document-replace"
                onTriggered: {
                    pm.importFrom(conflictDialog.path, true);
                    conflictDialog.close();
                }
            }
        ]

        UqLabel {
            id: conflictMessage
            objectName: "conflictMessage"
            wrapMode: Text.Wrap
        }
    }

    FileDialog {
        id: exportDialog

        property string profileId

        title: "Export a profile"
        fileMode: FileDialog.SaveFile
        defaultSuffix: "toml"
        nameFilters: ["Profiles (*.toml)"]
        onAccepted: pm.exportTo(exportDialog.profileId, String(selectedFile))
    }
    FileDialog {
        id: importDialog
        title: "Import a profile"
        fileMode: FileDialog.OpenFile
        nameFilters: ["Profiles (*.toml)"]
        onAccepted: pm.importFrom(String(selectedFile), false)
    }

    ColumnLayout {
        id: column
        width: Math.min(page.availableWidth - Kirigami.Units.largeSpacing * 2, Kirigami.Units.gridUnit * 40)
        x: Math.max(Kirigami.Units.largeSpacing, (page.availableWidth - width) / 2)
        spacing: Kirigami.Units.smallSpacing

        Item {
            implicitHeight: Kirigami.Units.smallSpacing
        }
        UqLabel {
            text: "Profiles"
            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.5
            font.bold: true
            Accessible.role: Accessible.Heading
        }
        UqLabel {
            text: "A profile is a set of settings you turn on together, such as for gaming or a presentation. Back to normal puts back the settings it changed, except the ones you changed yourself since."
            wrapMode: Text.Wrap
            opacity: 0.7
            Layout.fillWidth: true
        }
        ErrorCard {
            objectName: "uqdDown"
            visible: !page.ready
            title: "uQ's settings service isn't running"
            detail: "Start the uQ desktop service to see and turn on profiles."
            Layout.fillWidth: true
        }
        ErrorCard {
            objectName: "profilesError"
            visible: pm.error !== ""
            title: "That wasn't done"
            detail: pm.error
            Layout.fillWidth: true
        }
        // What the last Turn on or Back to normal kept and couldn't do.
        Kirigami.InlineMessage {
            id: reportMessage
            objectName: "profileReport"
            visible: false
            type: Kirigami.MessageType.Information
            showCloseButton: true
            Layout.fillWidth: true
        }
        Kirigami.InlineMessage {
            id: toast
            objectName: "profilesToast"
            visible: false
            type: Kirigami.MessageType.Information
            showCloseButton: true
            Layout.fillWidth: true
        }
        Timer {
            id: toastTimer
            interval: 6000
            onTriggered: toast.visible = false
        }

        SettingRow {
            settingId: "profiles"
            label: "Make a profile from your settings"
            highlight: App.highlight
            enabled: page.ready
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.largeSpacing

            QQC2.Button {
                objectName: "newProfile"
                text: "New profile"
                icon.name: "list-add"
                enabled: page.idle
                onClicked: newDialog.start()
            }
        }
        SettingRow {
            settingId: "profile-files"
            label: "Import a profile from a file"
            highlight: App.highlight
            enabled: page.ready
            Layout.fillWidth: true

            QQC2.Button {
                objectName: "importProfile"
                text: "Import…"
                icon.name: "document-import"
                enabled: page.idle
                onClicked: importDialog.open()
            }
        }

        UqLabel {
            text: "No profiles yet."
            visible: page.ready && page.profiles.length === 0
            opacity: 0.7
            Layout.leftMargin: Kirigami.Units.largeSpacing * 2
            Layout.topMargin: Kirigami.Units.largeSpacing
        }
        Repeater {
            model: page.profiles
            delegate: Kirigami.AbstractCard {
                id: card

                required property var modelData
                readonly property bool isOn: modelData.active === true
                readonly property var keys: modelData.keys || []

                Layout.fillWidth: true
                Layout.topMargin: Kirigami.Units.largeSpacing
                Accessible.role: Accessible.Grouping
                Accessible.name: modelData.name

                contentItem: ColumnLayout {
                    spacing: Kirigami.Units.smallSpacing

                    RowLayout {
                        spacing: Kirigami.Units.largeSpacing
                        Layout.fillWidth: true

                        Kirigami.Icon {
                            source: card.modelData.icon || "bookmarks"
                            implicitWidth: Kirigami.Units.iconSizes.medium
                            implicitHeight: Kirigami.Units.iconSizes.medium
                            Accessible.ignored: true
                        }
                        ColumnLayout {
                            spacing: 0
                            Layout.fillWidth: true

                            RowLayout {
                                spacing: Kirigami.Units.smallSpacing
                                UqLabel {
                                    text: card.modelData.name
                                    font.bold: true
                                    elide: Text.ElideRight
                                    Layout.fillWidth: true
                                }
                                Rectangle {
                                    visible: card.isOn
                                    radius: height / 2
                                    color: Kirigami.Theme.highlightColor
                                    implicitWidth: onTag.implicitWidth + Kirigami.Units.largeSpacing
                                    implicitHeight: onTag.implicitHeight + Kirigami.Units.smallSpacing
                                    UqLabel {
                                        id: onTag
                                        anchors.centerIn: parent
                                        text: "On"
                                        color: Kirigami.Theme.highlightedTextColor
                                    }
                                }
                            }
                            UqLabel {
                                text: page.settingsCount(card.keys.length)
                                opacity: 0.7
                            }
                        }
                        QQC2.Button {
                            text: "Turn on"
                            icon.name: "media-playback-start"
                            visible: !card.isOn
                            enabled: page.idle
                            Accessible.name: "Turn on " + card.modelData.name
                            onClicked: pm.apply(card.modelData.id)
                        }
                        QQC2.Button {
                            text: "Back to normal"
                            icon.name: "edit-undo"
                            visible: card.isOn
                            enabled: page.idle
                            Accessible.name: "Turn off " + card.modelData.name + " and go back to normal"
                            onClicked: pm.off()
                        }
                    }
                    QQC2.ToolButton {
                        id: expand
                        text: "Settings in this profile"
                        icon.name: checked ? "arrow-down" : "arrow-right"
                        checkable: true
                        display: QQC2.AbstractButton.TextBesideIcon
                    }
                    ProfileKeyList {
                        visible: expand.checked
                        keys: card.keys
                        editable: page.ready
                        Layout.fillWidth: true
                        Layout.leftMargin: Kirigami.Units.largeSpacing * 2
                        onRemoveKey: (domain, pathJson) => pm.removeKey(card.modelData.id, domain, pathJson)
                    }
                    Flow {
                        spacing: Kirigami.Units.smallSpacing
                        Layout.fillWidth: true

                        QQC2.Button {
                            text: "Add settings…"
                            icon.name: "list-add"
                            enabled: page.idle
                            Accessible.name: "Add settings to " + card.modelData.name
                            onClicked: addDialog.start(card.modelData)
                        }
                        QQC2.Button {
                            text: "Rename…"
                            icon.name: "edit-rename"
                            enabled: page.idle
                            Accessible.name: "Rename " + card.modelData.name
                            onClicked: renameDialog.start(card.modelData)
                        }
                        QQC2.Button {
                            text: "Export…"
                            icon.name: "document-export"
                            enabled: page.idle
                            Accessible.name: "Export " + card.modelData.name
                            onClicked: {
                                exportDialog.profileId = card.modelData.id;
                                exportDialog.open();
                            }
                        }
                        QQC2.Button {
                            text: "Delete"
                            icon.name: "edit-delete"
                            enabled: page.idle && !card.isOn
                            Accessible.name: "Delete " + card.modelData.name
                            Accessible.description: card.isOn ? "Turn it off first" : ""
                            onClicked: {
                                confirmDelete.profileId = card.modelData.id;
                                confirmDelete.open();
                            }
                        }
                        UqLabel {
                            text: "Turn it off first"
                            visible: card.isOn
                            opacity: 0.7
                            height: Kirigami.Units.gridUnit * 2
                            verticalAlignment: Text.AlignVCenter
                        }
                    }
                }
            }
        }
        Item {
            implicitHeight: Kirigami.Units.largeSpacing
        }
    }
}
