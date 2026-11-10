#pragma once

#include "rust/cxx.h"

// Create the QApplication (a widgets app: qqc2-desktop-style draws through a QStyle, which is how
// Kvantum themes the controls), load `url` and run the event loop. Returns the exit code, or -1
// when QML created no window (Qt Quick or Kirigami missing).
int uqs_run(rust::Str version, rust::Str url);
// Render the first visible Qt Quick window to a PNG (`--screenshots`).
bool uqs_grab_window(rust::Str path);
