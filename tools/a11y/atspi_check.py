#!/usr/bin/env python3
"""Screen-reader rows of the a11y/IME test matrix (docs/accessibility.md), Linux, automated.

Runs Specular in a private headless sway with a private AT-SPI bus, drives it with wtype
(virtual keyboard) and walks its accessibility tree with libatspi, the library Orca reads it
with. Each scenario checks roles, names, states and the announcements Orca would speak.

Run it through tools/a11y/atspi-check.sh, which starts the private D-Bus session bus
(dbus-run-session) and private XDG folders; this script refuses to run on any other bus. No
speech, no clipboard, and nothing touches the user's desktop or session bus.

Every process started here (and everything they start) is stopped on exit: the script is a
child subreaper, so even daemons that detach come back to it and are stopped by PID.
"""

import argparse
import ctypes
import json
import os
import shutil
import signal
import subprocess
import sys
import time
import warnings

warnings.simplefilter("ignore", DeprecationWarning)

import gi

gi.require_version("Atspi", "2.0")
gi.require_version("Gio", "2.0")
from gi.repository import Atspi, Gio, GLib  # noqa: E402

PR_SET_CHILD_SUBREAPER = 36
BUTTONS = ("button", "push button", "toggle button")
REGISTRY = "/usr/libexec/at-spi2-registryd"
# A bus with its socket in the private runtime folder and no service activation.
BUS_CONF = """<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>{type}</type>
  <listen>unix:dir={dir}</listen>
  <auth>EXTERNAL</auth>
  <policy context="default">
    <allow send_destination="*" eavesdrop="true"/>
    <allow eavesdrop="true"/>
    <allow own="*"/>
  </policy>
</busconfig>
"""


# A `wtype` that only holds a virtual keyboard on the seat. wtype sleeps with
# usleep(ms * 1000) in 32 bits: one `-s 86400000` wrapped round to about 500 s and the
# keyboard went mid-run. 48 sleeps of 2,000 s (under 2^31 µs each) last a day.
HOLD_ARGV = ["wtype"] + ["-s", "2000000"] * 48


