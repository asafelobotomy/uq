// Wallpaper: an image or a folder slideshow, how it fits, and the background colour, for all
// displays or for one. Everything is read from and written to uqd through WallpaperModel; a
// refused change shows uqd's message and the controls go back to the last good value.
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
        const list = [{ name: "*", title: "All displays" }];
        const seen = { "*": true };
        for (const s of Qt.application.screens) {
            if (!seen[s.name]) {
                seen[s.name] = true;
                list.push({ name: s.name, title: s.name });
            }
        }
        for (const name of Object.keys(outputs).sort()) {
            if (!seen[name]) {
                seen[name] = true;
                list.push({ name: name, title: name + " (not connected)" });
            }
        }
        return list.map(d => ({
            name: d.name,
            title: d.name !== "*" && outputs[d.name] ? d.title + " — own wallpaper" : d.title
        }));
    }
    readonly property bool hasOwn: outputs[selected] !== undefined
    /// What the editor shows: the display's own entry, else the one for all displays.
    readonly property var shown: outputs[selected] || (selected !== "*" ? outputs["*"] : undefined) || {}

    Component.onCompleted: wm.refresh()
    Accessible.role: Accessible.Pane
    Accessible.name: "Wallpaper"
    contentWidth: availableWidth

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

    ColumnLayout {
        id: column
        width: Math.min(page.availableWidth - Kirigami.Units.largeSpacing * 2, Kirigami.Units.gridUnit * 40)
        x: Math.max(Kirigami.Units.largeSpacing, (page.availableWidth - width) / 2)
        spacing: Kirigami.Units.smallSpacing

        Item {
            implicitHeight: Kirigami.Units.smallSpacing
        }
        UqLabel {
            text: "Wallpaper"
            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.5
            font.bold: true
            Accessible.role: Accessible.Heading
        }
        ErrorCard {
            objectName: "uqdDown"
            visible: !page.ready
            title: "uQ's settings service isn't running"
            detail: "Start uqd to change these settings."
            Layout.fillWidth: true
        }
        ErrorCard {
            objectName: "changeError"
            visible: wm.error !== ""
            title: "That change wasn't applied"
            detail: wm.error
            Layout.fillWidth: true
        }

        SettingRow {
            settingId: "wallpaper-display"
            label: "Display"
            enabled: page.ready
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.largeSpacing
            QQC2.ComboBox {
                id: displayBox
                objectName: "displayBox"
                model: page.displays.map(d => d.title)
                currentIndex: Math.max(0, page.displays.findIndex(d => d.name === page.selected))
                Accessible.name: "Display"
                Layout.preferredWidth: Kirigami.Units.gridUnit * 16
                onActivated: index => {
                    page.selected = page.displays[index].name;
                    displayBox.currentIndex = Qt.binding(() => Math.max(0, page.displays.findIndex(d => d.name === page.selected)));
                }
            }
        }
        UqLabel {
            visible: page.selected !== "*" && !page.hasOwn
            text: page.outputs["*"] ? "This display shows the All displays wallpaper. Changing anything here gives it its own." : "This display has no wallpaper of its own. Changing anything here gives it one."
            wrapMode: Text.Wrap
            color: Kirigami.Theme.disabledTextColor
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing
        }

        WallpaperEntryEditor {
            objectName: "editor"
            output: page.selected
            entry: page.shown
            problem: page.problems[page.selected] || ""
            highlight: App.highlight
            error: wm.error
            enabled: page.ready
            toPath: url => wm.localPath(url)
            onChanged: entry => wm.setEntry(page.selected, JSON.stringify(entry))
            Layout.fillWidth: true
        }

        QQC2.Button {
            objectName: "removeEntry"
            visible: page.hasOwn
            enabled: page.ready
            Accessible.name: page.selected === "*" ? "Remove the wallpaper" : "Use the All displays wallpaper"
            contentItem: UqLabel {
                text: page.selected === "*" ? "Remove the wallpaper" : "Use the All displays wallpaper"
                padding: Kirigami.Units.smallSpacing
            }
            Layout.leftMargin: Kirigami.Units.largeSpacing
            onClicked: wm.clearEntry(page.selected)
        }
        Item {
            implicitHeight: Kirigami.Units.largeSpacing
        }
    }
}
