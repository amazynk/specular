#!/usr/bin/env python3
"""Per-type adjustment-layer files for the PSD composite oracle
(crates/formats/specular-psd/tests/compose_oracle.rs): from each PSD named on the command
line, one copy per visible adjustment layer with every other adjustment layer hidden, plus a
copy with all of them hidden ("none"), written to OUT/<stem>-<key>.psd.

The copies keep the original merged image, which no longer matches: the oracle compares them
with psd-tools' own composite instead (tools/corpus/psd-compose-oracle.py, files under
`psd/solo/`). Only the visibility flag byte of layer records changes.

    psd-adjust-solo.py OUT FILE.psd...
"""
import os
import struct
import sys

ADJ = {b"brit", b"levl", b"curv", b"expA", b"vibA", b"hue2", b"blnc", b"blwh", b"phfl",
       b"mixr", b"clrL", b"nvrt", b"post", b"thrs", b"grdm", b"selc"}
WIDE = {b"LMsk", b"Lr16", b"Lr32", b"Layr", b"Mt16", b"Mt32", b"Mtrn", b"Alph", b"FMsk",
        b"lnk2", b"FEid", b"FXid", b"PxSD"}


def records(d):
    """(flags offset, adjustment key or None, visible) of each layer record."""
    psb = struct.unpack(">H", d[4:6])[0] == 2
    p = 26
    p += 4 + struct.unpack(">I", d[p:p + 4])[0]
    p += 4 + struct.unpack(">I", d[p:p + 4])[0]
    p += 8 if psb else 4
    p += 8 if psb else 4
    cnt = abs(struct.unpack(">h", d[p:p + 2])[0])
    p += 2
    out = []
    for _ in range(cnt):
        p += 16
        nch = struct.unpack(">H", d[p:p + 2])[0]
        p += 2 + nch * (10 if psb else 6) + 10
        flags_at = p
        p += 2
        el = struct.unpack(">I", d[p:p + 4])[0]
        p += 4
        end = p + el
        q = p
        q += 4 + struct.unpack(">I", d[q:q + 4])[0]
        q += 4 + struct.unpack(">I", d[q:q + 4])[0]
        q += ((1 + d[q] + 3) // 4) * 4
        key = None
        while q + 12 <= end:
            k = d[q + 4:q + 8]
            if psb and k in WIDE:
                ln = struct.unpack(">Q", d[q + 8:q + 16])[0]
                q += 16
            else:
                ln = struct.unpack(">I", d[q + 8:q + 12])[0]
                q += 12
            if k in ADJ and key is None:
                key = k
            q += ln
        out.append((flags_at, key, d[flags_at] & 2 == 0))
        p = end
    return out


def main():
    out = sys.argv[1]
    os.makedirs(out, exist_ok=True)
    for path in sys.argv[2:]:
        d = open(path, "rb").read()
        recs = records(d)
        adj = [r for r in recs if r[1] is not None and r[2]]
        stem = os.path.basename(os.path.dirname(path)) if os.path.basename(path) == "src.psd" \
            else os.path.splitext(os.path.basename(path))[0]
        for keep in adj + [None]:
            b = bytearray(d)
            for at, key, vis in adj:
                if (at, key, vis) != keep:
                    b[at] |= 2
            name = keep[1].decode().strip() if keep else "none"
            with open(os.path.join(out, f"{stem}-{name}.psd"), "wb") as f:
                f.write(b)
        print(f"{path}: {len(adj)} adjustment layers")


if __name__ == "__main__":
    main()
