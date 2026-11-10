"""The app suite of the performance gate (`tools/bench/perf-gate.py --suite app`).

Runs the real viewer in a private headless sway (tools/bench/stage1-measure.py's Session: own
XDG folders and HOME, no session bus) on generated inputs, one scripted run per case
(`SPECULAR_GATE_SCRIPT`, app/specular/src/app/frame_gate.rs), and gates what does not depend
on how loaded the machine is:

  frames      frames counted in a window where nothing should draw (idle, a button held
              still, a walk out of sight, a paused video, while the remote control asks for
              the state): any frame there fails;
  uncaused    frames that no cause asked for (anywhere in the run): any fails;
  one-frame   frames from a key press to the prefetched next image being sharp (budget 1:
              "next image < 16 ms" is one frame) and frames per orbit step of the 3D view;
  pixels      whether the first present already shows the image ("cold start to window"
              means the first present with the image);
  bytes read  information only: page-cache pages of the inputs after the run, starting from a
              dropped cache (fincore), and the bytes the app read with read() (/proc rchar).
              Under memory pressure from other work the page cache loses pages during the run
              (a CT run measured 16 KiB once and 25 MB the next), so neither is gated yet;
  peak RSS    against the baseline (+35 %: it moves by up to a third between runs) when
              blessed in the same environment (driver and profile), and the §2.3 ceiling
              (< 1 GB for a huge image); information only for the scroll benches.

Wall-clock times (time to first present, first view, input-to-frame latency, remote round
trip) are recorded as information only, marked "under load" when the load average was above
the CPU count / 4.

Bless with `perf-gate.py --suite app --bless` (writes tools/bench/perf-gate-app-baseline.json).
"""

import importlib.util
import json
import os
import shutil
import statistics
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
BASELINE = ROOT / "tools/bench/perf-gate-app-baseline.json"
SCHEMA = 1

_spec = importlib.util.spec_from_file_location("stage1", ROOT / "tools/bench/stage1-measure.py")
stage1 = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(stage1)


# ----- inputs -----------------------------------------------------------------------------


def prepare(bench_bin: Path, work: Path) -> dict:
    """Generates (or reuses) the inputs and returns their paths."""
    gen = work / "inputs"
    app = work / "app-inputs"
    subprocess.run([str(bench_bin), "gate", "gen", str(gen)], check=True, capture_output=True)
    subprocess.run([str(bench_bin), "gate", "gen-app", str(app)], check=True, capture_output=True)
    p = {
        "jpeg24": gen / "first-pixels-24mp.jpg",
        "tiff100k": gen / "tiff-100k.tif",
        "ct_pack": gen / "ct-512.pack",
        "gallery": app / "gallery",
        "video": ROOT / "crates/specular-video/tests/data/h264-444.mp4",
    }
    # Three 24 MP images to step through (hard links: separate files to the viewer).
    nxt = app / "next"
    nxt.mkdir(parents=True, exist_ok=True)
    for i in range(3):
        q = nxt / f"next-{i}.jpg"
        if not q.exists():
            os.link(p["jpeg24"], q)
    p["next"] = nxt
    # The CT series unpacked (the bench's `gate run` does the same beside the pack).
    ct = app / "ct"
    if not (ct / "slice-0511.dcm").exists():
        unpack(p["ct_pack"], ct)
    p["ct"] = ct
    # 100,000 files for the Library's grid: 100 distinct gallery pictures, 1,000 links each.
    lib = app / "lib100k"
    lib.mkdir(parents=True, exist_ok=True)
    for d in range(100):
        sub = lib / f"d{d:02d}"
        sub.mkdir(exist_ok=True)
        src = p["gallery"] / f"g{d:05d}.jpg"
        for i in range(1000):
            q = sub / f"p{d:02d}-{i:04d}.jpg"
            if not q.exists():
                os.link(src, q)
    p["lib100k"] = lib
    p["lib100k_db"] = app / "lib100k.sqlite3"
    # 50,000 points as GeoJSON for the map view (integers only, so every machine writes the
    # same bytes).
    gj = app / "points-50k.geojson"
    if not gj.exists():
        feats = []
        for i in range(50_000):
            c = i % 40
            lon = -170 + (c * 37 % 340) + ((i * 7919) % 2000 - 1000) / 10000
            lat = -50 + (c * 23 % 110) + ((i * 104729) % 2000 - 1000) / 10000
            feats.append(
                '{"type":"Feature","properties":{"n":%d},"geometry":{"type":"Point","coordinates":[%.4f,%.4f]}}'
                % (i, lon, lat)
            )
        gj.write_text('{"type":"FeatureCollection","features":[\n' + ",\n".join(feats) + "\n]}\n")
    p["geojson"] = gj
    return p


