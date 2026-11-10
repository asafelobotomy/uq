// Window rules (spec §5): rules that change how windows open and look, panel rules for bars,
// notifications and launchers, and workspace rules. Each rule can be turned on and off, edited,
// moved and deleted; a new one starts blank or from a template. "Pick a window" hides this
// window while the person clicks the window to match, and "Why does this window…?" lists the
// rules a clicked window gets. Everything goes through RulesModel. Window titles, app IDs, rule
// names, patterns and Hyprland's messages are shown through UqLabel (plain text) only.
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
            return JSON.parse(rm.stateJson);
        } catch (e) {
            return {};
        }
    }
    readonly property var catalogue: snap.fields || ({})
    readonly property var limits: snap.limits || ({})
    readonly property var templates: snap.templates || []
    readonly property bool ready: App.uqdUp && snap.error === undefined
    readonly property bool idle: page.ready && !rm.busy
    readonly property var kinds: ["window", "layer", "workspace"]
    readonly property string currentKind: kinds[tabs.currentIndex]
    /// The last pick, Why, reopen note and notice shown, so each is shown once.
    property int pickSeen: 0
    property int whySeen: 0
    property int reopenSeen: 0
    property int noticeSeen: 0
    /// A pick this page started is in flight: this window is hidden until it ends (F5).
    property bool picking: false
    /// What the pick is for: "new" (a new rule), "editor" (the open editor), "template" or "why".
    property string pickFor
    /// How the window was shown before a pick hid it (maximized or not), to show it so again.
    property int shownAs: Window.Windowed

    /// Read everything again; Main calls it when the window comes back to the front.
    function refresh() {
        rm.refresh();
    }
    function appWindow() {
        return typeof applicationWindow === "function" ? applicationWindow() : page.Window.window;
    }
    /// Hide this window and let the person click a window; it comes back when the pick ends.
    function startPick(target) {
        if (!page.idle)
            return;
        page.pickFor = target;
        page.picking = true;
        const w = page.appWindow();
        if (w) {
            page.shownAs = w.visibility === Window.Maximized ? Window.Maximized : Window.Windowed;
            w.hide();
        }
        rm.pick(target === "why" ? "why" : "match");
        // No request went out (no session bus): never stay hidden.
        if (!rm.busy)
            page.endPick();
        else
            pickFailsafe.restart();
    }
    function endPick() {
        page.picking = false;
        pickFailsafe.stop();
        const w = page.appWindow();
        if (w) {
            w.visibility = page.shownAs;
            w.raise();
            w.requestActivate();
        }
    }
    /// The lines of the "Why does this window…?" answer.
    function whyLines(why) {
        const w = why.window || {};
        const title = String(w.title || "");
        const app = String(w["class"] || "");
        const out = ["This window: " + (title !== "" ? title + " (" + app + ")" : app)];
        for (const r of why.rules || []) {
            let line = String(r.name) + (r.enabled ? "" : " (turned off)") + ": " + String(r.effects_text);
            if (r.verdict === "if_dialog")
                line += ", if it's a dialog";
            else if (r.verdict === "unknown")
                line += ", if it matches (can't tell from here)";
            if (r.source === "user_lua")
                line += ". Overridden in user.lua" + (r.line ? ", line " + r.line : "");
            out.push(line);
        }
        if ((why.rules || []).length === 0)
            out.push("No uQ window rule matches it.");
        out.push("Rules in user.lua aren't listed here.");
        return out;
    }
    /// Show what the last request brought: a pick, a Why answer, a reopen note or a notice.
    function showNews() {
        const s = page.snap;
        const pick = s.pick;
        if (pick && pick.seq > page.pickSeen) {
            page.pickSeen = pick.seq;
            if (page.picking)
                page.endPick();
            if (pick.status === "picked") {
                pickMessage.dismissed = true;
                if (page.pickFor === "editor")
                    editor.fillFromPick(String(pick.class_pattern || ""));
                else if (page.pickFor === "new") {
                    tabs.currentIndex = 0;
                    editor.startNew("window");
                    editor.fillFromPick(String(pick.class_pattern || ""));
                } else if (page.pickFor === "template")
                    templateDialog.app = String((pick.window || {})["class"] || "");
            } else {
                pickMessage.show([pickMessage.pickText(pick.status)]);
            }
        }
        const why = s.why;
        if (why && why.seq > page.whySeen) {
            page.whySeen = why.seq;
            whyMessage.show(page.whyLines(why));
        }
        const reopen = s.reopen;
        if (reopen && reopen.seq > page.reopenSeen) {
            page.reopenSeen = reopen.seq;
            reopenMessage.show([reopenMessage.reopenText(reopen.names, reopen.more)]);
        }
        const notice = s.notice;
        if (notice && notice.seq > page.noticeSeen) {
            page.noticeSeen = notice.seq;
            noticeMessage.show(notice.text !== "" ? [notice.text] : []);
            noticeTimer.restart();
        }
        // The window list may be newer: "Matches now" runs again on it.
        if (editor.opened)
            editor.refreshPreview();
    }
    function startTemplate(t) {
        tabs.currentIndex = 0;
        if (t.needs === "nothing") {
            rm.fromTemplate(t.id, "", "", false);
            return;
        }
        templateDialog.start(t);
    }
    function showHighlight() {
        const tab = { "window-rules": 0, "panel-rules": 1, "workspace-rules": 2 }[App.highlight];
        if (tab !== undefined)
            tabs.currentIndex = tab;
    }

    Component.onCompleted: {
        rm.refresh();
        page.showHighlight();
    }
    Accessible.role: Accessible.Pane
    Accessible.name: "Window rules"
    contentWidth: availableWidth

    RulesModel {
        id: rm
        onStateJsonChanged: page.showNews()
        // A pick that ends without a new pick status still brings the window back (F5).
        onBusyChanged: {
            if (!rm.busy && page.picking)
                page.endPick();
        }
    }
    Connections {
        target: App
        function onUqdUpChanged() {
            if (App.uqdUp)
                rm.refresh();
        }
        function onHighlightChanged() {
            page.showHighlight();
        }
    }
    // uqd gives up on a pick after 20 s; never stay hidden longer than that.
    Timer {
        id: pickFailsafe
        interval: 30000
        onTriggered: if (page.picking) page.endPick()
    }
    Timer {
        id: noticeTimer
        interval: 6000
        onTriggered: noticeMessage.dismissed = true
    }

    RuleEditor {
        id: editor
        fields: page.catalogue
        exact: text => rm.exact(text)
        preview: draft => rm.preview(draft)
        nameLimit: page.limits.name || 100
        patternLimit: page.limits.pattern || 256
        canSave: page.idle
        onRuleAccepted: (kind, ruleJson) => rm.save(kind, ruleJson)
        onPickRequested: page.startPick("editor")
        // "Matches now" should see the windows open now.
        onOpened: {
            if (page.idle)
                rm.refreshWindows();
        }
    }

    // Asks for what a template needs: an app (typed, or picked) and maybe a workspace.
    Kirigami.Dialog {
        id: templateDialog

        property var template: ({})
        property alias app: appBox.text
        readonly property bool needsWorkspace: template.needs === "app_and_workspace"
        readonly property bool complete: appBox.text.trim() !== "" && (!needsWorkspace || workspaceBox.text.trim() !== "")

        function start(t) {
            templateDialog.template = t;
            appBox.clear();
            workspaceBox.clear();
            quietBox.checked = false;
            templateDialog.open();
        }
        function add() {
            if (!page.idle || !templateDialog.complete)
                return;
            rm.fromTemplate(templateDialog.template.id, appBox.text.trim(), templateDialog.needsWorkspace ? workspaceBox.text.trim() : "", templateDialog.needsWorkspace && quietBox.checked);
            templateDialog.close();
        }

        title: "New rule from a template"
        standardButtons: Kirigami.Dialog.Cancel
        preferredWidth: Kirigami.Units.gridUnit * 26
        onOpened: appBox.forceActiveFocus()
        customFooterActions: [
            Kirigami.Action {
                text: "Add"
                icon.name: "list-add"
                enabled: page.idle && templateDialog.complete
                onTriggered: templateDialog.add()
            }
        ]

        ColumnLayout {
            spacing: Kirigami.Units.smallSpacing

            UqLabel {
                text: String(templateDialog.template.name || "")
                font.bold: true
                wrapMode: Text.Wrap
                Layout.fillWidth: true
            }
            UqLabel {
                text: "App ID"
            }
            RowLayout {
                QQC2.TextField {
                    id: appBox
                    maximumLength: 256
                    placeholderText: "Such as org.pulseaudio.pavucontrol"
                    Accessible.name: "App ID"
                    Layout.fillWidth: true
                    onAccepted: templateDialog.add()
                }
                QQC2.Button {
                    text: "Pick a window"
                    icon.name: "edit-select"
                    enabled: page.idle
                    onClicked: page.startPick("template")
                }
            }
            UqLabel {
                visible: templateDialog.needsWorkspace
                text: "Workspace"
            }
            RowLayout {
                visible: templateDialog.needsWorkspace
                QQC2.TextField {
                    id: workspaceBox
                    maximumLength: 64
                    placeholderText: "A number, or name:web"
                    Accessible.name: "Workspace"
                    Layout.fillWidth: true
                    onAccepted: templateDialog.add()
                }
                QQC2.CheckBox {
                    id: quietBox
                    text: "Quietly"
                    Accessible.description: "Open it there without switching to that workspace"
                }
            }
        }
    }

    Kirigami.PromptDialog {
        id: confirmDelete

        property string kind
        property string key

        title: "Delete this rule?"
        subtitle: "You can undo this from the history."
        standardButtons: Kirigami.Dialog.Cancel
        customFooterActions: [
            Kirigami.Action {
                text: "Delete"
                icon.name: "edit-delete"
                enabled: page.idle
                onTriggered: {
                    rm.remove(confirmDelete.kind, confirmDelete.key);
                    confirmDelete.close();
                }
            }
        ]
    }

    // One rule in a list: SettingRow gives the name, where it's overridden (with Open user.lua)
    // and the controls; the match and effects lines go under it. An inline component can't see
    // the page's ids, so it takes the page as `pg`.
    component RuleRow: ColumnLayout {
        id: ruleRow

        required property var modelData
        required property int index
        property var pg
        property string kind
        property int count: 0
        readonly property bool idle: !!pg && pg.idle
        readonly property string key: kind === "workspace" ? String(modelData.workspace) : String(modelData.id)
        readonly property string title: kind === "workspace" ? "Workspace " + String(modelData.workspace).replace(/^name:/, "") : String(modelData.name)
        readonly property var lines: {
            const m = ruleRow.modelData;
            if (ruleRow.kind === "window") {
                const n = m.matches_now || 0;
                return [String(m.match_text), String(m.effects_text), n === 0 ? "" : n === 1 ? "Matches 1 open window" : "Matches " + n + " open windows"];
            }
            if (ruleRow.kind === "layer")
                return ["Panel name: " + String(m.namespace), String(m.effects_text)];
            return [String(m.fields_text), m.layout ? "Layout: " + String(m.layout) : ""];
        }

        spacing: 0
        Layout.fillWidth: true
        Accessible.role: Accessible.Grouping
        Accessible.name: title

        SettingRow {
            label: ruleRow.title
            source: ({ source: ruleRow.modelData.source, detail: { line: ruleRow.modelData.line, path: ruleRow.modelData.path } })
            Layout.fillWidth: true

            QQC2.Switch {
                id: onSwitch
                visible: ruleRow.kind !== "workspace"
                checked: ruleRow.modelData.enabled === true
                enabled: ruleRow.idle
                Accessible.name: "Rule on"
                Accessible.description: ruleRow.title
                onToggled: {
                    ruleRow.pg.turnRule(ruleRow.kind, ruleRow.key, onSwitch.checked);
                    onSwitch.checked = Qt.binding(() => ruleRow.modelData.enabled === true);
                }
            }
            QQC2.ToolButton {
                text: "Edit"
                icon.name: "document-edit"
                display: QQC2.AbstractButton.IconOnly
                enabled: ruleRow.idle
                Accessible.description: ruleRow.title
                onClicked: ruleRow.pg.editRule(ruleRow.kind, ruleRow.modelData.raw)
            }
            QQC2.ToolButton {
                text: "Move up"
                icon.name: "go-up"
                display: QQC2.AbstractButton.IconOnly
                enabled: ruleRow.idle && ruleRow.index > 0
                Accessible.description: ruleRow.title
                onClicked: ruleRow.pg.moveRule(ruleRow.kind, ruleRow.key, ruleRow.index - 1)
            }
            QQC2.ToolButton {
                text: "Move down"
                icon.name: "go-down"
                display: QQC2.AbstractButton.IconOnly
                enabled: ruleRow.idle && ruleRow.index < ruleRow.count - 1
                Accessible.description: ruleRow.title
                onClicked: ruleRow.pg.moveRule(ruleRow.kind, ruleRow.key, ruleRow.index + 1)
            }
            QQC2.ToolButton {
                text: "Delete"
                icon.name: "edit-delete"
                display: QQC2.AbstractButton.IconOnly
                enabled: ruleRow.idle
                Accessible.description: ruleRow.title
                onClicked: ruleRow.pg.askDelete(ruleRow.kind, ruleRow.key)
            }
        }
        Repeater {
            model: ruleRow.lines.filter(l => l !== "")
            delegate: UqLabel {
                required property string modelData
                text: modelData
                wrapMode: Text.Wrap
                opacity: 0.7
                Layout.fillWidth: true
                Layout.leftMargin: Kirigami.Units.largeSpacing * 2
            }
        }
    }
    // What a RuleRow asks the page to do.
    function turnRule(kind, key, on) {
        rm.setEnabled(kind, key, on);
    }
    function editRule(kind, raw) {
        editor.edit(kind, raw);
    }
    function moveRule(kind, key, to) {
        rm.moveTo(kind, key, to);
    }
    function askDelete(kind, key) {
        confirmDelete.kind = kind;
        confirmDelete.key = key;
        confirmDelete.open();
    }

    ColumnLayout {
        id: column
        width: Math.min(page.availableWidth - Kirigami.Units.largeSpacing * 2, Kirigami.Units.gridUnit * 44)
        x: Math.max(Kirigami.Units.largeSpacing, (page.availableWidth - width) / 2)
        spacing: Kirigami.Units.smallSpacing

        Item {
            implicitHeight: Kirigami.Units.smallSpacing
        }
        UqLabel {
            text: "Window rules"
            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.5
            font.bold: true
            Accessible.role: Accessible.Heading
        }
        UqLabel {
            text: "Rules change how windows open and look: float an app's dialogs, open an app on a workspace, keep the screen awake. Later rules win over earlier ones."
            wrapMode: Text.Wrap
            opacity: 0.7
            Layout.fillWidth: true
        }
        ErrorCard {
            objectName: "uqdDown"
            visible: !page.ready
            title: "uQ's settings service isn't running"
            detail: "Start the uQ desktop service to see and change window rules."
            Layout.fillWidth: true
        }
        // The last rules change was refused by Hyprland's check (F9): its message, as plain text.
        RuleMessage {
            objectName: "sectionError"
            negative: true
            closable: false
            lines: page.snap.section && page.snap.section.source === "error" ? ["The last change to the rules wasn't applied. Hyprland said: " + String(page.snap.section.message || "")] : []
            Layout.fillWidth: true
        }
        RuleMessage {
            objectName: "rulesError"
            negative: true
            closable: false
            lines: rm.error !== "" ? ["That wasn't done: " + rm.error] : []
            Layout.fillWidth: true
        }
        RuleMessage {
            id: pickMessage
            objectName: "pickMessage"
            Layout.fillWidth: true
        }
        RuleMessage {
            id: whyMessage
            objectName: "whyMessage"
            Layout.fillWidth: true
        }
        RuleMessage {
            id: reopenMessage
            objectName: "reopenMessage"
            Layout.fillWidth: true
        }
        RuleMessage {
            id: noticeMessage
            objectName: "noticeMessage"
            Layout.fillWidth: true
        }

        SettingRow {
            settingId: "window-rules"
            label: "Add a rule"
            highlight: App.highlight
            enabled: page.ready
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.largeSpacing

            QQC2.Button {
                id: newButton
                objectName: "newRule"
                text: "New rule"
                icon.name: "list-add"
                enabled: page.idle
                onClicked: newMenu.open()

                QQC2.Menu {
                    id: newMenu
                    y: newButton.height

                    QQC2.MenuItem {
                        text: "Blank rule"
                        onTriggered: editor.startNew(page.currentKind)
                    }
                    QQC2.MenuSeparator {}
                    Instantiator {
                        model: page.templates
                        delegate: QQC2.MenuItem {
                            id: templateItem
                            required property var modelData
                            // Template names are shown as plain text (F28).
                            contentItem: UqLabel {
                                text: String(templateItem.modelData.name)
                                leftPadding: Kirigami.Units.largeSpacing
                                rightPadding: Kirigami.Units.largeSpacing
                                verticalAlignment: Text.AlignVCenter
                            }
                            Accessible.name: String(modelData.name)
                            onTriggered: page.startTemplate(templateItem.modelData)
                        }
                        // After "Blank rule" and the separator.
                        onObjectAdded: (index, object) => newMenu.insertItem(index + 2, object)
                        onObjectRemoved: (index, object) => newMenu.removeItem(object)
                    }
                }
            }
        }
        SettingRow {
            settingId: "pick-window"
            label: "Make a rule for a window by clicking it"
            highlight: App.highlight
            enabled: page.ready
            Layout.fillWidth: true

            QQC2.Button {
                objectName: "pickWindow"
                text: "Pick a window"
                icon.name: "edit-select"
                enabled: page.idle
                onClicked: page.startPick("new")
            }
        }
        SettingRow {
            settingId: "why-window"
            label: "See which rules a window gets"
            highlight: App.highlight
            enabled: page.ready
            Layout.fillWidth: true

            QQC2.Button {
                objectName: "whyWindow"
                text: "Why does this window…?"
                icon.name: "help-contextual"
                enabled: page.idle
                onClicked: page.startPick("why")
            }
        }

        QQC2.TabBar {
            id: tabs
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.largeSpacing

            QQC2.TabButton {
                text: "Windows"
            }
            QQC2.TabButton {
                objectName: "panelsTab"
                text: "Panels"
            }
            QQC2.TabButton {
                objectName: "workspacesTab"
                text: "Workspaces"
            }
        }
        SettingRow {
            settingId: "panel-rules"
            visible: tabs.currentIndex === 1
            label: "Panel rules change bars, notifications and launchers, such as blur behind them."
            highlight: App.highlight
            Layout.fillWidth: true
        }
        SettingRow {
            settingId: "workspace-rules"
            visible: tabs.currentIndex === 2
            label: "Workspace rules put a workspace on a screen, keep it when empty or change its gaps. Its layout is set on the Look page."
            highlight: App.highlight
            Layout.fillWidth: true
        }
        UqLabel {
            readonly property var list: page.snap[page.currentKind] || []
            visible: page.ready && list.length === 0
            text: "No rules here yet."
            opacity: 0.7
            Layout.leftMargin: Kirigami.Units.largeSpacing * 2
            Layout.topMargin: Kirigami.Units.largeSpacing
        }
        Repeater {
            model: tabs.currentIndex === 0 ? (page.snap.window || []) : []
            delegate: RuleRow {
                pg: page
                kind: "window"
                count: (page.snap.window || []).length
            }
        }
        Repeater {
            model: tabs.currentIndex === 1 ? (page.snap.layer || []) : []
            delegate: RuleRow {
                pg: page
                kind: "layer"
                count: (page.snap.layer || []).length
            }
        }
        Repeater {
            model: tabs.currentIndex === 2 ? (page.snap.workspace || []) : []
            delegate: RuleRow {
                pg: page
                kind: "workspace"
                count: (page.snap.workspace || []).length
            }
        }
        Item {
            implicitHeight: Kirigami.Units.largeSpacing
        }
    }
}
