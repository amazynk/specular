#!/usr/bin/env python3
"""Write the committed EVRC fixture of specular-evrc and FFmpeg's decode of it.

Usage: tools/corpus/evrc-fixtures.py [OUT_DIR]
  OUT_DIR defaults to crates/formats/specular-evrc/tests/data.

There is no EVRC encoder in FFmpeg or in any pip wheel, so the packets are built here from
a seeded generator: rate 1 and rate 1/2 runs with a slowly moving pitch delay, mid-range
gains and random pulses, rate 1/8 runs, and a few erased and blank frames. LSP indices are
drawn until the decoded LSPs are at least 0.05 / 2 pi apart, as the encoder keeps them
(C.S0014-A 4.4.3; FFmpeg conceals frames whose LSPs crowd closer). The codebooks are read
from the generated src/tables.rs. The packets go into a QCP file (RIFF 'QLCM', RFC 3625) with the EVRC codec
GUID, which the system ffmpeg decodes to 16-bit samples, with its postfilter
(`mixed.ffmpeg.s16`) and without (`mixed.nopf.s16`; `-postfilter 0`). FFmpeg emits nothing
for blank and erased packets, so their concealment has no oracle.
`cargo test -p specular-evrc --test oracle` compares our decoder with both.
"""

import math
import os
import random
import re
import struct
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
CRATE = os.path.join(ROOT, "crates", "formats", "specular-evrc")
OUT = sys.argv[1] if len(sys.argv) > 1 else os.path.join(CRATE, "tests", "data")

EVRC_GUID = bytes([0x8d, 0xd4, 0x89, 0xe6, 0x76, 0x90, 0xb5, 0x46,
                   0x91, 0xef, 0x73, 0x6a, 0x51, 0x00, 0xce, 0xb4])


def tables():
    src = open(os.path.join(CRATE, "src", "tables.rs"), encoding="utf-8").read()
    out = {}
    for m in re.finditer(r"static (LSP_\w+): \[\[f32; (\d+)\]; (\d+)\] = \[(.*?)\n\];", src, re.S):
        rows = re.findall(r"\[([^\[\]]+)\]", m.group(4))
        out[m.group(1)] = [[float(v) for v in r.split(",") if v.strip()] for r in rows]
    return out


T = tables()
BOOKS = {
    "full": [("LSP_FULL_1", 6), ("LSP_FULL_2", 6), ("LSP_FULL_3", 9), ("LSP_FULL_4", 7)],
    "half": [("LSP_HALF_1", 7), ("LSP_HALF_2", 7), ("LSP_HALF_3", 8)],
    "eighth": [("LSP_EIGHTH_1", 4), ("LSP_EIGHTH_2", 4)],
}


class Bits:
    def __init__(self):
        self.v = []

    def put(self, val, n):
        for i in range(n - 1, -1, -1):
            self.v.append((val >> i) & 1)

    def bytes(self, size):
        b = self.v + [0] * (size * 8 - len(self.v))
        return bytes(int("".join(map(str, b[i:i + 8])), 2) for i in range(0, size * 8, 8))


def lsp_indices(rng, rate, prev):
    """Indices near the previous ones (a smooth spectrum) whose LSPs ascend."""
    books = BOOKS[rate]
    for _ in range(10000):
        idx = []
        for k, (name, bits) in enumerate(books):
            n = len(T[name])
            if prev and rng.random() < 0.7:
                idx.append(min(n - 1, max(0, prev[k] + rng.randint(-2, 2))) if k < len(prev) else rng.randrange(n))
            else:
                idx.append(rng.randrange(n))
        w = [v for (name, _), i in zip(books, idx) for v in T[name][i]]
        # the encoder keeps codebook seams at least 0.05 / 2 pi apart (4.4.3)
        if all(b - a > 0.05 / (2 * math.pi) for a, b in zip(w, w[1:])) and w[0] > 0.01:
            return idx
    raise SystemExit("no ascending LSP set found")


