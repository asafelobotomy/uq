import QtQuick

// A page that reads again when asked (refresh) and may say it isn't to be kept alive.
Item {
    property int refreshes: 0
    property bool keepAlive: true
    function refresh() {
        refreshes += 1;
    }
}
