import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Services.Pam
import "logic/lock.js" as L

// The two PAM conversations: `uq-lock` (password, started on Enter) and `uq-lock-fingerprint`
// (started only when fprintd-list lists a finger; it never answers a prompt). Whichever succeeds
// first unlocks. Nothing is logged except result enum names: never the password, never a message.
Item {
    id: auth

    readonly property string user: Quickshell.env("USER") || ""
    // The PAM config directory; empty means /etc/pam.d (tests point it at temp stacks).
    property string pamDir: ""
    readonly property string configDirectory: pamDir !== "" ? pamDir : "/etc/pam.d"
    // {text, error} from lock.js messageView: the latest password-side PAM message.
    property var message: ({ text: "", error: false })
    // The password attempt's non-prompt PAM messages (messageView results), shown if it fails.
    property var attemptMessages: []
    // The fingerprint conversation's own latest message (shown with its hint).
    property var fprintMessage: ({ text: "", error: false })
    readonly property bool fprintRunning: fp.active
    readonly property bool busy: pw.active
    property bool done: false

    signal unlocked()

    // The typed password, held only until the context's echo-off prompt arrives; null otherwise.
    property var pending: null

    function resultName(r) {
        if (r === PamResult.Success) return "Success";
        if (r === PamResult.Failed) return "Failed";
        if (r === PamResult.Error) return "Error";
        if (r === PamResult.MaxTries) return "MaxTries";
        return "Unknown";
    }

    function succeed() {
        if (done)
            return;
        done = true;
        pending = null;
        if (pw.active) pw.abort();
        if (fp.active) fp.abort();
        fpRetry.stop();
        unlocked();
    }

    // Enter in the field. Returns true when the text was taken (the caller then clears the
    // field); false when refused, e.g. while an attempt is still being checked.
    function submit(typed) {
        if (done || typeof typed !== "string" || typed === "")
            return false;
        if (pw.active) {
            // A context waiting on its echo-off prompt takes this answer; anything else waits.
            var r = L.passwordReply(pw.responseRequired, pw.responseVisible, typed);
            if (r !== null) {
                pw.respond(r);
                return true;
            }
            message = { text: "Checking…", error: false };
            return false;
        }
        message = { text: "", error: false };
        attemptMessages = [];
        pending = typed;
        if (!pw.start()) {
            pending = null;
            message = { text: "Could not start authentication", error: true };
        }
        return true;
    }

    PamContext {
        id: pw
        config: "uq-lock"
        configDirectory: auth.configDirectory
        user: auth.user
        onPamMessage: {
            if (pw.responseRequired) {
                var r = L.passwordReply(pw.responseRequired, pw.responseVisible, auth.pending);
                auth.pending = null;
                if (r === null)
                    pw.abort();
                else
                    pw.respond(r);
                return;
            }
            auth.message = L.messageView(pw.message, pw.messageIsError);
            auth.attemptMessages = auth.attemptMessages.concat([auth.message]);
        }
        onCompleted: function (result) {
            auth.pending = null;
            console.info("uq-lock: password", auth.resultName(result));
            if (result === PamResult.Success)
                auth.succeed();
            else
                auth.message = L.failureView(auth.attemptMessages, "Authentication failed");
            auth.attemptMessages = [];
        }
        onError: function (error) {
            auth.pending = null;
            console.info("uq-lock: password error");
            auth.message = L.failureView(auth.attemptMessages, "Authentication error");
            auth.attemptMessages = [];
        }
    }

    // --- fingerprint ---
    // When fprintd-list was last run or the context last started: both are throttled by it.
    property double fprintAttemptMs: -L.FPRINT_RETRY_MS

    PamContext {
        id: fp
        config: "uq-lock-fingerprint"
        configDirectory: auth.configDirectory
        user: auth.user
        onPamMessage: {
            if (L.fprintAbortOnPrompt(fp.responseRequired)) {
                fp.abort();
                auth.fprintEnded();
                return;
            }
            auth.fprintMessage = L.messageView(fp.message, fp.messageIsError);
        }
        onCompleted: function (result) {
            console.info("uq-lock: fingerprint", auth.resultName(result));
            if (result === PamResult.Success)
                auth.succeed();
            else
                auth.fprintEnded();
        }
        onError: function (error) {
            console.info("uq-lock: fingerprint error");
            auth.fprintEnded();
        }
    }

    function startFprint() {
        if (done || fp.active)
            return;
        fprintAttemptMs = Date.now();
        fprintMessage = { text: "", error: false };
        if (!fp.start())
            fprintEnded();
    }

    // The context ended (failure, error, max tries, timeout, an aborted prompt), or fprintd-list
    // failed to start, exited non-zero or listed no finger: poll fprintd-list again, at most once
    // every FPRINT_RETRY_MS, for as long as the lock lasts. This also covers resume (fprintd
    // often isn't back yet on the first try).
    function fprintEnded() {
        if (done || fp.active)
            return;
        var wait = L.FPRINT_RETRY_MS - (Date.now() - fprintAttemptMs);
        fpRetry.interval = Math.max(0, wait);
        fpRetry.restart();
    }

    Timer {
        id: fpRetry
        onTriggered: {
            if (auth.done || fp.active || fprintList.running)
                return;
            if (!L.fprintCanRestart(auth.fprintAttemptMs, Date.now())) {
                auth.fprintEnded();
                return;
            }
            auth.listFingers();
        }
    }

    function listFingers() {
        fprintAttemptMs = Date.now();
        fprintList.didStart = false;
        fprintList.running = true;
    }

    Process {
        id: fprintList
        property bool didStart: false
        property bool gotExit: false
        property bool gotOut: false
        property int exitCode: -1
        property string outText: ""
        // Exit code and stdout arrive in either order; decide once both are in.
        function conclude() {
            if (!gotExit || !gotOut)
                return;
            gotExit = false;
            gotOut = false;
            if (L.fprintListed(outText, exitCode))
                auth.startFprint();
            else
                auth.fprintEnded();
        }
        command: ["fprintd-list", auth.user]
        stdout: StdioCollector {
            onStreamFinished: {
                fprintList.outText = text;
                fprintList.gotOut = true;
                fprintList.conclude();
            }
        }
        onStarted: { didStart = true; gotExit = false; gotOut = false; }
        onExited: function (code) {
            exitCode = code;
            gotExit = true;
            conclude();
        }
        // A failed start (fprintd not installed) reports only runningChanged: poll again later.
        onRunningChanged: {
            if (!running && !didStart)
                auth.fprintEnded();
        }
    }

    Component.onCompleted: {
        if (user !== "")
            listFingers();
    }
}
