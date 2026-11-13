// The bar's layout: one list with a heading for each zone (Left, Centre, Right) and the islands
// under them. Drag a row by its handle, or use its up and down buttons (Alt+Up / Alt+Down on a
// focused row), to reorder; moving past a heading puts the island in the next zone. Each row has
// a size and, where the island has some, options. The editor never changes anything itself: it
// asks (`edit(object)`, a uq_core BarEdit) and shows `bar` again once the page has uqd's answer.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import "barrows.js" as R
import "."

ColumnLayout {
    id: root

    /// shell.bar as uqd has it.
    property var bar: ({})
    /// [{id, name}] every island uQ has (the page's snapshot).
    property var catalog: []
    signal edit(var edit)

    readonly property var rows: R.rows(bar)
    readonly property var options: bar && bar.options ? bar.options : ({})
    readonly property alias list: view
    readonly property alias addCombo: addBox
    // Rows whose options are open, by island id.
    property var open: ({})
    readonly property var sizeIds: ["compact", "normal", "wide"]

    // Where keyboard focus goes back to once uqd's answer redraws the rows: the island an edit
    // moved or resized (`zone`/`pos` after the edit) and which control ("up", "down", "size").
    property var refocus: null
    function send(e, part) {
        if (e === null)
            return;
        if (part !== undefined && (e.op === "move" || e.op === "size"))
            refocus = { zone: e.op === "move" ? e.to_zone : e.zone,
                        pos: e.op === "move" ? e.to_index : e.index, part: part };
        edit(e);
    }
    function restoreFocus() {
        const r = refocus;
        refocus = null;
        if (r === null)
            return;
        view.forceLayout();
        for (let i = 0; i < view.count; i++) {
            const it = view.itemAtIndex(i);
            if (!it || it.kind !== "item" || it.zone !== r.zone || it.pos !== r.pos)
                continue;
            let c = r.part === "size" ? it.sizeBox : (r.part === "down" ? it.downButton : it.upButton);
            if (!c.enabled)
                c = c === it.upButton ? it.downButton : it.upButton;
            c.forceActiveFocus();
            return;
        }
    }
    function toggleOptions(id) {
        const o = Object.assign({}, open);
        o[id] = !o[id];
        open = o;
    }

    // The rows as drawn: kept in a ListModel so a drag can move a row while it is held. It is
    // rebuilt from `bar` whenever uqd's answer comes back.
    ListModel {
        id: model
    }
    function sync() {
        model.clear();
        for (const r of rows)
            model.append({ kind: r.kind, zone: r.zone, pos: r.kind === "item" ? r.index : -1,
                           islandId: r.kind === "item" ? r.id : "", size: r.kind === "item" ? r.size : "",
                           label: r.kind === "item" ? R.name(catalog, r.id) : r.name });
        if (refocus !== null)
            Qt.callLater(restoreFocus);
    }
    // Rebuilt only when the zones change, not when an island's options do, so an open options
    // panel (and the field being typed in) stays put.
    readonly property string rowsKey: JSON.stringify(rows)
    onRowsKeyChanged: sync()
    onCatalogChanged: sync()
    Component.onCompleted: sync()

    // Where the held row started, in `rows`; -1 when nothing is held.
    property int dragFrom: -1
    function dropped(at) {
        if (dragFrom < 0)
            return;
        const from = dragFrom;
        dragFrom = -1;
        // `at` is the row's place now; in `rows` it lands just before the row at `to`.
        const to = at > from ? at + 1 : at;
        const e = R.moveEdit(rows, from, to);
        if (e === null)
            sync();
        else
            send(e);
    }

    spacing: Kirigami.Units.smallSpacing

    ListView {
        id: view
        objectName: "barRows"
        Layout.fillWidth: true
        Layout.preferredHeight: contentHeight
        interactive: false
        // Every row exists (the page scrolls, not the list), so focus and tests reach them all.
        cacheBuffer: 100000
        model: model
        spacing: 0
        Accessible.role: Accessible.List
        Accessible.name: "Islands in the bar"

        delegate: Item {
            id: rowItem
            required property int index
            required property string kind
            required property string zone
            required property string islandId
            required property int pos
            required property string size
            required property string label
            readonly property bool isItem: kind === "item"
            readonly property bool hasOptions: isItem && R.WITH_OPTIONS.indexOf(islandId) >= 0
            readonly property bool optionsOpen: hasOptions && root.open[islandId] === true
            readonly property alias upButton: up
            readonly property alias downButton: down
            readonly property alias removeButton: remove
            readonly property alias sizeBox: sizeCombo
            readonly property alias optionsButton: optionsBtn

            width: view.width
            implicitHeight: col.implicitHeight

            ColumnLayout {
                id: col
                width: parent.width
                spacing: 0

                // A zone heading.
                UqLabel {
                    visible: !rowItem.isItem
                    text: rowItem.label
                    font.bold: true
                    Layout.topMargin: rowItem.index === 0 ? 0 : Kirigami.Units.largeSpacing
                    Accessible.role: Accessible.Heading
                }
                UqLabel {
                    readonly property bool empty: !rowItem.isItem
                        && (rowItem.index + 1 >= model.count || model.get(rowItem.index + 1).kind === "zone")
                    visible: empty
                    text: "Nothing here. Drag an island here, or add one below."
                    opacity: 0.7
                    Layout.leftMargin: Kirigami.Units.largeSpacing * 2
                }

                // An island.
                RowLayout {
                    id: line
                    visible: rowItem.isItem
                    spacing: Kirigami.Units.smallSpacing
                    Layout.fillWidth: true
                    Layout.leftMargin: Kirigami.Units.largeSpacing
                    activeFocusOnTab: false
                    Keys.onPressed: event => {
                        if (!(event.modifiers & Qt.AltModifier))
                            return;
                        if (event.key === Qt.Key_Up) {
                            root.send(R.stepEdit(root.rows, rowItem.index, -1), "up");
                            event.accepted = true;
                        } else if (event.key === Qt.Key_Down) {
                            root.send(R.stepEdit(root.rows, rowItem.index, 1), "down");
                            event.accepted = true;
                        }
                    }

                    Kirigami.ListItemDragHandle {
                        listItem: rowItem
                        listView: view
                        enabled: root.enabled
                        Accessible.name: "Drag " + rowItem.label
                        onMoveRequested: (oldIndex, newIndex) => {
                            if (newIndex < 1)
                                return;
                            if (root.dragFrom < 0)
                                root.dragFrom = oldIndex;
                            model.move(oldIndex, newIndex, 1);
                        }
                        onDropped: root.dropped(rowItem.index)
                    }
                    UqLabel {
                        text: rowItem.label
                        elide: Text.ElideRight
                        Layout.fillWidth: true
                    }
                    UqComboBox {
                        id: sizeCombo
                        model: ["Compact", "Normal", "Wide"]
                        currentIndex: Math.max(0, root.sizeIds.indexOf(rowItem.size))
                        enabled: root.enabled
                        Accessible.name: "Size of " + rowItem.label
                        onActivated: i => {
                            root.send(R.sizeEdit(root.rows[rowItem.index], root.sizeIds[i]), "size");
                            sizeCombo.currentIndex = Qt.binding(() => Math.max(0, root.sizeIds.indexOf(rowItem.size)));
                        }
                    }
                    QQC2.ToolButton {
                        id: optionsBtn
                        visible: rowItem.hasOptions
                        icon.name: "configure"
                        checkable: true
                        checked: rowItem.optionsOpen
                        enabled: root.enabled
                        Accessible.name: "Options for " + rowItem.label
                        onClicked: root.toggleOptions(rowItem.islandId)
                    }
                    QQC2.ToolButton {
                        id: up
                        icon.name: "go-up"
                        enabled: root.enabled && R.stepEdit(root.rows, rowItem.index, -1) !== null
                        Accessible.name: "Move " + rowItem.label + " up"
                        onClicked: root.send(R.stepEdit(root.rows, rowItem.index, -1), "up")
                    }
                    QQC2.ToolButton {
                        id: down
                        icon.name: "go-down"
                        enabled: root.enabled && R.stepEdit(root.rows, rowItem.index, 1) !== null
                        Accessible.name: "Move " + rowItem.label + " down"
                        onClicked: root.send(R.stepEdit(root.rows, rowItem.index, 1), "down")
                    }
                    QQC2.ToolButton {
                        id: remove
                        icon.name: "list-remove"
                        enabled: root.enabled
                        Accessible.name: "Remove " + rowItem.label + " from the bar"
                        onClicked: root.send(R.removeEdit(root.rows[rowItem.index]))
                    }
                }
                Loader {
                    active: rowItem.optionsOpen
                    visible: active
                    Layout.fillWidth: true
                    Layout.leftMargin: Kirigami.Units.largeSpacing * 4
                    Layout.bottomMargin: Kirigami.Units.smallSpacing
                    sourceComponent: BarIslandOptions {
                        island: rowItem.islandId
                        options: root.options[rowItem.islandId] || ({})
                        enabled: root.enabled
                        onChanged: (island, value) => root.send({ op: "options", island: island, value: value })
                    }
                }
            }
        }
    }

    RowLayout {
        spacing: Kirigami.Units.smallSpacing
        Layout.topMargin: Kirigami.Units.largeSpacing
        UqComboBox {
            id: addBox
            objectName: "addIsland"
            readonly property var choices: R.addable(root.bar, root.catalog)
            model: choices.map(c => c.name)
            enabled: root.enabled && choices.length > 0
            Accessible.name: "Island to add"
        }
        UqComboBox {
            id: zoneBox
            objectName: "addZone"
            model: ["Left", "Centre", "Right"]
            currentIndex: 2
            enabled: addBox.enabled
            Accessible.name: "Where to add it"
        }
        QQC2.Button {
            objectName: "addButton"
            text: "Add"
            icon.name: "list-add"
            enabled: addBox.enabled && addBox.currentIndex >= 0
            onClicked: root.send(R.addEdit(addBox.choices[addBox.currentIndex].id, R.ZONES[zoneBox.currentIndex]))
        }
        Item {
            Layout.fillWidth: true
        }
        QQC2.Button {
            objectName: "resetBar"
            text: "Reset the bar"
            icon.name: "edit-undo"
            enabled: root.enabled
            onClicked: root.send({ op: "reset" })
        }
    }
}
