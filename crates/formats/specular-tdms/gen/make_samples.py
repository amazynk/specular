#!/usr/bin/env python3
"""TDMS samples for specular-tdms, with nptdms as the oracle.

    make_samples.py fixtures DIR        the committed fixtures: DIR/*.tdms, DIR/expected/*.json
    make_samples.py corpus DIR N [SEED] a random corpus: DIR/*.tdms and DIR/*.tdms.json

Most files are built byte by byte here (big-endian, interleaved, carried-over object lists,
truncated segments, DAQmx raw data: things nptdms cannot write); one is written by nptdms's
own writer. Expected values are what nptdms reads (raw timestamps, DAQmx raw scaler data
too). Run with the oracle venv's python: `python -I make_samples.py ...`.
"""

import datetime
import itertools
import json
import math
import os
import random
import struct
import sys

import numpy as np
from nptdms import ChannelObject, GroupObject, RootObject, TdmsFile, TdmsWriter

TOC_META, TOC_NEW, TOC_RAW, TOC_IL, TOC_BE, TOC_DAQ = 2, 4, 8, 32, 64, 128
CODES = {
    "i8": 1, "i16": 2, "i32": 3, "i64": 4, "u8": 5, "u16": 6, "u32": 7, "u64": 8,
    "f32": 9, "f64": 10, "str": 0x20, "bool": 0x21, "time": 0x44,
    "c64": 0x08000C, "c128": 0x10000D,
}
FMT = {"i8": "b", "i16": "h", "i32": "i", "i64": "q", "u8": "B", "u16": "H", "u32": "I",
       "u64": "Q", "f32": "f", "f64": "d", "bool": "B"}
SIZE = {"i8": 1, "u8": 1, "bool": 1, "i16": 2, "u16": 2, "i32": 4, "u32": 4, "f32": 4,
        "i64": 8, "u64": 8, "f64": 8, "c64": 8, "time": 16, "c128": 16}
DAQ = {"u8": 0, "i8": 1, "u16": 2, "i16": 3, "u32": 4, "i32": 5, "u64": 6, "i64": 7,
       "f32": 8, "f64": 9}
LIMITS = {"i8": (-128, 127), "u8": (0, 255), "i16": (-32768, 32767), "u16": (0, 65535),
          "i32": (-2**31, 2**31 - 1), "u32": (0, 2**32 - 1), "i64": (-2**63, 2**63 - 1),
          "u64": (0, 2**64 - 1)}


class P:
    """Packs in a segment's byte order."""

    def __init__(self, be):
        self.be = be
        self.e = ">" if be else "<"

    def __call__(self, f, *v):
        return struct.pack(self.e + f, *v)

    def str(self, t):
        b = t.encode()
        return self("I", len(b)) + b


def value(p, t, v):
    if t in FMT:
        return p(FMT[t], int(v) if t != "f32" and t != "f64" else v)
    if t == "time":
        secs, frac = v
        return p("qQ", secs, frac) if p.be else p("Qq", frac, secs)
    if t == "c64":
        return p("ff", v.real, v.imag)
    if t == "c128":
        return p("dd", v.real, v.imag)
    raise ValueError(t)


def prop(p, name, t, v):
    out = p.str(name)
    if t == "str":
        return out + p("I", 0x20) + p.str(v)
    return out + p("I", CODES[t]) + value(p, t, v)


def obj(p, path, index, props=()):
    out = p.str(path)
    if index is None:
        out += p("I", 0xFFFFFFFF)
    elif index == "same":
        out += p("I", 0)
    elif index[0] == "plain":
        _, t, count, total = index
        if t == "str":
            out += p("I", 28) + p("IIQQ", CODES[t], 1, count, total)
        else:
            out += p("I", 20) + p("IIQ", CODES[t], 1, count)
    else:
        _, digital, count, scalers, widths = index
        out += p("I", 0x126A if digital else 0x1269) + p("IIQ", 0xFFFFFFFF, 1, count)
        out += p("I", len(scalers))
        for dt, buf, off, sid in scalers:
            out += p("IIIBI" if digital else "IIIII", dt, buf, off, 0, sid)
        out += p("I", len(widths)) + b"".join(p("I", w) for w in widths)
    props = list(props)
    return out + p("I", len(props)) + b"".join(prop(p, *q) for q in props)


