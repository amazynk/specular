#!/usr/bin/env python3
"""Fetches a small photo set for the object removal and depth oracle (depth and object removal) from
Wikimedia Commons: scenes with clear depth (streets, corridors, landscapes, portraits with
background, close-ups) and small objects to remove.

Licences: every file is kept only when Commons states CC0 or public domain (CC BY with
--cc-by; CC BY-SA never); the licence, author and source page of each file go to MANIFEST.tsv beside them.
Local use only, never committed (keep OUT under target/).

Usage: fetch-depth.py OUT_DIR [--per N] [--cc-by]
Then: removal-depth-oracle.py prep ORACLE_OUT MODELS OUT_DIR
"""

import json
import os
import random
import sys
import time
import urllib.parse
import urllib.request

API = "https://commons.wikimedia.org/w/api.php"
UA = "SpecularDev/0.1 (local test corpus fetch; python urllib)"
CATS = [
    "Streets", "Corridors", "Landscapes", "Alleys", "Staircases", "Flowers",
    "Dogs in grass", "Benches", "Beaches", "Forests",
]


def get(params):
    params = dict(params, format="json")
    url = API + "?" + urllib.parse.urlencode(params)
    req = urllib.request.Request(url, headers={"User-Agent": UA})
    last = None
    for attempt in range(4):
        try:
            with urllib.request.urlopen(req, timeout=60) as r:
                return json.load(r)
        except Exception as e:  # network hiccup or rate limit: back off
            last = e
            time.sleep(10 + 20 * attempt)
        finally:
            time.sleep(1.0)
    raise last


def members(cat, limit=200):
    d = get(dict(action="query", list="categorymembers", cmtitle="Category:" + cat,
                 cmtype="file", cmlimit=limit))
    return [m["title"] for m in d.get("query", {}).get("categorymembers", [])]


def info(titles, width, free):
    d = get(dict(action="query", titles="|".join(titles), prop="imageinfo",
                 iiprop="url|extmetadata|mime", iiurlwidth=width))
    out = []
    for p in d.get("query", {}).get("pages", {}).values():
        ii = (p.get("imageinfo") or [None])[0]
        if not ii or ii.get("mime") != "image/jpeg":
            continue
        meta = ii.get("extmetadata", {})
        lic = meta.get("LicenseShortName", {}).get("value", "")
        if not lic.lower().startswith(free) or "by-sa" in lic.lower():
            continue
        artist = " ".join(meta.get("Artist", {}).get("value", "").split())[:200]
        out.append((ii.get("thumburl") or ii["url"], lic, artist, ii.get("descriptionurl", "")))
    return out


def main():
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    out = os.path.abspath(sys.argv[1])
    per = int(sys.argv[sys.argv.index("--per") + 1]) if "--per" in sys.argv else 3
    free = ("cc0", "public domain", "pd")  # CC BY-SA is never kept (below)
    if "--cc-by" in sys.argv:
        free += ("cc by", "cc-by")
    os.makedirs(out, exist_ok=True)
    rng = random.Random(27)
    total, t0 = 0, time.time()
    with open(os.path.join(out, "MANIFEST.tsv"), "w") as man:
        man.write("file\tlicence\tauthor\tsource\n")
        for cat in CATS:
            titles = members(cat)
            rng.shuffle(titles)
            kept = 0
            for i in range(0, len(titles), 40):
                if kept >= per:
                    break
                for url, lic, artist, page in info(titles[i:i + 40], 1024, free):
                    if kept >= per:
                        break
                    name = f"{cat.replace(' ', '_').lower()}-{kept}.jpg"
                    req = urllib.request.Request(url, headers={"User-Agent": UA})
                    try:
                        with urllib.request.urlopen(req, timeout=60) as r:
                            data = r.read()
                    except Exception as e:
                        print(f"  skip {url}: {e}", file=sys.stderr)
                        continue
                    with open(os.path.join(out, name), "wb") as f:
                        f.write(data)
                    total += len(data)
                    rate = total / max(time.time() - t0, 1e-3) / 1e6
                    print(f"\r  {total / 1e6:6.1f} MB  {rate:5.2f} MB/s  {name[-50:]:50}",
                          end="", flush=True)
                    man.write(f"{name}\t{lic}\t{artist}\t{page}\n")
                    kept += 1
                    time.sleep(1.0)
            print(f"\n{cat}: {kept}", flush=True)


if __name__ == "__main__":
    main()
