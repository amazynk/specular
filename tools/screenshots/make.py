#!/usr/bin/env python3
"""Renders the README screenshots and the Linux store screenshots from the real app.

    python3 -I tools/screenshots/make.py [--specular target/release/specular] [--work DIR]
                                         [--photos DIR]

Downloads fourteen photos from Unsplash (Unsplash License) and haemoglobin from the RCSB Protein
Data Bank (PDB 4HHB, CC0) into the work folder, writes the photos' camera data, renders each view
with the app's hidden `--screenshot` mode and writes:

- `.github/assets/screenshots/*.webp`: the README's hero and feature grid, with rounded corners,
  a hairline edge and a soft shadow on transparency;
- `packaging/linux/screenshots/*.jpg`: the AppStream screenshots, unframed.

Run it on a 2× (Retina or HiDPI) display: the window sizes below are logical pixels. The gallery
shot shows the photo folder's path in its breadcrumb, so `--photos` takes a folder with a short
path; the committed shots used `--photos "/Users/Shared/Field notes"`. The app runs in portable
mode from a copy in the work folder, so nothing is written to your own settings or caches.

Needs `magick` (ImageMagick 7), `exiftool` and `curl`. Credits: `.github/assets/CREDITS.md`.
"""

import argparse
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

# File stem, Unsplash photo id, title, photographer, and the camera data written into the file:
# camera, lens, exposure, aperture, ISO, focal length.
PHOTOS = [
    ("DSC_2207", "photo-1517928260182-5688aead3066", "Aurora over a glacial lagoon", "v2osk", ("Nikon D800", "16.0-35.0 mm f/4.0", "30 s", "f/4", 1000, "16 mm")),
    ("IMG_7731", "photo-1511300636408-a63a89df3482", "Vestrahorn at dusk", "lucamicheli", ("Canon EOS 5D Mark IV", "EF11-24mm f/4L USM", "1/25 s", "f/8", 100, "12 mm")),
    ("DJI_0412", "photo-1619266465172-02a857c3556d", "Fagradalsfjall lava flow", "tetiana_gry", ("DJI Mavic Air 2", "24.0 mm f/2.8", "1/13 s", "f/2.8", 550, "24 mm")),
    ("DJI_0081", "photo-1784143366204-6521d6c2bb63", "Glacial river from above", "tomqtom", ("DJI Mavic 3 (Hasselblad L2D-20c)", "24.0 mm f/2.8", "1/120 s", "f/4", 100, "24 mm")),
    ("seljalandsfoss", "photo-1501908734255-16579c18c25f", "Behind the waterfall", "sortino", None),
    ("DSC04477", "photo-1547626740-02cb6aed9ef8", "Above the river valley", "sortino", ("Sony α7R II", "FE 16-35mm F4 ZA OSS", "1/160 s", "f/4", 320, "33 mm")),
    ("DSC00219", "photo-1500043357865-c6b8827edf10", "Black sand at Vík", "adamjang", ("Sony α7 II", "FE 55mm F1.8 ZA", "1/250 s", "f/5", 100, "55 mm")),
    ("DSC_4182", "photo-1781276798473-258ee0e096d8", "Puffin in flight", "coombez", ("Nikon Z 9", "NIKKOR Z 180-600mm f/5.6-6.3 VR", "1/1000 s", "f/6.3", 800, "520 mm")),
    ("DSC09113", "photo-1759845407006-dc69e591f506", "Hummingbird, 1/1600 s", "patrickbigelow_zoom_in", ("Sony α7R V", "FE 200-600mm F5.6-6.3 G OSS", "1/1600 s", "f/6.3", 2000, "600 mm")),
    ("IMG_0931", "photo-1573455494060-c5595004fb6c", "Lantern alley, Tokyo", "dnevozhai", ("Canon EOS 6D", "EF16-35mm f/4L IS USM", "1/25 s", "f/4", 400, "16 mm")),
    ("DSC01842", "photo-1613487957484-32c977a8bd62", "Akihabara signs", "patkrupa", ("Sony α7R III", "FE 35mm F2.8 ZA", "3.2 s", "f/20", 50, "35 mm")),
    ("golden_hour", "photo-1765813102971-47144ca56d03", "Golden hour portrait", "alef_visuals", None),
    ("IMG_2016", "photo-1531048468035-8305c2a7fe73", "Dunes at sunset", "markkuiper", ("Apple iPhone 6s", "iPhone 6s back camera 4.15mm f/2.2", "1/561 s", "f/2.2", 25, "29 mm")),
    ("IMG_5583", "photo-1531366936337-7c912a4589a7", "Aurora over the peaks", "lightscape", ("Canon EOS 6D", "12-24mm", "25 s", "f/5.6", 6400, "12 mm")),
]
MAKERS = {"Nikon": "NIKON CORPORATION", "Sony": "SONY"}
PDB = "https://files.rcsb.org/download/4HHB.pdb"

# Name, logical window size, arguments, and what to open (a photo stem, "PHOTOS" for the folder,
# or "MOLECULE").
SETTINGS = ["--set", "general.theme=dark", "--set", "general.tips=false", "--set", "cull.auto_advance=false"]
SHOTS = [
    ("hero", "1200x800", ["--show", "cull,cull-pick,cull-rate-5"], "IMG_7731"),
    ("gallery", "1000x625", ["--set", "gallery.thumbnail_size=200"], "PHOTOS"),
    ("zoom", "1000x625", ["--zoom", "100", "--show", "navigator"], "DSC_4182"),
    ("develop", "1200x750", ["--show", "develop"], "DSC04477"),
    ("palette", "1000x625", ["--show", "palette", "--query", "export"], "DJI_0412"),
    ("molecule", "1000x625", ["--zoom", "170"], "MOLECULE"),
    ("cheatsheet", "1000x625", ["--show", "cheatsheet"], "IMG_5583"),
]
# README images: width in pixels, corner radius and ImageMagick shadow.
HERO = (2400, 24, "100x36+0+28")
GRID = (1600, 18, "70x18+0+12")
# AppStream screenshots: shot and file name.
APPSTREAM = [("hero", "cull"), ("gallery", "gallery"), ("develop", "develop"), ("molecule", "molecule")]


