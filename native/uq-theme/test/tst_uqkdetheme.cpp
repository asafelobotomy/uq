// Tests for the vendored uqkdetheme: the palette read from a KDE colour scheme, and a live follow
// of uqd's theme/ folder being swapped.
#include "../uqkdetheme.h"

#include <QDir>
#include <QFile>
#include <QGuiApplication>
#include <QTemporaryDir>
#include <QTest>

static void writeScheme(const QString &dir, const QByteArray &window)
{
    QDir().mkpath(dir + "/kde");
    QFile f(dir + "/kde/uq.colors");
    QVERIFY(f.open(QIODevice::WriteOnly | QIODevice::Truncate));
    f.write("[Colors:Window]\nBackgroundNormal=" + window + "\nForegroundNormal=228,228,231\n"
            "[Colors:View]\nBackgroundNormal=43,43,46\nForegroundInactive=169,169,172\n"
            "[Colors:Selection]\nBackgroundNormal=138,180,248\n");
}

class TstUqKdeTheme : public QObject {
    Q_OBJECT
private slots:
    void aSchemeBecomesAPalette()
    {
        QTemporaryDir tmp;
        writeScheme(tmp.path(), "31,31,34");
        const QPalette p = uqtheme::paletteFromColors(tmp.path() + "/kde/uq.colors", QPalette());
        QCOMPARE(p.color(QPalette::Window), QColor(31, 31, 34));
        QCOMPARE(p.color(QPalette::Base), QColor(43, 43, 46));
        QCOMPARE(p.color(QPalette::Highlight), QColor(138, 180, 248));
        QCOMPARE(p.color(QPalette::PlaceholderText), QColor(169, 169, 172));
        QCOMPARE(p.color(QPalette::Disabled, QPalette::Text), QColor(169, 169, 172));
    }
    void aSwappedThemeIsFollowed()
    {
        QTemporaryDir tmp;
        qputenv("XDG_STATE_HOME", tmp.path().toUtf8());
        const QString theme = tmp.path() + "/uq/theme";
        writeScheme(theme, "1,2,3");
        uqtheme::followKdeTheme(QStringLiteral("uqkdetheme-test"));
        QCOMPARE(QGuiApplication::palette().color(QPalette::Window), QColor(1, 2, 3));
        QVERIFY(qGuiApp->property("KDE_COLOR_SCHEME_PATH").toString().contains("uqkdetheme-test-uq-"));

        // uqd's way: fill a staging folder, then rename it over theme/
        const QString staging = tmp.path() + "/uq/.theme-new";
        writeScheme(staging, "4,5,6");
        QVERIFY(QDir(theme).removeRecursively());
        QVERIFY(QDir().rename(staging, theme));
        QTRY_COMPARE_WITH_TIMEOUT(QGuiApplication::palette().color(QPalette::Window), QColor(4, 5, 6), 3000);
    }
};

QTEST_MAIN(TstUqKdeTheme)
#include "tst_uqkdetheme.moc"
