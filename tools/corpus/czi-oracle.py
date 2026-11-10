#!/usr/bin/env python3
"""Expected values for Specular's Zeiss CZI reader (libCZI in the decode host), from oracles
independent of the host's build: pylibCZIrw (ZEISS's Python wrapper, its own libCZI build) for
planes, bounding boxes and dimensions; OpenSlide (its own CZI reader, JPEG XR and zstd decoders)
for brightfield slides at 8 bits; the metadata XML read directly for scaling and the channels'
names and display settings; libCZI's CZIcmd for attachments (its embedded CZI images then read
by pylibCZIrw).

pylibCZIrw 6.1 (and aicspylibczi 4.0) decode zstd1 "HiLo" sub-blocks differently from their
zstd0 twins of the same image and from OpenSlide 4.0: files with zstd1 sub-blocks get
OpenSlide's values only.

Usage: czi-oracle.py CORPUS_DIR [CZICMD]
Writes CORPUS_DIR/expected.tsv; read by crates/specular-decode-host/tests/czi.rs and
crates/specular-decode/tests/czi.rs. Run with `python -I` (the corpus is untrusted data).

Lines (tab separated):
  info    FILE  dims     C0:2,S0:2,...           (letter start:size, sorted)
  info    FILE  bbox     x,y,w,h                 (layer 0)
  scene   FILE  INDEX    x,y,w,h                 (layer 0)
  scaling FILE  x,y,z                            (metres per pixel, "-" when absent)
  channel FILE  INDEX    name|#RRGGBB or -|low|high
  plane   FILE  SCENE x y w h zoom COORDS PIXEL  W H SHA256
          (SCENE -1: no filter; COORDS "C=1,Z=2" or "-"; PIXEL libCZI's number)
  plane8  FILE  x y w h  SHA256                  (OpenSlide's RGB at 8 bits, zoom 1, raw x y)
  attach  FILE  NAME     enc SHA256  |  px W H PIXEL SHA256 MAXSIDE
"""
import hashlib
import os
import subprocess
import sys
import tempfile
import xml.etree.ElementTree as ET

import numpy as np
from pylibCZIrw import czi as pyczi

try:
    import openslide
except ImportError:  # OpenSlide's values are left out.
    openslide = None

PIXEL = {"Gray8": 0, "Gray16": 1, "Gray32Float": 2, "Bgr24": 3, "Bgr48": 4, "Bgr96Float": 8}
ASSOCIATED_MAX = 8192
OVERVIEW = 1024


def sha(a):
    return hashlib.sha256(np.ascontiguousarray(a).tobytes()).hexdigest()


def rect(r):
    return f"{r.x},{r.y},{r.w},{r.h}"


def read_plane(r, roi, zoom, plane, scene, pixel_name):
    a = r.read(roi=roi, plane=plane, scene=scene, zoom=zoom, pixel_type=pixel_name)
    return a.shape[1], a.shape[0], sha(a)


def xml_facts(raw):
    root = ET.fromstring(raw)
    md = root.find("Metadata")
    scaling = []
    for d in "XYZ":
        v = None
        if md is not None:
            for dist in md.findall("Scaling/Items/Distance"):
                if dist.get("Id") == d:
                    t = dist.findtext("Value")
                    if t:
                        v = float(t)
        scaling.append(v)
    channels = []
    if md is not None:
        for c in md.findall("DisplaySetting/Channels/Channel"):
            col = (c.findtext("Color") or "").strip()
            mode = (c.findtext("ColorMode") or "").strip()
            # ZEN colours are #AARRGGBB; ColorMode None means no tint.
            tint = (
                "#" + col[-6:].upper()
                if len(col) in (7, 9) and col.startswith("#") and mode != "None"
                else "-"
            )
            low = c.findtext("Low") or "0"
            high = c.findtext("High") or "1"
            channels.append((c.get("Name") or "", tint, float(low), float(high)))
    return scaling, channels


def codings(czicmd, path):
    if not czicmd:
        return set()
    p = subprocess.run([czicmd, "-c", "PrintInformation", "-s", path, "-i", "AllSubBlocks"],
                       check=True, capture_output=True, text=True)
    return {w.split("=", 1)[1] for w in p.stdout.split() if w.startswith("compression=")}


def openslide_planes(path, whole, cases, out, name):
    """OpenSlide's 8-bit RGB of the zoom-1 regions (raw coordinates; OpenSlide's level 0
    starts at the document's bounding box)."""
    if openslide is None:
        return
    try:
        s = openslide.OpenSlide(path)
    except openslide.OpenSlideError:
        return
    for (x, y, w, h), z, _ in cases:
        if z != 1.0:
            continue
        im = np.asarray(s.read_region((x - whole.x, y - whole.y), 0, (w, h)))
        out.append(("plane8", name, str(x), str(y), str(w), str(h), sha(im[..., :3])))


