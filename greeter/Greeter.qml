import QtQuick
import "core"
import "auth"
import "logic/greeter.js" as G

// The login form on the main monitor (spec phase 5 §2.4): clock, user list (or a user-name field),
// the password field or whatever PAM is asking for, messages, the session picker and the power
// buttons. Tab order: user → password → session → power; Enter logs in; Esc cancels.
// Plain QtQuick (no Quickshell), so qmltestrunner can drive it with a fake greetd.
FocusScope {
    id: greeter

    // greeter/Auth.qml (or anything with its API).
    required property var auth
    // [{name, label, avatar}] (greeter.js visibleUsers); empty when AccountsService is missing.
    property var users: []
    // greeter.toml's hide_users: a user-name field instead of the list.
    property bool hideUsers: false
    // [{id, name, command}] (greeter.js sessionList), uQ first.
    property var sessions: []
    // greeter.js parseState: last user and each user's last session.
    property var stateData: G.parseState("")
    property date date: new Date()
    property bool reduceMotion: false

    property string selectedUser: ""
    property int sessionIndex: 0

    readonly property bool listShown: !hideUsers && users.length > 0
    readonly property string userName: listShown ? selectedUser : nameField.text.trim()
    readonly property var session: sessionIndex >= 0 && sessionIndex < sessions.length ? sessions[sessionIndex] : null

    signal powerRequested(string action)

    // Pick the starting user and session; `preferred` is a user to keep selected.
    function start(preferred) {
        selectUser(G.initialUser(users, stateData, preferred), false);
        // The field is pre-filled only when the list is missing; "Hide user list" shows no name
        // unless that user's session just ended straight away.
        if (!listShown)
            nameField.text = preferred ? preferred : (hideUsers ? "" : stateData.lastUser);
        focusField();
    }

    function selectUser(name, cancelFirst) {
        if (cancelFirst && name !== selectedUser)
            auth.cancel(name);
        selectedUser = name;
        var i = G.sessionIndexFor(sessions, stateData, name);
        sessionIndex = i < 0 ? 0 : i;
        clearFields();
    }

    function clearFields() {
        mainField.clear();
        visibleField.clear();
        newField.clear();
        confirmField.clear();
    }

    function focusField() {
        if (auth.mode === "new")
            newField.forceActiveFocus();
        else if (auth.mode === "visible")
            visibleField.forceActiveFocus();
        else if (!listShown && nameField.text.trim() === "")
            nameField.forceActiveFocus();
        else
            mainField.forceActiveFocus();
    }

    // Enter on the main field: start logging in, or answer an echo-off prompt.
    function submitMain() {
        if (auth.mode === "busy")
            return;
        if (auth.mode === "secret")
            auth.answer(mainField.text);
        else if (userName !== "")
            auth.submit(userName, mainField.text);
        else {
            nameField.forceActiveFocus();
            return;
        }
        mainField.clear();
    }

    function submitNew() {
        auth.answerNew(newField.text, confirmField.text);
        newField.clear();
        confirmField.clear();
        newField.forceActiveFocus();
    }

    function cancel() {
        auth.cancel("");
        clearFields();
        focusField();
    }

    onSessionsChanged: if (session === null && sessions.length > 0) sessionIndex = 0
    Binding {
        target: greeter.auth
        property: "session"
        value: greeter.session
    }
    Connections {
        target: greeter.auth
        function onModeChanged() { Qt.callLater(greeter.focusField); }
    }
    Keys.onEscapePressed: cancel()

    Clock {
        id: clockView
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.top: parent.top
        anchors.topMargin: Math.max(Theme.gapOut * 2, parent.height * 0.08)
        date: greeter.date
    }

    Rectangle {
        id: card
        anchors.centerIn: parent
        width: Math.min(Math.max(380, userList.implicitWidth + Theme.gapOut * 4), parent.width - Theme.gapOut * 2)
        height: form.implicitHeight + Theme.gapOut * 4
        radius: Theme.radius
        color: Theme.surface
        opacity: 0.94
        border.color: Theme.border
        border.width: Theme.borderWidth
    }

    Column {
        id: form
        anchors.centerIn: card
        width: card.width - Theme.gapOut * 4
        spacing: Theme.gapOut

        UserList {
            id: userList
            objectName: "userList"
            anchors.horizontalCenter: parent.horizontalCenter
            visible: greeter.listShown
            height: visible ? implicitHeight : 0
            users: greeter.users
            selected: greeter.selectedUser
            onChosen: name => {
                greeter.selectUser(name, true);
                greeter.focusField();
            }
        }

        UqInput {
            id: nameField
            objectName: "nameField"
            visible: !greeter.listShown
            width: parent.width
            implicitHeight: 36
            placeholder: "User name"
            Accessible.name: "User name"
            onEdited: text => {
                if (greeter.auth.mode !== "password")
                    greeter.auth.cancel(text.trim());
            }
            onAccepted: mainField.forceActiveFocus()
        }

        UqText {
            objectName: "promptLabel"
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            visible: greeter.auth.mode === "visible" || greeter.auth.mode === "new"
            text: greeter.auth.mode === "new" ? "Your password has expired. Choose a new one."
                                              : greeter.auth.promptText
            wrapMode: Text.Wrap
            elide: Text.ElideNone
        }

        PasswordField {
            id: mainField
            objectName: "passwordField"
            visible: greeter.auth.mode === "password" || greeter.auth.mode === "secret" || greeter.auth.mode === "busy"
            width: parent.width
            busy: greeter.auth.mode === "busy"
            prompt: greeter.auth.mode === "secret" ? greeter.auth.promptText : "Password"
            onAccepted: greeter.submitMain()
        }

        UqInput {
            id: visibleField
            objectName: "visibleField"
            visible: greeter.auth.mode === "visible"
            width: parent.width
            implicitHeight: 36
            placeholder: greeter.auth.promptText
            Accessible.name: greeter.auth.promptText
            onAccepted: {
                greeter.auth.answer(visibleField.text);
                visibleField.clear();
            }
        }

        PasswordField {
            id: newField
            objectName: "newField"
            visible: greeter.auth.mode === "new"
            width: parent.width
            prompt: "New password"
            onAccepted: confirmField.forceActiveFocus()
        }

        PasswordField {
            id: confirmField
            objectName: "confirmField"
            visible: greeter.auth.mode === "new"
            width: parent.width
            prompt: "Confirm"
            onAccepted: greeter.submitNew()
        }

        MessageView {
            objectName: "message"
            width: parent.width
            message: greeter.auth.message
        }
    }

    Row {
        anchors.bottom: parent.bottom
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottomMargin: Theme.gapOut * 2
        spacing: Theme.gapOut * 2

        SessionPicker {
            id: picker
            objectName: "sessionPicker"
            sessions: greeter.sessions
            currentIndex: greeter.sessionIndex
            onPicked: index => greeter.sessionIndex = index
        }

        PowerButtons {
            id: power
            objectName: "power"
            onRun: action => greeter.powerRequested(action)
        }
    }
}
