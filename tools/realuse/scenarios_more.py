"""Scenarios 2, 3, 6, 7, 10, 11 and 12 of tools/realuse/run.py (see scenarios.py)."""

import glob
import json
import os
import re
import subprocess
import time

import gen_library as gl
from scenarios import big_jpeg, ftl, library_dir, small_jpeg

# --- 2: cull ------------------------------------------------------------------------------------


def write_pair(folder, i):
    stem = f"IMG_{i:04}"
    with open(os.path.join(folder, stem + ".JPG"), "wb") as f:
        f.write(small_jpeg(i))
    with open(os.path.join(folder, stem + ".DNG"), "wb") as f:
        f.write(gl.dng(i, gl.facts(i)))
    return stem


def xmp_of(folder, stem):
    """The XMP sidecar text beside `stem` (None: no sidecar)."""
    for n in sorted(os.listdir(folder)):
        if n.startswith(stem + ".") and n.lower().endswith(".xmp") \
                and not n.endswith(".specular.xmp"):
            with open(os.path.join(folder, n), errors="replace") as f:
                return f.read()
    return None


def s2_cull(s, sc, a):
    folder = os.path.join(sc.dir, "shoot")
    slot1 = os.path.join(sc.dir, "slot1")
    slot2 = os.path.join(sc.dir, "slot2")
    for d in (folder, slot1, slot2):
        os.makedirs(d)
    stems = [write_pair(folder, i) for i in range(300)]
    # The step-to-sharp bench on the same folder (it exits by itself).
    bench = sc.app("bench", [os.path.join(folder, stems[0] + ".JPG")],
                   env={"SPECULAR_BENCH_CULL": "40,150"}, gate=False)
    t = time.time()
    while bench.proc.poll() is None and time.time() - t < 600:
        s.r.pump(1.0)
    line = None
    try:
        with open(bench.stderr, errors="replace") as f:
            line = next((ln for ln in f if ln.startswith('{"cull_bench"')), None)
    except OSError:
        pass
    if sc.check("cull bench prints its timings", line is not None):
        b = json.loads(line)["cull_bench"]
        sc.info("cull_bench", b)
        sc.time("cull_step_sharp_p50_ms", b["sharp_ms"]["p50"])
        sc.time("cull_step_sharp_p95_ms", b["sharp_ms"]["p95"])
    bench.quit()

    app = sc.app("viewer", ["--set", f"cull.slot_1={slot1}", "--set", f"cull.slot_2={slot2}",
                            os.path.join(folder, stems[0] + ".JPG")])
    if not sc.check("remote control answers", app.connect()):
        return
    app.idle(timeout_ms=60000)
    st = app.ui()
    if not st["cull"]:
        _, e = app.action("cull")
        sc.check("cull mode opens", e is None, str(e))
    st = app.wait_for("cull", lambda st: st["cull"], timeout=30)
    sc.check("cull runs on the folder", st is not None and st["cull"] == folder,
             st and st["cull"])
    app.idle(timeout_ms=30000)
    app.focus()
    st = app.call("state")
    sc.info("cull_state", st)
    sc.check("RAW+JPEG pairs are one item each", st["count"] == 300, st["count"])
    # Item 0: pick; 1: reject; 2: three stars; 3: pick then unflag; 4: pick, undone. A mark
    # moves on to the next item by itself, so each item is gone to first.
    def at(i, *keys):
        app.call("navigate", index=i)
        app.idle(quiet_ms=200, timeout_ms=20000)
        for k in keys:
            s.keys(*k)

    at(0, ["p"])
    after = app.call("state")["index"]
    sc.info("index_after_pick", after)
    at(1, ["x"])
    at(2, ["3"])
    at(3, ["p"])
    at(3, ["u"])
    at(4, ["p"], ["-M", "ctrl", "z", "-m", "ctrl"])
    app.idle(quiet_ms=500, timeout_ms=20000)
    marked = app.call("marked")["files"]
    sc.info("marked", marked)
    names = {os.path.basename(p) for p in marked}
    sc.check("the pick shows in `marked`; unflagged and undone ones do not",
             names == {stems[0] + ".JPG"} or names == {stems[0] + ".JPG", stems[0] + ".DNG"},
             sorted(names))
    sc.info("pair_halves_in_marked", sorted(names))
    sidecars = sorted(n for n in os.listdir(folder) if n.lower().endswith(".xmp"))
    sc.info("sidecars", sidecars)
    x0, x2 = xmp_of(folder, stems[0]), xmp_of(folder, stems[2])
    sc.check("the pick is in an XMP sidecar", x0 is not None,
             [n for n in sorted(os.listdir(folder)) if n.lower().endswith(".xmp")][:6])
    sc.check("three stars are in the XMP sidecar",
             x2 is not None and re.search(r'Rating(="|>)3', x2) is not None, (x2 or "")[:300])
    # Copy-to and move-to slots: the pair goes together.
    app.call("navigate", index=5)
    app.idle(quiet_ms=300, timeout_ms=20000)
    cur = os.path.basename(app.call("state")["file"])
    _, e = app.action("cull-copy-to:1")
    sc.check("copy to slot 1 runs", e is None, str(e))
    app.call("navigate", index=6)
    app.idle(quiet_ms=300, timeout_ms=20000)
    cur6 = os.path.basename(app.call("state")["file"])
    _, e = app.action("cull-move-to:2")
    sc.check("move to slot 2 runs", e is None, str(e))
    s.r.pump(1.0)
    app.idle(quiet_ms=500, timeout_ms=30000)
    s1, s2 = sorted(os.listdir(slot1)), sorted(os.listdir(slot2))
    sc.info("slots", {"copied": cur, "slot1": s1, "moved": cur6, "slot2": s2})
    stem5, stem6 = os.path.splitext(cur)[0], os.path.splitext(cur6)[0]
    sc.check("copy-to copies both halves of the pair",
             {stem5 + ".JPG", stem5 + ".DNG"} <= set(s1), s1)
    sc.check("move-to moves both halves of the pair",
             {stem6 + ".JPG", stem6 + ".DNG"} <= set(s2)
             and not os.path.exists(os.path.join(folder, stem6 + ".JPG"))
             and not os.path.exists(os.path.join(folder, stem6 + ".DNG")), s2)
    app.shot("cull")
    app.finish()


