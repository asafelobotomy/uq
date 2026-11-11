// Power (spec §2.6): the power mode, the battery, and what closing the lid or pressing the power
// button does. The actions are saved by uQ's system settings helper after an administrator
// password, and take effect after a restart; without the helper they are hidden with a note. A
// service that isn't installed shows "Not available" with the package to install.
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
            return JSON.parse(pm.stateJson);
        } catch (e) {
            return {};
        }
    }
    readonly property bool loaded: snap.actions !== undefined
    readonly property bool idle: page.loaded && !pm.busy
    readonly property var profiles: snap.profiles || ({})
    readonly property var power: snap.power || ({})
    readonly property var battery: power.battery || null
    readonly property var actions: snap.actions || ({})
    readonly property var saved: actions.saved || ({})
    readonly property var choices: actions.choices || []
    readonly property var choiceLabels: ({
            "suspend": "Sleep",
            "hibernate": "Hibernate",
            "poweroff": "Shut down",
            "lock": "Lock the screen",
            "ignore": "Do nothing"
        })
    readonly property var profileLabels: ({
            "power-saver": "Power saver",
            "balanced": "Balanced",
            "performance": "Performance"
        })
    property int noticeSeen: 0

    /// The combo's labels: the offered actions, and the saved one when it is something else.
    function actionLabels(current) {
        const out = page.choices.map(c => page.choiceLabels[c] || c);
        if (current && page.choices.indexOf(current) < 0)
            out.push("Something else (" + current + ")");
        return out;
    }
    function actionIndex(current) {
        const i = page.choices.indexOf(current);
        return i < 0 ? page.choices.length : i;
    }
    function saveAction(key, index) {
        const next = {
            lid: page.saved.lid,
            lid_on_ac: page.saved.lid_on_ac,
            power_key: page.saved.power_key
        };
        next[key] = page.choices[index];
        pm.setActions(String(next.lid), String(next.lid_on_ac), String(next.power_key));
    }

    function refresh() {
        pm.refresh();
    }
    function showNews() {
        const notice = page.snap.notice;
        if (notice && notice.seq > page.noticeSeen) {
            page.noticeSeen = notice.seq;
            noticeMessage.show(notice.text !== "" ? [notice.text] : []);
        }
    }

    Accessible.role: Accessible.Pane
    Accessible.name: "Power"
    contentWidth: availableWidth

    PowerModel {
        id: pm
        onStateJsonChanged: page.showNews()
    }
    // The battery changes on its own; read it again while the page is open.
    Timer {
        interval: 30000
        repeat: true
        running: page.visible
        onTriggered: pm.refresh()
    }

    ColumnLayout {
        width: Math.min(page.availableWidth - Kirigami.Units.largeSpacing * 2, Kirigami.Units.gridUnit * 44)
        x: Math.max(Kirigami.Units.largeSpacing, (page.availableWidth - width) / 2)
        spacing: Kirigami.Units.smallSpacing

        Item {
            implicitHeight: Kirigami.Units.smallSpacing
        }
        UqLabel {
            text: "Power"
            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.5
            font.bold: true
            Accessible.role: Accessible.Heading
        }
        RuleMessage {
            objectName: "powerError"
            negative: true
            closable: false
            lines: pm.error !== "" ? ["That wasn't done: " + pm.error] : []
            Layout.fillWidth: true
        }
        RuleMessage {
            id: noticeMessage
            Layout.fillWidth: true
        }

        // Power mode.
        SettingRow {
            settingId: "power-mode"
            label: "Power mode"
            highlight: App.highlight
            visible: page.profiles.available === true
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.largeSpacing

            UqComboBox {
                id: profileCombo
                readonly property var ids: page.profiles.list || []
                model: profileCombo.ids.map(p => page.profileLabels[p] || p)
                currentIndex: profileCombo.ids.indexOf(page.profiles.active)
                enabled: page.idle
                implicitContentWidthPolicy: QQC2.ComboBox.WidestText
                Accessible.name: "Power mode"
                onActivated: index => {
                    pm.setProfile(profileCombo.ids[index]);
                    profileCombo.currentIndex = Qt.binding(() => profileCombo.ids.indexOf(page.profiles.active));
                }
            }
        }
        UqLabel {
            visible: page.profiles.available === true && !!page.profiles.degraded
            text: "Performance is limited right now (" + String(page.profiles.degraded || "") + ")."
            wrapMode: Text.Wrap
            opacity: 0.7
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing
        }
        ErrorCard {
            objectName: "profilesMissing"
            visible: page.loaded && page.profiles.available === false
            title: "Power modes: not available"
            detail: String(page.profiles.text || "")
            Layout.fillWidth: true
        }

        // Battery.
        SettingRow {
            settingId: "battery"
            label: page.battery ? "Battery: " + page.battery.percentage + "%, " + page.battery.state.toLowerCase() : "Battery: none found"
            highlight: App.highlight
            visible: page.power.available === true
            Layout.fillWidth: true
        }
        UqLabel {
            visible: page.battery !== null && page.battery.time !== ""
            text: page.battery ? String(page.battery.time) : ""
            opacity: 0.7
            Layout.leftMargin: Kirigami.Units.largeSpacing
        }
        UqLabel {
            objectName: "batteryHealth"
            visible: page.battery !== null && page.battery.health !== null
            text: page.battery ? "Health: holds " + page.battery.health + "% of its original charge" : ""
            wrapMode: Text.Wrap
            opacity: 0.7
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing
        }
        ErrorCard {
            objectName: "upowerMissing"
            visible: page.loaded && page.power.available === false
            title: "Battery: not available"
            detail: String(page.power.text || "")
            Layout.fillWidth: true
        }

        // Lid and power button.
        UqLabel {
            text: "When you…"
            font.bold: true
            Layout.topMargin: Kirigami.Units.largeSpacing
        }
        UqLabel {
            objectName: "helperMissing"
            visible: page.loaded && !page.actions.helper
            text: "These can't be changed here: " + String(page.actions.helper_note || "")
            wrapMode: Text.Wrap
            opacity: 0.7
            Layout.fillWidth: true
        }
        SettingRow {
            settingId: "lid-action"
            label: "Close the lid"
            highlight: App.highlight
            visible: page.actions.helper === true && page.power.lid !== false
            Layout.fillWidth: true

            UqComboBox {
                id: lidCombo
                model: page.actionLabels(page.saved.lid)
                currentIndex: page.actionIndex(page.saved.lid)
                enabled: page.idle
                implicitContentWidthPolicy: QQC2.ComboBox.WidestText
                Accessible.name: "When the lid is closed"
                onActivated: index => {
                    if (index < page.choices.length)
                        page.saveAction("lid", index);
                    lidCombo.currentIndex = Qt.binding(() => page.actionIndex(page.saved.lid));
                }
            }
        }
        SettingRow {
            settingId: "lid-action-ac"
            label: "Close the lid while plugged in"
            highlight: App.highlight
            visible: page.actions.helper === true && page.power.lid !== false
            Layout.fillWidth: true

            UqComboBox {
                id: lidAcCombo
                model: page.actionLabels(page.saved.lid_on_ac)
                currentIndex: page.actionIndex(page.saved.lid_on_ac)
                enabled: page.idle
                implicitContentWidthPolicy: QQC2.ComboBox.WidestText
                Accessible.name: "When the lid is closed while plugged in"
                onActivated: index => {
                    if (index < page.choices.length)
                        page.saveAction("lid_on_ac", index);
                    lidAcCombo.currentIndex = Qt.binding(() => page.actionIndex(page.saved.lid_on_ac));
                }
            }
        }
        SettingRow {
            settingId: "power-key-action"
            label: "Press the power button"
            highlight: App.highlight
            visible: page.actions.helper === true
            Layout.fillWidth: true

            UqComboBox {
                id: keyCombo
                model: page.actionLabels(page.saved.power_key)
                currentIndex: page.actionIndex(page.saved.power_key)
                enabled: page.idle
                implicitContentWidthPolicy: QQC2.ComboBox.WidestText
                Accessible.name: "When the power button is pressed"
                onActivated: index => {
                    if (index < page.choices.length)
                        page.saveAction("power_key", index);
                    keyCombo.currentIndex = Qt.binding(() => page.actionIndex(page.saved.power_key));
                }
            }
        }
        UqLabel {
            objectName: "pendingNote"
            visible: page.actions.helper === true && page.actions.pending === true
            text: "Changes here take effect the next time the computer starts. Until then the old actions still apply."
            wrapMode: Text.Wrap
            color: Kirigami.Theme.neutralTextColor
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing
        }
        UqLabel {
            visible: page.actions.helper === true
            text: "Changing these asks for an administrator password."
            wrapMode: Text.Wrap
            opacity: 0.7
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing
        }
        Item {
            implicitHeight: Kirigami.Units.largeSpacing
        }
    }
}
