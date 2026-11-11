import QtQuick
import "core"
import "auth"

// The users to log in as (AccountsService, filtered by greeter.js visibleUsers): a row of
// avatars with plain-text names. Click, or the arrow keys, choose one.
ListView {
    id: list

    // [{name, label, avatar}] from greeter.js visibleUsers.
    property var users: []
    property string selected: ""
    // greeter.toml's reduce_motion: a flicked list stops at once instead of coasting, and
    // moving to another user scrolls without animating.
    property bool reduceMotion: false

    signal chosen(string name)

    function indexOf(name) {
        for (var i = 0; i < users.length; i++)
            if (users[i].name === name)
                return i;
        return -1;
    }
    function step(d) {
        if (users.length === 0)
            return;
        var i = indexOf(selected);
        var next = i < 0 ? 0 : Math.max(0, Math.min(users.length - 1, i + d));
        if (users[next].name !== selected)
            chosen(users[next].name);
    }

    orientation: ListView.Horizontal
    spacing: Theme.gapOut
    model: users
    currentIndex: indexOf(selected)
    implicitHeight: 120
    implicitWidth: Math.min(count * (88 + spacing), 600)
    clip: true
    boundsBehavior: Flickable.StopAtBounds
    activeFocusOnTab: true
    keyNavigationEnabled: false
    highlightFollowsCurrentItem: false
    Accessible.role: Accessible.List
    Accessible.name: "Users"
    Keys.onLeftPressed: step(-1)
    Keys.onRightPressed: step(1)

    Binding on flickDeceleration {
        when: list.reduceMotion
        value: 100000
    }
    Binding on highlightMoveDuration {
        when: list.reduceMotion
        value: 0
    }

    delegate: Item {
        id: tile
        required property var modelData
        required property int index
        readonly property bool isSelected: modelData.name === list.selected
        width: 88
        height: list.height
        Accessible.role: Accessible.ListItem
        Accessible.name: modelData.label
        Accessible.selected: isSelected

        Avatar {
            id: avatar
            anchors.horizontalCenter: parent.horizontalCenter
            size: tile.isSelected ? 72 : 56
            y: tile.isSelected ? 0 : 8
            avatar: tile.modelData.avatar
            userName: tile.modelData.label
            border.color: tile.isSelected ? Theme.accent : Theme.border
        }
        UqText {
            anchors.top: avatar.bottom
            anchors.topMargin: Theme.gapIn
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            text: tile.modelData.label
            color: tile.isSelected ? Theme.text : Theme.textMuted
        }
        MouseArea {
            anchors.fill: parent
            onClicked: list.chosen(tile.modelData.name)
        }
    }
}
