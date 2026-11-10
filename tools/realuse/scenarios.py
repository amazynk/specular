"""The ten real-use scenarios of tools/realuse/run.py. Each takes (session, scenario, args)
and records checks, timings and notes on the scenario."""

import glob
import json
import os
import re
import shutil
import struct
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
sys.path.insert(0, os.path.join(ROOT, "tools", "a11y"))

import gen_library as gl  # noqa: E402
import series_check  # noqa: E402

JPEG_SEEDS = os.path.join(ROOT, "crates/formats/specular-jpeg/fuzz/seeds")


def ftl(lang, file, key):
    """A one-line message of the app's translations (no arguments)."""
    path = os.path.join(ROOT, "crates/specular-i18n/i18n", lang or "en", file + ".ftl")
    with open(path, encoding="utf-8") as f:
        for line in f:
            if line.startswith(key + " = "):
                return line[len(key) + 3:].strip()
    return None


def library_dir(a):
    d = os.path.join(a.data, "library")
    if not os.path.exists(os.path.join(d, "generated.json")):
        gl.generate(d, a.unique, a.links)
        with open(os.path.join(d, "generated.json"), "w") as f:
            json.dump({"unique": a.unique, "links": a.links}, f)
    with open(os.path.join(d, "generated.json")) as f:
        g = json.load(f)
    return d, g["unique"], g["links"]


def small_jpeg(i, exif=True):
    f = gl.facts(i)
    return gl.jpeg(i, gl.exif_block(f) if exif else b"")


def big_jpeg(path):
    """A real camera-sized-ish JPEG from the committed seeds."""
    shutil.copy(os.path.join(JPEG_SEEDS, "s420.jpg"), path)


def loc_total(st):
    return sum(l["files"] for l in st["library"]["locations"])


# --- 1: a 100k-photo library ----------------------------------------------------------------


def s1_library(s, sc, a):
    lib, unique, links = library_dir(a)
    expected = unique + links + unique // 50  # JPEGs, their links, the DNG twins
    sc.info("expected_images", expected)
    # Pass 1: the app gate's `index` script adds the location, waits for the walk and the
    # facts pass and exits; its report has the index time and peak RSS.
    image = os.path.join(sc.dir, "shown.jpg")
    big_jpeg(image)
    t0 = time.time()
    cpu0 = os.times()
    app = sc.app("index", ["--library"], env={"SPECULAR_GATE_SCRIPT": "index",
                                             "SPECULAR_GATE_WALK": lib,
                                             "SPECULAR_GATE_IMAGE": image,
                                             "SPECULAR_GATE_SECS": "2"})
    connected = app.connect()
    sc.check("index: remote control answers", connected)
    peak = None
    last = None
    while app.proc.poll() is None and time.time() - t0 < 3600:
        if app.v:
            st, e = app.try_call("ui-state")
            if st:
                last = st["library"]
        s.r.pump(5.0)
    wall = time.time() - t0
    cpu1 = os.times()
    sc.time("index_wall_s", wall)
    sc.time("index_children_cpu_s", (cpu1.children_user + cpu1.children_system)
            - (cpu0.children_user + cpu0.children_system))
    sc.info("index_last_progress", last)
    app.v = None
    g = None
    try:
        with open(app.gate) as f:
            g = json.load(f)
    except (OSError, ValueError):
        pass
    sc.check("index: gate report written", g is not None)
    if g:
        sc.check("index: walk and facts finished", g.get("index_finished") is True,
                 {k: g.get(k) for k in ("index_ms", "index_finished", "settle_timeout")})
        sc.time("index_ms", g.get("index_ms"))
        sc.time("index_peak_rss_mb", g.get("peak_rss_mb"))
        sc.check("index: no frame without a cause", g.get("uncaused") == 0,
                 g.get("uncaused_at_ms"))
    app.finish()

    # Pass 2: the catalogue as left, counts and a scroll through the grid.
    app = sc.app("browse", ["--library"])
    if not sc.check("browse: remote control answers", app.connect()):
        return
    st = app.wait_for("the Library's locations", lambda st: st["library"]["searched"]
                      and st["library"]["locations"], timeout=120)
    if st:
        total = loc_total(st)
        sc.check("catalogue holds every generated image", total == expected,
                 {"catalogued": total, "expected": expected})
        pending = sum(l["pending"] for l in st["library"]["locations"])
        sc.check("no image waits for its facts", pending == 0, pending)
        sc.check("the Library lists them", st["library"]["total"] == expected,
                 st["library"]["total"])
    r = app.idle(timeout_ms=60000)
    sc.check("browse: settles", r["settled"], r)
    app.focus()
    app.shot("grid")
    t = time.time()
    for _ in range(200):
        s.keys("-k", "Next", settle=0.02)
    r = app.idle(timeout_ms=60000)
    sc.time("scroll_200_pgdn_s", time.time() - t)
    sc.check("grid settles after 200 Page Down", r["settled"], r)
    app.shot("grid-scrolled")
    app.finish()


