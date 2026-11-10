#!/usr/bin/env python3
"""Compare release binaries (plain vs PGO vs PGO+BOLT) on decode-heavy workloads.

    tools/release/pgo-bench.py --out DIR [--runs 11] [--no-open] [--only S,S] \\
        --variant plain D/plain/specular D/plain/specular-bench \\
        --variant pgo   D/specular       D/specular-bench \\
        --variant bolt  D/specular.bolt  D/specular-bench.bolt

The binaries come from `tools/release/pgo.sh --bolt --baseline` (in $OUT/dist). Benchmarks:

  gate:CASE    `specular-bench gate run CASE FILE`: one single-threaded decode per process, the
               perf-gate cases and inputs (tools/bench/perf-gate.py);
  thumb:FMT    `specular --thumbnail FILE OUT 256` (desktop thumbnailer: decode at the coarsest
               useful scale, orient, downscale, PNG encode);
  gallery      `specular --thumbnail-cache DIR` on a cold cache: gallery thumbnailing of a folder
               of mixed formats, all cores;
  open:FMT     `specular --size 1920x1080 --bench-open FILE` in a private headless sway (decode,
               GPU upload, first frame); skipped with --no-open or without sway.

The inputs are generated into DIR/inputs (the perf-gate generator, plus magick/cjpeg/cwebp/
heif-enc/ffmpeg at seeds the PGO training corpus does not use), so the binaries are never measured
on the files they were trained on. Every run is wrapped in `perf stat -e instructions:u,cycles:u`;
variants alternate order every round so that load drift hits them alike. The report gives the
median wall time with its min-max spread, and median user-space instructions and cycles, each
relative to the first variant. Instructions are the stable number on a loaded machine.

App runs get a private HOME and XDG_* directories under DIR, no DISPLAY and no session bus.
Standard library only; needs perf (perf_event_paranoid <= 2 for :u counters).
"""

import argparse
import json
import os
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def have(tool):
    return shutil.which(tool) is not None


def sh(cmd, **kw):
    subprocess.run(cmd, check=True, **kw)


def make_inputs(d: Path, gate_bin: str) -> dict:
    """Generates the inputs once; returns {name: path} for the thumbnail and open benchmarks."""
    gate = d / "gate"
    if not (gate / "jpeg-420.jpg").exists():
        sh([gate_bin, "gate", "gen", str(gate)], stdout=subprocess.DEVNULL)
    m = d / "measure"
    files = {}
    if not (m / ".done").exists():
        shutil.rmtree(m, ignore_errors=True)
        m.mkdir(parents=True)
        src = d / "src"
        src.mkdir(exist_ok=True)
        big, mid, gfx = src / "24mp.ppm", src / "6mp.png", src / "gfx.png"
        sh(["magick", "-seed", "7", "-size", "6000x4000", "plasma:fractal", "-blur", "0x0.8",
            "-depth", "8", str(big)])
        sh(["magick", str(big), "-resize", "3000x2000", str(mid)])
        sh(["magick", "-size", "2400x1600", "xc:none", "-fill", "#a03050c0", "-draw",
            "rectangle 200,150 1500,1200", "-fill", "#30a0e0", "-draw", "circle 1700,800 1700,1400",
            str(gfx)])
        if have("cjpeg"):
            with open(m / "jpeg-24mp-baseline.jpg", "wb") as o:
                sh(["cjpeg", "-quality", "90", str(big)], stdout=o)
            with open(m / "jpeg-24mp-progressive.jpg", "wb") as o:
                sh(["cjpeg", "-quality", "90", "-progressive", str(big)], stdout=o)
        sh(["magick", str(mid), str(m / "png-6mp.png")])
        if have("cwebp"):
            sh(["cwebp", "-quiet", "-q", "80", str(mid), "-o", str(m / "webp-6mp-lossy.webp")])
            sh(["cwebp", "-quiet", "-lossless", str(gfx), "-o", str(m / "webp-lossless.webp")])
        if have("heif-enc"):
            sh(["heif-enc", "--avif", "-q", "60", "-o", str(m / "avif-6mp.avif"), str(mid)],
               stdout=subprocess.DEVNULL)
            sh(["heif-enc", "-q", "60", "-o", str(m / "heic-6mp.heic"), str(mid)],
               stdout=subprocess.DEVNULL)
        if have("ffmpeg"):
            sh(["ffmpeg", "-y", "-loglevel", "error", "-i", str(mid), "-c:v", "libjxl", "-distance",
                "1.0", "-frames:v", "1", "-update", "1", str(m / "jxl-6mp-vardct.jxl")])
        sh(["magick", str(mid), "-compress", "LZW", str(m / "tiff-6mp-lzw.tif")])
        shutil.rmtree(src)
        (m / ".done").touch()
    for f in sorted(m.iterdir()):
        if not f.name.startswith("."):
            files[f.stem] = f
    # Gallery: every measured file three times (distinct names, so distinct cache entries).
    g = d / "gallery"
    if not g.exists():
        g.mkdir()
        for name, f in files.items():
            for i in range(3):
                os.link(f, g / f"{name}-{i}{f.suffix}")
    return files