def full(rng, st):
    b = Bits()
    b.put(0, 1)
    st["lsp_full"] = lsp_indices(rng, "full", st.get("lsp_full"))
    for (name, bits), i in zip(BOOKS["full"], st["lsp_full"]):
        b.put(i, bits)
    prev = st["tau"]
    st["tau"] = max(20, min(120, st["tau"] + rng.randint(-3, 3)))
    b.put(st["tau"] - 20, 7)
    b.put(st["tau"] - prev + 16, 5)
    for _ in range(3):
        b.put(rng.choice([2, 3, 4, 5]), 3)
        for _ in range(3):
            b.put(rng.randrange(121) | (rng.randrange(2) << 7), 8)
        b.put((rng.randrange(4) << 9) | (rng.randrange(4) << 7) | rng.randrange(121), 11)
        b.put(rng.randint(8, 18), 5)
    b.put(0, 1)  # TTY baud rate bit
    return bytes([4]) + b.bytes(22)


def half(rng, st):
    b = Bits()
    st["lsp_half"] = lsp_indices(rng, "half", st.get("lsp_half"))
    for (name, bits), i in zip(BOOKS["half"], st["lsp_half"]):
        b.put(i, bits)
    st["tau"] = max(20, min(120, st["tau"] + rng.randint(-3, 3)))
    b.put(st["tau"] - 20, 7)
    for _ in range(3):
        b.put(rng.choice([1, 2, 3, 4]), 3)
        b.put(rng.randrange(1024), 10)
        b.put(rng.randint(4, 9), 4)
    return bytes([3]) + b.bytes(10)


def eighth(rng, st):
    b = Bits()
    st["lsp_eighth"] = lsp_indices(rng, "eighth", st.get("lsp_eighth"))
    for (name, bits), i in zip(BOOKS["eighth"], st["lsp_eighth"]):
        b.put(i, bits)
    b.put(rng.choice([0, 2, 4, 8, 16, 24]), 8)
    return bytes([1]) + b.bytes(2)


def packets():
    rng = random.Random(20261108)
    st = {"tau": 60}
    out = []
    plan = [("full", 20), ("half", 10), ("full", 15), ("eighth", 8), ("full", 15),
            ("erase", 2), ("full", 10), ("half", 8), ("blank", 1), ("full", 6), ("eighth", 5)]
    for kind, n in plan:
        for _ in range(n):
            if kind == "full":
                out.append(full(rng, st))
            elif kind == "half":
                out.append(half(rng, st))
            elif kind == "eighth":
                out.append(eighth(rng, st))
            elif kind == "erase":
                out.append(bytes([14]))
            else:
                out.append(bytes([0]))
    return out


def qcp(pkts):
    name = b"TIA IS-127 Enhanced Variable Rate Codec, Speech Service Option 3"
    fmt = bytes([1, 0]) + EVRC_GUID + struct.pack("<H", 1) + name.ljust(80, b"\0")
    fmt += struct.pack("<HHHHHI", 8500, 23, 160, 8000, 16, 4)
    rates = [(22, 4), (10, 3), (2, 1), (0, 0)]
    fmt += b"".join(bytes([s, h]) for s, h in rates).ljust(16, b"\0")
    fmt += b"\0" * 20
    data = b"".join(pkts)
    vrat = struct.pack("<II", 1, len(pkts))
    body = b"QLCM"
    for cid, c in [(b"fmt ", fmt), (b"vrat", vrat), (b"data", data)]:
        body += cid + struct.pack("<I", len(c)) + c + (b"\0" if len(c) & 1 else b"")
    return b"RIFF" + struct.pack("<I", len(body)) + body


def main():
    os.makedirs(OUT, exist_ok=True)
    pkts = packets()
    path = os.path.join(OUT, "mixed.qcp")
    with open(path, "wb") as f:
        f.write(qcp(pkts))
    for name, opts in [("mixed.ffmpeg.s16", []), ("mixed.nopf.s16", ["-postfilter", "0"])]:
        pcm = subprocess.run(["ffmpeg", "-hide_banner", "-loglevel", "error", *opts, "-i", path,
                              "-f", "s16le", "-ac", "1", "-"], check=True, capture_output=True).stdout
        # FFmpeg outputs nothing for blank and erased packets (one byte); the test skips ours
        if len(pcm) != sum(len(p) > 1 for p in pkts) * 320:
            raise SystemExit(f"ffmpeg gave {len(pcm) // 2} samples for {len(pkts)} frames")
        with open(os.path.join(OUT, name), "wb") as f:
            f.write(pcm)
    print(f"evrc-fixtures: {len(pkts)} frames in {path}")


main()