def segment(be, objs, raw=b"", toc=0, version=4713, next_off=None, new=True):
    p = P(be)
    meta = b""
    if objs is not None:
        toc |= TOC_META
        if new:
            toc |= TOC_NEW
        meta = p("I", len(objs)) + b"".join(obj(p, *o) for o in objs)
    if raw:
        toc |= TOC_RAW
    if be:
        toc |= TOC_BE
    n = len(meta) + len(raw) if next_off is None else next_off
    return b"TDSm" + struct.pack("<I", toc) + p("IQQ", version, n, len(meta)) + meta + raw


def enc(p, t, vals):
    if t == "str":
        bs = [v.encode() for v in vals]
        ends = itertools.accumulate(len(b) for b in bs)
        return b"".join(p("I", e) for e in ends) + b"".join(bs)
    return b"".join(value(p, t, v) for v in vals)


def str_total(vals):
    return 4 * len(vals) + sum(len(v.encode()) for v in vals)


def plain(t, vals):
    return ("plain", t, len(vals), str_total(vals) if t == "str" else 0)


def interleave(p, cols):
    n = len(cols[0][1])
    return b"".join(value(p, t, v[i]) for i in range(n) for (t, v) in cols)


def daqmx_raw(be, n, widths, scalers):
    """Raw buffers of `n` rows; `scalers` are (buffer, byte offset, type, values)."""
    p = P(be)
    out = b""
    for b, w in enumerate(widths):
        rows = [bytearray(w) for _ in range(n)]
        for buf, off, t, vals in scalers:
            if buf != b:
                continue
            for i, v in enumerate(vals):
                bs = value(p, t, v)
                rows[i][off:off + len(bs)] = bs
        out += b"".join(bytes(r) for r in rows)
    return out


def rand_vals(r, t, n):
    if t in LIMITS:
        lo, hi = LIMITS[t]
        return [r.randint(lo, hi) for _ in range(n)]
    if t == "f32":
        return [struct.unpack("<f", struct.pack("<f", r.uniform(-1e6, 1e6)))[0] for _ in range(n)]
    if t == "f64":
        return [r.uniform(-1e12, 1e12) * (10 ** r.randint(-30, 30)) for _ in range(n)]
    if t == "bool":
        return [r.randint(0, 1) for _ in range(n)]
    if t == "time":
        return [(r.randint(3_000_000_000, 3_900_000_000), r.getrandbits(64)) for _ in range(n)]
    if t == "c64":
        f = lambda: struct.unpack("<f", struct.pack("<f", r.uniform(-1e3, 1e3)))[0]
        return [complex(f(), f()) for _ in range(n)]
    if t == "c128":
        return [complex(r.uniform(-1e9, 1e9), r.uniform(-1e9, 1e9)) for _ in range(n)]
    if t == "str":
        return ["".join(r.choice("abcdé ✓xyz'") for _ in range(r.randint(0, 9))) for _ in range(n)]
    raise ValueError(t)


# ----- the oracle -----------------------------------------------------------------------

def jprop(v):
    if isinstance(v, (bool, np.bool_)):
        return {"b": bool(v)}
    if isinstance(v, (int, np.integer)):
        return {"i": int(v)}
    if isinstance(v, (float, np.floating)):
        return {"f": jfloat(v)}
    if isinstance(v, str):
        return {"s": v}
    if isinstance(v, (complex, np.complexfloating)):
        return {"c": [jfloat(v.real), jfloat(v.imag)]}
    if hasattr(v, "seconds") and hasattr(v, "second_fractions"):
        return {"t": [int(v.seconds), int(v.second_fractions)]}
    raise TypeError(type(v))


def jfloat(x):
    x = float(x)
    if math.isnan(x):
        return None
    if math.isinf(x):
        return "inf" if x > 0 else "-inf"
    return x


def floats(a):
    return [jfloat(x) for x in a]


