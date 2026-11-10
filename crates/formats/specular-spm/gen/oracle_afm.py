#!/usr/bin/env python3
"""Expected planes of AFM files for specular-spm's tests, from independent readers.

    oracle_afm.py ROOT

For every .spm/.NNN (Nanoscope), .sxm, .jpk/.jpk-qi-image and .mdt file under ROOT (except
ROOT/expected) it writes ROOT/expected/REL.K.txt per page K:

    size W H P
    x OFFSET STEP UNIT        (or: x none)
    y OFFSET STEP UNIT
    plane I UNIT<TAB>LABEL

and REL.K.I.f32 (little-endian, rows top-down, SI units). Oracles: pySPM's Bruker reader
(Nanoscope), nanonispy (SXM), tifffile with AFMReader's JPK calibration (JPK), and the
small numpy reader below for NT-MDT (no Python reader exists). The display conventions
(rows top-down, backward scan lines mirrored, pixel-centre lateral calibrations from the
scan's top-left corner, units in SI) are applied here on top of the oracles' raw arrays.

pySPM, AFMReader and tifffile are found through $AFM_PYLIB (a pip --target directory);
pySPM's package __init__ is not imported (it needs scikit-image), only its Bruker module.
"""

import importlib.util
import os
import re
import struct
import sys
import types
from pathlib import Path

import numpy as np

if os.environ.get("AFM_PYLIB"):
    sys.path.insert(0, os.environ["AFM_PYLIB"])

PREFIX = {"f": 1e-15, "p": 1e-12, "n": 1e-9, "u": 1e-6, "µ": 1e-6, "μ": 1e-6, "~": 1e-6, "m": 1e-3,
          "k": 1e3, "M": 1e6, "G": 1e9}
BASES = {"m", "V", "A", "N", "Hz", "s", "Pa", "W", "F", "S", "T", "C"}


def si(unit):
    u = unit.strip()
    if u in BASES:
        return u, 1.0
    if u in ("Å", "Angstrom", "angstrom"):
        return "m", 1e-10
    if u in ("°", "º", "deg", "degree", "degrees"):
        return "deg", 1.0
    if u == "um":
        return "m", 1e-6
    if len(u) > 1 and u[1:] in BASES:
        if u[0] == "c" and u[1:] == "m":
            return "m", 1e-2
        if u[0] in PREFIX:
            return u[1:], PREFIX[u[0]]
    return u, 1.0


class Out:
    def __init__(self, root, rel):
        self.root, self.rel, self.k = root, rel, 0

    def page(self, planes, x, y):
        """planes: [(label, unit, 2-D array top-down)]; x, y: (offset, step, unit) or None."""
        h, w = planes[0][2].shape
        base = self.root / "expected" / f"{self.rel}.{self.k}"
        base.parent.mkdir(parents=True, exist_ok=True)
        lines = [f"size {w} {h} {len(planes)}"]
        for name, ax in (("x", x), ("y", y)):
            lines.append(f"{name} none" if ax is None else f"{name} {float(ax[0])!r} {float(ax[1])!r} {ax[2]}")
        for i, (label, unit, a) in enumerate(planes):
            assert a.shape == (h, w)
            lines.append(f"plane {i} {unit}\t{label}")
            np.ascontiguousarray(a, dtype="<f4").tofile(f"{base}.{i}.f32")
        Path(f"{base}.txt").write_text("\n".join(lines) + "\n")
        self.k += 1


def centres(origin, rng, n, unit):
    step = rng / n
    return (origin + step / 2, step, unit)


# --- Nanoscope: pySPM ---------------------------------------------------------------------

def load_bruker():
    stub = types.ModuleType("pySPM")
    stub.SPM_image = lambda **kw: kw
    sys.modules["pySPM"] = stub
    for d in sys.path:
        p = Path(d) / "pySPM" / "Bruker.py"
        if p.is_file():
            spec = importlib.util.spec_from_file_location("pySPM_Bruker", p)
            mod = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(mod)
            return mod.Bruker
    raise SystemExit("pySPM not found (set AFM_PYLIB)")


