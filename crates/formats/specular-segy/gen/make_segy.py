#!/usr/bin/env python3
"""Writes the small SEG-Y and Seismic Unix fixtures in tests/data (numpy only; every file
under 200 KB). The bytes are packed here by hand, so that segyio (gen/oracle.py) reads them
independently.

    python3 -I make_segy.py DIR

Every trace is a synthetic stacked section: Ricker reflections on gently dipping horizons
(one cut by a fault), a little noise, so sections, slices and wiggles look like seismic.
Layout: SEG-Y rev 0, rev 1 (2002) and rev 2.0 / 2.1 (2017, 2023) as published by the SEG
Technical Standards Committee.
"""

import pathlib
import struct
import sys

import numpy as np

RNG = np.random.default_rng(20261011)


def ricker(t, f=25.0):
    a = (np.pi * f * t) ** 2
    return (1 - 2 * a) * np.exp(-a)


def section(il, xl, ns, dt_ms, amp=1000.0):
    """One trace at (il, xl): reflections at horizon times that dip with il and xl."""
    t = np.arange(ns) * dt_ms / 1000.0
    tr = np.zeros(ns)
    horizons = [(0.12, 0.8), (0.22, -1.0), (0.31, 0.6), (0.42, -0.7), (0.55, 0.9)]
    for n, (t0, r) in enumerate(horizons):
        dip = 0.0025 * il + 0.0015 * xl + 0.01 * np.sin(xl / 5.0 + n)
        if n >= 2 and xl > 9:  # a fault throws the deeper horizons down
            dip += 0.03
        tr += r * ricker(t - (t0 + dip) * (ns * dt_ms / 1000.0) / 0.6)
    tr += 0.03 * RNG.standard_normal(ns)
    return tr * amp


def ibm(values):
    """float64 → IBM single precision (big-endian u32), truncating as most writers do."""
    out = np.zeros(len(values), dtype=">u4")
    for i, v in enumerate(values):
        v = float(v)
        if v == 0.0:
            continue
        sign = 0x80000000 if v < 0 else 0
        m, e = abs(v), 64
        while m >= 1.0:
            m /= 16.0
            e += 1
        while m < 1.0 / 16.0:
            m *= 16.0
            e -= 1
        frac = int(m * (1 << 24)) & 0xFFFFFF
        out[i] = sign | (e << 24) | frac
    return out.tobytes()


ASCII_TO_CP037 = {chr(c): bytes([c]).decode("ascii").encode("cp037") for c in range(32, 127)}


def textual(lines, ebcdic, pad=True):
    cards = []
    for i in range(40):
        s = lines[i] if i < len(lines) else (f"C{i + 1:2d}" if pad else "")
        cards.append(s[:80].ljust(80))
    raw = "".join(cards)
    return raw.encode("cp037") if ebcdic else raw.encode("ascii")


def binary(e, *, dt, ns, fmt, rev=(1, 0), fixed=1, n_ext=0, job=1, line=1, sorting=4,
           measurement=1, byte_word=False, ext_ns=0, ext_dt=0.0, traces_in_file=0,
           first_trace=0, ens=1):
    b = bytearray(400)
    struct.pack_into(e + "iii", b, 0, job, line, 1)
    struct.pack_into(e + "hhHHHHhhh", b, 12, ens, 0, dt, dt, ns, ns, fmt, 1, sorting)
    struct.pack_into(e + "h", b, 54, measurement)
    if ext_ns:
        struct.pack_into(e + "i", b, 68, ext_ns)
    if ext_dt:
        struct.pack_into(e + "d", b, 72, ext_dt)
    if byte_word:
        struct.pack_into(e + "I", b, 96, 0x01020304)
    b[300], b[301] = rev
    struct.pack_into(e + "hh", b, 302, fixed, n_ext)
    if traces_in_file:
        struct.pack_into(e + "Q", b, 312, traces_in_file)
    if first_trace:
        struct.pack_into(e + "Q", b, 320, first_trace)
    return bytes(b)