def jvalues(a):
    if hasattr(a, "seconds") and hasattr(a, "second_fractions"):
        return [int(s) + int(f) / 2**64 for s, f in zip(a.seconds, a.second_fractions)]
    a = np.asarray(a)
    if a.dtype.kind == "c":
        return floats(np.abs(a))
    if a.dtype.kind in "iub":
        return [int(x) for x in a]
    return floats(a.astype(np.float64))


REFUSED = ("RTD", "Strain", "Thermistor", "Thermocouple")


def expected(path):
    f = TdmsFile.read(path, raw_timestamps=True)
    out = {"props": {k: jprop(v) for k, v in f.properties.items()}, "groups": []}
    for g in f.groups():
        go = {"name": g.name, "props": {k: jprop(v) for k, v in g.properties.items()},
              "channels": []}
        for c in g.channels():
            co = {"name": c.name, "len": len(c),
                  "props": {k: jprop(v) for k, v in c.properties.items()}}
            types = [str(v) for k, v in itertools.chain(
                c.properties.items(), g.properties.items(), f.properties.items())
                if k.endswith("_Scale_Type")]
            if any(t in REFUSED for t in types):
                co["refused"] = True
            if c.data_type is not None and c.data_type.__name__ == "String":
                co["strings"] = [str(s) for s in c[:]]
            elif not co.get("refused"):
                try:
                    co["values"] = jvalues(c[:])
                except Exception as e:  # DAQmx data without scaling
                    co["values_error"] = str(e)
                raw = c.read_data(scaled=False)
                if isinstance(raw, dict):
                    co["scalers"] = {str(k): jvalues(v) for k, v in raw.items()}
                else:
                    a = np.asarray(raw) if not hasattr(raw, "seconds") else None
                    if a is not None and a.dtype.kind == "c":
                        co["re"] = floats(a.real)
                        co["im"] = floats(a.imag)
                    co["raw"] = jvalues(raw)
            go["channels"].append(co)
        out["groups"].append(go)
    return out


def write(path, data, exp_path):
    with open(path, "wb") as fh:
        fh.write(data)
    e = expected(path)
    with open(exp_path, "w") as fh:
        json.dump(e, fh, separators=(",", ":"), ensure_ascii=False)


# ----- fixtures -------------------------------------------------------------------------

def fx_writer(path):
    root = RootObject(properties={
        "title": "nptdms writer", "count": np.int32(-7), "big": np.uint64(2**63 + 5),
        "ratio": 0.125, "flag": True,
        "when": np.datetime64("2024-05-01T12:00:00.250", "us"),
    })
    group = GroupObject("Measured", properties={"operator": "Ada", "small": np.int8(-3)})
    chans = []
    r = random.Random(1)
    for t, dt in [("i8", np.int8), ("i16", np.int16), ("i32", np.int32), ("i64", np.int64),
                  ("u8", np.uint8), ("u16", np.uint16), ("u32", np.uint32),
                  ("u64", np.uint64), ("f32", np.float32), ("f64", np.float64)]:
        chans.append(ChannelObject("Measured", t, np.array(rand_vals(r, t, 17), dtype=dt),
                                   properties={"unit_string": "V", "wf_increment": 0.001,
                                               "wf_start_offset": 0.5,
                                               "wf_xunit_string": "s"}))
    chans.append(ChannelObject("Measured", "bool", np.array([True, False, True, True])))
    chans.append(ChannelObject("Measured", "c64", np.array([1 + 2j, -3.5 + 0.25j],
                                                           dtype=np.complex64)))
    chans.append(ChannelObject("Measured", "c128", np.array([3 + 4j, 1e300 - 1e300j])))
    chans.append(ChannelObject("Measured", "text", ["alpha", "", "gämma ✓"]))
    chans.append(ChannelObject("Measured", "time", np.array(
        ["2020-02-29T23:59:59.999999", "1904-01-01T00:00:00"], dtype="datetime64[us]")))
    with TdmsWriter(path) as w:
        w.write_segment([root, group] + chans)
        w.write_segment([c for c in chans if c.channel not in ("text",)])


