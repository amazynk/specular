#!/usr/bin/env python3
"""Oracle for the inspection report's DOCX writer (specular-markup `docx`).

Opens the dumped reports with python-docx and checks what a reader sees: the title and
notes paragraphs, the tables (facts and measurements, with their cells), the inline
pictures (count, PNG, size on the page) and the right-to-left settings. Optionally converts
them with LibreOffice when `soffice` is already installed (never installed by this script).

    DOCX_ORACLE_DIR=<dir> cargo test -p specular-markup docx_dump -- --ignored
    python3 -I tools/quality/docx_oracle.py <dir>
"""
import pathlib
import shutil
import subprocess
import sys

import docx
from docx.oxml.ns import qn

WANT_TABLES = [
    [["Camera", "FC6520 & <test>"], ["Place", "47.37010, 8.54600"]],
    [["#", "Measurement", "Value"], ["1", "Crack width", "Mean width 0.61 mm"],
     ["12", None, "4.2 cm²"]],
    [["Place", "47.37020, 8.54610"]],
]


def check(path, rtl):
    bad = 0
    checked = 0
    d = docx.Document(str(path))
    paras = [p.text for p in d.paragraphs]

    def expect(what, ok):
        nonlocal bad, checked
        checked += 1
        if not ok:
            bad += 1
            print(f"  MISMATCH {path.name}: {what}")

    expect("title first", paras[0] == ("דוח" if rtl else "Inspection report"))
    expect("title style", d.paragraphs[0].style.name == "Title")
    expect("notes kept with a line break",
           any(p == "Bridge pier 3, north face.\nSecond line." for p in paras))
    expect("headings per image",
           [p.text for p in d.paragraphs if p.style.name == "Heading 1"]
           == ["DJI_0001.JPG", "DJI_0002.JPG"])
    expect("no-measurements text", "No measurements" in paras)
    expect("table count", len(d.tables) == 3)
    for t, want in zip(d.tables, WANT_TABLES):
        cells = [[c.text for c in r.cells] for r in t.rows]
        expect(f"table rows {len(cells)}", len(cells) == len(want))
        for r, w in zip(cells, want):
            for c, wc in zip(r, w):
                if wc is not None:
                    expect(f"cell {c!r} == {wc!r}", c == wc)
    shapes = d.inline_shapes
    expect("three pictures", len(shapes) == 3)
    for s in shapes:
        # Within the 165 × 115 mm box, aspect kept by the writer.
        w_mm, h_mm = s.width / 36000, s.height / 36000
        expect(f"picture size {w_mm:.1f} × {h_mm:.1f} mm", w_mm <= 165.01 and h_mm <= 115.01)
        blip = s._inline.graphic.graphicData.pic.blipFill.blip
        part = d.part.related_parts[blip.embed]
        expect("picture is PNG", part.content_type == "image/png" and part.blob[:8] == b"\x89PNG\r\n\x1a\n")
    aspect = [round(s.width / s.height, 3) for s in shapes]
    expect(f"aspects {aspect}", aspect == [round(320 / 240, 3), round(160 / 120, 3), round(200 / 300, 3)])
    bidi = d.element.body.findall(".//" + qn("w:bidi"))
    vis = d.element.body.findall(".//" + qn("w:bidiVisual"))
    expect("right to left set" if rtl else "left to right", bool(bidi) == rtl and bool(vis) == rtl)
    expect("page size A4", abs(d.sections[0].page_width.mm - 210.0) < 0.5)
    return checked, bad


def main(d):
    total = bad = 0
    for name, rtl in [("report-ltr.docx", False), ("report-rtl.docx", True)]:
        c, b = check(d / name, rtl)
        total += c
        bad += b
    soffice = shutil.which("soffice") or shutil.which("libreoffice")
    if soffice:
        out = d / "pdf"
        out.mkdir(exist_ok=True)
        # A profile of its own beside the output, never the user's (~/.config/libreoffice).
        profile = (d / "lo-profile").resolve().as_uri()
        r = subprocess.run([soffice, f"-env:UserInstallation={profile}", "--headless",
                            "--convert-to", "pdf", "--outdir", str(out),
                            str(d / "report-ltr.docx")], capture_output=True, timeout=180)
        ok = r.returncode == 0 and (out / "report-ltr.pdf").exists()
        print(f"LibreOffice conversion: {'ok' if ok else 'FAILED'}")
        total += 1
        bad += 0 if ok else 1
    else:
        print("LibreOffice not installed: conversion not checked")
    print(f"{total} checks, {bad} mismatches")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(pathlib.Path(sys.argv[1])))
