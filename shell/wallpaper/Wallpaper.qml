pragma ComponentBehavior: Bound
import QtQuick
import Qt.labs.folderlistmodel
import Quickshell
import Quickshell.Wayland
import "../core"
import "../logic/wallpaper.js" as W

// One wallpaper layer per screen (spec §2.3): a background-layer window with a colour, and one
// image or a folder slideshow from Settings.values.wallpaper (this output's entry, else "*").
// It takes no input and reserves no space. An image that fails to load stays transparent, so the
// colour shows; a slide that fails is skipped at once.
PanelWindow {
    id: win

    required property var modelData

    // The formats Qt is expected to read. QML can't list the installed image plugins, so a slide
    // of a format without one (jxl, avif) errors and is skipped.
    readonly property var readableFormats: ["jpg", "jpeg", "png", "webp", "jxl", "avif"]
    // ui.reduce_motion: no crossfade. Theme has no such token yet, so this stays false (as Dim.qml).
    property bool reduceMotion: false
    readonly property int fadeMs: 300

    readonly property var entry: W.entryFor(Settings.values.wallpaper, modelData.name)
    readonly property string folder: entry && typeof entry.folder === "string" ? entry.folder : ""
    readonly property string imagePath: entry && typeof entry.image === "string" ? entry.image : ""
    readonly property int fit: W.fillMode(entry ? entry.fit : undefined)
    readonly property bool slideshow: folder !== ""

    property var slideList: []
    property int slideIndex: 0
    property var errored: ({})

    readonly property string target: {
        if (slideshow)
            return slideList.length > 0 && slideIndex >= 0 && slideIndex < slideList.length
                ? W.fileUrl(folder + "/" + slideList[slideIndex]) : "";
        return imagePath !== "" ? W.fileUrl(imagePath) : "";
    }

    screen: modelData
    color: "transparent"
    WlrLayershell.layer: WlrLayer.Background
    WlrLayershell.namespace: "uq-wallpaper"
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
    exclusionMode: ExclusionMode.Ignore
    // An empty region: clicks and keys go to whatever is above this layer.
    mask: Region {}
    anchors { top: true; bottom: true; left: true; right: true }

    function rebuildSlides() {
        var names = [];
        for (var i = 0; i < folderModel.count; i++)
            names.push(folderModel.get(i, "fileName"));
        slideList = W.slides(names, readableFormats);
        errored = ({});
        slideIndex = 0;
    }

    function advance() {
        var j = W.nextReadable(slideIndex, slideList.length, errored);
        if (j >= 0)
            slideIndex = j;
    }

    // The image on top (or none) and the one waiting to load the next source.
    property Image shown: null
    readonly property Image hidden: shown === imgA ? imgB : imgA

    function showTarget() {
        fade.stop();
        if (shown)
            shown.opacity = 1;
        if (target === "") {
            clearImages();
            return;
        }
        if (shown && shown.source.toString() === target)
            return;
        hidden.opacity = 0;
        hidden.z = 1;
        if (shown)
            shown.z = 0;
        hidden.source = target;
    }

    function clearImages() {
        imgA.opacity = 0;
        imgB.opacity = 0;
        shown = null;
    }

    function imageLoaded(img) {
        if (img === shown || img.source.toString() !== target)
            return;
        var old = shown;
        shown = img;
        if (reduceMotion || !old) {
            img.opacity = 1;
            if (old)
                old.opacity = 0;
        } else {
            fade.target = img;
            fade.old = old;
            fade.restart();
        }
    }

    function imageFailed(img) {
        if (img.source.toString() !== target)
            return;
        if (slideshow) {
            var e = errored;
            e[slideIndex] = true;
            errored = e;
            var j = W.nextReadable(slideIndex, slideList.length, errored);
            if (j >= 0 && j !== slideIndex) {
                slideIndex = j;
                return;
            }
        }
        // Single image, or every slide failed: show the colour.
        clearImages();
    }

    onTargetChanged: showTarget()
    onFitChanged: if (shown) shown.opacity = 1

    FolderListModel {
        id: folderModel

        folder: win.slideshow ? W.fileUrl(win.folder) : ""
        showDirs: false
        nameFilters: win.readableFormats.map(function (f) { return "*." + f; })
        onStatusChanged: if (status === FolderListModel.Ready) win.rebuildSlides()
        onCountChanged: if (status === FolderListModel.Ready) win.rebuildSlides()
    }

    onFolderChanged: {
        slideList = [];
        errored = ({});
        slideIndex = 0;
    }

    Timer {
        interval: W.intervalMs(win.entry)
        repeat: true
        running: win.slideshow && win.slideList.length > 1
        onTriggered: win.advance()
    }

    Rectangle {
        anchors.fill: parent
        color: W.fallbackColor(win.entry, Theme.bg)
    }

    NumberAnimation {
        id: fade

        property Image old: null

        property: "opacity"
        from: 0
        to: 1
        duration: win.fadeMs
        easing.type: Easing.InOutQuad
        onFinished: if (old) old.opacity = 0
    }

    component Slide: Image {
        anchors.fill: parent
        asynchronous: true
        cache: false
        opacity: 0
        fillMode: win.fit
        // Fill and fit decode at screen size; center and tile need the real size.
        sourceSize: win.fit === Image.PreserveAspectCrop || win.fit === Image.PreserveAspectFit
            ? Qt.size(win.width, win.height) : Qt.size(0, 0)
        onStatusChanged: {
            if (status === Image.Ready)
                win.imageLoaded(this);
            else if (status === Image.Error)
                win.imageFailed(this);
        }
    }

    Slide { id: imgA }
    Slide { id: imgB }
}
