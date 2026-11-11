import QtQuick
import "../core"

// The password field: a secret UqInput (echo off, ImhSensitiveData | ImhNoPredictiveText |
// ImhHiddenText, shortcuts not typed in). Callers clear it after every attempt.
UqInput {
    id: field

    // Shown while an attempt is being checked.
    property bool busy: false
    // The placeholder when idle ("Password", or a PAM prompt such as "New password").
    property string prompt: "Password"

    implicitHeight: 36
    secret: true
    placeholder: busy ? "Checking…" : prompt
    Accessible.name: prompt
}
