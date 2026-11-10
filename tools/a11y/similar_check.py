#!/usr/bin/env python3
"""The similar-shots screens (Duplicates / Similar shots) over AT-SPI, Linux, automated.

Opens a folder with byte-identical copies in the gallery, opens the Duplicates screen from the
palette and checks what a screen reader reads there through libatspi: the tabs, the scan
progress, the sensitivity slider (its value and range, set through the Value interface), the
header and footer buttons (names, key shortcuts, Action interface) and, in the review, the
members' descriptions (zoom, channel view), a protected member's unavailable Discard and an
unavailable "100 %" on a photo smaller than its pane, each with its reason. Footer buttons
are pressed through AT-SPI's Action interface (as Orca's "activate" does) and their effect
checked in the next tree; the controls' bounds are saved for the right-to-left run.

--rtl runs the app in Hebrew on the same folder and checks that the controls' bounds mirror
the left-to-right run's (its bounds.json one folder up) and that sequences read right to left.

--progress opens the Duplicates screen while a scan of generated images (under the output
folder, never the corpus) is still running, and checks the progress bar and its polite
announcements at the quarters.

Run it through `tools/a11y/atspi-check.sh --similar`; like atspi_check.py it refuses to run on
any bus but the private one, and every process it starts is stopped on exit.
"""

import json
import os
import re
import shutil
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import atspi_check as ac  # noqa: E402
from atspi_check import Atspi  # noqa: E402

BUTTONS = ac.BUTTONS
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))


def ftl(lang, key):
    """A simple one-line message of the app's translations (no arguments, no selectors)."""
    path = os.path.join(ROOT, "crates/specular-i18n/i18n", lang, "similar.ftl")
    with open(path, encoding="utf-8") as f:
        for line in f:
            if line.startswith(key + " = "):
                return line[len(key) + 3:].strip()
    return None


def ftl_pattern(lang, key):
    """A regex for a one-line message whose arguments are numbers, one group per argument."""
    parts = re.split(r"\{ \$\w+ \}", ftl(lang, key))
    return "(?:" + r"(\d+)".join(re.escape(x) for x in parts) + ")"


def num(m, i):
    """The `i`-th number of a match of the alternatives (whichever matched)."""
    got = [g for g in m.groups() if g is not None]
    return int(got[i])


def folder_with_dupes(a):
    """The committed test images (no broken file), a large committed JPEG and its
    byte-identical copy (one duplicate group, large enough to zoom to 100 %), and a copy of a
    7 × 5 test image rated through an XMP sidecar (a second group: smaller than any pane, with
    a protected member)."""
    folder = os.path.join(a.out, "similar-images")
    shutil.rmtree(folder, ignore_errors=True)
    os.makedirs(folder)
    for f in sorted(os.listdir(a.images)):
        if not f.startswith("zz-"):
            shutil.copy(os.path.join(a.images, f), folder)
    big = os.path.join(ROOT, "crates/specular-decode/data/jpeg-calibration.jpg")
    shutil.copy(big, os.path.join(folder, "calibration.jpg"))
    shutil.copy(big, os.path.join(folder, "calibration copy.jpg"))
    shutil.copy(os.path.join(folder, "beach.jpg"), os.path.join(folder, "beach copy.jpg"))
    with open(os.path.join(folder, "beach copy.xmp"), "w") as f:
        f.write('<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF '
                'xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description '
                'rdf:about="" xmlns:xmp="http://ns.adobe.com/xap/1.0/" xmp:Rating="3"/>'
                '</rdf:RDF></x:xmpmeta>\n')
    return folder


def helpers(r, app, trees):
    def tree(label, settle=0.3):
        r.pump(settle)
        app.clear_cache()
        t = ac.snapshot(app)
        with open(os.path.join(trees, f"{label}.txt"), "w") as f:
            for n in t.walk():
                f.write(n.line() + "\n")
        return t

    return tree


def focused(t):
    f = t.find(pred=lambda n: "focused" in n.states)
    return f[-1] if f else None


def screen(t):
    """The similar-shots dialog (the one with the tab list)."""
    d = [d for d in t.find(role="dialog") if d.find(role="page tab list")]
    return d[0] if d else None


