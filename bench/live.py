#!/usr/bin/env python3
"""uQ's live benchmark: the real shell and uqd, measured on a headless Hyprland.

    bench/live.py --src DIR --out results.json [--quick]

DIR is a uQ checkout whose target/release has uqctl, uqd and uqlip built. This script comes from
the checkout under test even when DIR is an older one (CI measures a pull request's base with the
same script, on the same machine). Needs Hyprland, quickshell, dbus-daemon and a DRM device (CI
gives the Arch container vkms; see bench/in-container.sh). Optional: strace (counts process
spawns). Everything runs in a private bus, runtime dir and home; nothing touches a real session.

Metrics ("lower is better", unit in the file): shell and uqd startup, idle memory, idle CPU,
idle wakeups (context switches per second, summed over threads), idle process spawns and idle
frames per minute, popup open/close latency, memory after repeated popups, and frame timing
while popups open and close (Qt's own QSG_RENDER_TIMING output). The rendering is software on
vkms, so frame times are only meaningful against another run on the same machine.
"""
import argparse, json, os, re, shutil, signal, socket, statistics, subprocess, sys, tempfile, time

CLK = os.sysconf("SC_CLK_TCK")
metrics, notes = {}, []


def put(name, value, unit, **extra):
    metrics[name] = {"value": round(value, 3), "unit": unit, **extra}
    print(f"  {name:<34} {value:>14.3f} {unit}", flush=True)


def note(msg):
    notes.append(msg)
    print("NOTE:", msg, flush=True)


# ---- process sampling (/proc) -----------------------------------------------------------------
def stat(pid):
    """utime+stime ticks of the process and of the children it has waited for."""
    with open(f"/proc/{pid}/stat") as f:
        s = f.read()
    r = s[s.rindex(")") + 2:].split()  # fields from 3 on
    return int(r[11]) + int(r[12]), int(r[13]) + int(r[14])


def ctxsw(pid):
    total = 0
    for t in os.listdir(f"/proc/{pid}/task"):
        try:
            for line in open(f"/proc/{pid}/task/{t}/status"):
                if "ctxt_switches" in line:
                    total += int(line.split()[1])
        except OSError:
            pass
    return total


def thread_ctxsw(pid):
    """{tid: (name, context switches)}; for finding out which threads wake while idle."""
    out = {}
    for t in os.listdir(f"/proc/{pid}/task"):
        try:
            n = int(open(f"/proc/{pid}/task/{t}/status").read().split("voluntary_ctxt_switches:")[1].split()[0])
            n += int(open(f"/proc/{pid}/task/{t}/status").read().split("nonvoluntary_ctxt_switches:")[1].split()[0])
            out[t] = (open(f"/proc/{pid}/task/{t}/comm").read().strip(), n)
        except (OSError, IndexError):
            pass
    return out


def status_kb(pid, key):
    for line in open(f"/proc/{pid}/status"):
        if line.startswith(key + ":"):
            return int(line.split()[1])
    return 0


def idle_sample(pid, seconds, prefix):
    quiet_start(seconds)
    s0, w0, t0, th0 = stat(pid), ctxsw(pid), time.monotonic(), thread_ctxsw(pid)
    time.sleep(seconds)
    s1, w1, t1, th1 = stat(pid), ctxsw(pid), time.monotonic(), thread_ctxsw(pid)
    dt = t1 - t0
    busy = sorted(((n - th0.get(t, (c, 0))[1]) / dt, c, t) for t, (c, n) in th1.items())[::-1]
    print(f"{prefix} wakeups per thread:", ", ".join(f"{c}[{t}]={r:.2f}/s" for r, c, t in busy if r > 0.05), flush=True)
    put(f"{prefix}_idle_cpu_pct", (s1[0] - s0[0]) / CLK / dt * 100, "pct")
    put(f"{prefix}_idle_children_cpu_pct", (s1[1] - s0[1]) / CLK / dt * 100, "pct")
    put(f"{prefix}_idle_wakeups_per_s", (w1 - w0) / dt, "per_s")
    put(f"{prefix}_idle_rss", status_kb(pid, "VmRSS") * 1024, "bytes")
    put(f"{prefix}_threads", status_kb(pid, "Threads"), "count")


