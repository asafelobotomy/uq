// Appearance's Cursor and icons tab.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Dialogs
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../../components"

SettingsTab {
    // Passed in from AppearancePage.qml.
    property var am
    property var page

    title: "Cursor and icons"
    iconName: "preferences-desktop-icons"

    SettingsCard {
        title: "Cursor"
        iconName: "preferences-desktop-cursors"

        OverrideCombo {
            settingId: "cursor-theme"
            iconName: "preferences-desktop-cursors"
            source: page.sourceOf("cursor-theme")
            onReapply: am.reapply("appearance")
            label: "Cursor theme"
            highlight: App.highlight
            enabled: page.ready
            choices: page.snap.cursorThemes || []
            value: page.appearance.cursor_theme || null
            onChosen: v => page.field("cursor_theme", v)
            Layout.fillWidth: true
        }
        OverrideSpin {
            busy: am.busy
            settingId: "cursor-size"
            iconName: "transform-scale"
            source: page.sourceOf("cursor-size")
            onReapply: am.reapply("appearance")
            label: "Cursor size"
            highlight: App.highlight
            enabled: page.ready
            from: 16
            to: 96
            fallback: (page.tokens.icons && page.tokens.icons.cursor_size) || 24
            unit: "px"
            value: page.appearance.cursor_size === undefined ? null : page.appearance.cursor_size
            onChosen: v => page.field("cursor_size", v)
            Layout.fillWidth: true
        }
    }
    SettingsCard {
        title: "Icons"
        iconName: "preferences-desktop-icons"

        OverrideCombo {
            settingId: "icon-theme"
            iconName: "preferences-desktop-icons"
            source: page.sourceOf("icon-theme")
            onReapply: am.reapply("appearance")
            label: "Icon theme"
            highlight: App.highlight
            enabled: page.ready
            choices: page.snap.iconThemes || []
            value: page.appearance.icon_theme || null
            onChosen: v => page.field("icon_theme", v)
            Layout.fillWidth: true
        }
    }
}
