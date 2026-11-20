import QtQuick
import Quickshell.Io

// Process wiring for Bridge. Instantiate once (shell.qml). Restart delays and the pending-call
// rejection on exit live in Bridge.
Item {
    id: io

    Process {
        id: proc
        // setpriv --pdeathsig: the unit keeps KillMode=process (apps the shell opened outlive a
        // shell restart), so a long-running helper has to end itself when the shell goes.
        command: ["/usr/bin/setpriv", "--pdeathsig", "TERM", "--", "uqctl", "bridge"]
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