def main():
    corpus = sys.argv[1]
    czicmd = sys.argv[2] if len(sys.argv) > 2 else None
    out = []
    for name in sorted(os.listdir(corpus)):
        if not name.endswith(".czi"):
            continue
        path = os.path.join(corpus, name)
        hilo = "zstd1" in codings(czicmd, path)
        with pyczi.open_czi(path) as r:
            box = r.total_bounding_box_no_pyramid
            dims = [f"{k}{v[0]}:{v[1] - v[0]}" for k, v in sorted(box.items()) if k not in "XY"]
            out.append(("info", name, "dims", ",".join(dims) or "-"))
            whole = r.total_bounding_rectangle_no_pyramid
            out.append(("info", name, "bbox", rect(whole)))
            scenes = r.scenes_bounding_rectangle_no_pyramid
            for i, s in sorted(scenes.items()):
                out.append(("scene", name, str(i), rect(s)))
            scaling, channels = xml_facts(r.raw_metadata)
            out.append(("scaling", name, ",".join("-" if v is None else repr(v) for v in scaling)))
            for i, (n, tint, low, high) in enumerate(channels):
                out.append(("channel", name, str(i), f"{n}|{tint}|{low!r}|{high!r}"))
            pages = (
                [(i, s) for i, s in sorted(scenes.items())]
                if len(scenes) > 1
                else [(next(iter(scenes)) if scenes else -1, next(iter(scenes.values())) if scenes else whole)]
            )
            for scene, s in pages:
                filt = scene if len(scenes) > 1 else None
                c0 = box["C"][0] if "C" in box else None
                pixel_name = r.get_channel_pixel_type(c0 or 0)
                last = {k: v[1] - 1 for k, v in box.items() if k not in "XYSMB"}
                first = {k: v[0] for k, v in box.items() if k not in "XYSMB"}
                # An overview at the coarsest power of two fitting OVERVIEW, the last plane.
                k = 0
                while max(s.w, s.h) / (1 << k) > OVERVIEW:
                    k += 1
                zoom = 1.0 / (1 << k)
                cases = [((s.x, s.y, s.w, s.h), zoom, last), ]
                # A 512 × 512 tile from the centre at full resolution, the first plane.
                tw, th = min(512, s.w), min(512, s.h)
                cases.append(((s.x + (s.w - tw) // 2, s.y + (s.h - th) // 2, tw, th), 1.0, first))
                openslide_planes(path, whole, cases, out, name)
                if hilo:
                    continue
                for roi, z, plane in cases:
                    pix = pixel_name
                    if "C" in plane:
                        pix = r.get_channel_pixel_type(plane["C"])
                    w, h, digest = read_plane(r, roi, z, plane, filt, pix)
                    coords = ",".join(f"{k}={v}" for k, v in sorted(plane.items())) or "-"
                    out.append((
                        "plane", name, str(filt if filt is not None else -1),
                        *map(str, roi), repr(z), coords, str(PIXEL[pix]), str(w), str(h), digest,
                    ))
        if czicmd:
            with tempfile.TemporaryDirectory(dir=corpus) as tmp:
                for att in ("Label", "SlidePreview", "Thumbnail"):
                    base = os.path.join(tmp, "a")
                    subprocess.run(
                        [czicmd, "-c", "ExtractAttachment", "-s", path, "-e",
                         '{"name":"%s"}' % att, "-o", base],
                        check=True, capture_output=True,
                    )
                    got = [f for f in os.listdir(tmp) if f.startswith(f"a_{att}_")]
                    for f in got:
                        p = os.path.join(tmp, f)
                        data = open(p, "rb").read()
                        if f.upper().endswith(".CZI"):
                            with pyczi.open_czi(p) as e:
                                b = e.total_bounding_rectangle
                                ebox = e.total_bounding_box
                                longest = max(b.w, b.h)
                                z = (np.float32(ASSOCIATED_MAX) / np.float32(longest)
                                     if longest > ASSOCIATED_MAX else np.float32(1.0))
                                plane = {"C": ebox["C"][0]} if "C" in ebox else {}
                                pix = e.get_channel_pixel_type(plane.get("C", 0))
                                a = e.read(roi=(b.x, b.y, b.w, b.h), plane=plane, zoom=float(z),
                                           pixel_type=pix)
                                out.append(("attach", name, att, "px", str(a.shape[1]),
                                            str(a.shape[0]), str(PIXEL[pix]), sha(a),
                                            str(ASSOCIATED_MAX)))
                        else:
                            out.append(("attach", name, att, "enc",
                                        hashlib.sha256(data).hexdigest()))
                        os.remove(p)
    with open(os.path.join(corpus, "expected.tsv"), "w") as f:
        for row in out:
            f.write("\t".join(row) + "\n")
    print(f"czi-oracle: {len(out)} expected values, pylibCZIrw {pyczi.__name__}")


if __name__ == "__main__":
    main()
