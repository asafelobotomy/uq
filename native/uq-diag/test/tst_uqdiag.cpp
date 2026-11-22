// The C++ uq-diag: the line format, codes, the crash folder and a real crash's record.
#include "../uqdiag.h"

#include <QDir>
#include <QFile>
#include <QJsonDocument>
#include <QJsonObject>
#include <QTemporaryDir>
#include <QTest>

#include <csignal>
#include <sys/wait.h>
#include <unistd.h>

class TestUqDiag : public QObject
{
    Q_OBJECT

private slots:
    void lines()
    {
        QCOMPARE(uqdiag::formatLine(QtWarningMsg, "reelo", "[REELO-003] no file", true),
                 QStringLiteral("<4>reelo: [REELO-003] no file"));
        QCOMPARE(uqdiag::formatLine(QtCriticalMsg, "reelo", "x", false), QStringLiteral("ERROR reelo: x"));
        QCOMPARE(uqdiag::formatLine(QtInfoMsg, "reelo", "a\nb", true), QStringLiteral("<6>reelo: a\n  b"));
    }

    void codes()
    {
        QCOMPARE(uqdiag::codeIn("qml: reelo[REELO-010] gone"), QStringLiteral("REELO-010"));
        QCOMPARE(uqdiag::codeIn("arr[3] [ab-123]"), QString());
    }

    void crashDir()
    {
        qputenv("XDG_STATE_HOME", "relative");
        qputenv("HOME", "/home/a");
        QCOMPARE(uqdiag::crashDir(), QStringLiteral("/home/a/.local/state/uq/crashes"));
        qputenv("XDG_STATE_HOME", "/s");
        QCOMPARE(uqdiag::crashDir(), QStringLiteral("/s/uq/crashes"));
    }

    // A child that crashes leaves crash.json and backtrace.txt, and still dies of its signal.
    void crashRecord()
    {
        QTemporaryDir tmp;
        qputenv("XDG_STATE_HOME", tmp.path().toUtf8());
        const pid_t pid = fork();
        if (pid == 0) {
            uqdiag::init("demo \"app\"", "1.2.3");
            ::raise(SIGSEGV);
            _exit(0);
        }
        int status = 0;
        QCOMPARE(waitpid(pid, &status, 0), pid);
        QVERIFY(WIFSIGNALED(status));
        QCOMPARE(WTERMSIG(status), SIGSEGV);
        const QDir crashes(tmp.path() + "/uq/crashes");
        const QStringList dirs = crashes.entryList(QDir::Dirs | QDir::NoDotAndDotDot);
        QCOMPARE(dirs.size(), 1);
        QVERIFY(dirs[0].endsWith("-" + QString::number(pid)));
        QFile f(crashes.filePath(dirs[0] + "/crash.json"));
        QVERIFY(f.open(QIODevice::ReadOnly));
        const QJsonObject o = QJsonDocument::fromJson(f.readAll()).object();
        QCOMPARE(o["program"].toString(), QStringLiteral("demo \"app\""));
        QCOMPARE(o["version"].toString(), QStringLiteral("1.2.3"));
        QCOMPARE(o["pid"].toInt(), int(pid));
        QCOMPARE(o["message"].toString(), QStringLiteral("Segmentation fault (11)"));
        QVERIFY(o["time"].toInteger() > 1700000000);
        QVERIFY(QFile::exists(crashes.filePath(dirs[0] + "/backtrace.txt")));
    }
};

QTEST_GUILESS_MAIN(TestUqDiag)
#include "tst_uqdiag.moc"
