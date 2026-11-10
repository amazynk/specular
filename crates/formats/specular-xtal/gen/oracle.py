#!/usr/bin/env python3
"""Writes fabio's frames and pyFAI's scattering angles for diffraction detector files.

    oracle.py ROOT [OUT]

Walks ROOT for detector files (anything fabio opens as CBF, EDF, ADSC/d*TREK SMV, Bruker,
MarCCD or R-AXIS, compressed copies included) and writes OUT/REL.txt and, per frame K (at
most the first ten), OUT/REL.K.f32.gz. OUT defaults to ROOT/expected; files under OUT itself
are skipped. Each slice of an EDF volume block (Dim_3) is a frame; `.Z` files are unpacked
by gzip(1) as fabio does; EDF BYTE_OFFSET blocks go through fabio's byte-offset decoder
(fabio's EDF reader needs an optional module for them).

The .txt is tab-separated lines:
    kind NAME               fabio's reader: Cbf, Edf, Smv, Bruker, MarCcd, Raxis
    size W H FRAMES         frame size and count (EDF: frames of the first frame's size)
    stats K MIN MAX MEAN N  finite values of frame K (masked pixels excluded)
    geom NAME VALUE         wavelength (Å), distance (mm), beam_x/beam_y (pixels, edge
                            coordinates), pixel_x/pixel_y (mm), two_theta (°), overload
    scatter X Y TTH Q D     pyFAI at pixel (X, Y)'s centre: 2θ (°), q (Å⁻¹), d (Å); only for
                            flat detectors with complete geometry and no 2θ swing
    error MESSAGE           fabio could not read the file
The .f32.gz holds frame K, gzipped, as little-endian float32, top (file) row first, masked
pixels NaN: negative counts in Pilatus/Eiger CBFs, EDF `Dummy` values.

The geometry is read from the headers here, independently of the Rust parsers; pyFAI turns
it into 2θ and q with its own detector model. Run with python -I; files are only read.
"""

import gzip
import os
import re
import struct
import sys
import warnings

import numpy as np

warnings.simplefilter("ignore")

import fabio  # noqa: E402

try:
    from pyFAI.integrator.azimuthal import AzimuthalIntegrator
except ImportError:  # older pyFAI
    from pyFAI.azimuthalIntegrator import AzimuthalIntegrator
import pyFAI.detectors  # noqa: E402

KINDS = {
    "CbfImage": "Cbf",
    "EdfImage": "Edf",
    "AdscImage": "Smv",
    "DtrekImage": "Smv",
    "BrukerImage": "Bruker",
    "Bruker100Image": "Bruker",
    "MarccdImage": "MarCcd",
    "RaxisImage": "Raxis",
}

SKIP_EXT = (".txt", ".f32", ".f32.gz", ".py", ".sh", ".md", ".json", ".tsv")


def unz(d):
    """A Unix compress (.Z) stream unpacked by gzip(1), as fabio does."""
    import subprocess

    return subprocess.run(["gzip", "-dc"], input=d, check=True, capture_output=True).stdout


class EdfOffset:
    """An EDF file of BYTE_OFFSET blocks, which fabio reads only with an optional module:
    the header parsed here, the blocks by fabio's own byte-offset decoder."""

    def __init__(self, raw):
        from fabio.compression import decByteOffset_numpy

        end = raw.index(b"}\n") + 2
        self.header = {}
        for st in raw[raw.index(b"{") + 1 : end - 2].decode("latin1").split(";"):
            if "=" in st:
                k, v = st.split("=", 1)
                self.header[k.strip()] = v.strip()
        w, h = int(self.header["Dim_1"]), int(self.header["Dim_2"])
        size = int(self.header["Size"])
        t = {"SignedInteger": np.int32, "UnsignedShort": np.uint16, "SignedShort": np.int16}
        v = decByteOffset_numpy(raw[end : end + size], size=w * h)
        self.data = v.astype(t[self.header["DataType"]]).reshape(h, w)
        self.nframes = 1


def raw_bytes(path):
    d = open(path, "rb").read()
    if d[:2] == b"\x1f\x9d":
        return unz(d)
    if d[:2] == b"\x1f\x8b":
        return gzip.decompress(d)
    if d[:3] == b"BZh":
        import bz2

        return bz2.decompress(d)
    return d


def num(s):
    m = re.search(r"[-+]?(\d+\.?\d*|\.\d+)([eE][-+]?\d+)?", s or "")
    return float(m.group(0)) if m else None