def button(d, prefix):
    b = [n for n in d.walk() if n.role in BUTTONS and n.name.startswith(prefix)]
    return b[0] if b else None


def press(n):
    ai = n.acc.get_action_iface()
    return ai is not None and Atspi.Action.do_action(ai, 0)


def lines(ns):
    return [n.line().strip() for n in ns]


def on(n):
    return n is not None and ("checked" in n.states or "pressed" in n.states)


def ext(n):
    """Bounds in window coordinates: [x, y, w, h]."""
    e = n.acc.get_extents(Atspi.CoordType.WINDOW)
    return [e.x, e.y, e.width, e.height]


def announced(evs):
    return [e for e in evs if e["type"] == "object:announcement"]


def layout(d):
    """The bounds the right-to-left run compares: the window's width, the tabs, Done (the
    last header button), the group rows or members, and the footer's buttons (in reading
    order)."""
    w = ext(d)
    out = {"W": w[2], "H": w[3]}
    for i, n in enumerate(d.find(role="page tab")):
        out[f"tab{i}"] = ext(n)
    btns = [n for n in d.walk() if n.role in BUTTONS and n.role != "page tab"]
    head = [n for n in btns if ext(n)[1] < w[3] * 0.2]
    if head:
        out["done"] = ext(head[-1])
    foot = [n for n in btns if ext(n)[1] > w[3] * 0.6]
    for i, n in enumerate(foot):
        out[f"foot{i}"] = ext(n)
    for i, n in enumerate(d.find(role="list item")):
        out[f"item{i}"] = ext(n)
    return out


# --- left to right -------------------------------------------------------------------------------


