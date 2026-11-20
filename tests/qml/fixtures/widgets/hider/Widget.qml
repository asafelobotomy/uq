import QtQuick

// A test widget that hides itself until told otherwise (an update icon with nothing to update).
Item {
    id: w
    property var ctx
    property bool hidden: true
}
