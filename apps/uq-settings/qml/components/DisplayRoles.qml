// The main display and where the bar shows. Both are saved as soon as they are chosen. Screen
// names come from the screens themselves, so they are shown through UqLabel (plain text), never
// as a stock control's `text:`.
//
// `screens` is `[{name, label}]` (the connected displays), `main` is the snapshot's
// `{output, label, connected}` or null, and `bar` is `{show, outputs}`.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import "."

ColumnLayout {
    id: root

    property var screens: []
    property var main: null
    property var bar: ({ show: "all", outputs: [] })
    property string highlight
    readonly property alias mainBox: mainCombo
    readonly property alias barBox: barCombo
    readonly property alias barRepeater: barScreens
    readonly property alias missingLabel: missing
    readonly property var barKeys: ["all", "main", "chosen"]
    readonly property string barShow: bar && bar.show ? bar.show : "all"
    readonly property var barOutputs: bar && bar.outputs ? bar.outputs : []
    readonly property bool mainMissing: main !== null && main !== undefined && main.connected === false
    readonly property int mainIndex: {
        if (!main || main.connected === false || !main.output)
            return 0;
        return screens.findIndex(s => s.name === main.output) + 1;
    }

    /// A display was chosen as the main one ("" = none).
    signal mainChosen(string output)
    /// The bar's places were chosen: `show` is all, main or chosen; `outputs` the connector
    /// names for chosen.
    signal barChosen(string show, var outputs)

    function names() {
        return screens.map(s => s.name);
    }
    function toggled(name, on) {
        const now = barOutputs.filter(n => n !== name);
        if (on)
            now.push(name);
        return screens.map(s => s.name).filter(n => now.indexOf(n) >= 0);
    }

    spacing: Kirigami.Units.smallSpacing

    SettingRow {
        settingId: "display-main"
        iconName: "favorite"
        label: "Main display"
        help: "Notifications, pop-ups and Spotlight open on the main display."
        highlight: root.highlight
        enabled: root.enabled
        Layout.fillWidth: true

        UqComboBox {
            id: mainCombo
            objectName: "mainBox"
            model: ["Not chosen"].concat(root.screens.map(s => s.label))
            currentIndex: root.mainIndex
            // A new model (a screen came or went) resets the index.
            onModelChanged: Qt.callLater(() => currentIndex = Qt.binding(() => root.mainIndex))
            Accessible.name: "Main display"
            Layout.preferredWidth: Kirigami.Units.gridUnit * 16
            onActivated: index => {
                root.mainChosen(index === 0 ? "" : root.screens[index - 1].name);
                currentIndex = Qt.binding(() => root.mainIndex);
            }
        }
    }
    UqLabel {
        id: missing
        objectName: "mainMissing"
        visible: root.mainMissing
        text: root.mainMissing ? "“" + root.main.label + "” isn't in use right now. Another display is used until it is." : ""
        wrapMode: Text.Wrap
        color: Kirigami.Theme.neutralTextColor
        Layout.fillWidth: true
        Layout.leftMargin: Kirigami.Units.largeSpacing
    }

    SettingRow {
        settingId: "display-bar"
        iconName: "view-split-top-bottom"
        label: "Show the bar on"
        highlight: root.highlight
        enabled: root.enabled
        Layout.fillWidth: true

        UqComboBox {
            id: barCombo
            objectName: "barBox"
            model: ["All displays", "Main display only", "Choose"]
            currentIndex: Math.max(0, root.barKeys.indexOf(root.barShow))
            Accessible.name: "Show the bar on"
            Layout.preferredWidth: Kirigami.Units.gridUnit * 16
            onActivated: index => {
                const show = root.barKeys[index];
                const keep = root.barOutputs.filter(n => root.names().indexOf(n) >= 0);
                root.barChosen(show, show === "chosen" ? (keep.length > 0 ? keep : root.names()) : []);
                currentIndex = Qt.binding(() => Math.max(0, root.barKeys.indexOf(root.barShow)));
            }
        }
    }
    ColumnLayout {
        objectName: "barScreens"
        visible: root.barShow === "chosen"
        spacing: 0
        Layout.leftMargin: Kirigami.Units.largeSpacing * 2
        Layout.fillWidth: true

        Repeater {
            id: barScreens
            model: root.screens
            QQC2.CheckBox {
                id: box
                required property var modelData
                readonly property bool on: root.barOutputs.indexOf(modelData.name) >= 0
                objectName: "barScreen"
                checked: on
                // At least one display keeps the bar.
                enabled: root.enabled && !(on && root.barOutputs.length <= 1)
                Accessible.name: "Show the bar on " + modelData.label
                contentItem: UqLabel {
                    text: box.modelData.label
                    elide: Text.ElideRight
                    verticalAlignment: Text.AlignVCenter
                    leftPadding: box.indicator.width + box.spacing
                }
                onToggled: {
                    root.barChosen("chosen", root.toggled(box.modelData.name, box.checked));
                    box.checked = Qt.binding(() => box.on);
                }
            }
        }
    }
}