def quiet_start(window):
    """Wait until a `window`-second sample fits inside one wall-clock minute, clear of both ends:
    the bar's clock repaints when the minute changes, which would put a few frames (and the wakeups
    behind them) into a window it falls in and not into one it misses."""
    while True:
        sec = time.time() % 60
        if 3 <= sec <= 57 - window:
            return
        time.sleep(0.5)


def med(xs):
    return statistics.median(xs)


# ---- environment ------------------------------------------------------------------------------
class Env:
    def __init__(self, src):
        self.src = os.path.abspath(src)
        self.tmp = tempfile.mkdtemp(prefix="uqb.")  # short: unix socket paths must stay < 108 bytes
        self.rt = self.tmp + "/rt"
        os.mkdir(self.rt, 0o700)
        self.procs = []
        self.bin = self.src + "/target/release"
        for b in ("uqctl", "uqd"):
            if not os.access(f"{self.bin}/{b}", os.X_OK):
                sys.exit(f"no {self.bin}/{b}: cargo build --release --locked -p uqctl -p uqd -p uqlip")
        for tool in ("Hyprland", "qs", "dbus-daemon"):
            if not shutil.which(tool):
                sys.exit(f"{tool} is not installed")
        cards = [d for d in os.listdir("/dev/dri") if d.startswith("card")] if os.path.isdir("/dev/dri") else []
        if not cards:
            sys.exit("no DRM device (/dev/dri/card*)")
        for d in ("state", "config/hypr/uq", "data", "home"):
            os.makedirs(f"{self.tmp}/{d}", exist_ok=True)

    def spawn(self, argv, env=None, log="proc", **kw):
        p = subprocess.Popen(argv, env=env or self.base_env(), stdout=open(f"{self.tmp}/{log}.log", "ab"),
                             stderr=subprocess.STDOUT, **kw)
        self.procs.append(p)
        return p

    def base_env(self):
        e = dict(os.environ, XDG_RUNTIME_DIR=self.rt, LC_ALL="C.UTF-8")
        for k in ("HYPRLAND_INSTANCE_SIGNATURE", "WAYLAND_DISPLAY", "DISPLAY", "UQ_DEV_SESSION"):
            e.pop(k, None)
        return e

    def start_session(self):
        # A private bus (no real settings, no real uqd) and a headless Hyprland.
        self.bus = f"unix:path={self.tmp}/bus"
        self.spawn(["dbus-daemon", "--session", f"--address={self.bus}", "--nofork", "--nopidfile"], log="dbus")
        open(f"{self.tmp}/hypr.lua", "w").write(
            'hl.monitor({ output = "", mode = "1280x720", position = "auto", scale = 1 })\n'
            "hl.config({ misc = { disable_hyprland_logo = true, disable_splash_rendering = true } })\n")
        # --gpu renders on the machine's own GPU (frame times that mean something); CI is software.
        # BENCH_HW=1 also nests Hyprland in the running session, for a desktop whose DRM seat is taken.
        env = self.base_env()
        if os.environ.get("BENCH_HW"):
            host = os.environ.get("WAYLAND_DISPLAY", "")
            host = host if host.startswith("/") else os.path.join(os.environ.get("XDG_RUNTIME_DIR", ""), host)
            if not os.path.exists(host):
                sys.exit("BENCH_HW needs a running Wayland session to nest in (WAYLAND_DISPLAY)")
            env["WAYLAND_DISPLAY"] = host
        elif not GPU:
            env["LIBGL_ALWAYS_SOFTWARE"] = "1"
        self.spawn(["Hyprland", "-c", f"{self.tmp}/hypr.lua"], env=env, log="hypr")
        sig = wl = None
        for _ in range(100):
            if os.path.isdir(f"{self.rt}/hypr"):
                for d in os.listdir(f"{self.rt}/hypr"):
                    if os.path.exists(f"{self.rt}/hypr/{d}/.socket.sock"):
                        sig = d
            wl = next((f for f in os.listdir(self.rt) if f.startswith("wayland-") and not f.endswith(".lock")), None)
            if sig and wl:
                break
            time.sleep(0.2)
        if not (sig and wl):
            print(open(f"{self.tmp}/hypr.log").read()[-3000:])
            sys.exit("headless Hyprland did not start")
        self.sig, self.wl = sig, wl
        time.sleep(0.5)

    def hypr_json(self, what):
        s = socket.socket(socket.AF_UNIX)
        s.connect(f"{self.rt}/hypr/{self.sig}/.socket.sock")
        s.sendall(f"j/{what}".encode())
        buf = b""
        while chunk := s.recv(65536):
            buf += chunk
        s.close()
        return json.loads(buf or b"null")

    def layers(self, level=None):
        """Namespaces of the layer surfaces; with level (0 background, 1 bottom, 2 top, 3 overlay),
        only those on that level."""
        names = []
        def walk(o, lvl):
            if isinstance(o, dict):
                if "namespace" in o and (level is None or lvl == str(level)):
                    names.append(o["namespace"])
                for k, v in o.items():
                    walk(v, k if k in ("0", "1", "2", "3") else lvl)
            elif isinstance(o, list):
                for v in o:
                    walk(v, lvl)
        walk(self.hypr_json("layers"), None)
        return names

    def wait_layer(self, ns, present=True, timeout=20.0, level=None):
        t0 = time.monotonic()
        while time.monotonic() - t0 < timeout:
            if (ns in self.layers(level)) == present:
                return time.monotonic() - t0
            time.sleep(0.005)
        return None

    def shell_env(self, **extra):
        e = self.base_env()
        e.update(WAYLAND_DISPLAY=self.wl, HYPRLAND_INSTANCE_SIGNATURE=self.sig, DBUS_SESSION_BUS_ADDRESS=self.bus,
                 PATH=self.bin + ":" + os.environ["PATH"], XDG_STATE_HOME=self.tmp + "/state",
                 XDG_CONFIG_HOME=self.tmp + "/config", XDG_DATA_HOME=self.tmp + "/data", HOME=self.tmp + "/home",
                 QT_QPA_PLATFORM="wayland")
        e.update(extra)
        return e

    def start_shell(self, wrap=None, **extra):
        """Start qs; returns (process, seconds until the bar's layer exists, or None)."""
        argv = (wrap or []) + ["qs", "-p", self.src + "/shell", "--no-duplicate"]
        t0 = time.monotonic()
        p = self.spawn(argv, env=self.shell_env(**extra), log="qs")
        up = None
        while time.monotonic() - t0 < 30 and p.poll() is None:
            if "uq-bar" in self.layers():
                up = time.monotonic() - t0
                break
            time.sleep(0.01)
        return p, up

    def stop(self, p):
        if p.poll() is None:
            # Under strace the process to stop is its child: strace detaches on SIGTERM and exits.
            subprocess.run(["pkill", "-TERM", "-P", str(p.pid)], capture_output=True)
            p.terminate()
            try:
                p.wait(10)
            except subprocess.TimeoutExpired:
                p.kill()
                p.wait()

    def ipc(self, *args):
        return subprocess.run(["qs", "ipc", "-p", self.src + "/shell", "call", "uq", *args], env=self.shell_env(),
                              capture_output=True, text=True, timeout=30)

    def cleanup(self):
        for p in reversed(self.procs):
            self.stop(p)
        shutil.rmtree(self.tmp, ignore_errors=True)


