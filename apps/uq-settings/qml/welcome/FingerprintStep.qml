// Welcome, step 7 (only with a fingerprint reader and no finger saved yet): save a fingerprint
// so the lock screen can be unlocked with it.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import "../components"

QQC2.ScrollView {
    id: step

    property var welcome: ({})
    property var model: null
    readonly property var enroll: {
        try {
            return JSON.parse(step.model ? step.model.enrollJson : "{}");
        } catch (e) {
            return {};
        }
    }
    readonly property string phase: enroll.phase || "idle"

    contentWidth: availableWidth

    ColumnLayout {
        width: Math.min(step.availableWidth, Kirigami.Units.gridUnit * 40)
        spacing: Kirigami.Units.largeSpacing

        UqLabel {
            text: "This computer has a fingerprint reader. Save your right index finger to unlock the screen with it. Your password still works too."
            wrapMode: Text.Wrap
            Layout.fillWidth: true
        }
        QQC2.Button {
            objectName: "enrollButton"
            visible: step.phase === "idle" || step.phase === "failed"
            text: "Save my fingerprint"
            icon.name: "fingerprint"
            onClicked: step.model.enroll()
        }
        QQC2.ProgressBar {
            visible: step.phase === "scanning" || step.phase === "done"
            from: 0
            to: Math.max(1, step.enroll.stages || 1)
            value: step.phase === "done" ? to : (step.enroll.stage || 0)
            indeterminate: step.phase === "scanning" && !(step.enroll.stages > 0)
            Accessible.name: "Fingerprint progress"
            Layout.fillWidth: true
        }
        UqLabel {
            objectName: "enrollMessage"
            visible: (step.enroll.message || "") !== ""
            text: step.enroll.message || ""
            color: step.phase === "failed" ? Kirigami.Theme.negativeTextColor : Kirigami.Theme.textColor
            wrapMode: Text.Wrap
            Layout.fillWidth: true
        }
        QQC2.Button {
            visible: step.phase === "scanning"
            text: "Stop"
            onClicked: step.model.cancelEnroll()
        }
    }
}
