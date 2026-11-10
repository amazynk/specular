#!/usr/bin/env python3
"""Fetches the face-free photo set for the People false-face check: walls, leaves,
fabric, facades, a sky and wood, from Wikimedia Commons, as 1280 px renditions.

The same 16 photos the turned-face passes were checked on (the AI denoise set
without its portraits). Every file is pinned by its Commons title and the SHA-1 Commons
states for the original, and kept only while Commons states CC0 or public domain; a changed
file or licence is skipped and reported. No people in them. PROVENANCE.tsv beside the files
lists title, licence, author, source page, pinned SHA-1 and the SHA-256 of the file as
downloaded (Commons may re-render a thumbnail, so that one is a record, not a pin).
Local use only, never committed.

Usage: fetch-face-free.py [OUT_DIR]   (default target/face-free, where the test looks)
Then:  cargo test -p specular-ai --test faces real_faces_found_in_turned_and_exif_photos
       (with SPECULAR_FACE_ROTATED and SPECULAR_FACE_MODELS; SPECULAR_FACE_FREE=OUT_DIR for
       another folder)
"""

import hashlib
import json
import os
import sys
import time
import urllib.parse
import urllib.request

API = "https://commons.wikimedia.org/w/api.php"
UA = "SpecularDev/0.1 (local test corpus fetch; python urllib)"
WIDTH = 1280
FREE = ("cc0", "public domain")

# (file name, Commons title, SHA-1 of the original as Commons states it).
PINS = [
    ("brick_walls-0.jpg", "Aniceto Lacson House entrance - NHCP.jpg",
     "71d81acbc9ee68ff5a8d064e80346bb59a6de555"),
    ("brick_walls-1.jpg", "Art-2450.jpg", "9205096715bf218eeda2d638e4df255aabf5a112"),
    ("brick_walls-2.jpg", "CCTVBrickWall3.jpg", "8957cab9791a3f5310eeb2f0bc5726a2d737d166"),
    ("leaves-0.jpg", "A Light in the Dark (31052454935).jpg",
     "cf91f50fdf1e9a5ad3f21fbe35988e10c907aa7e"),
    ("leaves-1.jpg", "Aaron Burden 2015-07-28 (Unsplash).jpg",
     "ec6fb21aa66c94a3ff934c12fd919bcf689e74fe"),
    ("leaves-2.jpg", "Alone (Unsplash 8i0PDTesYCY).jpg",
     "d196ff4fbd5a83164f69a0c1c6ca52c1ef84131e"),
    ("textiles-0.jpg", "African fabric called Ọnụ neli akụ.jpg",
     "8080ed81f4bd25ab6610203353d4093a03d18f7d"),
    ("textiles-1.jpg", "Basofil 2.jpg", "9e188dd3119ec46e612374dd3313463e8cb59d65"),
    ("textiles-2.jpg", "Blockprintsfromi01lewi 0004.jpg",
     "71b3ba2d37ccd1f8553d852e37612b1b1efc5184"),
    ("facades-0.jpg", "20120724Puccinistr17-19Saarbruecken2.jpg",
     "db3eeccccb3ee19d305f3e997ed0aee21f769e62"),
    ("facades-1.jpg", "Aland post 1989 Townhall-Mariahamn.jpg",
     "35cc91c5c53877b214c891ffd605983e91f872b9"),
    ("facades-2.jpg", "20120629Bismarckstr4 Saarbruecken1.jpg",
     "22ade217c0d3b3fd41d8bdda0f40d575fd2010fe"),
    ("clouds-0.jpg",
     "Two slender stems with leaf clusters against a cloudy sky, Lan Su Chinese Garden.jpg",
     "02d1bd2e1635849c44dac409b97126cbff3056c5"),
    ("wood_textures-0.jpg", "Aglomerado manchado.jpg",
     "f2836ea7f9898774a44d2e61095b5da5c4a44aad"),
    ("wood_textures-1.jpg", "Blue varnish oil painted old wood.jpg",
     "d13cf552bb820085f8edb59c009a1bd09a74291a"),
    ("wood_textures-2.jpg", "Gatunki drewna - Brockhaus 1937.jpg",
     "e5a6629957a09b77e11f745922d3f47bb2cc24f3"),
]


def fetch(url):
    req = urllib.request.Request(url, headers={"User-Agent": UA})
    last = None
    for attempt in range(4):
        try:
            with urllib.request.urlopen(req, timeout=60) as r:
                return r.read()
        except Exception as e:  # network hiccup or rate limit: back off
            last = e
            time.sleep(10 + 20 * attempt)
    raise last


def info(titles):
    q = dict(action="query", titles="|".join("File:" + t for t in titles), prop="imageinfo",
             iiprop="url|sha1|extmetadata|mime", iiurlwidth=WIDTH, format="json")
    d = json.loads(fetch(API + "?" + urllib.parse.urlencode(q)))
    out = {}
    for p in d.get("query", {}).get("pages", {}).values():
        ii = (p.get("imageinfo") or [None])[0]
        if ii:
            out[p["title"].removeprefix("File:")] = ii
    return out


def main():
    if len(sys.argv) > 2 or sys.argv[1:2] in (["-h"], ["--help"]):
        sys.exit(__doc__)
    root = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..")
    out = os.path.abspath(sys.argv[1] if len(sys.argv) == 2 else
                          os.path.join(root, "target", "face-free"))
    os.makedirs(out, exist_ok=True)
    meta = info([t for _, t, _ in PINS])
    total, t0, n, bad = 0, time.time(), len(PINS), 0
    rows = ["file\ttitle\tlicence\tauthor\tsource\tsha1_original\tsha256_file"]
    for i, (name, title, sha1) in enumerate(PINS):
        ii = meta.get(title)
        if ii is None:
            print(f"\n{title}: not on Commons any more: skipped", file=sys.stderr)
            bad += 1
            continue
        m = ii.get("extmetadata", {})
        lic = m.get("LicenseShortName", {}).get("value", "")
        if ii.get("sha1") != sha1:
            print(f"\n{title}: a new version (SHA-1 {ii.get('sha1')}): skipped", file=sys.stderr)
            bad += 1
            continue
        if not lic.lower().startswith(FREE):
            print(f"\n{title}: licence now {lic!r}: skipped", file=sys.stderr)
            bad += 1
            continue
        path = os.path.join(out, name)
        if os.path.isfile(path):
            data = open(path, "rb").read()
        else:
            data = fetch(ii.get("thumburl") or ii["url"])
            with open(path, "wb") as f:
                f.write(data)
            time.sleep(1.0)
        total += len(data)
        rate = total / max(time.time() - t0, 1e-3) / 1e6
        left = (n - i - 1) * (time.time() - t0) / (i + 1)
        bar = "#" * ((i + 1) * 20 // n)
        print(f"\r  [{bar:20}] {i + 1}/{n}  {total / 1e6:5.1f} MB  {rate:5.2f} MB/s  "
              f"ETA {left:3.0f} s  {name:22}", end="", flush=True)
        artist = " ".join(m.get("Artist", {}).get("value", "").split())[:200]
        rows.append("\t".join([name, title, lic, artist, ii.get("descriptionurl", ""), sha1,
                               hashlib.sha256(data).hexdigest()]))
    with open(os.path.join(out, "PROVENANCE.tsv"), "w") as f:
        f.write("\n".join(rows) + "\n")
    print(f"\n{len(rows) - 1} photos in {out}" + (f", {bad} skipped" if bad else ""))
    if bad:
        sys.exit(1)


if __name__ == "__main__":
    main()
