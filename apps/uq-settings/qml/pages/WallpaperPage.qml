// Wallpaper: an image or a folder slideshow, how it fits, and the background colour, for all
// displays or for one (picked from a row of display chips), beside a large preview. Everything is read from and written to uqd through WallpaperModel; a
// refused change shows uqd's message and the controls go back to the last good value.
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
            return JSON.parse(wm.stateJson);
        } catch (e) {
            return {};
        }
    }
    readonly property var outputs: snap.outputs || {}
    readonly property var problems: snap.problems || {}
    readonly property bool ready: App.uqdUp
    /// The display being edited ("*" = all displays).
    property string selected: "*"
    /// "All displays", the connected screens, then saved entries for displays that aren't connected.
    readonly property var displays: {
        const list = [{ name: "*", title: "All displays", connected: true }];
        const seen = { "*": true };
        for (const s of Qt.application.screens) {
            if (!seen[s.name]) {
                seen[s.name] = true;
                list.push({ name: s.name, title: s.name || "Display " + list.length, connected: true });
            }
        }
        for (const name of Object.keys(outputs).sort()) {
            if (!seen[name]) {
                seen[name] = true;
                list.push({ name: name, title: name, connected: false });
            }
        }
        return list.map(d => Object.assign(d, { own: d.name !== "*" && outputs[d.name] !== undefined }));
    }
    readonly property bool hasOwn: outputs[selected] !== undefined
    /// The selected display's shape (the first screen's for All displays or one not connected).
    readonly property real aspect: {
        const screens = Qt.application.screens;
        const s = screens.find(x => x.name === page.selected) || screens[0];
        return s && s.height > 0 ? s.width / s.height : 16 / 9;
    }
    /// The theme's background, what the wallpaper shows without a colour of its own.
    readonly property var tokens: {
        try {
            return JSON.parse(App.themeJson || "{}");
        } catch (e) {
            return {};
        }
    }
    /// What the editor shows: the display's own entry, else the one for all displays.
    readonly property var shown: outputs[selected] || (selected !== "*" ? outputs["*"] : undefined) || {}

    Component.onCompleted: wm.refresh()

    WallpaperModel {
        id: wm
    }

    Connections {
        target: App
        function onUqdUpChanged() {
            if (App.uqdUp)
                wm.refresh();
        }
    }

    banners: [
        ErrorCard {
            objectName: "uqdDown"
            visible: !page.ready
            title: "uQ's settings service isn't running"
            detail: "Start the uQ desktop service to change these settings."
            Layout.fillWidth: true
        },
        ErrorCard {
            objectName: "changeError"
            visible: wm.error !== ""
            title: "That change wasn't applied"
            detail: wm.error
            Layout.fillWidth: true
        }
    ]

    SettingsTab {
        title: "Wallpaper"
        iconName: "preferences-desktop-wallpaper"

        SettingsCard {
            id: wallCard
            wide: true

            // The displays as chips: All displays, the connected screens, then saved ones that
            // aren't connected (dimmed). A dot marks a display with a wallpaper of its own.
            SettingRow {
                settingId: "wallpaper-display"
                highlight: App.highlight
                enabled: page.ready
                wideControls: true
                Layout.fillWidth: true

                RowLayout {
                    id: displayBox
                    objectName: "displayBox"
                    spacing: Kirigami.Units.smallSpacing
                    Layout.fillWidth: true
                    Accessible.role: Accessible.Grouping
                    Accessible.name: "Display"

                    QQC2.ButtonGroup {
                        id: displayGroup
                    }
                    Repeater {
                        model: page.displays
                        QQC2.Button {
                            required property var modelData
                            objectName: "display-" + modelData.name
                            checkable: true
                            checked: page.selected === modelData.name
                            QQC2.ButtonGroup.group: displayGroup
                            opacity: modelData.connected ? 1 : 0.6
                            Accessible.name: modelData.title + (modelData.connected ? "" : ", not connected") + (modelData.own ? ", own wallpaper" : "")
                            QQC2.ToolTip.visible: hovered && (modelData.own || !modelData.connected)
                            QQC2.ToolTip.text: !modelData.connected ? "Not connected" : "Has its own wallpaper"
                            contentItem: RowLayout {
                                spacing: Kirigami.Units.smallSpacing
                                Kirigami.Icon {
                                    source: modelData.name === "*" ? "view-grid" : "monitor"
                                    implicitWidth: Kirigami.Units.iconSizes.small
                                    implicitHeight: Kirigami.Units.iconSizes.small
                                    Accessible.ignored: true
                                }
                                UqLabel {
                                    text: modelData.title
                                    font.bold: page.selected === modelData.name
                                }
                                Rectangle {
                                    visible: modelData.own
                                    implicitWidth: Kirigami.Units.smallSpacing * 2
                                    implicitHeight: implicitWidth
                                    radius: implicitWidth / 2
                                    color: Kirigami.Theme.highlightColor
                                }
                            }
                            onClicked: {
                                page.selected = modelData.name;
                                checked = Qt.binding(() => page.selected === modelData.name);
                            }
                        }
                    }
                    Item {
                        Layout.fillWidth: true
                    }
                    QQC2.Button {
                        objectName: "removeEntry"
                        visible: page.hasOwn
                        enabled: page.ready
                        Accessible.name: page.selected === "*" ? "Remove the wallpaper" : "Use the All displays wallpaper"
                        contentItem: RowLayout {
                            spacing: Kirigami.Units.smallSpacing
                            Kirigami.Icon {
                                source: page.selected === "*" ? "edit-delete" : "edit-undo"
                                implicitWidth: Kirigami.Units.iconSizes.small
                                implicitHeight: Kirigami.Units.iconSizes.small
                                Accessible.ignored: true
                            }
                            UqLabel {
                                text: page.selected === "*" ? "Remove the wallpaper" : "Use the All displays wallpaper"
                            }
                        }
                        onClicked: wm.clearEntry(page.selected)
                    }
                }
            }
            RowLayout {
                visible: page.selected !== "*" && !page.hasOwn
                spacing: Kirigami.Units.smallSpacing
                Layout.fillWidth: true
                Layout.leftMargin: Kirigami.Units.largeSpacing
                Kirigami.Icon {
                    source: "documentinfo"
                    implicitWidth: Kirigami.Units.iconSizes.small
                    implicitHeight: Kirigami.Units.iconSizes.small
                    Accessible.ignored: true
                }
                UqLabel {
                    objectName: "inheritNote"
                    text: page.outputs["*"] ? "Shows the All displays wallpaper. A change here gives it its own." : "No wallpaper of its own yet. A change here gives it one."
                    wrapMode: Text.Wrap
                    opacity: 0.7
                    Layout.fillWidth: true
                }
            }

            WallpaperEntryEditor {
                objectName: "editor"
                output: page.selected
                entry: page.shown
                aspect: page.aspect
                themeBackground: (page.tokens.colors && page.tokens.colors.bg) || Kirigami.Theme.backgroundColor
                problem: page.problems[page.selected] || (page.selected !== "*" && !page.hasOwn ? page.problems["*"] || "" : "")
                highlight: App.highlight
                error: wm.error
                enabled: page.ready
                toPath: url => wm.localPath(url)
                onChanged: patch => wm.patchEntry(page.selected, JSON.stringify(patch))
                Layout.fillWidth: true
                Layout.topMargin: Kirigami.Units.smallSpacing
            }
        }
    }
}
