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

SettingsPage {
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
    readonly property var choiceIcons: ({
            "suspend": "system-suspend",
            "hibernate": "system-hibernate",
            "poweroff": "system-shutdown",
            "lock": "system-lock-screen",
            "ignore": "dialog-cancel"
        })
    readonly property var profileLabels: ({
            "power-saver": "Power saver",
            "balanced": "Balanced",
            "performance": "Performance"
        })
    readonly property var profileIcons: ({
            "power-saver": "battery-profile-powersave",
            "balanced": "battery-profile-balanced",
            "performance": "battery-profile-performance"
        })
    property int noticeSeen: 0

    /// The combo's entries: the offered actions, and the saved one when it is something else.
    function actionItems(current) {
        const out = page.choices.map(c => ({
                    text: page.choiceLabels[c] || c,
                    icon: page.choiceIcons[c] || ""
                }));
        if (current && page.choices.indexOf(current) < 0)
            out.push({
                text: "Something else (" + current + ")",
                icon: "dialog-question"
            });
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
    /// The battery icon for a charge level, charging or not.
    function batteryIcon(pct, charging) {
        const step = Math.max(0, Math.min(100, Math.round(pct / 10) * 10));
        const name = "battery-" + (step < 100 ? (step < 10 ? "00" : "0") : "") + step;
        return charging ? name + "-charging" : name;
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

    PowerModel {
        id: pm
        onStateJsonChanged: page.showNews()
    }
    // The model follows UPower and power-profiles-daemon itself (power.rs), so the battery and
    // the power mode update as they change.

    banners: [
        RuleMessage {
            objectName: "powerError"
            negative: true
            closable: false
            lines: pm.error !== "" ? ["That wasn't done: " + pm.error] : []
            Layout.fillWidth: true
        },
        RuleMessage {
            id: noticeMessage
            Layout.fillWidth: true
        }
    ]

    SettingsTab {
        title: "Power"
        iconName: "battery-profile-balanced"

        SettingsCard {
            id: modeCard
            title: "Power mode"
            iconName: "speedometer"
            help: "Balanced suits most uses. Power saver makes the battery last longer; Performance is faster but uses more power."

            SettingRow {
                settingId: "power-mode"
                label: ""
                highlight: App.highlight
                wideControls: true
                visible: page.profiles.available === true
                Layout.fillWidth: true

                QQC2.ButtonGroup {
                    id: profileGroup
                }
                Repeater {
                    model: page.profiles.list || []
                    delegate: QQC2.Button {
                        id: tile
                        required property string modelData
                        readonly property string label: page.profileLabels[modelData] || modelData
                        objectName: "profile-" + modelData
                        checkable: true
                        checked: page.profiles.active === modelData
                        enabled: page.idle
                        QQC2.ButtonGroup.group: profileGroup
                        Accessible.name: tile.label
                        // The three share the row's width equally.
                        Layout.fillWidth: true
                        Layout.preferredWidth: Kirigami.Units.gridUnit * 7
                        Layout.preferredHeight: Kirigami.Units.gridUnit * 4.5
                        contentItem: ColumnLayout {
                            spacing: Kirigami.Units.smallSpacing
                            Kirigami.Icon {
                                source: page.profileIcons[tile.modelData] || "speedometer"
                                implicitWidth: Kirigami.Units.iconSizes.medium
                                implicitHeight: Kirigami.Units.iconSizes.medium
                                Layout.alignment: Qt.AlignHCenter
                                Accessible.ignored: true
                            }
                            UqLabel {
                                text: tile.label
                                font.bold: tile.checked
                                horizontalAlignment: Text.AlignHCenter
                                elide: Text.ElideRight
                                Layout.fillWidth: true
                            }
                        }
                        onClicked: {
                            if (page.profiles.active !== modelData)
                                pm.setProfile(modelData);
                            tile.checked = Qt.binding(() => page.profiles.active === tile.modelData);
                        }
                    }
                }
            }
            RowLayout {
                visible: page.profiles.available === true && !!page.profiles.degraded
                spacing: Kirigami.Units.smallSpacing
                Layout.fillWidth: true
                Layout.leftMargin: Kirigami.Units.largeSpacing
                Kirigami.Icon {
                    source: "dialog-warning"
                    implicitWidth: Kirigami.Units.iconSizes.small
                    implicitHeight: Kirigami.Units.iconSizes.small
                    Accessible.ignored: true
                }
                UqLabel {
                    text: "Performance is limited right now (" + String(page.profiles.degraded || "") + ")."
                    wrapMode: Text.Wrap
                    opacity: 0.8
                    Layout.fillWidth: true
                }
            }
            ErrorCard {
                objectName: "profilesMissing"
                visible: page.loaded && page.profiles.available === false
                title: "Not available"
                detail: String(page.profiles.text || "")
                Layout.fillWidth: true
            }
        }

        SettingsCard {
            title: "Battery"
            iconName: page.battery ? page.batteryIcon(page.battery.percentage, page.battery.state === "Charging") : "battery-missing"

            // The charge, as a gauge with the number in it.
            RowLayout {
                objectName: "batteryGauge"
                visible: page.battery !== null
                spacing: Kirigami.Units.largeSpacing
                Layout.fillWidth: true
                Layout.leftMargin: Kirigami.Units.largeSpacing
                Layout.rightMargin: Kirigami.Units.largeSpacing
                Accessible.role: Accessible.ProgressBar
                Accessible.name: page.battery ? "Battery " + page.battery.percentage + "%, " + String(page.battery.state).toLowerCase() : ""

                RowLayout {
                    spacing: 0
                    Layout.fillWidth: true
                    Rectangle {
                        id: shell
                        readonly property real pct: page.battery ? Math.max(0, Math.min(100, Number(page.battery.percentage))) : 0
                        Layout.fillWidth: true
                        implicitHeight: Kirigami.Units.gridUnit * 2.5
                        radius: Kirigami.Units.cornerRadius * 2
                        color: "transparent"
                        border.width: Math.max(1, Kirigami.Units.smallSpacing / 2)
                        border.color: Qt.rgba(Kirigami.Theme.textColor.r, Kirigami.Theme.textColor.g, Kirigami.Theme.textColor.b, 0.5)

                        Rectangle {
                            anchors.left: parent.left
                            anchors.top: parent.top
                            anchors.bottom: parent.bottom
                            anchors.margins: Kirigami.Units.smallSpacing
                            width: Math.max(0, (parent.width - Kirigami.Units.smallSpacing * 2) * shell.pct / 100)
                            radius: Kirigami.Units.cornerRadius
                            color: shell.pct <= 10 ? Kirigami.Theme.negativeTextColor : shell.pct <= 25 ? Kirigami.Theme.neutralTextColor : Kirigami.Theme.positiveTextColor
                            Behavior on width {
                                NumberAnimation { duration: Kirigami.Units.longDuration }
                            }
                        }
                        RowLayout {
                            anchors.centerIn: parent
                            spacing: Kirigami.Units.smallSpacing
                            Kirigami.Icon {
                                visible: page.battery !== null && page.battery.state === "Charging"
                                source: "battery-full-charging"
                                implicitWidth: Kirigami.Units.iconSizes.small
                                implicitHeight: Kirigami.Units.iconSizes.small
                                Accessible.ignored: true
                            }
                            UqLabel {
                                objectName: "batteryPercent"
                                text: page.battery ? page.battery.percentage + "%" : ""
                                font.bold: true
                                font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.3
                            }
                        }
                    }
                    // The battery's tip.
                    Rectangle {
                        implicitWidth: Kirigami.Units.smallSpacing
                        implicitHeight: shell.implicitHeight / 2.5
                        radius: Kirigami.Units.cornerRadius / 2
                        color: Qt.rgba(Kirigami.Theme.textColor.r, Kirigami.Theme.textColor.g, Kirigami.Theme.textColor.b, 0.5)
                    }
                }
            }
            SettingRow {
                settingId: "battery"
                iconName: page.battery ? "chronometer" : "battery-missing"
                label: page.battery ? String(page.battery.state) : "No battery found"
                highlight: App.highlight
                visible: page.power.available === true
                Layout.fillWidth: true

                UqLabel {
                    objectName: "batteryTime"
                    visible: page.battery !== null && page.battery.time !== ""
                    text: page.battery ? String(page.battery.time) : ""
                    opacity: 0.8
                }
            }
            SettingRow {
                objectName: "batteryHealth"
                visible: page.battery !== null && page.battery.health !== null
                iconName: "security-high"
                label: "Health"
                help: "How much charge the battery holds now, compared with when it was new."
                Layout.fillWidth: true

                UqLabel {
                    text: page.battery && page.battery.health !== null ? page.battery.health + "%" : ""
                    opacity: 0.8
                }
            }
            ErrorCard {
                objectName: "upowerMissing"
                visible: page.loaded && page.power.available === false
                title: "Not available"
                detail: String(page.power.text || "")
                Layout.fillWidth: true
            }
        }

        SettingsCard {
            title: "When you…"
            iconName: "system-shutdown"
            help: page.actions.helper === true ? "Changing these asks for an administrator password. They take effect the next time the computer starts." : ""
            wide: true

            UqLabel {
                objectName: "helperMissing"
                visible: page.loaded && !page.actions.helper
                text: "These can't be changed here: " + String(page.actions.helper_note || "")
                wrapMode: Text.Wrap
                opacity: 0.7
                Layout.fillWidth: true
                Layout.leftMargin: Kirigami.Units.largeSpacing
            }
            GridLayout {
                visible: page.actions.helper === true
                columns: page.width > Kirigami.Units.gridUnit * 80 ? 3 : 1
                columnSpacing: Kirigami.Units.largeSpacing
                rowSpacing: Kirigami.Units.smallSpacing
                uniformCellWidths: true
                Layout.fillWidth: true
                id: actionsGrid

                SettingRow {
                    settingId: "lid-action"
                    iconName: "computer-laptop"
                    label: "Close the lid"
                    highlight: App.highlight
                    visible: page.power.lid !== false
                    Layout.fillWidth: true

                    IconComboBox {
                        id: lidCombo
                        model: page.actionItems(page.saved.lid)
                        currentIndex: page.actionIndex(page.saved.lid)
                        enabled: page.idle
                        Accessible.name: "When the lid is closed"
                        Layout.preferredWidth: Kirigami.Units.gridUnit * 10
                        onActivated: index => {
                            if (index < page.choices.length)
                                page.saveAction("lid", index);
                            lidCombo.currentIndex = Qt.binding(() => page.actionIndex(page.saved.lid));
                        }
                    }
                }
                SettingRow {
                    settingId: "lid-action-ac"
                    iconName: "battery-full-charging"
                    label: "Close the lid, plugged in"
                    highlight: App.highlight
                    visible: page.power.lid !== false
                    Layout.fillWidth: true

                    IconComboBox {
                        id: lidAcCombo
                        model: page.actionItems(page.saved.lid_on_ac)
                        currentIndex: page.actionIndex(page.saved.lid_on_ac)
                        enabled: page.idle
                        Accessible.name: "When the lid is closed while plugged in"
                        Layout.preferredWidth: Kirigami.Units.gridUnit * 10
                        onActivated: index => {
                            if (index < page.choices.length)
                                page.saveAction("lid_on_ac", index);
                            lidAcCombo.currentIndex = Qt.binding(() => page.actionIndex(page.saved.lid_on_ac));
                        }
                    }
                }
                SettingRow {
                    settingId: "power-key-action"
                    iconName: "system-shutdown"
                    label: "Press the power button"
                    highlight: App.highlight
                    Layout.fillWidth: true

                    IconComboBox {
                        id: keyCombo
                        model: page.actionItems(page.saved.power_key)
                        currentIndex: page.actionIndex(page.saved.power_key)
                        enabled: page.idle
                        Accessible.name: "When the power button is pressed"
                        Layout.preferredWidth: Kirigami.Units.gridUnit * 10
                        onActivated: index => {
                            if (index < page.choices.length)
                                page.saveAction("power_key", index);
                            keyCombo.currentIndex = Qt.binding(() => page.actionIndex(page.saved.power_key));
                        }
                    }
                }
            }
            RowLayout {
                objectName: "pendingNote"
                visible: page.actions.helper === true && page.actions.pending === true
                spacing: Kirigami.Units.smallSpacing
                Layout.fillWidth: true
                Layout.leftMargin: Kirigami.Units.largeSpacing
                Kirigami.Icon {
                    source: "system-reboot"
                    implicitWidth: Kirigami.Units.iconSizes.small
                    implicitHeight: Kirigami.Units.iconSizes.small
                    Accessible.ignored: true
                }
                UqLabel {
                    text: "Takes effect the next time the computer starts."
                    wrapMode: Text.Wrap
                    color: Kirigami.Theme.neutralTextColor
                    Layout.fillWidth: true
                }
            }
        }
    }
}