def gate_cases(gate_bin: str, gate_dir: Path):
    out = subprocess.run([gate_bin, "gate", "list"], check=True, capture_output=True, text=True).stdout
    for line in out.splitlines():
        case, kind, f = line.split("\t")
        yield case, str(gate_dir / f if kind == "generated" else ROOT / f)


def private_env(d: Path) -> dict:
    home = d / "home"
    for sub in ("config", "cache", "data", "state"):
        (home / sub).mkdir(parents=True, exist_ok=True)
    xdg = d / "xdg"
    xdg.mkdir(exist_ok=True)
    xdg.chmod(0o700)
    env = {k: v for k, v in os.environ.items()
           if k not in ("DISPLAY", "WAYLAND_DISPLAY", "DBUS_SESSION_BUS_ADDRESS", "SPECULAR_LOG_FILE")}
    env.update(HOME=str(home), XDG_CONFIG_HOME=str(home / "config"), XDG_CACHE_HOME=str(home / "cache"),
               XDG_DATA_HOME=str(home / "data"), XDG_STATE_HOME=str(home / "state"),
               XDG_RUNTIME_DIR=str(xdg), TMPDIR=str(d / "tmp"))
    (d / "tmp").mkdir(exist_ok=True)
    return env


class Sway:
    def __init__(self, d: Path, env: dict):
        self.proc = None
        conf = d / "sway.conf"
        conf.write_text("xwayland disable\noutput HEADLESS-1 resolution 2560x1440\n")
        xdg = Path(env["XDG_RUNTIME_DIR"])
        for s in xdg.glob("wayland-*"):
            s.unlink()
        e = dict(env, WLR_BACKENDS="headless", WLR_LIBINPUT_NO_DEVICES="1")
        self.log = open(d / "sway.log", "w")
        self.proc = subprocess.Popen(["sway", "-c", str(conf)], env=e, stdout=self.log,
                                     stderr=subprocess.STDOUT)
        for _ in range(100):
            socks = [s for s in xdg.glob("wayland-[0-9]*") if s.is_socket()]
            if socks:
                self.display = socks[0].name
                return
            time.sleep(0.1)
        self.stop()
        raise RuntimeError(f"headless sway did not start (see {d / 'sway.log'})")

    def stop(self):
        if self.proc and self.proc.poll() is None:
            self.proc.terminate()
            try:
                self.proc.wait(10)
            except subprocess.TimeoutExpired:
                self.proc.kill()
                self.proc.wait()
        self.proc = None


