import QtQuick

// A plugin page that uses its context: the theme and one command on load.
Item {
    property var ctx
    property var result: null

    onCtxChanged: {
        if (ctx && result === null)
            ctx.command.run("status", [], r => result = r);
    }
}
