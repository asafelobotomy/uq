import QtQuick

// A well-behaved test widget: shows a label through ctx.UqText and records what ctx delivers.
Item {
    id: w
    property var ctx
    property var lastProps: null
    property var lastSignal: null
    property var lastRun: null
    property Component popup: Component {
        Item {
            property var ctx
            width: 40
            height: 20
        }
    }

    function run() {
        ctx.command.run("status", ["a"], function (r) { w.lastRun = r; });
    }

    Loader {
        id: label
        sourceComponent: w.ctx ? w.ctx.UqText : null
        onLoaded: item.text = "ok"
    }
    Component.onCompleted: {
        ctx.dbus.onProps(function (p) { w.lastProps = p; });
        ctx.dbus.onSignal(function (m, a) { w.lastSignal = [m, a]; });
    }
}
