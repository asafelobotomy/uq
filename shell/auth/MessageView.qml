import QtQuick
import "../core"

// One PAM or status message ({text, error} from auth.js messageView / failureView), as plain
// text; hidden when empty. `fallback` is shown instead of an empty message when set.
UqText {
    id: view

    property var message: ({ text: "", error: false })
    property string fallback: ""
    // Read before the text by screen readers ("Message", "Fingerprint").
    property string label: "Message"

    readonly property string shownText: message && message.text ? message.text : fallback

    horizontalAlignment: Text.AlignHCenter
    wrapMode: Text.Wrap
    elide: Text.ElideNone
    visible: text !== ""
    text: shownText
    color: message && message.error && message.text ? Theme.error : Theme.textMuted
    Accessible.role: Accessible.StaticText
    Accessible.name: label + " " + text
}
