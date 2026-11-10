#!/usr/bin/env python3
"""A DICOM series in the viewer, MPR in a small window and the Studies browser, over AT-SPI
and the remote control, Linux, automated.

Writes a small CT series (64 × 64, five slices 2.5 mm apart, Explicit VR Little Endian) and a
JPEG beside it under the output folder, opens the first slice in the Medical profile and
checks:
- series mode: the viewer lists the five slices, not the folder's six files;
- a relist in series mode (after a sixth slice is copied in, the sort order changes in the
  settings file): the series is read again (six slices), not ignored;
- MPR (Shift+R, sent as the `mpr` action): its three views, the 3D view and the controls are
  named, and in a full and a small window (480 × 360) no control lies outside the window or
  over the 3D view, which keeps a usable height;
- "Show studies on this disc" (the `studies` action), with a second series added, opens the
  Studies browser on the folder (its DICOMDIR search runs off the UI thread).

Run it through `tools/a11y/atspi-check.sh --series`; like atspi_check.py it refuses to run on
any bus but the private one, and every process it starts is stopped on exit.
"""

import json
import os
import shutil
import struct
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import atspi_check as ac  # noqa: E402
from atspi_check import Atspi  # noqa: E402

BUTTONS = ac.BUTTONS
CT = "1.2.840.10008.5.1.4.1.1.2"
EXPLICIT_LE = "1.2.840.10008.1.2.1"
LONG = ("OB", "OW", "OF", "SQ", "UN", "UT")


def element(group, elem, vr, value):
    if isinstance(value, str):
        value = value.encode("ascii")
        if len(value) % 2:
            value += b"\0" if vr == "UI" else b" "
    elif len(value) % 2:
        value += b"\0"
    tag = struct.pack("<HH", group, elem) + vr.encode("ascii")
    if vr in LONG:
        return tag + b"\0\0" + struct.pack("<I", len(value)) + value
    return tag + struct.pack("<H", len(value)) + value


def write_slice(path, k, size=64, spacing=2.5, number=1):
    """Slice `k` of series `number` of a synthetic CT: a sphere of 400 HU in air, signed
    16-bit."""
    study = "1.2.826.0.1.3680043.9.7.1"
    series = f"{study}.{number + 1}"
    sop = f"{series}.{k + 1}"
    z = k * spacing
    px = bytearray()
    c = (size - 1) / 2
    for j in range(size):
        for i in range(size):
            r2 = (i - c) ** 2 + (j - c) ** 2 + ((z - 5.0) / 1.0) ** 2
            px += struct.pack("<h", 400 if r2 < 20 ** 2 else -1000)
    meta_rest = b"".join([
        element(0x0002, 0x0001, "OB", b"\0\1"),
        element(0x0002, 0x0002, "UI", CT),
        element(0x0002, 0x0003, "UI", sop),
        element(0x0002, 0x0010, "UI", EXPLICIT_LE),
        element(0x0002, 0x0012, "UI", "1.2.826.0.1.3680043.9.7.99"),
    ])
    meta = element(0x0002, 0x0000, "UL", struct.pack("<I", len(meta_rest))) + meta_rest
    data = b"".join([
        element(0x0008, 0x0016, "UI", CT),
        element(0x0008, 0x0018, "UI", sop),
        element(0x0008, 0x0060, "CS", "CT"),
        element(0x0008, 0x103E, "LO", "Phantom"),
        element(0x0010, 0x0010, "PN", "Test^Phantom"),
        element(0x0020, 0x000D, "UI", study),
        element(0x0020, 0x000E, "UI", series),
        element(0x0020, 0x0011, "IS", str(number)),
        element(0x0020, 0x0013, "IS", str(k + 1)),
        element(0x0020, 0x0032, "DS", f"0\\0\\{z}"),
        element(0x0020, 0x0037, "DS", "1\\0\\0\\0\\1\\0"),
        element(0x0028, 0x0002, "US", struct.pack("<H", 1)),
        element(0x0028, 0x0004, "CS", "MONOCHROME2"),
        element(0x0028, 0x0010, "US", struct.pack("<H", size)),
        element(0x0028, 0x0011, "US", struct.pack("<H", size)),
        element(0x0028, 0x0030, "DS", "1\\1"),
        element(0x0028, 0x0100, "US", struct.pack("<H", 16)),
        element(0x0028, 0x0101, "US", struct.pack("<H", 16)),
        element(0x0028, 0x0102, "US", struct.pack("<H", 15)),
        element(0x0028, 0x0103, "US", struct.pack("<H", 1)),
        element(0x0028, 0x1050, "DS", "40"),
        element(0x0028, 0x1051, "DS", "400"),
        element(0x0028, 0x1052, "DS", "0"),
        element(0x0028, 0x1053, "DS", "1"),
        element(0x7FE0, 0x0010, "OW", bytes(px)),
    ])
    with open(path, "wb") as f:
        f.write(b"\0" * 128 + b"DICM" + meta + data)


def ext(n):
    e = n.acc.get_extents(Atspi.CoordType.WINDOW)
    return [e.x, e.y, e.width, e.height]


def overlap(a, b):
    return a[0] < b[0] + b[2] and b[0] < a[0] + a[2] and a[1] < b[1] + b[3] and b[1] < a[1] + a[3]


