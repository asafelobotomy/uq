// One setting: a label on the left, its control(s) on the right. When `highlight` (bound to
// App.highlight by the page) becomes this row's `settingId`, the row flashes for 2 s.
import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Rectangle {
    id: row

    property string settingId
    property string label
    property string highlight
    readonly property bool flashing: flash.running
    default property alias content: slot.data

    function flashNow() {
        flash.restart();
    }
    function check() {
        if (highlight !== "" && highlight === settingId)
            flashNow();
    }

    implicitWidth: layout.implicitWidth + Kirigami.Units.largeSpacing * 2
    implicitHeight: layout.implicitHeight + Kirigami.Units.smallSpacing * 2
    radius: Kirigami.Units.cornerRadius
    color: flashing ? Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g,
                              Kirigami.Theme.highlightColor.b, 0.25) : "transparent"
    Behavior on color {
        ColorAnimation { duration: Kirigami.Units.shortDuration }
    }

    Accessible.role: Accessible.Grouping
    Accessible.name: label

    onHighlightChanged: check()
    Component.onCompleted: check()

    Timer {
        id: flash
        interval: 2000
    }

    RowLayout {
        id: layout
        anchors.fill: parent
        anchors.leftMargin: Kirigami.Units.largeSpacing
        anchors.rightMargin: Kirigami.Units.largeSpacing
        spacing: Kirigami.Units.largeSpacing

        UqLabel {
            text: row.label
            wrapMode: Text.Wrap
            Layout.fillWidth: true
        }
        RowLayout {
            id: slot
            spacing: Kirigami.Units.smallSpacing
        }
    }
}
