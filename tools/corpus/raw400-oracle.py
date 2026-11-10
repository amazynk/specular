"""Dump LibRaw's (rawpy's) view of the raw400 corpus for the specular-raw `raw400` test.

    $CARGO_TARGET_DIR/venv/bin/python -I tools/corpus/raw400-oracle.py tools/corpus/raw400

For every file LibRaw opens, writes under <corpus>/oracle/: `<name>.u16` (the raw image as
LibRaw unpacks it, little-endian u16, row-major; single-channel files only) and `<name>.txt`
(width height pattern black white make model). LibRaw is an oracle only: its numbers are
compared with ours, nothing of it is used by Specular.
"""
import os
import sys

import numpy as np
import rawpy

corp = sys.argv[1]
out = os.path.join(corp, "oracle")
os.makedirs(out, exist_ok=True)
for dp, dn, fn in sorted(os.walk(corp)):
    if os.path.abspath(dp).startswith(os.path.abspath(out)):
        continue
    for f in sorted(fn):
        if f.startswith(".") or f.endswith((".zip", ".txt")):
            continue
        p = os.path.join(dp, f)
        rel = os.path.relpath(p, corp).replace("/", "__")
        try:
            with rawpy.imread(p) as r:
                ri = r.raw_image
                if ri.ndim != 2:
                    continue
                pat = "".join(r.color_desc.decode()[i] for i in r.raw_pattern.flatten())
                np.ascontiguousarray(ri, dtype="<u2").tofile(os.path.join(out, rel + ".u16"))
                with open(os.path.join(out, rel + ".txt"), "w") as t:
                    t.write("%d %d %s %d %d\n" % (ri.shape[1], ri.shape[0], pat,
                                                  min(r.black_level_per_channel), r.white_level))
        except Exception:
            pass
