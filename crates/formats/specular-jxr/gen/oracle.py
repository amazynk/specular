#!/usr/bin/env python3
"""Cross-checks the specular-jxr decoder against jxrlib (JxrEncApp / JxrDecApp) as the oracle.

jxrlib is only a test oracle: JxrEncApp encodes synthetic TIFF inputs across pixel formats and
codestream options, JxrDecApp decodes each file to a TIFF in its native pixel format, and the
strip bytes must equal what `examples/jxr` writes for the same file.

Usage: python oracle.py WORK_DIR JXR_EXAMPLE [FILTER]
  WORK_DIR     scratch directory for inputs and outputs
  JXR_EXAMPLE  path to the built `jxr` example binary
  FILTER       optional substring; only cases whose name contains it run
"""

import os
import struct
import subprocess
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import tiffraw  # noqa: E402


def write_tiff(path, arr, photometric, sample_format=1, extra=None, bps=None):
    """Writes an uncompressed little-endian single-strip TIFF from an HxWxC array (or, with
    `bps` = 1, from an HxW boolean array packed MSB first)."""
    if bps == 1:
        h, w = arr.shape
        c = 1
        data = np.packbits(arr, axis=1).tobytes()
    else:
        h, w, c = arr.shape
        bps = arr.dtype.itemsize * 8
        data = arr.astype(arr.dtype.newbyteorder("<")).tobytes()
    entries = []

    def tag(t, ty, vals):
        entries.append((t, ty, vals))

    tag(256, 4, [w])
    tag(257, 4, [h])
    tag(258, 3, [bps] * c)
    tag(259, 3, [1])
    tag(262, 3, [photometric])
    tag(273, 4, [0])  # patched below
    tag(277, 3, [c])
    tag(278, 4, [h])
    tag(279, 4, [len(data)])
    tag(284, 3, [1])
    if extra is not None:
        tag(338, 3, [extra])
    tag(339, 3, [sample_format] * c)
    n = len(entries)
    ifd = 8
    ext = ifd + 2 + 12 * n + 4
    blobs = b""
    recs = []
    for t, ty, vals in entries:
        f = {3: "<H", 4: "<I"}[ty]
        raw = b"".join(struct.pack(f, v) for v in vals)
        if len(raw) <= 4:
            recs.append((t, ty, len(vals), raw.ljust(4, b"\0")))
        else:
            recs.append((t, ty, len(vals), struct.pack("<I", ext + len(blobs))))
            blobs += raw
            if len(blobs) % 2:
                blobs += b"\0"
    strip = ext + len(blobs)
    out = b"II*\0" + struct.pack("<I", ifd) + struct.pack("<H", n)
    for t, ty, cnt, val in recs:
        if t == 273:
            val = struct.pack("<I", strip)
        out += struct.pack("<HHI", t, ty, cnt) + val
    out += b"\0\0\0\0" + blobs + data
    open(path, "wb").write(out)


