#include "uq-settings/cpp/window.h"

#include <QApplication>
#include <QDir>
#include <QGuiApplication>
#include <QIcon>
#include <QQmlApplicationEngine>
#include <QQuickWindow>
#include <QStandardPaths>
#include <QString>

namespace {
QString toQString(rust::Str s)
{
    return QString::fromUtf8(s.data(), static_cast<qsizetype>(s.size()));
}

void describeApplication(rust::Str version)
{
    QGuiApplication::setApplicationName(QStringLiteral("uq-settings"));
    QGuiApplication::setApplicationDisplayName(QStringLiteral("uQ Settings"));
    QGuiApplication::setApplicationVersion(toQString(version));
    // The Wayland app id; matches packaging/org.uq.Settings.desktop.
    QGuiApplication::setDesktopFileName(QStringLiteral("org.uq.Settings"));

    // Outside a full desktop (Hyprland, the offscreen screenshot run) Qt may search no icon
    // folders and have no icon theme: add the XDG ones, then use the first installed theme.
    QStringList paths = QIcon::themeSearchPaths();
    for (const QString &dir : QStandardPaths::locateAll(QStandardPaths::GenericDataLocation, QStringLiteral("icons"),
                                                        QStandardPaths::LocateDirectory)) {
        if (!paths.contains(dir))
            paths.append(dir);
    }
    QIcon::setThemeSearchPaths(paths);
    const QString current = QIcon::themeName();
    const bool none = current.isEmpty() || current == QStringLiteral("hicolor");
    for (const auto &theme : {QStringLiteral("breeze"), QStringLiteral("breeze-dark"), QStringLiteral("Papirus"),
                              QStringLiteral("Adwaita")}) {
        bool found = false;
        for (const QString &dir : QIcon::themeSearchPaths())
            found = found || QDir(dir).exists(theme + QStringLiteral("/index.theme"));
        if (!found)
            continue;
        if (none)
            QIcon::setThemeName(theme);
        else
            QIcon::setFallbackThemeName(theme);
        break;
    }
}

} // namespace

int uqs_run(rust::Str version, rust::Str url)
{
    // Qt keeps references to argc/argv for the application's lifetime. The command line was
    // already parsed in Rust; Qt options come from the environment (QT_QPA_PLATFORM, ...).
    static int argc = 1;
    static char name[] = "uq-settings";
    static char *argv[] = {name, nullptr};
    QApplication app(argc, argv);
    describeApplication(version);
    QQmlApplicationEngine engine;
    engine.load(QUrl(toQString(url)));
    if (QGuiApplication::topLevelWindows().isEmpty())
        return -1;
    return QApplication::exec();
}

bool uqs_grab_window(rust::Str path)
{
    const auto file = toQString(path);
    for (QWindow *w : QGuiApplication::topLevelWindows()) {
        if (auto *quick = qobject_cast<QQuickWindow *>(w); quick && quick->isVisible())
            return quick->grabWindow().save(file);
    }
    return false;
}
