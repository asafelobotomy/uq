// Appearance's Fonts tab: the interface, document and monospace fonts and their sizes.
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

    title: "Fonts"
    iconName: "preferences-desktop-font"

    SettingsCard {
        title: "Fonts"
        iconName: "preferences-desktop-font"

        OverrideCombo {
            settingId: "ui-font"
            iconName: "font-select-symbolic"
            source: page.sourceOf("ui-font")
            onReapply: am.reapply("appearance")
            label: "Interface font"
            highlight: App.highlight
            enabled: page.ready
            choices: Qt.fontFamilies()
            value: page.appearance.ui_font || null
            onChosen: v => page.field("ui_font", v)
            Layout.fillWidth: true
        }
        OverrideCombo {
            settingId: "mono-font"
            iconName: "utilities-terminal"
            source: page.sourceOf("mono-font")
            onReapply: am.reapply("appearance")
            label: "Monospace font"
            highlight: App.highlight
            enabled: page.ready
            choices: Qt.fontFamilies()
            value: page.appearance.mono_font || null
            onChosen: v => page.field("mono_font", v)
            Layout.fillWidth: true
        }
        OverrideSpin {
            busy: am.busy
            settingId: "font-size"
            iconName: "format-font-size-more"
            source: page.sourceOf("font-size")
            onReapply: am.reapply("appearance")
            label: "Font size"
            highlight: App.highlight
            enabled: page.ready
            from: 6
            to: 32
            fallback: page.themeFontSize
            unit: "pt"
            value: page.appearance.font_size === undefined ? null : page.appearance.font_size
            onChosen: v => page.field("font_size", v)
            Layout.fillWidth: true
        }
    }

    // The chosen fonts at the chosen size, so a change shows before it reaches the apps.
    SettingsCard {
        title: "Preview"
        iconName: "view-preview"

        UqLabel {
            objectName: "uiFontSample"
            text: "The quick brown fox jumps over the lazy dog"
            font.family: page.uiFont
            font.pointSize: page.fontSize
            elide: Text.ElideRight
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing
        }
        UqLabel {
            text: "Aa Bb Cc 0123456789"
            font.family: page.uiFont
            font.pointSize: page.fontSize * 1.6
            font.bold: true
            elide: Text.ElideRight
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing
        }
        Rectangle {
            radius: Kirigami.Units.cornerRadius
            color: Kirigami.Theme.backgroundColor
            implicitHeight: monoSample.implicitHeight + Kirigami.Units.largeSpacing * 2
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing
            Layout.rightMargin: Kirigami.Units.largeSpacing
            UqLabel {
                id: monoSample
                objectName: "monoFontSample"
                text: "$ ls -l ~/Pictures   0O 1lI {}"
                font.family: page.monoFont
                font.pointSize: page.fontSize
                elide: Text.ElideRight
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                anchors.margins: Kirigami.Units.largeSpacing
            }
        }
    }
}
