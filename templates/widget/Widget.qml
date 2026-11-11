// A uQ widget. `ctx` is set by the shell before the widget is shown (docs/integrating.md):
// ctx.Theme, ctx.UqText, ctx.UqInput, ctx.slot, ctx.dbus, ctx.command, ctx.openPopup(),
// ctx.closePopup() and ctx.fail(). Show every piece of text through ctx.UqText (plain text only);
// never use Text or Label, and don't import Quickshell.Io or the shell's own modules.
import QtQuick

Item {
    id: widget

    property var ctx
    property var status: null
    property string problem: ""

    // Optional: shown under the bar icon (bar slot) or under the tile (quick slot) when the
    // widget calls ctx.openPopup(). It gets the same `ctx`.
    property Component popup: Component {
        Item {
            property var ctx
            implicitWidth: 220
            implicitHeight: 40
            Loader {
                anchors.fill: parent
                sourceComponent: ctx ? ctx.UqText : null
                onLoaded: item.text = Qt.binding(function () {
                    return widget.problem !== "" ? widget.problem
                        : (widget.status ? "State: " + widget.status.state : "Checking…");
                })
            }
        }
    }

    function refresh() {
        // The callback gets {code: 0, output}, {code, error, output} or {error}.
        ctx.command.run("status", [], function (result) {
            widget.problem = result.error ? String(result.error) : "";
            widget.status = result.error ? null : result.output;
        });
    }

    Component.onCompleted: refresh()

    Timer {
        interval: 30000
        running: true
        repeat: true
        onTriggered: widget.refresh()
    }

    // The bar slot is one 26 px cell; a quick tile is a full-width 32 px row.
    Rectangle {
        anchors.fill: parent
        radius: widget.ctx ? widget.ctx.Theme.radius - 2 : 4
        color: mouse.containsMouse && widget.ctx ? widget.ctx.Theme.overlay : "transparent"
    }
    Loader {
        anchors.centerIn: parent
        sourceComponent: widget.ctx ? widget.ctx.UqText : null
        onLoaded: {
            item.text = Qt.binding(function () { return widget.problem !== "" ? "!" : "E"; });
        }
    }
    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        onClicked: widget.ctx.openPopup()
    }
}