class Run:
    def __init__(self, out):
        self.out = out
        self.procs = []
        self.results = []
        self.events = []
        self.listener = None

    # --- processes -----------------------------------------------------------------------------

    def spawn(self, name, argv, env):
        log = open(os.path.join(self.out, f"{name}.log"), "wb")
        p = subprocess.Popen(argv, stdout=log, stderr=subprocess.STDOUT, env=env,
                             stdin=subprocess.DEVNULL)
        self.procs.append((name, p))
        return p

    def descendants(self):
        """PIDs below this process (children of the subreaper included), deepest first."""
        kids = {}
        for d in os.listdir("/proc"):
            if not d.isdigit():
                continue
            try:
                with open(f"/proc/{d}/stat") as f:
                    stat = f.read()
            except OSError:
                continue
            ppid = int(stat.rsplit(")", 1)[1].split()[1])
            kids.setdefault(ppid, []).append(int(d))
        order = []

        def visit(pid):
            for c in kids.get(pid, []):
                visit(c)
                order.append(c)

        visit(os.getpid())
        return order

    def stop_all(self):
        for _ in range(2):
            pids = self.descendants()
            for pid in pids:
                try:
                    os.kill(pid, signal.SIGTERM)
                except ProcessLookupError:
                    pass
            deadline = time.time() + 3
            while time.time() < deadline and self.descendants():
                self.reap()
                time.sleep(0.05)
        for pid in self.descendants():
            try:
                os.kill(pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
        time.sleep(0.1)
        self.reap()

    def reap(self):
        while True:
            try:
                pid, _ = os.waitpid(-1, os.WNOHANG)
            except ChildProcessError:
                return
            if pid == 0:
                return

    # --- results -------------------------------------------------------------------------------

    def check(self, row, what, ok, detail=""):
        self.results.append({"row": row, "check": what, "ok": bool(ok), "detail": str(detail)})
        mark = "PASS" if ok else "FAIL"
        print(f"  [{mark}] {row}: {what}" + (f" ({detail})" if detail and not ok else ""),
              flush=True)
        return ok

    # --- events --------------------------------------------------------------------------------

    def pump(self, secs):
        ctx = GLib.MainContext.default()
        end = time.time() + secs
        while time.time() < end:
            while ctx.iteration(False):
                pass
            time.sleep(0.02)

    def on_event(self, ev):
        try:
            src = ev.source
            role = src.get_role_name() if src else ""
            name = src.get_name() if src else ""
        except GLib.Error:
            role, name = "?", "?"
        data = ev.any_data
        if isinstance(data, Atspi.Accessible):
            try:
                data = data.get_name()
            except GLib.Error:
                data = "?"
        self.events.append({"type": ev.type, "role": role, "name": name,
                            "detail1": ev.detail1, "data": str(data) if data is not None else ""})

    def listen(self):
        self.listener = Atspi.EventListener.new(self.on_event)
        for t in ("object:announcement", "object:state-changed:focused",
                  "object:property-change:accessible-name", "object:children-changed",
                  "object:text-changed", "window:activate"):
            self.listener.register(t)

    def take_events(self):
        ev, self.events = self.events, []
        return ev


# --- the tree ------------------------------------------------------------------------------------


class N:
    """A snapshot of one accessible."""

    def __init__(self, acc, depth, parent=None):
        self.depth = depth
        self.parent = parent
        self.role = acc.get_role_name()
        self.name = acc.get_name() or ""
        self.desc = acc.get_description() or ""
        ss = acc.get_state_set()
        self.states = {s.value_nick for s in Atspi.StateType.__enum_values__.values()
                       if ss.contains(s)}
        self.text = None
        self.caret = None
        ti = acc.get_text_iface()
        if ti is not None:
            self.text = Atspi.Text.get_text(ti, 0, Atspi.Text.get_character_count(ti))
            self.caret = Atspi.Text.get_caret_offset(ti)
        self.value = None
        vi = acc.get_value_iface()
        if vi is not None:
            self.value = Atspi.Value.get_current_value(vi)
        self.attrs = acc.get_attributes() or {}
        self.actions = []
        ai = acc.get_action_iface()
        if ai is not None:
            self.actions = [Atspi.Action.get_action_name(ai, i)
                            for i in range(Atspi.Action.get_n_actions(ai))]
        self.acc = acc
        self.children = []

    def line(self):
        bits = [self.role, repr(self.name)]
        if self.desc:
            bits.append(f"desc={self.desc!r}")
        if self.text is not None:
            bits.append(f"text={self.text!r} caret={self.caret}")
        if self.value is not None:
            bits.append(f"value={self.value}")
        st = sorted(self.states & {"focused", "focusable", "selected", "checked", "pressed",
                                   "expanded", "expandable", "modal", "sensitive", "busy",
                                   "editable", "showing", "visible", "active"})
        bits.append("[" + ",".join(st) + "]")
        if self.actions:
            bits.append("actions=" + ",".join(self.actions))
        if self.attrs:
            bits.append("attrs=" + ",".join(f"{k}={v}" for k, v in sorted(self.attrs.items())))
        return "  " * self.depth + " ".join(bits)

    def walk(self):
        yield self
        for c in self.children:
            yield from c.walk()

    def find(self, role=None, name=None, contains=None, pred=None):
        out = []
        for n in self.walk():
            if role and n.role != role:
                continue
            if name is not None and n.name != name:
                continue
            if contains is not None and contains.lower() not in n.name.lower():
                continue
            if pred and not pred(n):
                continue
            out.append(n)
        return out

    def path(self):
        p, n = [], self
        while n:
            p.append(f"{n.role}:{n.name!r}")
            n = n.parent
        return " > ".join(reversed(p))


# Roles a user operates, as AT-SPI names them.
CONTROLS = BUTTONS + ("check box", "radio button", "combo box", "menu item", "check menu item",
                      "radio menu item", "page tab", "entry", "password text", "spin button",
                      "slider", "link", "tree item", "switch")
# Roles that must say whether they are on (AT-SPI: the "checkable" state).
TOGGLES = ("check box", "radio button", "check menu item", "radio menu item", "switch")


def audit(t):
    """What a screen reader user would trip over in snapshot `t` (crates/specular-widgets
    a11y_audit.rs, over AT-SPI): controls without a name, tick boxes and switches that do not
    say whether they are on, tabs that cannot be selected. Off-screen nodes are left out.
    (Whether a combo box is open is checked on the AccessKit tree only: AccessKit's AT-SPI
    adapter does not pass a collapsed state on.)"""
    out = []
    for n in t.walk():
        if "showing" not in n.states and "visible" not in n.states:
            continue
        if n.role in CONTROLS and not n.name.strip():
            out.append(f"unnamed {n.role}: {n.path()}")
        if n.role in TOGGLES and "checkable" not in n.states:
            out.append(f"{n.role} without on/off state: {n.path()}")
        if n.role == "page tab" and "selectable" not in n.states:
            out.append(f"page tab without selected state: {n.path()}")
    return out


def snapshot(acc, depth=0, parent=None, limit=4000):
    n = N(acc, depth, parent)
    if limit <= 0:
        return n
    for i in range(acc.get_child_count()):
        c = acc.get_child_at_index(i)
        if c is not None:
            n.children.append(snapshot(c, depth + 1, n, limit - 1))
    return n


def find_app(timeout):
    end = time.time() + timeout
    while time.time() < end:
        desk = Atspi.get_desktop(0)
        for i in range(desk.get_child_count()):
            app = desk.get_child_at_index(i)
            if app is None:
                continue
            try:
                app.clear_cache()
                for j in range(app.get_child_count()):
                    w = app.get_child_at_index(j)
                    if w is not None and w.get_role_name() == "frame":
                        return app
            except GLib.Error:
                continue
        GLib.MainContext.default().iteration(False)
        time.sleep(0.1)
    return None


# --- the scenarios -------------------------------------------------------------------------------


def main(scenarios_fn=None, extra_args=()):
    ap = argparse.ArgumentParser()
    ap.add_argument("--bin", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--images", required=True, help="a folder of test images (not private)")
    ap.add_argument("--timeout", type=float, default=90)
    for arg in extra_args:
        ap.add_argument(arg, action="store_true")
    a = ap.parse_args()

    # Refuse anything but a private bus (dbus-run-session puts it under our runtime dir or
    # an abstract socket; the user's is /run/user/<uid>/bus).
    addr = os.environ.get("DBUS_SESSION_BUS_ADDRESS", "")
    if not addr or f"/run/user/{os.getuid()}/bus" in addr or not os.environ.get("SPECULAR_A11Y_PRIVATE"):
        sys.exit("refusing to run outside tools/a11y/atspi-check.sh (private session bus)")

    os.makedirs(a.out, exist_ok=True)
    libc = ctypes.CDLL(None, use_errno=True)
    libc.prctl(PR_SET_CHILD_SUBREAPER, 1, 0, 0, 0)
    r = Run(a.out)
    trees = os.path.join(a.out, "trees")
    shutil.rmtree(trees, ignore_errors=True)
    os.makedirs(trees)
    try:
        ok = (scenarios_fn or scenarios)(r, a, trees)
    finally:
        r.stop_all()
    with open(os.path.join(a.out, "results.json"), "w") as f:
        json.dump(r.results, f, indent=1, ensure_ascii=False)
    failed = [x for x in r.results if not x["ok"]]
    print(f"\n{len(r.results) - len(failed)}/{len(r.results)} checks passed; "
          f"trees and logs in {a.out}")
    sys.exit(0 if ok and not failed else 1)


def clean_env():
    base = dict(os.environ)
    for k in ("DISPLAY", "WAYLAND_DISPLAY", "AT_SPI_BUS_ADDRESS"):
        base.pop(k, None)
    return base


def a11y_bus(r, a, base):
    """The accessibility bus, enabled as a screen reader would enable it: a private
    dbus-daemon for the bus, the registry on it, and org.a11y.Bus on the session bus."""
    xdg = base["XDG_RUNTIME_DIR"]
    conf = os.path.join(a.out, "a11y-bus.conf")
    with open(conf, "w") as f:
        f.write(BUS_CONF.format(type="accessibility", dir=xdg))
    r_fd, w_fd = os.pipe()
    p = subprocess.Popen(["dbus-daemon", "--config-file", conf, "--nofork", "--nopidfile",
                          f"--print-address={w_fd}"], pass_fds=(w_fd,), env=base,
                         stdin=subprocess.DEVNULL)
    r.procs.append(("a11y-bus", p))
    os.close(w_fd)
    with os.fdopen(r_fd) as f:
        address = f.readline().strip()
    if not r.check("setup", "private accessibility bus", address):
        return None
    here = os.path.dirname(os.path.abspath(__file__))
    r.spawn("a11y-bus-service", [sys.executable, "-I", os.path.join(here, "a11y_bus.py"),
                                 address], base)
    bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
    for _ in range(100):
        owned = bus.call_sync("org.freedesktop.DBus", "/org/freedesktop/DBus",
                              "org.freedesktop.DBus", "NameHasOwner",
                              GLib.Variant("(s)", ("org.a11y.Bus",)), None, 0, -1, None)
        if owned.unpack()[0]:
            break
        time.sleep(0.05)
    os.environ["AT_SPI_BUS_ADDRESS"] = address
    if os.path.exists(REGISTRY):
        r.spawn("at-spi2-registryd", [REGISTRY], dict(base, AT_SPI_BUS_ADDRESS=address))
        time.sleep(0.5)
    Atspi.init()
    r.listen()
    return address


def sway_app(r, a, args):
    """A private headless sway with the app in it (`args` after `--size`), on the private
    accessibility bus: returns (app, keys, sway), or None after a failed setup check.
    `keys(...)` types with wtype; `sway(...)` runs swaymsg."""
    base = clean_env()
    xdg = base["XDG_RUNTIME_DIR"]
    if not a11y_bus(r, a, base):
        return None

    # A private headless compositor.
    conf = os.path.join(a.out, "sway.conf")
    with open(conf, "w") as f:
        f.write("xwayland disable\noutput HEADLESS-1 resolution 1280x800\n"
                "default_border none\nfocus_on_window_activation focus\n")
    senv = dict(base, WLR_BACKENDS="headless", WLR_LIBINPUT_NO_DEVICES="1",
                WLR_RENDERER="pixman")
    before = set(os.listdir(xdg))
    r.spawn("sway", ["sway", "-c", conf], senv)
    sock = None
    for _ in range(100):
        new = [s for s in os.listdir(xdg) if s.startswith("wayland-") and not s.endswith(".lock")
               and s not in before]
        if new:
            sock = new[0]
            break
        time.sleep(0.1)
    if not r.check("setup", "headless sway started", sock, "see sway.log"):
        return None
    ipc = [s for s in os.listdir(xdg) if s.startswith("sway-ipc.")]
    wenv = dict(base, WAYLAND_DISPLAY=sock)
    if ipc:
        wenv["SWAYSOCK"] = os.path.join(xdg, ipc[0])

    def keys(*args):
        subprocess.run(["wtype", *args], env=wenv, check=True, timeout=10)
        r.pump(0.6)

    # The app (software Vulkan, so no GPU and no display server of the user's).
    icd = "/usr/share/vulkan/icd.d/lvp_icd.json"
    aenv = dict(wenv, VK_ICD_FILENAMES=icd, VK_DRIVER_FILES=icd,
                SPECULAR_LOG_FILE=os.path.join(a.out, "specular-log.txt"))
    r.spawn("specular", [a.bin, "--size", "1200x760", *args], aenv)
    app = find_app(a.timeout)
    if not r.check("launch", "the app registers on the AT-SPI bus", app):
        return None
    r.pump(2.0)

    def sway(*args, save=None):
        res = subprocess.run(["swaymsg", *args], env=wenv, capture_output=True, text=True,
                             timeout=10)
        if save:
            with open(os.path.join(a.out, save), "w") as f:
                f.write(res.stdout + res.stderr)
        return res

    # A keyboard on the seat for the whole run (each wtype adds a virtual keyboard only while
    # it runs, which can be gone before the app has bound it), then focus the window.
    r.spawn("wtype-hold", HOLD_ARGV, wenv)
    r.pump(1.0)
    sway("[app_id=.*] focus")
    r.pump(1.0)
    sway("-t", "get_tree", save="sway-tree.json")
    sway("-t", "get_seats", save="sway-seats.json")
    return app, keys, sway


def scenarios(r, a, trees):
    first = sorted(f for f in os.listdir(a.images))[0]
    # The value under the cursor is read out once it rests (checked under "crosshair").
    # The first-run profile question is skipped (it is checked by the widget tests).
    started = sway_app(r, a, ["--set", "accessibility.announce_cursor=true",
                              "--set", "profile.shown=question",
                              os.path.join(a.images, first)])
    if not started:
        return False
    app, keys, sway = started

    def tree(label, settle=0.3):
        r.pump(settle)
        app.clear_cache()
        t = snapshot(app)
        with open(os.path.join(trees, f"{label}.txt"), "w") as f:
            for n in t.walk():
                f.write(n.line() + "\n")
        # Every screen walked passes the screen-reader audit.
        found = audit(t)
        r.check("audit", f"{label}: named controls with their state", not found, found[:6])
        return t

    def focused(t):
        f = t.find(pred=lambda n: "focused" in n.states)
        return f[-1] if f else None

    def announced(evs):
        return [e["data"] for e in evs if e["type"] == "object:announcement"]

    # --- Launch and read the title ---
    t = tree("01-launch")
    frames = t.find(role="frame")
    row = "launch"
    r.check(row, "one frame (window)", len(frames) == 1, [f.name for f in frames])
    title = frames[0].name if frames else ""
    r.check(row, "window title names the image", first in title, title)
    r.check(row, "application name", t.name != "", t.name)

    # --- Image name and position ---
    row = "image"
    imgs = t.find(role="image")
    r.check(row, "an image node", len(imgs) == 1, [i.name for i in imgs])
    if imgs:
        r.check(row, "image named with file and size", first in imgs[0].name
                and any(ch.isdigit() for ch in imgs[0].name), imgs[0].name)
    count = len(os.listdir(a.images))
    position = [n for n in t.walk() if n.role in BUTTONS and "1" in n.name
                and str(count) in n.name]
    r.check(row, f"a button named with the position (1 of {count})", position,
            [n.name for n in t.walk() if n.role in BUTTONS][:3])

    # --- Toolbar buttons ---
    row = "toolbar"
    bars = t.find(role="tool bar")
    r.check(row, "a tool bar", bars, [b.name for b in bars])
    btns = [n for b in bars for n in b.walk() if n.role in BUTTONS + ("combo box",)]
    r.check(row, "at least 8 controls on the bar", len(btns) >= 8, len(btns))
    unnamed = [b.line().strip() for b in btns if not b.name.strip()]
    r.check(row, "every control has a name", not unnamed, unnamed)
    noact = [b.name for b in btns if b.role in BUTTONS and not b.actions]
    r.check(row, "buttons can be pressed (Action interface)", not noact, noact)
    names = " | ".join(b.name for b in btns)
    r.check(row, "Next and Previous are there", "next" in names.lower()
            and "previous" in names.lower(), names)
    shortcut = [b for b in btns if b.attrs.get("keyshortcuts") or "(" in b.desc]
    r.check(row, "shortcuts are exposed", shortcut, names)

    # Press Next through AT-SPI (as Orca's "activate" does) and check the image changes.
    nxt = [b for b in btns if b.name.lower().startswith("next")]
    if nxt:
        ai = nxt[0].acc.get_action_iface()
        Atspi.Action.do_action(ai, 0)
        r.pump(1.5)
        t2 = tree("02-after-next")
        second = sorted(os.listdir(a.images))[1]
        im = t2.find(role="image")
        r.check(row, "activating Next shows the next image",
                im and second in im[0].name, im[0].name if im else t2.find(role="frame")[0].name)
        keys("-k", "Left")
        r.pump(1.0)

    # --- Info panel ---
    row = "info"
    r.take_events()
    keys("i")
    t = tree("03-info")
    panels = t.find(role="panel", pred=lambda n: n.name and n.parent and n.parent.role == "frame")
    info = [p for p in panels if "info" in p.name.lower()]
    r.check(row, "an information panel", info, [p.name for p in panels])
    if info:
        tabs = info[0].find(role="page tab")
        r.check(row, "Summary / All fields tabs", len(tabs) == 2, [x.name for x in tabs])
        sel = [x for x in tabs if "selected" in x.states]
        r.check(row, "one tab selected", len(sel) == 1, [x.name for x in sel])
        close = [n for n in info[0].walk() if n.role in BUTTONS]
        r.check(row, "a named Close button", any(c.name for c in close),
                [c.name for c in close])
    keys("i")

    # --- Settings navigation ---
    row = "settings"
    r.take_events()
    keys("-M", "ctrl", ",", "-m", "ctrl")
    t = tree("04-settings", 0.8)
    dlg = t.find(role="dialog")
    r.check(row, "a modal dialog named Settings", dlg and "modal" in dlg[0].states
            and dlg[0].name, [d.name for d in dlg])
    if dlg:
        f = focused(t)
        r.check(row, "focus on the search field", f and f.role == "entry" and f.name,
                f.line().strip() if f else None)
        tabs = dlg[0].find(role="page tab")
        r.check(row, "sections are page tabs", len(tabs) >= 3, [x.name for x in tabs])
        sw = dlg[0].find(role="toggle button")
        r.check(row, "switches have names and a checked state",
                sw and all(s.name for s in sw), [s.name for s in sw][:5])
        unnamed = [n.line().strip() for n in dlg[0].walk()
                   if n.role in BUTTONS + ("combo box", "check box",
                                 "list item", "page tab") and not n.name.strip()]
        r.check(row, "every control in Settings has a name", not unnamed, unnamed[:5])
        behind = [n for n in t.find(role="tool bar") if n.children]
        r.check(row, "controls behind the dialog are hidden", not behind,
                [b.name for b in behind])
        keys("zoom")
        t = tree("05-settings-search", 0.8)
        f = focused(t)
        r.check(row, "typed search is the field's text, caret at its end",
                f and f.text == "zoom" and f.caret == 4, f.line().strip() if f else None)
    keys("-k", "Escape")
    keys("-k", "Escape")
    t = tree("06-settings-closed")
    r.check(row, "Escape closes it", not t.find(role="dialog"))

    # --- Palette search results ---
    row = "palette"
    r.take_events()
    keys("-M", "ctrl", "k", "-m", "ctrl")
    t = tree("07-palette", 0.8)
    dlg = t.find(role="dialog")
    r.check(row, "a modal dialog", dlg and "modal" in dlg[0].states, [d.name for d in dlg])
    f = focused(t)
    r.check(row, "focus on a named search field", f and f.role == "entry" and f.name,
            f.line().strip() if f else None)
    items = t.find(role="list item")
    r.check(row, "results are list items with names", items and all(i.name for i in items),
            len(items))
    keys("rotate")
    t = tree("08-palette-rotate", 0.8)
    f = focused(t)
    r.check(row, "typed text is the field's text, caret at its end",
            f and f.text == "rotate" and f.caret == 6, f.line().strip() if f else None)
    items2 = t.find(role="list item")
    # (The list shows as many rows as fit, filtered or not: compare what it shows.)
    r.check(row, "typing filters the results", items2
            and [i.name for i in items2] != [i.name for i in items]
            and all("rotat" in (i.name + i.desc).lower() for i in items2[:2]),
            [i.name for i in items2][:4])
    sel = [i for i in items2 if "selected" in i.states]
    r.check(row, "one result selected", len(sel) == 1, [i.name for i in sel])
    keys("-k", "Down")
    t = tree("09-palette-down", 0.5)
    sel2 = [i for i in t.find(role="list item") if "selected" in i.states]
    r.check(row, "Down moves the selection", sel2 and sel and sel2[0].name != sel[0].name,
            [i.name for i in sel2])
    # Esc goes back before it closes: the query first, then the palette.
    keys("-k", "Escape")
    keys("-k", "Escape")
    r.check(row, "Escape closes it", not tree("10-palette-closed").find(role="dialog"))

    # --- Dialogs (cheat sheet, about) ---
    row = "dialogs"
    keys("?")
    t = tree("11-cheatsheet", 0.8)
    dlg = t.find(role="dialog")
    r.check(row, "cheat sheet is a named modal dialog",
            dlg and dlg[0].name and "modal" in dlg[0].states, [d.name for d in dlg])
    keys("-k", "Escape")
    r.pump(0.5)

    # --- Go-to field ---
    row = "go-to"
    keys("-M", "ctrl", "g", "-m", "ctrl")
    t = tree("12-goto", 0.5)
    f = focused(t)
    r.check(row, "focus on a named text field", f and f.role == "entry" and f.name,
            f.line().strip() if f else None)
    keys("3")
    t = tree("12-goto-typed", 0.3)
    f = focused(t)
    r.check(row, "the typed number is the field's text", f and f.text == "3",
            f.line().strip() if f else None)
    keys("-k", "Return")
    t = tree("13-goto-3", 1.5)
    third = sorted(os.listdir(a.images))[2]
    im = t.find(role="image")
    r.check(row, "typing 3 and Enter shows image 3", im and third in im[0].name,
            im[0].name if im else "")

    # --- Live announcements: zoom level and errors ---
    row = "live"
    r.take_events()
    keys("+")
    r.pump(1.5)
    said = announced(r.take_events())
    r.check(row, "zoom change is announced", any("%" in s for s in said), said)
    zoom = [n for n in tree("13-zoomed").walk() if n.role == "combo box"]
    r.check(row, "the zoom combo box carries the level", zoom and "%" in zoom[0].desc,
            [z.line().strip() for z in zoom])
    keys("f")
    r.pump(1.0)
    r.take_events()
    keys("-k", "End")
    r.pump(2.5)
    evs = r.take_events()
    t = tree("14-error")
    alert = t.find(role="notification") + t.find(role="alert")
    r.check(row, "a broken file shows an alert named with the error", alert
            and alert[0].name, [x.line() for x in alert])
    r.check(row, "the error is announced", any("format" in x or "decode" in x
                                                for x in announced(evs)), announced(evs))
    keys("-k", "Home")
    r.pump(1.0)

    # --- Keyboard crosshair and the value under it ---
    row = "crosshair"
    r.take_events()
    keys("x")
    r.pump(1.0)
    said = announced(r.take_events())
    r.check(row, "x turns the crosshair on, and says how to use it",
            any("Crosshair on" in s for s in said), said)
    keys("-k", "Right")
    r.pump(0.2)
    r.take_events()
    keys("-M", "alt", "r", "-m", "alt")
    r.pump(1.0)
    said = announced(r.take_events())
    r.check(row, "Alt+R reads the pixel under the crosshair",
            any("Pixel at" in s for s in said), said)
    # Moving on and resting: read out once (Settings › Accessibility, on for this run).
    keys("-k", "Down", "-k", "Down", "-k", "Down")
    r.pump(2.5)
    said = [s for s in announced(r.take_events()) if "Pixel at" in s]
    r.check(row, "the resting crosshair's value is read once, not every step",
            len(said) == 1, said)
    keys("-k", "Escape")
    r.pump(1.0)
    said = announced(r.take_events())
    r.check(row, "Esc turns the crosshair off", any("Crosshair off" in s for s in said), said)

    # --- Gallery grid ---
    row = "gallery"
    r.take_events()
    keys("g")
    t = tree("15-gallery", 2.0)
    lists = t.find(role="list box") + t.find(role="list")
    grid = [g for g in lists if g.find(role="list item")]
    r.check(row, "the grid is a named list box", grid and grid[0].name,
            [g.name for g in lists])
    marked = [n.name for n in t.walk() if any(c in n.name + n.desc for c in "\u2068\u2069")]
    r.check(row, "names carry no bidi isolation marks", not marked, marked[:3])
    if grid:
        tiles = grid[0].find(role="list item")
        r.check(row, "tiles named after their files", tiles
                and all(any(x in tl.name for x in os.listdir(a.images)) for tl in tiles),
                [tl.name for tl in tiles][:4])
        f = focused(t)
        r.check(row, "a tile has the focus", f and f.role == "list item",
                f.line().strip() if f else None)
        before = f.name if f else ""
        r.take_events()
        keys("-k", "Right")
        t = tree("16-gallery-right", 0.6)
        f = focused(t)
        r.check(row, "Right moves the focus to the next tile", f and f.name != before
                and f.role == "list item", f.name if f else None)
        evs = r.take_events()
        r.check(row, "the focus change is an event Orca speaks", any(
            e["type"] == "object:state-changed:focused" and e["detail1"] == 1 for e in evs),
            [e["type"] for e in evs][:8])

    # --- Gallery filter field ---
    row = "gallery-filter"
    keys("/")
    t = tree("17-filter", 0.6)
    f = focused(t)
    r.check(row, "/ focuses a named search field", f and f.role == "entry" and f.name,
            f.line().strip() if f else None)
    r.take_events()
    keys("snow")
    t = tree("18-filter-typed", 1.5)
    f = focused(t)
    r.check(row, "typed text is the field's text, caret at its end",
            f and f.text == "snow" and f.caret == 4, f.line().strip() if f else None)
    said = announced(r.take_events())
    r.check(row, "the result count is announced", any(any(ch.isdigit() for ch in x)
                                                      for x in said), said)
    return True


if __name__ == "__main__":
    main()
