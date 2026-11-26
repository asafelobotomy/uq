#!/usr/bin/env python3
"""Num Lock across keyboards on a real Hyprland (run by scripts/numlock-live.sh, as root).

Creates real kernel keyboards with uinput before Hyprland starts: a typing keyboard, a
"Power Button" (a button-only keyboard, as every PC has) and "emobie-inject" (emobie's typing
device). Then it checks what an app sees when the keypad 1 is pressed on the typing keyboard:
a digit means the app has Num Lock on.

  driver.py <stock|uq> <hypr.lua> <client.qml> <workdir>

stock: plain numlock_by_default = true. Expected to show the bug (focus brings Num Lock back).
uq:    the same plus base.lua's numlock-devices block, and emobie's lock sync before it types.
"""
import json, os, subprocess, sys, time
from evdev import UInput, ecodes as e

mode, cfg, client, work = sys.argv[1:5]
TESTER = "tester"

def mk(name, keys, vendor=0x1234, product=0x5678):
    return UInput({e.EV_KEY: keys, e.EV_LED: [e.LED_NUML, e.LED_CAPSL]}, name=name,
                  vendor=vendor, product=product, bustype=e.BUS_USB)

typing = mk("Test Keyboard", list(range(1, 128)))
power = UInput({e.EV_KEY: [e.KEY_POWER]}, name="Power Button", bustype=e.BUS_HOST)
inject = mk("emobie-inject", list(range(1, 249)), 0x2E6F, 0x696E)
time.sleep(1.5)  # udev (on the host) tags them before Hyprland enumerates

rt = os.path.join(work, "rt")
os.makedirs(rt, mode=0o700, exist_ok=True)
subprocess.run(["chown", "-R", TESTER, work], check=True)
env = f"XDG_RUNTIME_DIR={rt} LIBSEAT_BACKEND=seatd LIBGL_ALWAYS_SOFTWARE=1"
hypr = subprocess.Popen(["su", TESTER, "-c", f"env {env} Hyprland -c {cfg} >{work}/hypr.log 2>&1"])

def sig():
    d = os.path.join(rt, "hypr")
    for _ in range(100):
        if os.path.isdir(d):
            for s in os.listdir(d):
                if os.path.exists(os.path.join(d, s, ".socket.sock")):
                    return s
        time.sleep(0.2)
    sys.exit("FAIL: Hyprland never opened its socket\n" + open(f"{work}/hypr.log").read()[-3000:])

SIG = sig()

def hc(*args):
    out = subprocess.run(["su", TESTER, "-c", "env XDG_RUNTIME_DIR=%s HYPRLAND_INSTANCE_SIGNATURE=%s hyprctl %s"
                          % (rt, SIG, " ".join("'%s'" % a for a in args))], capture_output=True, text=True)
    return out.stdout

def keyboards():
    try:
        return json.loads(hc("-j", "devices"))["keyboards"]
    except Exception:
        return []

for _ in range(50):
    names = [k["name"] for k in keyboards()]
    if {"test-keyboard", "power-button", "emobie-inject"} <= set(names):
        break
    time.sleep(0.2)
else:
    sys.exit("FAIL: Hyprland did not pick up the uinput keyboards: %s" % names)

wl = next(f for f in os.listdir(rt) if f.startswith("wayland-") and not f.endswith(".lock"))
subprocess.Popen(["su", TESTER, "-c", f"env XDG_RUNTIME_DIR={rt} WAYLAND_DISPLAY={wl} QT_QPA_PLATFORM=wayland "
                  f"qs -p {client} >{work}/client.log 2>&1"])
for _ in range(50):
    if '"numB"' in hc("-j", "clients") and '"numA"' in hc("-j", "clients"):
        break
    time.sleep(0.2)
else:
    sys.exit("FAIL: client windows never appeared")
time.sleep(1)

def tap(dev, key):
    dev.write(e.EV_KEY, key, 1); dev.syn(); time.sleep(0.03)
    dev.write(e.EV_KEY, key, 0); dev.syn(); time.sleep(0.15)

def focus(title):
    hc("dispatch", 'hl.dsp.focus({ window = "title:%s" })' % title)
    time.sleep(0.4)

def text(title):
    last = ""
    for line in open(f"{work}/client.log", errors="replace"):
        i = line.find(title + ":")
        if i >= 0:
            last = line[i + len(title) + 1:].rstrip("\n")
    return last

def app_numlock():
    """Press keypad 1 in window A; True when the app typed a digit (it sees Num Lock on)."""
    before = text("numA").count("1")
    tap(typing, e.KEY_KP1)
    time.sleep(0.3)
    return text("numA").count("1") > before

def locks():
    return {k["name"]: k["numLock"] for k in keyboards()}

def emobie_sync():
    """emobie's lock_sync: match emobie-inject to the main keyboard before typing."""
    kbs = keyboards()
    ours = next(k for k in kbs if k["name"] == "emobie-inject")
    if ours["main"]:
        return
    truth = next((k for k in kbs if k["main"]), None)
    if truth and truth["numLock"] != ours["numLock"]:
        tap(inject, e.KEY_NUMLOCK)

def emobie_paste():
    if mode == "uq":
        emobie_sync()
    inject.write(e.EV_KEY, e.KEY_LEFTCTRL, 1); inject.syn(); time.sleep(0.05)
    inject.write(e.EV_KEY, e.KEY_LEFTCTRL, 0); inject.syn(); time.sleep(0.3)

results = []
def check(name, got, want):
    ok = got == want
    results.append(ok)
    print(("PASS" if ok else "FAIL") + f": {name} (app Num Lock {got}, want {want}; keyboards {locks()})")

focus("numA")
check("starts with Num Lock on", app_numlock(), True)
tap(typing, e.KEY_NUMLOCK)
check("Num Lock off on the keyboard", app_numlock(), False)
focus("numB"); focus("numA")
check("still off after focus moves", app_numlock(), False)
tap(typing, e.KEY_NUMLOCK)
check("Num Lock back on", app_numlock(), True)
emobie_paste()
check("still on after an emobie paste", app_numlock(), True)
tap(typing, e.KEY_NUMLOCK)
check("off again with one press", app_numlock(), False)
emobie_paste()
check("still off after an emobie paste", app_numlock(), False)
focus("numB"); focus("numA")
check("still off after paste and focus move", app_numlock(), False)

hypr.terminate()
print(f"{mode}: {sum(results)}/{len(results)} passed")
sys.exit(0 if all(results) else 1)
