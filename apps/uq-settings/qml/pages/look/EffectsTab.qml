// Look's Effects tab: blur, shadows, dimming and opacity.
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

    title: "Effects"
    iconName: "preferences-desktop-effects"

    SettingsCard {
        title: "Blur"
        iconName: "blur"

        OverrideSwitch {
            settingId: "blur"
            iconName: "blur"
            source: page.sourceOf("blur")
            onReapply: lm.reapply("look")
            label: "Blur behind windows"
            highlight: App.highlight
            enabled: page.ready
            fallback: !!page.dLook.blur
            value: page.saved(page.look, "blur")
            onChosen: v => page.setLook("blur", v)
            Layout.fillWidth: true
        }
        PixelSlider {
            busy: lm.busy
            settingId: "blur-size"
            iconName: "zoom-in"
            source: page.sourceOf("blur-size")
            onReapply: lm.reapply("look")
            label: "Blur size"
            help: "How far each pixel is spread."
            enabled: page.ready
            from: 1
            to: 20
            unit: ""
            useLabel: "Use default"
            fallback: page.num(page.dLook.blur_size)
            value: page.saved(page.look, "blur_size")
            onChosen: v => page.setLook("blur_size", v)
        }
        PixelSlider {
            busy: lm.busy
            settingId: "blur-passes"
            iconName: "media-playlist-repeat"
            source: page.sourceOf("blur-passes")
            onReapply: lm.reapply("look")
            label: "Blur passes"
            help: "More passes look smoother and use more power."
            enabled: page.ready
            from: 1
            to: 4
            unit: ""
            useLabel: "Use default"
            fallback: page.num(page.dLook.blur_passes)
            value: page.saved(page.look, "blur_passes")
            onChosen: v => page.setLook("blur_passes", v)
        }
    }
    SettingsCard {
        title: "Shadows"
        iconName: "object-order-front"

        OverrideSwitch {
            settingId: "shadow"
            iconName: "object-order-front"
            source: page.sourceOf("shadow")
            onReapply: lm.reapply("look")
            label: "Window shadows"
            highlight: App.highlight
            enabled: page.ready
            fallback: !!page.dLook.shadow
            value: page.saved(page.look, "shadow")
            onChosen: v => page.setLook("shadow", v)
            Layout.fillWidth: true
        }
    }
}