def fx_interleaved():
    r = random.Random(2)
    cols = [("i16", rand_vals(r, "i16", 11)) for _ in range(4)]
    objs = [("/'IL'", None)] + [(f"/'IL'/'ch{k}'", plain("i16", cols[k][1])) for k in range(4)]
    p = P(False)
    d = segment(False, objs, interleave(p, cols), toc=TOC_IL)
    cols2 = [("i16", rand_vals(r, "i16", 11)) for _ in range(4)]
    # No metadata: the previous layout again, here with two chunks and a partial one.
    raw = interleave(p, cols2) * 2 + interleave(p, cols2)[:8 * 3 + 2]
    d += segment(False, None, raw, toc=TOC_IL)
    return d


def fx_big_endian():
    r = random.Random(3)
    p = P(True)
    types = ["f64", "i32", "u16", "str", "time", "c64", "bool", "i64", "f32"]
    vals = {t: rand_vals(r, t, 6) for t in types}
    props = [("s", "str", "Größe"), ("i", "i32", -5), ("u", "u64", 2**64 - 1),
             ("f", "f32", 1.5), ("d", "f64", -2.25e-300), ("b", "bool", 1),
             ("t", "time", (3_700_000_000, 2**63)),
             ("k", "i8", -128), ("w", "u16", 65535)]
    objs = [("/", None, props), ("/'BE'", None, [("note", "str", "big-endian")])]
    objs += [(f"/'BE'/'{t}'", plain(t, vals[t])) for t in types]
    d = segment(True, objs, b"".join(enc(p, t, vals[t]) for t in types))
    vals2 = {t: rand_vals(r, t, 6) for t in types}
    objs2 = [(f"/'BE'/'{t}'", plain(t, vals2[t]) if t == "str" else "same") for t in types]
    d += segment(True, objs2, b"".join(enc(p, t, vals2[t]) for t in types), new=False)
    return d


def fx_carry():
    r = random.Random(4)
    p = P(False)
    a, b = rand_vals(r, "f32", 4), rand_vals(r, "i8", 3)
    d = segment(False, [("/'G'", None), ("/'G'/'a'", plain("f32", a)),
                        ("/'G'/'b'", plain("i8", b))], enc(p, "f32", a) + enc(p, "i8", b))
    a2, b2 = rand_vals(r, "f32", 4), rand_vals(r, "i8", 3)
    d += segment(False, None, enc(p, "f32", a2) + enc(p, "i8", b2))
    a3, c3 = rand_vals(r, "f32", 4), rand_vals(r, "u32", 5)
    d += segment(False, [("/'G'/'b'", None), ("/'G'/'c'", plain("u32", c3), [("x", "f64", 2.0)]),
                         ("/'G'/'a'", "same", [("late", "str", "yes")])],
                 enc(p, "f32", a3) + enc(p, "u32", c3), new=False)
    c4, b4 = rand_vals(r, "u32", 2), rand_vals(r, "i8", 3)
    d += segment(False, [("/'G'/'c'", plain("u32", c4)), ("/'G'/'b'", "same")],
                 enc(p, "u32", c4) + enc(p, "i8", b4))
    # Properties only.
    d += segment(False, [("/'G'", None, [("closed", "bool", 1)])], new=False)
    return d


def fx_chunks():
    r = random.Random(5)
    p = P(False)
    x, y, s = rand_vals(r, "i32", 5), rand_vals(r, "f64", 3), rand_vals(r, "str", 2)
    objs = [("/'C'/'x'", plain("i32", x)), ("/'C'/'y'", plain("f64", y)),
            ("/'C'/'s'", plain("str", s))]
    chunk = enc(p, "i32", x) + enc(p, "f64", y) + enc(p, "str", s)
    raw = chunk * 3 + chunk[:20 + 8 * 2 + 3]
    return segment(False, objs, raw)


def fx_incomplete():
    r = random.Random(6)
    p = P(False)
    x, y = rand_vals(r, "u16", 8), rand_vals(r, "f64", 8)
    objs = [("/'T'/'x'", plain("u16", x)), ("/'T'/'y'", plain("f64", y))]
    d = segment(False, objs, enc(p, "u16", x) + enc(p, "f64", y))
    x2, y2 = rand_vals(r, "u16", 8), rand_vals(r, "f64", 8)
    raw = (enc(p, "u16", x2) + enc(p, "f64", y2)) * 2
    d += segment(False, None, raw[:16 + 64 + 16 + 20], next_off=0xFFFFFFFFFFFFFFFF)
    return d


