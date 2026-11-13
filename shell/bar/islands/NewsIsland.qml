import QtQuick
import QtQuick.Layouts
import "../../core"
import "../../logic/ticker.js" as T
import "../../logic/barlayout.js" as L
import ".."

// News headlines (`news`): headlines from the chosen RSS/Atom feeds scroll by. The popup lists
// the newest ones (click or Enter opens one in the browser) and edits the feeds: suggested
// ones, a feed's own https URL, remove. The bridge fetches and parses the feeds; headlines are
// plain text, and only links the bridge sent can be opened.
Island {
    id: root

    readonly property var ev: Bridge.news
    readonly property var items: T.headlines(ev)
    readonly property var feeds: (BarLayout.options.news || {}).feeds || []
    readonly property var feedInfo: ev && Array.isArray(ev.feeds) ? ev.feeds : []
    readonly property string note: T.state(ev, now)
    property double now: Date.now()

    implicitWidth: (feeds.length === 0 ? empty.implicitWidth : L.textWidth(size, Theme.fontSize) * 1.5)
        + Theme.gapIn * 2
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
    function feedTitle(url) {
        for (var i = 0; i < feedInfo.length; i++)
            if (feedInfo[i] && feedInfo[i].url === url && typeof feedInfo[i].title === "string"
                    && feedInfo[i].title !== "")
                return feedInfo[i].title.slice(0, 80);
        for (var j = 0; j < T.PRESET_FEEDS.length; j++)
            if (T.PRESET_FEEDS[j].url === url)
                return T.PRESET_FEEDS[j].name;
        return url;
    }
    function feedFailed(url) {
        for (var i = 0; i < feedInfo.length; i++)
            if (feedInfo[i] && feedInfo[i].url === url)
                return feedInfo[i].ok === false;
        return false;
    }
    function setFeeds(list) {
        BarLayout.setOptions("news", { feeds: list });
    }

    UqText {
        id: empty
        visible: root.feeds.length === 0
        anchors.verticalCenter: parent.verticalCenter
        text: "Pick news feeds"
        color: Theme.textMuted
    }

    Marquee {
        visible: root.feeds.length > 0
        anchors.fill: parent
        model: root.items.length > 0 ? root.items.slice(0, 10)
            : [{ title: root.note || "Loading…", source: "" }]
        delegate: Row {
            required property var modelData
            spacing: Theme.gapIn
            UqText {
                visible: modelData.source !== ""
                text: modelData.source
                color: Theme.textMuted
            }
            UqText {
                text: modelData.title
                elide: Text.ElideNone
            }
        }
    }

    MouseArea {
        anchors.fill: parent
        onClicked: {
            if (popup.open && popup.anchorItem === root)
                popup.hide();
            else
                popup.show(root, details);
        }
    }

    IslandPopup {
        id: popup
    }

    Component {
        id: details
        ColumnLayout {
            id: col
            spacing: Theme.gapIn
            width: Theme.fontSize * 36
            property bool editing: root.feeds.length === 0
            property string addError: ""
            property bool checking: false

            RowLayout {
                Layout.fillWidth: true
                UqText {
                    text: col.editing ? "News feeds" : "Headlines"
                    font.bold: true
                    Layout.fillWidth: true
                }
                MenuRow {
                    id: toggle
                    visible: root.feeds.length > 0
                    text: col.editing ? "Show headlines" : "Edit feeds"
                    onActivated: col.editing = !col.editing
                }
            }
            UqText {
                visible: root.note !== "" && !col.editing
                text: root.note
                color: Theme.textMuted
            }

            // Headlines
            ColumnLayout {
                visible: !col.editing
                spacing: 0
                Layout.fillWidth: true
                Repeater {
                    id: heads
                    model: col.editing ? [] : root.items.slice(0, 20)
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
                    text: root.note !== "" ? root.note : "No headlines yet"
                    color: Theme.textMuted
                }
            }

            // Feeds
            ColumnLayout {
                visible: col.editing
                spacing: Theme.gapIn
                Layout.fillWidth: true
                Repeater {
                    model: col.editing ? root.feeds : []
                    delegate: RowLayout {
                        id: frow
                        required property string modelData
                        Layout.fillWidth: true
                        UqText {
                            text: root.feedTitle(frow.modelData)
                                + (root.feedFailed(frow.modelData) ? " (can't be read)" : "")
                            Layout.fillWidth: true
                        }
                        MenuRow {
                            text: "Remove"
                            Accessible.name: "Remove " + root.feedTitle(frow.modelData)
                            onActivated: root.setFeeds(root.feeds.filter(function (f) { return f !== frow.modelData; }))
                        }
                    }
                }
                UqText {
                    text: "Suggested"
                    color: Theme.textMuted
                    visible: suggestions.count > 0
                }
                Repeater {
                    id: suggestions
                    model: col.editing ? T.PRESET_FEEDS.filter(function (p) { return root.feeds.indexOf(p.url) < 0; }) : []
                    delegate: MenuRow {
                        required property var modelData
                        required property int index
                        Layout.fillWidth: true
                        text: "Add " + modelData.name
                        enabled: root.feeds.length < 8
                        onActivated: root.setFeeds(root.feeds.concat([modelData.url]))
                        Component.onCompleted: if (index === 0 && root.feeds.length === 0) forceActiveFocus()
                    }
                }
                RowLayout {
                    Layout.fillWidth: true
                    UqInput {
                        id: urlInput
                        Layout.fillWidth: true
                        placeholder: "https://… feed address"
                        maximumLength: 512
                        Accessible.name: "Feed address"
                        onAccepted: addUrl.activated()
                    }
                    MenuRow {
                        id: addUrl
                        text: col.checking ? "Checking…" : "Add"
                        onActivated: {
                            var u = urlInput.text.trim();
                            if (!T.validFeed(u)) {
                                col.addError = "Use the feed's https:// address";
                                return;
                            }
                            if (root.feeds.indexOf(u) >= 0 || root.feeds.length >= 8) {
                                col.addError = root.feeds.length >= 8 ? "At most 8 feeds" : "Already added";
                                return;
                            }
                            col.checking = true;
                            col.addError = "";
                            Bridge.call("PreviewFeed", [u]).then(function () {
                                col.checking = false;
                                root.setFeeds(root.feeds.concat([u]));
                                urlInput.clear();
                            }).catch(function (e) {
                                col.checking = false;
                                col.addError = "That address isn't a feed uQ can read";
                            });
                        }
                    }
                }
                UqText {
                    visible: col.addError !== ""
                    text: col.addError
                    color: Theme.textMuted
                }
                UqText {
                    text: "uQ asks only these sites for their headlines."
                    color: Theme.textMuted
                    wrapMode: Text.WordWrap
                    Layout.fillWidth: true
                }
            }
        }
    }
}