# Only the "frame rendered in 12ms" lines: the other renderloop lines carry "elapsed since last call:
# 443 ms", the idle gap before a frame, which an earlier version read as a 400 ms frame.
FRAME = re.compile(r"frame rendered in")
BLOCKED = re.compile(r"blockedForSync=([0-9]+) ms")
NUM_MS = re.compile(r"([0-9]+(?:\.[0-9]+)?)\s*ms")


def frames_in(path):
    """Total frame time (ms) of each frame line in Qt's QSG_RENDER_TIMING output."""
    out = []
    for line in open(path, errors="replace"):
        if FRAME.search(line):
            m = NUM_MS.search(line)
            if m:
                out.append(float(m.group(1)))
    return out


def blocked_in(path):
    """How long the GUI thread waited on the render thread (blockedForSync) for each frame it prepared;
    the first frame after a popup maps includes creating its surface and swapchain."""
    return [int(m.group(1)) for line in open(path, errors="replace") for m in [BLOCKED.search(line)] if m]


def frame_lines(path):
    """Each frame line of Qt's render timing, in full (the split into polish, sync, render, swap)."""
    return [l.strip() for l in open(path, errors="replace") if FRAME.search(l)]


def explain_layers(env, what):
    """A popup's layer never showed up on its level: print every layer Hyprland lists and the end of
    the shell's log, which says whether the shell, the compositor or the level is at fault."""
    print(f"== {what}: layers now")
    print(json.dumps(env.hypr_json("layers"), indent=1)[:3000])
    print("-- qs.log (tail)")
    print("".join(open(env.tmp + "/qs.log", errors="replace").readlines()[-40:]))