def nums(s):
    return [float(m.group(0)) for m in re.finditer(r"[-+]?(\d+\.?\d*|\.\d+)([eE][-+]?\d+)?", s or "")]


def pos(v):
    return v if v is not None and np.isfinite(v) and v > 0 else None


def cbf_geom(raw):
    text = raw[: raw.find(b"--CIF-BINARY-FORMAT-SECTION--")].decode("latin1")
    text = text.replace("\\\r\n", "").replace("\\\n", "")
    g = {}
    hdr = {}
    for line in text.splitlines():
        line = line.strip()
        if not line.startswith("#"):
            continue
        parts = line[1:].strip().split(None, 1)
        if len(parts) == 2:
            hdr[parts[0].rstrip(":=")] = parts[1]

    def m_to_mm(v):
        n = num(v)
        if n is None:
            return None
        return n if " mm" in v else n * 1000.0

    g["wavelength"] = pos(num(hdr.get("Wavelength")))
    g["distance"] = pos(m_to_mm(hdr.get("Detector_distance", "")))
    b = nums(hdr.get("Beam_xy"))
    if len(b) >= 2:
        g["beam_x"], g["beam_y"] = b[0], b[1]
    p = [m_to_mm(x) for x in hdr.get("Pixel_size", "").split(" x ")] if "Pixel_size" in hdr else []
    if len(p) == 2 and all(p):
        g["pixel_x"], g["pixel_y"] = p
    g["two_theta"] = num(hdr.get("Detector_2theta"))
    g["overload"] = pos(num(hdr.get("Count_cutoff")))
    conv = re.search(r"_array_data\.header_convention\s+\"?([A-Za-z_]+)", text)
    conv = conv.group(1).upper() if conv else ""
    det = hdr.get("Detector", "").upper()
    mask = conv.startswith(("PILATUS", "SLS")) or "PILATUS" in det or "EIGER" in det or g["overload"] is not None
    return g, mask


def edf_geom(h):
    hl = {k.lower(): v for k, v in h.items()}

    def f(k):
        return num(hl.get(k.lower()))

    g = {}
    if f("Center_1") is not None and f("Center_2") is not None:
        g["beam_x"] = f("Center_1") - (f("Offset_1") or 0)
        g["beam_y"] = f("Center_2") - (f("Offset_2") or 0)
    elif pos(f("Beam center x")) and pos(f("Beam center y")):
        g["beam_x"], g["beam_y"] = f("Beam center x"), f("Beam center y")
    if pos(f("PSize_1")) and pos(f("PSize_2")):
        g["pixel_x"], g["pixel_y"] = f("PSize_1") * 1000, f("PSize_2") * 1000
    elif pos(f("Real pixel size x (mm)")) and pos(f("Real pixel size y (mm)")):
        g["pixel_x"], g["pixel_y"] = f("Real pixel size x (mm)"), f("Real pixel size y (mm)")
    if pos(f("SampleDistance")):
        g["distance"] = f("SampleDistance") * 1000
    else:
        g["distance"] = pos(f("Distance in mm"))
    if pos(f("WaveLength")):
        g["wavelength"] = f("WaveLength") * 1e10
    else:
        g["wavelength"] = pos(f("Wavelength alpha1")) or pos(f("Wavelength alpha"))
    dummy = f("Dummy")
    ddummy = abs(f("DDummy") or 0.0)
    return g, (dummy, ddummy) if dummy not in (None, 0.0) else None


def smv_geom(raw):
    n = int(re.search(rb"HEADER_BYTES=\s*(\d+)", raw[:128]).group(1))
    h = {}
    for line in raw[:n].split(b"\0")[0].decode("latin1").splitlines():
        if "=" in line:
            k, v = line.split("=", 1)
            h[k.strip()] = v.strip().rstrip(";").strip()
    g = {}
    if "DETECTOR_NAMES" in h:
        p = h["DETECTOR_NAMES"].split()[0]
        b = nums(h.get(p + "SPATIAL_BEAM_POSITION"))
        if len(b) >= 2:
            g["beam_x"], g["beam_y"] = b[:2]
        s = nums(h.get(p + "SPATIAL_DISTORTION_INFO"))
        if len(s) >= 4:
            g["pixel_x"], g["pixel_y"] = s[2], s[3]
        for name, v in zip(h.get(p + "GONIO_NAMES", "").split(), nums(h.get(p + "GONIO_VALUES"))):
            if name.lower() == "distance":
                g["distance"] = pos(v)
            if name.lower() == "2theta":
                g["two_theta"] = v
    else:
        px = pos(num(h.get("PIXEL_SIZE")))
        if px:
            g["pixel_x"] = g["pixel_y"] = px
            # BEAM_CENTER_X runs along the slow axis, BEAM_CENTER_Y along the fast one.
            g["beam_x"] = num(h.get("BEAM_CENTER_Y")) / px
            g["beam_y"] = num(h.get("BEAM_CENTER_X")) / px
        g["distance"] = pos(num(h.get("DISTANCE")))
        g["two_theta"] = num(h.get("TWOTHETA"))
    w = nums(h.get("WAVELENGTH"))
    g["wavelength"] = pos(w[1] if len(w) >= 2 and w[0] == int(w[0]) and w[0] >= 1 else (w[0] if w else None))
    g["overload"] = pos(num(h.get("SATURATED_VALUE"))) or pos(num(h.get("CCD_IMAGE_SATURATION")))
    return g


