#!/usr/bin/env python3
"""Minimal QMP client for the uQ VM harness (python3 stdlib only).

    qmp.py SOCKET screendump FILE.png [HEAD]
    qmp.py SOCKET key COMBO...         e.g. key meta_l-l   key ret   key meta_l
    qmp.py SOCKET type TEXT            types TEXT as key presses (US layout)
    qmp.py SOCKET click X Y [BUTTON]   absolute pointer (virtio-tablet), X/Y in guest pixels
    qmp.py SOCKET move X Y
    qmp.py SOCKET scroll X Y N [up|down]   N wheel steps at X,Y (default down)
    qmp.py SOCKET status               prints the run state (running, suspended, ...)
    qmp.py SOCKET wakeup               system_wakeup (resume from S3)
    qmp.py SOCKET quit
    qmp.py SOCKET raw JSON             any command, e.g. '{"execute":"query-status"}'

Every input event goes through QEMU's emulated devices, so the guest sees real keyboard and
pointer input (libinput, idle timers, the lock screen) exactly as from hardware.
"""
import json
import os
import socket
import sys
import time

# Screen size the absolute pointer maps onto (virtio-gpu's default mode); override with
# UQVM_SCREEN=WxH when the guest runs another mode.
SCREEN = tuple(int(v) for v in os.environ.get("UQVM_SCREEN", "1280x800").split("x"))

SHIFTED = {
    "!": "1", "@": "2", "#": "3", "$": "4", "%": "5", "^": "6", "&": "7", "*": "8",
    "(": "9", ")": "0", "_": "minus", "+": "equal", "{": "bracket_left", "}": "bracket_right",
    "|": "backslash", ":": "semicolon", '"': "apostrophe", "~": "grave_accent", "<": "comma",
    ">": "dot", "?": "slash",
}
PLAIN = {
    " ": "spc", "-": "minus", "=": "equal", "[": "bracket_left", "]": "bracket_right",
    "\\": "backslash", ";": "semicolon", "'": "apostrophe", "`": "grave_accent", ",": "comma",
    ".": "dot", "/": "slash", "\n": "ret", "\t": "tab",
}


class Qmp:
    def __init__(self, path, timeout=30):
        self.s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.s.settimeout(timeout)
        self.s.connect(path)
        self.f = self.s.makefile("rwb", buffering=0)
        self._read()  # greeting
        self.cmd("qmp_capabilities")

    def _read(self):
        while True:
            line = self.f.readline()
            if not line:
                raise ConnectionError("QMP socket closed")
            msg = json.loads(line)
            if "event" in msg:
                continue
            return msg

    def cmd(self, name, **args):
        req = {"execute": name}
        if args:
            req["arguments"] = args
        self.f.write((json.dumps(req) + "\n").encode())
        r = self._read()
        if "error" in r:
            raise RuntimeError(f"{name}: {r['error'].get('desc', r['error'])}")
        return r.get("return")


def char_keys(c):
    if c.isalpha() and c.isascii():
        return ["shift", c.lower()] if c.isupper() else [c]
    if c.isdigit():
        return [c]
    if c in PLAIN:
        return [PLAIN[c]]
    if c in SHIFTED:
        return ["shift", SHIFTED[c]]
    raise ValueError(f"cannot type {c!r}")


def abs_axis(v, size):
    return int(int(v) * 32767 / max(size - 1, 1))


def main(argv):
    if len(argv) < 3:
        print(__doc__, file=sys.stderr)
        return 2
    q = Qmp(argv[1])
    op, rest = argv[2], argv[3:]
    if op == "screendump":
        args = {"filename": os.path.abspath(rest[0]), "format": "png"}
        if len(rest) > 1:
            args["head"] = int(rest[1])
        q.cmd("screendump", **args)
    elif op == "key":
        for combo in rest:
            q.cmd("send-key", keys=[{"type": "qcode", "data": k} for k in combo.split("-")],
                  **{"hold-time": 60})
            time.sleep(0.15)
    elif op == "type":
        for c in " ".join(rest):
            q.cmd("send-key", keys=[{"type": "qcode", "data": k} for k in char_keys(c)],
                  **{"hold-time": 50})
            # Slower than a person types: faster input loses characters under llvmpipe load.
            time.sleep(0.15)
    elif op in ("click", "move"):
        x, y = abs_axis(rest[0], SCREEN[0]), abs_axis(rest[1], SCREEN[1])
        ev = [{"type": "abs", "data": {"axis": "x", "value": x}},
              {"type": "abs", "data": {"axis": "y", "value": y}}]
        if op == "click":
            # Come from a pixel away: a pointer already at X,Y gets no motion event, and a surface
            # mapped under a still pointer never sees it enter.
            near = abs_axis(max(int(rest[0]) - 2, 0), SCREEN[0])
            q.cmd("input-send-event", events=[{"type": "abs", "data": {"axis": "x", "value": near}},
                                              {"type": "abs", "data": {"axis": "y", "value": y}}])
            time.sleep(0.15)
        q.cmd("input-send-event", events=ev)
        if op == "click":
            btn = rest[2] if len(rest) > 2 else "left"
            # Let the surface under the pointer see the motion (hover) before the press; a press
            # in the same instant as the move can land on the surface the pointer just left.
            time.sleep(0.3)
            q.cmd("input-send-event", events=ev)
            time.sleep(0.1)
            q.cmd("input-send-event", events=[{"type": "btn", "data": {"down": True, "button": btn}}])
            time.sleep(0.08)
            q.cmd("input-send-event", events=[{"type": "btn", "data": {"down": False, "button": btn}}])
    elif op == "scroll":
        x, y = abs_axis(rest[0], SCREEN[0]), abs_axis(rest[1], SCREEN[1])
        q.cmd("input-send-event", events=[{"type": "abs", "data": {"axis": "x", "value": x}},
                                          {"type": "abs", "data": {"axis": "y", "value": y}}])
        time.sleep(0.2)
        btn = "wheel-up" if len(rest) > 3 and rest[3] == "up" else "wheel-down"
        for _ in range(int(rest[2])):
            q.cmd("input-send-event", events=[{"type": "btn", "data": {"down": True, "button": btn}}])
            q.cmd("input-send-event", events=[{"type": "btn", "data": {"down": False, "button": btn}}])
            time.sleep(0.08)
    elif op == "status":
        print(q.cmd("query-status")["status"])
    elif op == "wakeup":
        q.cmd("system_wakeup")
    elif op == "quit":
        try:
            q.cmd("quit")
        except ConnectionError:
            pass
    elif op == "raw":
        req = json.loads(rest[0])
        print(json.dumps(q.cmd(req["execute"], **req.get("arguments", {}))))
    else:
        print(f"unknown op {op}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
