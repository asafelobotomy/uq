// Tests for the vendored C++ uq-theme: parsing, and a live follow of a replaced file and of a
// swapped theme/ folder (as uqd does it), with no polling.
#include "../uqtheme.h"

#include <QDir>
#include <QFile>
#include <QJsonDocument>
#include <QJsonObject>
#include <QSignalSpy>
#include <QTemporaryDir>
#include <QTest>

static QByteArray tokens(const QString &accent, int radius = 10)
{
    QJsonObject colors;
    for (const char *k : {"bg", "surface", "surface_alt", "overlay", "text", "text_muted", "accent",
                          "accent_text", "border", "success", "warning", "error"})
        colors.insert(k, QString::fromLatin1(k) == "accent" ? accent : QStringLiteral("#202020"));
    QJsonObject t{{"name", "Test"},
                  {"variant", "light"},
                  {"colors", colors},
                  {"ui", QJsonObject{{"radius", radius}, {"gap_in", 4}}},
                  {"fonts", QJsonObject{{"ui", "Inter"}, {"size", 11}}}};
    return QJsonDocument(t).toJson();
}

static void write(const QString &path, const QByteArray &body)
{
    QFile f(path);
    QVERIFY(f.open(QIODevice::WriteOnly | QIODevice::Truncate));
    f.write(body);
}

class TstUqTheme : public QObject {
    Q_OBJECT
private slots:
    void aFullThemeIsRead()
    {
        const QVariantMap t = uqtheme::parse(tokens("#d4737f"));
        QCOMPARE(t["present"].toBool(), true);
        QCOMPARE(t["variant"].toString(), QStringLiteral("light"));
        QCOMPARE(t["colors"].toMap()["accent"].toString(), QStringLiteral("#d4737f"));
        QCOMPARE(t["radius"].toInt(), 10);
        QCOMPARE(t["gapIn"].toInt(), 4);
        QCOMPARE(t["gapOut"].toInt(), 12);
        QCOMPARE(t["fontUi"].toString(), QStringLiteral("Inter"));
        QCOMPARE(uqtheme::parse(tokens("#ffffff", 500))["radius"].toInt(), 32);
    }
    void aPartialOrBrokenFileIsNoTheme()
    {
        QCOMPARE(uqtheme::parse("not json")["present"].toBool(), false);
        QCOMPARE(uqtheme::parse(R"({"colors":{"bg":"#000000"}})")["present"].toBool(), false);
        QCOMPARE(uqtheme::parse(tokens("red"))["present"].toBool(), false);
        QCOMPARE(uqtheme::load("/nonexistent/tokens.json")["present"].toBool(), false);
    }
    void thePathFollowsXdg()
    {
        qputenv("XDG_STATE_HOME", "/x/state");
        QCOMPARE(uqtheme::tokensPath(), QStringLiteral("/x/state/uq/theme/tokens.json"));
        qputenv("UQTHEME_TEST_TOKENS", "/fixture.json");
        QCOMPARE(uqtheme::tokensPath("UQTHEME_TEST_TOKENS"), QStringLiteral("/fixture.json"));
        qputenv("XDG_STATE_HOME", "relative");
        QVERIFY(uqtheme::tokensPath().endsWith("/.local/state/uq/theme/tokens.json"));
    }
    void changesAreFollowedLive()
    {
        QTemporaryDir tmp;
        const QString theme = tmp.path() + "/uq/theme";
        const QString file = theme + "/tokens.json";
        uqtheme::Watcher w(file);
        QCOMPARE(w.tokens()["present"].toBool(), false);
        QSignalSpy spy(&w, &uqtheme::Watcher::tokensChanged);

        // the first theme ever: no folders yet
        QVERIFY(QDir().mkpath(theme));
        QTest::qWait(300);
        write(file + ".tmp", tokens("#111111"));
        QVERIFY(QFile::rename(file + ".tmp", file));
        QTRY_COMPARE_WITH_TIMEOUT(w.tokens()["colors"].toMap()["accent"].toString(),
                                  QStringLiteral("#111111"), 5000);

        // the whole folder swapped, as uqd's atomic_replace_dir does it
        const QString staging = tmp.path() + "/uq/theme.staging";
        QVERIFY(QDir().mkpath(staging));
        write(staging + "/tokens.json", tokens("#222222"));
        QVERIFY(QDir().rename(theme, tmp.path() + "/uq/theme.old"));
        QVERIFY(QDir().rename(staging, theme));
        QVERIFY(QDir(tmp.path() + "/uq/theme.old").removeRecursively());
        QTRY_COMPARE_WITH_TIMEOUT(w.tokens()["colors"].toMap()["accent"].toString(),
                                  QStringLiteral("#222222"), 5000);

        // rewritten in place (an editor)
        write(file, tokens("#333333"));
        QTRY_COMPARE_WITH_TIMEOUT(w.tokens()["colors"].toMap()["accent"].toString(),
                                  QStringLiteral("#333333"), 5000);

        // a write with the same tokens is not a change
        const int before = spy.count();
        write(file, tokens("#333333"));
        QTest::qWait(500);
        QCOMPARE(spy.count(), before);
    }
};

QTEST_GUILESS_MAIN(TstUqTheme)
#include "tst_uqtheme.moc"