POPUPS = (("lookseedo", "uq-lookseedo"), ("appmenu", "uq-appmenu"),
          ("quicksettings", "uq-quick"), ("powermenu", "uq-power"))


def popup_frames(env, diag=False):
    """For each popup, open and close it four times and read the frames Qt logged while it opened:
    the slowest frame and the longest the GUI thread waited for the render thread. The first cycle
    loads the popup's QML, so the medians use the other three. With diag, every frame line is printed."""
    log = env.tmp + "/qs.log"
    out = {}
    for name, layer in POPUPS:
        worst, waits = [], []
        for cycle in range(4):
            n0, b0 = len(frame_lines(log)), len(blocked_in(log))
            env.ipc("open", name)
            if env.wait_layer(layer, True, 10, level=3 if name == "lookseedo" else 2) is None and cycle == 0:
                explain_layers(env, f"{name} did not reach its open level")
            time.sleep(0.6)
            frames = frame_lines(log)[n0:]
            blocked = blocked_in(log)[b0:]
            env.ipc("close", name)
            env.wait_layer(layer, False, 10, level=3 if name == "lookseedo" else 2)
            time.sleep(0.6)
            ms = [float(NUM_MS.search(l).group(1)) for l in frames if NUM_MS.search(l)]
            if cycle > 0 and ms:
                worst.append(max(ms))
                waits.append(max(blocked) if blocked else 0)
            if diag:
                print(f"== diag {name} cycle {cycle}: {len(frames)} frames, blockedForSync {blocked}")
                for l in frames:
                    print(f"   {l}")
        if worst:
            out[name] = (statistics.median(worst), statistics.median(waits))
    return out


def count_execve(path, since=None):
    n = 0
    for line in open(path, errors="replace"):
        if "execve(" in line and "ENOENT" not in line and "= -1" not in line:
            if since is not None:
                try:
                    if float(line.split()[1 if line.split()[0].isdigit() else 0]) < since:
                        continue
                except ValueError:
                    pass
            n += 1
    return n


def spawn_summary(path, since, who):
    """Which commands were started while idle, for the notes (the count alone says only that)."""
    seen = {}
    for line in open(path, errors="replace"):
        m = re.search(r"execve\(\"([^\"]+)\", \[(.*?)\]", line)
        if not m or "= -1" in line:
            continue
        try:
            if float(line.split()[1]) < since:
                continue
        except (ValueError, IndexError):
            continue
        key = m.group(2).replace('"', "")[:110]
        seen[key] = seen.get(key, 0) + 1
    for k, n in sorted(seen.items(), key=lambda kv: -kv[1])[:8]:
        note(f"{who} idle spawn x{n}: {k}")


