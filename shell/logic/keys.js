.pragma library

// Which key presses a password field ignores. Pure: UqInput feeds in the event's modifiers and key.
// Shortcuts pressed over the lock reach its focused field (Hyprland doesn't run them while locked),
// and a TextInput would type SUPER+Space or SUPER+N as " n". So anything with Super (Meta) is
// dropped, and Ctrl combos other than the editing ones. Ctrl+Alt is AltGr on some layouts and types.

// Qt::KeyboardModifier and Qt::Key values (stable Qt ABI).
var CTRL = 0x04000000, ALT = 0x08000000, META = 0x10000000;
var EDIT_KEYS = [
    0x41,       // A: select all
    0x43,       // C
    0x56,       // V: paste
    0x58,       // X
    0x59,       // Y
    0x5a,       // Z
    0x01000003, // Backspace
    0x01000007, // Delete
    0x01000012, // Left
    0x01000014, // Right
    0x01000010, // Home
    0x01000011, // End
    0x01000004, // Return
    0x01000005  // Enter
];

function blocks(modifiers, key) {
    var m = Number(modifiers) || 0;
    if (m & META)
        return true;
    if ((m & CTRL) && !(m & ALT))
        return EDIT_KEYS.indexOf(Number(key)) < 0;
    return false;
}