# --- 3: Develop and export -------------------------------------------------------------------


def walk_files(d):
    return {os.path.join(dp, n) for dp, _, ns in os.walk(d) for n in ns}


def s3_develop(s, sc, a):
    folder = os.path.join(sc.dir, "photos")
    os.makedirs(folder)
    photo = os.path.join(folder, "IMG_0001.DNG")
    with open(photo, "wb") as f:
        f.write(gl.dng(12345, gl.facts(12345)))
    big_jpeg(os.path.join(folder, "IMG_0002.jpg"))
    sidecar = photo + ".specular.xmp"
    app = sc.app("viewer", [photo])
    if not sc.check("remote control answers", app.connect()):
        return
    app.idle(timeout_ms=60000)
    t = time.time()
    _, e = app.action("develop")
    sc.check("the develop action runs", e is None, str(e))
    st = app.wait_for("Develop", lambda st: st["develop"], timeout=30)
    app.idle(timeout_ms=60000)
    sc.time("enter_develop_s", time.time() - t)
    sc.check("Develop opens on the photo", st is not None and st["develop_file"] == photo,
             st and st["develop_file"])
    app.focus()
    app.shot("develop")
    # Speed edit: hold E, two arrow steps up (exposure).
    t = time.time()
    s.keys("-P", "e", "-k", "Up", "-k", "Up", "-p", "e")
    r = app.idle(quiet_ms=500, timeout_ms=30000)
    # Until the window is idle: the refined render, then the speed-edit readout fading out.
    sc.time("speed_edit_to_idle_s", time.time() - t)
    sc.info("speed_edit_idle", r)
    sc.check("settles after a slider change", r["settled"], r)
    # The crop tool on, and confirmed.
    s.keys("r")
    app.idle(quiet_ms=400, timeout_ms=20000)
    app.shot("crop")
    s.keys("-k", "Return")
    app.idle(quiet_ms=400, timeout_ms=20000)
    end = time.time() + 15
    while time.time() < end and not os.path.exists(sidecar):
        s.r.pump(0.3)
    sc.check("the Develop sidecar is written", os.path.exists(sidecar), sorted(os.listdir(folder)))
    body = open(sidecar, errors="replace").read() if os.path.exists(sidecar) else ""
    sc.check("the sidecar holds the exposure change", "xposure" in body, body[:400])
    # Export from the app: the Export sheet (current photo, its default folder).
    before = walk_files(folder)
    _, e = app.action("develop-export")
    sc.check("the Export sheet opens", e is None, str(e))
    st = app.wait_for("the Export sheet", lambda st: st["dialogs"], timeout=15)
    sc.info("export_dialog", st and st["dialogs"])
    sc.info("export_focus", st and st["focus"])
    app.shot("export-sheet")
    t = time.time()
    s.keys("-k", "Return")
    new = set()
    end = time.time() + 120
    while time.time() < end:
        new = {p for p in walk_files(folder) - before
               if not p.endswith((".xmp", ".part")) and not os.path.basename(p).startswith(".")}
        if new:
            s.r.pump(1.0)
            break
        s.r.pump(0.3)
    sc.time("app_export_s", time.time() - t)
    sc.info("app_export_files", sorted(new))
    sc.check("the app exports the photo (Enter on the sheet)", bool(new), sorted(new))
    app.finish()
    # The CLI on the same sidecar.
    if new:
        out = os.path.join(sc.dir, "cli")
        os.makedirs(out)
        appfile = sorted(new)[0]
        fmt = os.path.splitext(appfile)[1].lstrip(".").lower()
        fmt = {"jpg": "jpeg", "tif": "tiff"}.get(fmt, fmt)
        home = os.path.join(sc.dir, "home")
        env = dict(s.wenv, HOME=home, XDG_CONFIG_HOME=os.path.join(home, "config"),
                   XDG_DATA_HOME=os.path.join(home, "data"),
                   XDG_CACHE_HOME=os.path.join(home, "cache"), SPECULAR_NO_NETWORK="1")
        t = time.time()
        p = subprocess.run([a.bin, "develop", "export", photo, "--format", fmt, "--folder", out],
                           env=env, capture_output=True, text=True, timeout=600)
        sc.time("cli_export_s", time.time() - t)
        sc.check("the CLI export runs", p.returncode == 0, (p.stdout + p.stderr)[-400:])
        cli = sorted(glob.glob(os.path.join(out, "*")))
        sc.info("cli_files", cli)
        if cli:
            with open(cli[0], "rb") as f1, open(appfile, "rb") as f2:
                same = f1.read() == f2.read()
            sc.check("app export = CLI export, bit for bit", same,
                     {"app": [appfile, os.path.getsize(appfile)],
                      "cli": [cli[0], os.path.getsize(cli[0])]})
    # Read again: a new session opens Develop on the photo with its sidecar.
    app = sc.app("reopen", [photo])
    if app.connect():
        app.idle(timeout_ms=60000)
        app.action("develop")
        st = app.wait_for("Develop", lambda st: st["develop"], timeout=30)
        app.idle(timeout_ms=60000)
        app.shot("reopened")
        sc.check("Develop reopens on the edited photo", st is not None)
        app.finish()


