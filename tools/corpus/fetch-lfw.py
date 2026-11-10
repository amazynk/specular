#!/usr/bin/env python3
"""Identity-labelled faces for the People grouping oracle: a fixed sample of
Labeled Faces in the Wild (University of Massachusetts; published for research, local use
only, never committed).

Downloads lfw.tgz (180 MB, the figshare mirror scikit-learn uses) into DL_DIR, extracts
`--people` identities with at least 10 photos, `--each` photos per identity, into
OUT_DIR/identity/<name>/, then deletes the archive.

Usage: fetch-lfw.py DL_DIR OUT_DIR [--people 40] [--each 10]
"""

import os
import random
import sys
import tarfile
import urllib.request

URL = "https://ndownloader.figshare.com/files/5976018"


def arg(name, default):
    return int(sys.argv[sys.argv.index(name) + 1]) if name in sys.argv else default


if __name__ == "__main__":
    if len(sys.argv) < 3:
        sys.exit(__doc__)
    dl, out = sys.argv[1], sys.argv[2]
    people, each = arg("--people", 40), arg("--each", 10)
    os.makedirs(dl, exist_ok=True)
    tgz = os.path.join(dl, "lfw.tgz")
    if not os.path.exists(tgz):
        def hook(n, bs, total):
            print(f"\r  lfw.tgz {n * bs / 1e6:6.1f} of {total / 1e6:.1f} MB", end="", flush=True)
        urllib.request.urlretrieve(URL, tgz, hook)
        print()
    with tarfile.open(tgz) as t:
        members = [m for m in t.getmembers() if m.name.endswith(".jpg")]
        by = {}
        for m in members:
            by.setdefault(m.name.split("/")[1], []).append(m)
        names = sorted(n for n, v in by.items() if len(v) >= 10)
        rng = random.Random(3)
        for n in sorted(rng.sample(names, min(people, len(names)))):
            folder = os.path.join(out, "identity", n)
            os.makedirs(folder, exist_ok=True)
            for i, m in enumerate(sorted(by[n], key=lambda m: m.name)[:each]):
                with open(os.path.join(folder, f"{i:02d}.jpg"), "wb") as f:
                    f.write(t.extractfile(m).read())
    os.remove(tgz)
    print(f"{people} people × {each} photos in {out}/identity")