def scenarios(r, a, trees):
    if a.rtl:
        return rtl(r, a, trees)
    if a.progress:
        return progress(r, a, trees)
    folder = folder_with_dupes(a)
    started = ac.sway_app(r, a, [os.path.join(folder, "beach.jpg")])
    if not started:
        return False
    app, keys, _ = started
    tree = helpers(r, app, trees)
    bounds = {}

    # --- Open the Duplicates screen from the gallery's palette ---
    row = "similar-open"
    keys("g")
    r.pump(2.0)
    keys("-M", "ctrl", "k", "-m", "ctrl")
    r.pump(0.8)
    keys("Duplicates")
    r.pump(0.8)
    keys("-k", "Return")
    d = None
    for i in range(40):
        t = tree("s01-dupes", 0.5)
        d = screen(t)
        if d and len(d.find(role="list item")) >= 2:
            break
    r.check(row, "a named dialog with a tab list", d and d.name, d.line() if d else None)
    if not d:
        return False
    tabs = d.find(role="page tab")
    r.check(row, "two tabs, Duplicates selected", len(tabs) == 2 and any(
        "selected" in x.states and x.name.startswith("Duplicates") for x in tabs), lines(tabs))
    r.check(row, "the tabs give their key (Tab)", all("(Tab)" in x.desc for x in tabs),
            lines(tabs))
    marked = [n.name for n in d.walk() if any(c in n.name + n.desc for c in "⁦⁧⁨⁩")]
    r.check(row, "names carry no bidi isolation marks", not marked, marked[:3])
    rows = d.find(role="list item")
    r.check(row, "the groups are list items; the first's description names the best pick and "
            "the rest", len(rows) == 2 and "photos" in rows[0].name
            and "calibration" in rows[0].desc and "Discard identical file" in rows[0].desc,
            lines(rows))
    f = focused(t)
    r.check(row, "the group row has the focus", f and f.role == "list item",
            f.line().strip() if f else None)

    # --- The header: summary, progress, slider ---
    row = "similar-header"
    labels = [n for n in d.walk() if n.role == "label" and n.name]
    r.check(row, "the summary is a label", labels, lines(labels))
    bars = d.find(role="progress bar")
    # The scan of a few small images is long done: the progress node only shows while
    # scanning (--progress checks it during a scan).
    r.check(row, "no progress bar once scanned", not bars, lines(bars))
    r.check(row, "Duplicates have no sensitivity", not d.find(role="slider"))
    sim = [x for x in tabs if x.name.startswith("Similar")]
    r.check(row, "pressing the Similar shots tab through AT-SPI", sim and press(sim[0]))
    t = tree("s02-similar-tab", 1.5)
    d = screen(t)
    tabs = d.find(role="page tab") if d else []
    r.check(row, "selects it", any("selected" in x.states and x.name.startswith("Similar")
                                   for x in tabs), lines(tabs))
    sliders = d.find(role="slider") if d else []
    r.check(row, "the sensitivity is a named slider", sliders and sliders[0].name == "Sensitivity",
            lines(sliders))
    if sliders:
        s = sliders[0]
        vi = s.acc.get_value_iface()
        lo, hi = Atspi.Value.get_minimum_value(vi), Atspi.Value.get_maximum_value(vi)
        r.check(row, "its range is 0 to 100", (lo, hi) == (0, 100), (lo, hi))
        r.check(row, "its value is in the range", s.value is not None and lo <= s.value <= hi,
                s.value)
        r.check(row, "it describes its ends", "Strict" in s.desc and "Loose" in s.desc, s.desc)
        want = 40 if s.value != 40 else 60
        Atspi.Value.set_current_value(vi, want)
        t = tree("s02-slider-set", 1.0)
        d = screen(t)
        s2 = d.find(role="slider") if d else []
        r.check(row, "setting the value through AT-SPI moves it", s2 and s2[0].value == want,
                lines(s2))
        r.pump(1.0)
    dup = [x for x in tabs if x.name.startswith("Duplicates")]
    if dup:
        press(dup[0])

    # --- The list's buttons ---
    row = "similar-list-buttons"
    t = tree("s03-list", 1.5)
    d = screen(t)
    bounds["list"] = layout(d)
    btns = [n for n in d.walk() if n.role in BUTTONS]
    r.check(row, "Done (no Back in the list)", button(d, "Done") and not button(d, "Back"),
            lines(btns))
    acc = button(d, "Accept all")
    r.check(row, "Accept all is a button with its key (A)", acc and "(A)" in acc.desc,
            acc.line().strip() if acc else lines(btns))
    disc = button(d, "Discard:")
    r.check(row, "the discard action button with its key (D)", disc and "(D)" in disc.desc,
            disc.line().strip() if disc else None)
    noact = [b.name for b in btns if not b.actions]
    r.check(row, "every button can be pressed", not noact, noact)
    if acc:
        r.check(row, "pressing Accept all through AT-SPI", press(acc))
        t = tree("s04-accept-all", 0.8)
        conf = [n for n in t.walk() if "Accept the suggestions" in n.name + n.desc]
        r.check(row, "opens the confirmation", conf, lines(conf))
        keys("-k", "Escape")
        r.pump(0.8)

    # --- The review and its footer ---
    row = "similar-review"
    keys("-k", "Return")
    t = tree("s05-review", 1.5)
    d = screen(t)
    if not r.check(row, "Enter opens the review in the dialog", d and button(d, "Keep"),
                   d.line() if d else None):
        return True
    bounds["review"] = layout(d)
    btns = [n for n in d.walk() if n.role in BUTTONS]
    want = {"Keep": "P", "Discard": "X", "Undecided": "U", "Accept and next": "Space",
            "Not similar": "N", "Skip": "S", "Compare": "C", "100 %": "Z"}
    got = {b.name: b.desc for b in btns}
    miss = {k: v for k, v in want.items() if f"({v})" not in got.get(k, "")}
    r.check(row, "the footer buttons with their keys", not miss, got)
    members = d.find(role="list item")
    r.check(row, "two members, described with their size", len(members) == 2 and all(
        " × " in m.desc for m in members), lines(members))
    cmp = button(d, "Compare")
    zoom = button(d, "100 %")
    if cmp and zoom:
        was = on(cmp)
        r.check(row, "Compare and 100 % are toggle buttons", cmp.role == "toggle button"
                and zoom.role == "toggle button", lines([cmp, zoom]))
        r.check(row, "100 % is available on a photo larger than its pane",
                "sensitive" in zoom.states and zoom.actions, zoom.line().strip())
        r.check(row, "pressing Compare through AT-SPI", press(cmp))
        t = tree("s06-compare", 1.0)
        d = screen(t)
        cmp2 = button(d, "Compare")
        r.check(row, "toggles it", cmp2 and on(cmp2) != was,
                cmp2.line().strip() if cmp2 else None)
        if not on(cmp2):
            press(cmp2)
            t = tree("s06-compare-on", 1.0)
            d = screen(t)
        r.check(row, "pressing 100 % through AT-SPI", press(button(d, "100 %")))
        t = tree("s07-zoom", 1.2)
        d = screen(t)
        z = button(d, "100 %")
        r.check(row, "turns the zoom on", on(z), z.line().strip() if z else None)
        members = d.find(role="list item")
        r.check(row, "members in the compare describe their zoom", members and all(
            "%" in m.desc for m in members), lines(members))
        # A channel view (Shift+1, red): in every large member's description.
        red = "Red channel"
        keys("-M", "shift", "1", "-m", "shift")
        for _ in range(10):
            t = tree("s07-red", 0.5)
            d = screen(t)
            members = d.find(role="list item")
            if members and all(red in m.desc for m in members):
                break
        r.check("similar-channel", "Shift+1: members describe the red channel view",
                members and all(red in m.desc for m in members), lines(members))
        keys("-M", "shift", "1", "-m", "shift")
        t = tree("s07-normal", 1.0)
        d = screen(t)
        members = d.find(role="list item")
        r.check("similar-channel", "Shift+1 again: back to normal, not described",
                members and not any("channel" in m.desc for m in members), lines(members))
    keep = button(d, "Keep")
    f = focused(t)
    before = f.name.split(":")[0] if f else ""
    r.check(row, "pressing Keep through AT-SPI", keep and press(keep))
    t = tree("s08-keep", 0.8)
    kept = [m for m in screen(t).find(role="list item") if m.name.startswith(before + ":")]
    r.check(row, "marks the focused member kept", kept and ": Keep," in kept[0].name,
            lines(kept))
    nxt = button(screen(t), "Accept and next")
    r.check(row, "pressing Accept and next through AT-SPI", nxt and press(nxt))
    t = tree("s09-next", 1.0)
    d = screen(t)
    members = d.find(role="list item") if d else []
    r.check(row, "goes on to the next group (the tiny beach photo and its rated copy)",
            d and button(d, "Keep") and any(m.name.startswith("beach") for m in members),
            lines(members))
    if not d or not button(d, "Keep"):
        save_bounds(a, bounds)
        return True

    # --- "100 %" on a photo smaller than its pane: unavailable, says why ---
    row = "similar-zoom-small"
    why = ftl("en", "similar-zoom-already")
    z = button(d, "100 %")
    r.check(row, "100 % is unavailable (not sensitive, no action)",
            z and "sensitive" not in z.states and not z.actions,
            z.line().strip() if z else None)
    r.check(row, "its description gives the reason", z and z.desc == why,
            z.desc if z else None)
    r.take_events()
    keys("z")
    t = tree("s10-zoom-small", 1.0)
    d = screen(t)
    z = button(d, "100 %")
    said = [e["data"] for e in announced(r.take_events())]
    r.check(row, "Z announces the reason (toast)", why in said, said)
    r.check(row, "and the zoom stays off", z and not on(z), z.line().strip() if z else None)

    # --- A protected (rated) member: Discard unavailable, says why ---
    row = "similar-protected"
    why = ftl("en", "similar-toast-protected")
    disc = None
    for i in range(3):
        t = tree(f"s11-protected-{i}", 0.6)
        d = screen(t)
        f = focused(t)
        disc = button(d, "Discard")
        if f and "Rated" in f.name:
            break
        keys("-k", "Right")
    r.check(row, "the rated copy has the focus, named as protected", f and "Rated" in f.name,
            f.line().strip() if f else None)
    r.check(row, "Discard is unavailable (not sensitive, no action)",
            disc and "sensitive" not in disc.states and not disc.actions,
            disc.line().strip() if disc else None)
    r.check(row, "its description gives the reason", disc and disc.desc == why,
            disc.desc if disc else None)
    keep = button(d, "Keep")
    r.check(row, "Keep stays available", keep and "sensitive" in keep.states and keep.actions,
            keep.line().strip() if keep else None)
    keys("-k", "Left")
    t = tree("s12-unprotected", 0.6)
    d = screen(t)
    disc = button(d, "Discard")
    r.check(row, "the other member's Discard is available again",
            disc and "sensitive" in disc.states and disc.actions,
            disc.line().strip() if disc else None)
    save_bounds(a, bounds)
    return True