def bruker_geom(h, w, hgt):
    g = {}
    ang = nums(h.get("ANGLES"))
    if len(ang) >= 4:
        g["two_theta"] = ang[0]
    d = nums(h.get("DISTANC"))
    if d and d[0] > 0:
        g["distance"] = d[0] * 10
    wl = nums(h.get("WAVELEN"))
    if wl and wl[0] > 0:
        g["wavelength"] = wl[0]
    scale = 512.0 / w
    det = h.get("DETTYPE", "").split()
    if len(det) >= 2 and float(det[1]) > 0:
        g["pixel_x"] = g["pixel_y"] = 10.0 / float(det[1]) * scale
    c = nums(h.get("CENTER"))
    if len(c) >= 2 and (c[0] or c[1]):
        # 512-mode pixels, y up from the bottom row.
        g["beam_x"], g["beam_y"] = c[0] / scale, hgt - c[1] / scale
    return g


def marccd_geom(raw):
    big = struct.unpack("<i", raw[1052:1056])[0] != 1234
    e = ">" if big else "<"

    def i(at):
        return struct.unpack(e + "i", raw[at : at + 4])[0]

    g = {}
    d = i(1664) / 1000 or i(1720) / 1000
    g["distance"] = pos(d)
    g["beam_x"], g["beam_y"] = i(1668) / 1000, i(1672) / 1000
    g["pixel_x"], g["pixel_y"] = i(1796) / 1e6, i(1800) / 1e6
    g["wavelength"] = pos(i(1932) * 1e-5)
    g["two_theta"] = i(1692) / 1000
    g["overload"] = pos(float(i(1128)))
    return g


def raxis_geom(raw):
    def f(at):
        return struct.unpack(">f", raw[at : at + 4])[0]

    g = {
        "wavelength": pos(f(292)),
        "distance": pos(f(344)),
        "beam_x": f(540),
        "beam_y": f(544),
        "pixel_x": f(776),
        "pixel_y": f(780),
        "two_theta": f(556),
    }
    curved = struct.unpack(">i", raw[448:452])[0] == 1
    return g, curved


