// Keyboard shortcuts (spec §5), in three tabs: uQ's own shortcuts, a card per group, each with
// its keys drawn as keycaps, a switch and Reset; shortcuts of your own (open a panel, start an
// app, run a command); and the shortcuts marked in user.lua, read-only. Change opens the editor, which can listen for the
// keys; while it listens uQ's shortcuts are paused (never for long: uQ puts them back after 15
// seconds, and this page does as soon as it stops listening, closes or loses focus). Everything
// goes through KeybindsModel. Shortcut names, keys, app names, user.lua text and Hyprland's
// messages are shown through UqLabel (plain text) only. The raw ids live in AdvancedDetails.
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
            return JSON.parse(km.stateJson);
        } catch (e) {
            return {};
        }
    }
    readonly property var groups: snap.groups || []
    readonly property var custom: snap.custom || []
    readonly property var userLua: snap.user_lua || []
    readonly property var limits: snap.limits || ({})
    readonly property bool ready: App.uqdUp && snap.groups !== undefined
    readonly property bool idle: page.ready && !km.busy
    /// Hyprland's message for the last change it refused, or "".
    readonly property string sectionMessage: page.snap.section && page.snap.section.source === "error" ? String(page.snap.section.message || "") : ""
    /// Whether any of uQ's shortcuts was changed (**Put all back** has something to do).
    readonly property bool anyChanged: page.groups.some(g => g.binds.some(b => b.changed === true))
    /// Whether pausing the shortcuts works; once it hasn't, the field is hidden for this visit.
    property bool captureOk: true
    property int captureSeen: 0
    property int noticeSeen: 0
    /// Every shortcut and the id it goes by, for AdvancedDetails.
    readonly property var idRows: {
        const rows = [];
        for (const g of page.groups) {
            for (const b of g.binds)
                rows.push({ label: String(b.label), lua: String(b.id) + " (" + (b.combo ? String(b.combo) : "no keys") + ")" });
        }
        for (const b of page.custom)
            rows.push({ label: String(b.name), lua: String(b.id) + " (" + (b.combo ? String(b.combo) : "no keys") + ")" });
        return rows;
    }

    /// Read everything again; Main calls it when the window comes back to the front.
    function refresh() {
        km.refresh();
    }
    /// What the last request brought: the end of a save, a pause that didn't work, a notice.
    function showNews() {
        const s = page.snap;
        // The editor's save has ended (busy goes false before the error and the state arrive).
        if (editor.saving && !km.busy)
            editor.finishSave(km.error);
        const capture = s.capture;
        if (capture && capture.seq > page.captureSeen) {
            page.captureSeen = capture.seq;
            if (capture.state === "unavailable") {
                page.captureOk = false;
                editor.stopCapture();
            }
        }
        const notice = s.notice;
        if (notice && notice.seq > page.noticeSeen) {
            page.noticeSeen = notice.seq;
            noticeMessage.show(notice.text !== "" ? [notice.text] : []);
            noticeTimer.restart();
        }
    }
    /// A shortcut's switch was flipped.
    function turn(id, on) {
        km.setEnabled(id, on);
    }

    Component.onCompleted: km.refresh()
    Component.onDestruction: editor.stopCapture()

    KeybindsModel {
        id: km
        onStateJsonChanged: page.showNews()
    }
    Connections {
        target: App
        function onUqdUpChanged() {
            if (App.uqdUp)
                km.refresh();
        }
    }
    // Pressing keys for a new shortcut ends when this window isn't the one being typed in.
    Connections {
        target: page.Window.window
        ignoreUnknownSignals: true
        function onActiveChanged() {
            if (page.Window.window && !page.Window.window.active)
                editor.stopCapture();
        }
    }
    Timer {
        id: noticeTimer
        interval: 6000
        onTriggered: noticeMessage.dismissed = true
    }

    ShortcutEditor {
        id: editor
        catalogue: page.snap.catalogue || ({})
        apps: page.snap.apps || []
        limits: page.limits
        check: draft => km.check(draft)
        comboFrom: (key, modifiers) => km.comboFromKey(key, modifiers)
        captureAvailable: page.captureOk
        canSave: page.idle
        busy: km.busy
        errorNow: () => km.error
        onBindAccepted: json => {
            km.saveCustom(json);
            // No request went out (no session bus): say so here.
            if (!km.busy)
                editor.finishSave(km.error !== "" ? km.error : "uQ isn't running");
        }
        onComboAccepted: (id, combo) => {
            km.setCombo(id, combo);
            if (!km.busy)
                editor.finishSave(km.error !== "" ? km.error : "uQ isn't running");
        }
        onCaptureStarted: km.pauseCapture()
        onCaptureEnded: km.resumeCapture()
    }

    Kirigami.PromptDialog {
        id: confirmReset

        title: "Put all back?"
        subtitle: "Put all of uQ's shortcuts back to how they came? Your own shortcuts stay."
        standardButtons: Kirigami.Dialog.Cancel
        customFooterActions: [
            Kirigami.Action {
                text: "Put all back"
                icon.name: "edit-undo"
                enabled: page.idle
                onTriggered: {
                    km.resetAll();
                    confirmReset.close();
                }
            }
        ]
    }

    Kirigami.PromptDialog {
        id: confirmDelete

        property string bindId

        title: "Delete this shortcut?"
        subtitle: "You can undo this from the history."
        standardButtons: Kirigami.Dialog.Cancel
        customFooterActions: [
            Kirigami.Action {
                text: "Delete"
                icon.name: "edit-delete"
                enabled: page.idle
                onTriggered: {
                    km.deleteCustom(confirmDelete.bindId);
                    confirmDelete.close();
                }
            }
        ]
    }

    /// An icon for each group of uQ's shortcuts.
    function groupIcon(id) {
        switch (id) {
        case "apps_and_desktop":
            return "user-desktop";
        case "windows":
            return "window";
        case "workspaces":
            return "virtual-desktops";
        case "sound_and_brightness":
            return "audio-volume-high";
        case "media":
            return "media-playback-start";
        case "profiles":
            return "bookmarks";
        }
        return "input-keyboard";
    }

    banners: [
        ErrorCard {
            objectName: "uqdDown"
            visible: !page.ready
            title: "uQ's settings service isn't running"
            detail: "Start the uQ desktop service to see and change keyboard shortcuts."
            Layout.fillWidth: true
        },
        // The last change was refused by Hyprland's check: its message, as plain text.
        RuleMessage {
            objectName: "sectionError"
            negative: true
            closable: false
            lines: page.sectionMessage !== "" ? ["The last change to the shortcuts wasn't applied. Hyprland said: " + page.sectionMessage] : []
            Layout.fillWidth: true
        },
        RuleMessage {
            objectName: "keybindsError"
            negative: true
            closable: false
            // A write Hyprland refused is already said by sectionError: one card, not two.
            lines: km.error !== "" && !(page.sectionMessage !== "" && km.error.indexOf(page.sectionMessage) >= 0) ? ["That wasn't done: " + km.error] : []
            Layout.fillWidth: true
        },
        RuleMessage {
            id: noticeMessage
            objectName: "noticeMessage"
            Layout.fillWidth: true
        }
    ]

    SettingsTab {
        title: "uQ's shortcuts"
        iconName: "preferences-desktop-keyboard-shortcut"

        // A card per group of uQ's shortcuts.
        Repeater {
            model: page.groups
            delegate: SettingsCard {
                id: group

                required property var modelData

                objectName: "groupCard"
                title: String(group.modelData.label)
                iconName: page.groupIcon(group.modelData.id)

                Repeater {
                    model: group.modelData.binds
                    delegate: ShortcutRow {
                        required property var modelData

                        bind: modelData
                        idle: page.idle
                        settingId: modelData.id === "uq.profile.next" ? "next-profile-shortcut" : ""
                        highlight: App.highlight
                        Layout.fillWidth: true
                        onChangeRequested: editor.change(modelData)
                        onSwitched: on => page.turn(modelData.id, on)
                        onResetRequested: km.reset(modelData.id)
                    }
                }
            }
        }

        SettingsCard {
            title: "Changing keys"
            iconName: "input-keyboard"

            SettingRow {
                settingId: "shortcut-list"
                iconName: "document-edit"
                label: "Click a shortcut's keys to change them"
                help: "Two shortcuts can't use the same keys."
                highlight: App.highlight
                enabled: page.ready
                Layout.fillWidth: true
            }
            SettingRow {
                settingId: "shortcut-capture"
                iconName: "media-playback-pause"
                label: "Shortcuts pause while you press new keys"
                help: "For 15 seconds at most, and only while the editor listens."
                highlight: App.highlight
                enabled: page.ready
                Layout.fillWidth: true
            }
            SettingRow {
                settingId: "reset-shortcuts"
                iconName: "edit-undo"
                label: "All of uQ's shortcuts as they came"
                help: "Your own shortcuts stay."
                highlight: App.highlight
                enabled: page.ready
                Layout.fillWidth: true

                QQC2.Button {
                    objectName: "putAllBack"
                    text: "Put all back"
                    icon.name: "edit-undo"
                    enabled: page.idle && page.anyChanged
                    onClicked: confirmReset.open()
                }
            }
        }

        SettingsCard {
            AdvancedDetails {
                kind: "shortcuts"
                rows: page.idRows
                Layout.fillWidth: true
            }
        }
    }

    SettingsTab {
        title: "My shortcuts"
        iconName: "user-identity"

        SettingsCard {
            title: "Your own shortcuts"
            iconName: "user-identity"
            help: "Open a uQ panel, start an app or run a command with keys of your own."
            wide: true

            actions: [
                QQC2.Button {
                    objectName: "addShortcut"
                    text: "Add shortcut"
                    icon.name: "list-add"
                    enabled: page.idle && page.custom.length < (page.limits.custom || 100)
                    onClicked: editor.startNew()
                }
            ]

            SettingRow {
                settingId: "add-shortcut"
                visible: page.custom.length === 0
                iconName: "list-add"
                label: "No shortcuts of your own yet"
                highlight: App.highlight
                enabled: page.ready
                Layout.fillWidth: true
            }
            Repeater {
                model: page.custom
                delegate: ShortcutRow {
                    required property var modelData

                    bind: modelData
                    idle: page.idle
                    Layout.fillWidth: true
                    onChangeRequested: editor.change(modelData)
                    onSwitched: on => page.turn(modelData.id, on)
                    onDeleteRequested: {
                        confirmDelete.bindId = modelData.id;
                        confirmDelete.open();
                    }
                }
            }
        }
    }

    SettingsTab {
        title: "From user.lua"
        iconName: "text-x-script"
        available: page.userLua.length > 0

        SettingsCard {
            objectName: "userLuaList"
            title: "Marked in user.lua"
            iconName: "text-x-script"
            help: "Your own lines in user.lua, marked so they show here. Change them in that file."
            wide: true

            Repeater {
                model: page.userLua
                delegate: RowLayout {
                    id: luaRow

                    required property var modelData

                    spacing: Kirigami.Units.largeSpacing
                    Layout.fillWidth: true
                    Layout.leftMargin: Kirigami.Units.largeSpacing
                    Layout.rightMargin: Kirigami.Units.largeSpacing
                    Accessible.role: Accessible.Grouping
                    Accessible.name: luaRow.modelData.combo_text ? String(luaRow.modelData.combo_text) : "Other keys"

                    KeyCaps {
                        visible: !!luaRow.modelData.combo_text
                        combo: luaRow.modelData.combo_text ? String(luaRow.modelData.combo_text) : ""
                        Layout.preferredWidth: Kirigami.Units.gridUnit * 12
                    }
                    UqLabel {
                        visible: !luaRow.modelData.combo_text
                        text: "Other keys"
                        opacity: 0.7
                        Layout.preferredWidth: Kirigami.Units.gridUnit * 12
                    }
                    UqLabel {
                        text: String(luaRow.modelData.text)
                        wrapMode: Text.Wrap
                        font: Kirigami.Theme.fixedWidthFont
                        opacity: 0.8
                        Layout.fillWidth: true
                    }
                    UqLabel {
                        text: "Line " + luaRow.modelData.line
                        opacity: 0.7
                    }
                }
            }
        }
    }
}
