"""Cuts an AVI written by FFmpeg into the test fixture layout: u32 width, u32 height,
u32 extradata size, extradata, then per frame u32 size and the frame (all big-endian).

    python3 -I avi_frames.py IN.avi OUT.frames
"""

import struct
import sys


def chunks(d, at, end):
    while at + 8 <= end:
        cid = d[at : at + 4]
        size = struct.unpack("<I", d[at + 4 : at + 8])[0]
        yield cid, at + 8, size
        at += 8 + size + (size & 1)


def main():
    d = open(sys.argv[1], "rb").read()
    w = h = 0
    extra = b""
    frames = []

    def walk(at, end):
        nonlocal w, h, extra
        for cid, body, size in chunks(d, at, end):
            if cid in (b"RIFF", b"LIST"):
                kind = d[body : body + 4]
                if kind in (b"AVI ", b"hdrl", b"strl", b"movi", b"rec "):
                    walk(body + 4, body + size)
            elif cid == b"strf" and w == 0:
                w, h = struct.unpack("<ii", d[body + 4 : body + 12])
                h = abs(h)
                extra = d[body + 40 : body + size]
            elif cid[2:] in (b"dc", b"db") and cid[:2] == b"00":
                frames.append(d[body : body + size])

    walk(0, len(d))
    out = struct.pack(">III", w, h, len(extra)) + extra
    for f in frames:
        out += struct.pack(">I", len(f)) + f
    open(sys.argv[2], "wb").write(out)


main()
