import QtQuick
import QtTest
import "../../shell/core"
import "../../shell/auth"
import "../../shell/logic/auth.js" as A
import "../../shell/logic/lock.js" as L

// shell/auth/: the logic the lock screen and the greeter share, and its components.
TestCase {
    name: "auth"
    width: 400; height: 400
    when: windowShown

    Component { id: clockC; Clock {} }
    Component { id: avatarC; Avatar {} }
    Component { id: fieldC; PasswordField {} }
    Component { id: msgC; MessageView {} }

    function inner(f) {
        for (var i = 0; i < f.children.length; i++)
            if (f.children[i].echoMode !== undefined)
                return f.children[i];
        return null;
    }

    function test_lock_reexports_the_shared_logic() {
        compare(L.MAX_MESSAGE, A.MAX_MESSAGE);
        compare(L.FAILURE_LINES, A.FAILURE_LINES);
        compare(L.messageView("x\ny", true), A.messageView("x\ny", true));
        compare(L.failureView([], "f"), A.failureView([], "f"));
        compare(L.avatarSource("/a b", false, "", "n"), A.avatarSource("/a b", false, "", "n"));
        compare(L.passwordReply(true, false, "p"), A.passwordReply(true, false, "p"));
        compare(L.initialOf("ömer"), A.initialOf("ömer"));
    }

    function test_message_view_is_plain_and_bounded() {
        compare(A.messageView("<b>hi</b>‮there", true), { text: "<b>hi</b> there", error: true });
        compare(A.messageView("", true).error, false);
        compare(A.messageView(new Array(400).join("z"), false).text.length, A.MAX_MESSAGE);
    }

    function test_clock_formats_the_given_date() {
        var c = createTemporaryObject(clockC, this, { date: new Date(2026, 10, 11, 9, 5) });
        compare(c.children[0].text, "09:05");
        verify(c.children[1].text.indexOf("11") >= 0);
        compare(c.children[0].textFormat, Text.PlainText);
    }

    function test_avatar_initial_and_image() {
        var a = createTemporaryObject(avatarC, this, { avatar: { initial: "S" }, userName: "solon" });
        compare(a.hasImage, false);
        compare(a.children[1].text, "S");
        compare(a.Accessible.name, "Avatar for solon");
        var b = createTemporaryObject(avatarC, this, { avatar: { path: "file:///nonexistent.png" } });
        compare(b.hasImage, true);
        compare(b.children[1].text, "");
        compare(b.size, 96);
    }

    function test_password_field_is_secret() {
        var f = createTemporaryObject(fieldC, this);
        var t = inner(f);
        compare(f.secret, true);
        compare(t.echoMode, TextInput.Password);
        compare(t.inputMethodHints, Qt.ImhSensitiveData | Qt.ImhNoPredictiveText | Qt.ImhHiddenText);
        compare(f.placeholder, "Password");
        f.busy = true;
        compare(f.placeholder, "Checking…");
        f.busy = false;
        f.prompt = "New password";
        compare(f.placeholder, "New password");
        f.text = "secret";
        f.clear();
        compare(f.text, "");
    }

    function test_message_view() {
        var m = createTemporaryObject(msgC, this);
        compare(m.text, "");
        compare(m.textFormat, Text.PlainText);
        m.message = { text: "<i>Locked</i>", error: true };
        compare(m.text, "<i>Locked</i>");
        compare(m.color, Theme.error);
        compare(m.Accessible.name, "Message <i>Locked</i>");
        m.message = { text: "", error: false };
        m.fallback = "Or touch the fingerprint sensor";
        compare(m.text, "Or touch the fingerprint sensor");
        compare(m.color, Theme.textMuted);
    }
}
