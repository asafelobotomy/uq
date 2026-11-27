import QtQuick
import "../core"

// The card behind the login and unlock forms: the theme's surface, slightly see-through over the
// wallpaper, with the theme's radius and border. Sized by the caller (around its form).
Rectangle {
    radius: Theme.radius
    color: Theme.surface
    opacity: 0.94
    border.color: Theme.border
    border.width: Theme.borderWidth
}