def fx_v4712():
    p = P(False)
    v = [1.0, 2.0, 3.5]
    return segment(False, [("/'Old'/'v'", plain("f64", v), [("unit_string", "str", "m")])],
                   enc(p, "f64", v), version=4712)


def fx_escaped():
    p = P(False)
    v = [7, 8, 9]
    return segment(False, [("/'it''s'", None, [("q", "str", "'quoted'")]),
                           ("/'it''s'/'a/b ''c'''", plain("u8", v))], enc(p, "u8", v))


def scale_linear(i, slope, intercept, src):
    return [(f"NI_Scale[{i}]_Scale_Type", "str", "Linear"),
            (f"NI_Scale[{i}]_Linear_Slope", "f64", slope),
            (f"NI_Scale[{i}]_Linear_Y_Intercept", "f64", intercept),
            (f"NI_Scale[{i}]_Linear_Input_Source", "u32", src)]


def scale_poly(i, coef, src):
    out = [(f"NI_Scale[{i}]_Scale_Type", "str", "Polynomial"),
           (f"NI_Scale[{i}]_Polynomial_Coefficients_Size", "u32", len(coef)),
           (f"NI_Scale[{i}]_Polynomial_Input_Source", "u32", src)]
    return out + [(f"NI_Scale[{i}]_Polynomial_Coefficients[{k}]", "f64", c)
                  for k, c in enumerate(coef)]


def scale_table(i, xs, ys, src):
    out = [(f"NI_Scale[{i}]_Scale_Type", "str", "Table"),
           (f"NI_Scale[{i}]_Table_Pre_Scaled_Values_Size", "u32", len(xs)),
           (f"NI_Scale[{i}]_Table_Scaled_Values_Size", "u32", len(ys)),
           (f"NI_Scale[{i}]_Table_Input_Source", "u32", src)]
    out += [(f"NI_Scale[{i}]_Table_Pre_Scaled_Values[{k}]", "f64", x) for k, x in enumerate(xs)]
    return out + [(f"NI_Scale[{i}]_Table_Scaled_Values[{k}]", "f64", y) for k, y in enumerate(ys)]


def fx_daqmx():
    r = random.Random(7)
    n = 9
    a, b = rand_vals(r, "i16", n), rand_vals(r, "i32", n)
    sc = [(0, 0, "i16", a), (0, 2, "i32", b)]
    ia = ("daqmx", False, n, [(DAQ["i16"], 0, 0, 0)], [6])
    ib = ("daqmx", False, n, [(DAQ["i32"], 0, 2, 0)], [6])
    pa = [("NI_Number_Of_Scales", "u32", 2)] + scale_linear(1, 3.0517578125e-4, -0.5, 0)
    pb = [("NI_Number_Of_Scales", "u32", 2)] + scale_poly(1, [0.25, 1e-6, -2e-12], 0)
    d = segment(False, [("/'Dev'", None), ("/'Dev'/'ai0'", ia, pa), ("/'Dev'/'ai1'", ib, pb)],
                daqmx_raw(False, n, [6], sc), toc=TOC_DAQ)
    a2, b2 = rand_vals(r, "i16", n), rand_vals(r, "i32", n)
    d += segment(False, [("/'Dev'/'ai0'", "same"), ("/'Dev'/'ai1'", "same")],
                 daqmx_raw(False, n, [6], [(0, 0, "i16", a2), (0, 2, "i32", b2)]),
                 toc=TOC_DAQ, new=False)
    return d


