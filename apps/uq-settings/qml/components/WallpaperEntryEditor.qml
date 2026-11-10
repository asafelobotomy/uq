// The controls for one display's wallpaper entry: image, fit, slideshow folder, interval and
// colour. `entry` is what uqd has saved (or what this display inherits). Every field is reloaded
// only when the saved content changes, never on a refresh that brings the same content, so an
// edit in progress isn't clobbered; a refused change reloads them from the saved content.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Dialogs
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import "."

ColumnLayout {
    id: root

    /// The display these controls edit ("*" = all displays).
    property string output: "*"
    /// The saved entry: `{image, fit, folder, interval, color}`, each possibly missing or null.
    property var entry: ({})
    property string highlight
    /// uqd's last refusal; a new one puts the fields back to the saved values.
    property string error
    /// A function turning a dialog's file:// URL into a local path.
    property var toPath: url => String(url)
    /// The problem with this display's wallpaper, "" when it's fine.
    property string problem
    readonly property string savedKey: JSON.stringify([root.output, saved("image", null), saved("fit", "fill"), saved("folder", null), saved("interval", 300), saved("color", null)])
    readonly property alias imageField: imagePath
    readonly property alias folderField: folderPath
    readonly property alias colorField: colorText
    readonly property alias minutesBox: minutes
    readonly property alias secondsBox: seconds
    readonly property var fits: [
        { id: "fill", name: "Fill the screen" },
        { id: "fit", name: "Fit inside the screen" },
        { id: "center", name: "Centre" },
        { id: "tile", name: "Tile" }
    ]
    readonly property string imageFilter: "Images (*.jpg *.jpeg *.png *.webp *.jxl *.avif)"
    /// The whole new entry, for the model to save.
    signal changed(var entry)

    function saved(key, fallback) {
        const v = (entry || {})[key];
        return v === undefined || v === null ? fallback : v;
    }
    function commit(patch) {
        root.changed(Object.assign({
            image: root.saved("image", null),
            fit: root.saved("fit", "fill"),
            folder: root.saved("folder", null),
            interval: root.saved("interval", 300),
            color: root.saved("color", null)
        }, patch));
    }
    function fitIndex(id) {
        return Math.max(0, fits.findIndex(f => f.id === id));
    }
    function reload() {
        imagePath.text = saved("image", "");
        folderPath.text = saved("folder", "");
        fitBox.currentIndex = fitIndex(saved("fit", "fill"));
        const t = saved("interval", 300);
        minutes.value = Math.floor(t / 60);
        seconds.value = t % 60;
        colorText.text = saved("color", "");
    }
    /// The interval from the two boxes, clamped to 10 s to 24 h and shown back in them.
    function applyInterval() {
        const t = Math.min(86400, Math.max(10, minutes.value * 60 + seconds.value));
        minutes.value = Math.floor(t / 60);
        seconds.value = t % 60;
        if (t !== saved("interval", 300))
            commit({ interval: t });
    }
    function applyPath(key, text) {
        const v = text.trim();
        if (v !== saved(key, ""))
            commit(key === "image" ? { image: v === "" ? null : v } : { folder: v === "" ? null : v });
    }
    function applyColor() {
        const v = colorText.text.trim();
        if (v === "")
            commit({ color: null });
        else if (colorText.acceptableInput && v.toLowerCase() !== String(saved("color", "")).toLowerCase())
            commit({ color: v.toLowerCase() });
    }

    onSavedKeyChanged: reload()
    onErrorChanged: {
        if (error !== "")
            Qt.callLater(reload);
    }
    Component.onCompleted: reload()
    spacing: Kirigami.Units.smallSpacing

    FileDialog {
        id: imageDialog
        title: "Choose a wallpaper image"
        nameFilters: [root.imageFilter]
        onAccepted: {
            const p = root.toPath(selectedFile);
            if (p !== "")
                root.commit({ image: p });
        }
    }
    FolderDialog {
        id: folderDialog
        title: "Choose a slideshow folder"
        onAccepted: {
            const p = root.toPath(selectedFolder);
            if (p !== "")
                root.commit({ folder: p });
        }
    }

    SettingRow {
        settingId: "wallpaper-image"
        label: "Image"
        highlight: root.highlight
        enabled: root.enabled
        Layout.fillWidth: true
        QQC2.TextField {
            id: imagePath
            placeholderText: "No image"
            Accessible.name: "Image path"
            Layout.preferredWidth: Kirigami.Units.gridUnit * 16
            onEditingFinished: root.applyPath("image", text)
        }
        QQC2.Button {
            Accessible.name: "Choose an image"
            contentItem: UqLabel {
                text: "Choose…"
                padding: Kirigami.Units.smallSpacing
            }
            onClicked: imageDialog.open()
        }
        QQC2.Button {
            Accessible.name: "Clear the image"
            enabled: imagePath.text !== ""
            contentItem: UqLabel {
                text: "Clear"
                padding: Kirigami.Units.smallSpacing
            }
            onClicked: root.commit({ image: null })
        }
    }
    SettingRow {
        settingId: "wallpaper-fit"
        label: "Fit"
        highlight: root.highlight
        enabled: root.enabled
        Layout.fillWidth: true
        QQC2.ComboBox {
            id: fitBox
            objectName: "fitBox"
            model: root.fits.map(f => f.name)
            Accessible.name: "Fit"
            onActivated: index => {
                root.commit({ fit: root.fits[index].id });
                fitBox.currentIndex = Qt.binding(() => root.fitIndex(root.saved("fit", "fill")));
            }
        }
    }
    SettingRow {
        settingId: "wallpaper-slideshow"
        label: "Slideshow folder (replaces the image)"
        highlight: root.highlight
        enabled: root.enabled
        Layout.fillWidth: true
        QQC2.TextField {
            id: folderPath
            placeholderText: "No folder"
            Accessible.name: "Slideshow folder path"
            Layout.preferredWidth: Kirigami.Units.gridUnit * 16
            onEditingFinished: root.applyPath("folder", text)
        }
        QQC2.Button {
            Accessible.name: "Choose a folder"
            contentItem: UqLabel {
                text: "Choose…"
                padding: Kirigami.Units.smallSpacing
            }
            onClicked: folderDialog.open()
        }
        QQC2.Button {
            Accessible.name: "Clear the folder"
            enabled: folderPath.text !== ""
            contentItem: UqLabel {
                text: "Clear"
                padding: Kirigami.Units.smallSpacing
            }
            onClicked: root.commit({ folder: null })
        }
    }
    SettingRow {
        settingId: "wallpaper-interval"
        label: "Change the picture every"
        highlight: root.highlight
        enabled: root.enabled && folderPath.text !== ""
        Layout.fillWidth: true
        QQC2.SpinBox {
            id: minutes
            from: 0
            to: 1440
            editable: true
            Accessible.name: "Interval minutes"
            onValueModified: root.applyInterval()
        }
        UqLabel {
            text: "min"
        }
        QQC2.SpinBox {
            id: seconds
            from: 0
            to: 59
            editable: true
            Accessible.name: "Interval seconds"
            onValueModified: root.applyInterval()
        }
        UqLabel {
            text: "s"
        }
    }
    SettingRow {
        settingId: "wallpaper-color"
        label: "Background colour"
        highlight: root.highlight
        enabled: root.enabled
        Layout.fillWidth: true
        Rectangle {
            width: Kirigami.Units.gridUnit
            height: Kirigami.Units.gridUnit
            radius: Kirigami.Units.cornerRadius
            border.width: 1
            border.color: Kirigami.Theme.disabledTextColor
            color: colorText.acceptableInput ? colorText.text : "transparent"
        }
        QQC2.TextField {
            id: colorText
            placeholderText: "#rrggbb"
            maximumLength: 7
            validator: RegularExpressionValidator { regularExpression: /^#[0-9a-fA-F]{6}$/ }
            Accessible.name: "Background colour (#rrggbb)"
            Layout.preferredWidth: Kirigami.Units.gridUnit * 7
            onEditingFinished: root.applyColor()
        }
        QQC2.Button {
            Accessible.name: "Use the theme's background"
            enabled: colorText.text !== ""
            contentItem: UqLabel {
                text: "Use theme"
                padding: Kirigami.Units.smallSpacing
            }
            onClicked: root.commit({ color: null })
        }
    }
    UqLabel {
        objectName: "problem"
        visible: root.problem !== ""
        text: root.problem
        wrapMode: Text.Wrap
        color: Kirigami.Theme.negativeTextColor
        Layout.fillWidth: true
        Layout.leftMargin: Kirigami.Units.largeSpacing
    }
}
