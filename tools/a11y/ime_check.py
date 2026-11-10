#!/usr/bin/env python3
"""IME rows of the a11y/IME test matrix (docs/accessibility.md), Linux/IBus, automated.

Runs Specular on a private Xvfb with a private IBus daemon (XIM front end, Cangjie 5 table
engine), types Cangjie codes with xdotool and checks what reaches the app (its `input` log:
winit's Ime events and the IME cursor area it reports) and what a screen reader then reads
(the focused field's text over AT-SPI, on the same private accessibility bus as
atspi_check.py).

Run it through `tools/a11y/atspi-check.sh --ime`; like atspi_check.py it refuses to run on any
bus but the private one, and every process it starts is stopped on exit.
"""

import os
import re
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import atspi_check as ac  # noqa: E402

# Cangjie 5: "a" is 日 (first candidate); "hqi" is 我.
CODE, CODE_CHAR = "a", "日"


def free_display():
    """An X display number with neither a socket file nor an abstract socket in use."""
    with open("/proc/net/unix", errors="replace") as f:
        used = f.read()
    for n in range(240, 400):
        if f"/tmp/.X11-unix/X{n}\n" in used or os.path.exists(f"/tmp/.X11-unix/X{n}"):
            continue
        return n
    return None


def scenarios(r, a, trees):
    base = ac.clean_env()
    run = base["XDG_RUNTIME_DIR"]
    if not ac.a11y_bus(r, a, base):
        return False

    # 1. A private X server: no TCP, no socket file, no lock file (abstract socket only).
    n = free_display()
    if not r.check("setup", "a free X display number", n is not None):
        return False
    display = f":{n}"
    r.spawn("xvfb", ["Xvfb", display, "-nolisten", "tcp", "-nolisten", "unix", "-nolock",
                     "-screen", "0", "1280x800x24", "-nocursor"], base)
    xenv = dict(base, DISPLAY=display)
    for _ in range(100):
        if subprocess.run(["xdotool", "getdisplaygeometry"], env=xenv, capture_output=True,
                          timeout=5).returncode == 0:
            break
        time.sleep(0.1)
    else:
        r.check("setup", "Xvfb started", False, "see xvfb.log")
        return False

    # 2. A private IBus with its XIM server and the Cangjie 5 table engine.
    sock = os.path.join(run, "ibus.sock")
    ienv = dict(xenv, IBUS_ADDRESS=f"unix:path={sock}")
    r.spawn("ibus-daemon", ["ibus-daemon", "--xim", "--panel=disable", "--config=disable",
                            "--emoji-extension=disable", f"--address=unix:path={sock}",
                            "--cache=refresh", "--verbose"], ienv)
    engine = None
    for _ in range(150):
        res = subprocess.run(["ibus", "engine", "table:cangjie5"], env=ienv,
                             capture_output=True, text=True, timeout=10)
        if res.returncode == 0:
            engine = subprocess.run(["ibus", "engine"], env=ienv, capture_output=True,
                                    text=True, timeout=10).stdout.strip()
            break
        time.sleep(0.2)
    if not r.check("setup", "IBus runs the Cangjie 5 table engine", engine == "table:cangjie5",
                   engine or res.stderr.strip()):
        return False
    # The daemon answers before its XIM front end (ibus-x11) has registered on the X server.
    # An app started in between finds no input method, and connects only later (when the
    # server appears), so its first field's Enabled event may come late or not at all.
    for _ in range(150):
        servers = subprocess.run(["xprop", "-root", "XIM_SERVERS"], env=xenv,
                                 capture_output=True, text=True, timeout=10).stdout
        if "@server=ibus" in servers:
            break
        time.sleep(0.1)
    if not r.check("setup", "IBus's XIM server is registered", "@server=ibus" in servers,
                   servers.strip()):
        return False

    # 3. The app on X11, with IBus as its XIM input method and the input log on.
    icd = "/usr/share/vulkan/icd.d/lvp_icd.json"
    log = os.path.join(a.out, "ime-specular-log.txt")
    if os.path.exists(log):
        os.remove(log)
    aenv = dict(ienv, VK_ICD_FILENAMES=icd, VK_DRIVER_FILES=icd, XMODIFIERS="@im=ibus",
                SPECULAR_LOG="input:debug", SPECULAR_LOG_FILE=log)
    first = sorted(os.listdir(a.images))[0]
    # The first-run profile question is skipped (it is checked by the widget tests).
    r.spawn("specular-x11", [a.bin, "--size", "1200x760", "--set", "profile.shown=question",
                             os.path.join(a.images, first)], aenv)
    app = ac.find_app(a.timeout)
    if not r.check("ime-setup", "the app runs on X11 and registers on AT-SPI", app):
        return False
    r.pump(2.0)
    win = subprocess.run(["xdotool", "search", "--sync", "--onlyvisible", "--pid",
                          str(r.procs[-1][1].pid)], env=xenv, capture_output=True, text=True,
                         timeout=30).stdout.split()
    if not r.check("ime-setup", "an X window", win):
        return False
    subprocess.run(["xdotool", "windowfocus", "--sync", win[-1]], env=xenv, timeout=10)
    r.pump(0.5)

    def keys(*args, settle=0.6):
        subprocess.run(["xdotool", *args], env=xenv, check=True, timeout=10)
        r.pump(settle)

    def until(pred, secs=5.0):
        """Pumps events until `pred()` returns something true (returned) or `secs` pass
        (then its last value): waits for a state instead of for a fixed time."""
        end = time.time() + secs
        while True:
            v = pred()
            if v or time.time() > end:
                return v
            r.pump(0.1)

    def out(*argv):
        return subprocess.run(list(argv), env=ienv, capture_output=True, text=True,
                              timeout=10).stdout

    def set_engine(name):
        """Switches IBus's engine and waits until it reports it."""
        subprocess.run(["ibus", "engine", name], env=ienv, timeout=10)
        return until(lambda: out("ibus", "engine").strip() == name)

    def set_layout(layout):
        """Sets the X layout and waits until the server reports it."""
        subprocess.run(["setxkbmap", "-layout", layout], env=xenv, timeout=10)
        return until(lambda: re.search(rf"^layout:\s+{layout}$", out("setxkbmap", "-query"),
                                       re.M))

    seen = [0]

    def new_log():
        """Input-log lines since the last call."""
        try:
            with open(log, encoding="utf-8", errors="replace") as f:
                lines = f.read().splitlines()
        except OSError:
            return []
        out, seen[0] = lines[seen[0]:], len(lines)
        return out

    def ime_events(lines):
        return [m.group(1) for x in lines if (m := re.search(r"\bime (\w+\(.*\)|\w+)$", x))]

    def areas(lines):
        return [m.group(1) for x in lines if (m := re.search(r"ime area (.*)$", x))]

    def field(label):
        r.pump(0.3)
        app.clear_cache()
        t = ac.snapshot(app)
        with open(os.path.join(trees, f"{label}.txt"), "w") as f:
            for node in t.walk():
                f.write(node.line() + "\n")
        foc = t.find(pred=lambda x: "focused" in x.states)
        return foc[-1] if foc else None

    def entry(label):
        """The focused node when it is a text field, else None."""
        f = field(label)
        return f if f and f.role == "entry" else None

    def ime_rows(row, open_keys, close_keys, label, want=None):
        """`want(text, caret)`: the committed field's text and caret are right (default: the
        committed character alone, the caret after it)."""
        want = want or (lambda text, caret: text == CODE_CHAR and caret == 1)
        new_log()
        keys(*open_keys, settle=0.2)
        # Wait for the state (the field focused, its IME area reported), not for a time.
        lines = []

        def allowed():
            lines.extend(new_log())
            return any(x.startswith("Some") for x in areas(lines))

        until(allowed)
        f = until(lambda: entry(f"ime-{label}-open"))
        lines.extend(new_log())
        r.check(row, "the field is focused", f and f.role == "entry",
                f.line().strip() if f else None)
        r.check(row, "the app reports an IME cursor area", any(x.startswith("Some")
                                                              for x in areas(lines)),
                areas(lines))
        start = [x for x in areas(lines) if x.startswith("Some")]
        opened = lines

        # Preedit: the composition is shown, the candidate window follows the caret.
        keys("type", "--delay", "80", CODE)
        lines = new_log()
        # The first CJK glyph can take a moment (fallback font lookup) before the frame that
        # moves the area; wait for it rather than for a fixed time.
        for _ in range(30):
            if any(x.startswith("Some") and x not in start for x in areas(lines)):
                break
            r.pump(0.1)
            lines += new_log()
        # winit's X11 backend sends Enabled when the input method starts composing (XIM's
        # preedit start), not when the field takes the keys: sometimes on focus, mostly with
        # the first key. So it is checked before the first Preedit, not right after opening.
        evs = ime_events(opened + lines)
        first = next((k for k, e in enumerate(evs) if e.startswith("Preedit(")), len(evs))
        r.check(row, "IME is enabled before the first preedit",
                any(e == "Enabled" for e in evs[:first]), evs)
        pre = [e for e in ime_events(lines) if e.startswith("Preedit(") and '""' not in e]
        r.check(row, "preedit: composing reaches the app as Preedit", pre, ime_events(lines))
        moved = [x for x in areas(lines) if x.startswith("Some")]
        r.check(row, "candidate window: the IME area moves with the composition",
                moved and (not start or moved[-1] != start[-1]), (start[-1:], moved))
        f = field(f"ime-{label}-preedit")
        r.check(row, "preedit is not committed text yet", f and CODE_CHAR not in (f.text or ""),
                f.line().strip() if f else None)

        # Commit: Space picks the first candidate.
        keys("key", "space")
        lines = new_log()
        commits = [e for e in ime_events(lines) if e.startswith("Commit(")]
        r.check(row, f"commit: Space commits {CODE_CHAR}", any(CODE_CHAR in c for c in commits),
                ime_events(lines))
        f = field(f"ime-{label}-commit")
        r.check(row, f"commit: the field's text has {CODE_CHAR!r}, caret after it",
                f and want(f.text or "", f.caret), f.line().strip() if f else None)
        committed = f.text if f else None

        # Cancel: Escape drops the composition and leaves the field (and dialog) open.
        keys("type", "--delay", "80", CODE)
        new_log()
        keys("key", "Escape")
        lines = new_log()
        evs = ime_events(lines)
        r.check(row, "cancel: Escape clears the preedit without committing",
                any(e in ('Preedit("", None)',) or e.startswith('Preedit("",') for e in evs)
                and not any(e.startswith("Commit(") for e in evs), evs)
        f = field(f"ime-{label}-cancel")
        r.check(row, "cancel: the field stays focused with only the committed text",
                f and f.role == "entry" and f.text == committed, f.line().strip() if f else None)
        keys(*close_keys, settle=0.2)
        r.check(row, "closing leaves the field",
                until(lambda: not entry(f"ime-{label}-closed")),
                "a text field still has the focus")
        new_log()

    ime_rows("ime-palette", ["key", "ctrl+k"], ["key", "Escape", "Escape"], "palette")
    ime_rows("ime-settings", ["key", "ctrl+comma"], ["key", "Escape", "Escape", "Escape"],
             "settings")
    # Rename (F2): the stem is selected, so the commit replaces it and the extension stays;
    # Esc then closes the sheet without renaming.
    # (F2 by its keycode: xdotool sends the keysym with Alt held under this keymap.)
    ime_rows("ime-rename", ["key", "68"], ["key", "Escape"], "rename",
             want=lambda text, caret: text.startswith(CODE_CHAR) and text.endswith(".jpg")
             and caret == 1)
    keys("key", "g", settle=2.0)
    ime_rows("ime-gallery-filter", ["key", "slash"], ["key", "Escape", "Escape"], "filter")
    keys("key", "g", settle=2.0)

    # RTL: IBus's Hebrew keyboard engine (it switches the X layout to il), typed on the keys a
    # Hebrew typist uses: A K U O = ש ל ו ם. (xdotool's own non-layout typing remaps one
    # spare keycode per character and loses all but the last under XIM, so it is not used.)
    # Emoji: no layout has them, so two keys get emoji keysyms (as a compose or emoji picker
    # would deliver them); 👍🏽 is a base emoji plus a skin-tone modifier.
    # (Without its panel IBus does not switch the X layout itself, so setxkbmap does.)
    # Each switch is waited for until the server reports it. The palette opens on Ctrl and
    # K's key code (45): the Hebrew layout has no "k" keysym, and asking xdotool for one makes
    # it remap a spare key code and back around the press, which races the app's reading of
    # the changed keymap (the app takes Ctrl+ל as Ctrl+K by the key's position).
    r.check("ime-rtl", "the Hebrew engine and layout are on",
            set_engine("xkb:il::heb") and set_layout("il"))
    rows = (("ime-rtl", "שלום", ["38", "45", "30", "32"]),
            ("ime-emoji", "👍🏽", ["38", "39"]))
    for row, text, presses in rows:
        if row == "ime-emoji":
            state = [bool(set_engine("xkb:us::eng")), bool(set_layout("us"))]
            subprocess.run(["xmodmap", "-e", "keycode 38 = U1F44D", "-e", "keycode 39 = U1F3FD"],
                           env=xenv, timeout=10)
            state.append(bool(until(lambda: re.search(r"^keycode\s+39 = U0*1F3FD",
                                                      out("xmodmap", "-pke"), re.M))))
            r.check(row, "the US engine and layout, with two emoji keys, are on", all(state),
                    (state, out("ibus", "engine").strip(),
                     [x for x in out("xmodmap", "-pke").splitlines() if " 39 " in x]))
            # The app reads the changed keymap when the server tells it (MappingNotify).
            r.pump(0.5)
        keys("key", "ctrl+45", settle=0.2)
        f = until(lambda: entry(f"{row}-open"))
        r.check(row, "Ctrl+K opens the palette's field", f, "no text field has the focus")
        new_log()
        keys("key", "--delay", "120", *presses, settle=0.2)
        f = until(lambda: (lambda g: g if g and g.text == text else None)(field(row)), 5.0)
        f = f or field(row)
        r.check(row, f"typed {text!r} is the field's text, caret at its end",
                f and f.text == text and f.caret == len(text), f.line().strip() if f else None)
        # The first Escape clears the query, the second closes the palette.
        keys("key", "Escape", "Escape", settle=0.2)
        r.check(row, "Escape closes the palette", until(lambda: not entry(f"{row}-closed")),
                "a text field still has the focus")
    return True


if __name__ == "__main__":
    ac.main(scenarios)
