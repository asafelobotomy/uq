// One rule as a card: its app's icon (or the kind's), its name and switch on the title row,
// where it's overridden (with Open user.lua), what it matches and does, and Edit, Move and
// Delete. An inline component can't see the page's ids, so it takes the page as `pg`.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../../components"

SettingsCard {
    id: ruleCard

    required property var modelData
    required property int index
    property var pg
    property string kind
    property int count: 0
    readonly property bool idle: !!pg && pg.idle
    readonly property string key: kind === "workspace" ? String(modelData.workspace) : String(modelData.id)
    readonly property string ruleTitle: kind === "workspace" ? "Workspace " + String(modelData.workspace).replace(/^name:/, "") : String(modelData.name)
    readonly property string appId: kind === "window" && pg ? pg.appIdOf(modelData) : ""
    readonly property string kindIcon: kind === "window" ? "window" : kind === "layer" ? "dashboard-show" : "virtual-desktops"
    readonly property int matchesNow: kind === "window" ? (modelData.matches_now || 0) : 0
    /// `[{icon, text}]`: what the rule matches and what it does.
    readonly property var lines: {
        const m = ruleCard.modelData;
        if (ruleCard.kind === "window")
            return [{ icon: "view-filter", text: String(m.match_text) }, { icon: "configure", text: String(m.effects_text) }];
        if (ruleCard.kind === "layer")
            return [{ icon: "view-filter", text: "Panel name: " + String(m.namespace) }, { icon: "configure", text: String(m.effects_text) }];
        return [{ icon: "configure", text: String(m.fields_text) }, { icon: "view-grid", text: m.layout ? "Layout: " + String(m.layout) : "" }];
    }

    objectName: "ruleCard"
    title: ruleTitle
    iconName: appId !== "" && appIcon.status === Kirigami.Icon.Ready ? appId : kindIcon
    opacity: kind === "workspace" || modelData.enabled === true ? 1 : 0.75

    // Whether the theme has an icon for the app (an app ID with none shows the kind's icon).
    Kirigami.Icon {
        id: appIcon
        visible: false
        source: ruleCard.appId
        fallback: ""
    }

    actions: [
        QQC2.Switch {
            id: onSwitch
            visible: ruleCard.kind !== "workspace"
            checked: ruleCard.modelData.enabled === true
            enabled: ruleCard.idle
            Accessible.name: "Rule on"
            Accessible.description: ruleCard.ruleTitle
            onToggled: {
                ruleCard.pg.turnRule(ruleCard.kind, ruleCard.key, onSwitch.checked);
                onSwitch.checked = Qt.binding(() => ruleCard.modelData.enabled === true);
            }
        }
    ]

    SourceLine {
        source: ({ source: ruleCard.modelData.source, detail: { line: ruleCard.modelData.line, path: ruleCard.modelData.path, message: ruleCard.modelData.message, name: ruleCard.modelData.profile } })
        Layout.fillWidth: true
    }
    Repeater {
        model: ruleCard.lines.filter(l => l.text !== "")
        delegate: RowLayout {
            id: line

            required property var modelData

            spacing: Kirigami.Units.smallSpacing
            Layout.fillWidth: true

            Kirigami.Icon {
                source: line.modelData.icon
                implicitWidth: Kirigami.Units.iconSizes.small
                implicitHeight: Kirigami.Units.iconSizes.small
                Layout.alignment: Qt.AlignTop
                Accessible.ignored: true
            }
            UqLabel {
                text: line.modelData.text
                wrapMode: Text.Wrap
                opacity: 0.8
                Layout.fillWidth: true
            }
        }
    }
    RowLayout {
        spacing: Kirigami.Units.smallSpacing
        Layout.fillWidth: true

        // How many open windows it matches now.
        Kirigami.Icon {
            visible: ruleCard.matchesNow > 0
            source: "window-new"
            implicitWidth: Kirigami.Units.iconSizes.small
            implicitHeight: Kirigami.Units.iconSizes.small
            Accessible.ignored: true
        }
        UqLabel {
            objectName: "matchesNow"
            visible: ruleCard.matchesNow > 0
            text: ruleCard.matchesNow === 1 ? "Matches 1 open window" : "Matches " + ruleCard.matchesNow + " open windows"
            opacity: 0.7
        }
        Item {
            Layout.fillWidth: true
        }
        QQC2.ToolButton {
            text: "Edit"
            icon.name: "document-edit"
            display: QQC2.AbstractButton.IconOnly
            enabled: ruleCard.idle
            Accessible.description: ruleCard.ruleTitle
            QQC2.ToolTip.text: "Edit"
            QQC2.ToolTip.visible: hovered
            QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
            onClicked: ruleCard.pg.editRule(ruleCard.kind, ruleCard.modelData.raw)
        }
        QQC2.ToolButton {
            text: "Move earlier"
            icon.name: "go-previous"
            display: QQC2.AbstractButton.IconOnly
            visible: ruleCard.count > 1
            enabled: ruleCard.idle && ruleCard.index > 0
            Accessible.description: ruleCard.ruleTitle
            QQC2.ToolTip.text: "Move earlier"
            QQC2.ToolTip.visible: hovered
            QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
            onClicked: ruleCard.pg.moveRule(ruleCard.kind, ruleCard.key, ruleCard.index - 1)
        }
        QQC2.ToolButton {
            text: "Move later"
            icon.name: "go-next"
            display: QQC2.AbstractButton.IconOnly
            visible: ruleCard.count > 1
            enabled: ruleCard.idle && ruleCard.index < ruleCard.count - 1
            Accessible.description: ruleCard.ruleTitle
            QQC2.ToolTip.text: "Move later"
            QQC2.ToolTip.visible: hovered
            QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
            onClicked: ruleCard.pg.moveRule(ruleCard.kind, ruleCard.key, ruleCard.index + 1)
        }
        QQC2.ToolButton {
            text: "Delete"
            icon.name: "edit-delete"
            display: QQC2.AbstractButton.IconOnly
            enabled: ruleCard.idle
            Accessible.description: ruleCard.ruleTitle
            QQC2.ToolTip.text: "Delete"
            QQC2.ToolTip.visible: hovered
            QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
            onClicked: ruleCard.pg.askDelete(ruleCard.kind, ruleCard.key)
        }
    }
}