def scenarios(r, a, trees):
    folder = os.path.join(a.out, "series", "CT1")
    shutil.rmtree(os.path.dirname(folder), ignore_errors=True)
    os.makedirs(folder)
    for k in range(5):
        write_slice(os.path.join(folder, f"IM{k:04}.dcm"), k)
    spare = os.path.join(a.out, "series", "IM0005.dcm")
    write_slice(spare, 5)
    shutil.copy(os.path.join(a.images, "beach.jpg"), os.path.join(folder, "beach.jpg"))

    started = ac.sway_app(r, a, ["--log", "folder", "--set", "profile.shown=question",
                                 "--set", "profile.capabilities=general,medical",
                                 os.path.join(folder, "IM0000.dcm")])
    if not started:
        return False
    app, keys, sway = started

    def tree(label, settle=0.3):
        r.pump(settle)
        app.clear_cache()
        t = ac.snapshot(app)
        with open(os.path.join(trees, f"{label}.txt"), "w") as f:
            for n in t.walk():
                f.write(n.line() + "\n")
        found = ac.audit(t)
        r.check("audit", f"{label}: named controls with their state", not found, found[:6])
        return t

    def remote(*args):
        res = subprocess.run([a.bin, "--remote", *args], capture_output=True, text=True,
                             timeout=20)
        return res.stdout.strip()

    def state():
        try:
            return json.loads(remote("state"))
        except (ValueError, subprocess.TimeoutExpired):
            return {}

    def wait_count(n, secs):
        end = time.time() + secs
        s = {}
        while time.time() < end:
            s = state()
            if s.get("count") == n:
                return s
            r.pump(0.3)
        return s

    # --- Series mode ---
    row = "series"
    s = wait_count(5, 30)
    r.check(row, "series mode lists the five slices (not the folder's six files)",
            s.get("count") == 5, s)
    tree("01-series")

    # --- A relist in series mode ---
    row = "relist"
    # Copied in from outside, then a relist (the sort order changed in settings.toml, which
    # the app watches): the viewer has no watch of its own on the folder. A reloaded file
    # replaces the --set values, so the profile goes into the file too.
    shutil.copy(spare, os.path.join(folder, "IM0005.dcm"))
    conf = os.path.join(os.environ["XDG_CONFIG_HOME"], "specular", "settings.toml")
    with open(conf, "a") as f:
        f.write('\n[profile]\ncapabilities = ["general", "medical"]\nshown = "question"\n'
                '\n[browsing]\nsort_order = "size"\n')
    t0 = time.time()
    s = wait_count(6, 60)
    with open(os.path.join(a.out, "relist-seconds.txt"), "w") as f:
        f.write(f"{time.time() - t0:.1f}\n")
    r.check(row, "a slice copied in joins the series", s.get("count") == 6, s)
    r.check(row, "the image shown stays shown", (s.get("file") or "").endswith("IM0000.dcm"),
            s.get("file"))

    # --- MPR ---
    row = "mpr"
    said = subprocess.run([a.bin, "--remote", "action", "mpr"], capture_output=True,
                          text=True, timeout=20)
    r.check(row, "MPR opens", said.returncode == 0, (said.stdout + said.stderr).strip())
    r.pump(3.0)

    def check_slot(label):
        t = tree(label, 0.8)
        frame = t.find(role="frame")
        win = ext(frame[0]) if frame else [0, 0, 0, 0]
        views = [n for n in t.find(role="image")
                 if n.name.split(" ")[0] in ("Axial", "Coronal", "Sagittal", "Oblique")]
        r.check(row, f"{label}: three named views", len(views) == 3, [v.name for v in views])
        v3 = [n for n in t.find(role="image") if n.name.startswith("3D view")]
        r.check(row, f"{label}: the 3D view is named", v3, [n.name for n in t.find(role="image")])
        group = t.find(role="panel", name="MPR controls") + t.find(name="MPR controls")
        btns = [n for g in group[:1] for n in g.walk() if n.role in BUTTONS]
        names = [b.name for b in btns]
        r.check(row, f"{label}: MPR's buttons", "Close MPR" in names and "Thin" in names, names)
        outside = [b.name for b in btns
                   if not (0 <= ext(b)[0] and ext(b)[0] + ext(b)[2] <= win[2]
                           and 0 <= ext(b)[1] and ext(b)[1] + ext(b)[3] <= win[3])]
        r.check(row, f"{label}: every button inside the window", not outside, outside)
        if v3:
            box = ext(v3[0])
            over = [b.name for b in btns if overlap(ext(b), box)]
            r.check(row, f"{label}: no button over the 3D view", not over, (box, over))
            r.check(row, f"{label}: the 3D view keeps a usable height", box[3] >= 40, box)
        return t

    check_slot("02-mpr")
    sway("[app_id=.*] floating enable, resize set 480 360")
    r.pump(2.0)
    check_slot("03-mpr-small")
    sway("[app_id=.*] floating disable")
    remote("action", "mpr")  # closes it again
    r.pump(1.0)

    # --- Studies ---
    # A second series in the folder: the browser offers both (a single one opens at once).
    row = "studies"
    for k in range(3):
        write_slice(os.path.join(folder, f"SE2-{k}.dcm"), k, number=2)
    before = state()
    said = subprocess.run([a.bin, "--remote", "action", "studies"], capture_output=True,
                          text=True, timeout=20)
    with open(os.path.join(a.out, "studies-reply.txt"), "w") as f:
        f.write(said.stdout + said.stderr)
    r.pump(3.0)
    t = tree("04-studies", 0.5)
    titled = [n.name for n in t.walk() if n.name.startswith("Studies")]
    r.check(row, "the Studies browser opens on the folder", titled,
            (said.stdout + said.stderr).strip())
    after = state()
    r.check(row, "the viewer keeps answering while it reads", after.get("count"), (before, after))
    return True


if __name__ == "__main__":
    ac.main(scenarios)