# --- 6: EXIF search -------------------------------------------------------------------------


def expected_count(unique, links, jpeg_pred, raw_pred):
    n = 0
    for i in range(unique):
        f = gl.facts(i)
        if jpeg_pred(f):
            n += 1 + gl.links_of(i, unique, links)
        if f["raw"] and raw_pred(f):
            n += 1
    return n


QUERIES = [
    ("iso>=3200", lambda f: f["iso"] >= 3200, lambda f: f["iso"] >= 3200),
    ("flash:yes", lambda f: f["flash"] & 1 == 1, lambda f: False),
    ("focal35:24..35", lambda f: 24 <= f["focal35"] <= 35, lambda f: False),
    ("exif:0xA405=50", lambda f: f["focal35"] == 50, lambda f: False),
    ("iso>=3200 flash:yes focal35:24..35",
     lambda f: f["iso"] >= 3200 and f["flash"] & 1 and 24 <= f["focal35"] <= 35,
     lambda f: False),
    ("artist:kim", lambda f: f["artist"] == "Kim Poe", lambda f: False),
]


def s6_search(s, sc, a):
    _, unique, links = library_dir(a)
    home = os.path.join(os.path.dirname(sc.dir), "1-library", "home")
    if not os.path.isdir(home):
        sc.check("needs the catalogue of scenario 1 (run --only 1,6)", False, home)
        return
    env = {"HOME": home, "XDG_CONFIG_HOME": os.path.join(home, "config"),
           "XDG_CACHE_HOME": os.path.join(home, "cache"),
           "XDG_DATA_HOME": os.path.join(home, "data"),
           "XDG_STATE_HOME": os.path.join(home, "state")}
    app = sc.app("library", ["--library"], env=env)
    if not sc.check("remote control answers", app.connect()):
        return
    app.wait_for("the Library", lambda st: st["library"]["searched"], timeout=120)
    app.idle(timeout_ms=60000)
    app.focus()
    lat = {}
    for q, jp, rp in QUERIES:
        want = expected_count(unique, links, jp, rp)
        want_no_raw = expected_count(unique, links, jp, lambda f: False)
        s.keys("/")
        s.keys("-M", "ctrl", "a", "-m", "ctrl", "-k", "BackSpace")
        app.wait_for("the cleared search", lambda st: not st["library"]["searching"],
                     timeout=120, every=0.1)
        t = time.time()
        s.type_text(q, settle=0.05)
        s.r.pump(0.2)
        st = app.wait_for(f"search {q}", lambda st: not st["library"]["searching"],
                          timeout=120, every=0.05)
        lat[q] = round(time.time() - t, 2)
        got = st["library"]["total"] if st else None
        sc.check(f"{q}: count as generated", got in (want, want_no_raw),
                 {"got": got, "expected": want, "without_raw": want_no_raw,
                  "error": st and st["library"]["search_error"]})
        s.keys("-k", "Escape")
        s.keys("-k", "Escape")
    sc.info("query_latency_s_incl_typing", lat)
    # A bad tag: the inline message.
    s.keys("/")
    s.keys("-M", "ctrl", "a", "-m", "ctrl", "-k", "BackSpace")
    s.type_text("exif:0xBEEF=1")
    st = app.wait_for("the bad-tag message", lambda st: st["library"]["search_error"]
                      or st["alerts"], timeout=10)
    sc.check("a bad EXIF tag is reported", st is not None,
             st and {"search_error": st["library"]["search_error"], "alerts": st["alerts"]})
    app.shot("bad-tag")
    # Completion: the value picker for `lens:`.
    s.keys("-M", "ctrl", "a", "-m", "ctrl", "-k", "BackSpace")
    s.type_text("lens:")
    app.idle(quiet_ms=400, timeout_ms=10000)
    app.shot("completion")
    s.keys("-k", "Escape")
    app.finish(check_errors=False)