def nanoscope(path, out):
    Bruker = load_bruker()
    b = Bruker(str(path))
    # The scan list's size, for image lists without their own (old files).
    head = path.read_bytes()[:200000].decode("latin-1")
    m = re.search(r"\\\*Ciao scan list(.*?)\\\*", head, re.S)
    scan = re.search(r"\\Scan size: ([^\r\n]*)", m.group(1), re.I) if m else None
    pages = []
    for layer in b.layers:
        # pySPM wants these exact spellings; older headers write them in lower case.
        for key, default in ((b"Line Direction", [b"Trace"]), (b"Aspect Ratio", [b"1:1"]), (b"Scan Size", None)):
            if key not in layer:
                v = next((v for k, v in layer.items() if k.lower() == key.lower()), None)
                if v is None and key == b"Scan Size" and scan:
                    t = scan.group(1).split()
                    s = float(t[0])
                    lines, samps = int(layer[b"Number of lines"][0]), int(layer[b"Samps/line"][0])
                    v = [f"{s} {s * lines / samps} {t[1]}".encode()]
                layer[key] = v if v is not None else default
    seen = {}
    for i, layer in enumerate(b.layers):
        if b"@2:Image Data" not in layer:
            continue
        name = re.match(r'([^ ]+) \[([^\]]*)\] "([^"]*)"', layer[b"@2:Image Data"][0].decode("latin-1")).group(3)
        back = layer[b"Line Direction"][0] == b"Retrace"
        img = b.get_channel(channel=name, backward=back, lazy=False)
        data = np.flipud(img["BIN"])
        # pySPM's unit string only drops "/V" and falls back to the soft scale's "V" type
        # letter when the soft scale has no unit; the values are its own, the unit becomes
        # the soft scale's numerator, else the hard value's unit.
        zs = layer[b"@2:Z scale"][0].decode("latin-1")
        soft = re.match(r"\S+\s+\[([^\]]+)\]", zs)
        unit = img["zscale"]
        if soft:
            t = b.scanners[0][b"@" + soft.group(1).encode("latin-1")][0].decode("latin-1").split()
            unit = t[2].split("/")[0] if len(t) > 2 and t[2].split("/")[0] else zs.split()[-1]
        unit, f = si(unit)
        label = name + (" (retrace)" if back else "")
        h, w = data.shape
        key = (w, h)
        if key not in seen:
            real = img["real"]
            lu, lf = si(real["unit"])
            seen[key] = len(pages)
            pages.append(([], centres(0, real["x"] * lf, w, lu), centres(0, real["y"] * lf, h, lu)))
        pages[seen[key]][0].append((label, unit, data * f))
    for planes, x, y in pages:
        out.page(planes, x, y)


# --- SXM: nanonispy ------------------------------------------------------------------------

def sxm(path, out):
    np.float, np.int = float, int  # nanonispy predates numpy 2
    import nanonispy.read as nap

    s = nap.Scan(str(path))
    h = s.header
    nx, ny = (int(v) for v in h["scan_pixels"])
    up = h["scan_dir"].strip().lower() == "up"
    info = h["data_info"]
    planes = []
    for name, unit, direction in zip(info["Name"], info["Unit"], info["Direction"]):
        u, f = si(unit)
        sig = s.signals[name]
        fwd = np.flipud(sig["forward"]) if up else sig["forward"]
        planes.append((f"{name} forward" if direction == "both" else name, u, fwd.astype(np.float64) * f))
        if direction == "both":
            bwd = np.flipud(sig["backward"]) if up else sig["backward"]
            planes.append((f"{name} backward", u, np.fliplr(bwd).astype(np.float64) * f))
    rx, ry = h["scan_range"][:2]
    cx, cy = h["scan_offset"][:2]
    out.page(planes, centres(cx - rx / 2, rx, nx, "m"), centres(cy - ry / 2, ry, ny, "m"))


# --- JPK: tifffile + AFMReader's calibration -----------------------------------------------

def jpk(path, out):
    import tifffile
    from AFMReader import jpk as ar

    tags = ar._load_jpk_tags(None)
    tif = tifffile.TiffFile(str(path))
    p0 = tif.pages[0].tags
    direction = p0[0x804B].value if 0x804B in p0 else None
    flip = direction != "topDown"
    pages, seen = [], {}
    for idx in range(1, len(tif.pages)):
        page = tif.pages[idx]
        if 0x8050 not in page.tags:
            continue
        scaling, offset = ar._get_z_scaling(tif, idx, tags)
        data = page.asarray().astype(np.float64) * scaling + offset
        if flip:
            data = np.flipud(data)
        shown = page.tags[0x8081].value
        unit = ""
        for n in range(page.tags[0x8080].value):
            if 0x8090 + 0x30 * n in page.tags and page.tags[0x8090 + 0x30 * n].value == shown:
                t = page.tags.get(0x80A2 + 0x30 * n)
                unit = t.value if t is not None else ""
        u, f = si(unit)
        label = page.tags[0x8052].value if 0x8052 in page.tags and page.tags[0x8052].value else page.tags[0x8050].value
        retrace = page.tags[0x8051].value
        if isinstance(retrace, bytes):
            retrace = int.from_bytes(retrace[:1], "big")
        if retrace:
            label += " (retrace)"
        hh, ww = data.shape
        if (ww, hh) not in seen:
            seen[(ww, hh)] = len(pages)
            pages.append(([], centres(p0[0x8040].value, p0[0x8042].value, ww, "m"),
                          centres(p0[0x8041].value, p0[0x8043].value, hh, "m")))
        pages[seen[(ww, hh)]][0].append((label, u, data * f))
    for planes, x, y in pages:
        out.page(planes, x, y)


# --- NT-MDT: own numpy reader --------------------------------------------------------------