# --- 4: a DICOM study --------------------------------------------------------------------------


def s4_dicom(s, sc, a):
    study = os.path.join(sc.dir, "disc")
    ct = os.path.join(study, "CT1")
    ct2 = os.path.join(study, "CT2")
    os.makedirs(ct)
    os.makedirs(ct2)
    order = [3, 0, 4, 1, 2]  # written out of order: the viewer sorts by position
    for n, k in enumerate(order):
        series_check.write_slice(os.path.join(ct, f"IM{n:04}.dcm"), k)
    for k in range(3):
        series_check.write_slice(os.path.join(ct2, f"IM{k:04}.dcm"), k, number=2)
    first = os.path.join(ct, "IM0000.dcm")
    app = sc.app("viewer", ["--set", "profile.capabilities=general,medical", first])
    if not sc.check("remote control answers", app.connect()):
        return
    t = time.time()
    r = app.idle(timeout_ms=60000)
    sc.time("first_slice_s", time.time() - t)
    sc.check("first slice settles", r["settled"], r)
    st = app.call("state")
    sc.check("series mode lists the five slices", st["count"] == 5, st)
    md = app.call("metadata")
    sc.info("metadata_keys", sorted(md)[:40])
    # Series order: stepping goes by position, so the slice index follows z.
    zs = []
    for i in range(5):
        app.call("navigate", index=i)
        app.idle(quiet_ms=300, timeout_ms=20000)
        zs.append(os.path.basename(app.call("state").get("file") or ""))
    sc.info("series_files_in_order", zs)
    by_z = [f"IM{order.index(k):04}.dcm" for k in range(5)]
    sc.check("series steps in slice position order, not name order", zs == by_z,
             {"got": zs, "by_position": by_z})
    # W/L through view-set.
    v, e = app.try_call("view-set", window={"center": 40, "width": 400})
    sc.check("window and level set by view-set", e is None, str(e))
    app.idle(quiet_ms=300, timeout_ms=20000)
    vg = app.call("view-get")
    w = vg.get("window") or {}
    sc.check("view-get reads the window back in HU",
             (w.get("center"), w.get("width"), w.get("unit")) == (40, 400, "HU"), w)
    app.shot("wl")
    # MPR.
    t = time.time()
    _, e = app.action("mpr")
    sc.check("MPR opens", e is None, str(e))
    st = app.wait_for("MPR", lambda st: st["mpr"], timeout=30)
    r = app.idle(timeout_ms=60000)
    sc.time("mpr_first_view_s", time.time() - t)
    sc.check("MPR settles", r["settled"] and st is not None, r)
    app.shot("mpr")
    app.action("mpr")
    app.idle(timeout_ms=20000)
    # Studies browser.
    _, e = app.action("studies")
    sc.check("Studies browser opens", e is None, str(e))
    app.idle(quiet_ms=500, timeout_ms=20000)
    st = app.ui()
    sc.info("studies_focus", st["focus"])
    app.shot("studies")
    s.keys("-k", "Escape")
    app.idle(quiet_ms=300, timeout_ms=20000)
    # Anonymised export of the series to a folder (the portal answers the folder).
    out = os.path.join(sc.dir, "anon")
    os.makedirs(out)
    s.portal.answer([out])
    _, e = app.action("export-anonymised")
    sc.check("the anonymised export sheet opens", e is None, str(e))
    st = app.wait_for("the sheet", lambda st: st["dialogs"], timeout=15)
    sc.info("anon_sheet", st and {"dialogs": st["dialogs"], "focus": st["focus"]})
    app.shot("anon-sheet")
    s.keys("-k", "Return")
    files = []
    end = time.time() + 120
    while time.time() < end:
        files = sorted(glob.glob(os.path.join(out, "**", "*"), recursive=True))
        files = [f for f in files if os.path.isfile(f)]
        if files and not s.portal.pending():
            s.r.pump(2.0)
            files = [f for f in sorted(glob.glob(os.path.join(out, "**", "*"), recursive=True))
                     if os.path.isfile(f)]
            break
        s.r.pump(0.5)
    sc.info("anon_files", [os.path.relpath(f, out) for f in files])
    sc.info("portal_calls", s.portal.taken())
    if sc.check("anonymised copies are written", bool(files), files):
        phi = [os.path.basename(f) for f in files
               if b"Test^Phantom" in open(f, "rb").read()]
        sc.check("no patient name in the copies", not phi, phi)
        app.call("open", files=[files[0]])
        r = app.idle(timeout_ms=30000)
        st = app.call("state")
        sc.check("an anonymised copy opens again", r["settled"]
                 and os.path.basename(st.get("file") or "") == os.path.basename(files[0]), st)
    app.finish()


