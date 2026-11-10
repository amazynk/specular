#!/usr/bin/env python3
"""segyio's reading of SEG-Y and Seismic Unix files, as the oracle for specular-segy
(tests/oracle.rs). segyio (LGPL) is only ever run here, from a venv; it is never linked.

    python3 -I oracle.py DIR OUT.tsv [MAX_VALUES]

Rows (tab separated):
  file   name kind endian format samples traces interval_us t0 ext_headers
  text   name card text                 (cards 1-6 and 40 of the textual header)
  hdr    name trace byte value          (every standard field of three traces)
  xy     name trace x y                 (CDP X / Y with the coordinate scalar)
  tsum   name trace sum                 (float64 sum of every trace)
  val    name trace sample value        (up to MAX_VALUES samples, strided)
  geom   name il_byte xl_byte ilines xlines offsets  (a structured 3-D open that worked)
  sect   name il_index xl_index sample value  (samples of segyio's inline sections)
  cell   name il xl trace               (unstructured 3-D: each trace's cell)
  error  name words
"""

import os
import sys

import numpy as np
import segyio

FIELDS = [1, 5, 9, 13, 17, 21, 25, 29, 37, 41, 45, 69, 71, 73, 77, 81, 85, 89, 109, 115,
          117, 157, 159, 161, 163, 165, 181, 185, 189, 193, 197, 201]
KEYS = [(189, 193), (9, 21), (17, 13)]
VALID = {1, 2, 3, 5, 6, 7, 8, 9, 10, 11, 12, 15, 16}


def fmt(x):
    return repr(float(x))


def scaled(v, s):
    if s == 0:
        return float(v)
    return float(v) * s if s > 0 else float(v) / -s


def is_ebcdic(raw):
    """More letters, digits and spaces read as EBCDIC than as ASCII."""
    def score(text, ok):
        return sum(1 for b, c in zip(raw, text) if ok(b) and (c.isascii() and c.isalnum()
                                                          or (c == " " and b != 0)))
    e = score(raw.decode("cp037"), lambda b: b >= 0x40)
    a = score(raw.decode("latin-1"), lambda b: 0x20 <= b < 0x7F)
    return e > a


def open_any(path, su, **kw):
    """segyio with the byte order whose binary header (or first trace) makes sense."""
    last = None
    for endian in ("big", "little"):
        try:
            if su:
                f = segyio.su.open(path, endian=endian, ignore_geometry=True, **kw) if not kw \
                    else segyio.su.open(path, endian=endian, **kw)
            else:
                f = segyio.open(path, "r", endian=endian, ignore_geometry=not kw, **kw)
        except Exception as e:  # noqa: BLE001
            last = e
            continue
        if su:
            if 0 < len(f.samples) < 30000 and f.tracecount > 0:
                return f, endian
        elif int(f.format) in VALID and len(f.samples) > 0:
            return f, endian
        f.close()
    raise last or RuntimeError("no byte order")


