import QtQuick
import "../../core"
import "../../logic/sanitize.js" as S

// The only StyledText in the shell (spec §2.5). It takes the raw, untrusted notification body
// and renders only what logic/sanitize.js lets through: <b> <i> <u> <br> and <a href>. A link
// shows its target under the body on hover and is never followed. scripts/lint-shell.sh allows
// a Text in this file and nowhere else outside shell/core/UqText.qml.
Column {
    id: root

    property string raw: ""
    property string hoveredLink: ""

    spacing: 2

    Text {
        id: body
        width: root.width
        text: S.sanitize(root.raw)
        textFormat: Text.StyledText
        wrapMode: Text.Wrap
        elide: Text.ElideRight
        color: Theme.text
        linkColor: Theme.accent
        font.family: Theme.fontUi
        font.pixelSize: Theme.fontSize
        Accessible.name: S.plainBody(root.raw)

        onLinkHovered: link => root.hoveredLink = link
        onLinkActivated: link => {}   // never followed
    }

    UqText {
        width: root.width
        visible: root.hoveredLink !== ""
        text: root.hoveredLink
        color: Theme.textMuted
        font.pixelSize: Math.max(1, Theme.fontSize - 1)
    }
}
