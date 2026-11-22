// uq-diag 1.0.0
//
// See uqdiag.h. The crash handler only uses async-signal-safe calls (open, write, mkdir,
// rename, time, getpid) on buffers prepared by init(); backtrace() is called once in init() so
// its library is loaded before a crash needs it.
#include "uqdiag.h"

#include <QByteArray>
#include <QDir>
#include <QRegularExpression>
#include <QtGlobal>

#include <cerrno>
#include <csignal>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <ctime>
#include <execinfo.h>
#include <fcntl.h>
#include <sys/stat.h>
#include <unistd.h>

namespace uqdiag {
namespace {

QByteArray g_program;
bool g_journal = false;

// Prepared for the crash handler: the crash folder (made by init) and JSON-escaped strings.
char g_dir[1024];
char g_programJson[256];
char g_versionJson[128];

const int kSignals[] = {SIGSEGV, SIGABRT, SIGBUS, SIGFPE, SIGILL};

QByteArray jsonEscape(const QByteArray &in)
{
    QByteArray out;
    for (char c : in) {
        if (c == '"' || c == '\\') {
            out += '\\';
            out += c;
        } else if (static_cast<unsigned char>(c) >= 0x20) {
            out += c;
        }
    }
    return out;
}

// Async-signal-safe helpers.
size_t slen(const char *s)
{
    size_t n = 0;
    while (s[n])
        ++n;
    return n;
}
void put(int fd, const char *s)
{
    size_t n = slen(s);
    while (n > 0) {
        ssize_t w = ::write(fd, s, n);
        if (w <= 0)
            return;
        s += w;
        n -= static_cast<size_t>(w);
    }
}
// Decimal text of `v` into `buf` (at least 24 bytes); returns buf.
char *num(long long v, char *buf)
{
    char tmp[24];
    int i = 0;
    bool neg = v < 0;
    unsigned long long u = neg ? static_cast<unsigned long long>(-v) : static_cast<unsigned long long>(v);
    do {
        tmp[i++] = static_cast<char>('0' + u % 10);
        u /= 10;
    } while (u && i < 22);
    int j = 0;
    if (neg)
        buf[j++] = '-';
    while (i)
        buf[j++] = tmp[--i];
    buf[j] = 0;
    return buf;
}
void cat(char *dst, size_t cap, const char *src)
{
    size_t n = slen(dst);
    while (*src && n + 1 < cap)
        dst[n++] = *src++;
    dst[n] = 0;
}

const char *signalName(int sig)
{
    switch (sig) {
    case SIGSEGV: return "Segmentation fault (11)";
    case SIGABRT: return "Aborted (6)";
    case SIGBUS: return "Bus error (7)";
    case SIGFPE: return "Floating point exception (8)";
    case SIGILL: return "Illegal instruction (4)";
    }
    return "Signal";
}

void onCrash(int sig)
{
    char t[24], p[24];
    num(static_cast<long long>(::time(nullptr)), t);
    num(static_cast<long long>(::getpid()), p);
    char folder[1200] = {0};
    cat(folder, sizeof folder, g_dir);
    cat(folder, sizeof folder, "/");
    cat(folder, sizeof folder, g_program.constData());
    cat(folder, sizeof folder, "-");
    cat(folder, sizeof folder, t);
    cat(folder, sizeof folder, "-");
    cat(folder, sizeof folder, p);
    ::mkdir(folder, 0700);

    char path[1300] = {0};
    cat(path, sizeof path, folder);
    cat(path, sizeof path, "/backtrace.txt");
    int fd = ::open(path, O_WRONLY | O_CREAT | O_TRUNC | O_CLOEXEC, 0600);
    if (fd >= 0) {
        void *frames[64];
        int n = ::backtrace(frames, 64);
        ::backtrace_symbols_fd(frames, n, fd);
        ::close(fd);
    }

    char tmp[1300] = {0}, final_[1300] = {0};
    cat(tmp, sizeof tmp, folder);
    cat(tmp, sizeof tmp, "/.crash.json.tmp");
    cat(final_, sizeof final_, folder);
    cat(final_, sizeof final_, "/crash.json");
    fd = ::open(tmp, O_WRONLY | O_CREAT | O_TRUNC | O_CLOEXEC, 0600);
    if (fd >= 0) {
        put(fd, "{\n  \"program\": \"");
        put(fd, g_programJson);
        put(fd, "\",\n  \"version\": \"");
        put(fd, g_versionJson);
        put(fd, "\",\n  \"time\": ");
        put(fd, t);
        put(fd, ",\n  \"pid\": ");
        put(fd, p);
        put(fd, ",\n  \"thread\": \"\",\n  \"message\": \"");
        put(fd, signalName(sig));
        put(fd, "\",\n  \"location\": \"\",\n  \"backtrace\": \"(in backtrace.txt)\"\n}\n");
        ::fsync(fd);
        ::close(fd);
        ::rename(tmp, final_);
    }
    // Die as the program would have, so systemd and coredumps see the real signal.
    ::signal(sig, SIG_DFL);
    ::raise(sig);
}

void onMessage(QtMsgType type, const QMessageLogContext &, const QString &msg)
{
    const QByteArray line =
        formatLine(type, QString::fromUtf8(g_program), msg, g_journal).toUtf8() + '\n';
    ::fwrite(line.constData(), 1, static_cast<size_t>(line.size()), stderr);
    ::fflush(stderr);
    if (type == QtFatalMsg)
        ::abort();
}

} // namespace

QString formatLine(QtMsgType type, const QString &program, const QString &message, bool journal)
{
    int priority = 6;
    const char *word = "INFO";
    switch (type) {
    case QtDebugMsg: priority = 7; word = "DEBUG"; break;
    case QtInfoMsg: priority = 6; word = "INFO"; break;
    case QtWarningMsg: priority = 4; word = "WARN"; break;
    case QtCriticalMsg: priority = 3; word = "ERROR"; break;
    case QtFatalMsg: priority = 2; word = "FATAL"; break;
    }
    QString head = journal ? QStringLiteral("<%1>").arg(priority)
                           : QStringLiteral("%1 ").arg(QLatin1String(word), 5);
    // One journal entry per message: later lines are indented, not new entries' heads.
    QString body = message;
    body.replace(QLatin1Char('\n'), QLatin1String("\n  "));
    return head + program + QLatin1String(": ") + body;
}

QString codeIn(const QString &message)
{
    static const QRegularExpression re(QStringLiteral("\\[([A-Z][A-Z0-9]{0,11}-[0-9]{3})\\]"));
    const auto m = re.match(message);
    return m.hasMatch() ? m.captured(1) : QString();
}

QString crashDir()
{
    QString state = qEnvironmentVariable("XDG_STATE_HOME");
    if (state.isEmpty() || !QDir::isAbsolutePath(state))
        state = qEnvironmentVariable("HOME", QStringLiteral("/tmp")) + QStringLiteral("/.local/state");
    return state + QStringLiteral("/uq/crashes");
}

void init(const char *program, const char *version)
{
    g_program = QByteArray(program);
    g_journal = qEnvironmentVariableIsSet("JOURNAL_STREAM") && !::isatty(STDERR_FILENO);
    qInstallMessageHandler(onMessage);

    const QByteArray dir = crashDir().toUtf8();
    QDir().mkpath(QString::fromUtf8(dir));
    std::snprintf(g_dir, sizeof g_dir, "%s", dir.constData());
    std::snprintf(g_programJson, sizeof g_programJson, "%s", jsonEscape(g_program).constData());
    std::snprintf(g_versionJson, sizeof g_versionJson, "%s", jsonEscape(QByteArray(version)).constData());

    void *warm[1];
    ::backtrace(warm, 1);
    struct sigaction sa;
    std::memset(&sa, 0, sizeof sa);
    sa.sa_handler = onCrash;
    sa.sa_flags = SA_RESETHAND;
    sigemptyset(&sa.sa_mask);
    for (int s : kSignals)
        ::sigaction(s, &sa, nullptr);
}

} // namespace uqdiag
