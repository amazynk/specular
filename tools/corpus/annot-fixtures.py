"""annot-fixtures.py [out dir]: synthetic slide annotation sidecars for specular-wsi's tests

Writes NDP.view (.ndpa), Aperio ImageScope (.xml, UTF-8 with a BOM and UTF-16) and ASAP
(.xml) files into crates/formats/specular-wsi/tests/data/annot (or the folder given): every
shape kind each format has, names, layers and groups, colours, shuffled ASAP point orders,
decimal commas and unreadable shapes. Deterministic; plain python. The expected values come
from annot-oracle.py.

The NDPA file belongs to a slide 4000 x 3000 pixels of 0.25 µm whose centre lies
(1000, -2000) nm from the centre of the glass (written to frame.txt for the oracle and tests).
"""
import math
import sys
from pathlib import Path

OUT = Path(sys.argv[1]) if len(sys.argv) > 1 else (
    Path(__file__).resolve().parent.parent.parent / "crates/formats/specular-wsi/tests/data/annot"
)

# nm per pixel x / y, slide offset x / y (nm), width, height.
FRAME = (250.0, 250.0, 1000.0, -2000.0, 4000, 3000)


def star(cx, cy, r0, r1, n):
    pts = []
    for i in range(2 * n):
        r = r1 if i % 2 == 0 else r0
        a = math.pi * i / n
        pts.append((round(cx + r * math.cos(a), 3), round(cy + r * math.sin(a), 3)))
    return pts


def blob(cx, cy, r, n):
    return [
        (round(cx + r * (1 + 0.2 * math.sin(5 * t)) * math.cos(t), 4),
         round(cy + r * (1 + 0.2 * math.sin(5 * t)) * math.sin(t), 4))
        for t in (2 * math.pi * i / n for i in range(n))
    ]


def aperio_region(rid, typ, text, pts):
    v = "".join(f'<Vertex X="{x}" Y="{y}" Z="0"/>' for x, y in pts)
    return (f'<Region Id="{rid}" Type="{typ}" Zoom="0.5" Text="{text}" NegativeROA="0">'
            f"<Attributes/><Vertices>{v}</Vertices></Region>\n")


def aperio():
    tumour = "".join([
        aperio_region(1, 0, "star", star(800, 600, 60, 140, 12)),
        aperio_region(2, 1, "box", [(100, 100), (420, 100), (420, 300), (100, 300)]),
        aperio_region(3, 2, "oval", [(1500, 900), (1900, 1150)]),
        aperio_region(4, 3, "look here", [(2000, 400), (2200, 520)]),
        aperio_region(5, 4, "r1", [(300, 2500), (1100, 2900)]),
    ])
    stroma = "".join([
        aperio_region(6, 0, "", [(2500.5, 1700.25)]),
        aperio_region(7, 0, "lobe", blob(3000, 2000, 300, 400)),
        # Too few vertices for a region: left out.
        aperio_region(8, 0, "empty", []),
    ])
    return (
        '<?xml version="1.0" encoding="UTF-8"?>\n'
        '<Annotations MicronsPerPixel="0.250000">\n'
        '<Annotation Id="1" Name="Tumor" ReadOnly="0" NameReadOnly="0" LineColorReadOnly="0" '
        'Incremental="0" Type="4" LineColor="255" Visible="1" Selected="1" MarkupImagePath="" '
        f'MacroName=""><Attributes/><Regions><RegionAttributeHeaders/>\n{tumour}</Regions>'
        "<Plots/></Annotation>\n"
        '<Annotation Id="2" Name="" LineColor="16711680" Visible="1"><Attributes>'
        '<Attribute Name="Stroma" Id="0" Value=""/></Attributes><Regions>\n'
        f"{stroma}</Regions><Plots/></Annotation>\n"
        "</Annotations>\n"
    )


def aperio_utf16():
    regions = aperio_region(1, 0, "Fläche ü", [(10, 10), (60, 10), (60, 50), (10, 50)])
    return (
        '<?xml version="1.0" encoding="UTF-16"?>\n<Annotations>\n'
        '<Annotation Id="7" LineColor="65535"><Regions>\n'
        f"{regions}</Regions></Annotation>\n</Annotations>\n"
    )


def asap_annotation(name, typ, group, coords, colour="#F4FA58", order=None, comma=False):
    idx = list(range(len(coords)))
    if order:
        idx = order
    fmt = (lambda v: str(v).replace(".", ",")) if comma else str
    c = "".join(f'<Coordinate Order="{i}" X="{fmt(coords[i][0])}" Y="{fmt(coords[i][1])}"/>' for i in idx)
    return (f'<Annotation Name="{name}" Type="{typ}" PartOfGroup="{group}" Color="{colour}">'
            f"<Coordinates>{c}</Coordinates></Annotation>\n")


