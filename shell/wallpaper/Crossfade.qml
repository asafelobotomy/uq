import QtQuick

// The wallpaper's image stack: a colour with two Images over it. Setting `target` (a file URL, or
// "" for none) loads it into the idle Image and crossfades it in once loaded; until then the
// previous image stays visible. A target that fails to load emits failed(), and (clearOnFail)
// drops the old image so the colour shows. Kept apart from the PanelWindow so it can be tested.
Item {
    id: root

    property string target: ""
    property int fit: Image.PreserveAspectCrop
    property color color: "black"
    property bool reduceMotion: false
    property int fadeMs: 300
    // False while a slideshow is skipping bad slides: keep the old image until the next loads.
    property bool clearOnFail: true
    // One picture across several screens: {x, y, width, height}, the box the image covers in this
    // item's coordinates (the item clips it). null: the image fills the item.
    property var span: null
    readonly property real boxX: span ? span.x : 0
    readonly property real boxY: span ? span.y : 0
    readonly property real boxWidth: span ? span.width : width
    readonly property real boxHeight: span ? span.height : height

    clip: true

    signal failed

    // The image on top (or null) and the target it was loaded for.
    property Image shown: null
    readonly property string shownTarget: shown ? shown.want : ""
    readonly property Image idle: shown === imgA ? imgB : imgA

    function show() {
        // A fade in progress finishes first (onFinished calls this again), so a change in the
        // middle of a crossfade never pops.
        if (fade.running)
            return;
        if (target === "") {
            clear();
            return;
        }
        if (shown && shown.want === target)
            return;
        var h = idle;
        h.opacity = 0;
        h.z = 1;
        if (shown)
            shown.z = 0;
        // Setting a source an Image already holds gives no status change, so a Ready one is
        // used as it is and anything else is reset first.
        if (h.want === target && h.status === Image.Ready) {
            loaded(h);
            return;
        }
        h.want = target;
        h.source = "";
        h.source = target;
    }

    function clear() {
        fade.stop();
        for (var img of [imgA, imgB]) {
            img.opacity = 0;
            img.want = "";
            img.source = "";
        }
        shown = null;
    }

    function loaded(img) {
        if (img.want !== target || img === shown)
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

    function errored(img) {
        if (img.want !== target)
            return;
        if (clearOnFail)
            clear();
        failed();
    }

    onTargetChanged: show()

    Rectangle {
        anchors.fill: parent
        color: root.color
    }

    NumberAnimation {
        id: fade

        property Image old: null

        property: "opacity"
        from: 0
        to: 1
        duration: root.fadeMs
        easing.type: Easing.InOutQuad
        onFinished: {
            if (old)
                old.opacity = 0;
            root.show();
        }
    }

    component Slide: Image {
        property string want: ""

        x: root.boxX
        y: root.boxY
        width: root.boxWidth
        height: root.boxHeight
        asynchronous: true
        cache: false
        opacity: 0
        fillMode: root.fit
        // Fill and fit decode at screen size; center and tile need the real size.
        sourceSize: root.fit === Image.PreserveAspectCrop || root.fit === Image.PreserveAspectFit
            ? Qt.size(root.boxWidth, root.boxHeight) : Qt.size(0, 0)
        onStatusChanged: {
            if (status === Image.Ready)
                root.loaded(this);
            else if (status === Image.Error)
                root.errored(this);
        }
    }

    Slide { id: imgA }
    Slide { id: imgB }
}