# --- 7: Open as… ----------------------------------------------------------------------------


def s7_open_as(s, sc, a):
    folder = os.path.join(sc.dir, "odd")
    os.makedirs(folder)
    with open(os.path.join(folder, "game.scr"), "wb") as f:
        f.write(bytes((i * 37) & 0xFF for i in range(6912)))
    with open(os.path.join(folder, "cloud.pcd"), "w") as f:
        f.write("# .PCD v0.7 - Point Cloud Data file format\nVERSION 0.7\nFIELDS x y z\n"
                "SIZE 4 4 4\nTYPE F F F\nCOUNT 1 1 1\nWIDTH 4\nHEIGHT 1\n"
                "VIEWPOINT 0 0 0 1 0 0 0\nPOINTS 4\nDATA ascii\n0 0 0\n1 0 0\n0 1 0\n0 0 1\n")
    with open(os.path.join(folder, "scan.raw"), "wb") as f:
        f.write(bytes((i * 7) & 0xFF for i in range(64 * 64 * 2)))
    # Neither a PLS playlist nor Gerber: the two formats claiming .pls are asked about.
    with open(os.path.join(folder, "paste.pls"), "w") as f:
        f.write("just some words\nand more words\n")
    big_jpeg(os.path.join(folder, "a.jpg"))
    app = sc.app("viewer", [os.path.join(folder, "a.jpg")])
    if not sc.check("remote control answers", app.connect()):
        return
    app.idle(timeout_ms=60000)
    app.focus()
    results = {}
    for name in ("game.scr", "cloud.pcd", "scan.raw", "paste.pls"):
        app.call("open", files=[os.path.join(folder, name)])
        st = app.wait_for(f"a prompt for {name}", lambda st: st["dialogs"], timeout=8)
        results[name] = st and st["dialogs"]
        app.shot("ask-" + name)
        if st and st["dialogs"]:
            if name == "paste.pls":
                # The first format, for every .pls in this folder, then Open.
                s.keys("1")
                s.keys("-k", "Right")
                app.shot("ask-scope")
                s.keys("-k", "Return")
            else:
                s.keys("-k", "Escape")
            app.idle(quiet_ms=400, timeout_ms=20000)
    sc.info("dialogs", results)
    sc.check("a headerless .raw asks for its layout", bool(results.get("scan.raw")), results)
    sc.check("a .pls that is neither format asks which kind of file",
             bool(results.get("paste.pls")), results)
    sc.check("a 6912-byte .scr opens without asking", not results.get("game.scr"), results)
    sc.check("a PCD point cloud opens without asking", not results.get("cloud.pcd"), results)
    data = os.path.join(sc.dir, "home", "data")
    end = time.time() + 5
    tsv = []
    while time.time() < end and not tsv:
        tsv = glob.glob(os.path.join(data, "**", "file-kinds.tsv"), recursive=True)
        s.r.pump(0.3)
    sc.check("the choice is remembered in file-kinds.tsv", bool(tsv), tsv)
    if tsv:
        with open(tsv[0]) as f:
            sc.info("file_kinds", f.read()[:600])
    app.call("open", files=[os.path.join(folder, "a.jpg")])
    app.idle(quiet_ms=300, timeout_ms=20000)
    app.call("open", files=[os.path.join(folder, "paste.pls")])
    st = app.wait_for("a second prompt", lambda st: st["dialogs"], timeout=4)
    sc.check("a remembered choice does not ask again", st is None, st and st["dialogs"])
    app.finish(check_errors=False)