GPU = False


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--src", default=".")
    ap.add_argument("--out", default="bench-live.json")
    ap.add_argument("--quick", action="store_true")
    ap.add_argument("--diag", action="store_true", help="print each popup's open and close frames in full")
    ap.add_argument("--gpu", action="store_true", help="render on the real GPU instead of llvmpipe")
    a = ap.parse_args()
    global GPU
    GPU = a.gpu
    settle, window, starts, cycles = (3, 6, 2, 3) if a.quick else (8, 25, 5, 10)
    env = Env(a.src)
    try:
        env.start_session()
        # uqlip, as on a real desktop: without it the shell's clipboard watcher ends and is retried
        # every few seconds, which would show up as idle spawns that a running session does not have.
        uqlip = env.spawn([f"{env.bin}/uqlip"], env=env.shell_env(), log="uqlip")
        time.sleep(1.5)
        if uqlip.poll() is not None:
            note("uqlip exited at start: " + open(env.tmp + "/uqlip.log").read()[-300:])
            uqlip = None
        print("== shell startup (bar layer visible)")
        times = []
        for _ in range(starts):
            p, up = env.start_shell()
            if up is None:
                print(open(env.tmp + "/qs.log").read()[-3000:])
                sys.exit("the shell never put its bar up")
            times.append(up * 1000)
            env.stop(p)
            env.wait_layer("uq-bar", present=False, timeout=10)
        put("shell_startup_ms", med(times), "ms", spread=round(max(times) - min(times), 1))

        print("== shell idle")
        # Render timing on: idle frames are counted, and the output is also the frame-time source.
        p, up = env.start_shell(QSG_RENDER_TIMING="1", QT_LOGGING_RULES="qt.scenegraph.time.*=true")
        time.sleep(settle)
        quiet_start(window)  # idle_sample waits for a quiet window too; start the frame count inside it
        f_before = len(frames_in(env.tmp + "/qs.log"))
        idle_sample(p.pid, window, "shell")
        f_idle = len(frames_in(env.tmp + "/qs.log")) - f_before
        put("shell_idle_frames_per_min", f_idle / window * 60, "per_min")

        print("== popups")
        opens, closes = [], []
        # Two throwaway cycles: the first open loads the popup's QML once, which is not growth.
        for _ in range(2):
            env.ipc("open", "lookseedo")
            env.wait_layer("uq-lookseedo", True, 10, level=3)
            time.sleep(0.4)
            env.ipc("close", "lookseedo")
            env.wait_layer("uq-lookseedo", False, 10, level=3)
            time.sleep(0.4)
        rss0 = status_kb(p.pid, "VmRSS") * 1024
        f_mark = len(frames_in(env.tmp + "/qs.log"))
        first_sync = []
        for _ in range(cycles):
            b_mark = len(blocked_in(env.tmp + "/qs.log"))
            t0 = time.monotonic()
            r = env.ipc("open", "lookseedo")
            if r.returncode or env.wait_layer("uq-lookseedo", True, 10, level=3) is None:
                note(f"open lookseedo failed: {r.stdout.strip()} {r.stderr.strip()[-200:]}")
                explain_layers(env, "lookseedo did not reach the overlay level")
                break
            opens.append((time.monotonic() - t0) * 1000)
            time.sleep(0.4)  # let the open animation finish before closing
            waits = blocked_in(env.tmp + "/qs.log")[b_mark:]
            if waits:
                first_sync.append(max(waits))
            t0 = time.monotonic()
            env.ipc("close", "lookseedo")
            env.wait_layer("uq-lookseedo", False, 10, level=3)
            closes.append((time.monotonic() - t0) * 1000)
            time.sleep(0.4)
        if opens:
            # Includes the `qs ipc` client's own start, a constant that cancels in a comparison.
            put("popup_open_lookseedo_ms", med(opens), "ms")
            put("popup_close_lookseedo_ms", med(closes), "ms")
            if first_sync:
                put("popup_first_sync_ms", med(first_sync), "ms")
            put("shell_rss_after_popups", status_kb(p.pid, "VmRSS") * 1024, "bytes")
            put("shell_rss_growth_popups", status_kb(p.pid, "VmRSS") * 1024 - rss0, "bytes")
        fr = frames_in(env.tmp + "/qs.log")[f_mark:]
        if len(fr) >= 5:
            fr.sort()
            put("frame_p50_ms", fr[len(fr) // 2], "ms")
            put("frame_p95_ms", fr[int(len(fr) * 0.95)], "ms")
            put("frames_per_popup_cycle", len(fr) / max(1, len(opens)), "count")
        else:
            note(f"only {len(fr)} frame-timing lines while popups ran; sample of the log:\n"
                 + "".join(open(env.tmp + "/qs.log", errors="replace").readlines()[-8:]))
        for name, (frame_ms, sync_ms) in popup_frames(env, a.diag).items():
            put(f"popup_{name}_open_frame_ms", frame_ms, "ms")
            put(f"popup_{name}_first_sync_ms", sync_ms, "ms")
        env.stop(p)

        if shutil.which("strace"):
            print("== shell process spawns")
            slog = env.tmp + "/strace-qs.log"
            wrap = ["strace", "-f", "--seccomp-bpf", "-ttt", "-e", "trace=execve", "-o", slog]
            p, up = env.start_shell(wrap=wrap)
            if up is None:
                note("the shell did not start under strace; spawns not counted")
            else:
                t_up = time.time()
                put("shell_startup_spawns", count_execve(slog), "count")
                time.sleep(settle)
                quiet_start(window)
                t_idle = time.time()
                time.sleep(window)
                put("shell_idle_spawns_per_min", count_execve(slog, since=t_idle) / window * 60, "per_min")
                spawn_summary(slog, t_idle, "shell")
            env.stop(p)
        else:
            note("strace is not installed; process spawns not counted")

        if os.environ.get("UQ_BENCH_TRACE") and shutil.which("strace"):
            print("== shell idle syscall trace (diagnostic)")
            tlog = env.tmp + "/strace-wake.log"
            wrap = ["strace", "-f", "--seccomp-bpf", "-y", "-ttt", "-e", "trace=read,write,recvmsg,sendmsg,timerfd_settime,inotify_add_watch", "-o", tlog]
            p, up = env.start_shell(wrap=wrap)
            if up is None:
                note("diagnostic trace: shell did not start")
            else:
                time.sleep(settle)
                quiet_start(window)
                t_idle = time.time()
                time.sleep(window)
                counts = {}
                for line in open(tlog, errors="replace"):
                    m = re.match(r"(?:\[pid\s+)?(\d+)\]?\s+(\d+\.\d+) (\w+)\((.*)", line)
                    if not m or float(m.group(2)) < t_idle:
                        continue
                    k = (m.group(1) + " " + m.group(3), re.sub(r"\d{3,}", "N", m.group(4)[:80]))
                    counts[k] = counts.get(k, 0) + 1
                for (sc, arg), n in sorted(counts.items(), key=lambda kv: -kv[1])[:40]:
                    print(f"TRACE {n:5d} {sc} {arg}", flush=True)
                for ln in open(env.tmp + "/qs.log", errors="replace").readlines()[-40:]:
                    print("QSLOG", ln.rstrip()[:200], flush=True)
            env.stop(p)

        if uqlip is not None:
            print("== uqlip idle")
            idle_sample(uqlip.pid, window, "uqlip")

        print("== uqd")
        share = env.tmp + "/share"
        os.makedirs(share)
        for d in ("config/hypr", "themes", "templates"):
            if os.path.exists(f"{env.src}/{d}"):
                os.symlink(f"{env.src}/{d}", share + "/" + os.path.basename(d))
        for f in ("generated.lua", "user.lua"):
            open(f"{env.tmp}/config/hypr/uq/{f}", "a").close()
        uenv = env.shell_env(UQ_DATA_DIR=share)
        busctl = shutil.which("busctl")
        def up():
            return subprocess.run([busctl, "--user", "status", "org.uq.Desktop1"], env=uenv,
                                  capture_output=True).returncode == 0
        starts_d = []
        d = None
        for i in range(starts):
            t0 = time.monotonic()
            d = env.spawn([f"{env.bin}/uqd"], env=uenv, log="uqd")
            while time.monotonic() - t0 < 30 and d.poll() is None and not up():
                time.sleep(0.01)
            if d.poll() is not None:
                print(open(env.tmp + "/uqd.log").read()[-2000:])
                note("uqd exited at start; uqd metrics skipped")
                d = None
                break
            starts_d.append((time.monotonic() - t0) * 1000)
            if i < starts - 1:
                env.stop(d)
        if d is not None:
            put("uqd_startup_ms", med(starts_d), "ms", spread=round(max(starts_d) - min(starts_d), 1))
            time.sleep(settle)
            idle_sample(d.pid, window, "uqd")
            env.stop(d)
            if shutil.which("strace"):
                slog = env.tmp + "/strace-uqd.log"
                d = env.spawn(["strace", "-f", "--seccomp-bpf", "-ttt", "-e", "trace=execve", "-o", slog,
                               f"{env.bin}/uqd"], env=uenv, log="uqd")
                time.sleep(settle)
                quiet_start(window)
                t_idle = time.time()
                time.sleep(window)
                put("uqd_idle_spawns_per_min", count_execve(slog, since=t_idle) / window * 60, "per_min")
                spawn_summary(slog, t_idle, "uqd")
                env.stop(d)
    finally:
        env.cleanup()
    with open(a.out, "w") as f:
        json.dump({"tool": "uq", "metrics": metrics, "notes": notes}, f, indent=2)
        f.write("\n")
    print(f"wrote {a.out}: {len(metrics)} metrics")


if __name__ == "__main__":
    main()
