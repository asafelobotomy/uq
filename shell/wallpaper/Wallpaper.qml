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
    // ui.reduce_motion (Settings.reduceMotion): no crossfade.
    property bool reduceMotion: Settings.reduceMotion
    readonly property int fadeMs: 300

    readonly property var entry: W.entryFor(Settings.values.wallpaper, modelData.name)
    readonly property string folder: entry && typeof entry.folder === "string" ? entry.folder : ""
    readonly property string imagePath: entry && typeof entry.image === "string" ? entry.image : ""
    readonly property int fit: W.fillMode(entry ? entry.fit : undefined)
    readonly property bool slideshow: folder !== ""
    // One picture across screens (the "*" entry with fit "span"): the box it covers relative to
    // this screen, or null when this screen draws its own.
    readonly property var spanBox: W.spanGeometry(Quickshell.screens, modelData.name,
        W.spanNames(Settings.values.wallpaper, Quickshell.screens))
    readonly property bool spanning: spanBox !== null && entry !== null && entry.fit === "span"
    // Spanning slideshows follow the clock, so every screen shows the same slide.
    readonly property bool clocked: slideshow && spanning

    property var slideList: []
    property int slideIndex: 0
    // Slides that failed to load, by file name.
    property var badNames: ({})
    // The same as indices into slideList, for W.nextReadable.
    readonly property var errored: {
        var e = {};
        for (var i = 0; i < slideList.length; i++)
            if (badNames[slideList[i]])
                e[i] = true;
        return e;
    }
    // Every slide failed: show the colour.
    readonly property bool exhausted: slideList.length > 0 && W.nextReadable(0, slideList.length, errored) < 0

    readonly property string target: {
        if (slideshow)
            return !exhausted && slideIndex >= 0 && slideIndex < slideList.length
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

    // Re-read the folder. The current slide stays current when the list only grows or shrinks
    // elsewhere; the position resets only when it is gone.
    function rebuildSlides() {
        var names = [];
        for (var i = 0; i < folderModel.count; i++)
            names.push(folderModel.get(i, "fileName"));
        var list = W.slides(names, readableFormats);
        if (JSON.stringify(list) === JSON.stringify(slideList))
            return;
        var cur = slideList[slideIndex];
        var at = list.indexOf(cur);
        slideList = list;
        slideIndex = at >= 0 ? at : 0;
        syncSlide();
    }

    // Clocked slideshows: take the slide the clock says (the same on every spanning screen).
    function syncSlide() {
        if (!clocked)
            return;
        var j = W.clockIndex(Date.now(), W.intervalMs(entry) / 1000, slideList.length, errored);
        if (j >= 0)
            slideIndex = j;
    }

    onClockedChanged: syncSlide()

    function advance() {
        var j = W.nextReadable(slideIndex, slideList.length, errored);
        if (j >= 0)
            slideIndex = j;
    }

    // The slide on screen failed to load: mark it and move on at once.
    function slideFailed() {
        if (!slideshow)
            return;
        var b = Object.assign({}, badNames);
        b[slideList[slideIndex]] = true;
        badNames = b;
        if (clocked)
            syncSlide();
        else
            advance();
    }

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
        badNames = ({});
        slideIndex = 0;
    }

    Timer {
        interval: W.intervalMs(win.entry)
        repeat: true
        running: win.slideshow && !win.clocked && win.slideList.length > 1
        onTriggered: win.advance()
    }

    // The same slideshow on every spanning screen: wake on each interval boundary of the clock
    // (so on the minute for whole-minute intervals) and take the slide it names. `tick` makes
    // the interval be worked out again after every wake.
    property int tick: 0
    Timer {
        interval: {
            win.tick;
            return W.msToBoundary(Date.now(), W.intervalMs(win.entry) / 1000);
        }
        repeat: true
        running: win.clocked && win.slideList.length > 1
        onTriggered: {
            win.syncSlide();
            win.tick++;
        }
    }

    Crossfade {
        anchors.fill: parent
        target: win.target
        fit: win.fit
        span: win.spanning ? win.spanBox : null
        color: W.fallbackColor(win.entry, Theme.bg)
        reduceMotion: win.reduceMotion
        fadeMs: win.fadeMs
        // A slideshow skips a bad slide without flashing the colour.
        clearOnFail: !win.slideshow
        onFailed: win.slideFailed()
    }
}
