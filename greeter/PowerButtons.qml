import QtQuick
import "core"
import "logic/greeter.js" as G

// Suspend, Restart and Shut down. Each needs a second click (within G.POWER_CONFIRM_MS) to run;
// the first one changes its label to "Click again to …". `run(action)` is the confirmed request.
Row {
    id: power

    // {action, at} while a button is armed (greeter.js powerClick), else null.
    property var armed: null
    // Test hook: the clock the confirm window is measured with.
    property var now: function () { return Date.now(); }

    signal run(string action)

    spacing: Theme.gapIn * 2

    function click(action) {
        var r = G.powerClick(armed, action, now());
        armed = r.armed;
        if (armed)
            disarm.restart();
        if (r.run !== "")
            power.run(r.run);
    }

    Timer {
        id: disarm
        interval: G.POWER_CONFIRM_MS
        onTriggered: power.armed = null
    }

    Repeater {
        model: G.POWER_ACTIONS
        delegate: Button {
            required property string modelData
            objectName: "power-" + modelData
            label: power.armed && power.armed.action === modelData ? G.POWER_CONFIRM_LABELS[modelData]
                                                                   : G.POWER_LABELS[modelData]
            emphasis: power.armed !== null && power.armed.action === modelData
            onActivated: power.click(modelData)
        }
    }
}
