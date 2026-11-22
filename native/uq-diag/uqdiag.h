// uq-diag 1.0.0
//
// uQ's log format and crash records for a Qt 6 C++ app. Vendor this file and uqdiag.cpp
// unchanged (see uQ's native/uq-diag/README.md); they need only Qt Core.
//
// uqdiag::init("reelo", APP_VERSION) at the top of main():
// - Qt's messages (qWarning, QML's console.warn) go to stderr the way every uQ program logs:
//   under systemd each line starts with its syslog priority (`<4>reelo: [REELO-003] …`), so
//   journald files it at the right level; in a terminal it reads `WARN reelo: [REELO-003] …`.
// - A crash (SIGSEGV, SIGABRT, SIGBUS, SIGFPE, SIGILL) leaves a record in uQ's crash folder,
//   $XDG_STATE_HOME/uq/crashes/<program>-<time>-<pid>/ (crash.json and backtrace.txt), before
//   the program dies as it would have. uQ notices it, says so in a notification and lists it
//   on Settings › Problems with a report people can copy.
//
// Start every warning with your app's code in brackets, `[REELO-003] …` (in QML,
// `console.warn("reelo[REELO-003] …")`), and list the codes in /usr/share/uq/diag/<app>.toml.
#pragma once

#include <QString>
#include <QtGlobal>

namespace uqdiag {

/// The version of these files; check-uq-diag.sh compares it with the one a repo pins.
inline constexpr const char *VERSION = "1.0.0";

/// Install the message handler and the crash handlers. Call once, before QGuiApplication.
/// `program` and `version` are copied.
void init(const char *program, const char *version);

/// The first `[AREA-NNN]` in `message`, or an empty string.
QString codeIn(const QString &message);

/// $XDG_STATE_HOME/uq/crashes (only an absolute XDG_STATE_HOME counts), else
/// ~/.local/state/uq/crashes.
QString crashDir();

/// The line `init`'s handler writes for one message (without the newline): for tests.
QString formatLine(QtMsgType type, const QString &program, const QString &message, bool journal);

} // namespace uqdiag
