import QtQuick
import "logic.js" as Logic

// A page split over sibling files: logic.js (imported) and StatusRow.qml (a component).
Item {
    property string marker: Logic.describe({ running: true })
    readonly property alias row: row

    StatusRow {
        id: row
        summary: Logic.describe(null)
    }
}