def save_bounds(a, bounds):
    with open(os.path.join(a.out, "bounds.json"), "w") as f:
        json.dump(bounds, f, indent=1)


# --- right to left -------------------------------------------------------------------------------


def rtl(r, a, trees):
    """Hebrew: the controls' bounds against the left-to-right run's (same folder, window and
    screens), and sequences in reading order laid out right to left."""
    ltr_path = os.path.join(os.path.dirname(a.out), "bounds.json")
    try:
        with open(ltr_path) as f:
            ltr = json.load(f)
    except OSError:
        ltr = None
    if not r.check("setup", "the left-to-right run's bounds", ltr, ltr_path):
        return False
    folder = folder_with_dupes(a)
    started = ac.sway_app(r, a, ["--set", "general.language=he",
                                 os.path.join(folder, "beach.jpg")])
    if not started:
        return False
    app, keys, _ = started
    tree = helpers(r, app, trees)
    # As in the left-to-right run, through the palette, by the screen's Hebrew name.
    keys("g")
    r.pump(2.0)
    keys("-M", "ctrl", "k", "-m", "ctrl")
    r.pump(0.8)
    keys(ftl("he", "similar-duplicates"))
    r.pump(0.8)
    keys("-k", "Return")
    d = None
    for i in range(40):
        t = tree("h01-dupes", 0.5)
        d = screen(t)
        if d and len(d.find(role="list item")) >= 2:
            break
    row = "similar-rtl-list"
    done = ftl("he", "similar-done")
    r.check(row, "the dialog in Hebrew", d and button(d, done), d.line() if d else None)
    if not d:
        return False
    compare(r, row, ltr["list"], layout(d), ["tab0", "item0", "item1"], ["done"])
    tabs = d.find(role="page tab")
    xs = [ext(x)[0] for x in tabs]
    r.check(row, "the tabs read right to left", len(xs) == 2 and xs[0] > xs[1], xs)

    row = "similar-rtl-review"
    keys("-k", "Return")
    for i in range(10):
        t = tree("h02-review", 0.6)
        d = screen(t)
        if d and button(d, ftl("he", "similar-keep")):
            break
    if not r.check(row, "Enter opens the review", d and button(d, ftl("he", "similar-keep")),
                   d.line() if d else None):
        return True
    got = layout(d)
    compare(r, row, ltr["review"], got, ["foot0", "tab0"], ["done"])
    foot = [got[k] for k in sorted((k for k in got if k.startswith("foot")),
                                   key=lambda k: int(k[4:]))]
    rows = {}
    for b in foot:
        rows.setdefault(b[1], []).append(b[0])
    ok = all(all(x[i] > x[i + 1] for i in range(len(x) - 1)) for x in rows.values())
    r.check(row, "the footer's buttons read right to left on each row", foot and ok, rows)
    members = d.find(role="list item")
    xs = [ext(m)[0] for m in members]
    r.check(row, "the members read right to left", len(xs) == 2 and xs[0] > xs[1], xs)
    marked = [n.name for n in d.walk() if any(c in n.name + n.desc for c in "⁦⁧⁨⁩")]
    r.check(row, "names carry no bidi isolation marks", not marked, marked[:3])
    return True


