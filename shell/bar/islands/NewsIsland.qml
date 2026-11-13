import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Widgets
import "../../core"
import "../../logic/ticker.js" as T
import ".."

// News headlines (`news`). Icon: a news icon. Small: headlines scrolling. Medium and Large: with
// their source. A click lists the newest (click or Enter opens one in the browser); the feeds
// are chosen in edit mode. The bridge fetches and parses the feeds; headlines are plain text,
// and only links the bridge sent can be opened.
Island {
    id: root

    readonly property var ev: Bridge.news
    readonly property var items: T.headlines(ev)
    readonly property var feeds: (BarLayout.options.news || {}).feeds || []
    readonly property string note: T.state(ev, now)
    readonly property bool withSource: size === "medium" || size === "large"
    property double now: Date.now()

    Accessible.role: Accessible.Button
    Accessible.name: items.length > 0 ? "News: " + items[0].title : "News"

    Timer {
        interval: 60000
        repeat: true
        running: popup.open || root.note !== ""
        onTriggered: root.now = Date.now()
    }

    function open(link) {
        Bridge.call("OpenLink", [link]).catch(function (e) {
            console.warn("uq: news link: " + e);
        });
        popup.hide();
    }

    IconImage {
        visible: root.isIcon
        anchors.centerIn: parent
        implicitSize: Theme.fontSize + 7
        source: Quickshell.iconPath("application-rss+xml", "news-subscribe")
    }

    UqText {
        visible: !root.isIcon && root.feeds.length === 0
        anchors.fill: parent
        verticalAlignment: Text.AlignVCenter
        text: "No feeds"
        color: Theme.textMuted
    }

    Marquee {
        visible: !root.isIcon && root.feeds.length > 0
        anchors.fill: parent
        model: root.items.length > 0 ? root.items.slice(0, 10)
            : [{ title: root.note || "Loading…", source: "" }]
        delegate: Row {
            required property var modelData
            spacing: Theme.gapIn
            UqText {
                visible: root.withSource && modelData.source !== ""
                text: modelData.source
                color: Theme.textMuted
                elide: Text.ElideNone
            }
            UqText {
                text: modelData.title
                elide: Text.ElideNone
            }
        }
    }

    MouseArea {
        anchors.fill: parent
        onClicked: popup.open && popup.anchorItem === root ? popup.hide() : popup.show(root, details)
    }

    LazyPopup {
        id: popup
    }

    Component {
        id: details
        ColumnLayout {
            spacing: Theme.gapIn
            width: Theme.fontSize * 36

            UqText {
                text: "Headlines"
                baseBold: true
            }
            UqText {
                visible: root.note !== ""
                text: root.note
                color: Theme.textMuted
            }
            ColumnLayout {
                spacing: 0
                Layout.fillWidth: true
                Repeater {
                    model: root.items.slice(0, 20)
                    delegate: MenuRow {
                        required property var modelData
                        required property int index
                        Layout.fillWidth: true
                        text: modelData.title
                        detail: [modelData.source, T.age(modelData.published, root.now)]
                            .filter(function (s) { return s !== ""; }).join(" · ")
                        onActivated: root.open(modelData.link)
                        Component.onCompleted: if (index === 0) forceActiveFocus()
                    }
                }
                UqText {
                    visible: root.items.length === 0
                    text: root.feeds.length === 0 ? "No feeds chosen yet."
                        : (root.note !== "" ? root.note : "No headlines yet")
                    color: Theme.textMuted
                }
            }
            MenuRow {
                id: change
                Layout.fillWidth: true
                text: "Change the feeds…"
                onActivated: {
                    popup.hide();
                    root.editRequested();
                }
                Component.onCompleted: if (root.items.length === 0) forceActiveFocus()
            }
        }
    }
}
