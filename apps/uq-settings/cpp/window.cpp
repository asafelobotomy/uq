#include "uq-settings/cpp/window.h"
#include "uq-settings/src/window.cxx.h"

#include <KColorScheme>
#include <KSharedConfig>
#include <QApplication>
#include <QDir>
#include <QFile>
#include <QFont>
#include <QGuiApplication>
#include <QIcon>
#include <QQmlApplicationEngine>
#include <QQuickStyle>
#include <QQuickWindow>
#include <QStandardPaths>
#include <QString>

namespace {
QString toQString(rust::Str s)
{
    return QString::fromUtf8(s.data(), static_cast<qsizetype>(s.size()));
}

QString toQString(const rust::String &s)
{
    return QString::fromUtf8(s.data(), static_cast<qsizetype>(s.size()));
}

bool iconThemeInstalled(const QString &theme)
{
    for (const QString &dir : QIcon::themeSearchPaths()) {
        if (QDir(dir).exists(theme + QStringLiteral("/index.theme")))
            return true;
    }
    return false;
}

// qqc2-desktop-style when it is installed: it draws through the QStyle, so Kvantum themes the
// controls. Without it the app would fail to load at all; Fusion follows the palette instead.
void chooseStyle(const QQmlApplicationEngine &engine)
{
    if (qEnvironmentVariableIsSet("QT_QUICK_CONTROLS_STYLE"))
        return;
    for (const QString &dir : engine.importPathList()) {
        if (QFile::exists(dir + QStringLiteral("/org/kde/desktop/qmldir"))) {
            QQuickStyle::setStyle(QStringLiteral("org.kde.desktop"));
            return;
        }
    }
    qWarning("uq-settings: qqc2-desktop-style isn't installed; using the Fusion style");
    QQuickStyle::setStyle(QStringLiteral("Fusion"));
}

void describeApplication(rust::Str version)
{
    QGuiApplication::setApplicationName(QStringLiteral("uq-settings"));
    QGuiApplication::setApplicationDisplayName(QStringLiteral("uQ Settings"));
    QGuiApplication::setApplicationVersion(toQString(version));
    // The Wayland app id; matches packaging/org.uq.Settings.desktop.
    QGuiApplication::setDesktopFileName(QStringLiteral("org.uq.Settings"));

    // Outside a full desktop (Hyprland, the offscreen screenshot run) Qt may search no icon
    // folders: add the XDG ones. The theme is uQ's (`uqs_apply_look`) or a fallback.
    QStringList paths = QIcon::themeSearchPaths();
    for (const QString &dir : QStandardPaths::locateAll(QStandardPaths::GenericDataLocation, QStringLiteral("icons"),
                                                        QStandardPaths::LocateDirectory)) {
        if (!paths.contains(dir))
            paths.append(dir);
    }
    QIcon::setThemeSearchPaths(paths);
}

// Without a theme from uQ: keep Qt's (qt6ct's) icon theme, or use the first one installed.
void fallbackIconTheme()
{
    const QString current = QIcon::themeName();
    const bool none = current.isEmpty() || current == QStringLiteral("hicolor");
    for (const auto &theme : {QStringLiteral("breeze"), QStringLiteral("breeze-dark"), QStringLiteral("Papirus"),
                              QStringLiteral("Adwaita")}) {
        if (!iconThemeInstalled(theme))
            continue;
        if (none)
            QIcon::setThemeName(theme);
        else
            QIcon::setFallbackThemeName(theme);
        break;
    }
}

} // namespace

void uqs_apply_look(const UqsLook &look)
{
    // Kirigami's colours come from KColorScheme, which reads the scheme named by this property
    // (else kdeglobals, which outside Plasma means Breeze Light). The file keeps its path across
    // theme changes, so re-read it before the palette change makes Kirigami fetch the colours.
    const QString colors = toQString(look.colors);
    if (!colors.isEmpty() && QFile::exists(colors)) {
        qApp->setProperty("KDE_COLOR_SCHEME_PATH", colors);
        KSharedConfigPtr config = KSharedConfig::openConfig(colors);
        config->reparseConfiguration();
        QApplication::setPalette(KColorScheme::createApplicationPalette(config));
    }

    const QString icons = toQString(look.icon_theme);
    if (!icons.isEmpty() && iconThemeInstalled(icons))
        QIcon::setThemeName(icons);
    else
        fallbackIconTheme();

    const QString family = toQString(look.font);
    if (!family.isEmpty() || look.point_size > 0) {
        QFont font = QApplication::font();
        if (!family.isEmpty())
            font.setFamily(family);
        if (look.point_size > 0)
            font.setPointSize(look.point_size);
        QApplication::setFont(font);
    }
}

int uqs_run(rust::Str version, rust::Str url, const UqsLook &look)
{
    // Qt keeps references to argc/argv for the application's lifetime. The command line was
    // already parsed in Rust; Qt options come from the environment (QT_QPA_PLATFORM, ...).
    static int argc = 1;
    static char name[] = "uq-settings";
    static char *argv[] = {name, nullptr};
    QApplication app(argc, argv);
    describeApplication(version);
    uqs_apply_look(look);
    QQmlApplicationEngine engine;
    chooseStyle(engine);
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
