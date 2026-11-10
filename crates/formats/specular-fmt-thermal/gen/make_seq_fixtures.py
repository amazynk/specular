#!/usr/bin/env python3
"""Writes the synthetic FLIR sequence fixtures in tests/data for specular-fmt-thermal.

    python3 -I make_seq_fixtures.py OUT_DIR

Needs numpy, and pyjpegls (CharLS) for the CSQ frames' JPEG-LS coding. Every file is built
here from the FFF layout; nothing is copied from a real camera file.

  seq_raw.seq   six 16 x 12 frames, raw little-endian counts, a hot spot moving right,
                40 ms apart with a recorded rate of 25 frames a second; frame 4 has its own
                Planck O; three bytes of padding between frames 2 and 3
  csq_jls.csq   four 16 x 12 frames, counts coded lossless JPEG-LS (as FLIR CSQ files are),
                no recorded rate, 1/30 s apart by their times
"""

import math
import os
import struct
import sys

import numpy as np

OUT = sys.argv[1]
os.makedirs(OUT, exist_ok=True)

W, H = 16, 12
R1, R2, B, F, O = 16951.796875, 0.0142948674, 1435.1, 1.0, -7142


def counts(t, o=O):
    return R1 / (R2 * (math.exp(B / (t + 273.15)) - F)) - o


def scene(k, o=O):
    """Counts of a 10..40 degree ramp with an 80 degree spot at column 2 + 2k."""
    y, x = np.mgrid[0:H, 0:W]
    t = 10 + 30 * (x + y * W) / (W * H - 1)
    c = min(2 + 2 * k, W - 3)
    t[4:7, c:c + 3] = 80.0 + k
    return np.round(np.vectorize(lambda v: counts(v, o))(t)).astype(np.uint16)


def camera_info(secs, ms, rate, o=O):
    e = "<"
    r = bytearray(0x9ac)

    def f(at, v):
        r[at:at + 4] = struct.pack(e + "f", v)

    r[0:2] = struct.pack(e + "H", 2)
    f(0x20, 0.95)
    f(0x24, 2.0)
    f(0x28, 22.0 + 273.15)
    f(0x2c, 19.0 + 273.15)
    f(0x30, 19.0 + 273.15)
    f(0x34, 1.0)
    f(0x3c, 0.4)
    f(0x58, R1)
    f(0x5c, B)
    f(0x60, F)
    f(0x70, 0.006569)
    f(0x74, 0.01262)
    f(0x78, -0.002276)
    f(0x7c, -0.00667)
    f(0x80, 1.9)
    f(0x90, 150 + 273.15)
    f(0x94, -20 + 273.15)
    r[0xd4:0xd4 + 15] = b"Synthetic T1000"
    r[0x170:0x170 + 5] = b"FOL25"
    f(0x1b4, 25.0)
    r[0x308:0x30c] = struct.pack(e + "i", o)
    f(0x30c, R2)
    r[0x384:0x38e] = struct.pack(e + "IIh", secs, ms, -60)
    r[0x464:0x466] = struct.pack(e + "H", rate)
    return bytes(r)


def raw_record(w, h, payload):
    return struct.pack("<HHH", 2, w, h) + bytes(26) + payload


def fff(records, creator, version):
    n = len(records)
    name = creator.encode().ljust(16, b"\0")
    head = b"FFF\0" + name + struct.pack("<III", version, 64, n) + bytes(64 - 32)
    body = b""
    entries = b""
    at = 64 + 32 * n
    for i, (ty, sub, data) in enumerate(records):
        entries += struct.pack("<HHIIIIIII", ty, sub, 100, i + 1, at + len(body), len(data), 0, 0, 0)
        body += data
    return head + entries + body


def write(name, data):
    with open(os.path.join(OUT, name), "wb") as fh:
        fh.write(data)


frames = []
for k in range(6):
    o = -7100 if k == 4 else O
    px = scene(k, o)
    frames.append(fff([(0x20, 0, camera_info(1700000000, 40 * k, 25, o)),
                       (0x01, 2, raw_record(W, H, px.astype("<u2").tobytes()))],
                      "ResearchIR", 100))
write("seq_raw.seq", b"".join(frames[:3]) + b"\0\0\0" + b"".join(frames[3:]))

import jpeg_ls  # noqa: E402 - only the CSQ needs it

frames = []
for k in range(4):
    px = scene(k)
    jls = bytes(jpeg_ls.encode(px))
    assert jls[:4] == b"\xff\xd8\xff\xf7"
    frames.append(fff([(0x20, 0, camera_info(1700000100, (1000 * k) // 30, 0)),
                       (0x01, 4, raw_record(W, H, jls))],
                      "RTP", 101))
write("csq_jls.csq", b"".join(frames))