def unpack(pack: Path, out: Path) -> None:
    data = pack.read_bytes()
    out.mkdir(parents=True, exist_ok=True)
    at = 0

    def u32():
        nonlocal at
        v = int.from_bytes(data[at : at + 4], "little")
        at += 4
        return v

    for _ in range(u32()):
        n = u32()
        name = data[at : at + n].decode()
        at += n
        ln = u32()
        (out / name).write_bytes(data[at : at + ln])
        at += ln


def tree_files(paths) -> list:
    out = []
    for p in paths:
        p = Path(p)
        if p.is_dir():
            out += [q for q in sorted(p.rglob("*")) if q.is_file()]
        elif p.exists():
            out.append(p)
    return out


def resident_bytes(files) -> int:
    """Page-cache bytes of these files (fincore), counting each inode once."""
    seen = set()
    total = 0
    batch = []
    for f in files:
        try:
            ino = os.stat(f).st_ino
        except OSError:
            continue
        if ino in seen:
            continue
        seen.add(ino)
        batch.append(str(f))
    for i in range(0, len(batch), 500):
        r = subprocess.run(
            ["fincore", "--bytes", "--noheadings", "--output", "RES", *batch[i : i + 500]],
            capture_output=True,
            text=True,
        )
        for line in r.stdout.split():
            try:
                total += int(line)
            except ValueError:
                pass
    return total


# ----- cases ------------------------------------------------------------------------------
#
# Each gate is (metric, kind, budget): kind "zero" (must be 0), "max" (at most budget),
# "true", "regress" (at most baseline * (1 + budget), plus 1 for counts), "regress_env"
# (the same, only against a baseline from the same environment), "info" (recorded only).


