// A uQ Settings page. `ctx` is set by Settings: ctx.theme, ctx.command, ctx.dbus and
// ctx.openApp (docs/settings-pages.md). All text goes through UqLabel (plain text only); pure
// logic lives in logic.js beside this file.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.uq.settings
import "logic.js" as Logic

Item {
    id: page

    property var ctx
    /// The search row to flash (from [[setting]] ids).
    property string highlight
    property var status: null
    property string problem

    function refresh() {
        if (!page.ctx)
            return;
        page.ctx.command.run("status", [], result => {
            page.problem = Logic.problemOf(result);
            if (page.problem === "")
                page.status = result.output;
        });
    }

    onCtxChanged: refresh()

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 16
        spacing: 12

        UqLabel {
            text: Logic.describe(page.status)
            Layout.fillWidth: true
        }
        UqLabel {
            visible: page.problem !== ""
            text: page.problem
            color: (page.ctx && page.ctx.theme.colors && page.ctx.theme.colors.error) || "red"
            wrapMode: Text.Wrap
            Layout.fillWidth: true
        }
        QQC2.Button {
            text: "Forget the last item"
            enabled: page.status !== null && (page.status.last || "") !== ""
            // Settings asks "Forget <item>? This can't be undone." first; runs only on Confirm.
            onClicked: page.ctx.command.run("forget", [page.status.last], () => page.refresh())
        }
        Item {
            Layout.fillHeight: true
        }
    }
}
