// One setting: a label on the left, its control(s) on the right; when the row is too narrow for
// both side by side (a small window), the controls go under the label instead. When `highlight` (bound to
// App.highlight by the page) becomes this row's `settingId`, the row scrolls into view and
// flashes for 2 s. Under the label, `source` (uqd's `{source, detail}` for the row) says where the
// live value comes from; `uq` or none shows nothing, so the row keeps its height.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Rectangle {
    id: row

    property string settingId
    property string label
    /// An icon before the label: what the setting is about, at a glance.
    property string iconName: ""
    /// A longer explanation, shown as a "?" tooltip after the label.
    property string help: ""
    /// The controls take the row's whole width (a gallery, a canvas, a row of tiles) instead of
    /// their own; give them `Layout.fillWidth`. Never size them from the card's width: the card's
    /// width follows its contents, and the two chase each other.
    property bool wideControls: false
    property string highlight
    readonly property bool flashing: flash.running
    /// The controls sit under the label: there is no room for them beside it.
    readonly property bool stacked: {
        if (row.width <= 0 || slot.children.length === 0)
            return false;
        const sp = Kirigami.Units.largeSpacing;
        const label = labelText.short ? labelText.fullWidth : Kirigami.Units.gridUnit * 10;
        const help = rowHelp.visible ? rowHelp.implicitWidth + Kirigami.Units.smallSpacing : 0;
        const icon = rowIcon.visible ? rowIcon.implicitWidth + sp : 0;
        const controls = row.wideControls ? Kirigami.Units.gridUnit * 10 : slot.implicitWidth;
        const need = Kirigami.Units.smallSpacing * 2 + icon + (labelColumn.visible ? label + help + sp : 0) + controls;
        // A little slack: a control can grow a few pixels once its value arrives.
        return need + Kirigami.Units.largeSpacing > row.width;
    }
    default property alias content: slot.data
    /// Where the live value comes from: `{source, detail}` from the page's `sources`, or null.
    property var source: null
    /// Whether **Open user.lua** opens the file (tests turn it off).
    property bool opensFiles: true
    readonly property string sourceKind: source && source.source ? source.source : ""
    readonly property var sourceDetail: source && source.detail ? source.detail : ({})
    readonly property string sourceText: {
        switch (sourceKind) {
        case "theme":
            return "From the theme";
        case "profile":
            return sourceDetail.name ? "From the " + sourceDetail.name + " profile" : "From a profile";
        case "default":
            return "Default";
        case "user_lua":
            return sourceDetail.line ? "Overridden in user.lua, line " + sourceDetail.line : "Overridden in user.lua";
        case "runtime":
            return "Changed while running (not by uQ)";
        case "error":
            return sourceDetail.message ? String(sourceDetail.message) : "";
        default:
            return "";
        }
    }
    readonly property string userLuaUrl: sourceDetail.path ? "file://" + sourceDetail.path.split("/").map(encodeURIComponent).join("/") : ""

    /// **Open user.lua** was pressed; `line` is the line that sets this value.
    signal openUserLua(int line)
    /// **Re-apply** was pressed: put uQ's value back over the one changed while running.
    signal reapply()

    function flashNow() {
        flash.restart();
        revealer.reveal(row, Kirigami.Units.largeSpacing);
    }
    /// The SettingsTab this row is on, if any.
    function tabOf() {
        for (let p = row.parent; p; p = p.parent) {
            if (p.isSettingsTab === true)
                return p;
        }
        return null;
    }
    function check() {
        if (highlight === "" || highlight !== settingId)
            return;
        // A search result on another tab: show that tab first, then scroll to the row.
        const tab = tabOf();
        if (tab && !tab.visible) {
            tab.activate();
            Qt.callLater(flashNow);
        } else {
            flashNow();
        }
    }

    implicitWidth: layout.implicitWidth + Kirigami.Units.largeSpacing * 2
    // Every row at least one control tall, so a card's rows sit on an even rhythm.
    implicitHeight: Math.max(layout.implicitHeight + Kirigami.Units.smallSpacing * 2, Kirigami.Units.gridUnit * 2.25)
    radius: Kirigami.Units.cornerRadius
    color: flashing ? Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g,
                              Kirigami.Theme.highlightColor.b, 0.25) : "transparent"
    Behavior on color {
        ColorAnimation { duration: Kirigami.Units.shortDuration }
    }

    Accessible.role: Accessible.Grouping
    Accessible.name: label
    Accessible.description: sourceText

    onHighlightChanged: check()
    Component.onCompleted: check()
    // Layout may place the row after the highlight arrives (a page that just loaded); follow it.
    onYChanged: if (flashing) revealer.reveal(row, Kirigami.Units.largeSpacing)

    ScrollReveal {
        id: revealer
    }

    Timer {
        id: flash
        interval: 2000
    }

    GridLayout {
        id: layout
        anchors.fill: parent
        anchors.leftMargin: Kirigami.Units.smallSpacing
        anchors.rightMargin: Kirigami.Units.smallSpacing
        anchors.topMargin: row.stacked ? Kirigami.Units.smallSpacing : 0
        anchors.bottomMargin: row.stacked ? Kirigami.Units.smallSpacing : 0
        columns: row.stacked ? 2 : 3
        columnSpacing: Kirigami.Units.largeSpacing
        rowSpacing: Kirigami.Units.smallSpacing

        // The icon column is kept on rows without an icon too, so every label in a card starts
        // at the same place.
        Kirigami.Icon {
            id: rowIcon
            objectName: "rowIcon"
            opacity: row.iconName !== "" ? 1 : 0
            visible: !(row.wideControls && row.iconName === "")
            source: row.iconName
            implicitWidth: Kirigami.Units.iconSizes.smallMedium
            implicitHeight: Kirigami.Units.iconSizes.smallMedium
            Layout.alignment: Qt.AlignVCenter
            Accessible.ignored: true
        }
        ColumnLayout {
            id: labelColumn
            spacing: 0
            // Wide controls: the label column shows only when there is a label or a source.
            visible: !row.wideControls || row.label !== "" || row.sourceText !== ""
            Layout.fillWidth: !row.wideControls || row.stacked

            RowLayout {
                spacing: Kirigami.Units.smallSpacing
                Layout.fillWidth: true
                UqLabel {
                    id: labelText
                    text: row.label
                    readonly property real fullWidth: Math.ceil(labelMetrics.advanceWidth) + 2
                    readonly property bool short: fullWidth <= Kirigami.Units.gridUnit * 14
                    // A short label stays on one line (the controls give way first); a long one
                    // may wrap down to a readable width.
                    wrapMode: short && !row.stacked ? Text.NoWrap : Text.Wrap
                    Layout.fillWidth: true
                    // The text's own width, measured unwrapped: a wrapped Text's implicitWidth
                    // follows its wrapped width, which squeezed short labels a letter a line.
                    // TextMetrics (not a FontMetrics call) so it follows the font once it's set.
                    Layout.maximumWidth: fullWidth
                    Layout.minimumWidth: row.stacked ? 0 : (short ? fullWidth : Kirigami.Units.gridUnit * 10)
                    TextMetrics {
                        id: labelMetrics
                        font: labelText.font
                        text: row.label
                    }
                }
                HelpTip {
                    id: rowHelp
                    objectName: "rowHelp"
                    tip: row.help
                }
                // Always there: without it the capped label made this row, and so the label
                // column, stop stretching, and the controls sat just after short labels.
                Item {
                    Layout.fillWidth: true
                }
            }
            RowLayout {
                objectName: "sourceLine"
                visible: row.sourceText !== ""
                spacing: Kirigami.Units.smallSpacing
                Layout.fillWidth: true

                Kirigami.Icon {
                    objectName: "sourceWarning"
                    visible: row.sourceKind === "user_lua"
                    source: "dialog-warning"
                    implicitWidth: Kirigami.Units.iconSizes.small
                    implicitHeight: Kirigami.Units.iconSizes.small
                    Accessible.ignored: true
                }
                UqLabel {
                    objectName: "sourceText"
                    text: row.sourceText
                    wrapMode: Text.Wrap
                    opacity: row.sourceKind === "theme" || row.sourceKind === "default" || row.sourceKind === "profile" ? 0.7 : 1
                    color: row.sourceKind === "error" ? Kirigami.Theme.negativeTextColor : Kirigami.Theme.textColor
                    Layout.fillWidth: true
                }
                QQC2.Button {
                    objectName: "openUserLuaButton"
                    visible: row.sourceKind === "user_lua"
                    enabled: row.userLuaUrl !== ""
                    text: "Open user.lua"
                    icon.name: "document-open"
                    onClicked: {
                        row.openUserLua(row.sourceDetail.line || 0);
                        if (row.opensFiles)
                            Qt.openUrlExternally(row.userLuaUrl);
                    }
                }
                QQC2.Button {
                    objectName: "reapplyButton"
                    visible: row.sourceKind === "runtime"
                    text: "Re-apply"
                    icon.name: "view-refresh"
                    onClicked: row.reapply()
                }
            }
        }
        // Stacked: under the label, lined up with it, as wide as the row allows.
        RowLayout {
            id: slot
            spacing: Kirigami.Units.smallSpacing
            Layout.fillWidth: row.wideControls || row.stacked
            Layout.columnSpan: row.stacked ? 2 : 1
            Layout.leftMargin: row.stacked && rowIcon.visible ? rowIcon.implicitWidth + Kirigami.Units.largeSpacing : 0
            Layout.maximumWidth: row.stacked ? Math.max(0, layout.width - Layout.leftMargin) : -1
        }
    }
}