# --- 10: client gallery and picks -------------------------------------------------------------


def s10_client(s, sc, a):
    folder = os.path.join(sc.dir, "wedding")
    out = os.path.join(sc.dir, "site")
    os.makedirs(folder)
    os.makedirs(out)
    files = []
    for i in range(30):
        p = os.path.join(folder, f"IMG_{i:04}.jpg")
        with open(p, "wb") as f:
            f.write(small_jpeg(i))
        files.append(p)
    with open(os.path.join(folder, "IMG_0003.dng"), "wb") as f:
        f.write(gl.dng(3, gl.facts(3)))
    app = sc.app("viewer", [files[0]])
    if not sc.check("remote control answers", app.connect()):
        return
    app.idle(timeout_ms=60000)
    r = app.call("mark", files=files, flag="pick")
    sc.check("30 picks marked over the remote control", r.get("marked") == 30, r)
    m = app.call("marked")["files"]
    sc.check("`marked` lists the 30 picks at once", len(m) == 30, len(m))
    app.focus()
    s.portal.answer([out])
    t = time.time()
    _, e = app.action("client-gallery")
    sc.check("the client gallery sheet opens", e is None, str(e))
    st = app.wait_for("the sheet", lambda st: st["dialogs"], timeout=15)
    sc.info("sheet", st and {"dialogs": st["dialogs"], "focus": st["focus"]})
    app.idle(quiet_ms=400, timeout_ms=15000)
    app.shot("client-sheet")
    calls = len(s.portal.taken())
    for _ in range(3):
        s.keys("-k", "Return")
        end = time.time() + 8
        while time.time() < end and len(s.portal.taken()) == calls:
            s.r.pump(0.2)
        if len(s.portal.taken()) > calls:
            break
        sc.note("Enter on the client gallery sheet asked for no folder; again")
        app.focus()
    site = None
    end = time.time() + 300
    while time.time() < end:
        idx = glob.glob(os.path.join(out, "*", "index.html"))
        if idx:
            site = os.path.dirname(idx[0])
            if len(glob.glob(os.path.join(site, "thumbs", "*"))) >= 30:
                break
        s.r.pump(0.5)
    sc.time("build_s_per_photo", (time.time() - t) / 30)
    sc.info("portal_calls", s.portal.taken())
    if not sc.check("the site is written (Enter, then the folder from the portal)",
                    site is not None, os.listdir(out)):
        app.finish(check_errors=False)
        return
    with open(os.path.join(site, "index.html"), errors="replace") as f:
        html = f.read()
    remote = re.findall(r"""(?:src|href)\s*=\s*["'](https?:[^"']*)""", html)
    sc.check("self-contained: no http(s) links", not remote, remote[:3])
    imgs = sorted(glob.glob(os.path.join(site, "images", "*")))
    thumbs = sorted(glob.glob(os.path.join(site, "thumbs", "*")))
    sc.check("an image and a thumbnail per pick", len(imgs) >= 30 and len(thumbs) >= 30,
             {"images": len(imgs), "thumbs": len(thumbs)})
    gps = []
    for p in imgs:
        with open(p, "rb") as f:
            head = f.read(8192)
        if b"\x25\x88" in head or b"\x88\x25" in head:
            gps.append(os.path.basename(p))
    sc.check("GPS removed from the published images", not gps, gps[:3])
    # Picks back: clear the marks, then the import dialog.
    app.call("mark", files=files, flag="none")
    picks = os.path.join(sc.dir, "picks.json")
    with open(picks, "w") as f:
        json.dump({"format": "specular-picks", "version": 1, "gallery": "wedding",
                   "picks": ["IMG_0001.jpg", "img_0002.JPG", "IMG_0003"]}, f)
    s.portal.answer([picks])
    _, e = app.action("import-client-picks")
    sc.check("the import dialog opens", e is None, str(e))
    st = app.wait_for("the import dialog", lambda st: st["dialogs"], timeout=15)
    sc.info("import_dialog", st and {"dialogs": st["dialogs"], "focus": st["focus"]})
    # Tab to "Choose picks file…", Enter (the portal answers picks.json), back to the field,
    # Enter applies.
    s.keys("-k", "Tab")
    s.keys("-k", "Return")
    end = time.time() + 15
    while time.time() < end and s.portal.pending():
        s.r.pump(0.2)
    app.idle(quiet_ms=400, timeout_ms=15000)
    app.shot("import-picks")
    s.keys("-k", "Up")  # back to the names field, where Enter applies
    st = app.ui()
    sc.info("focus_before_apply", st["focus"])
    s.keys("-k", "Return")
    app.idle(quiet_ms=400, timeout_ms=15000)
    st = app.ui()
    sc.info("after_apply", {"dialogs": st["dialogs"], "focus": st["focus"]})
    app.shot("after-apply")
    marked = sorted(os.path.basename(p) for p in app.call("marked")["files"])
    sc.info("marked_after_import", marked)
    sc.check("picks marked back by name, case and stem (RAW+JPEG pair too)",
             {"IMG_0001.jpg", "IMG_0002.jpg", "IMG_0003.jpg", "IMG_0003.dng"} <= set(marked)
             and len(marked) == 4, marked)
    app.finish(check_errors=False)


