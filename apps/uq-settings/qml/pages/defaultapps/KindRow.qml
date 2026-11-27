// One kind on the Default apps tab: its icon, its name, the app that opens it, and a button to go
// back to the system's choice. App names come from other packages' files, so they reach the
// combo as model strings, which IconComboBox paints through UqLabel.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../../components"

SettingRow {
    id: row

    required property var kind
    required property string iconFor
    required property bool idle

    signal chosen(string app)
    signal resetRequested

    readonly property var apps: row.kind.apps || []
    readonly property bool mine: row.kind.source === "you" || row.kind.source === "override"

    /// The combo's entries, each `{text, icon}`: "Not chosen" when nothing is set, else the
    /// apps, uQ's own marked; a chosen app that is gone is listed last.
    function items() {
        const list = row.apps.map(a => ({
                    text: String(a.name) + (a.native === true ? " (uQ's own)" : ""),
                    icon: String(a.icon || "")
                }));
        const i = row.apps.findIndex(a => a.id === row.kind.current);
        if (row.kind.current && i < 0)
            return list.concat([{
                    text: String(row.kind.current) + " (not installed)",
                    icon: "dialog-warning"
                }]);
        if (!row.kind.current)
            return [{
                    text: "Not chosen",
                    icon: "dialog-cancel"
                }].concat(list);
        return list;
    }
    function index() {
        const i = row.apps.findIndex(a => a.id === row.kind.current);
        if (!row.kind.current)
            return 0;
        return i < 0 ? row.apps.length : i;
    }
    /// The app behind combo entry `i`, or "" for an entry that isn't one.
    function appAt(i) {
        const j = row.kind.current ? i : i - 1;
        return j >= 0 && j < row.apps.length ? String(row.apps[j].id) : "";
    }

    settingId: "default-" + row.kind.id
    iconName: row.iconFor
    label: String(row.kind.label)
    help: {
        if (row.apps.length === 0)
            return "No installed app says it can open this.";
        if (row.kind.source === "system")
            return "The system's choice. Choosing one here makes it yours.";
        if (row.kind.source === "override")
            return "Set in your desktop's own list, which wins over this page. Choosing one here moves it.";
        if (row.kind.native && row.kind.native !== row.kind.current)
            return "uQ has its own app for this.";
        return "";
    }
    highlight: App.highlight
    Layout.fillWidth: true

    IconComboBox {
        id: combo
        model: row.items()
        currentIndex: row.index()
        fallbackIcon: "application-x-executable"
        enabled: row.idle && row.apps.length > 0
        Accessible.name: String(row.kind.label)
        Layout.preferredWidth: Kirigami.Units.gridUnit * 14
        onActivated: i => {
            const app = row.appAt(i);
            if (app !== "" && app !== row.kind.current)
                row.chosen(app);
            combo.currentIndex = Qt.binding(() => row.index());
        }
    }
    QQC2.ToolButton {
        objectName: "reset_" + row.kind.id
        icon.name: "edit-undo"
        text: "Use the system's choice"
        display: QQC2.AbstractButton.IconOnly
        // Kept in place when hidden, so every row's box lines up.
        opacity: row.mine ? 1 : 0
        enabled: row.idle && row.mine
        Accessible.ignored: !row.mine
        QQC2.ToolTip.visible: hovered
        QQC2.ToolTip.text: "Use the system's choice"
        onClicked: row.resetRequested()
    }
}
