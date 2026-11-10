#!/usr/bin/env python3
"""Fetch Monkey's Audio files made by Monkey's Audio 3.97 (file version 3970) from public
archive.org items, and decode them with the reference decoder for specular-ape's `old` test.

Usage: python3 -I tools/corpus/fetch-old-ape.py OUT
  OUT/bin/mac comes from fetch-lossless.sh; files go to OUT/samples/old-ape (about 40 MB).

No encoder of versions 3.95-3.98 can be built here: the SDKs of that time (MAC_SDK_397.zip,
2002) carry the old "Monkey's Audio Source Code License Agreement", which forbids use from
another program and commercial use without written permission. So the samples are real
files of the time; most are fetched only in part (the first 6 MB: a cut file whose whole
frames are compared). A scan of archive.org's 250 Monkey's Audio items found only 3970 and
3990 files (no 3950, 3960 or 3980); among the 3970 ones, levels 2000, 3000 and 4000.
Dev corpus only: never committed or bundled. Each download is checked against its SHA-256.
"""

import hashlib
import os
import subprocess
import sys
import urllib.parse
import urllib.request

OUT = sys.argv[1]
S = os.path.join(OUT, "samples", "old-ape")
MAC = os.path.join(OUT, "bin", "mac")
os.makedirs(S, exist_ok=True)

# name, archive.org item, file, bytes to fetch (0: whole), SHA-256 of what is fetched
FILES = [
    ("ia-3970-a.ape", "3anhluabungchaytrivanvinh", "1 Gioi thieu.ape", 0,
     "404ada048b2b6db899cf6690fc8cb80e0306d670798aa718eee4733d276f0521"),
    ("ia-3970-b.ape", "3anhluabungchaytrivanvinh", "10 Giesu con yeu Chua - Tri Van Vinh.ape", 6000000,
     "638eda0e7657e41f5a5a813840b199a0c7774a72c6b70c4f9cd29d069ae52a2d"),
    ("ia-3970-c.ape", "vicl-60932", "Witch Hunter ROBIN (ウィッチハンターロビン) Original Sound Score 2 (APE)/CDImage.ape", 6000000,
     "1458a6dfdecaad9aa1ad2f4e7cafd838e4c7749b72f247506182d9acb147d36c"),
    ("ia-3970-d.ape", "cdimage_202602", "scorpions - [1996] pure instinct/CDImage.ape", 6000000,
     "38631d79c0280e8235d3de19e886292b43f0f0190d9a16f27de5a893e975a156"),
    ("ia-3970-e.ape", "polka-krzysztof-komeda", "! Jazz/Krzysztof Komeda/Krzysztof Komeda - The Complete Recordings Of Krzysztof Komeda (vol1)/Krzysztof Komeda - The Complete Recordings Of Krzysztof Komeda Vol. 01.ape", 6000000,
     "1ca01eb966f6c47f88810abf97cccde43f89b5247cb162bd3a6cd1efa0d0b216"),
    ("ia-3970-f.ape", "vpcg-84898", "Souten Kouro Original Soundtrack [日本テレビ系アニメ 蒼天航路 オリジナル・サウンドトラック] (VPCG-84898) APE/CDImage.ape", 6000000,
     "24b39e6b31c6e0a8e280d7b9ffd6b28a603daa476eb4a0e8929ef5b0ee29273e"),
]


def fetch(name, ident, path, nbytes):
    p = os.path.join(S, name)
    if os.path.exists(p):
        return p
    url = f"https://archive.org/download/{ident}/{urllib.parse.quote(path)}"
    req = urllib.request.Request(url)
    if nbytes:
        req.add_header("Range", f"bytes=0-{nbytes - 1}")
    with urllib.request.urlopen(req, timeout=120) as r, open(p + ".part", "wb") as f:
        total = int(r.headers.get("Content-Length") or 0)
        got = 0
        while True:
            b = r.read(1 << 20)
            if not b:
                break
            f.write(b)
            got += len(b)
            print(f"fetch-old-ape: {name} {got >> 20}/{total >> 20} MB", flush=True)
    os.replace(p + ".part", p)
    return p


manifest = []
for name, ident, path, nbytes, want in FILES:
    p = fetch(name, ident, path, nbytes)
    h = hashlib.sha256(open(p, "rb").read()).hexdigest()
    if h != want:
        sys.exit(f"fetch-old-ape: {name}: checksum mismatch")
    wav = p[:-4] + ".wav"
    # A cut file ends in an error; mac keeps what it decoded.
    subprocess.run([MAC, p, wav, "-d"], capture_output=True)
    manifest.append((name, os.path.basename(wav), "cut" if nbytes else "whole"))
with open(os.path.join(S, "manifest.tsv"), "w") as f:
    for m in manifest:
        f.write("\t".join(m) + "\n")
print(f"fetch-old-ape: {len(manifest)} files in {S}")
