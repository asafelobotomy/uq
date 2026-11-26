// Window rules (spec §5), a tab each: rules that change how windows open and look, panel rules
// for bars, notifications and launchers, workspace rules, and the window rules installed apps
// offer (AppRuleList). Each rule is a card, with its app's icon when it matches one app, a
// switch, and Edit, Move and Delete; a new one starts blank or from a template. "Pick a window"
// hides this window while the person clicks the window to match, and "Why does this window…?"
// lists the rules a clicked window gets. Everything goes through RulesModel. Window titles, app IDs, rule
// names, patterns and Hyprland's messages are shown through UqLabel (plain text) only.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../components"
import "rules"

SettingsPage {
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
    /// Hyprland's message for the last rules change it refused, or "".
    readonly property string sectionMessage: page.snap.section && page.snap.section.source === "error" ? String(page.snap.section.message || "") : ""
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
        // The editor's save has ended (busy goes false before the error and the state arrive).
        // The editor also ends it on busy going false; finishSave runs only once.
        if (editor.saving && !rm.busy)
            editor.finishSave(rm.error);
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
                    page.show(windowsTab);
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
        page.show(windowsTab);
        if (t.needs === "nothing") {
            rm.fromTemplate(t.id, "", "", false);
            return;
        }
        templateDialog.start(t);
    }
    /// The app ID a window rule's pattern matches exactly ("^org\.kde\.dolphin$" is
    /// "org.kde.dolphin"), or "" when it matches more than one app or none.
    function appIdOf(rule) {
        const m = (rule && rule.raw && rule.raw.matches) || {};
        const pattern = String(m["class"] || m.initial_class || "");
        const id = pattern.replace(/^\^/, "").replace(/\$$/, "").replace(/\\(.)/g, "$1");
        return /^[A-Za-z0-9._-]+$/.test(id) ? id : "";
    }

    Component.onCompleted: rm.refresh()

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
        check: (kind, draft, replaces) => rm.check(kind, draft, replaces)
        nameLimit: page.limits.name || 100
        patternLimit: page.limits.pattern || 256
        canSave: page.idle
        busy: rm.busy
        errorNow: () => rm.error
        onRuleAccepted: (kind, ruleJson, replaces) => {
            rm.save(kind, ruleJson, replaces);
            // No request went out (no session bus, or the rule couldn't be read): say so here.
            if (!rm.busy)
                editor.finishSave(rm.error !== "" ? rm.error : "uQ isn't running");
        }
        onPickRequested: page.startPick("editor")
        // "Matches now" should see the windows open now.
        onOpened: {
            if (page.idle)
                rm.refreshWindows();
        }
    }

    // Asks for what a template needs: an app (typed, or picked) and maybe a workspace.
    TemplateDialog {
        id: templateDialog
        page: page
        rm: rm
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

    // What a RuleCard asks the page to do.
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

    banners: [
        ErrorCard {
            objectName: "uqdDown"
            visible: !page.ready
            title: "uQ's settings service isn't running"
            detail: "Start the uQ desktop service to see and change window rules."
            Layout.fillWidth: true
        },
        // The last rules change was refused by Hyprland's check (F9): its message, as plain text.
        RuleMessage {
            objectName: "sectionError"
            negative: true
            closable: false
            lines: page.sectionMessage !== "" ? ["The last change to the rules wasn't applied. Hyprland said: " + page.sectionMessage] : []
            Layout.fillWidth: true
        },
        RuleMessage {
            objectName: "rulesError"
            negative: true
            closable: false
            // A write Hyprland refused is already said by sectionError: one card, not two.
            lines: rm.error !== "" && !(page.sectionMessage !== "" && rm.error.indexOf(page.sectionMessage) >= 0) ? ["That wasn't done: " + rm.error] : []
            Layout.fillWidth: true
        },
        RuleMessage {
            id: pickMessage
            objectName: "pickMessage"
            Layout.fillWidth: true
        },
        RuleMessage {
            id: whyMessage
            objectName: "whyMessage"
            Layout.fillWidth: true
        },
        RuleMessage {
            id: reopenMessage
            objectName: "reopenMessage"
            Layout.fillWidth: true
        },
        RuleMessage {
            id: noticeMessage
            objectName: "noticeMessage"
            Layout.fillWidth: true
        }
    ]

    WindowRulesTab {
        id: windowsTab
        editor: editor
        page: page
    }

    SettingsTab {
        title: "Panels"
        iconName: "dashboard-show"

        SettingsCard {
            title: "New panel rule"
            iconName: "list-add"

            SettingRow {
                settingId: "panel-rules"
                iconName: "dashboard-show"
                label: "Bars, notifications and launchers"
                help: "Panel rules change them, such as blur behind them. Later rules win over earlier ones."
                highlight: App.highlight
                enabled: page.ready
                Layout.fillWidth: true

                QQC2.Button {
                    objectName: "newPanelRule"
                    text: "New rule"
                    icon.name: "list-add"
                    enabled: page.idle
                    onClicked: editor.startNew("layer")
                }
            }
            UqLabel {
                visible: page.ready && (page.snap.layer || []).length === 0
                text: "No panel rules yet."
                opacity: 0.7
                Layout.leftMargin: Kirigami.Units.largeSpacing
            }
        }
        Repeater {
            model: page.snap.layer || []
            delegate: RuleCard {
                pg: page
                kind: "layer"
                count: (page.snap.layer || []).length
            }
        }
    }

    SettingsTab {
        title: "Workspaces"
        iconName: "virtual-desktops"

        SettingsCard {
            title: "New workspace rule"
            iconName: "list-add"

            SettingRow {
                settingId: "workspace-rules"
                iconName: "virtual-desktops"
                label: "Screen, gaps, kept when empty"
                help: "Workspace rules put a workspace on a screen, keep it when empty or change its gaps. Its layout is set on the Look page."
                highlight: App.highlight
                enabled: page.ready
                Layout.fillWidth: true

                QQC2.Button {
                    objectName: "newWorkspaceRule"
                    text: "New rule"
                    icon.name: "list-add"
                    enabled: page.idle
                    onClicked: editor.startNew("workspace")
                }
            }
            UqLabel {
                visible: page.ready && (page.snap.workspace || []).length === 0
                text: "No workspace rules yet."
                opacity: 0.7
                Layout.leftMargin: Kirigami.Units.largeSpacing
            }
        }
        Repeater {
            model: page.snap.workspace || []
            delegate: RuleCard {
                pg: page
                kind: "workspace"
                count: (page.snap.workspace || []).length
            }
        }
    }

    SettingsTab {
        title: "From apps"
        iconName: "package-x-generic"
        // Like the list, shown only when an app offers rules (or their files have problems).
        available: page.ready && (((page.snap.apps || {}).rules || []).length + ((page.snap.apps || {}).problems || []).length + ((page.snap.apps || {}).missing || []).length) > 0

        // Window rules installed apps offer (Phase 4 §3.3), each off until turned on here.
        SettingsCard {
            title: "Rules apps you installed offer"
            iconName: "package-x-generic"
            wide: true

            AppRuleList {
                objectName: "appRules"
                apps: page.ready ? (page.snap.apps || ({})) : ({})
                canChange: page.idle
                highlight: App.highlight
                Layout.fillWidth: true
            }
        }
    }
}