# --- 11: a slider dragged in Develop (preview while dragging) ------------------------------


def s11_drag(s, sc, a):
    """The Exposure slider dragged with a virtual pointer (lavapipe): drafts are timed, and
    Automatic keeps the view's size while they fit the frame budget (steps down when they
    don't); Always sharp always draws at the view's size."""
    folder = os.path.join(sc.dir, "photos")
    os.makedirs(folder)
    # A 12 MP picture (gradients and fine stripes), so the fit view is large.
    photo = os.path.join(folder, "IMG_0001.ppm")
    w, h = 4000, 3000
    with open(photo, "wb") as f:
        f.write(b"P6 %d %d 255\n" % (w, h))
        row = bytes(v for x in range(w) for v in (x * 255 // w, (x * 7) % 256, 128))
        for y in range(h):
            f.write(row[(y % 64) * 3:] + row[:(y % 64) * 3])
    from vpointer import VPointer
    pointer = VPointer(s.wenv)
    for mode in ("automatic", "sharp"):
        app = sc.app(f"viewer-{mode}", ["--set", f"performance.drag_preview={mode}", photo])
        if not sc.check(f"{mode}: remote control answers", app.connect()):
            return
        app.idle(timeout_ms=60000)
        app.action("develop")
        st = app.wait_for("Develop", lambda st: st["develop"], timeout=30)
        app.idle(timeout_ms=60000)
        app.focus()
        # Its label in the run's language.
        name = ftl(sc.lang, "develop", "develop-exposure")
        st = app.wait_for("the Exposure slider",
                          lambda st: any(x["label"] == name and x["bounds"]
                                         for x in st["sliders"]), timeout=20)
        slider = st and next(x for x in st["sliders"] if x["label"] == name)
        if not sc.check(f"{mode}: the Exposure slider is on screen", slider, st and st["sliders"]):
            app.quit()
            app.finish(check_errors=False)
            continue
        x0, y0, x1, y1 = slider["bounds"]
        # The knob (the row's lower part, the value at 0 in the middle of the track).
        x, y = int((x0 + x1) / 2), int(y1 - 12)
        sc.info(f"{mode}_slider", slider)
        pointer.move_to(x, y)
        s.r.pump(0.2)
        pointer.button(True)
        seen = []
        for i in range(60):
            x += 2 if i % 20 < 10 else -2
            pointer.move_to(x, y)
            s.r.pump(0.05)
            if i % 6 == 5:
                d = app.ui()["develop_drag"]
                seen.append((d["level"], d["timed"], d["last"], d["dragging"]))
        mid = app.ui()["develop_drag"]
        pointer.button(False)
        r = app.idle(quiet_ms=500, timeout_ms=30000)
        after = app.ui()["develop_drag"]
        sc.info(f"{mode}_levels_while_dragging", seen)
        sc.info(f"{mode}_after_release", after)
        sc.check(f"{mode}: the drag is seen as a slider drag", any(x[3] for x in seen), seen)
        sc.check(f"{mode}: drafts are timed while dragging", mid["timed"] > 0, mid)
        if mode == "sharp":
            sc.check("sharp: every draft at the view's size",
                     all(x[0] == "Full" for x in seen)
                     and all(x[2] is None or x[2]["divisor"] == 1 for x in seen), seen)
        else:
            # Drafts within the budget stay sharp; slower ones step down.
            ms = [x[2]["ms"] for x in seen if x[2]]
            slow = len(ms) >= 4 and sorted(ms)[len(ms) // 2] > mid["budget_ms"]
            sc.info("automatic_median_ms", ms and sorted(ms)[len(ms) // 2])
            sc.check("automatic: slow drafts step down, fast ones stay at the view's size",
                     any(x[0] != "Full" for x in seen) == slow, [mid["budget_ms"], seen])
        sc.check(f"{mode}: settles after the release", r["settled"], r)
        sc.check(f"{mode}: the drag has ended", not after["dragging"], after)
        app.shot(f"after-{mode}")
        app.quit()
        app.finish()
    pointer.close()


# --- 12: desktop search ---------------------------------------------------------------------------


def cli_env(s, sc):
    """The environment sc.app gives the viewer (the same home and XDG folders), for the
    command-line modes: `--integrate`, `--search-provider`."""
    home = os.path.join(sc.dir, "home")
    os.makedirs(home, exist_ok=True)
    e = dict(s.wenv)
    e.update(HOME=home, XDG_CONFIG_HOME=os.path.join(home, "config"),
             XDG_CACHE_HOME=os.path.join(home, "cache"),
             XDG_DATA_HOME=os.path.join(home, "data"),
             XDG_STATE_HOME=os.path.join(home, "state"),
             SPECULAR_LOG_FILE=os.path.join(sc.dir, "cli-log.txt"), SPECULAR_NO_NETWORK="1")
    return e


def gdbus(env, dest, path, method, *args):
    """One method call on the private session bus; its printed answer ("" on failure)."""
    r = subprocess.run(["gdbus", "call", "--session", "--dest", dest, "--object-path", path,
                        "--method", method, *args], env=env, capture_output=True, text=True,
                       timeout=30)
    return r.stdout.strip() if r.returncode == 0 else ""


GNOME = ("app.specular.Specular.SearchProvider", "/app/specular/Specular/SearchProvider",
         "org.gnome.Shell.SearchProvider2")
KRUNNER = ("app.specular.Specular.KRunner", "/app/specular/KRunner", "org.kde.krunner1")


def s12_desktop_search(s, sc, a):
    folder = os.path.join(sc.dir, "pics")
    os.makedirs(folder)
    beach = []
    for i in range(5):
        p = os.path.join(folder, f"beach_{i}.jpg")
        with open(p, "wb") as f:
            f.write(small_jpeg(i))
        beach.append(p)
    with open(os.path.join(folder, "hill.jpg"), "wb") as f:
        f.write(small_jpeg(9))
    for n, body in {"beach notes.txt": "a day at the beach\n",
                    "beach list.csv": "a,b\n1,2\n"}.items():
        with open(os.path.join(folder, n), "w") as f:
            f.write(body)
    # The catalogue, as the Library leaves it.
    app = sc.app("index", ["--library"], env={"SPECULAR_GATE_SCRIPT": "index",
                                             "SPECULAR_GATE_WALK": folder,
                                             "SPECULAR_GATE_SECS": "1"})
    app.connect()
    end = time.time() + 600
    while app.proc.poll() is None and time.time() < end:
        s.r.pump(0.5)
    app.v = None
    app.finish(check_errors=False)

    env = cli_env(s, sc)
    r = subprocess.run([a.bin, "--integrate", "single-instance"], env=env, capture_output=True,
                       text=True, timeout=60)
    sc.check("single instance through D-Bus is on", r.returncode == 0, r.stdout + r.stderr)
    s.r.spawn(f"{sc.key}-provider", [a.bin, "--search-provider"], env)
    dbus = ("org.freedesktop.DBus", "/org/freedesktop/DBus")
    end = time.time() + 30
    while time.time() < end and "true" not in gdbus(env, *dbus, "org.freedesktop.DBus.NameHasOwner",
                                                    KRUNNER[0]):
        s.r.pump(0.2)
    got = gdbus(env, GNOME[0], GNOME[1], GNOME[2] + ".GetInitialResultSet", "['beach']")
    sc.info("initial_result_set", got)
    found = sorted(re.findall(r"'([^']+)'", got))
    sc.check("GNOME: the beach pictures, no text files", found == sorted(beach), found)
    got = gdbus(env, GNOME[0], GNOME[1], GNOME[2] + ".GetResultMetas", f"['{beach[0]}']")
    sc.check("GNOME: a result's name and folder", "'beach_0.jpg'" in got and "pics" in got,
             got[:300])
    got = gdbus(env, *KRUNNER[:2], KRUNNER[2] + ".Match", "beach_3")
    sc.check("KRunner: one match, with its folder", got.count("beach_3.jpg") >= 2, got[:300])

    # "More results" and picking a result go to the running viewer.
    app = sc.app("viewer", ["--library"])
    if not sc.check("remote control answers", app.connect()):
        return
    app.wait_for("the Library", lambda st: st["library"]["searched"], timeout=120)
    gdbus(env, GNOME[0], GNOME[1], GNOME[2] + ".LaunchSearch", "['beach', 'ext:jpg', '-beach_4']", "0")
    st = app.wait_for("the Library searching", lambda st: st["library"]["query"]
                      == "beach ext:jpg -beach_4" and not st["library"]["searching"], timeout=60)
    sc.check("LaunchSearch: the Library shows the query", st is not None,
             st and st["library"]["query"])
    # (The Library itself lists the text files too: ext:jpg leaves them out.)
    sc.check("LaunchSearch: four pictures", st is not None and st["library"]["total"] == 4,
             st and st["library"]["total"])
    app.idle(timeout_ms=30000)
    app.shot("launch-search")
    gdbus(env, GNOME[0], GNOME[1], GNOME[2] + ".ActivateResult", beach[2], "[]", "0")
    end = time.time() + 30
    shown = None
    while time.time() < end:
        st, _ = app.try_call("state")
        shown = st and st.get("file")
        if shown == beach[2]:
            break
        s.r.pump(0.3)
    sc.check("ActivateResult: the picture opens in the running viewer", shown == beach[2], shown)
    # An id the provider did not hand out opens nothing.
    gdbus(env, GNOME[0], GNOME[1], GNOME[2] + ".ActivateResult",
          os.path.join(folder, "beach notes.txt"), "[]", "0")
    s.r.pump(2.0)
    st, _ = app.try_call("state")
    sc.check("ActivateResult: only results open", st and st.get("file") == beach[2],
             st and st.get("file"))
    app.finish()
