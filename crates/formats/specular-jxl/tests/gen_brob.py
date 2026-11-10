"""Rewrite a JPEG XL container's metadata boxes as Brotli-compressed `brob` boxes.

    python3 -I gen_brob.py IN.jxl OUT.jxl [--xmp XMP_FILE]

Each top-level `Exif`, `xml ` and `jhgm` box becomes a `brob` box (ISO/IEC 18181-2: the
inner box type, then the Brotli-compressed body) in the same place. With --xmp, an `xml `
box holding XMP_FILE is added (compressed too) after the `ftyp` box. Needs Python's `brotli`
module.

Fixtures made with it (the inputs are committed fixtures themselves):

    specular-jxl/tests/data/gainmap_hdr_brob.jxl     from gainmap_hdr.jxl
    specular-thumbs/tests/data/gpsmeta/gps_brob.jxl  from gps.jxl, --xmp gps_brob.xmp
"""

import struct
import sys

import brotli

COMPRESS = {b"Exif", b"xml ", b"jhgm"}


def boxes(data):
    i = 0
    while i < len(data):
        size, kind = struct.unpack(">I4s", data[i : i + 8])
        head = 8
        if size == 1:
            size = struct.unpack(">Q", data[i + 8 : i + 16])[0]
            head = 16
        elif size == 0:
            size = len(data) - i
        yield kind, data[i + head : i + size]
        i += size


def box(kind, body):
    return struct.pack(">I", 8 + len(body)) + kind + body


def brob(kind, body):
    return box(b"brob", kind + brotli.compress(body, quality=11))


def main():
    args = sys.argv[1:]
    xmp = None
    if "--xmp" in args:
        k = args.index("--xmp")
        with open(args[k + 1], "rb") as f:
            xmp = f.read()
        del args[k : k + 2]
    src, dst = args
    with open(src, "rb") as f:
        data = f.read()
    out = b""
    for kind, body in boxes(data):
        out += brob(kind, body) if kind in COMPRESS else box(kind, body)
        if kind == b"ftyp" and xmp is not None:
            out += brob(b"xml ", xmp)
    with open(dst, "wb") as f:
        f.write(out)


if __name__ == "__main__":
    main()
