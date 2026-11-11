// Displays: where each screen sits, its resolution and refresh rate, scale, rotation, whether
// it is on, mirroring and variable refresh rate. Changes collect in a draft; Apply tries them
// live, and they stay only if kept within 15 seconds. Otherwise the old layout comes back by
// itself, even if this app is closed or the screen went black.
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
            return JSON.parse(dm.stateJson);
        } catch (e) {
            return {};
        }
    }
    readonly property bool ready: App.uqdUp
    readonly property var view: snap.view || []
    readonly property bool dirty: snap.dirty === true
    property string selected: ""
    readonly property var current: view.find(o => o.name === selected) || view[0] || null
    readonly property var cfg: current ? (current.config || {}) : ({})
    property bool showDetails: false

    readonly property var rotations: ["Normal", "Turned right (90°)", "Upside down (180°)", "Turned left (270°)", "Flipped", "Flipped, turned right", "Flipped, upside down", "Flipped, turned left"]
    readonly property var vrrNames: ["Off", "On", "Full screen only", "Full screen games and video"]

    /// `[w, h, rate]` of the selected display's mode: the draft's, else what it runs now.
    readonly property var mode: {
        const m = (cfg.mode || (current ? current.live_mode : "") || "").match(/^(\d+)x(\d+)(?:@([\d.]+))?/);
        return m ? [Number(m[1]), Number(m[2]), m[3] ? Number(m[3]) : 0] : [0, 0, 0];
    }
    readonly property var sizes: current ? current.modes : []
    readonly property int sizeIndex: Math.max(0, sizes.findIndex(s => s.w === mode[0] && s.h === mode[1]))
    readonly property var size: sizes[sizeIndex] || ({ w: 0, h: 0, rates: [], scale_choices: [1] })
    readonly property real scale: cfg.scale !== undefined ? cfg.scale : (current ? current.live_scale : 1)
    readonly property var scales: {
        const s = (size.scale_choices || [1]).slice();
        if (s.indexOf(page.scale) < 0)
            s.unshift(page.scale);
        return s;
    }
    readonly property var others: view.filter(o => current && o.name !== current.name)

    function edit(field, value) {
        if (current)
            dm.edit(current.name, field, JSON.stringify(value));
    }
    function rateText(r) {
        return (Math.round(r * 100) / 100) + " Hz";
    }
    function refresh() {
        dm.refresh();
    }

    Component.onCompleted: dm.refresh()
    Accessible.role: Accessible.Pane
    Accessible.name: "Displays"
    contentWidth: availableWidth

    DisplaysModel {
        id: dm
    }

    Connections {
        target: App
        function onUqdUpChanged() {
            if (App.uqdUp)
                dm.refresh();
        }
    }

    RevertDialog {
        id: revertDialog
        parent: QQC2.Overlay.overlay
        active: dm.pendingSeconds >= 0
        seconds: Math.max(0, dm.pendingSeconds)
        onKeep: dm.keep()
        onRevert: dm.revert()
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
            text: "Displays"
            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.5
            font.bold: true
            Accessible.role: Accessible.Heading
        }
        ErrorCard {
            objectName: "uqdDown"
            visible: !page.ready
            title: "uQ's settings service isn't running"
            detail: "Start the uQ desktop service to change these settings."
            Layout.fillWidth: true
        }
        ErrorCard {
            objectName: "noDisplays"
            visible: page.ready && page.view.length === 0 && !dm.busy
            title: "Can't read your displays right now"
            detail: "Try again in a moment."
            Layout.fillWidth: true
        }
        ErrorCard {
            objectName: "changeError"
            visible: dm.error !== ""
            title: "Your displays weren't changed"
            detail: dm.error
            Layout.fillWidth: true
        }

        SettingRow {
            settingId: "display-arrangement"
            label: "Arrangement: drag a display to move it"
            highlight: App.highlight
            visible: page.view.length > 1
            Layout.fillWidth: true
        }
        DisplayArrangement {
            objectName: "arrangement"
            visible: page.view.length > 1
            outputs: page.view
            selected: page.current ? page.current.name : ""
            enabled: page.ready && !dm.busy
            onPicked: name => page.selected = name
            onMoved: (name, x, y) => dm.moveOutput(name, x, y)
            Layout.fillWidth: true
            Layout.preferredHeight: Kirigami.Units.gridUnit * 12
        }

        RowLayout {
            visible: page.view.length > 1
            Layout.fillWidth: true
            UqLabel {
                text: "Display"
                Layout.fillWidth: true
            }
            UqComboBox {
                objectName: "displayPicker"
                model: page.view.map(o => o.label)
                currentIndex: Math.max(0, page.view.findIndex(o => page.current && o.name === page.current.name))
                Accessible.name: "Display"
                Layout.preferredWidth: Kirigami.Units.gridUnit * 16
                onActivated: index => page.selected = page.view[index].name
            }
        }

        SwitchRow {
            settingId: "display-enabled"
            label: "Use this display"
            highlight: App.highlight
            enabled: page.ready && page.current !== null
            value: page.current ? page.current.enabled : true
            onChosen: v => page.edit("enabled", v ? null : false)
            Layout.fillWidth: true
        }
        SettingRow {
            settingId: "display-resolution"
            label: "Resolution"
            highlight: App.highlight
            enabled: page.ready && page.current !== null && page.current.enabled
            Layout.fillWidth: true

            UqComboBox {
                model: page.sizes.map(s => s.w + " × " + s.h)
                currentIndex: page.sizeIndex
                Accessible.name: "Resolution"
                onActivated: index => {
                    const s = page.sizes[index];
                    page.edit("mode", s.w + "x" + s.h + (s.rates.length ? "@" + s.rates[0] : ""));
                }
            }
        }
        SettingRow {
            settingId: "display-refresh"
            label: "Refresh rate"
            highlight: App.highlight
            enabled: page.ready && page.current !== null && page.current.enabled && page.size.rates.length > 0
            Layout.fillWidth: true

            UqComboBox {
                model: page.size.rates.map(r => page.rateText(r))
                currentIndex: Math.max(0, page.size.rates.findIndex(r => Math.abs(r - page.mode[2]) < 0.5))
                Accessible.name: "Refresh rate"
                onActivated: index => page.edit("mode", page.size.w + "x" + page.size.h + "@" + page.size.rates[index])
            }
        }
        SettingRow {
            settingId: "display-scale"
            label: "Scale"
            highlight: App.highlight
            enabled: page.ready && page.current !== null && page.current.enabled
            Layout.fillWidth: true

            UqComboBox {
                model: page.scales.map(s => Math.round(s * 100) + "%")
                currentIndex: Math.max(0, page.scales.indexOf(page.scale))
                Accessible.name: "Scale"
                onActivated: index => page.edit("scale", page.scales[index])
            }
        }
        SettingRow {
            settingId: "display-rotation"
            label: "Rotation"
            highlight: App.highlight
            enabled: page.ready && page.current !== null && page.current.enabled
            Layout.fillWidth: true

            UqComboBox {
                model: page.rotations
                currentIndex: page.cfg.transform !== undefined ? page.cfg.transform : 0
                Accessible.name: "Rotation"
                onActivated: index => page.edit("transform", index === 0 ? null : index)
            }
        }
        SettingRow {
            settingId: "display-mirror"
            label: "Show the same picture as"
            highlight: App.highlight
            enabled: page.ready && page.current !== null && page.current.enabled && page.others.length > 0
            Layout.fillWidth: true

            UqComboBox {
                model: ["Don't mirror"].concat(page.others.map(o => o.label))
                currentIndex: page.cfg.mirror_of ? Math.max(0, page.others.findIndex(o => o.name === page.cfg.mirror_of) + 1) : 0
                Accessible.name: "Mirror"
                onActivated: index => page.edit("mirror_of", index === 0 ? null : page.others[index - 1].name)
            }
        }
        SettingRow {
            settingId: "display-vrr"
            label: "Variable refresh rate"
            highlight: App.highlight
            enabled: page.ready && page.current !== null && page.current.enabled
            Layout.fillWidth: true

            UqComboBox {
                model: page.vrrNames
                currentIndex: page.cfg.vrr !== undefined ? page.cfg.vrr : 0
                Accessible.name: "Variable refresh rate"
                onActivated: index => page.edit("vrr", index === 0 ? null : index)
            }
        }

        QQC2.Button {
            objectName: "detailsToggle"
            text: "Details"
            checkable: true
            checked: page.showDetails
            visible: page.current !== null
            onToggled: page.showDetails = checked
        }
        UqLabel {
            visible: page.showDetails && page.current !== null
            text: page.current ? "Connector: " + page.current.name + (page.current.description ? "\nModel: " + page.current.description : "") : ""
            wrapMode: Text.Wrap
            opacity: 0.8
            Layout.fillWidth: true
        }

        DisplayArrangements {
            objectName: "arrangements"
            arrangements: page.snap.arrangements || []
            offer: page.snap.offer || ""
            offerName: page.snap.offer_name || ""
            autoSwitch: page.snap.auto_switch !== false
            failed: page.snap.arrangement_failed === true
            idle: page.ready && !dm.busy && dm.pendingSeconds < 0
            highlight: App.highlight
            onRemember: name => dm.remember(name)
            onRename: (id, name) => dm.renameArrangement(id, name)
            onForget: id => dm.forgetArrangement(id)
            onAutoSwitchChosen: on => dm.setAutoSwitch(on)
            Layout.fillWidth: true
        }

        RowLayout {
            Layout.topMargin: Kirigami.Units.largeSpacing
            Layout.alignment: Qt.AlignRight
            QQC2.Button {
                objectName: "discard"
                text: "Discard"
                enabled: page.dirty && !dm.busy
                onClicked: dm.discard()
            }
            QQC2.Button {
                objectName: "apply"
                text: "Apply"
                enabled: page.ready && page.dirty && !dm.busy && dm.pendingSeconds < 0
                onClicked: dm.apply()
            }
        }

        UqLabel {
            text: "Main display and bar"
            font.bold: true
            Layout.topMargin: Kirigami.Units.largeSpacing
            Accessible.role: Accessible.Heading
        }
        DisplayRoles {
            objectName: "roles"
            screens: page.view.map(o => ({ name: o.name, label: o.label }))
            main: page.snap.main || null
            bar: page.snap.bar || ({ show: "all", outputs: [] })
            highlight: App.highlight
            enabled: page.ready && !dm.busy && dm.pendingSeconds < 0
            onMainChosen: output => dm.setMain(output)
            onBarChosen: (show, outputs) => dm.setBar(show, JSON.stringify(outputs))
            Layout.fillWidth: true
        }

        UqLabel {
            text: "Workspaces"
            font.bold: true
            Layout.topMargin: Kirigami.Units.largeSpacing
            Accessible.role: Accessible.Heading
        }
        WorkspacePlanner {
            objectName: "planner"
            plan: page.snap.plan || ({ strategy: "off", count: 10, move_back: true, manual: [], screens: [], preview: [] })
            highlight: App.highlight
            enabled: page.ready && !dm.busy && dm.pendingSeconds < 0
            onPlanChosen: (strategy, count, moveBack) => dm.setPlan(strategy, count, moveBack)
            onManualChosen: (workspace, output) => dm.setManual(workspace, output)
            Layout.fillWidth: true
        }

        Item {
            implicitHeight: Kirigami.Units.largeSpacing
        }
    }
}