# --- 5: a folder of mixed formats -------------------------------------------------------------

SKIP_WORDS = ("expected", "bad", "corrupt", "trunc", "invalid", "broken", "fuzz", "crash",
              "oracle", "reference", "golden", "ref.", "_ref", "-ref", "out.", "diff")
DATA_EXTS = {".txt", ".json", ".tsv", ".csv", ".md", ".py", ".rs", ".f32", ".u16", ".bin",
             ".sha256", ".toml", ".yaml", ".yml", ".sh", ".xml", ".lbl", ".hdr", ".log"}


def mixed_files(limit=80):
    picks = []
    for crate in sorted(glob.glob(os.path.join(ROOT, "crates/formats/*"))):
        cands = []
        for sub in ("tests/fixtures", "tests/data"):
            for p in sorted(glob.glob(os.path.join(crate, sub, "*"))):
                n = os.path.basename(p).lower()
                ext = os.path.splitext(n)[1]
                if (not os.path.isfile(p) or ext in DATA_EXTS or not ext
                        or any(w in n for w in SKIP_WORDS) or os.path.getsize(p) > 20 << 20):
                    continue
                cands.append(p)
        if cands:
            picks.append(cands[0])
    return picks[:limit]


def s5_mixed(s, sc, a):
    folder = os.path.join(sc.dir, "mixed")
    os.makedirs(folder)
    names = []
    for p in mixed_files():
        crate = p.split("/crates/formats/")[1].split("/")[0].replace("specular-", "")
        n = f"{crate}-{os.path.basename(p)}"
        shutil.copy(p, os.path.join(folder, n))
        names.append(n)
    sc.info("files", names)
    names.sort()
    # Every file is looked at in turn: a short sound file playing on into the folder's next
    # one (Settings › Viewing, on by default, gapless) would move the viewer on to the last
    # sound of the folder before its state is read.
    app = sc.app("viewer", ["--set", "viewing.sound_play_next=false",
                            os.path.join(folder, names[0])])
    if not sc.check("remote control answers", app.connect()):
        return
    app.idle(timeout_ms=60000)
    st = app.call("state")
    count = st["count"]
    sc.info("listed", count)
    # Fixtures include raw streams and side files no viewer lists (.frames, .framemd5, .ibd).
    sc.check("most fixtures are listed", count >= len(names) * 0.7,
             {"listed": count, "copied": len(names)})
    stalls, errors, moved = [], [], []
    per = {}
    for i in range(count):
        t = time.time()
        _, e = app.try_call("navigate", index=i)
        r = app.idle(quiet_ms=400, timeout_ms=30000)
        dt = time.time() - t
        st = app.call("state")
        f = os.path.basename(st.get("file") or "?")
        if st.get("index") != i:
            moved.append((i, st.get("index"), f))
        per[f] = round(dt, 2)
        # An animation or a playing sound draws on by design: a stall is loading that does
        # not finish.
        if not r["settled"] and "loading" in r["held_by"]:
            stalls.append((f, r["held_by"]))
        elif not r["settled"]:
            sc.info(f"keeps drawing: {f}", r["held_by"])
        if e is not None:
            stalls.append((f, str(e)))
        u = app.ui()
        alerts = [x.get("label") for x in u["alerts"] if x.get("label")]
        if alerts:
            errors.append((f, alerts[:2]))
    sc.info("open_s_per_file", per)
    sc.check("navigate index stays on the file asked for", not moved, moved[:8])
    sc.check("next/prev never stalls (each settles in 30 s)", not stalls, stalls[:8])
    sc.info("alerts_per_file", errors)
    reports = app.open_reports()
    sc.info("open_report_lines", len(reports))
    sc.check("an open report per shown file", len(reports) >= count * 0.7,
             {"reports": len(reports), "listed": count})
    # Gallery thumbnails.
    app.action("gallery")
    st = app.wait_for("the gallery", lambda st: st["gallery"], timeout=20)
    r = app.idle(timeout_ms=90000)
    sc.check("gallery settles with its thumbnails", r["settled"], r)
    st = app.ui()
    sc.info("gallery_images", st["gallery_images"])
    app.shot("gallery")
    app.finish(check_errors=False)