def trace_header(e, *, seq, ns, dt, il=0, xl=0, cdp=0, ffid=0, chan=0, ep=0, x=0.0, y=0.0,
                 scalar=-100, delay=0, offset=0):
    h = bytearray(240)
    sc = abs(scalar) if scalar < 0 else 1
    struct.pack_into(e + "iiiiiii", h, 0, seq, seq, ffid, chan, ep, cdp, 1)
    struct.pack_into(e + "h", h, 28, 1)
    struct.pack_into(e + "i", h, 36, offset)
    struct.pack_into(e + "hh", h, 68, 1, scalar)
    struct.pack_into(e + "ii", h, 72, round(x * sc), round(y * sc))
    struct.pack_into(e + "h", h, 88, 1)
    struct.pack_into(e + "h", h, 108, delay)
    struct.pack_into(e + "HH", h, 114, ns, dt)
    struct.pack_into(e + "hhhhh", h, 156, 2026, 284, 12, 0, 0)
    struct.pack_into(e + "ii", h, 180, round(x * sc), round(y * sc))
    struct.pack_into(e + "ii", h, 188, il, xl)
    struct.pack_into(e + "i", h, 196, cdp)
    return bytes(h)


def samples(e, fmt, v):
    if fmt == 1:
        return ibm(v) if e == ">" else np.frombuffer(ibm(v), ">u4").astype("<u4").tobytes()
    codes = {2: "i4", 3: "i2", 5: "f4", 6: "f8", 8: "i1", 9: "i8", 10: "u4", 11: "u2",
             12: "u8", 16: "u1"}
    if fmt in codes:
        dt = np.dtype(e + codes[fmt])
        if dt.kind in "iu":
            info = np.iinfo(dt)
            v = np.clip(np.round(v), info.min, info.max)
        return np.asarray(v).astype(dt).tobytes()
    if fmt in (7, 15):
        n = np.clip(np.round(v), -(1 << 23) if fmt == 7 else 0,
                    (1 << 23) - 1 if fmt == 7 else (1 << 24) - 1).astype(np.int64)
        u = (n & 0xFFFFFF).astype(np.uint32)
        b = np.stack([(u >> 16) & 0xFF, (u >> 8) & 0xFF, u & 0xFF], axis=1).astype(np.uint8)
        if e == "<":
            b = b[:, ::-1]
        return b.tobytes()
    raise ValueError(fmt)


def text_lines(title, extra=()):
    lines = [
        f"C 1 SPECULAR TEST FIXTURE: {title}",
        "C 2 SYNTHETIC STACKED DATA, NOT A REAL SURVEY",
        "C 3 INLINE BYTES 189-192  CROSSLINE BYTES 193-196",
        "C 4 CDP X BYTES 181-184  CDP Y BYTES 185-188  SCALAR -100",
        "C 5 SAMPLE INTERVAL IN BINARY HEADER",
    ]
    lines += list(extra)
    lines += [f"C{i:2d}" for i in range(len(lines) + 1, 40)]
    lines.append("C40 END TEXTUAL HEADER")
    return lines


def write_segy(path, *, e=">", fmt, dt, ns, cells, ebcdic=True, rev=(1, 0), title="",
               ext=(), n_ext=None, lengths=None, byte_word=False, delay=0, key="std",
               amp=1000.0, trace_count=False, extra_text=()):
    out = bytearray(textual(text_lines(title, extra_text), ebcdic))
    n_ext_field = len(ext) if n_ext is None else n_ext
    first = 3600 + 3200 * len(ext) if trace_count else 0
    out += binary(e, dt=dt, ns=ns, fmt=fmt, rev=rev, fixed=0 if lengths else 1,
                  n_ext=n_ext_field, byte_word=byte_word,
                  traces_in_file=len(cells) if trace_count else 0, first_trace=first)
    for stanza in ext:
        out += textual(stanza, ebcdic, pad=False)
    for seq, (il, xl) in enumerate(cells, start=1):
        n = lengths[seq - 1] if lengths else ns
        x = 620000.0 + 12.5 * xl + 3.75 * il
        y = 6080000.0 + 25.0 * il - 2.5 * xl
        kw = dict(seq=seq, ns=n, dt=dt, x=x, y=y, delay=delay, cdp=1000 + seq)
        if key == "std":
            kw.update(il=il, xl=xl)
        elif key == "9/21":
            kw.update(ffid=il, cdp=xl)
        elif key == "17/13":
            kw.update(ep=il, chan=xl)
        elif key == "line":
            kw.update(cdp=xl)
        out += trace_header(e, **kw)
        v = section(il, xl, n, dt / 1000.0, amp)
        if fmt in (10, 11, 12, 15, 16):  # unsigned: lifted above zero
            v = v + 1.5 * amp
        out += samples(e, fmt, v)
    pathlib.Path(path).write_bytes(out)
    assert len(out) < 200_000, (path, len(out))