def fx_daqmx_multi():
    r = random.Random(8)
    n = 7
    a0, a1, c = rand_vals(r, "i16", n), rand_vals(r, "u16", n), rand_vals(r, "i16", n)
    i0 = ("daqmx", False, n, [(DAQ["i16"], 0, 0, 0), (DAQ["u16"], 0, 2, 1)], [4, 2])
    i1 = ("daqmx", False, n, [(DAQ["i16"], 1, 0, 0)], [4, 2])
    p0 = [("NI_Number_Of_Scales", "u32", 3),
          ("NI_Scale[2]_Scale_Type", "str", "Add"),
          ("NI_Scale[2]_Add_Left_Operand_Input_Source", "u32", 0),
          ("NI_Scale[2]_Add_Right_Operand_Input_Source", "u32", 1)]
    p1 = [("NI_Number_Of_Scales", "u32", 2)] + scale_table(1, [-1000.0, 0.0, 2500.0],
                                                           [10.0, 0.0, -5.0], 0)
    raw = daqmx_raw(False, n, [4, 2], [(0, 0, "i16", a0), (0, 2, "u16", a1), (1, 0, "i16", c)])
    d = segment(False, [("/'M'/'pair'", i0, p0), ("/'M'/'table'", i1, p1)], raw, toc=TOC_DAQ)
    # Cut short inside the second buffer.
    d += segment(False, None, raw[:4 * n + 2 * 3 + 1], toc=TOC_DAQ,
                 next_off=0xFFFFFFFFFFFFFFFF)
    return d


def fx_daqmx_digital():
    r = random.Random(9)
    n = 12
    bits = [r.randint(0, 255) for _ in range(n)]
    lines = []
    for k, bit in enumerate([0, 1, 5, 9]):
        lines.append((f"/'Dig'/'line{k}'",
                      ("daqmx", True, n, [(DAQ["u8"], 0, bit, 0)], [2]),
                      [("NI_Number_Of_Scales", "u32", 1)]))
    rows = b"".join(bytes([v, v ^ 0x5A]) for v in bits)
    return segment(True, lines, rows, toc=TOC_DAQ)


def fx_scaled():
    r = random.Random(10)
    p = P(False)
    v = rand_vals(r, "i16", 10)
    gprops = [("NI_Number_Of_Scales", "u32", 1)] + scale_linear(0, 0.5, 10.0, 0xFFFFFFFF)
    objs = [("/'S'", None, gprops),
            ("/'S'/'group_scaled'", plain("i16", v)),
            ("/'S'/'already'", plain("i16", v), [("NI_Scaling_Status", "str", "scaled"),
                                                ("NI_Number_Of_Scales", "u32", 1)]
             + scale_linear(0, 2.0, 0.0, 0xFFFFFFFF)),
            ("/'S'/'poly'", plain("i16", v), scale_poly(0, [1.0, 0.5, 0.25], 0xFFFFFFFF)),
            ("/'S'/'chain'", plain("i16", v), [("NI_Number_Of_Scales", "u32", 2)]
             + scale_linear(0, 2.0, 1.0, 0xFFFFFFFF) + scale_linear(1, -1.0, 3.0, 0)),
            ("/'S'/'thermo'", plain("i16", v), [("NI_Number_Of_Scales", "u32", 1),
                                                ("NI_Scale[0]_Scale_Type", "str", "Thermocouple"),
                                                ("NI_Scale[0]_Thermocouple_Input_Source", "u32",
                                                 0xFFFFFFFF)])]
    return segment(False, objs, enc(p, "i16", v) * 5)


FIXTURES = {
    "interleaved": fx_interleaved, "big_endian": fx_big_endian, "carry": fx_carry,
    "chunks": fx_chunks, "incomplete": fx_incomplete, "v4712": fx_v4712,
    "escaped": fx_escaped, "daqmx": fx_daqmx, "daqmx_multi": fx_daqmx_multi,
    "daqmx_digital": fx_daqmx_digital, "scaled": fx_scaled,
}


def fixtures(d):
    os.makedirs(os.path.join(d, "expected"), exist_ok=True)
    path = os.path.join(d, "writer.tdms")
    fx_writer(path)
    with open(path, "rb") as fh:
        data = fh.read()
    write(path, data, os.path.join(d, "expected", "writer.json"))
    for name, f in FIXTURES.items():
        write(os.path.join(d, name + ".tdms"), f(), os.path.join(d, "expected", name + ".json"))


# ----- random corpus --------------------------------------------------------------------

NUMERIC = ["i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64", "f32", "f64", "bool",
           "time", "c64", "c128"]


