"""Minimal reader for the uncompressed TIFFs JxrDecApp writes: returns (tags, raw strip bytes)."""

import struct
import sys


def read(path):
    d = open(path, "rb").read()
    le = d[:2] == b"II"
    e = "<" if le else ">"
    (off,) = struct.unpack(e + "I", d[4:8])
    (n,) = struct.unpack(e + "H", d[off : off + 2])
    tags = {}
    sizes = {1: 1, 2: 1, 3: 2, 4: 4, 5: 8, 6: 1, 7: 1, 8: 2, 9: 4, 10: 8, 11: 4, 12: 8}
    fmts = {1: "B", 3: "H", 4: "I", 8: "h", 9: "i"}
    for k in range(n):
        p = off + 2 + 12 * k
        tag, ty, cnt = struct.unpack(e + "HHI", d[p : p + 8])
        size = sizes.get(ty, 1) * cnt
        vo = p + 8 if size <= 4 else struct.unpack(e + "I", d[p + 8 : p + 12])[0]
        if ty in fmts:
            f = fmts[ty]
            w = struct.calcsize(f)
            tags[tag] = [struct.unpack(e + f, d[vo + i * w : vo + (i + 1) * w])[0] for i in range(cnt)]
        else:
            tags[tag] = d[vo : vo + size]
    offs = tags.get(273, [])
    counts = tags.get(279, [])
    raw = b"".join(d[o : o + c] for o, c in zip(offs, counts))
    return tags, raw


if __name__ == "__main__":
    t, raw = read(sys.argv[1])
    for k, v in sorted(t.items()):
        if isinstance(v, list) and len(v) > 8:
            v = v[:8] + ["..."]
        print(k, v)
    print("raw", len(raw))
