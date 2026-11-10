#!/usr/bin/env python3
"""The committed QCELP-13 oracle fixture of specular-qcelp.

Usage: tools/corpus/qcelp-fixtures.py OUT_DIR   (normally crates/formats/specular-qcelp/tests/data)

There is no QCELP encoder to be had, so this builds a QCP file (RFC 3625, RIFF `QLCM`) of
pseudorandom but plausible frames: every rate (1, 1/2, 1/4, 1/8),
LSP codes quantised from a wandering speech-like LSP set that pass the sanity checks of
C.S0020-A §2.4.8.7.3, pitch
lags and gains over their ranges and moderate codebook gains. The tables and packet layouts
are read from crates/formats/specular-qcelp/src/tables.rs. ffmpeg (the system one, 8.0.1 when
written) decodes it to `random.s16` (16-bit little-endian), which the `fixture` test compares
against with an SNR bound.

Writes OUT_DIR/random.qcp and OUT_DIR/random.s16. Nothing is downloaded.
"""

import os
import re
import struct
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
TABLES = open(os.path.join(HERE, "../../crates/formats/specular-qcelp/src/tables.rs")).read()


def table(name):
    m = re.search(r"static %s: [^=]*= \[(.*?)\n\];" % name, TABLES, re.S)
    return m.group(1)


LSPVQ = []
for k in range(1, 6):
    pairs = re.findall(r"\[(-?[\d.]+), (-?[\d.]+)\]", table(f"LSPVQ{k}"))
    LSPVQ.append([(float(a), float(b)) for a, b in pairs])
PACK = {}
for name in ("PACK_FULL", "PACK_HALF", "PACK_QUARTER", "PACK_EIGHTH"):
    PACK[name] = [tuple(int(v) for v in t) for t in re.findall(r"\((\d+), (\d+), (\d+)\)", table(name))]


class Lcg:
    def __init__(self, seed):
        self.s = seed

    def below(self, n):
        self.s = (self.s * 1103515245 + 12345) & 0x7FFFFFFF
        return (self.s >> 8) % n


R = Lcg(20261108)


def lsp_ok(idx, rate):
    w, acc = [], 0.0
    for k in range(5):
        x, y = LSPVQ[k][idx[k]]
        acc += x
        w.append(acc)
        acc += y
        w.append(acc)
    # no near-coincident pairs: such razor-sharp resonances ring for seconds and make any
    # two float decoders drift apart
    if any(w[i + 1] - w[i] < 0.025 for i in range(9)) or w[0] < 0.025:
        return False
    if rate in ("full", "half"):
        return 0.66 < w[9] < 0.985 and all(abs(w[n] - w[n - 4]) >= 0.0931 for n in range(4, 10))
    return 0.70 < w[9] < 0.97 and all(abs(w[n] - w[n - 3]) >= 0.08 for n in range(3, 10))


TARGET = [0.06, 0.13, 0.22, 0.30, 0.40, 0.48, 0.58, 0.67, 0.77, 0.86]


def lspv(rate):
    """Quantise a slowly wandering, speech-like LSP set the way the encoder does
    (§2.4.3.2.6.2, unit weights), so the codes are ones an encoder would send."""
    for _ in range(1000):
        for i in range(10):
            TARGET[i] += (R.below(21) - 10) / 2000
        TARGET.sort()
        for i in range(10):
            lo = TARGET[i - 1] + 0.05 if i else 0.04
            TARGET[i] = min(max(TARGET[i], lo), 0.93 - 0.05 * (9 - i))
        idx, prev = [], 0.0
        for k in range(5):
            t0, t1 = TARGET[2 * k], TARGET[2 * k + 1]
            best = min(range(len(LSPVQ[k])),
                       key=lambda j: (t0 - prev - LSPVQ[k][j][0]) ** 2
                       + (t1 - prev - LSPVQ[k][j][0] - LSPVQ[k][j][1]) ** 2)
            idx.append(best)
            prev += LSPVQ[k][best][0] + LSPVQ[k][best][1]
        if lsp_ok(idx, rate):
            return idx
    raise SystemExit("no plausible LSP codes")


