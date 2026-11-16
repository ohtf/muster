# SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
# SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception
#
# Desktop app checks on a real X server (run inside the container by
# tests/x11/run.sh): the button's size and default corner, a moved button is
# saved and restored per monitor layout, a saved position off every monitor
# falls back to the corner, and the keyboard shortcut raises an alarm only
# when held (with key auto-repeat on), muting the sound turns the status
# amber, and an xrdp remote session is reported.

import json
import os
import subprocess
import sys
import time

from Xlib import X, XK, display
from Xlib.ext import xtest

D = "/tmp/d"
SOCK = "/tmp/d.sock"
env = dict(os.environ, DISPLAY=":99", PANIC_BUTTON_DATA_DIR=D,
           PANIC_BUTTON_CONTROL_SOCKET=SOCK, PANIC_BUTTON_LOG="info,panic_button_desktop=debug")
env.pop("WAYLAND_DISPLAY", None)
failures = []

def check(name, ok, detail=""):
    print(("ok   " if ok else "FAIL ") + name + (f": {detail}" if detail else ""), flush=True)
    if not ok:
        failures.append(name)

def find(d, title):
    def walk(w):
        try:
            if w.get_wm_name() == title:
                return w
            for c in w.query_tree().children:
                r = walk(c)
                if r:
                    return r
        except Exception:
            return None
    return walk(d.screen().root)

def pos(d, w):
    g = w.get_geometry()
    t = w.translate_coords(d.screen().root, 0, 0)
    return (-t.x, -t.y), (g.width, g.height)

def start(extra=None):
    log = open("/tmp/app.log", "a")
    p = subprocess.Popen(["/w/panic-button-desktop"], env=dict(env, **(extra or {})), stdout=log, stderr=log)
    d = display.Display(":99")
    for _ in range(60):
        w = find(d, "Panic Button button")
        if w and w.get_attributes().map_state == X.IsViewable:
            time.sleep(2)
            return p, d, w
        time.sleep(0.5)
    print(open("/tmp/app.log").read()[-3000:])
    sys.exit("button window never appeared")

def alarms():
    out = subprocess.run(["/w/panic-button", "--format", "json", "--control-socket", SOCK, "ctl", "alarms"],
                         capture_output=True, text=True)
    return json.loads(out.stdout or "[]")

subprocess.run(["/w/panic-button", "--data-dir", D, "practice", "init", "--name", "Test", "--label", "Desk",
                "--recovery-kit", "/tmp/kit"], env=dict(env, PANIC_BUTTON_PASSPHRASE="correct horse battery"),
               check=True, capture_output=True)

p, d, w = start()
(x, y), size = pos(d, w)
check("76 px button in the bottom-right corner", (x, y) == (1820, 980) and size == (76, 76), f"{(x, y)} size {size}")

w.configure(x=300, y=400)
d.sync()
time.sleep(2)
f = os.path.join(D, "button-position.json")
saved = json.load(open(f)) if os.path.exists(f) else None
check("move is saved", saved == {"positions": {"1920x1080+0+0@100": [300, 400]}}, str(saved))

# Shortcut: a short press is not an alarm; a 2.5 s hold (with key repeat) is.
def press(seconds):
    codes = [d.keysym_to_keycode(XK.string_to_keysym(k)) for k in ("Control_L", "Shift_L", "F12")]
    for c in codes:
        xtest.fake_input(d, X.KeyPress, c)
    d.sync()
    time.sleep(seconds)
    for c in reversed(codes):
        xtest.fake_input(d, X.KeyRelease, c)
    d.sync()
    time.sleep(1)

d.change_keyboard_control(auto_repeat_mode=X.AutoRepeatModeOn)
press(0.6)
check("short shortcut press raises nothing", alarms() == [], str(alarms()))
press(2.5)
a = alarms()
check("held shortcut raises an alarm", len(a) == 1 and a[0].get("own") is True, json.dumps(a)[:300])

def reasons():
    out = subprocess.run(["/w/panic-button", "--format", "json", "--control-socket", SOCK, "ctl", "status"],
                         capture_output=True, text=True)
    return json.loads(out.stdout or "{}").get("reasons", [])

subprocess.run(["pactl", "set-sink-mute", "@DEFAULT_SINK@", "1"], check=True)
time.sleep(12)
r = reasons()
check("muted sound is reported", "Sound is muted on this computer" in r, str(r))
subprocess.run(["pactl", "set-sink-mute", "@DEFAULT_SINK@", "0"], check=True)
time.sleep(12)
r = reasons()
check("unmuted sound clears it", "Sound is muted on this computer" not in r, str(r))

p.terminate()
p.wait(10)
time.sleep(1)
p, d, w = start()
(x, y), _ = pos(d, w)
check("position restored after restart", (x, y) == (300, 400), str((x, y)))

# A saved position on a monitor that is no longer there.
p.terminate()
p.wait(10)
json.dump({"positions": {"1920x1080+0+0@100": [2500, 400]}}, open(f, "w"))
p, d, w = start()
(x, y), _ = pos(d, w)
check("off-screen position falls back to the corner", (x, y) == (1820, 980), str((x, y)))
p.terminate()
p.wait(10)

# In an xrdp remote session the computer stops taking part, and says why.
p, d, w = start({"XRDP_SESSION": "1"})
time.sleep(3)
r = reasons()
check("remote session is reported", any("remote session" in x for x in r), str(r))
p.terminate()

log = open("/tmp/app.log").read()
check("shortcut registered", "keyboard shortcut keys=Ctrl+Shift+F12 active=true" in log,
      [l for l in log.splitlines() if "shortcut" in l][:3])
print("FAILURES: " + ", ".join(failures) if failures else "all passed")
sys.exit(1 if failures else 0)