# --- 8: the text-file skip rule ------------------------------------------------------------------


def esri_grid():
    rows = ["ncols 4", "nrows 3", "xllcorner 0.0", "yllcorner 0.0", "cellsize 1.0",
            "NODATA_value -9999"]
    rows += [" ".join(str(r * 4 + c) for c in range(4)) for r in range(3)]
    return "\n".join(rows) + "\n"


def gaussian_cube():
    lines = ["Generated cube", "density", "    1    0.000000    0.000000    0.000000",
             "    3    0.500000    0.000000    0.000000",
             "    3    0.000000    0.500000    0.000000",
             "    3    0.000000    0.000000    0.500000",
             "    8    0.000000    0.000000    0.000000    0.000000"]
    vals = [f"{(i % 7) * 0.1:.5E}" for i in range(27)]
    for k in range(0, 27, 6):
        lines.append(" ".join(vals[k:k + 6]))
    return "\n".join(lines) + "\n"


def pds3(path_img):
    w, h = 8, 4
    lbl = ("PDS_VERSION_ID = PDS3\r\nRECORD_TYPE = FIXED_LENGTH\r\nRECORD_BYTES = 8\r\n"
           "FILE_RECORDS = 4\r\n^IMAGE = (\"%s\", 1)\r\nOBJECT = IMAGE\r\n  LINES = %d\r\n"
           "  LINE_SAMPLES = %d\r\n  SAMPLE_TYPE = UNSIGNED_INTEGER\r\n  SAMPLE_BITS = 8\r\n"
           "END_OBJECT = IMAGE\r\nEND\r\n" % (os.path.basename(path_img).upper(), h, w))
    return lbl, bytes(range(w * h))