def run(*args):
    subprocess.run([str(a) for a in args], check=True)


def fetch(url, dest):
    if not dest.exists():
        run("curl", "-sSfL", "-o", dest, url)


def photos(folder, cache):
    folder.mkdir(parents=True, exist_ok=True)
    cache.mkdir(parents=True, exist_ok=True)
    for i, (stem, uid, title, credit, cam) in enumerate(PHOTOS):
        raw = cache / f"{stem}.jpg"
        fetch(f"https://images.unsplash.com/{uid}?w=3200&q=90&fm=jpg", raw)
        out = folder / f"{stem}.jpg"
        shutil.copyfile(raw, out)
        tags = [
            f"-ImageDescription={title}",
            f"-XMP-dc:Title={title}",
            f"-Artist=@{credit} on Unsplash",
            f"-DateTimeOriginal=2026:09:{11 + i // 4:02d} {18 + i % 4}:{i * 7 % 60:02d}:00",
        ]
        if cam:
            model, lens, exposure, aperture, iso, focal = cam
            make = MAKERS.get(model.split()[0], model.split()[0])
            tags += [
                f"-Make={make}",
                f"-Model={model}",
                f"-LensModel={lens}",
                f"-ExposureTime={exposure.removesuffix(' s')}",
                f"-FNumber={aperture.removeprefix('f/')}",
                f"-ISO={iso}",
                f"-FocalLength={focal.removesuffix(' mm')}",
            ]
        run("exiftool", "-q", "-overwrite_original", *tags, out)


def app_copy(specular, work):
    """The app and its decode host in portable mode, so settings and caches stay in `work`."""
    app = work / "app"
    app.mkdir(parents=True, exist_ok=True)
    for name in (specular.name, "specular-decode-host"):
        src = specular.parent / name
        if src.exists():
            shutil.copy2(src, app / name)
    (app / "portable").touch()
    return app / specular.name


def frame(src, dest, width, radius, shadow):
    with tempfile.TemporaryDirectory() as d:
        small = Path(d) / "small.png"
        run("magick", src, "-resize", f"{width}x", small)
        w, h = subprocess.run(
            ["magick", "identify", "-format", "%w %h", str(small)], capture_output=True, text=True, check=True
        ).stdout.split()
        w, h = int(w), int(h)
        run(
            "magick", small,
            "(", "-size", f"{w}x{h}", "xc:none", "-fill", "white",
            "-draw", f"roundrectangle 0,0 {w - 1},{h - 1} {radius},{radius}", ")",
            "-alpha", "set", "-compose", "DstIn", "-composite",
            "(", "-size", f"{w}x{h}", "xc:none", "-fill", "none", "-stroke", "rgba(255,255,255,0.14)",
            "-strokewidth", "2", "-draw", f"roundrectangle 1,1 {w - 2},{h - 2} {radius},{radius}", ")",
            "-compose", "Over", "-composite",
            "(", "+clone", "-background", "rgba(0,0,0,0.5)", "-shadow", shadow, ")",
            "+swap", "-background", "none", "-compose", "Over", "-layers", "merge", "+repage",
            "-quality", "88", "-define", "webp:alpha-quality=100", "-define", "webp:method=6", dest,
        )


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--specular", type=Path, default=ROOT / "target/release/specular")
    ap.add_argument("--work", type=Path, default=Path(tempfile.gettempdir()) / "specular-screenshots")
    ap.add_argument("--photos", type=Path, help="folder for the photos (default: WORK/Field notes)")
    a = ap.parse_args()
    for tool in ("magick", "exiftool", "curl"):
        if not shutil.which(tool):
            sys.exit(f"{tool} not found")
    if not a.specular.exists():
        sys.exit(f"{a.specular} not found: build it first (cargo build --release -p specular)")
    work = a.work.resolve()
    folder = (a.photos or work / "Field notes").resolve()
    photos(folder, work / "downloads")
    mol = work / "molecule"
    mol.mkdir(parents=True, exist_ok=True)
    fetch(PDB, mol / "4HHB.pdb")
    app = app_copy(a.specular.resolve(), work)
    raw = work / "raw"
    raw.mkdir(exist_ok=True)
    env = {k: v for k, v in os.environ.items() if not k.startswith("SPECULAR_")}
    for name, size, args, what in SHOTS:
        target = {"PHOTOS": folder, "MOLECULE": mol / "4HHB.pdb"}.get(what, folder / f"{what}.jpg")
        out = raw / f"{name}.png"
        cmd = [app, "--screenshot", out, "--size", size, *SETTINGS, *args, target]
        subprocess.run([str(c) for c in cmd], check=True, env=env, cwd=work)
    shots = ROOT / ".github/assets/screenshots"
    for name, *_ in SHOTS:
        frame(raw / f"{name}.png", shots / f"{name}.webp", *(HERO if name == "hero" else GRID))
    store = ROOT / "packaging/linux/screenshots"
    store.mkdir(parents=True, exist_ok=True)
    for name, file in APPSTREAM:
        run("magick", raw / f"{name}.png", "-resize", "1600x", "-strip", "-quality", "88", store / f"{file}.jpg")
    print(f"wrote {shots.relative_to(ROOT)}/*.webp and {store.relative_to(ROOT)}/*.jpg")


if __name__ == "__main__":
    main()
