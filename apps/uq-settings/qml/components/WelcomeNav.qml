// Welcome's buttons along the bottom: Back, Skip and Next (Finish on the last step), with
// "Step N of M". Plain QtQuick and Controls, so it is tested offscreen without Kirigami.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts

RowLayout {
    id: nav

    property int index: 0
    property int count: 1
    property bool first: index === 0
    property bool last: index + 1 >= count
    /// A request is in flight: the buttons wait, so a quick double press moves one step.
    property bool busy: false

    signal back()
    signal skip()
    signal next()
    signal finish()

    spacing: 8

    QQC2.Button {
        objectName: "backButton"
        text: "Back"
        icon.name: "go-previous"
        enabled: !nav.first && !nav.busy
        Accessible.name: "Back"
        onClicked: nav.back()
    }
    UqLabel {
        objectName: "stepCount"
        text: "Step " + (nav.index + 1) + " of " + nav.count
        horizontalAlignment: Text.AlignHCenter
        opacity: 0.7
        Layout.fillWidth: true
    }
    QQC2.Button {
        objectName: "skipButton"
        text: "Skip"
        visible: !nav.last
        enabled: !nav.busy
        Accessible.name: "Skip this step"
        onClicked: nav.skip()
    }
    QQC2.Button {
        objectName: "nextButton"
        text: "Next"
        icon.name: "go-next"
        visible: !nav.last
        enabled: !nav.busy
        highlighted: true
        Accessible.name: "Next"
        onClicked: nav.next()
    }
    QQC2.Button {
        objectName: "finishButton"
        text: "Finish"
        icon.name: "dialog-ok"
        visible: nav.last
        highlighted: true
        Accessible.name: "Finish"
        onClicked: nav.finish()
    }
}
