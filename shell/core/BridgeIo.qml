import QtQuick
import Quickshell.Io

// Process wiring for Bridge. Instantiate once (shell.qml). Restart delays and the pending-call
// rejection on exit live in Bridge.
Item {
    id: io

    Process {
        id: proc
        command: ["uqctl", "bridge"]
        stdinEnabled: true
        running: Bridge.processWanted
        stdout: SplitParser {
            onRead: data => Bridge.feed(data)
        }
        onStarted: {
            Bridge.writer = line => proc.write(line + "\n");
            Bridge.processStarted();
        }
        onExited: Bridge.processExited()
        // A command that fails to start (e.g. uqctl not on PATH) emits neither started nor
        // exited, only runningChanged; Bridge ignores the duplicate when exited also fires.
        onRunningChanged: if (!running) Bridge.processExited()
    }
}
