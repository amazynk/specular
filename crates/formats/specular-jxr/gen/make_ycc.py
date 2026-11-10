#!/usr/bin/env python3
"""Generates the YCC pixel-format test vectors in ../tests/vectors (ycc-*.jxr, ycc.txt).

jxrlib can neither encode nor fully decode the YCC pixel formats (JxrEncApp has no YCC input,
JxrDecApp and imagecodecs refuse YCC output with WMP_errUnsupportedFormat), so the vectors are
made by re-labelling JxrEncApp files whose internal colour format matches: the pixel-format GUID
and the codestream's OUTPUT_CLR_FMT are rewritten, the coded data is untouched. The checks:
  * the RGB hash: JxrDecApp's native decode of the file before re-labelling (the test restores
    the two fields and decodes it bit-exact);
  * the luma hash: JxrDecApp's decode of the YCC file to 8bppGray or 16bppGray (its only YCC
    output), which must equal the Y channel (8 and 16-bit integer formats only, else "-");
  * chroma: the test rebuilds RGB from the YCC samples with T.832's inverse colour transform.

Usage: python make_ycc.py OUT_DIR WORK_DIR
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import oracle  # noqa: E402
import tiffraw  # noqa: E402
from make_vectors import fnv  # noqa: E402

H, W = 45, 67
TILED = (100, 150)

# (name, -c format, input kind, JxrEncApp options, alpha option, YCC GUID last byte,
#  OUTPUT_CLR_FMT, JxrDecApp gray format for the luma check or None)
CASES = [
    ("ycc420-8-lossless", 9, "rgb8", ["-d", "1"], [], 0x44, 1, 2),
    ("ycc420-8-q", 9, "rgb8", ["-d", "1", "-q", "0.7"], [], 0x44, 1, 2),
    ("ycc420-8-tiles", 9, "rgb8", ["-d", "1", "-U", "2", "2", "-l", "2", "-q", "0.5"], [], 0x44, 1, 2),
    ("ycc422-8-lossless", 9, "rgb8", ["-d", "2"], [], 0x45, 2, 2),
    ("ycc422-8-q", 9, "rgb8", ["-d", "2", "-q", "0.7"], [], 0x45, 2, 2),
    ("ycc444-8-lossless", 9, "rgb8", ["-d", "3"], [], 0x48, 3, 2),
    ("ycc444-8-q", 9, "rgb8", ["-d", "3", "-q", "0.6"], [], 0x48, 3, 2),
    ("ycc444-16-q", 10, "rgb16", ["-q", "0.6"], [], 0x4A, 3, 3),
    ("ycc444-10-q", 31, "rgb16", ["-q", "0.6"], [], 0x49, 3, None),
    ("ycc444-16s-q", 11, "rgbf", ["-q", "0.6"], [], 0x4B, 3, None),
    ("ycc420a-8-q-a2", 22, "rgba8", ["-d", "1", "-q", "0.7"], ["-a", "2"], 0x4C, 1, 2),
    ("ycc420a-8-q-a3", 22, "rgba8", ["-d", "1", "-q", "0.7"], ["-a", "3", "-Q", "4"], 0x4C, 1, 2),
    ("ycc444a-8-lossless-a3", 22, "rgba8", ["-d", "3"], ["-a", "3"], 0x50, 3, 2),
    ("ycc444a-16-q-a2", 23, "rgba16", ["-q", "0.6"], ["-a", "2"], 0x52, 3, 3),
]


def relabel(d, out_fmt, guid_last):
    """Returns the original (GUID byte, OUTPUT_CLR_FMT) and rewrites them in `d`."""
    image = None
    old_guid = None
    for _pos, tag, _ty, _cnt, val in oracle.ifd_entries(d):
        if tag == 0xBC01:
            old_guid = d[val + 15]
            d[val + 15] = guid_last
        elif tag == 0xBCC0:
            image = val
    assert d[image : image + 8] == b"WMPHOTO\0"
    old_fmt = d[image + 11] >> 4
    d[image + 11] = (out_fmt << 4) | (d[image + 11] & 15)
    return old_guid, old_fmt


def decode(jxr, path, fmt=None):
    if os.path.exists(path):
        os.remove(path)
    r = oracle.run(["JxrDecApp", "-i", jxr, "-o", path] + (["-c", str(fmt)] if fmt is not None else []))
    if r.returncode != 0 or not os.path.exists(path):
        sys.exit(f"{jxr}: oracle decoder failed")
    return tiffraw.read(path)[1]


def main():
    out, work = sys.argv[1], sys.argv[2]
    os.makedirs(out, exist_ok=True)
    os.makedirs(work, exist_ok=True)
    lines = ["# name width height rgb-hash luma-hash (fnv1a64, little-endian) - make_ycc.py"]
    for name, fmt, kind, opts, aopt, guid, out_fmt, gray in CASES:
        h, w = TILED if "tiles" in name else (H, W)
        src = os.path.join(work, f"in-{kind}-{h}x{w}.tif")
        if not os.path.exists(src):
            oracle.inputs(src, kind, h, w, h * 7 + w)
        enc = os.path.join(work, name + ".enc.jxr")
        r = oracle.run(["JxrEncApp", "-i", src, "-o", enc, "-c", str(fmt)] + opts + aopt)
        if r.returncode != 0:
            sys.exit(f"{name}: encoder failed")
        rgb = decode(enc, os.path.join(work, name + ".rgb.tif"))
        d = bytearray(open(enc, "rb").read())
        relabel(d, out_fmt, guid)
        jxr = os.path.join(out, name + ".jxr")
        open(jxr, "wb").write(bytes(d))
        luma = "-"
        if gray is not None:
            luma = f"{fnv(decode(jxr, os.path.join(work, name + '.y.tif'), gray)):016x}"
        lines.append(f"{name} {w} {h} {fnv(rgb):016x} {luma}")
        print(lines[-1])
    with open(os.path.join(out, "ycc.txt"), "w") as f:
        f.write("\n".join(lines) + "\n")


if __name__ == "__main__":
    main()