def write_su(path, *, e, ns, dt, n):
    out = bytearray()
    for seq in range(1, n + 1):
        out += trace_header(e, seq=seq, ns=ns, dt=dt, cdp=200 + seq, x=1000.0 + 25 * seq,
                            y=2000.0)
        out += samples(e, 5, section(1, seq, ns, dt / 1000.0, 1.0))
    pathlib.Path(path).write_bytes(out)
    assert len(out) < 200_000


def grid(il, xl):
    return [(i, x) for i in il for x in xl]


def main(d):
    d = pathlib.Path(d)
    d.mkdir(parents=True, exist_ok=True)
    # 3-D volume, IBM float, big-endian, EBCDIC, rev 1.
    write_segy(d / "ibm_volume.sgy", fmt=1, dt=4000, ns=120,
               cells=grid(range(100, 108), range(300, 320, 2)), title="IBM FLOAT 3-D, REV 1")
    # 2-D line, int16, rev 0.
    write_segy(d / "int16_line.sgy", fmt=3, dt=2000, ns=250, rev=(0, 0), key="line",
               cells=[(0, 2000 + i) for i in range(150)], title="INT16 2-D LINE, REV 0",
               delay=-20, amp=8000.0)
    # 3-D, little-endian rev 2.0 with the byte-order word, ASCII, two extended stanzas,
    # trace count and first-trace offset stated.
    ext = [["((SEG: Specular test extended header))", "PROCESSING: NONE"],
           ["((SEG: EndText))"]]
    write_segy(d / "le_rev2.sgy", e="<", fmt=5, dt=4000, ns=100, rev=(2, 0), ebcdic=False,
               byte_word=True, ext=ext, cells=grid(range(1, 7), range(1, 8)),
               title="LITTLE-ENDIAN REV 2.0", trace_count=True)
    # 3-D with gaps: an irregular outline, a missing inline (inline step 2), and crossline-
    # major trace order.
    cells = []
    for xl in range(500, 514):
        for il in range(10, 34, 2):
            if il == 20:
                continue
            if xl - 500 + (il - 10) // 2 < 3 or (il > 26 and xl > 509):
                continue
            cells.append((il, xl))
    write_segy(d / "gaps_volume.sgy", fmt=5, dt=4000, ns=80, cells=cells,
               title="IEEE FLOAT 3-D WITH GAPS, CROSSLINE SORTED", delay=100)
    # 3-D with the keys in the older places.
    write_segy(d / "keys_9_21.sgy", fmt=2, dt=4000, ns=60, rev=(0, 0), key="9/21",
               cells=grid(range(40, 46), range(1, 9)), title="INT32, KEYS IN 9 AND 21",
               amp=100000.0)
    write_segy(d / "keys_17_13.sgy", fmt=8, dt=4000, ns=60, key="17/13",
               cells=grid(range(7, 12), range(1, 7)), title="INT8, KEYS IN 17 AND 13",
               amp=100.0)
    # The other sample formats, small; rev 2.1 for the rev 2 ones.
    for code, name, amp in [(6, "float64", 1000.0), (7, "int24", 1e6), (9, "int64", 1e9),
                            (10, "uint32", 1e6), (11, "uint16", 1e4), (12, "uint64", 1e9),
                            (15, "uint24", 1e6), (16, "uint8", 100.0)]:
        cells = grid(range(1, 4), range(1, 5))
        path = d / f"fmt_{name}.sgy"
        write_segy(path, fmt=code, dt=4000, ns=40, rev=(2, 1), cells=cells,
                   title=f"SAMPLE FORMAT {code} ({name.upper()})", amp=amp,
                   e="<" if code in (7, 12) else ">", byte_word=code in (7, 12))
    # Depth data: a card says so.
    write_segy(d / "depth_volume.sgy", fmt=5, dt=5000, ns=50, cells=grid(range(1, 4),
               range(1, 4)), title="DEPTH", extra_text=["C 6 DEPTH DOMAIN, METRES"])
    # Refusals.
    lengths = [60] * 5 + [64] + [60] * 6
    write_segy(d / "variable.sgy", fmt=5, dt=4000, ns=60, cells=grid(range(1, 4), range(1, 5)),
               lengths=lengths, title="VARIABLE TRACE LENGTH")
    # Seismic Unix, both byte orders.
    write_su(d / "line_le.su", e="<", ns=150, dt=4000, n=60)
    write_su(d / "line_be.su", e=">", ns=100, dt=2000, n=40)


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "tests/data")
