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

SettingsPage {
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
    readonly property bool canEdit: page.ready && page.current !== null && page.current.enabled

    readonly property var rotationNames: ["Normal", "Turned right", "Upside down", "Turned left"]
    readonly property var vrrNames: ["Off", "On", "Full screen only", "Full screen games and video"]
    /// The draft's rotation, 0 to 3, and whether it is flipped (transforms 4 to 7).
    readonly property int xform: page.cfg.transform !== undefined && page.cfg.transform !== null ? page.cfg.transform : 0
    readonly property int turn: page.xform % 4
    readonly property bool flipped: page.xform >= 4

    /// `[w, h, rate]` of the selected display's mode: the draft's, else what it runs now.
    readonly property var mode: {
        const m = (cfg.mode || (current ? current.live_mode : "") || "").match(/^(\d+)x(\d+)(?:@([\d.]+))?/);
        return m ? [Number(m[1]), Number(m[2]), m[3] ? Number(m[3]) : 0] : [0, 0, 0];
    }
    readonly property var sizes: current ? current.modes : []
    readonly property int sizeIndex: Math.max(0, sizes.findIndex(s => s.w === mode[0] && s.h === mode[1]))
    readonly property var size: sizes[sizeIndex] || ({ w: 0, h: 0, rates: [], scale_choices: [1] })
    readonly property int rateIndex: Math.max(0, page.size.rates.findIndex(r => Math.abs(r - page.mode[2]) < 0.5))
    readonly property real shownScale: cfg.scale !== undefined ? cfg.scale : (current ? current.live_scale : 1)
    readonly property var scales: {
        const s = (size.scale_choices || [1]).slice();
        if (s.indexOf(page.shownScale) < 0)
            s.unshift(page.shownScale);
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
    function setTransform(turn, flipped) {
        const t = turn + (flipped ? 4 : 0);
        page.edit("transform", t === 0 ? null : t);
    }
    /// The main display's connector, or "" when none is chosen or it isn't connected.
    readonly property string mainOutput: page.snap.main && page.snap.main.connected !== false ? String(page.snap.main.output || "") : ""
    /// Whether the bar shows on the display `name`.
    function barOn(name) {
        const bar = page.snap.bar || ({ show: "all", outputs: [] });
        if (bar.show === "main")
            return page.mainOutput === "" || page.mainOutput === name;
        if (bar.show === "chosen")
            return (bar.outputs || []).indexOf(name) >= 0;
        return true;
    }
    function refresh() {
        dm.refresh();
    }

    Component.onCompleted: dm.refresh()

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

    // Apply and Discard stay on the title row, so they are there on every tab.
    headerItems: [
        QQC2.Button {
            objectName: "discard"
            text: "Discard"
            icon.name: "edit-undo"
            enabled: page.dirty && !dm.busy
            onClicked: dm.discard()
        },
        QQC2.Button {
            objectName: "apply"
            text: "Apply"
            icon.name: "dialog-ok-apply"
            highlighted: page.dirty
            enabled: page.ready && page.dirty && !dm.busy && dm.pendingSeconds < 0
            QQC2.ToolTip.visible: hovered
            QQC2.ToolTip.text: "Try the changes. They go back by themselves unless you keep them within 15 seconds."
            onClicked: dm.apply()
        }
    ]

    banners: [
        ErrorCard {
            objectName: "uqdDown"
            visible: !page.ready
            title: "uQ's settings service isn't running"
            detail: "Start the uQ desktop service to change these settings."
            Layout.fillWidth: true
        },
        ErrorCard {
            objectName: "noDisplays"
            visible: page.ready && page.view.length === 0 && !dm.busy
            title: "Can't read your displays right now"
            detail: "Try again in a moment."
            Layout.fillWidth: true
        },
        ErrorCard {
            objectName: "changeError"
            visible: dm.error !== ""
            title: "Your displays weren't changed"
            detail: dm.error
            Layout.fillWidth: true
        }
    ]

    SettingsTab {
        title: "Arrangement"
        iconName: "preferences-desktop-display-randr"

        // The hero: the screens as they sit. Click one to pick it, drag it to move it.
        SettingsCard {
            id: arrangementCard
            title: "Arrangement"
            iconName: "preferences-desktop-display-randr"
            help: page.view.length > 1 ? "Drag a display to move it. Click one to change its settings." : ""
            Layout.fillHeight: true

            SettingRow {
                settingId: "display-arrangement"
                label: ""
                highlight: App.highlight
                wideControls: true
                Layout.fillWidth: true
                Layout.fillHeight: true

                DisplayArrangement {
                    objectName: "arrangement"
                    outputs: page.view
                    selected: page.current ? page.current.name : ""
                    enabled: page.ready && !dm.busy
                    onPicked: name => page.selected = name
                    onMoved: (name, x, y) => dm.moveOutput(name, x, y)
                    Layout.fillWidth: true
                    Layout.preferredHeight: Kirigami.Units.gridUnit * 16
                }
            }

            // Every display, the ones off or mirrored too: pick one to change it.
            Flow {
                objectName: "displayPicker"
                visible: page.view.length > 1
                spacing: Kirigami.Units.smallSpacing
                Layout.fillWidth: true
                Layout.leftMargin: Kirigami.Units.largeSpacing

                QQC2.ButtonGroup {
                    id: pickGroup
                }
                Repeater {
                    model: page.view
                    QQC2.Button {
                        id: pick
                        required property var modelData
                        checkable: true
                        checked: page.current !== null && page.current.name === modelData.name
                        QQC2.ButtonGroup.group: pickGroup
                        Accessible.name: String(modelData.label)
                        onClicked: page.selected = modelData.name
                        contentItem: RowLayout {
                            spacing: Kirigami.Units.smallSpacing
                            Kirigami.Icon {
                                source: pick.modelData.enabled ? "video-display" : "system-shutdown"
                                implicitWidth: Kirigami.Units.iconSizes.small
                                implicitHeight: Kirigami.Units.iconSizes.small
                                opacity: pick.modelData.enabled ? 1 : 0.5
                                Accessible.ignored: true
                            }
                            UqLabel {
                                text: String(pick.modelData.label)
                                font.bold: pick.checked
                                elide: Text.ElideRight
                                Layout.maximumWidth: Kirigami.Units.gridUnit * 14
                            }
                        }
                    }
                }
            }
        }

        // The picked display's own settings, beside the picture.
        SettingsCard {
            objectName: "displayCard"
            title: page.current ? String(page.current.label) : "Display"
            iconName: "video-display"
            help: page.current ? "Connector " + page.current.name + (page.current.description ? ". " + page.current.description : "") : ""

            SwitchRow {
                settingId: "display-enabled"
                iconName: "system-shutdown"
                label: "Use this display"
                highlight: App.highlight
                enabled: page.ready && page.current !== null
                value: page.current ? page.current.enabled : true
                onChosen: v => page.edit("enabled", v ? null : false)
                Layout.fillWidth: true
            }
            SettingRow {
                settingId: "display-resolution"
                iconName: "view-fullscreen"
                label: "Resolution"
                highlight: App.highlight
                enabled: page.canEdit
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
                iconName: "speedometer"
                label: "Refresh rate"
                highlight: App.highlight
                enabled: page.canEdit && page.size.rates.length > 0
                Layout.fillWidth: true

                // A few rates show as buttons; a long list stays a menu.
                QQC2.ButtonGroup {
                    id: rateGroup
                }
                Repeater {
                    model: page.size.rates.length <= 4 ? page.size.rates : []
                    ChoiceButton {
                        id: rateButton
                        required property real modelData
                        required property int index
                        objectName: "rate"
                        checkable: true
                        checked: index === page.rateIndex
                        QQC2.ButtonGroup.group: rateGroup
                        Accessible.name: page.rateText(modelData)
                        label: page.rateText(rateButton.modelData)
                        onClicked: {
                            page.edit("mode", page.size.w + "x" + page.size.h + "@" + modelData);
                            checked = Qt.binding(() => rateButton.index === page.rateIndex);
                        }
                    }
                }
                UqComboBox {
                    visible: page.size.rates.length > 4
                    model: page.size.rates.map(r => page.rateText(r))
                    currentIndex: page.rateIndex
                    Accessible.name: "Refresh rate"
                    onActivated: index => page.edit("mode", page.size.w + "x" + page.size.h + "@" + page.size.rates[index])
                }
            }
            SettingRow {
                settingId: "display-scale"
                iconName: "zoom-in"
                label: "Scale"
                help: "Bigger makes text and windows larger, with less on the screen."
                highlight: App.highlight
                enabled: page.canEdit
                Layout.fillWidth: true

                QQC2.ButtonGroup {
                    id: scaleGroup
                }
                Repeater {
                    model: page.scales.length <= 6 ? page.scales : []
                    ChoiceButton {
                        id: scaleButton
                        required property real modelData
                        objectName: "scale"
                        checkable: true
                        checked: modelData === page.shownScale
                        QQC2.ButtonGroup.group: scaleGroup
                        Accessible.name: Math.round(modelData * 100) + "%"
                        label: Math.round(scaleButton.modelData * 100) + "%"
                        onClicked: {
                            page.edit("scale", modelData);
                            checked = Qt.binding(() => scaleButton.modelData === page.shownScale);
                        }
                    }
                }
                UqComboBox {
                    visible: page.scales.length > 6
                    model: page.scales.map(s => Math.round(s * 100) + "%")
                    currentIndex: Math.max(0, page.scales.indexOf(page.shownScale))
                    Accessible.name: "Scale"
                    onActivated: index => page.edit("scale", page.scales[index])
                }
            }
            SettingRow {
                settingId: "display-rotation"
                iconName: "object-rotate-right"
                label: "Rotation"
                highlight: App.highlight
                enabled: page.canEdit
                Layout.fillWidth: true

                QQC2.ButtonGroup {
                    id: turnGroup
                }
                // Each button shows a screen turned the way it would be.
                Repeater {
                    model: 4
                    QQC2.Button {
                        id: turnButton
                        required property int index
                        objectName: "turn" + index
                        checkable: true
                        checked: index === page.turn
                        QQC2.ButtonGroup.group: turnGroup
                        Accessible.name: page.rotationNames[index]
                        QQC2.ToolTip.visible: hovered
                        QQC2.ToolTip.text: page.rotationNames[index]
                        implicitWidth: Kirigami.Units.gridUnit * 2.5
                        implicitHeight: Kirigami.Units.gridUnit * 2.5
                        contentItem: Item {
                            Rectangle {
                                anchors.centerIn: parent
                                width: Kirigami.Units.gridUnit * 1.4
                                height: Kirigami.Units.gridUnit * 0.9
                                rotation: turnButton.index * 90
                                radius: 2
                                color: "transparent"
                                border.width: 2
                                border.color: Kirigami.Theme.textColor
                                // The screen's top edge, so the turn shows.
                                Rectangle {
                                    anchors.left: parent.left
                                    anchors.right: parent.right
                                    anchors.top: parent.top
                                    height: 4
                                    color: Kirigami.Theme.highlightColor
                                }
                            }
                        }
                        onClicked: {
                            page.setTransform(index, page.flipped);
                            checked = Qt.binding(() => turnButton.index === page.turn);
                        }
                    }
                }
                QQC2.ToolButton {
                    objectName: "flip"
                    text: "Mirror image"
                    icon.name: "object-flip-horizontal"
                    display: QQC2.AbstractButton.IconOnly
                    checkable: true
                    checked: page.flipped
                    Accessible.name: "Mirror image"
                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.text: "Mirror image"
                    onClicked: {
                        page.setTransform(page.turn, !page.flipped);
                        checked = Qt.binding(() => page.flipped);
                    }
                }
            }
            SettingRow {
                settingId: "display-mirror"
                iconName: "view-split-left-right"
                label: "Show the same picture as"
                highlight: App.highlight
                enabled: page.canEdit && page.others.length > 0
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
                iconName: "kt-speed-limits"
                label: "Variable refresh rate"
                help: "Lets the screen match a game's or video's frame rate, for smoother motion."
                highlight: App.highlight
                enabled: page.canEdit
                Layout.fillWidth: true

                UqComboBox {
                    model: page.vrrNames
                    currentIndex: page.cfg.vrr !== undefined ? page.cfg.vrr : 0
                    Accessible.name: "Variable refresh rate"
                    onActivated: index => page.edit("vrr", index === 0 ? null : index)
                }
            }
        }
    }

    SettingsTab {
        title: "Main display and bar"
        iconName: "favorite"

        SettingsCard {
            title: "Main display and bar"
            iconName: "favorite"

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
        }

        // What the choices give: each screen in use, with the bar where it shows and a star on
        // the main one.
        SettingsCard {
            objectName: "rolesPreview"
            title: "Preview"
            iconName: "view-preview"

            Flow {
                spacing: Kirigami.Units.largeSpacing * 2
                Layout.fillWidth: true
                Layout.margins: Kirigami.Units.largeSpacing

                Repeater {
                    model: page.view.filter(o => o.enabled && !o.mirror_of)
                    ColumnLayout {
                        id: screen
                        required property var modelData
                        readonly property bool isMain: page.mainOutput === modelData.name
                        readonly property bool hasBar: page.barOn(modelData.name)
                        readonly property real aspect: modelData.rect && modelData.rect.h > 0 ? modelData.rect.w / modelData.rect.h : 16 / 9
                        spacing: Kirigami.Units.smallSpacing
                        Accessible.role: Accessible.Graphic
                        Accessible.name: String(modelData.label) + (isMain ? ", main display" : "") + (hasBar ? ", with the bar" : ", no bar")

                        Rectangle {
                            implicitHeight: Kirigami.Units.gridUnit * 5
                            implicitWidth: implicitHeight * screen.aspect
                            radius: Kirigami.Units.cornerRadius
                            color: Kirigami.Theme.backgroundColor
                            border.width: screen.isMain ? 2 : 1
                            border.color: screen.isMain ? Kirigami.Theme.highlightColor : Qt.rgba(Kirigami.Theme.textColor.r, Kirigami.Theme.textColor.g, Kirigami.Theme.textColor.b, 0.3)

                            Rectangle {
                                objectName: "previewBar"
                                visible: screen.hasBar
                                anchors.left: parent.left
                                anchors.right: parent.right
                                anchors.top: parent.top
                                anchors.margins: 3
                                height: Kirigami.Units.smallSpacing * 2
                                radius: 2
                                color: Kirigami.Theme.highlightColor
                            }
                            Kirigami.Icon {
                                visible: screen.isMain
                                source: "favorite"
                                anchors.centerIn: parent
                                implicitWidth: Kirigami.Units.iconSizes.medium
                                implicitHeight: Kirigami.Units.iconSizes.medium
                                Accessible.ignored: true
                            }
                        }
                        UqLabel {
                            text: String(screen.modelData.label)
                            elide: Text.ElideRight
                            horizontalAlignment: Text.AlignHCenter
                            Layout.fillWidth: true
                            Layout.maximumWidth: Kirigami.Units.gridUnit * 12
                        }
                    }
                }
            }
        }
    }

    SettingsTab {
        title: "Workspaces"
        iconName: "virtual-desktops"

        SettingsCard {
            title: "Workspaces across screens"
            iconName: "virtual-desktops"
            wide: true

            WorkspacePlanner {
                objectName: "planner"
                plan: page.snap.plan || ({ strategy: "off", count: 10, move_back: true, manual: [], screens: [], preview: [] })
                highlight: App.highlight
                enabled: page.ready && !dm.busy && dm.pendingSeconds < 0
                onPlanChosen: (strategy, count, moveBack) => dm.setPlan(strategy, count, moveBack)
                onManualChosen: (workspace, output) => dm.setManual(workspace, output)
                Layout.fillWidth: true
            }
        }
    }

    SettingsTab {
        title: "Saved"
        iconName: "document-save"

        SettingsCard {
            title: "Saved arrangements"
            iconName: "bookmarks-organize"
            help: "How your screens are set up for each set of screens you connect."
            wide: true

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
        }
    }
}
