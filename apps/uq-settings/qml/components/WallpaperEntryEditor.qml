// The controls for one display's wallpaper entry, beside a large preview of it: image, fit (each
// choice drawn with the picture), slideshow folder, interval and colour. `entry` is what uqd has saved (or what this display inherits). Every field is reloaded
// only when the saved content changes, never on a refresh that brings the same content, so an
// edit in progress isn't clobbered; a refused change reloads them from the saved content.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Dialogs
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import "."

GridLayout {
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
    /// The display's width over its height, for the preview's shape.
    property real aspect: 16 / 9
    /// The colour behind the picture when the entry has none: the theme's background.
    property color themeBackground: Kirigami.Theme.backgroundColor
    /// The picture on show in the preview: the image field's file, "" for none.
    readonly property string pictureUrl: imagePath.text.trim() === "" ? "" : "file://" + imagePath.text.trim().split("/").map(encodeURIComponent).join("/")
    readonly property color shownColor: colorText.acceptableInput ? colorText.text : root.themeBackground
    readonly property string shownFit: root.fits[fitBox.currentIndex] ? root.fits[fitBox.currentIndex].id : "fill"
    readonly property string savedKey: JSON.stringify([root.output, saved("image", null), saved("fit", "fill"), saved("folder", null), saved("interval", 300), saved("color", null)])
    readonly property alias imageField: imagePath
    readonly property alias folderField: folderPath
    readonly property alias colorField: colorText
    readonly property alias minutesBox: minutes
    readonly property alias secondsBox: seconds
    readonly property var baseFits: [
        { id: "fill", name: "Fill the screen", short: "Fill" },
        { id: "fit", name: "Fit inside the screen", short: "Fit" },
        { id: "center", name: "Centre", short: "Centre" },
        { id: "tile", name: "Tile", short: "Tile" }
    ]
    /// "Span across displays" is only for All displays: the other screens draw their own picture.
    readonly property var fits: output === "*"
        ? baseFits.concat([{ id: "span", name: "Span across displays", short: "Span" }]) : baseFits
    readonly property alias fitField: fitBox
    readonly property string imageFilter: "Images (*.jpg *.jpeg *.png *.webp *.jxl *.avif)"
    /// The fields to change, e.g. `{fit: "tile"}`.
    signal changed(var patch)

    function saved(key, fallback) {
        const v = (entry || {})[key];
        return v === undefined || v === null ? fallback : v;
    }
    /// Only the changed fields: the model merges them into the entry uqd has at that moment.
    function commit(patch) {
        root.changed(patch);
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

    // A new model (All displays gains "Span across displays") resets the combo's index.
    onFitsChanged: Qt.callLater(() => fitBox.currentIndex = fitIndex(saved("fit", "fill")))
    onSavedKeyChanged: reload()
    onErrorChanged: {
        if (error !== "")
            Qt.callLater(reload);
    }
    Component.onCompleted: reload()
    columns: root.width >= Kirigami.Units.gridUnit * 46 ? 2 : 1
    columnSpacing: Kirigami.Units.largeSpacing * 2
    rowSpacing: Kirigami.Units.largeSpacing

    // The picture drawn as a fit would show it on a screen of this shape: cropped to fill, whole
    // inside, small in the middle, repeated, or (span) cropped like fill.
    component FitPicture: Rectangle {
        id: pic
        property string fit: "fill"
        property string url: ""
        /// The colour behind the picture.
        property color backdrop: "transparent"
        /// The picture couldn't be opened (a moved or unreadable file).
        readonly property bool failed: one.status === Image.Error
        color: pic.backdrop
        clip: true
        Accessible.ignored: true

        Image {
            id: one
            visible: pic.fit !== "tile" && pic.url !== ""
            source: pic.url
            asynchronous: true
            sourceSize.width: Kirigami.Units.gridUnit * 40
            fillMode: pic.fit === "fit" || pic.fit === "center" ? Image.PreserveAspectFit : Image.PreserveAspectCrop
            anchors.centerIn: parent
            width: pic.fit === "center" ? parent.width * 0.5 : parent.width
            height: pic.fit === "center" ? parent.height * 0.5 : parent.height
        }
        Grid {
            visible: pic.fit === "tile" && pic.url !== ""
            columns: 3
            Repeater {
                model: pic.fit === "tile" && pic.url !== "" ? 9 : 0
                Image {
                    source: pic.url
                    asynchronous: true
                    sourceSize.width: Kirigami.Units.gridUnit * 40
                    fillMode: Image.PreserveAspectCrop
                    width: pic.width / 3
                    height: pic.height / 3
                }
            }
        }
    }

    ColorDialog {
        id: colorDialog
        title: "Background colour"
        selectedColor: root.shownColor
        onAccepted: {
            colorText.text = String(selectedColor).substring(0, 7);
            root.applyColor();
        }
    }

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

    // The preview: the display's shape with the picture as the chosen fit draws it.
    ColumnLayout {
        spacing: Kirigami.Units.smallSpacing
        Layout.alignment: Qt.AlignTop
        Layout.preferredWidth: Kirigami.Units.gridUnit * 24
        Layout.fillWidth: root.columns === 1

        FitPicture {
            id: preview
            objectName: "preview"
            fit: root.shownFit
            backdrop: root.shownColor
            url: folderPath.text.trim() === "" ? root.pictureUrl : ""
            radius: Kirigami.Units.cornerRadius
            border.width: 1
            border.color: Qt.rgba(Kirigami.Theme.textColor.r, Kirigami.Theme.textColor.g, Kirigami.Theme.textColor.b, 0.2)
            Layout.fillWidth: true
            Layout.preferredHeight: width / Math.max(0.2, root.aspect)
            Accessible.role: Accessible.Graphic
            Accessible.name: "Wallpaper preview"
            Accessible.ignored: false

            // A slideshow, nothing at all, or a picture that can't be opened: say which, on a
            // plate in the window colours so it reads on any wallpaper colour.
            Rectangle {
                anchors.centerIn: parent
                visible: preview.url === "" || preview.failed
                color: Kirigami.Theme.backgroundColor
                opacity: 0.9
                radius: Kirigami.Units.cornerRadius
                implicitWidth: plate.implicitWidth + Kirigami.Units.largeSpacing * 2
                implicitHeight: plate.implicitHeight + Kirigami.Units.largeSpacing * 2
                ColumnLayout {
                    id: plate
                    anchors.centerIn: parent
                    spacing: Kirigami.Units.smallSpacing
                    Kirigami.Icon {
                        source: preview.failed ? "image-missing" : folderPath.text.trim() !== "" ? "folder-pictures" : "preferences-desktop-wallpaper"
                        implicitWidth: Kirigami.Units.iconSizes.large
                        implicitHeight: Kirigami.Units.iconSizes.large
                        Layout.alignment: Qt.AlignHCenter
                    }
                    UqLabel {
                        objectName: "previewCaption"
                        text: preview.failed ? "Can't open the picture" : folderPath.text.trim() !== "" ? "Slideshow" : "Colour only"
                        Layout.alignment: Qt.AlignHCenter
                    }
                }
            }
        }
        RowLayout {
            visible: root.problem !== ""
            spacing: Kirigami.Units.smallSpacing
            Layout.fillWidth: true
            Kirigami.Icon {
                source: "dialog-warning"
                implicitWidth: Kirigami.Units.iconSizes.small
                implicitHeight: Kirigami.Units.iconSizes.small
                Layout.alignment: Qt.AlignTop
                Accessible.ignored: true
            }
            UqLabel {
                objectName: "problem"
                text: root.problem
                wrapMode: Text.Wrap
                color: Kirigami.Theme.negativeTextColor
                Layout.fillWidth: true
            }
        }
    }

    ColumnLayout {
        spacing: Kirigami.Units.smallSpacing
        Layout.alignment: Qt.AlignTop
        Layout.fillWidth: true

        SettingRow {
            settingId: "wallpaper-image"
            iconName: "image-x-generic"
            label: "Image"
            highlight: root.highlight
            enabled: root.enabled
            Layout.fillWidth: true
            QQC2.TextField {
                id: imagePath
                placeholderText: "No image"
                Accessible.name: "Image path"
                Layout.preferredWidth: Kirigami.Units.gridUnit * 14
                onEditingFinished: root.applyPath("image", text)
            }
            QQC2.Button {
                text: "Choose…"
                icon.name: "document-open"
                Accessible.name: "Choose an image"
                onClicked: imageDialog.open()
            }
            QQC2.ToolButton {
                text: "Clear"
                icon.name: "edit-clear"
                display: QQC2.AbstractButton.IconOnly
                Accessible.name: "Clear the image"
                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.text: "Clear the image"
                enabled: imagePath.text !== ""
                onClicked: root.commit({ image: null })
            }
        }
        // Each fit drawn with the picture: pick the one that looks right.
        SettingRow {
            settingId: "wallpaper-fit"
            iconName: "zoom-fit-best"
            label: "Fit"
            highlight: root.highlight
            enabled: root.enabled
            Layout.fillWidth: true

            RowLayout {
                id: fitBox
                objectName: "fitBox"
                /// The fits' names, in order; `currentIndex` is the one in use.
                readonly property var model: root.fits.map(f => f.name)
                property int currentIndex: 0
                spacing: Kirigami.Units.smallSpacing
                Accessible.role: Accessible.Grouping
                Accessible.name: "Fit"

                QQC2.ButtonGroup {
                    id: fitGroup
                }
                Repeater {
                    model: root.fits
                    QQC2.Button {
                        required property var modelData
                        required property int index
                        objectName: "fit-" + modelData.id
                        checkable: true
                        checked: fitBox.currentIndex === index
                        QQC2.ButtonGroup.group: fitGroup
                        Accessible.name: modelData.name
                        QQC2.ToolTip.visible: hovered
                        QQC2.ToolTip.text: modelData.name
                        contentItem: ColumnLayout {
                            spacing: Kirigami.Units.smallSpacing
                            FitPicture {
                                fit: modelData.id
                                url: root.pictureUrl
                                backdrop: root.shownColor
                                radius: Kirigami.Units.cornerRadius / 2
                                implicitWidth: Kirigami.Units.gridUnit * 3
                                implicitHeight: implicitWidth / Math.max(0.2, root.aspect)
                                Layout.alignment: Qt.AlignHCenter
                                Rectangle {
                                    visible: root.pictureUrl === ""
                                    anchors.centerIn: parent
                                    width: Kirigami.Units.iconSizes.small + Kirigami.Units.smallSpacing * 2
                                    height: width
                                    radius: width / 2
                                    color: Kirigami.Theme.backgroundColor
                                }
                                Kirigami.Icon {
                                    visible: root.pictureUrl === ""
                                    source: ["zoom-fit-best", "zoom-fit-page", "zoom-original", "view-grid", "view-split-left-right"][index] || ""
                                    anchors.centerIn: parent
                                    implicitWidth: Kirigami.Units.iconSizes.small
                                    implicitHeight: Kirigami.Units.iconSizes.small
                                }
                            }
                            UqLabel {
                                text: modelData.short
                                font.pointSize: Kirigami.Theme.smallFont.pointSize
                                horizontalAlignment: Text.AlignHCenter
                                Layout.fillWidth: true
                            }
                        }
                        onClicked: {
                            root.commit({ fit: modelData.id });
                            fitBox.currentIndex = Qt.binding(() => root.fitIndex(root.saved("fit", "fill")));
                        }
                    }
                }
            }
        }
        SettingRow {
            settingId: "wallpaper-color"
            iconName: "fill-color"
            label: "Background colour"
            help: "Shown around a picture that doesn't fill the screen, or alone without one."
            highlight: root.highlight
            enabled: root.enabled
            Layout.fillWidth: true
            QQC2.Button {
                objectName: "colorSwatch"
                Accessible.name: "Pick a background colour"
                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.text: "Pick a colour"
                onClicked: colorDialog.open()
                contentItem: Rectangle {
                    implicitWidth: Kirigami.Units.gridUnit * 2
                    implicitHeight: Kirigami.Units.gridUnit
                    radius: Kirigami.Units.cornerRadius
                    border.width: 1
                    border.color: Kirigami.Theme.disabledTextColor
                    color: root.shownColor
                }
            }
            QQC2.TextField {
                id: colorText
                placeholderText: "Theme"
                maximumLength: 7
                validator: RegularExpressionValidator { regularExpression: /^#[0-9a-fA-F]{6}$/ }
                Accessible.name: "Background colour (#rrggbb)"
                Layout.preferredWidth: Kirigami.Units.gridUnit * 5
                onEditingFinished: root.applyColor()
            }
            QQC2.ToolButton {
                text: "Use theme"
                icon.name: "edit-undo"
                display: QQC2.AbstractButton.IconOnly
                Accessible.name: "Use the theme's background"
                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.text: "Use the theme's background"
                enabled: colorText.text !== ""
                onClicked: root.commit({ color: null })
            }
        }
        Kirigami.Separator {
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.smallSpacing
            Layout.bottomMargin: Kirigami.Units.smallSpacing
        }
        SettingRow {
            settingId: "wallpaper-slideshow"
            iconName: "folder-pictures"
            label: "Slideshow folder"
            help: "Shows the folder's pictures in turn, in place of the image."
            highlight: root.highlight
            enabled: root.enabled
            Layout.fillWidth: true
            QQC2.TextField {
                id: folderPath
                placeholderText: "No folder"
                Accessible.name: "Slideshow folder path"
                Layout.preferredWidth: Kirigami.Units.gridUnit * 14
                onEditingFinished: root.applyPath("folder", text)
            }
            QQC2.Button {
                text: "Choose…"
                icon.name: "folder-open"
                Accessible.name: "Choose a folder"
                onClicked: folderDialog.open()
            }
            QQC2.ToolButton {
                text: "Clear"
                icon.name: "edit-clear"
                display: QQC2.AbstractButton.IconOnly
                Accessible.name: "Clear the folder"
                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.text: "Clear the folder"
                enabled: folderPath.text !== ""
                onClicked: root.commit({ folder: null })
            }
        }
        SettingRow {
            settingId: "wallpaper-interval"
            iconName: "media-playlist-shuffle"
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
    }
}