def perf_run(argv, env, tmp: Path, before=None):
    """One run under perf stat; returns (wall_ms, instructions, cycles, stdout)."""
    if before:
        before()
    stat = tmp / "perf.csv"
    cmd = ["perf", "stat", "-x", ",", "-e", "instructions:u,cycles:u", "-o", str(stat), "--"] + argv
    t = time.perf_counter_ns()
    p = subprocess.run(cmd, env=env, capture_output=True, text=True, timeout=300)
    wall = (time.perf_counter_ns() - t) / 1e6
    if p.returncode != 0:
        raise RuntimeError(f"{' '.join(argv)} failed ({p.returncode}): {p.stderr[-500:]}")
    counts = {}
    for line in stat.read_text().splitlines():
        parts = line.split(",")
        if len(parts) > 2 and parts[0].replace(".", "").isdigit():
            counts[parts[2].split(":")[0]] = float(parts[0])
    return wall, counts.get("instructions"), counts.get("cycles"), p.stdout


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--out", required=True, type=Path)
    ap.add_argument("--runs", type=int, default=11)
    ap.add_argument("--no-open", action="store_true")
    ap.add_argument("--only", default="", help="comma-separated substrings of benchmark names")
    ap.add_argument("--variant", nargs=3, action="append", required=True,
                    metavar=("LABEL", "SPECULAR", "SPECULAR_BENCH"))
    a = ap.parse_args()
    d = a.out.resolve()
    d.mkdir(parents=True, exist_ok=True)
    variants = [(l, str(Path(s).resolve()), str(Path(b).resolve())) for l, s, b in a.variant]
    env = private_env(d)
    tmp = Path(tempfile.mkdtemp(dir=d / "tmp"))
    inputs = make_inputs(d / "inputs", variants[0][2])
    cache = Path(env["XDG_CACHE_HOME"]) / "specular"

    benches = []  # (name, kind, argv(specular, bench) -> list, before)
    for case, f in gate_cases(variants[0][2], d / "inputs" / "gate"):
        benches.append((f"gate:{case}", "cli", lambda s, b, c=case, f=f: [b, "gate", "run", c, f], None))
    for name, f in inputs.items():
        benches.append((f"thumb:{name}", "cli",
                        lambda s, b, f=f: [s, "--thumbnail", str(f), str(tmp / "t.png"), "256"], None))
    benches.append(("gallery", "cli",
                    lambda s, b: [s, "--thumbnail-cache", str(d / "inputs" / "gallery")],
                    lambda: shutil.rmtree(cache, ignore_errors=True)))
    if not a.no_open and have("sway"):
        for name, f in inputs.items():
            benches.append((f"open:{name}", "open",
                            lambda s, b, f=f: [s, "--size", "1920x1080", "--bench-open", str(f)], None))
    if a.only:
        keys = a.only.split(",")
        benches = [x for x in benches if any(k in x[0] for k in keys)]

    sway = None
    if any(k == "open" for _, k, _, _ in benches):
        sway = Sway(d, env)
    open_env = dict(env, WAYLAND_DISPLAY=sway.display) if sway else env
    data = {}  # name -> label -> list of (wall, instr, cycles)
    try:
        for rnd in range(a.runs):
            print(f"round {rnd + 1}/{a.runs}, load {os.getloadavg()[0]:.1f}", file=sys.stderr, flush=True)
            order = variants if rnd % 2 == 0 else variants[::-1]
            for name, kind, argv, before in benches:
                for label, s, b in order:
                    r = perf_run(argv(s, b), open_env if kind == "open" else env, tmp, before)
                    data.setdefault(name, {}).setdefault(label, []).append(r[:3])
    finally:
        if sway:
            sway.stop()
        shutil.rmtree(tmp, ignore_errors=True)

    (d / "results.json").write_text(json.dumps(
        {"runs": a.runs, "variants": [v[0] for v in variants], "data": data}, indent=1))
    labels = [v[0] for v in variants]
    base = labels[0]
    hdr = "| benchmark | " + " | ".join(
        [f"{base} wall ms (spread)", f"{base} Ginstr"]
        + [f"{l} wall / instr / cycles" for l in labels[1:]]) + " |"
    lines = [hdr, "|" + "---|" * (hdr.count("|") - 1)]
    for name, per in data.items():
        def med(label, i):
            return statistics.median(x[i] for x in per[label])

        def spread(label):
            w = [x[0] for x in per[label]]
            return (max(w) - min(w)) / statistics.median(w) * 100
        row = [name, f"{med(base, 0):.1f} (±{spread(base) / 2:.0f}%)", f"{med(base, 1) / 1e9:.3f}"]
        for l in labels[1:]:
            row.append(" / ".join(f"{(med(l, i) / med(base, i) - 1) * 100:+.1f}%" for i in range(3))
                       + f" (±{spread(l) / 2:.0f}%)")
        lines.append("| " + " | ".join(row) + " |")
    table = "\n".join(lines)
    (d / "results.md").write_text(table + "\n")
    print(table)


if __name__ == "__main__":
    main()