def compare(r, row, ltr, rtl_, start, end):
    """`start`: controls laid out from the reading start (their start edge mirrors: left in
    LTR, right in RTL); `end`: from the reading end. Items as wide in both runs (rows,
    members) must mirror whole. Within 2 px."""
    w = ltr["W"]
    r.check(row, "the same window width", rtl_["W"] == w, (w, rtl_["W"]))
    for k in start + end:
        a, b = ltr.get(k), rtl_.get(k)
        if not a or not b:
            r.check(row, f"{k} in both runs", False, (a, b))
            continue
        if k in start:
            ok = abs(a[0] - (w - b[0] - b[2])) <= 2
        else:
            ok = abs((w - a[0] - a[2]) - b[0]) <= 2
        if abs(a[2] - b[2]) <= 2:
            ok = ok and abs(a[0] - (w - b[0] - b[2])) <= 2
        r.check(row, f"{k} mirrors (LTR {a}, RTL {b}, window {w})", ok and abs(a[1] - b[1]) <= 2)


# --- scan progress -------------------------------------------------------------------------------


def generate(folder, n):
    """`n` distinct generated JPEGs (noise over a gradient, 640 × 480): no two alike."""
    from PIL import Image  # noqa: PLC0415

    os.makedirs(folder, exist_ok=True)
    have = len([f for f in os.listdir(folder) if f.endswith(".jpg")])
    if have == n:
        return
    for f in os.listdir(folder):
        os.remove(os.path.join(folder, f))
    for i in range(n):
        im = Image.effect_noise((640, 480), 40 + i % 50).convert("RGB")
        g = Image.linear_gradient("L").resize((640, 480)).rotate(i * 7 % 360)
        im = Image.merge("RGB", (im.getchannel(0), g, Image.effect_noise((640, 480), 30)
                                 .convert("L")))
        im.save(os.path.join(folder, f"gen-{i:04d}.jpg"), quality=90)


