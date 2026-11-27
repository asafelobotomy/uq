// Open windows: their order.
import QtQuick
import QtQuick.Layouts
import Quickshell
import "../../core"
import "../../logic/barlayout.js" as L
import "../../logic/ticker.js" as T
import "../../logic/weather.js" as Weather
import "../../logic/work.js" as Work
import "../islands"

ColumnLayout {
    readonly property string mode: Work.SORTS.indexOf(pop.opts.sort) >= 0 ? pop.opts.sort : Work.DEFAULT_SORT
    id: sortBox
    // Passed in from IslandForm.qml.
    property var pop

    visible: pop.island === "work"
    spacing: 2
    Layout.fillWidth: true
    Layout.topMargin: Theme.gapIn
    UqText {
        text: "Order windows by"
        color: Theme.textMuted
    }
    Repeater {
        model: pop.island === "work" ? Work.SORTS : []
        delegate: MenuRow {
            required property string modelData
            Layout.fillWidth: true
            text: Work.SORT_NAMES[modelData]
            detail: sortBox.mode === modelData ? "✓" : ""
            Accessible.checkable: true
            Accessible.checked: sortBox.mode === modelData
            onActivated: BarLayout.setOptions("work", Object.assign({}, pop.opts, { sort: modelData }))
        }
    }
    UqText {
        visible: Work.sortHasOrder(sortBox.mode)
        text: "Order"
        color: Theme.textMuted
    }
    Repeater {
        model: pop.island === "work" && Work.sortHasOrder(sortBox.mode) ? [false, true] : []
        delegate: MenuRow {
            required property bool modelData
            Layout.fillWidth: true
            text: Work.ORDER_NAMES[sortBox.mode][modelData ? 1 : 0]
            detail: (pop.opts.descending === true) === modelData ? "✓" : ""
            onActivated: BarLayout.setOptions("work", Object.assign({}, pop.opts, { sort: sortBox.mode, descending: modelData }))
        }
    }
    UqText {
        visible: sortBox.mode === "activity"
        text: "The app that last sent a notification comes first."
        color: Theme.textMuted
        wrapMode: Text.WordWrap
        Layout.fillWidth: true
    }
}
