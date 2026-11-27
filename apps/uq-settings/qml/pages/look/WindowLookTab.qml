// Look's Windows tab: gaps, borders, rounding and how new windows open.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../../components"

SettingsTab {
    // Passed in from LookPage.qml.
    property var lm
    property var page

    title: "Windows"
    iconName: "window"

    SettingsCard {
        title: "Gaps and borders"
        iconName: "distribute-horizontal-x"

        PixelSlider {
            id: gapsIn
            busy: lm.busy
            settingId: "gaps-inner"
            iconName: "view-split-left-right"
            source: page.sourceOf("gaps-inner")
            onReapply: lm.reapply("look")
            label: "Between windows"
            enabled: page.ready
            from: 0
            to: 50
            fallback: page.num(page.ui.gap_in)
            value: page.saved(page.look, "gaps_in")
            onChosen: v => page.setLook("gaps_in", v)
        }
        PixelSlider {
            id: gapsOut
            busy: lm.busy
            settingId: "gaps-outer"
            iconName: "view-fullscreen"
            source: page.sourceOf("gaps-outer")
            onReapply: lm.reapply("look")
            label: "Around the screen"
            enabled: page.ready
            from: 0
            to: 100
            fallback: page.num(page.ui.gap_out)
            value: page.saved(page.look, "gaps_out")
            onChosen: v => page.setLook("gaps_out", v)
        }
        PixelSlider {
            id: borderWidth
            busy: lm.busy
            settingId: "border-width"
            iconName: "format-border-style"
            source: page.sourceOf("border-width")
            onReapply: lm.reapply("look")
            label: "Border width"
            enabled: page.ready
            from: 0
            to: 20
            fallback: page.num(page.ui.border_width)
            value: page.saved(page.look, "border_width")
            onChosen: v => page.setLook("border_width", v)
        }
        PixelSlider {
            id: rounding
            busy: lm.busy
            settingId: "rounding"
            iconName: "draw-rectangle-rounded"
            source: page.sourceOf("rounding")
            onReapply: lm.reapply("look")
            label: "Corner rounding"
            enabled: page.ready
            from: 0
            to: 40
            fallback: page.num(page.ui.radius)
            value: page.saved(page.look, "rounding")
            onChosen: v => page.setLook("rounding", v)
        }
    }

    // A small screen with three windows, drawn at twice the scale of a 1920-wide screen so
    // a few pixels still show: what the four sliders do, as they move.
    SettingsCard {
        title: "Preview"
        iconName: "view-preview"

        Rectangle {
            id: screen
            objectName: "windowPreview"
            readonly property real scale: width / 960
            readonly property real gapIn: gapsIn.live * scale
            readonly property real gapOut: gapsOut.live * scale
            readonly property real bw: borderWidth.live > 0 ? Math.max(1, borderWidth.live * scale) : 0
            readonly property real rad: rounding.live * scale
            readonly property real innerW: width - gapOut * 2
            readonly property real innerH: height - gapOut * 2
            color: Qt.tint(Kirigami.Theme.backgroundColor, Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.25))
            radius: Kirigami.Units.cornerRadius
            clip: true
            Layout.fillWidth: true
            Layout.preferredHeight: width * 9 / 16
            Layout.maximumHeight: Kirigami.Units.gridUnit * 14
            Accessible.role: Accessible.Graphic
            Accessible.name: "Preview of the gaps, borders and corners"

            Repeater {
                // The windows as fractions of the space inside the outer gap.
                model: [[0, 0, 0.5, 1, true], [0.5, 0, 0.5, 0.5, false], [0.5, 0.5, 0.5, 0.5, false]]
                Rectangle {
                    required property var modelData
                    x: screen.gapOut + modelData[0] * screen.innerW + (modelData[0] > 0 ? screen.gapIn / 2 : 0)
                    y: screen.gapOut + modelData[1] * screen.innerH + (modelData[1] > 0 ? screen.gapIn / 2 : 0)
                    width: modelData[2] * screen.innerW - (modelData[0] > 0 || modelData[2] < 1 ? screen.gapIn / 2 : 0)
                    height: modelData[3] * screen.innerH - (modelData[1] > 0 || modelData[3] < 1 ? screen.gapIn / 2 : 0)
                    radius: Math.min(screen.rad, width / 2, height / 2)
                    color: Kirigami.Theme.backgroundColor
                    border.width: screen.bw
                    border.color: modelData[4] ? Kirigami.Theme.highlightColor : Kirigami.Theme.disabledTextColor
                    Behavior on x { NumberAnimation { duration: Kirigami.Units.shortDuration } }
                    Behavior on y { NumberAnimation { duration: Kirigami.Units.shortDuration } }
                    Behavior on width { NumberAnimation { duration: Kirigami.Units.shortDuration } }
                    Behavior on height { NumberAnimation { duration: Kirigami.Units.shortDuration } }

                    Column {
                        x: Math.max(parent.radius / 2, Kirigami.Units.smallSpacing) + parent.border.width
                        y: Math.max(parent.radius / 2, Kirigami.Units.smallSpacing) + parent.border.width
                        spacing: Kirigami.Units.smallSpacing
                        Repeater {
                            model: [0.6, 0.4, 0.5]
                            Rectangle {
                                required property real modelData
                                width: parent.parent.width * modelData
                                height: Kirigami.Units.smallSpacing
                                radius: height / 2
                                color: Kirigami.Theme.disabledTextColor
                                opacity: 0.5
                            }
                        }
                    }
                }
            }
        }
    }
}
