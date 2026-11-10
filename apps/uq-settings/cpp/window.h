#pragma once

#include "rust/cxx.h"

// Application name, version and Wayland app id, plus an icon theme for Kirigami.Icon outside a
// full desktop; call after the QGuiApplication exists.
void uqs_describe_application(rust::Str version);
// Whether QML created a window (false: Qt Quick or Kirigami missing).
bool uqs_has_window();
// Render the first visible Qt Quick window to a PNG (`--screenshots`).
bool uqs_grab_window(rust::Str path);