def scatter(g, w, h):
    need = ("wavelength", "distance", "beam_x", "beam_y", "pixel_x", "pixel_y")
    if any(g.get(k) is None for k in need) or abs(g.get("two_theta") or 0.0) > 1e-9:
        return []
    px, py = g["pixel_x"] / 1000, g["pixel_y"] / 1000
    det = pyFAI.detectors.Detector(pixel1=py, pixel2=px, max_shape=(h, w))
    ai = AzimuthalIntegrator(
        dist=g["distance"] / 1000,
        poni1=g["beam_y"] * py,
        poni2=g["beam_x"] * px,
        wavelength=g["wavelength"] * 1e-10,
        detector=det,
    )
    bx, by = int(g["beam_x"]), int(g["beam_y"])
    pts = [(0, 0), (w - 1, 0), (0, h - 1), (w - 1, h - 1), (w // 2, h // 2), (w // 3, 2 * h // 3),
           (min(max(bx + 5, 0), w - 1), min(max(by, 0), h - 1)), (min(max(bx, 0), w - 1), min(max(by - 7, 0), h - 1))]
    out = []
    for x, y in pts:
        tth = float(ai.tth(np.array([y]), np.array([x]))[0])
        q = float(ai.qFunction(np.array([y]), np.array([x]))[0]) / 10.0  # nm⁻¹ → Å⁻¹
        if tth <= 1e-12:
            continue
        d = g["wavelength"] / (2 * np.sin(tth / 2))
        out.append((x, y, np.degrees(tth), q, d))
    return out


class Slice:
    def __init__(self, data):
        self.data = data


def frames(img):
    """Fabio's frames; each slice of a volume block (EDF Dim_3) is a frame of its own."""
    n = getattr(img, "nframes", 1) or 1
    k = 0
    for i in range(n):
        fr = img if i == 0 else img.getframe(i)
        if fr.data.ndim == 3:
            for s in fr.data:
                yield k, Slice(s)
                k += 1
        else:
            yield k, fr
            k += 1


def open_image(path, out_base):
    """fabio's reader for `path`; `.Z` files are unpacked beside the output first."""
    d = open(path, "rb").read(4)
    if d[:2] == b"\x1f\x9d":
        tmp = out_base + ".unz"
        open(tmp, "wb").write(unz(open(path, "rb").read()))
        try:
            img = fabio.open(tmp)
            _ = img.data
            return img
        finally:
            os.remove(tmp)
    try:
        img = fabio.open(path)
        _ = img.data
        return img
    except Exception:
        raw = raw_bytes(path)
        if raw.lstrip()[:1] == b"{" and b"BYTE_OFFSET" in raw[:4096]:
            return EdfOffset(raw)
        raise


def oracle(path, out_base):
    lines = []
    try:
        img = open_image(path, out_base)
        kind = "Edf" if isinstance(img, EdfOffset) else KINDS.get(type(img).__name__)
        if kind is None:
            return None
        raw = raw_bytes(path)
        data0 = next(frames(img))[1].data
        h, w = data0.shape
        mask_negative = False
        dummy = None
        curved = False
        if kind == "Cbf":
            g, mask_negative = cbf_geom(raw)
        elif kind == "Edf":
            g, dummy = edf_geom(img.header)
        elif kind == "Smv":
            g = smv_geom(raw)
        elif kind == "Bruker":
            g = bruker_geom(img.header, w, h)
        elif kind == "MarCcd":
            g = marccd_geom(raw)
        else:
            g, curved = raxis_geom(raw)
        kept = []
        for k, fr in frames(img):
            if fr.data.shape != (h, w):
                break
            kept.append((k, fr))
        lines.append(f"kind\t{kind}")
        lines.append(f"size\t{w}\t{h}\t{len(kept)}")
        for k, fr in kept[:10]:
            a = fr.data.astype(np.float64)
            if mask_negative:
                a[a < 0] = np.nan
            if dummy is not None:
                dv, dd = dummy
                tol = max(dd, np.finfo(np.float32).eps * abs(dv))
                a[np.abs(a - dv) <= tol] = np.nan
            fin = a[np.isfinite(a)]
            lines.append(f"stats\t{k}\t{float(fin.min())!r}\t{float(fin.max())!r}\t{float(fin.mean())!r}\t{fin.size}")
            with gzip.open(f"{out_base}.{k}.f32.gz", "wb", compresslevel=1) as z:
                z.write(a.astype("<f4").tobytes())
        for name in ("wavelength", "distance", "beam_x", "beam_y", "pixel_x", "pixel_y", "two_theta", "overload"):
            v = g.get(name)
            if v is not None:
                lines.append(f"geom\t{name}\t{float(v)!r}")
        if not curved:
            for x, y, tth, q, d in scatter(g, w, h):
                lines.append(f"scatter\t{x}\t{y}\t{float(tth)!r}\t{float(q)!r}\t{float(d)!r}")
    except Exception as e:  # noqa: BLE001
        lines = [f"error\t{type(e).__name__}: {e}"]
    with open(out_base + ".txt", "w") as f:
        f.write("\n".join(lines) + "\n")
    return lines


def main():
    root = os.path.abspath(sys.argv[1])
    out = os.path.abspath(sys.argv[2]) if len(sys.argv) > 2 else os.path.join(root, "expected")
    os.makedirs(out, exist_ok=True)
    for dirpath, dirs, files in os.walk(root):
        dirs[:] = sorted(d for d in dirs if os.path.join(dirpath, d) != out)
        for name in sorted(files):
            if name.endswith(SKIP_EXT):
                continue
            path = os.path.join(dirpath, name)
            rel = os.path.relpath(path, root)
            base = os.path.join(out, rel)
            os.makedirs(os.path.dirname(base), exist_ok=True)
            r = oracle(path, base)
            if r is not None:
                print(rel, r[0] if r else "")


if __name__ == "__main__":
    main()
