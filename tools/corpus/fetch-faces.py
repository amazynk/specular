#!/usr/bin/env python3
"""Fetches the People/Culling oracle corpus from Wikimedia Commons.

Licences: every file is kept only when Commons states a free licence (CC BY, CC BY-SA, CC0
or public domain); the licence, author and source page of each file are written to
MANIFEST.tsv beside them. Local use only, never committed (the folder is gitignored).

  identity/<person>/NN.jpg  photos from a public figure's Commons category and its
                            subcategories (640 px wide); the oracle keeps those where the
                            reference detector finds exactly one face, as that person
  group/NN.jpg              group photographs (1600 px wide) for detection recall
  eyes/NN.jpg               photos of people with closed eyes (640 px)

Usage: fetch-faces.py OUT_DIR [--per-person N]
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

PEOPLE = [
    "Barack Obama", "Angela Merkel", "Joe Biden", "Emmanuel Macron", "Justin Trudeau",
    "Volodymyr Zelenskyy", "Kamala Harris", "Olaf Scholz", "Narendra Modi", "Jacinda Ardern",
    "Ursula von der Leyen", "Hillary Clinton", "Bernie Sanders", "Elizabeth Warren",
    "Lionel Messi", "Cristiano Ronaldo", "Serena Williams", "Roger Federer", "Rafael Nadal",
    "Greta Thunberg", "Bill Gates", "Tom Hanks", "Keanu Reeves", "Nancy Pelosi",
    "Boris Johnson",
]
GROUPS = ["Group photographs", "Group portraits"]
EYES = ["Closed eyes", "People with closed eyes", "Sleeping people", "Blinking"]
FREE = ("cc by", "cc-by", "cc0", "public domain", "pd", "cc by-sa", "cc-by-sa")


def get(params):
    params = dict(params, format="json")
    url = API + "?" + urllib.parse.urlencode(params)
    req = urllib.request.Request(url, headers={"User-Agent": UA})
    for attempt in range(4):
        try:
            with urllib.request.urlopen(req, timeout=60) as r:
                return json.load(r)
        except Exception as e:  # network hiccup or rate limit: back off
            time.sleep(10 + 20 * attempt)
            last = e
        finally:
            time.sleep(1.0)
    raise last


def members(cat, kind, limit=500):
    out, cont = [], {}
    while len(out) < limit:
        d = get(dict(action="query", list="categorymembers", cmtitle="Category:" + cat,
                     cmtype=kind, cmlimit=min(500, limit), **cont))
        out += [m["title"] for m in d["query"]["categorymembers"]]
        if "continue" not in d:
            break
        cont = d["continue"]
    return out[:limit]


def info(titles, width):
    d = get(dict(action="query", titles="|".join(titles), prop="imageinfo",
                 iiprop="url|extmetadata|mime", iiurlwidth=width))
    out = []
    for p in d.get("query", {}).get("pages", {}).values():
        ii = (p.get("imageinfo") or [None])[0]
        if not ii or ii.get("mime") != "image/jpeg":
            continue
        meta = ii.get("extmetadata", {})
        lic = meta.get("LicenseShortName", {}).get("value", "")
        if not lic.lower().startswith(FREE):
            continue
        artist = meta.get("Artist", {}).get("value", "")
        out.append((p["title"], ii.get("thumburl") or ii["url"], lic,
                    " ".join(artist.split())[:200], ii.get("descriptionurl", "")))
    return out


done_bytes = 0
t0 = time.time()


def fetch(url, path):
    global done_bytes
    if os.path.exists(path):
        return True
    req = urllib.request.Request(url, headers={"User-Agent": UA})
    try:
        with urllib.request.urlopen(req, timeout=60) as r:
            data = r.read()
    except Exception as e:
        print(f"  skip {url}: {e}", file=sys.stderr)
        return False
    with open(path, "wb") as f:
        f.write(data)
    done_bytes += len(data)
    rate = done_bytes / max(time.time() - t0, 1e-3) / 1e6
    print(f"\r  {done_bytes / 1e6:7.1f} MB  {rate:5.2f} MB/s  {path[-60:]:60}", end="", flush=True)
    time.sleep(1.0)
    return True


def take(titles, n, width, folder, manifest, rng):
    rng.shuffle(titles)
    kept = 0
    for i in range(0, len(titles), 40):
        if kept >= n:
            break
        for title, url, lic, artist, page in info(titles[i:i + 40], width):
            if kept >= n:
                break
            path = os.path.join(folder, f"{kept:02d}.jpg")
            if fetch(url, path):
                manifest.write(f"{os.path.relpath(path, OUT)}\t{lic}\t{artist}\t{page}\n")
                kept += 1
    return kept


if __name__ == "__main__":
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    OUT = os.path.abspath(sys.argv[1])
    per = int(sys.argv[sys.argv.index("--per-person") + 1]) if "--per-person" in sys.argv else 16
    rng = random.Random(7)
    os.makedirs(OUT, exist_ok=True)
    import io
    import threading
    from concurrent.futures import ThreadPoolExecutor
    lock = threading.Lock()

    def one(job):
        name, cats, n, width, subs = job
        files = []
        for c in cats:
            files += members(c, "file", 400)
            for sub in members(c, "subcat", subs) if subs else []:
                if len(files) > 120:
                    break
                files += members(sub.split(":", 1)[1], "file", 100)
        folder = os.path.join(OUT, name)
        os.makedirs(folder, exist_ok=True)
        buf = io.StringIO()
        k = take(sorted(set(files)), n, width, folder, buf, random.Random(7))
        with lock:
            man.write(buf.getvalue())
            man.flush()
            print(f"\n{name}: {k} of {len(files)}", flush=True)

    # Identities come from fetch-lfw.py unless asked for here (Commons rate-limits).
    jobs = [(os.path.join("identity", p.replace(" ", "_")), [p], per, 640, 4) for p in PEOPLE
            if "--commons-people" in sys.argv]
    jobs += [("group", GROUPS, 80, 1600, 0), ("eyes", EYES, 40, 640, 0)]
    with open(os.path.join(OUT, "MANIFEST.tsv"), "w") as man:
        man.write("file\tlicence\tauthor\tsource\n")
        with ThreadPoolExecutor(1) as ex:
            list(ex.map(one, jobs))
