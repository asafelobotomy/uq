// The displays as rectangles, scaled to fit, each draggable. Dropping one emits `moved` with the
// spot in the displays' own (logical pixel) coordinates; the page model snaps it to its
// neighbours. A click selects a display. Names are plain text (UqLabel).
import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

Item {
    id: area

    /// The page model's `view`: [{name, label, rect: {x, y, w, h}, enabled, mirror_of}].
    property var outputs: []
    property string selected: ""
    /// Preview pixels per display pixel; 0 fits the layout into the item.
    property real fixedScale: 0
    readonly property var shown: outputs.filter(o => o.enabled && !o.mirror_of)
    readonly property real minX: shown.length ? Math.min(...shown.map(o => o.rect.x)) : 0
    readonly property real minY: shown.length ? Math.min(...shown.map(o => o.rect.y)) : 0
    readonly property real spanW: shown.length ? Math.max(...shown.map(o => o.rect.x + o.rect.w)) - minX : 1
    readonly property real spanH: shown.length ? Math.max(...shown.map(o => o.rect.y + o.rect.h)) - minY : 1
    readonly property real margin: Kirigami.Units.gridUnit
    readonly property real previewScale: fixedScale > 0 ? fixedScale : Math.max(0.01, Math.min((width - margin * 2) / Math.max(1, spanW), (height - margin * 2) / Math.max(1, spanH)) * 0.8)
    readonly property real offsetX: (width - spanW * previewScale) / 2
    readonly property real offsetY: (height - spanH * previewScale) / 2

    signal moved(string name, int x, int y)
    signal picked(string name)

    implicitHeight: Kirigami.Units.gridUnit * 14
    Accessible.role: Accessible.Pane
    Accessible.name: "Display arrangement"

    Repeater {
        id: rects
        model: area.shown

        Rectangle {
            id: box
            required property var modelData
            readonly property bool isSelected: modelData.name === area.selected
            objectName: "display-" + modelData.name
            x: area.offsetX + (modelData.rect.x - area.minX) * area.previewScale
            y: area.offsetY + (modelData.rect.y - area.minY) * area.previewScale
            width: Math.max(Kirigami.Units.smallSpacing * 2, modelData.rect.w * area.previewScale)
            height: Math.max(Kirigami.Units.smallSpacing * 2, modelData.rect.h * area.previewScale)
            radius: Kirigami.Units.cornerRadius
            // Kirigami's colours throughout, like the label inside: mixing them with the Qt
            // palette put text of one scheme on a box of the other.
            Kirigami.Theme.colorSet: Kirigami.Theme.Button
            Kirigami.Theme.inherit: false
            color: isSelected ? Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.35) : Kirigami.Theme.backgroundColor
            border.width: isSelected ? 2 : 1
            border.color: isSelected ? Kirigami.Theme.highlightColor : Kirigami.ColorUtils.linearInterpolation(Kirigami.Theme.backgroundColor, Kirigami.Theme.textColor, 0.3)
            Accessible.role: Accessible.Button
            Accessible.name: modelData.label

            Column {
                anchors.centerIn: parent
                width: parent.width - Kirigami.Units.smallSpacing * 2
                UqLabel {
                    width: parent.width
                    horizontalAlignment: Text.AlignHCenter
                    elide: Text.ElideRight
                    font.bold: box.isSelected
                    text: box.modelData.label
                }
                // The size it takes up, so a scaled screen reads as smaller.
                UqLabel {
                    objectName: "displaySize"
                    visible: box.height > Kirigami.Units.gridUnit * 3
                    width: parent.width
                    horizontalAlignment: Text.AlignHCenter
                    elide: Text.ElideRight
                    opacity: 0.7
                    text: box.modelData.rect.w + " × " + box.modelData.rect.h
                }
            }

            MouseArea {
                anchors.fill: parent
                drag.target: box
                cursorShape: Qt.SizeAllCursor
                onPressed: area.picked(box.modelData.name)
                onReleased: {
                    if (!drag.active && box.x === area.offsetX + (box.modelData.rect.x - area.minX) * area.previewScale)
                        return;
                    const gx = Math.round((box.x - area.offsetX) / area.previewScale + area.minX);
                    const gy = Math.round((box.y - area.offsetY) / area.previewScale + area.minY);
                    // Snap back to the model's place until it answers.
                    box.x = Qt.binding(() => area.offsetX + (box.modelData.rect.x - area.minX) * area.previewScale);
                    box.y = Qt.binding(() => area.offsetY + (box.modelData.rect.y - area.minY) * area.previewScale);
                    area.moved(box.modelData.name, gx, gy);
                }
            }
        }
    }
}
