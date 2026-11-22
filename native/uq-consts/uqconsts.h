// uq-consts 1.0.0
//
// The values uQ, Bindle and uQ's native apps share. Generated from uQ's
// native/uq-consts/consts.toml by gen.py: vendor this header unchanged. Header-only; the path
// functions need Qt Core.
#pragma once

#include <QDir>
#include <QString>

namespace uqconsts {

inline constexpr const char *version = "1.0.0";

namespace paths {

inline constexpr const char *uqDir = "uq"; // relative to stateHome()
inline constexpr const char *themeDir = "uq/theme"; // relative to stateHome()
inline constexpr const char *tokens = "uq/theme/tokens.json"; // relative to stateHome()
inline constexpr const char *crashesDir = "uq/crashes"; // relative to stateHome()
inline constexpr const char *quickshellCrashesDir = "quickshell/crashes"; // relative to cacheHome()

inline QString base(const char *var, const char *homeRel)
{
    const QString v = qEnvironmentVariable(var);
    if (QDir::isAbsolutePath(v))
        return v;
    return QDir::homePath() + QLatin1Char('/') + QLatin1String(homeRel);
}

/// $XDG_STATE_HOME when it is absolute, else ~/.local/state.
inline QString stateHome() { return base("XDG_STATE_HOME", ".local/state"); }
/// $XDG_CACHE_HOME when it is absolute, else ~/.cache.
inline QString cacheHome() { return base("XDG_CACHE_HOME", ".cache"); }

inline QString uqDirPath() { return stateHome() + QLatin1Char('/') + QLatin1String(uqDir); }
inline QString themeDirPath() { return stateHome() + QLatin1Char('/') + QLatin1String(themeDir); }
inline QString tokensPath() { return stateHome() + QLatin1Char('/') + QLatin1String(tokens); }
inline QString crashesDirPath() { return stateHome() + QLatin1Char('/') + QLatin1String(crashesDir); }
inline QString quickshellCrashesDirPath() { return cacheHome() + QLatin1Char('/') + QLatin1String(quickshellCrashesDir); }

} // namespace paths

namespace install {
inline constexpr const char *dataDir = "/usr/share/uq";
inline constexpr const char *pagesDir = "/usr/share/uq/settings/pages";
inline constexpr const char *widgetsDir = "/usr/share/uq/shell/widgets";
inline constexpr const char *launcherDir = "/usr/share/uq/launcher";
inline constexpr const char *actionsDir = "/usr/share/uq/launcher/actions";
inline constexpr const char *integrationsDir = "/usr/share/uq/integrations";
inline constexpr const char *diagDir = "/usr/share/uq/diag";
inline constexpr const char *themesDir = "/usr/share/uq/themes";
inline constexpr const char *renderedThemesDir = "/usr/share/uq/themes/rendered";
} // namespace install

namespace dbus {
inline constexpr const char *desktopName = "org.uq.Desktop1";
inline constexpr const char *desktopPath = "/org/uq/Desktop1";
inline constexpr const char *desktopInterface = "org.uq.Desktop1";
inline constexpr const char *launcherProviderPath = "/org/uq/Launcher1";
inline constexpr const char *launcherProviderInterface = "org.uq.Launcher1.Provider";
} // namespace dbus

namespace api {
inline constexpr int uqApi = 3;
inline constexpr int pageDefaultApi = 1;
} // namespace api

namespace motion {
inline constexpr int fast = 100;
inline constexpr int base = 140;
inline constexpr int slow = 220;
inline constexpr double popScale = 0.96;
inline constexpr const char *easing = "OutCubic";
inline constexpr const char *easingExit = "InCubic";
} // namespace motion

namespace shape {
inline constexpr int fallbackRadius = 10;
inline constexpr int controlInset = 2;
inline constexpr int innerInset = 4;
inline constexpr int gap = 8;
} // namespace shape

namespace typeScale {
inline constexpr int fallbackSize = 11;
inline constexpr int smallOffset = 1;
inline constexpr int smallMin = 8;
inline constexpr double headingScale = 1.5;
} // namespace typeScale

/// uQ's Graphite theme, the palette to fall back to where uQ isn't running.
namespace palette {
struct Color {
    const char *key;
    const char *value;
};
inline constexpr Color dark[] = {
    {"bg", "#1f1f22"},
    {"surface", "#2b2b2e"},
    {"surface_alt", "#353538"},
    {"overlay", "#464649"},
    {"text", "#e4e4e7"},
    {"text_muted", "#a9a9ac"},
    {"accent", "#8ab4f8"},
    {"accent_text", "#1f1f22"},
    {"border", "#5a5a5d"},
    {"success", "#81c995"},
    {"warning", "#fdd663"},
    {"error", "#f28b82"},
};
inline constexpr Color light[] = {
    {"bg", "#f4f4f5"},
    {"surface", "#e7e7e8"},
    {"surface_alt", "#dcdcdd"},
    {"overlay", "#c8c8c9"},
    {"text", "#18181b"},
    {"text_muted", "#5a5a5c"},
    {"accent", "#2b5bb8"},
    {"accent_text", "#f4f4f5"},
    {"border", "#b2b2b4"},
    {"success", "#1e7a3a"},
    {"warning", "#8a6100"},
    {"error", "#b3261e"},
};
} // namespace palette

} // namespace uqconsts