MDT_UNITS = {-10: ("1/cm", 1), -5: ("m", 1), -4: ("m", 1e-2), -3: ("m", 1e-3), -2: ("m", 1e-6), -1: ("m", 1e-9),
             0: ("m", 1e-10), 1: ("A", 1e-9), 2: ("V", 1), 4: ("Hz", 1e3), 5: ("deg", 1), 6: ("%", 1),
             7: ("°C", 1), 8: ("V", 1), 9: ("s", 1), 10: ("s", 1e-3), 11: ("s", 1e-6), 12: ("s", 1e-9),
             13: ("counts", 1), 14: ("px", 1), 20: ("A", 1), 21: ("A", 1e-3), 22: ("A", 1e-6), 23: ("A", 1e-9),
             24: ("A", 1e-12), 25: ("V", 1), 26: ("V", 1e-3), 27: ("V", 1e-6), 28: ("V", 1e-9), 29: ("V", 1e-12),
             30: ("N", 1), 31: ("N", 1e-3), 32: ("N", 1e-6), 33: ("N", 1e-9), 34: ("N", 1e-12)}
MDA_TYPES = {-1: "<i1", 1: "<u1", -2: "<i2", 2: "<u2", -4: "<i4", 4: "<u4", -8: "<i8", 8: "<u8", -5892: "<f4",
             -13320: "<f8"}


def mdt(path, out):
    b = path.read_bytes()
    last = struct.unpack_from("<H", b, 12)[0]
    at = 33
    for _ in range(last + 1):
        size, ty = struct.unpack_from("<IH", b, at)
        var_size = struct.unpack_from("<H", b, at + 20)[0]
        d = at + 22
        if ty == 0:
            sc = [struct.unpack_from("<ffh", b, d + 10 * i) for i in range(3)]
            p = d + var_size
            _, w, h, ndots = struct.unpack_from("<4H", b, p)
            p += 8
            if ndots:
                p += 4 + struct.unpack_from("<i", b, p)[0]
                dots = [struct.unpack_from("<ffii", b, p + 16 * i) for i in range(ndots)]
                p += 16 * ndots + 2 * sum(fw + bw for _, _, fw, bw in dots)
            raw = np.frombuffer(b, "<i2", w * h, p).reshape(h, w)
            zu, zf = MDT_UNITS.get(sc[2][2], ("", 1))
            data = (sc[2][0] + sc[2][1] * raw.astype(np.float64)) * zf
            tat = p + 2 * w * h
            label = "Z"
            if tat + 4 <= at + size:
                n = struct.unpack_from("<I", b, tat)[0]
                t = b[tat + 4:tat + 4 + n].decode("cp1251").strip()
                if t and t.isascii():
                    label = t

            def ax(s, n):
                u, f = MDT_UNITS.get(s[2], ("", 1))
                return ((s[0] + s[1] / 2) * f, s[1] * f, u)

            out.page([(label, zu, data)], ax(sc[0], w), ax(sc[1], h))
        elif ty == 106:
            name_size, comm_size = struct.unpack_from("<II", b, d + 44)
            data_offset, _ = struct.unpack_from("<II", b, d + 68)
            s = d + 76 + name_size + comm_size
            ndim, nmes = struct.unpack_from("<II", b, s + 16)
            if ndim == 2 and nmes == 1:
                c, cals = s + 24, []
                for _ in range(3):
                    tot, fixed, ln, lc, lu = struct.unpack_from("<5I", b, c)
                    bias, scale, lo, hi, dt, la = struct.unpack_from("<ddQQiI", b, c + 44)
                    nm = b[c + fixed:c + fixed + ln].decode("latin-1").strip()
                    un = b[c + fixed + ln + lc:c + fixed + ln + lc + lu].decode("latin-1").strip()
                    cals.append((nm, un, bias, scale, hi - lo + 1, dt))
                    c += max(tot, fixed + ln + lc + lu + la)
                (_, xu, xb, xs, w, _), (_, yu, yb, ys, h, _), (zn, zu, zb, zs, _, zt) = cals
                raw = np.frombuffer(b, MDA_TYPES[zt], w * h, d + data_offset).reshape(h, w)
                u, f = si(zu)
                data = (raw.astype(np.float64) * zs + zb) * f
                xu2, xf = si(xu)
                yu2, yf = si(yu)
                out.page([(zn or "Z", u, data)], ((xb + xs / 2) * xf, xs * xf, xu2), ((yb + ys / 2) * yf, ys * yf, yu2))
        at += size


def kind(p):
    n = p.name.lower()
    if n.endswith(".sxm"):
        return sxm
    if n.endswith(".jpk") or n.endswith(".jpk-qi-image"):
        return jpk
    if n.endswith(".mdt"):
        return mdt
    if n.endswith(".spm") or re.search(r"\.\d{3}$", n):
        return nanoscope
    return None


def main():
    root = Path(sys.argv[1]).resolve()
    for p in sorted(root.rglob("*")):
        if not p.is_file() or "expected" in p.relative_to(root).parts:
            continue
        f = kind(p)
        if f is None:
            continue
        rel = p.relative_to(root).as_posix()
        out = Out(root, rel)
        try:
            f(p, out)
            print(f"{rel}: {out.k} pages")
        except Exception as e:  # noqa: BLE001 — report and go on
            print(f"{rel}: oracle failed: {e!r}")


if __name__ == "__main__":
    main()