def cases(p: dict) -> list:
    secs = "3"
    c = []

    def case(name, args, script=None, gates=(), env=None, inputs=(), timeout=180, what=""):
        e = dict(env or {})
        if script:
            e.setdefault("SPECULAR_GATE_SCRIPT", script)
            e.setdefault("SPECULAR_GATE_SECS", secs)
        c.append(
            dict(name=name, args=args, env=e, gates=list(gates), inputs=list(inputs), timeout=timeout, what=what)
        )

    # Peak RSS moves by up to a third between runs here (thumbnail and decode timing), so the
    # tolerance is wide; it catches a doubled buffer, not a few MB.
    common = [("uncaused", "zero", None), ("peak_rss_mb", "regress_env", 0.35)]
    # Scroll benches follow a wall-clock script: how much they load depends on the machine.
    scroll = [("uncaused", "zero", None), ("peak_rss_mb", "info", None)]
    idle = [("window.frames", "zero", None), *common]
    case(
        "cold-start",
        [str(p["jpeg24"])],
        env={"SPECULAR_BENCH_STARTUP": "1"},
        gates=[
            ("pixels_in_first_present", "true", None),
            ("bytes_read", "info", None),
            ("read_syscall_bytes", "info", None),
            ("startup.first_present_ms", "info", None),
            ("startup.pixels_ms", "info", None),
            *common,
        ],
        inputs=[p["jpeg24"]],
        what="cold start to window < 150 ms: the first present has the image",
    )
    case(
        "next-image",
        [str(p["next"] / "next-0.jpg")],
        "next",
        gates=[("frames_to_sharp", "max", 1), ("next_sharp_ms", "info", None), *idle],
        inputs=[p["next"]],
        what="next image (prefetched) < 16 ms: sharp in the first frame after the key",
    )
    case(
        "loupe-move",
        [str(p["jpeg24"])],
        "loupe",
        gates=[
            ("loupe_scene_redraws", "zero", None),
            ("loupe_jobs_cancelled", "zero", None),
            ("loupe_frames", "info", None),
            ("loupe_jobs_started", "info", None),
            *idle,
        ],
        inputs=[p["jpeg24"]],
        what="loupe follows the pointer without redrawing the image under it; idle once still",
    )
    case(
        "tiff-100k-first-view",
        [str(p["tiff100k"])],
        "idle",
        gates=[
            ("settle_timeout", "false", None),
            ("bytes_read", "info", None),
            ("read_syscall_bytes", "info", None),
            ("peak_rss_mb", "max", 1024),
            ("settle_ms", "info", None),
            *idle,
        ],
        inputs=[p["tiff100k"]],
        what="100k x 100k TIFF first view < 300 ms, bounded memory",
    )
    case(
        "ct-512-first-slice",
        [str(p["ct"] / "slice-0000.dcm")],
        "idle",
        gates=[
            ("settle_timeout", "false", None),
            ("bytes_read", "info", None),
            ("read_syscall_bytes", "info", None),
            ("settle_ms", "info", None),
            *idle,
        ],
        inputs=[p["ct"]],
        what="512-slice CT series to first slice < 500 ms",
    )
    case(
        "mpr-first-view",
        [str(p["ct"] / "slice-0000.dcm")],
        "mpr",
        gates=[
            ("settle_timeout", "false", None),
            ("mpr_first_view_ms", "info", None),
            ("bytes_read", "info", None),
            ("read_syscall_bytes", "info", None),
            *idle,
        ],
        inputs=[p["ct"]],
        timeout=300,
        what="512-slice CT volume ready < 3 s; MPR first view",
    )
    case(
        "vr-orbit",
        [str(p["ct"] / "slice-0000.dcm")],
        "orbit",
        env={"SPECULAR_GATE_STEPS": "20"},
        gates=[
            # One frame per key step (steps 50 ms apart may share one), plus the frame in
            # flight when the first step goes.
            ("orbit_frames", "max", 21),
            ("orbit_frames_to_rest", "regress", 0.25),
            ("orbit_frame_cpu_max_ms", "info", None),
            ("input_latency_ms.p50", "info", None),
            *idle,
        ],
        timeout=300,
        what="volume-rendering orbit: one frame per step, nothing after it rests",
    )
    case(
        "map-frame",
        [str(p["geojson"])],
        "idle",
        gates=[("settle_timeout", "false", None), ("settle_ms", "info", None), *idle],
        inputs=[p["geojson"]],
        what="a map of 50,000 points, then no frame while still",
    )
    case(
        "gallery-scroll",
        ["--screenshot", "{tmp}/gallery.png", "--show", "gallery-scroll-bench", str(p["gallery"])],
        env={"SPECULAR_SCROLL_BENCH_HZ": "60", "SPECULAR_THUMB_CACHE": "{tmp}/thumbs"},
        gates=[
            ("scroll.engine.failed", "zero", None),
            ("scroll.phases.scroll.p99", "info", None),
            ("scroll.phases.scroll.missed", "info", None),
            *scroll,
        ],
        timeout=300,
        what="gallery scroll at the display refresh (2,000 files)",
    )
    case(
        "library-scroll-100k",
        ["--library", "--screenshot", "{tmp}/library.png", "--show", "library-scroll-bench"],
        env={
            "SPECULAR_SCROLL_BENCH_HZ": "60",
            "SPECULAR_THUMB_CACHE": "{tmp}/thumbs",
            "SPECULAR_LIBRARY_DB": "{tmp}/library.sqlite3",
            "SPECULAR_GATE_COPY_DB": str(p["lib100k_db"]),
        },
        gates=[
            ("scroll.items", "min", 100_000),
            ("scroll.engine.failed", "zero", None),
            ("scroll.phases.scroll.p99", "info", None),
            ("scroll.phases.scroll.missed", "info", None),
            *scroll,
        ],
        timeout=600,
        what="Library grid scroll, 100k items, 60 fps",
    )
    case("idle-still", [str(p["jpeg24"])], "idle", gates=idle, what="idle CPU/GPU ~0 %: no repaint")
    case("idle-press", [str(p["jpeg24"])], "press", gates=idle, what="a button held still draws nothing")
    case(
        "idle-hidden-walk",
        ["--library"],
        "walk",
        env={
            # The 100k-file folder: indexing it takes minutes, so the walk runs all through.
            "SPECULAR_GATE_WALK": str(p["lib100k"]),
            "SPECULAR_GATE_IMAGE": str(p["jpeg24"]),
            "SPECULAR_LIBRARY_DB": "{tmp}/library.sqlite3",
            "SPECULAR_THUMB_CACHE": "{tmp}/thumbs",
            "SPECULAR_GATE_SECS": "6",
        },
        # The background-work chip in the viewer may show the walk's count, once a second.
        gates=[
            ("walk_started", "true", None),
            ("walk_busy_at_end", "true", None),
            ("window.other_frames", "zero", None),
            ("window.causes.work", "max", 7),
            ("uncaused", "zero", None),
            ("peak_rss_mb", "regress_env", 0.35),
        ],
        timeout=300,
        what="a Library walk out of sight draws nothing",
    )
    case(
        "idle-video-paused",
        [str(p["video"])],
        "video-paused",
        gates=[("was_playing", "true", None), *idle],
        what="a paused video draws nothing",
    )
    case(
        "remote-state",
        [str(p["jpeg24"])],
        "idle",
        env={"SPECULAR_GATE_SECS": "4"},
        gates=[
            ("remote.ok", "true", None),
            ("causes.remote", "none", None),
            ("remote.round_trip_ms.p50", "info", None),
            *idle,
        ],
        what="remote control: state answered, no frame drawn for it",
    )
    return c


