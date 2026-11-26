// One field of the catalogue: a box to include it, and its value. An inline component can't
// see the dialog's ids, so it takes the dialog as `ed`.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import ".."

ColumnLayout {
    id: row

    required property var modelData
    property var ed
    property string group
    readonly property string key: modelData.key
    readonly property bool included: ed ? ed.isOn(group, key) : false
    readonly property var value: ed ? ed.valueOf(group, key) : undefined
    readonly property string kind: modelData.kind
    readonly property int lo: modelData.min === null ? 0 : modelData.min
    readonly property int hi: modelData.max === null ? 100000 : modelData.max

    /// Put the value into the text box (not a binding: one assignment would break it).
    function syncText() {
        const t = row.value === undefined || row.value === null ? "" : String(row.value);
        if (textBox.text !== t)
            textBox.text = t;
    }

    spacing: 0
    Layout.fillWidth: true
    onValueChanged: row.syncText()
    Component.onCompleted: row.syncText()

    RowLayout {
        spacing: Kirigami.Units.smallSpacing
        Layout.fillWidth: true

        QQC2.CheckBox {
            id: box
            objectName: row.group + "On_" + row.key
            checked: row.included
            Accessible.name: row.modelData.label
            onToggled: {
                row.ed.setField(row.group, row.key, box.checked, undefined);
                box.checked = Qt.binding(() => row.included);
            }
        }
        UqLabel {
            text: row.modelData.label
            Layout.minimumWidth: Kirigami.Units.gridUnit * 8
            MouseArea {
                anchors.fill: parent
                onClicked: row.ed.setField(row.group, row.key, !row.included, undefined)
            }
        }
        // Yes or No, for a thing a window is or isn't (match fields only).
        UqComboBox {
            objectName: row.group + "YesNo_" + row.key
            visible: row.kind === "bool" && row.group === "match"
            enabled: row.included
            model: ["Yes", "No"]
            currentIndex: row.value === false ? 1 : 0
            Accessible.name: row.modelData.label
            onActivated: index => row.ed.setField(row.group, row.key, true, index === 0)
        }
        UqComboBox {
            objectName: row.group + "Choice_" + row.key
            visible: row.kind === "choice"
            enabled: row.included
            model: row.modelData.choices
            currentIndex: Math.max(0, row.modelData.choices.indexOf(row.value))
            Accessible.name: row.modelData.label
            onActivated: index => row.ed.setField(row.group, row.key, true, row.modelData.choices[index])
        }
        QQC2.TextField {
            id: textBox
            objectName: row.group + "Value_" + row.key
            visible: row.kind === "text" || row.kind === "pattern"
            maximumLength: row.ed ? row.ed.patternLimit : 256
            placeholderText: row.kind === "pattern" ? "Pattern" : ""
            Accessible.name: row.modelData.label
            Layout.fillWidth: true
            onTextEdited: row.ed.setField(row.group, row.key, row.included || textBox.text !== "", textBox.text)
        }
        QQC2.Button {
            objectName: row.group + "Exact_" + row.key
            visible: row.kind === "pattern"
            text: "Use exactly this text"
            Accessible.description: "Match exactly what is typed, not as a pattern"
            onClicked: row.ed.useExact(row.key)
        }
        QQC2.SpinBox {
            objectName: row.group + "Number_" + row.key
            visible: row.kind === "int" || row.kind === "float"
            editable: true
            from: row.lo
            to: row.hi
            stepSize: row.kind === "float" ? 5 : 1
            value: row.value === undefined ? row.lo : (row.kind === "float" ? Math.round(row.value * 100) : row.value)
            textFromValue: (v, locale) => row.kind === "float" ? (v / 100).toFixed(2) : String(v)
            valueFromText: (text, locale) => row.kind === "float" ? Math.round(parseFloat(text) * 100) : parseInt(text, 10)
            Accessible.name: row.modelData.label
            onValueModified: row.ed.setField(row.group, row.key, true, row.kind === "float" ? value / 100 : value)
        }
        QQC2.SpinBox {
            id: first
            objectName: row.group + "A_" + row.key
            visible: row.kind === "pair"
            editable: true
            from: row.lo
            to: row.hi
            value: Array.isArray(row.value) ? row.value[0] : row.lo
            Accessible.name: row.modelData.label + ", first number"
            onValueModified: row.ed.setField(row.group, row.key, true, [value, second.value])
        }
        QQC2.SpinBox {
            id: second
            objectName: row.group + "B_" + row.key
            visible: row.kind === "pair"
            editable: true
            from: row.lo
            to: row.hi
            value: Array.isArray(row.value) ? row.value[1] : row.lo
            Accessible.name: row.modelData.label + ", second number"
            onValueModified: row.ed.setField(row.group, row.key, true, [first.value, value])
        }
        QQC2.CheckBox {
            objectName: row.group === "effect" && !!row.ed && row.key === row.ed.quietlyBeside ? "quietly" : ""
            visible: !!row.ed && row.group === "effect" && row.ed.kind === "window" && row.key === row.ed.quietlyBeside
            enabled: row.included
            text: "Quietly"
            checked: !!row.ed && row.ed.quietly
            Accessible.description: "Open it there without switching to that workspace"
            onToggled: {
                row.ed.quietly = checked;
                row.ed.refreshPreview();
            }
        }
    }
    UqLabel {
        visible: row.group === "effect" && row.modelData.static === true
        text: "applies when the window opens"
        opacity: 0.7
        font.italic: true
        Layout.leftMargin: Kirigami.Units.gridUnit * 2
    }
}
