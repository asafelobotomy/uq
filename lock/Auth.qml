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
    // {text, error} from lock.js messageView: the latest password-side PAM message.
    property var message: ({ text: "", error: false })
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

    // Enter in the field. The caller clears the field right after.
    function submit(typed) {
        if (done || typeof typed !== "string" || typed === "")
            return;
        if (pw.active) {
            // A context waiting on its echo-off prompt takes this answer; anything else, none.
            var r = L.passwordReply(pw.responseRequired, pw.responseVisible, typed);
            if (r !== null)
                pw.respond(r);
            return;
        }
        message = { text: "", error: false };
        pending = typed;
        if (!pw.start()) {
            pending = null;
            message = { text: "Could not start authentication", error: true };
        }
    }

    PamContext {
        id: pw
        config: "uq-lock"
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
        }
        onCompleted: function (result) {
            auth.pending = null;
            console.info("uq-lock: password", auth.resultName(result));
            if (result === PamResult.Success) {
                auth.succeed();
            } else if (!auth.message.error) {
                auth.message = { text: "Authentication failed", error: true };
            }
        }
        onError: function (error) {
            auth.pending = null;
            console.info("uq-lock: password error");
            if (!auth.message.error)
                auth.message = { text: "Authentication error", error: true };
        }
    }

    // --- fingerprint ---
    property double fprintStartedMs: -L.FPRINT_RETRY_MS

    PamContext {
        id: fp
        config: "uq-lock-fingerprint"
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
        fprintStartedMs = Date.now();
        fprintMessage = { text: "", error: false };
        if (!fp.start())
            fprintEnded();
    }

    // Failure, error, max tries, timeout or an aborted prompt: try again (after fprintd-list
    // re-confirms a finger), at most once every FPRINT_RETRY_MS. This also covers resume.
    function fprintEnded() {
        if (done)
            return;
        var wait = L.FPRINT_RETRY_MS - (Date.now() - fprintStartedMs);
        fpRetry.interval = Math.max(0, wait);
        fpRetry.restart();
    }

    Timer {
        id: fpRetry
        onTriggered: {
            if (auth.done || fp.active)
                return;
            if (!L.fprintCanRestart(auth.fprintStartedMs, Date.now())) {
                auth.fprintEnded();
                return;
            }
            fprintList.running = true;
        }
    }

    Process {
        id: fprintList
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
        }
        command: ["fprintd-list", auth.user]
        stdout: StdioCollector {
            onStreamFinished: {
                fprintList.outText = text;
                fprintList.gotOut = true;
                fprintList.conclude();
            }
        }
        onStarted: { gotExit = false; gotOut = false; }
        onExited: function (code) {
            exitCode = code;
            gotExit = true;
            conclude();
        }
    }

    Component.onCompleted: {
        if (user !== "")
            fprintList.running = true;
    }
}
