pragma Singleton
import QtQuick

// What each screen's wallpaper layer shows (shell/wallpaper/Wallpaper.qml publishes it), for
// what has to draw the same picture over a part of the screen: the strips at the screen's edges
// that cover a neighbouring page's window (shell/windows/ScreenPages.qml). Per screen name:
// {target (the image URL, "" for none), fit, span (the picture's box, as Crossfade takes it),
// color, backdrop (uQ's own backdrop is shown)}.
QtObject {
    id: root

    property var screens: ({})

    function set(name, state) {
        var s = Object.assign({}, root.screens);
        s[name] = state;
        root.screens = s;
    }
}
