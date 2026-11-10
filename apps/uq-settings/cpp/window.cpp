#include "uq-settings/cpp/window.h"

#include <QDir>
#include <QGuiApplication>
#include <QIcon>
#include <QQuickWindow>
#include <QStandardPaths>
#include <QString>

namespace {
QString toQString(rust::Str s)
{
    return QString::fromUtf8(s.data(), static_cast<qsizetype>(s.size()));
}
}

void uqs_describe_application(rust::Str version)
{
    QGuiApplication::setApplicationName(QStringLiteral("uq-settings"));
    QGuiApplication::setApplicationDisplayName(QStringLiteral("uQ Settings"));
    QGuiApplication::setApplicationVersion(toQString(version));
    // The Wayland app id; matches uq-settings.desktop.
    QGuiApplication::setDesktopFileName(QStringLiteral("uq-settings"));

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

bool uqs_has_window()
{
    return !QGuiApplication::topLevelWindows().isEmpty();
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
