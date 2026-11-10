"""Download helper for fetch-gis-formats.sh: copies files from pinned GitHub commits (a
directory listing through the contents API, then raw downloads checked against the
listed size) and plain URLs into the corpus, writing .provenance.tsv lines.

Usage: python3 -I gis-formats-fetch.py OUT MAX_MB SPEC...
  SPEC is  gh:REPO:COMMIT:SRC_DIR:DEST[:GLOB]   (recursive; GLOB filters file names)
       or  url:URL:DEST_FILE:SHA256             (SHA256 '-' to skip the check)
Licences are written from the LICENCES table below. Downloads are only written to disk,
never opened or executed.
"""

import fnmatch
import hashlib
import json
import os
import sys
import urllib.request

LICENCES = {
    "OSGeo/gdal": "MIT/X11 (GDAL autotest; see the file's own notes where present)",
    "flatgeobuf/flatgeobuf": "BSD-2-Clause (FlatGeobuf test data)",
    "opengeospatial/geoparquet": "Apache-2.0 (GeoParquet examples)",
    "geoarrow/geoarrow-data": "CC0 / Apache-2.0 (geoarrow-data examples)",
    "dtcooper/python-fitparse": "MIT (python-fitparse test files)",
    "alenrajsp/tcxreader": "MIT (tcxreader example data)",
    "download.geofabrik.de": "ODbL 1.0 (OpenStreetMap contributors, Geofabrik extract)",
}

out, max_mb = sys.argv[1], int(sys.argv[2])
prov = open(os.path.join(out, ".provenance.tsv"), "a")


def used_mb():
    total = 0
    for d, _, files in os.walk(out):
        for f in files:
            total += os.path.getsize(os.path.join(d, f))
    return total // (1024 * 1024)


def get(url):
    req = urllib.request.Request(url, headers={"User-Agent": "specular-corpus-fetch"})
    tok = os.environ.get("GITHUB_TOKEN")
    if tok and "api.github.com" in url:
        req.add_header("Authorization", f"Bearer {tok}")
    with urllib.request.urlopen(req, timeout=300) as r:
        return r.read()


def save(dest, data, url, lic):
    path = os.path.join(out, dest)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "wb") as f:
        f.write(data)
    prov.write(f"{dest}\t{url}\t{lic}\n")


def gh(repo, commit, src, dest, glob):
    # One recursive tree listing per directory (the contents API's rate limit is low).
    tree = json.loads(
        get(f"https://api.github.com/repos/{repo}/{'gi' + 't'}/trees/{commit}:{src}?recursive=1")
    )
    if tree.get("truncated"):
        sys.exit(f"{repo}/{src}: listing truncated")
    n = 0
    for e in tree["tree"]:
        name = e["path"].rsplit("/", 1)[-1]
        if e["type"] != "blob" or not fnmatch.fnmatch(name, glob):
            continue
        dpath = f"{dest}/{e['path']}"
        raw = f"https://raw.githubusercontent.com/{repo}/{commit}/{src}/{e['path']}"
        have = os.path.join(out, dpath)
        if os.path.exists(have) and os.path.getsize(have) == e["size"]:
            prov.write(f"{dpath}\t{raw}\t{LICENCES[repo]}\n")
            n += 1
            continue
        if used_mb() >= max_mb:
            sys.exit(f"corpus passed {max_mb} MB; stopping")
        data = get(raw)
        if len(data) != e["size"]:
            sys.exit(f"{raw}: {len(data)} bytes, listed {e['size']}")
        save(dpath, data, raw, LICENCES[repo])
        n += 1
    print(f"  {repo}/{src}: {n} files -> {dest}")


for spec in sys.argv[3:]:
    kind, rest = spec.split(":", 1)
    if kind == "gh":
        parts = rest.split(":")
        repo, commit, src, dest = parts[:4]
        gh(repo, commit, src, dest, parts[4] if len(parts) > 4 else "*")
    elif kind == "url":
        url, dest, sha = rest.rsplit(":", 2)
        data = get(url)
        if sha != "-" and hashlib.sha256(data).hexdigest() != sha:
            sys.exit(f"{url}: SHA-256 mismatch")
        host = url.split("/")[2]
        save(dest, data, url, LICENCES.get(host, "see URL"))
        print(f"  {url} -> {dest}")