def progress(r, a, trees):
    """The Duplicates screen opened while a scan runs: a progress bar with its value, and
    polite announcements at the scan's quarters only."""
    row = "similar-progress"
    n = int(os.environ.get("SPECULAR_SCAN_IMAGES", "600"))
    folder = os.path.join(a.out, "scan-images")
    try:
        generate(folder, n)
    except ImportError:
        r.check("setup", "Pillow to generate the images", False)
        return False
    started = ac.sway_app(r, a, [os.path.join(folder, "gen-0000.jpg")])
    if not started:
        return False
    app, keys, _ = started
    tree = helpers(r, app, trees)
    keys("g")
    r.pump(0.5)
    keys("-M", "ctrl", "k", "-m", "ctrl")
    r.pump(0.5)
    keys("Duplicates")
    r.pump(0.3)
    r.take_events()
    keys("-k", "Return")
    seen, said, start = [], [], time.time()
    # Either phase: finding the groups, then measuring the similar members' sharpness (its
    # own total). The name moves at the quarters only; the value counts every image.
    pat = re.compile("|".join(ftl_pattern("en", k) for k in
                              ("similar-scanning", "similar-checking-sharpness")))
    end = time.time() + a.timeout * 3
    k = 0
    while time.time() < end:
        t = tree(f"p{k:03d}", 0.4)
        k += 1
        said += announced(r.take_events())
        d = screen(t)
        bars = d.find(role="progress bar") if d else []
        if bars:
            seen.append(bars[0])
        elif d and seen:
            break
    took = time.time() - start
    r.check(row, f"a progress bar while scanning ({len(seen)} trees in {took:.1f} s)", seen,
            lines(seen[:1]))
    if seen:
        b = seen[-1]
        m = pat.search(b.name)
        r.check(row, "named with the scan's progress", m, b.line().strip())
        r.check(row, "its value counts the images", b.value is not None and m
                and 0 <= b.value <= num(m, 1), (b.value, b.name))
        r.check(row, "it is a polite live region", "live" not in b.attrs
                or b.attrs.get("live") == "polite", b.attrs)
    texts = [e["data"] for e in said]
    scans = [pat.search(x) for x in texts]
    scans = [(num(x, 0), num(x, 1)) for x in scans if x]
    r.check(row, "the progress is announced", scans, texts[:12])
    quarters = all(dn % max(1, -(-tot // 4)) == 0 or dn == tot for dn, tot in scans)
    r.check(row, "only at the quarters", scans and quarters, scans)
    r.check(row, "a handful of times, not at every image", 0 < len(scans) <= 10, len(scans))
    polite = [e["detail1"] for e in said if pat.search(e["data"])]
    r.check(row, "politely (not assertive)", polite and all(p != 2 for p in polite), polite)
    return True


if __name__ == "__main__":
    ac.main(scenarios, extra_args=("--rtl", "--progress"))
