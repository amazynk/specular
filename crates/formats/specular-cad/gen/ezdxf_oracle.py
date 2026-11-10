#!/usr/bin/env python3
"""Writes tests/data/oracle.tsv: what ezdxf (MIT) reads from each corpus file.

Usage: ezdxf_oracle.py CORPUS_DIR OUT_TSV

Per file: the $ACADVER, entity counts by type as stored (ENTITIES and every block
definition, from ezdxf's low-level tag loader, so nothing ezdxf adds or merges on load
counts; a POLYLINE's vertices, an INSERT's attributes and SEQENDs not counted), and the
model-space extents of the geometry (blocks expanded, curves exact, text, XLINE/RAY,
viewports and invisible entities left out: text extents depend on font metrics).

ezdxf is only an oracle for the tests; nothing of it is in the reader.
"""

import io
import os
import sys
from collections import Counter

import ezdxf
from ezdxf import bbox, disassemble, recover
from ezdxf.lldxf.loader import load_dxf_structure
from ezdxf.lldxf.tagger import ascii_tags_loader, binary_tags_loader

SENTINEL = b"AutoCAD Binary DXF\r\n\x1a\x00"
NOT_COUNTED = {"BLOCK", "ENDBLK", "VERTEX", "SEQEND", "ATTRIB"}
SKIPPED = {"TEXT", "MTEXT", "ATTRIB", "ATTDEF", "XLINE", "RAY", "VIEWPORT"}


def load(path):
    try:
        return ezdxf.readfile(path), "ok"
    except Exception:  # noqa: BLE001
        pass
    try:
        doc, _auditor = recover.readfile(path)
        return doc, "recovered"
    except Exception as e:  # noqa: BLE001
        return None, "error:" + type(e).__name__


def counts(path):
    with open(path, "rb") as f:
        data = f.read()
    if data.startswith(SENTINEL):
        tags = binary_tags_loader(data)
    else:
        tags = ascii_tags_loader(io.StringIO(data.decode("latin-1"), newline=None))
    sections = load_dxf_structure(tags, ignore_missing_eof=True)
    c = Counter()
    for name in ("ENTITIES", "BLOCKS"):
        for ent in sections.get(name, [])[1:]:
            ty = ent[0].value
            if ty not in NOT_COUNTED:
                c[ty] += 1
    return c


def extents(doc):
    """Geometry-only model extents: text, infinite lines, viewports and invisible
    entities left out (text size depends on fonts the reader does not have)."""
    msp = doc.modelspace()
    ext = bbox.BoundingBox()
    try:
        for e in disassemble.recursive_decompose(msp):
            if e.dxftype() in SKIPPED or e.dxf.get("invisible", 0):
                continue
            if e.dxftype() == "SPLINE":
                # ezdxf's path of a spline with unclamped knots strays far from the
                # curve; its own evaluation does not.
                ext.extend(e.construction_tool().approximate(512))
                continue
            for p in disassemble.to_primitives([e]):
                if p.is_empty:
                    continue
                b = p.bbox(fast=False)
                if b.has_data:
                    ext.extend(b)
    except Exception:  # noqa: BLE001
        return "-"
    if not ext.has_data:
        return "-"
    return "%r %r %r %r" % (ext.extmin.x, ext.extmin.y, ext.extmax.x, ext.extmax.y)


def main():
    root, out = sys.argv[1], sys.argv[2]
    files = []
    for d, _, names in os.walk(root):
        for n in names:
            if n.lower().endswith(".dxf"):
                files.append(os.path.relpath(os.path.join(d, n), root))
    files.sort()
    rows = ["# file\tstatus\tversion\tcounts\textents (ezdxf %s)" % ezdxf.__version__]
    for rel in files:
        path = os.path.join(root, rel)
        try:
            c = counts(path)
        except Exception as e:  # noqa: BLE001
            rows.append("\t".join([rel, "error:" + type(e).__name__, "-", "-", "-"]))
            continue
        cs = ",".join("%s=%d" % kv for kv in sorted(c.items())) or "-"
        doc, status = load(path)
        if doc is None:
            rows.append("\t".join([rel, status, "-", cs, "-"]))
            continue
        rows.append("\t".join([rel, status, doc.dxfversion, cs, extents(doc)]))
        print(rel, status, file=sys.stderr)
    with open(out, "w", encoding="utf-8") as f:
        f.write("\n".join(rows) + "\n")


if __name__ == "__main__":
    main()