def s8_text_skip(s, sc, a):
    folder = os.path.join(sc.dir, "mixed")
    os.makedirs(folder)
    real = []
    # Real ones.
    sky = os.path.join(ROOT, "crates/formats/specular-medvol/tests/data/skyscan_png")
    for p in glob.glob(os.path.join(sky, "*")):
        shutil.copy(p, folder)
    real.append("sample_rec.log")
    shutil.copy(os.path.join(ROOT, "crates/formats/specular-em/tests/data/zxy.map"),
                os.path.join(folder, "density.map"))
    real.append("density.map")
    with open(os.path.join(folder, "dens.cube"), "w") as f:
        f.write(gaussian_cube())
    real.append("dens.cube")
    with open(os.path.join(folder, "terrain.asc"), "w") as f:
        f.write(esri_grid())
    real.append("terrain.asc")
    lbl, img = pds3(os.path.join(folder, "frame.img"))
    with open(os.path.join(folder, "frame.lbl"), "w") as f:
        f.write(lbl)
    with open(os.path.join(folder, "FRAME.IMG"), "wb") as f:
        f.write(img)
    real.append("frame.lbl")
    big_jpeg(os.path.join(folder, "a-photo.jpg"))
    real.append("a-photo.jpg")
    # Text twins.
    twins = {"notes.log": "build ok\nall good\n", "config.xml": "<config><a>1</a></config>\n",
             "lut.cube": "TITLE \"lut\"\nLUT_3D_SIZE 2\n0 0 0\n1 0 0\n0 1 0\n1 1 0\n0 0 1\n"
             "1 0 1\n0 1 1\n1 1 1\n", "site.map": "# a map of the site\nA -> B\n",
             "printer.lbl": "^XA^FO50,50^FDHello^FS^XZ\n", "key.asc":
             "-----BEGIN PGP PUBLIC KEY BLOCK-----\nabc\n-----END PGP PUBLIC KEY BLOCK-----\n"}
    for n, body in twins.items():
        with open(os.path.join(folder, n), "w") as f:
            f.write(body)
    sc.info("real", real)
    sc.info("twins", sorted(twins))
    listing = sorted(os.listdir(folder))
    sc.info("folder", listing)

    app = sc.app("viewer", [os.path.join(folder, "a-photo.jpg")])
    if not sc.check("remote control answers", app.connect()):
        return
    app.idle(timeout_ms=60000)
    shown = []
    st = app.call("state")
    for i in range(st["count"]):
        app.call("navigate", index=i)
        app.idle(quiet_ms=300, timeout_ms=20000)
        shown.append(os.path.basename(app.call("state")["file"]))
        u = app.ui()
        if u["dialogs"]:
            sc.note(f"a dialog while showing {shown[-1]}", u["dialogs"])
    sc.info("viewer_listing", shown)
    # A .log is a plain-text document the text viewer opens (specular-textdoc); it must open
    # as text, not as a SkyScan reconstruction. The other twins are not listed at all.
    leaked = [n for n in shown if n in twins and n != "notes.log"]
    sc.check("viewer: no text twin listed (a plain .log aside)", not leaked, leaked)
    as_text = [ln for ln in app.open_reports() if "/notes.log:" in ln]
    sc.check("viewer: notes.log opens as plain text, not as a SkyScan log",
             bool(as_text) and "Plain text" in as_text[0], as_text[:1])
    missing = [n for n in real if n not in shown and n.upper() not in shown]
    sc.check("viewer: every real file listed", not missing, missing)
    # Gallery.
    app.action("gallery")
    app.wait_for("the gallery", lambda st: st["gallery"], timeout=20)
    app.idle(timeout_ms=60000)
    st = app.ui()
    sc.check("gallery: lists the same files", st["gallery_images"] == len(shown),
             {"gallery": st["gallery_images"], "viewer": len(shown)})
    sc.check("gallery: no prompt", not st["dialogs"], st["dialogs"])
    app.finish(check_errors=False)
    # Library.
    app = sc.app("library", ["--library"], env={"SPECULAR_GATE_SCRIPT": "index",
                                               "SPECULAR_GATE_WALK": folder,
                                               "SPECULAR_GATE_SECS": "1"})
    app.connect()
    end = time.time() + 600
    last = None
    while app.proc.poll() is None and time.time() < end:
        st, _ = app.try_call("ui-state") if app.v else (None, None)
        if st:
            last = st
        s.r.pump(1.0)
    app.v = None
    app.finish(check_errors=False)
    if last:
        total = loc_total(last)
        sc.info("library_catalogued", total)
        sc.check("Library: catalogues the viewer's files, no twins", total == len(shown),
                 {"library": total, "viewer": len(shown)})


# --- 9: tethering with the mock camera -----------------------------------------------------------


