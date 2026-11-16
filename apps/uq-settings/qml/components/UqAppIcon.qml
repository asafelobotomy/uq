// An app's icon from the icon theme, by name, or a rounded tile with the app's first letter when
// the theme has no such icon. For settings pages (uq_api = 2): the name may come from an app's
// command output, so only a plain theme icon name is looked up, never a path or a URL.
import QtQuick
import org.kde.kirigami as Kirigami

Item {
    id: icon

    /// A theme icon name (`firefox`, `org.gimp.GIMP`). Anything else shows the letter tile.
    property string name: ""
    /// The app's name: its first letter stands in for a missing icon, and it is the accessible name.
    property string label: ""
    /// Width and height in pixels.
    property int size: Kirigami.Units.iconSizes.medium
    /// Whether the theme had the icon (false while the letter tile shows).
    // Kirigami's `valid` only says the source is set; `status` says whether the theme had it.
    readonly property bool found: icon.plainName && themed.status === Kirigami.Icon.Ready

    readonly property bool plainName: /^[A-Za-z0-9_+-][A-Za-z0-9._+-]{0,127}$/.test(icon.name)

    implicitWidth: icon.size
    implicitHeight: icon.size
    Accessible.role: Accessible.Graphic
    Accessible.name: icon.label

    Kirigami.Icon {
        id: themed
        anchors.fill: parent
        source: icon.plainName ? icon.name : ""
        // No stand-in icon: a name the theme lacks shows the letter tile instead.
        fallback: ""
        visible: icon.found
        Accessible.ignored: true
    }

    Rectangle {
        anchors.fill: parent
        visible: !icon.found
        radius: Math.round(width * 0.22)
        color: Kirigami.Theme.highlightColor
        UqLabel {
            anchors.centerIn: parent
            text: icon.label.length > 0 ? icon.label.charAt(0).toUpperCase() : "?"
            color: Kirigami.Theme.highlightedTextColor
            font.bold: true
            font.pixelSize: Math.round(icon.height * 0.45)
            Accessible.ignored: true
        }
    }
}
