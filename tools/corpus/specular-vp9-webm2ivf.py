#!/usr/bin/env python3
"""Copies the video frames of a WebM file into an IVF file, every block as it is (ffmpeg's
remux drops the frames before the first keyframe, which the intra-only vectors start with).

Usage: specular-vp9-webm2ivf.py IN.webm OUT.ivf
Reads SimpleBlocks and Blocks of track 1 without lacing (what libvpx's vectors use)."""
import struct
import sys

CONTAINERS = {0x18538067, 0x1F43B675, 0xA0}  # Segment, Cluster, BlockGroup


def vint(b, p, keep_marker):
    first = b[p]
    n = 1
    while n <= 8 and not first & (0x80 >> (n - 1)):
        n += 1
    v = first if keep_marker else first & (0xFF >> n)
    for x in b[p + 1:p + n]:
        v = (v << 8) | x
    unknown = not keep_marker and v == (1 << (7 * n)) - 1
    return v, n, unknown


def walk(b, p, end, frames, size):
    while p < end:
        eid, n, _ = vint(b, p, True)
        ln, m, unknown = vint(b, p + n, False)
        body = p + n + m
        stop = end if unknown else min(end, body + ln)
        if eid in CONTAINERS:
            walk(b, body, stop, frames, size)
        elif eid in (0xA3, 0xA1):  # SimpleBlock, Block
            track, t, _ = vint(b, body, False)
            if track == 1:
                frames.append(b[body + t + 3:stop])
        elif eid == 0xB0:
            size[0] = int.from_bytes(b[body:stop], "big")
        elif eid == 0xBA:
            size[1] = int.from_bytes(b[body:stop], "big")
        elif eid == 0x1654AE6B or eid == 0xAE or eid == 0xE0:  # Tracks, TrackEntry, Video
            walk(b, body, stop, frames, size)
        p = stop


def main():
    data = open(sys.argv[1], "rb").read()
    frames, size = [], [0, 0]
    walk(data, 0, len(data), frames, size)
    with open(sys.argv[2], "wb") as f:
        f.write(b"DKIF" + struct.pack("<HH4sHHIIII", 0, 32, b"VP90", size[0], size[1], 1000, 1,
                                      len(frames), 0))
        for i, fr in enumerate(frames):
            f.write(struct.pack("<IQ", len(fr), i))
            f.write(fr)


if __name__ == "__main__":
    main()