def content(h, w, c, seed):
    """Smooth gradients, edges and noise in [0, 1): exercises every band and the overlap filters."""
    rng = np.random.default_rng(seed)
    y, x = np.mgrid[0:h, 0:w].astype(np.float64)
    planes = []
    for k in range(c):
        g = 0.5 + 0.4 * np.sin(x / (7 + 3 * k) + y / (11 + k)) * np.cos(y / (5 + 2 * k))
        g += ((x // 13 + y // 9 + k) % 2) * 0.15 - 0.075
        g += rng.normal(0, 0.04, (h, w))
        planes.append(np.clip(g, 0, 0.999))
    a = np.stack(planes, axis=2)
    a[: h // 4, : w // 5] = 0.0  # flat corner
    return a


def inputs(path, kind, h, w, seed):
    """Writes one TIFF input of `kind` (bw, gray8, rgb8, rgba16, cmyk8, rgbf ...)."""
    if kind == "bw":
        write_tiff(path, content(h, w, 1, seed)[:, :, 0] > 0.5, 1, bps=1)
        return
    base = kind.rstrip("0123456789f")
    alpha = base.endswith("a") and base != "gray"
    c = {"gray": 1, "rgb": 3, "rgba": 4, "cmyk": 4, "cmyka": 5}[base]
    a = content(h, w, c, seed)
    if kind.endswith("f"):
        arr = (a * 3.0 - 0.5).astype(np.float32)  # HDR range with negatives
        sf = 3
    elif kind.endswith("16"):
        arr = (a * 65536).astype(np.uint16)
        sf = 1
    else:
        arr = (a * 256).astype(np.uint8)
        sf = 1
    photometric = {"gray": 1, "rgb": 2, "rgba": 2, "cmyk": 5, "cmyka": 5}[base]
    write_tiff(path, arr, photometric, sf, extra=2 if alpha else None)


# (name, -c format, input kind)
FORMATS = [
    ("bgr24", 0, "rgb8"),
    ("bw1", 1, "bw"),
    ("gray8", 2, "gray8"),
    ("gray16", 3, "gray16"),
    ("gray16fix", 4, "grayf"),
    ("gray16half", 5, "grayf"),
    ("gray32fix", 7, "grayf"),
    ("gray32f", 8, "grayf"),
    ("rgb24", 9, "rgb8"),
    ("rgb48", 10, "rgb16"),
    ("rgb48fix", 11, "rgbf"),
    ("rgb48half", 12, "rgbf"),
    ("rgb96fix", 14, "rgbf"),
    ("rgb128f", 15, "rgbf"),
    ("rgbe", 16, "rgbf"),
    ("cmyk32", 17, "cmyka8"),  # see strip_alpha
    ("bgra32", 22, "rgba8"),
    ("rgba64", 23, "rgba16"),
    ("rgba64fix", 24, "rgbaf"),
    ("rgba64half", 25, "rgbaf"),
    ("rgba128fix", 27, "rgbaf"),
    ("rgba128f", 28, "rgbaf"),
    ("bgr555", 29, "rgb8"),
    ("bgr565", 30, "rgb8"),
    ("bgr101010", 31, "rgb16"),
    ("cmyka40", 32, "cmyka8"),
    # 64bppCMYK / 80bppCMYKAlpha: JxrEncApp fails to encode them (exit 151); see make_cmyk16.py.
    ("bgr32", 34, "rgb8"),
]

# (name, extra JxrEncApp options, size)
MODES = [
    ("lossless", [], (45, 67)),
    ("q", ["-q", "0.6"], (45, 67)),
    ("q20", ["-q", "20"], (45, 67)),
    ("l0", ["-l", "0", "-q", "0.8"], (45, 67)),
    ("l2", ["-l", "2", "-q", "0.8"], (45, 67)),
    ("spatial", ["-f", "-q", "0.7"], (45, 67)),
    ("seq", ["-p", "-q", "0.7"], (45, 67)),
    ("d420", ["-d", "1", "-q", "0.7"], (45, 67)),
    ("d422", ["-d", "2", "-q", "0.7"], (45, 67)),
    ("d420l2", ["-d", "1", "-l", "2", "-q", "0.5"], (45, 67)),
    ("d422l2", ["-d", "2", "-l", "2", "-q", "0.5"], (45, 67)),
    ("d0", ["-d", "0", "-q", "0.7"], (45, 67)),
    ("tiles", ["-U", "2", "3", "-q", "0.7"], (100, 150)),
    ("tilesvh", ["-V", "2", "3", "-H", "1", "2", "-q", "0.7", "-l", "2"], (100, 150)),
    ("tilessp", ["-U", "3", "2", "-f", "-q", "0.7"], (100, 150)),
    ("tiles422", ["-U", "2", "2", "-d", "2", "-l", "2", "-q", "0.5"], (100, 150)),
    ("tiles420", ["-U", "2", "2", "-d", "1", "-l", "2", "-q", "0.5"], (100, 150)),
    ("trim", ["-F", "6", "-q", "0.9"], (45, 67)),
    ("trimsp", ["-F", "4", "-f", "-q", "0.9"], (45, 67)),
    ("skiphp", ["-s", "2"], (45, 67)),
    ("skipflex", ["-s", "1"], (45, 67)),
    ("dconly", ["-s", "3"], (45, 67)),
]

ALPHA_FORMATS = {22, 23, 24, 25, 27, 28, 32, 33}

# JxrEncApp's TIFF reader flags every input with more than three samples as having alpha, so it
# cannot read plain CMYK. Those cases encode 40bppCMYKAlpha with planar alpha and drop the alpha
# plane afterwards: the primary codestream is unchanged apart from its ALPHA_IMAGE_PLANE_FLAG.
STRIP_ALPHA = {17: (32, 0x1C)}


# JxrDecApp keeps TRIM_FLEXBITS in the primary plane's coding context only and reads an
# interleaved alpha plane's flexbits untrimmed, while JxrEncApp (and T.832) trims every plane of
# the tile. JxrDecApp misdecodes these files (garbage in spatial mode), so they are checked
# against JxrDecApp's decode of the same image with planar alpha (decoded first): the colour
# channels must match exactly and the (separately quantised) alpha must be closer to it than
# JxrDecApp's own interleaved decode is.
def oracle_bug(opts, aopt):
    return "-F" in opts and aopt[:2] == ["-a", "3"]


def check_trim_alpha(got, bad, planar, width):
    dt = {1: np.uint8, 2: np.uint16, 4: np.uint32}[width]
    got, bad, planar = (np.frombuffer(x, dt).reshape(-1, 4).astype(np.float64) for x in (got, bad, planar))
    if not np.array_equal(got[:, :3], planar[:, :3]):
        return "colour differs from the planar-alpha decode"
    mine, theirs = (np.abs(x[:, 3] - planar[:, 3]).mean() for x in (got, bad))
    if mine > theirs:
        return f"alpha further from planar ({mine:.1f}) than JxrDecApp ({theirs:.1f})"
    return None


def ifd_entries(d):
    (off,) = struct.unpack("<I", d[4:8])
    (n,) = struct.unpack("<H", d[off : off + 2])
    return [(off + 2 + 12 * k,) + struct.unpack("<HHII", d[off + 2 + 12 * k : off + 14 + 12 * k]) for k in range(n)]


def strip_alpha(path, guid_last):
    d = bytearray(open(path, "rb").read())
    image = None
    for pos, tag, _ty, _cnt, val in ifd_entries(d):
        if tag == 0xBC01:
            d[val + 15] = guid_last
        elif tag == 0xBCC0:
            image = val
        elif tag in (0xBCC2, 0xBCC3):
            struct.pack_into("<H", d, pos, 0xFFF0 + (tag & 1))  # unknown private tags
    assert d[image : image + 8] == b"WMPHOTO\0"
    d[image + 10] &= 0xFE
    open(path, "wb").write(bytes(d))


def run(cmd):
    return subprocess.run(cmd, capture_output=True, text=True)


def main():
    work, exe = sys.argv[1], sys.argv[2]
    filt = sys.argv[3] if len(sys.argv) > 3 else ""
    os.makedirs(work, exist_ok=True)
    ok = bad = skipped = 0
    for fname, fmt, kind in FORMATS:
        for mname, opts, (h, w) in MODES:
            alphas = [["-a", "2"], ["-a", "3", "-Q", "4"]] if fmt in ALPHA_FORMATS else [[]]
            for ai, aopt in enumerate(alphas):
                name = f"{fname}-{mname}" + (f"-a{ai + 2}" if aopt else "")
                if filt and filt not in name:
                    continue
                src = os.path.join(work, f"in-{kind}-{h}x{w}.tif")
                if not os.path.exists(src):
                    inputs(src, kind, h, w, h * 7 + w)
                jxr = os.path.join(work, name + ".jxr")
                efmt, strip = (STRIP_ALPHA[fmt][0], STRIP_ALPHA[fmt][1]) if fmt in STRIP_ALPHA else (fmt, None)
                eopt = ["-a", "2"] if strip is not None else aopt
                r = run(["JxrEncApp", "-i", src, "-o", jxr, "-c", str(efmt)] + opts + eopt)
                if r.returncode != 0 or not os.path.getsize(jxr):
                    skipped += 1
                    print(f"SKIP {name}: encoder failed")
                    continue
                if strip is not None:
                    strip_alpha(jxr, strip)
                tif = os.path.join(work, name + ".tif")
                if os.path.exists(tif):
                    os.remove(tif)
                r = run(["JxrDecApp", "-i", jxr, "-o", tif])
                if r.returncode != 0 or not os.path.exists(tif):
                    skipped += 1
                    print(f"SKIP {name}: oracle decoder failed")
                    continue
                _, want = tiffraw.read(tif)
                raw = os.path.join(work, name + ".raw")
                r = run([exe, jxr, raw])
                if r.returncode != 0:
                    bad += 1
                    print(f"FAIL {name}: {r.stderr.strip()}")
                    continue
                got = open(raw, "rb").read()
                if fname == "rgbe" and "-d" in opts and opts[opts.index("-d") + 1] == "0":
                    # JxrDecApp formats YONLY-coded RGBE through its Y_ONLY path (one biased byte
                    # per pixel, the rest zero), so only check for grey shared-exponent pixels.
                    px = np.frombuffer(got, np.uint8).reshape(-1, 4)
                    grey = (px[:, 0] == px[:, 1]).all() and (px[:, 1] == px[:, 2]).all() and px[:, 3].any()
                    ok, bad = (ok + 1, bad) if grey else (ok, bad + 1)
                    if not grey:
                        print(f"FAIL {name}: not grey")
                    continue
                if oracle_bug(opts, aopt):
                    planar = os.path.join(work, f"{fname}-{mname}-a2.tif")
                    width = len(want) // (h * w * 4)
                    why = check_trim_alpha(got, want, tiffraw.read(planar)[1], width)
                    if why is None:
                        ok += 1
                    else:
                        bad += 1
                        print(f"FAIL {name}: {why}")
                    continue
                if fmt == 1:
                    # Native BlackWhite output has one byte per pixel; the TIFF packs rows MSB first.
                    got = np.packbits(np.frombuffer(got, np.uint8).reshape(h, w), axis=1).tobytes()
                if got == want:
                    ok += 1
                else:
                    bad += 1
                    n = min(len(got), len(want))
                    first = next((i for i in range(n) if got[i] != want[i]), n)
                    diff = sum(1 for i in range(n) if got[i] != want[i])
                    print(f"FAIL {name}: len {len(got)} vs {len(want)}, first diff @{first}, {diff} bytes differ")
    print(f"{ok} ok, {bad} failed, {skipped} skipped")


if __name__ == "__main__":
    main()