def rows_for(path, name, max_values):
    su = name.endswith(".su")
    out = []
    try:
        f, endian = open_any(path, su)
    except Exception as e:  # noqa: BLE001
        return [f"error\t{name}\t{e}".replace("\n", " ")]
    with f:
        ns, n = len(f.samples), f.tracecount
        dt = float(f.header[0][segyio.TraceField.TRACE_SAMPLE_INTERVAL]) if su \
            else float(f.bin[segyio.BinField.Interval]) or float(
                f.header[0][segyio.TraceField.TRACE_SAMPLE_INTERVAL])
        t0 = float(f.samples[0])
        ext = 0 if su else int(f.ext_headers)
        form = 5 if su else int(f.format)
        out.append(f"file\t{name}\t{'su' if su else 'segy'}\t{endian}\t{form}\t{ns}\t{n}\t"
                   f"{fmt(dt)}\t{fmt(t0)}\t{ext}")
        if not su:
            # segyio converts the textual header from EBCDIC unconditionally, so it is read
            # from the file with Python's codecs (cp037 or Latin-1), control codes as spaces.
            with open(path, "rb") as fh:
                raw = fh.read(3200)
            text = "".join(" " if ord(c) < 0x20 or 0x7F <= ord(c) < 0xA0 else c
                           for c in raw.decode("cp037" if is_ebcdic(raw) else "latin-1"))
            for card in list(range(6)) + [39]:
                line = text[card * 80:(card + 1) * 80].rstrip()
                out.append(f"text\t{name}\t{card + 1}\t{line}")
        for t in sorted({0, n // 2, n - 1}):
            h = f.header[t]
            for b in FIELDS:
                out.append(f"hdr\t{name}\t{t}\t{b}\t{h[b]}")
        for t in range(n):
            h = f.header[t]
            s = h[71]
            x, y = h[181], h[185]
            if x == 0 and y == 0:
                x, y = h[73], h[77]
            out.append(f"xy\t{name}\t{t}\t{fmt(scaled(x, s))}\t{fmt(scaled(y, s))}")
        with open(path, "rb") as fh:
            fh.seek(3224)
            code = int.from_bytes(fh.read(2), "big" if endian == "big" else "little")
        if not su and code in (7, 15):
            # segyio 1.9 has no 24-bit formats: numpy reads the raw samples.
            form = code
            out[0] = out[0].replace(f"\tsegy\t{endian}\t1\t", f"\tsegy\t{endian}\t{code}\t")
            raw = np.fromfile(path, dtype=np.uint8)[3600:].reshape(n, 240 + 3 * ns)[:, 240:]
            b = raw.reshape(n, ns, 3).astype(np.int64)
            if endian == "little":
                b = b[:, :, ::-1]
            u = (b[:, :, 0] << 16) | (b[:, :, 1] << 8) | b[:, :, 2]
            if code == 7:
                u = np.where(u >= 1 << 23, u - (1 << 24), u)
            data = u.astype(np.float64)
        else:
            data = np.stack([np.asarray(f.trace[t], dtype=np.float64) for t in range(n)])
        for t in range(n):
            out.append(f"tsum\t{name}\t{t}\t{fmt(data[t].sum())}")
        step = max(1, data.size // max_values)
        for i in range(0, data.size, step):
            t, k = divmod(i, ns)
            out.append(f"val\t{name}\t{t}\t{k}\t{fmt(data[t, k])}")
        keys = [(int(f.header[t][189]), int(f.header[t][193]), int(f.header[t][9]),
                 int(f.header[t][21]), int(f.header[t][17]), int(f.header[t][13]))
                for t in range(n)]
    if su:
        return out
    with open(path, "rb") as fh:
        fh.seek(3224)
        code = int.from_bytes(fh.read(2), endian)
    # Structured 3-D, as segyio sees it (it needs a full, sorted grid).
    for ib, xb in KEYS:
        try:
            g = segyio.open(path, "r", endian=endian, iline=ib, xline=xb)
        except Exception:  # noqa: BLE001
            continue
        with g:
            if g.unstructured or len(g.ilines) < 2 or len(g.xlines) < 2:
                continue
            il = ",".join(str(int(v)) for v in g.ilines)
            xl = ",".join(str(int(v)) for v in g.xlines)
            offsets = len(g.offsets)
            out.append(f"geom\t{name}\t{ib}\t{xb}\t{il}\t{xl}\t{offsets}")
            if offsets > 1:
                return out
            nx = len(g.xlines)
            if code in (7, 15):  # segyio reads these as IBM floats
                return out
            for i, ilno in enumerate(g.ilines):
                sec = np.asarray(g.iline[ilno], dtype=np.float64)  # [xl, sample]
                for x in range(0, nx, max(1, nx // 3)):
                    for k in range(0, ns, max(1, ns // 5)):
                        out.append(f"sect\t{name}\t{i}\t{x}\t{k}\t{fmt(sec[x, k])}")
        return out
    # Not a full grid for segyio: each trace's cell at the standard bytes, when set.
    if any(k[0] or k[1] for k in keys):
        for t, k in enumerate(keys):
            out.append(f"cell\t{name}\t{k[0]}\t{k[1]}\t{t}")
    return out


def main():
    d, dst = sys.argv[1], sys.argv[2]
    max_values = int(sys.argv[3]) if len(sys.argv) > 3 else 700
    rows = [f"# Written by gen/oracle.py (segyio {segyio.__dict__.get('__version__', '1.9')},"
            f" numpy {np.__version__}); do not edit."]
    for name in sorted(os.listdir(d)):
        if not name.endswith((".sgy", ".segy", ".su")):
            continue
        try:
            rows += rows_for(os.path.join(d, name), name, max_values)
        except Exception as e:  # noqa: BLE001  (an oracle failure, not a refusal)
            rows.append(f"skip\t{name}\t{e}".replace("\n", " "))
    with open(dst, "w") as o:
        o.write("\n".join(rows) + "\n")


if __name__ == "__main__":
    main()