def s9_tether(s, sc, a):
    cam = os.path.join(sc.dir, "camera")
    dest = os.path.join(sc.dir, "Tethered")
    os.makedirs(cam)
    os.makedirs(dest)
    for i in range(20):
        if i % 4 == 3:
            with open(os.path.join(cam, f"DSC{i:04}.DNG"), "wb") as f:
                f.write(gl.dng(i, gl.facts(i)))
        else:
            with open(os.path.join(cam, f"DSC{i:04}.JPG"), "wb") as f:
                f.write(small_jpeg(i))
    start = os.path.join(dest, "start.jpg")
    big_jpeg(start)
    app = sc.app("viewer", ["--set", f"cull.tether_folder={dest}",
                            "--set", "cull.tether_rename={name}_tethered", start],
                 env={"SPECULAR_TETHER_MOCK": cam})
    if not sc.check("remote control answers", app.connect()):
        return
    app.idle(timeout_ms=60000)
    _, e = app.action("cull")
    sc.check("cull mode opens on the folder", e is None, str(e))
    app.wait_for("cull", lambda st: st["cull"], timeout=20)
    _, e = app.action("tether")
    sc.check("tether panel opens", e is None, str(e))
    app.idle(timeout_ms=20000)
    app.shot("tether-panel")
    lat = []
    for i in range(20):
        before = set(os.listdir(dest))
        t = time.time()
        _, e = app.action("tether-capture")
        if e:
            sc.note(f"capture {i}", str(e))
        new = set()
        end = time.time() + 30
        while time.time() < end and not new:
            new = {n for n in set(os.listdir(dest)) - before if not n.startswith(".")}
            s.r.pump(0.1)
        if new:
            r = app.idle(quiet_ms=300, timeout_ms=20000)
            lat.append(round(time.time() - t, 2))
    landed = sorted(n for n in os.listdir(dest) if n != "start.jpg" and not n.startswith("."))
    sc.info("landed", landed)
    sc.info("capture_to_shown_s", lat)
    if lat:
        sc.time("capture_to_shown_p50_s", sorted(lat)[len(lat) // 2])
    sc.check("20 shots land in the folder", len(landed) == 20, len(landed))
    sc.check("rename pattern applied", landed and all("_tethered" in n for n in landed),
             landed[:3])
    st = app.call("state")
    sc.check("the shots join the cull run", st["count"] >= 21, st["count"])
    app.shot("tether-captured")
    # Faults: card full, then unplugged.
    # (The mock reads its fault files on the app's next turn: a request wakes it first.)
    n_before = len(os.listdir(dest))
    open(os.path.join(cam, ".card-full"), "w").close()
    app.ui()
    s.r.pump(0.5)
    app.action("tether-capture")
    s.r.pump(2.0)
    app.idle(quiet_ms=200, timeout_ms=5000)
    u = app.ui()
    sc.info("card_full_alerts", u["alerts"])
    sc.check("card full: no file lands", len(os.listdir(dest)) == n_before,
             sorted(os.listdir(dest))[-3:])
    full = ftl(sc.lang, "tether", "tether-card-full")
    sc.check("card full: the panel says so", full in u["texts"], full)
    # The card's facts are read again on the refusal: no shots left, not the old count.
    tmpl = ftl(sc.lang, "tether", "tether-card-shots") or ""
    before, _, after = tmpl.partition("{ $shots }")
    zero = re.compile(re.escape(before[-2:]) + "[\u2068\u2069]*0[\u2068\u2069]*"
                      + re.escape(after))
    card = [t for t in u["texts"] if zero.search(t)]
    sc.check("card full: the card line says no shots left", bool(card),
             [t for t in u["texts"] if after.strip() and after.strip() in t][:3])
    app.shot("card-full")
    open(os.path.join(cam, ".unplug"), "w").close()
    app.ui()
    s.r.pump(0.5)
    app.action("tether-capture")
    s.r.pump(3.0)
    app.idle(quiet_ms=200, timeout_ms=5000)
    u2 = app.ui()
    sc.info("unplug_alerts", u2["alerts"])
    none = ftl(sc.lang, "tether", "tether-none")
    sc.check("unplugged: the panel says no camera, the card line is gone",
             none in u2["texts"] and full not in u2["texts"], [none, full])
    app.shot("unplugged")
    app.finish()


from scenarios_more import (  # noqa: E402
    s2_cull, s3_develop, s6_search, s7_open_as, s10_client, s11_drag, s12_desktop_search)

ALL = [
    ("1-library", "100k-photo library: index, counts, grid scroll", s1_library),
    ("2-cull", "Cull: marks, sidecars, undo, pairs, copy and move", s2_cull),
    ("3-develop", "Develop and export round trip", s3_develop),
    ("4-dicom", "DICOM study: series, W/L, MPR, Studies", s4_dicom),
    ("5-mixed", "Folder of mixed formats", s5_mixed),
    ("6-search", "EXIF search at 100k", s6_search),
    ("7-openas", "Open as…", s7_open_as),
    ("8-textskip", "Text-file skip rule", s8_text_skip),
    ("9-tether", "Tethering with the mock camera", s9_tether),
    ("10-client", "Client gallery and picks", s10_client),
    ("11-drag", "Develop slider drag: preview while dragging", s11_drag),
    ("12-dsearch", "Desktop search: GNOME Shell and KRunner providers", s12_desktop_search),
]