def random_file(r):
    be = r.random() < 0.3
    p = P(be)
    out = b""
    daq = r.random() < 0.25
    nch = r.randint(1, 6)
    names = [f"/'g{r.randint(0, 2)}'/'c{k}'" for k in range(nch)]
    if daq:
        widths = [r.choice([2, 4, 6, 8, 12]) for _ in range(r.randint(1, 2))]
        chans = []
        for name in names:
            b = r.randrange(len(widths))
            t = r.choice([t for t in DAQ if SIZE[t] <= widths[b]])
            off = r.randint(0, widths[b] - SIZE[t])
            chans.append((name, b, off, t))
        for _ in range(r.randint(1, 4)):
            n = r.randint(1, 20)
            sc, objs = [], []
            for name, b, off, t in chans:
                vals = rand_vals(r, t, n)
                if t in ("f32", "f64"):
                    vals = [float(v) for v in vals]
                sc.append((b, off, t, vals))
                # numpy keeps float32 data in float32 through a linear scale (NI scales in
                # double): float32 scalers get a polynomial, which nptdms takes in double.
                if t == "f32":
                    scale = scale_poly(1, [r.uniform(-10, 10), r.uniform(-2, 2)], 0)
                else:
                    scale = scale_linear(1, r.uniform(-2, 2), r.uniform(-10, 10), 0)
                props = [("NI_Number_Of_Scales", "u32", 2)] + scale
                objs.append((name, ("daqmx", False, n, [(DAQ[t], b, off, 0)], widths), props))
            raw = daqmx_raw(be, n, widths, sc)
            reps = r.randint(1, 3)
            out += segment(be, objs, raw * reps, toc=TOC_DAQ)
        return out
    types = {name: r.choice(NUMERIC + ["str"]) for name in names}
    il = r.random() < 0.3
    if il:
        for name in names:
            if types[name] == "str":
                types[name] = "f64"
    prev_n = None
    for seg in range(r.randint(1, 5)):
        n = r.randint(0, 25) if not il else r.randint(1, 25)
        # A segment without metadata reuses the string sizes too: only without strings.
        reuse = (seg > 0 and r.random() < 0.3 and prev_n is not None
                 and "str" not in types.values())
        if reuse:
            n = prev_n
        cols = [(types[nm], rand_vals(r, types[nm], n)) for nm in names]
        if il:
            raw = interleave(p, cols)
        else:
            raw = b"".join(enc(p, t, v) for t, v in cols)
        if not reuse:
            objs = [(nm, plain(t, v), [("seg", "u32", seg)]) for nm, (t, v) in zip(names, cols)]
        reps = 1 if any(t == "str" for t, _ in cols) else r.randint(1, 3)
        raw = raw * reps
        last = seg == 4 or r.random() < 0.2
        cut = last and r.random() < 0.5 and len(raw) > 2
        if cut:
            raw = raw[:r.randint(1, len(raw) - 1)]
        out += segment(be, None if reuse else objs, raw, toc=TOC_IL if il else 0,
                       next_off=0xFFFFFFFFFFFFFFFF if cut else None)
        prev_n = n
        if last:
            break
    return out


def corpus(d, count, seed):
    os.makedirs(d, exist_ok=True)
    r = random.Random(seed)
    ok = skipped = 0
    for i in range(count):
        path = os.path.join(d, f"r{i:04}.tdms")
        data = random_file(r)
        try:
            write(path, data, path + ".json")
            ok += 1
        except Exception as e:  # nptdms refused it: leave it out
            skipped += 1
            print(f"skip {path}: {e}", file=sys.stderr)
            if os.path.exists(path + ".json"):
                os.remove(path + ".json")
            # Kept aside: the reader must still take them without failing badly.
            os.makedirs(os.path.join(d, "refused"), exist_ok=True)
            os.replace(path, os.path.join(d, "refused", os.path.basename(path)))
    print(f"{ok} files written, {skipped} skipped")


if __name__ == "__main__":
    if sys.argv[1] == "fixtures":
        fixtures(sys.argv[2])
    else:
        corpus(sys.argv[2], int(sys.argv[3]), int(sys.argv[4]) if len(sys.argv) > 4 else 1)
