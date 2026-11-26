"""Qt's frame timing in the shell's log, the popups' open and close frames, and strace's
process spawns, for bench/live.py."""
import json, re, statistics, time


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
            warm = name != "lookseedo"  # kept mapped: the layer never appears or goes, so just wait
            env.ipc("open", name)
            if warm:
                time.sleep(0.3)
            elif env.wait_layer(layer, True, 10, level=3) is None and cycle == 0:
                explain_layers(env, f"{name} did not reach its open level")
            time.sleep(0.6)
            frames = frame_lines(log)[n0:]
            blocked = blocked_in(log)[b0:]
            env.ipc("close", name)
            if not warm:
                env.wait_layer(layer, False, 10, level=3)
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