# ----- running ----------------------------------------------------------------------------


def get(d, key):
    for k in key.split("."):
        if not isinstance(d, dict) or k not in d:
            return None
        d = d[k]
    return d


def run_case(sess, app_bin: Path, case: dict, work: Path, log_dir: Path) -> dict:
    tmp = work / "app-runs" / case["name"]
    shutil.rmtree(tmp, ignore_errors=True)
    tmp.mkdir(parents=True)
    report = tmp / "gate.json"
    sub = lambda v: v.replace("{tmp}", str(tmp))  # noqa: E731
    env = sess.app_env(SPECULAR_GATE_REPORT=str(report), SPECULAR_LOG_FILE=str(log_dir / f"{case['name']}.log"))
    env.update({k: sub(v) for k, v in case["env"].items()})
    # A run that may change a catalogue works on a copy of the prepared one.
    src = env.pop("SPECULAR_GATE_COPY_DB", None)
    if src and Path(src).exists():
        shutil.copy(src, env["SPECULAR_LIBRARY_DB"])
    args = [sub(a) for a in case["args"]]
    files = tree_files(case["inputs"])
    stage1.evict(files)
    cmd = [str(app_bin), "--new-window", "--size", "1920x1080", *args]
    t0 = time.time()
    proc = subprocess.Popen(cmd, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    remote = None
    if case["name"] == "remote-state":
        remote = remote_probe(app_bin, env, report, proc)
    try:
        out, err = proc.communicate(timeout=case["timeout"])
    except subprocess.TimeoutExpired:
        proc.kill()
        out, err = proc.communicate()
        return {"error": f"timed out after {case['timeout']} s", "stderr": err[-1500:]}
    res = {}
    if report.exists():
        try:
            res = json.loads(report.read_text())
        except json.JSONDecodeError as e:
            res = {"error": f"bad report: {e}"}
    else:
        res = {"error": f"no report (exit {proc.returncode})", "stderr": err[-1500:]}
    res["wall_ms"] = (time.time() - t0) * 1000
    w = res.get("window")
    if isinstance(w, dict):
        w["other_frames"] = w.get("frames", 0) - w.get("causes", {}).get("work", 0)
    res["exit"] = proc.returncode
    if files:
        res["bytes_read"] = resident_bytes(files)
        res["input_bytes"] = sum(os.stat(f).st_size for f in {os.stat(f).st_ino: f for f in files}.values())
    for line in out.splitlines():
        if line.startswith('{"startup"'):
            st = json.loads(line)["startup"]
            res["startup"] = st
            res["pixels_in_first_present"] = st.get("pixels_ms") is not None and st.get("pixels_ms") == st.get(
                "first_present_ms"
            )
    for line in err.splitlines():
        if line.startswith("scroll-bench "):
            try:
                res["scroll"] = json.loads(line[len("scroll-bench ") :])
            except json.JSONDecodeError:
                pass
    if remote is not None:
        res["remote"] = remote
    return res


def remote_probe(app_bin: Path, env: dict, report: Path, proc) -> dict:
    """Asks the running viewer for its state 20 times during its counting window."""
    # The window starts once the image settled; wait for the socket, then a little more.
    rts = []
    ok = False
    deadline = time.time() + 120
    while time.time() < deadline and proc.poll() is None:
        r = subprocess.run([str(app_bin), "--remote", "hello"], env=env, capture_output=True, text=True)
        if r.returncode == 0:
            break
        time.sleep(0.2)
    for _ in range(20):
        if proc.poll() is not None:
            break
        t = time.perf_counter()
        r = subprocess.run([str(app_bin), "--remote", "state"], env=env, capture_output=True, text=True)
        rts.append((time.perf_counter() - t) * 1000)
        ok = ok or (r.returncode == 0 and "{" in r.stdout)
        time.sleep(0.05)
    rts.sort()
    return {
        "ok": ok,
        "n": len(rts),
        "round_trip_ms": {"p50": statistics.median(rts) if rts else None, "max": rts[-1] if rts else None},
        "note": "includes starting the client process",
    }


def index_library(sess, app_bin: Path, p: dict, work: Path, log_dir: Path) -> None:
    """Indexes the 100k-file folder into the catalogue the Library scroll case copies (once;
    kept while the folder and the catalogue are there)."""
    db = p["lib100k_db"]
    if db.exists():
        return
    print("indexing the 100k-file Library catalogue (once) ...", flush=True)
    tmp_db = db.with_suffix(".part")
    for f in tmp_db.parent.glob(tmp_db.name + "*"):
        f.unlink()
    report = work / "lib100k-index.json"
    env = sess.app_env(
        SPECULAR_GATE_REPORT=str(report),
        SPECULAR_GATE_SCRIPT="index",
        SPECULAR_GATE_SECS="0",
        SPECULAR_GATE_WALK=str(p["lib100k"]),
        SPECULAR_LIBRARY_DB=str(tmp_db),
        SPECULAR_THUMB_CACHE=str(work / "lib100k-thumbs"),
        SPECULAR_LOG_FILE=str(log_dir / "lib100k-index.log"),
    )
    t0 = time.time()
    subprocess.run([str(app_bin), "--new-window", "--library"], env=env, capture_output=True, timeout=2400)
    shutil.rmtree(work / "lib100k-thumbs", ignore_errors=True)
    r = json.loads(report.read_text()) if report.exists() else {}
    print(f"  indexed in {time.time() - t0:.0f} s: {r.get('index_finished')}", flush=True)
    if r.get("index_finished"):
        tmp_db.rename(db)


def warm_up(sess, app_bin: Path, p: dict, work: Path, log_dir: Path) -> None:
    """One run that waits for the viewer's idle-time calibration of this machine (saved in the
    session's cache folder, which every case shares), so no case counts frames while it runs
    or as it ends."""
    print("calibrating the viewer's paths (once per session) ...", flush=True)
    report = work / "warm-up.json"
    env = sess.app_env(
        SPECULAR_GATE_REPORT=str(report),
        SPECULAR_GATE_SCRIPT="idle",
        SPECULAR_GATE_SECS="0",
        SPECULAR_GATE_SETTLE_MAX="900",
        SPECULAR_LOG_FILE=str(log_dir / "warm-up.log"),
    )
    subprocess.run([str(app_bin), "--new-window", str(p["jpeg24"])], env=env, capture_output=True, timeout=1200)
    r = json.loads(report.read_text()) if report.exists() else {}
    print(f"  calibrated after {r.get('settle_ms', 0) / 1000:.0f} s (timed out: {r.get('settle_timeout')})", flush=True)


def environment(app_bin: Path, profile: str) -> dict:
    env = {"profile": profile}
    try:
        r = subprocess.run([str(app_bin), "--hw-report"], capture_output=True, text=True, timeout=60)
        for line in r.stdout.splitlines():
            if line.strip().startswith("Adapter"):
                env["gpu"] = line.strip().removeprefix("Adapter").strip()[:120]
                break
    except (OSError, subprocess.TimeoutExpired):
        pass
    env["cpus"] = os.cpu_count()
    return env


def same_env(a: dict, b: dict) -> bool:
    return all(a.get(k) == b.get(k) for k in ("profile", "gpu"))


def check(name: str, res: dict, base: dict | None, env_match: bool, gates: list) -> list:
    """(metric, value, budget text, status) rows for one case."""
    rows = []
    if "error" in res:
        return [("run", res.get("error"), "", "FAIL")]
    for metric, kind, budget in gates:
        v = get(res, metric)
        b = get(base, metric) if base else None
        status = "ok"
        btxt = ""
        if kind == "zero":
            btxt = "0"
            status = "ok" if v == 0 else "FAIL"
        elif kind == "none":
            btxt = "0"
            status = "ok" if not v else "FAIL"
        elif kind == "true":
            btxt = "true"
            status = "ok" if v is True else "FAIL"
        elif kind == "false":
            btxt = "false"
            status = "ok" if v is False else "FAIL"
        elif kind == "min":
            btxt = f">= {budget}"
            status = "ok" if v is not None and v >= budget else "FAIL"
        elif kind == "max":
            btxt = f"<= {budget}"
            status = "ok" if v is not None and v <= budget else "FAIL"
        elif kind in ("regress", "regress_env"):
            if b is None or (kind == "regress_env" and not env_match):
                btxt = "baseline" if b is None else "other env"
                status = "info" if v is not None else "FAIL"
            else:
                limit = b * (1 + budget) + (1 if isinstance(b, int) else 0)
                btxt = f"<= {limit:,.1f} (base {b:,.1f} +{budget * 100:g} %)"
                status = "ok" if v is not None and v <= limit else "FAIL"
        else:
            status = "info"
        if isinstance(v, float):
            v = round(v, 2)
        rows.append((metric, v, btxt, status))
    return rows


def main(args, target_dir: Path) -> int:
    profile = args.profile
    pdir = "debug" if profile == "dev" else profile
    app_bin = Path(args.app_bin) if args.app_bin else target_dir / pdir / "specular"
    bench_bin = Path(args.bin) if args.bin else target_dir / pdir / "specular-bench"
    if not args.no_build:
        for cmd in (
            ["cargo", "build", "--profile", profile, "--locked", "-p", "specular", "--bin", "specular"],
            ["cargo", "build", "--profile", profile, "--locked", "-p", "specular-bench", "--features", "gate"],
        ):
            if args.cargo_jobs:
                cmd += ["-j", str(args.cargo_jobs)]
            print("+", " ".join(cmd), flush=True)
            subprocess.run(cmd, check=True, cwd=ROOT)
    for tool in ("sway", "fincore"):
        if not shutil.which(tool):
            sys.exit(f"the app suite needs {tool}")
    work = target_dir / "perfgate"
    paths = prepare(bench_bin, work)
    all_cases = cases(paths)
    if args.only:
        all_cases = [c for c in all_cases if any(s in c["name"] for s in args.only)]
    load0 = os.getloadavg()[0]
    env = environment(app_bin, profile)
    log_dir = work / "app-logs"
    log_dir.mkdir(parents=True, exist_ok=True)
    results = {}
    # A short root: the compositor's sockets must fit in sun_path (108 bytes, 88 here).
    with stage1.Session(str(target_dir / "pg")) as sess:
        if any(c["name"] == "library-scroll-100k" for c in all_cases):
            index_library(sess, app_bin, paths, work, log_dir)
        warm_up(sess, app_bin, paths, work, log_dir)
        for c in all_cases:
            print(f"running {c['name']} ...", flush=True)
            results[c["name"]] = run_case(sess, app_bin, c, work, log_dir)
    load1 = os.getloadavg()[0]
    under_load = max(load0, load1) > (os.cpu_count() or 4) / 4
    env["load_average"] = [round(load0, 1), round(load1, 1)]
    cur = {"schema": SCHEMA, "env": env, "under_load": under_load, "cases": results}
    out = Path(args.out) if args.out else work / "app-results.json"
    out.write_text(json.dumps(cur, indent=2) + "\n")
    print(f"results: {out}")

    baseline = Path(args.baseline_app) if args.baseline_app else BASELINE
    base = json.loads(baseline.read_text()) if baseline.exists() else None
    env_match = bool(base) and same_env(env, base.get("env", {}))
    ok = True
    md = [f"### App gate\n\nLoad average {load0:.0f} → {load1:.0f}"
          f"{' (wall-clock rows are under load)' if under_load else ''}.\n\n"
          "| case | metric | value | budget | status |\n|---|---|---|---|---|\n"]
    print(f"{'case':22} {'metric':32} {'value':>14}  {'budget':34} status")
    for c in all_cases:
        res = results[c["name"]]
        rows = check(c["name"], res, (base or {}).get("cases", {}).get(c["name"]), env_match, c["gates"])
        for metric, v, btxt, status in rows:
            if status == "FAIL":
                ok = False
            tag = " (under load)" if status == "info" and metric.endswith("ms") and under_load else ""
            print(f"{c['name']:22} {metric:32} {str(v):>14}  {btxt:34} {status}{tag}")
            md.append(f"| {c['name']} | {metric} | {v} | {btxt} | {status}{tag} |\n")
    if args.markdown:
        with open(args.markdown, "a") as f:
            f.write("".join(md))
    if args.bless:
        blessed = {"schema": SCHEMA, "env": env, "cases": {}}
        for n, r in results.items():
            if "error" in r:
                print(f"not blessing {n}: {r['error']}")
                continue
            keep = {}
            for c in all_cases:
                if c["name"] == n:
                    for metric, kind, _ in c["gates"]:
                        if kind in ("regress", "regress_env"):
                            v = get(r, metric)
                            if v is not None:
                                d = keep
                                parts = metric.split(".")
                                for k in parts[:-1]:
                                    d = d.setdefault(k, {})
                                d[parts[-1]] = v
            blessed["cases"][n] = keep
        if base and args.only:
            base["cases"].update(blessed["cases"])
            base["env"] = env
            blessed = base
        baseline.write_text(json.dumps(blessed, indent=2) + "\n")
        print(f"baseline written: {baseline}")
        return 0
    return 0 if ok else 1
