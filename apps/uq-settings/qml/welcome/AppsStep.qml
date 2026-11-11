// Welcome, step 6: the apps made for uQ that are installed, and what each missing one adds,
// with its package name to copy (or bindle, when it is installed, to install them).
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import "../components"

QQC2.ScrollView {
    id: step

    property var welcome: ({})
    property var model: null
    readonly property var apps: welcome.apps || []
    readonly property var installed: apps.filter(a => a.installed)
    readonly property var missing: apps.filter(a => !a.installed)

    contentWidth: availableWidth

    ColumnLayout {
        width: Math.min(step.availableWidth, Kirigami.Units.gridUnit * 40)
        spacing: Kirigami.Units.smallSpacing

        UqLabel {
            text: "These apps add pages, widgets and launcher results to uQ."
            wrapMode: Text.Wrap
            Layout.fillWidth: true
        }
        UqLabel {
            visible: step.installed.length > 0
            text: "Installed"
            font.bold: true
            Layout.topMargin: Kirigami.Units.largeSpacing
        }
        Repeater {
            model: step.installed
            UqLabel {
                required property var modelData
                text: "✓ " + modelData.name + ": " + modelData.description
                wrapMode: Text.Wrap
                Layout.fillWidth: true
            }
        }
        RowLayout {
            visible: step.missing.length > 0
            Layout.topMargin: Kirigami.Units.largeSpacing
            Layout.fillWidth: true
            UqLabel {
                text: "Not installed"
                font.bold: true
                Layout.fillWidth: true
            }
            QQC2.Button {
                objectName: "installWithBindle"
                visible: step.welcome.bindle === true
                text: "Install with bindle"
                icon.name: "system-software-install"
                onClicked: step.model.openBindle()
            }
        }
        Repeater {
            model: step.missing
            ColumnLayout {
                id: row
                required property var modelData
                readonly property string where: modelData.package || ""
                spacing: 2
                Layout.fillWidth: true
                Layout.topMargin: Kirigami.Units.smallSpacing
                UqLabel {
                    text: row.modelData.name + ": " + row.modelData.description
                    wrapMode: Text.Wrap
                    Layout.fillWidth: true
                }
                RowLayout {
                    visible: row.where !== ""
                    UqLabel {
                        text: "Package:"
                        color: Kirigami.Theme.disabledTextColor
                    }
                    QQC2.TextField {
                        id: pkg
                        text: row.where
                        readOnly: true
                        selectByMouse: true
                        Accessible.name: "Package name for " + row.modelData.name
                        Layout.preferredWidth: Kirigami.Units.gridUnit * 10
                    }
                    QQC2.Button {
                        text: "Copy"
                        icon.name: "edit-copy"
                        Accessible.name: "Copy the package name"
                        onClicked: {
                            pkg.selectAll();
                            pkg.copy();
                            pkg.deselect();
                        }
                    }
                }
                UqLabel {
                    visible: row.where === ""
                    text: "Download it from github.com/" + (row.modelData.project || "") + ", or install it with bindle."
                    wrapMode: Text.Wrap
                    color: Kirigami.Theme.disabledTextColor
                    Layout.fillWidth: true
                }
            }
        }
    }
}
