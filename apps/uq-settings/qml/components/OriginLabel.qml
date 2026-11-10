// Where a history entry came from, in words that end a sentence ("Changed from the terminal").
// Plain text: a profile's name comes from uqd.
import QtQuick

UqLabel {
    /// The entry's origin as uqd journals it: settings, uqctl, shell, profile:<name>, restore,
    /// import or unknown.
    property string origin

    text: {
        switch (origin) {
        case "settings":
            return "here";
        case "uqctl":
            return "from the terminal";
        case "shell":
            return "from the panel";
        case "restore":
            return "by an undo or a restore";
        case "import":
            return "from an import";
        }
        if (origin.startsWith("profile:") && origin.length > "profile:".length)
            return "from the profile " + origin.slice("profile:".length);
        return "from somewhere else";
    }
}
