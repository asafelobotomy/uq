import QtQuick

// Loads fine, then throws from a callback it gave ctx: a runtime error.
Item {
    property var ctx
    function poke() {
        ctx.command.run("status", [], function () { throw new Error("boom"); });
    }
}