def asap():
    poly = blob(1200, 1500, 220, 60)
    n = len(poly)
    shuffled = [(i * 7) % n for i in range(n)]
    ann = "".join([
        asap_annotation("Annotation 0", "Polygon", "_0", poly, order=shuffled),
        asap_annotation("Annotation 1", "Spline", "_0", star(400, 400, 30, 80, 5)),
        asap_annotation("Annotation 2", "Rectangle", "_1", [(50, 60), (250, 60), (250, 160), (50, 160)]),
        asap_annotation("Annotation 3", "Dot", "None", [(77.5, 88.25)], colour="#00FFFF"),
        asap_annotation("Annotation 4", "PointSet", "_2", [(10, 10), (20, 15), (30, 40), (45.5, 12), (60, 70)]),
        asap_annotation("Annotation 5", "Measurement", "None", [(100, 1000), (400, 1400)], colour="#FFFFFF"),
        asap_annotation("Annotation 6", "Polygon", "_2", [(2000.5, 100.25), (2300.75, 120.5), (2150.125, 400.0)],
                        comma=True),
        # Group colour when the annotation gives none.
        '<Annotation Name="Annotation 7" Type="Polygon" PartOfGroup="_1"><Coordinates>'
        '<Coordinate Order="0" X="0" Y="0"/><Coordinate Order="1" X="30" Y="0"/>'
        '<Coordinate Order="2" X="30" Y="30"/></Coordinates></Annotation>\n',
        # Unreadable: no coordinates.
        '<Annotation Name="Annotation 8" Type="Polygon" PartOfGroup="_0" Color="#FF0000"><Coordinates/></Annotation>\n',
    ])
    groups = (
        '<Group Name="_0" PartOfGroup="None" Color="#FF0000"><Attributes/></Group>\n'
        '<Group Name="_1" PartOfGroup="None" Color="#00FF00"><Attributes/></Group>\n'
        '<Group Name="_2" PartOfGroup="None" Color="#0000FF"><Attributes/></Group>\n'
    )
    return (f'<?xml version="1.0"?>\n<ASAP_Annotations>\n<Annotations>\n{ann}</Annotations>\n'
            f"<AnnotationGroups>\n{groups}</AnnotationGroups>\n</ASAP_Annotations>\n")


def nm(px, py):
    """Slide pixels to NDPA nanometres from the glass centre."""
    sx, sy, ox, oy, w, h = FRAME
    return (round((px - w / 2) * sx + ox), round((py - h / 2) * sy + oy))


def ndpa_state(i, title, body):
    return (f'<ndpviewstate id="{i}"><title>{title}</title><details/><coordformat>nanometers'
            f"</coordformat><lens>20.0</lens><x>0</x><y>0</y><z>0</z><showtitle>1</showtitle>"
            f"<showhistogram>0</showhistogram><showlineprofile>0</showlineprofile>{body}</ndpviewstate>\n")


def pointlist(pts):
    return "<pointlist>" + "".join(
        "<point><x>{}</x><y>{}</y></point>".format(*nm(x, y)) for x, y in pts
    ) + "</pointlist>"


def ndpa():
    states = [
        ndpa_state(1, "Tumour", '<annotation type="freehand" displayname="AnnotateFreehand" color="#ff0000">'
                   f"<measuretype>0</measuretype><closed>1</closed>{pointlist(star(2000, 1500, 100, 250, 9))}"
                   "</annotation>"),
        ndpa_state(2, "Margin", '<annotation type="freehand" displayname="AnnotateFreehandLine" color="#00ff00">'
                   f"<closed>0</closed>{pointlist(blob(1000, 1000, 150, 40)[:25])}</annotation>"),
        ndpa_state(3, "Field", '<annotation type="freehand" displayname="AnnotateRectangle" '
                   'specialtype="rectangle" color="#ffff00"><closed>1</closed>'
                   f"{pointlist([(3000, 200), (3600, 200), (3600, 700), (3000, 700)])}</annotation>"),
        ndpa_state(4, "Nucleus", '<annotation type="circle" displayname="AnnotateCircle" color="#0000ff">'
                   "<x>{}</x><y>{}</y><radius>{}</radius></annotation>".format(*nm(500, 2500), 30000)),
        ndpa_state(5, "Mitosis", '<annotation type="pin" displayname="AnnotatePin" color="#ff00ff">'
                   "<x>{}</x><y>{}</y><icon>pinred</icon></annotation>".format(*nm(1234, 2345))),
        ndpa_state(6, "Width", '<annotation type="linearmeasure" displayname="AnnotateRuler" color="#ffffff">'
                   "<x1>{}</x1><y1>{}</y1><x2>{}</x2><y2>{}</y2></annotation>".format(
                       *nm(100, 100), *nm(700, 900))),
        ndpa_state(7, "Note", '<annotation type="pointer" displayname="AnnotatePointer" color="#00ffff">'
                   "<x>{}</x><y>{}</y></annotation>".format(*nm(3900, 2900))),
        # Not a shape: left out.
        ndpa_state(8, "Text", '<annotation type="text" color="#000000"><x>0</x><y>0</y></annotation>'),
    ]
    return ('<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n<annotations>\n'
            + "".join(states) + "</annotations>\n")


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    (OUT / "aperio.xml").write_bytes(b"\xef\xbb\xbf" + aperio().encode())
    (OUT / "aperio-utf16.xml").write_bytes(b"\xff\xfe" + aperio_utf16().encode("utf-16-le"))
    (OUT / "asap.xml").write_text(asap())
    (OUT / "slide.ndpa").write_text(ndpa())
    (OUT / "frame.txt").write_text(" ".join(str(v) for v in FRAME) + "\n")
    for p in sorted(OUT.iterdir()):
        print(f"{p.name:18} {p.stat().st_size:8} bytes")


if __name__ == "__main__":
    main()
