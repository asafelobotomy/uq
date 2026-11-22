// Tests for the generated C++ uq-consts header.
#include "../uqconsts.h"

#include <QTest>

#include <cstring>
#include <iterator>

class TestUqConsts : public QObject {
    Q_OBJECT
private slots:
    void absoluteStateHomeWins()
    {
        qputenv("HOME", "/home/a");
        qputenv("XDG_STATE_HOME", "/st");
        QCOMPARE(uqconsts::paths::tokensPath(), QStringLiteral("/st/uq/theme/tokens.json"));
    }
    void relativeStateHomeIsIgnored()
    {
        qputenv("HOME", "/home/a");
        qputenv("XDG_STATE_HOME", "rel");
        QCOMPARE(uqconsts::paths::stateHome(), QStringLiteral("/home/a/.local/state"));
        qunsetenv("XDG_STATE_HOME");
        QCOMPARE(uqconsts::paths::crashesDirPath(), QStringLiteral("/home/a/.local/state/uq/crashes"));
    }
    void cacheHome()
    {
        qputenv("HOME", "/home/a");
        qunsetenv("XDG_CACHE_HOME");
        QCOMPARE(uqconsts::paths::quickshellCrashesDirPath(),
                 QStringLiteral("/home/a/.cache/quickshell/crashes"));
    }
    void values()
    {
        QCOMPARE(QLatin1String(uqconsts::dbus::desktopName), QLatin1String("org.uq.Desktop1"));
        QVERIFY(uqconsts::motion::fast < uqconsts::motion::base);
        QCOMPARE(std::size(uqconsts::palette::dark), size_t(12));
        QCOMPARE(QLatin1String(uqconsts::palette::light[0].key), QLatin1String("bg"));
        QVERIFY(std::strncmp(uqconsts::install::pagesDir, uqconsts::install::dataDir,
                             std::strlen(uqconsts::install::dataDir)) == 0);
    }
};

QTEST_GUILESS_MAIN(TestUqConsts)
#include "tst_uqconsts.moc"
