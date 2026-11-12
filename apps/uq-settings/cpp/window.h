#pragma once

#include "rust/cxx.h"

// The look uqd rendered for the current theme (see `app::ThemeLook`). Empty strings and a size
// of 0 leave that part as Qt and qt6ct set it.
struct UqsLook;

// Create the QApplication (a widgets app: qqc2-desktop-style draws through a QStyle, which is how
// Kvantum themes the controls), apply `look`, load `url` and run the event loop. Returns the exit
// code, or -1 when QML created no window (Qt Quick or Kirigami missing).
int uqs_run(rust::Str version, rust::Str url, const UqsLook &look);
// Apply a theme's look to the running app: the KDE colour scheme (Kirigami's colours and the
// palette), the icon theme and the interface font.
void uqs_apply_look(const UqsLook &look);
// Render the first visible Qt Quick window to a PNG (`--screenshots`).
bool uqs_grab_window(rust::Str path);
