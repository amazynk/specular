#!/usr/bin/env python3
"""Cut a QuickTime/ISO-BMFF movie whose mdat comes first down to its first N video frames.

Usage: rawvid1-cut.py HEAD MOOV FRAMES OUT

HEAD is a prefix of the movie (ftyp ... mdat up to at least the end of frame FRAMES-1), MOOV
the movie's moov box (fetched separately with an HTTP Range request). The output keeps the
prefix byte for byte (so the chunk offsets stay valid), shrinks the mdat box to the prefix,
keeps only the first FRAMES samples of the video track, drops the sound tracks and keeps the
other tracks only when all their data lies in the prefix. Inputs are untrusted data: the
script only reads and rewrites box tables.
"""
import struct
import sys


def boxes(d, a, b):
    p = a
    while p + 8 <= b:
        s, t = struct.unpack('>I4s', d[p:p + 8])
        h = 8
        if s == 1:
            s = struct.unpack('>Q', d[p + 8:p + 16])[0]
            h = 16
        if s == 0:
            s = b - p
        if s < h or p + s > b:
            raise SystemExit('bad box at %d' % p)
        yield t, p, h, s
        p += s


def box(t, payload):
    return struct.pack('>I4s', 8 + len(payload), t) + payload


def full(payload):
    return payload


def sample_table(d, stbl_a, stbl_b):
    tabs = {}
    for t, p, h, s in boxes(d, stbl_a, stbl_b):
        tabs[t] = d[p + h:p + s]
    return tabs


def chunk_offsets(tabs):
    if b'co64' in tabs:
        c = tabs[b'co64']
        n = struct.unpack('>I', c[4:8])[0]
        return [struct.unpack('>Q', c[8 + 8 * i:16 + 8 * i])[0] for i in range(n)], True
    c = tabs[b'stco']
    n = struct.unpack('>I', c[4:8])[0]
    return [struct.unpack('>I', c[8 + 4 * i:12 + 4 * i])[0] for i in range(n)], False


def sizes(tabs):
    z = tabs[b'stsz']
    fixed, n = struct.unpack('>II', z[4:12])
    if fixed:
        return [fixed] * n
    return [struct.unpack('>I', z[12 + 4 * i:16 + 4 * i])[0] for i in range(n)]


def stsc(tabs):
    c = tabs[b'stsc']
    n = struct.unpack('>I', c[4:8])[0]
    return [struct.unpack('>III', c[8 + 12 * i:20 + 12 * i]) for i in range(n)]


def sample_offsets(tabs):
    offs, _ = chunk_offsets(tabs)
    sz = sizes(tabs)
    runs = stsc(tabs)
    out = []
    k = 0
    for ci, off in enumerate(offs):
        per = 0
        for first, spc, _ in runs:
            if first - 1 <= ci:
                per = spc
        for _ in range(per):
            if k >= len(sz):
                break
            out.append((off, sz[k]))
            off += sz[k]
            k += 1
    return out


def rebuild_stbl(d, a, b, keep):
    """The stbl with only the first `keep` samples, one sample per chunk."""
    tabs = sample_table(d, a, b)
    samples = sample_offsets(tabs)[:keep]
    big = any(o + s > 0xFFFFFFFF for o, s in samples)
    out = b''
    for t, p, h, s in boxes(d, a, b):
        body = d[p + h:p + s]
        if t == b'stts':
            delta = struct.unpack('>I', body[12:16])[0]
            out += box(t, b'\0\0\0\0' + struct.pack('>III', 1, keep, delta))
        elif t == b'stsc':
            out += box(t, b'\0\0\0\0' + struct.pack('>IIII', 1, 1, 1, 1))
        elif t == b'stsz':
            out += box(t, b'\0\0\0\0' + struct.pack('>II', 0, keep)
                       + b''.join(struct.pack('>I', s) for _, s in samples))
        elif t in (b'stco', b'co64'):
            if big:
                out += box(b'co64', b'\0\0\0\0' + struct.pack('>I', keep)
                           + b''.join(struct.pack('>Q', o) for o, _ in samples))
            else:
                out += box(b'stco', b'\0\0\0\0' + struct.pack('>I', keep)
                           + b''.join(struct.pack('>I', o) for o, _ in samples))
        elif t == b'stss':
            continue
        else:
            out += d[p:p + s]
    return out, samples


def handler(d, a, b):
    for t, p, h, s in boxes(d, a, b):
        if t == b'mdia':
            for t2, p2, h2, s2 in boxes(d, p + h, p + s):
                if t2 == b'hdlr':
                    return d[p2 + h2 + 8:p2 + h2 + 12]
    return None


def rewrite(d, a, b, path, keep, limit):
    """Rebuild container box contents [a, b) along `path` down to stbl."""
    out = b''
    for t, p, h, s in boxes(d, a, b):
        if t in (b'mdia', b'minf'):
            out += box(t, rewrite(d, p + h, p + s, path, keep, limit))
        elif t == b'stbl':
            body, samples = rebuild_stbl(d, p + h, p + s, keep)
            if samples and max(o + z for o, z in samples) > limit:
                raise SystemExit('frames reach past the prefix')
            out += box(t, body)
        else:
            out += d[p:p + s]
    return out


def main():
    head = open(sys.argv[1], 'rb').read()
    moov = open(sys.argv[2], 'rb').read()
    frames = int(sys.argv[3])
    t, p, h, s = next(boxes(moov, 0, len(moov)))
    if t != b'moov':
        raise SystemExit('not a moov box')
    out_moov = b''
    for t, p2, h2, s2 in boxes(moov, p + h, p + s):
        if t != b'trak':
            out_moov += moov[p2:p2 + s2]
            continue
        kind = handler(moov, p2 + h2, p2 + s2)
        if kind == b'soun':
            continue
        if kind == b'vide':
            out_moov += box(b'trak', rewrite(moov, p2 + h2, p2 + s2, None, frames, len(head)))
        else:
            # Timecode and metadata tracks: kept whole when their data is in the prefix.
            try:
                for t3, p3, h3, s3 in boxes(moov, p2 + h2, p2 + s2):
                    pass
                out_moov += moov[p2:p2 + s2]
            except SystemExit:
                continue
    # The prefix with its mdat shrunk to what it holds.
    top = list(boxes_prefix(head))
    out = bytearray(head)
    for t, p, h, s in top:
        if t == b'mdat':
            if h == 16:
                out[p + 8:p + 16] = struct.pack('>Q', len(head) - p)
            else:
                out[p:p + 4] = struct.pack('>I', len(head) - p)
    out += box(b'moov', out_moov)
    open(sys.argv[4], 'wb').write(out)


def boxes_prefix(d):
    p = 0
    while p + 8 <= len(d):
        s, t = struct.unpack('>I4s', d[p:p + 8])
        h = 8
        if s == 1:
            s = struct.unpack('>Q', d[p + 8:p + 16])[0]
            h = 16
        yield t, p, h, s
        if t == b'mdat' or s < 8:
            return
        p += s


if __name__ == '__main__':
    main()