def pack(name, fields, nbytes):
    bits = []
    for f, n, pos in PACK[name]:
        v = fields.get((f, n), 0)
        bits.append((v >> pos) & 1)
    bits += [0] * (nbytes * 8 - len(bits))
    return bytes(int("".join(map(str, bits[i:i + 8])), 2) for i in range(0, len(bits), 8))


def frame(rate):
    f = {}
    if rate in ("full", "half", "quarter"):
        for k, v in enumerate(lspv(rate), 1):
            f[(0, k)] = v
    if rate in ("full", "half"):
        for i in range(1, 5):
            f[(2, i)] = R.below(123) + 1  # PLAG 1..123 (lags 17..139.5; 140.5 up are invalid)
            f[(3, i)] = R.below(2)
            f[(4, i)] = R.below(3)  # pitch gain up to 0.75
        nsub = 16 if rate == "full" else 4
        for i in range(1, nsub + 1):
            f[(5, i)] = R.below(128)
            f[(7, i)] = R.below(2)
            diff = rate == "full" and i % 4 == 0
            f[(6, i)] = R.below(3) + 2 if diff else R.below(4) + 4
        return (4, pack("PACK_FULL", f, 34)) if rate == "full" else (3, pack("PACK_HALF", f, 16))
    if rate == "quarter":
        g = R.below(4) + 4
        for i in range(1, 6):
            f[(6, i)] = g
        return 2, pack("PACK_QUARTER", f, 7)
    for i in range(1, 11):
        f[(1, i)] = R.below(2)
    f[(6, 1)] = R.below(4)
    f[(8, 0)] = R.below(16)
    if all(f[(1, i)] for i in range(1, 11)):
        f[(1, 1)] = 0
    return 1, pack("PACK_EIGHTH", f, 3)


def main():
    out = sys.argv[1]
    os.makedirs(out, exist_ok=True)
    plan = []
    # talk spurts at full and half rate with unvoiced quarter-rate bits, pauses at 1/8 (no
    # blank or erasure packets: ffmpeg's QCP reader drops them, which would shift the timing)
    for spurt in range(8):
        plan += ["eighth"] * (4 + R.below(4))
        plan += ["quarter"] * 2
        plan += ["full" if spurt % 2 == 0 else "half"] * (10 + R.below(6))
    data = bytearray()
    for r in plan:
        if r == "erasure":
            data += bytes([14])
        elif r == "blank":
            data += bytes([0])
        else:
            octet, payload = frame(r)
            data += bytes([octet]) + payload
    guid = bytes.fromhex("416d7f5e15b1d011ba9100805fb4b97e")
    fmt = struct.pack("<BB16sH80sHHHHHI", 1, 0, guid, 2, b"Qcelp 13K", 14000, 35, 160, 8000, 16, 4)
    fmt += struct.pack("<8B", 34, 4, 16, 3, 7, 2, 3, 1) + bytes(8) + bytes(20)
    vrat = struct.pack("<II", 1, len(plan))
    body = b"QLCM" + b"fmt " + struct.pack("<I", len(fmt)) + fmt
    body += b"vrat" + struct.pack("<I", len(vrat)) + vrat
    body += b"data" + struct.pack("<I", len(data)) + bytes(data) + (b"\0" if len(data) & 1 else b"")
    path = os.path.join(out, "random.qcp")
    with open(path, "wb") as f:
        f.write(b"RIFF" + struct.pack("<I", len(body)) + body)
    pcm = subprocess.run(["ffmpeg", "-hide_banner", "-loglevel", "error", "-i", path, "-f", "s16le", "-ac", "1", "-"],
                         check=True, capture_output=True).stdout
    with open(os.path.join(out, "random.s16"), "wb") as f:
        f.write(pcm)
    print(f"{len(plan)} packets, {len(pcm) // 2} samples from ffmpeg")


main()
