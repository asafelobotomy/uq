// uqlip's Settings page: the clipboard history, what it keeps and what it never keeps.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.uq.settings
import "logic.js" as Logic

SettingsPage {
    id: page
    pageId: "uqlip"

    property var ctx
    property string highlight
    property var config: null
    property var status: null
    property string problem: ""

    function refresh() {
        if (!page.ctx)
            return;
        page.ctx.command.run("status", [], r => {
            page.status = r && !r.error ? r.output : null;
        });
    }

    function done(r) {
        page.problem = Logic.problemOf(r);
        page.refresh();
    }

    function setValue(key, value) {
        page.ctx.command.run("set", [key, String(value)], page.done);
    }

    onCtxChanged: {
        if (!page.ctx)
            return;
        page.ctx.command.watch("config", r => {
            if (r && !r.error)
                page.config = r.output;
            page.refresh();
        });
    }

    banners: [
        ErrorCard {
            visible: page.problem !== ""
            title: "That didn't work"
            detail: page.problem
        }
    ]

    SettingsTab {
        title: "History"
        iconName: "edit-paste"

        SettingsCard {
            title: "Clipboard history"
            iconName: "edit-paste"
            help: "Open it with Super+Shift+V or the clipboard island on the bar. Pick an entry and it goes back on the clipboard; paste it where you like."
            UqLabel {
                text: Logic.describe(page.status)
                wrapMode: Text.Wrap
                Layout.fillWidth: true
            }
            SwitchRow {
                settingId: "enabled"
                label: "Keep a clipboard history"
                highlight: page.highlight
                value: page.config ? page.config.enabled !== false : true
                onChosen: v => page.setValue("enabled", v ? "true" : "false")
            }
            SettingRow {
                settingId: "pause"
                label: "Pause"
                highlight: page.highlight
                QQC2.Button {
                    text: page.status && page.status.paused ? "Keep copies again" : "Stop keeping copies"
                    enabled: page.status !== null
                    onClicked: page.ctx.command.run(page.status.paused ? "resume" : "pause", [], page.done)
                }
            }
        }

        SettingsCard {
            title: "Forget"
            iconName: "edit-clear-history"
            help: "Pinned entries stay unless you choose to forget them too."
            SettingRow {
                settingId: "forget"
                label: "Everything not pinned"
                highlight: page.highlight
                QQC2.Button {
                    text: "Forget"
                    onClicked: page.ctx.command.run("clear", [], page.done)
                }
            }
            SettingRow {
                label: "Everything, pinned too"
                QQC2.Button {
                    text: "Forget all"
                    onClicked: page.ctx.command.run("clear-all", [], page.done)
                }
            }
        }
    }

    SettingsTab {
        title: "Privacy"
        iconName: "security-high"

        SettingsCard {
            title: "Never kept"
            iconName: "security-high"
            help: "Copies marked secret by a password manager are never read. Copies from the apps below are skipped too."
            UqLabel {
                text: "Copies from these apps are never kept (the app's window class):"
                wrapMode: Text.Wrap
                Layout.fillWidth: true
            }
            Repeater {
                model: Logic.apps(page.config)
                delegate: SettingRow {
                    required property string modelData
                    label: modelData
                    QQC2.Button {
                        text: "Remove"
                        onClicked: page.ctx.command.run("ignore-remove", [modelData], page.done)
                    }
                }
            }
            SettingRow {
                settingId: "ignored"
                label: "Add an app"
                highlight: page.highlight
                QQC2.TextField {
                    id: appName
                    placeholderText: "Window class"
                    maximumLength: 64
                    onAccepted: addApp.clicked()
                }
                QQC2.Button {
                    id: addApp
                    text: "Add"
                    enabled: Logic.validApp(appName.text)
                    onClicked: {
                        page.ctx.command.run("ignore-add", [appName.text.trim()], page.done);
                        appName.text = "";
                    }
                }
            }
        }
    }

    SettingsTab {
        title: "Forgetting"
        iconName: "edit-clear-history"

        SettingsCard {
            title: "Automatic forgetting"
            iconName: "edit-clear-history"
            help: "Pinned entries are never removed."
            SettingRow {
                settingId: "expire_hours"
                label: "Forget copies after (hours, 0 = never)"
                highlight: page.highlight
                QQC2.SpinBox {
                    from: 0
                    to: 8760
                    stepSize: 6
                    editable: true
                    value: page.config ? page.config.expire_hours : 0
                    onValueModified: page.setValue("expire_hours", value)
                }
            }
            SwitchRow {
                settingId: "clear_on_login"
                label: "Forget copies at login"
                highlight: page.highlight
                value: page.config ? page.config.clear_on_login === true : false
                onChosen: v => page.setValue("clear_on_login", v ? "true" : "false")
            }
            SwitchRow {
                settingId: "skip_secret_looking"
                label: "Skip copies that look like secrets"
                highlight: page.highlight
                value: page.config && page.config.privacy ? page.config.privacy.skip_secret_looking !== false : true
                onChosen: v => page.setValue("skip_secret_looking", v ? "true" : "false")
            }
        }
    }

    SettingsTab {
        title: "Limits"
        iconName: "preferences-system"

        SettingsCard {
            title: "How much is kept"
            iconName: "preferences-system"
            help: "Older entries go first. Pinned entries are never removed."
            SettingRow {
                settingId: "max_items"
                label: "Entries kept"
                highlight: page.highlight
                QQC2.SpinBox {
                    from: 10
                    to: 5000
                    stepSize: 10
                    editable: true
                    value: page.config ? page.config.max_items : 200
                    onValueModified: page.setValue("max_items", value)
                }
            }
            SettingRow {
                settingId: "max_entry_mb"
                label: "Largest entry (MB)"
                highlight: page.highlight
                QQC2.SpinBox {
                    from: 1
                    to: 256
                    editable: true
                    value: page.config ? page.config.max_entry_mb : 16
                    onValueModified: page.setValue("max_entry_mb", value)
                }
            }
            SettingRow {
                settingId: "max_text_kb"
                label: "Longest text (KB)"
                highlight: page.highlight
                QQC2.SpinBox {
                    from: 1
                    to: 65536
                    editable: true
                    value: Logic.textKb(page.config)
                    onValueModified: page.setValue("max_text_kb", value)
                }
            }
        }
    }
}
