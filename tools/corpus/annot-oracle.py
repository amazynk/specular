"""annot-oracle.py <out.tsv> [--frame nmx nmy offx offy w h] <file>...: slide annotations as an
independent reader sees them

The oracle of specular-wsi's annotation tests (tests/annot.rs reads tests/data/annot/expected.tsv;
the corpus test the fetched CAMELYON / ImageScope files' expected.tsv, written by
fetch-vendormicro.sh). Parses NDP.view (.ndpa), Aperio ImageScope and ASAP XML with
xml.etree and measures every shape with shapely (the oracle venv; run with python -I).

Lines (tab-separated), one per shape in file order:
  file index shape name group colour n x0 y0 x1 y1 area length points
shape: polygon polyline rectangle ellipse point points ruler arrow; colour: rrggbb or -;
x0..y1: the bounding box of the points; area: shapely's polygon area (an ellipse's from a
4096-segment polygon), 0 for open shapes; length: the line's length along its points, 0 for
closed shapes; points: x,y;x,y... (%.6f). A line `skipped file count` follows each file.
"""
import math
import sys
import xml.etree.ElementTree as ET

from shapely import affinity
from shapely.geometry import LineString, Point, Polygon


def text(path):
    data = open(path, "rb").read()
    if data[:2] in (b"\xff\xfe", b"\xfe\xff"):
        s = data.decode("utf-16")
    else:
        s = data.decode("utf-8-sig")
    # ElementTree refuses a str with an encoding declaration of UTF-16.
    return s.replace('encoding="UTF-16"', 'encoding="UTF-8"')


def num(s):
    if s is None:
        return None
    s = s.strip()
    try:
        v = float(s)
    except ValueError:
        try:
            v = float(s.replace(",", "."))
        except ValueError:
            return None
    return v if math.isfinite(v) else None


def hexcol(s):
    if not s:
        return None
    h = s.strip().lstrip("#")
    if len(h) != 6:
        return None
    try:
        int(h, 16)
    except ValueError:
        return None
    return h.lower()


def box(pts):
    xs = [p[0] for p in pts]
    ys = [p[1] for p in pts]
    x0, x1, y0, y1 = min(xs), max(xs), min(ys), max(ys)
    return [(x0, y0), (x1, y0), (x1, y1), (x0, y1)]


MIN = {"polygon": 3, "polyline": 2, "ruler": 2, "arrow": 2, "ellipse": 2, "rectangle": 2,
       "point": 1, "points": 1}


def ndpa(root, frame):
    sx, sy, ox, oy, w, h = frame

    def px(x, y):
        return ((x - ox) / sx + w / 2, (y - oy) / sy + h / 2)

    def child(e, n):
        c = e.find(n)
        return num(c.text) if c is not None else None

    out, skipped = [], 0
    for vs in root.iter("ndpviewstate"):
        t = vs.find("title")
        name = (t.text or "") if t is not None else ""
        a = vs.find("annotation")
        if a is None:
            skipped += 1
            continue
        col = hexcol(a.get("color"))
        typ = a.get("type", "")
        shape = None
        if typ == "freehand":
            pts = []
            for p in a.iter("point"):
                x, y = child(p, "x"), child(p, "y")
                if x is not None and y is not None:
                    pts.append(px(x, y))
            c = a.find("closed")
            closed = c is None or (c.text or "").strip() != "0"
            rect = a.get("specialtype") == "rectangle" or a.get("displayname") == "AnnotateRectangle"
            if rect and len(pts) >= 2:
                shape, pts = "rectangle", box(pts)
            else:
                shape = "polygon" if closed else "polyline"
        elif typ == "circle":
            x, y, r = child(a, "x"), child(a, "y"), child(a, "radius")
            if None not in (x, y, r):
                shape, pts = "ellipse", [px(x - r, y - r), px(x + r, y + r)]
        elif typ in ("pin", "pointer"):
            x, y = child(a, "x"), child(a, "y")
            if None not in (x, y):
                shape, pts = "point", [px(x, y)]
        elif typ == "linearmeasure":
            v = [child(a, k) for k in ("x1", "y1", "x2", "y2")]
            if None not in v:
                shape, pts = "ruler", [px(v[0], v[1]), px(v[2], v[3])]
        if shape is None:
            skipped += 1
            continue
        out.append((shape, name, "", col, pts))
    return out, skipped


def aperio(root):
    out = []
    for li, layer in enumerate(root.iter("Annotation")):
        lc = layer.get("LineColor")
        col = None
        if lc is not None and lc.strip().isdigit():
            v = int(lc)
            col = "%02x%02x%02x" % (v & 255, (v >> 8) & 255, (v >> 16) & 255)
        group = (layer.get("Name") or "").strip() and layer.get("Name")
        if not group:
            at = layer.find("Attributes/Attribute")
            group = at.get("Name") if at is not None and (at.get("Name") or "").strip() else None
        if not group:
            group = layer.get("Id") or str(li + 1)
        for r in layer.iter("Region"):
            pts = []
            for v in r.iter("Vertex"):
                x, y = num(v.get("X")), num(v.get("Y"))
                if x is not None and y is not None:
                    pts.append((x, y))
            typ = int(num(r.get("Type")) or 0)
            if typ == 1 and len(pts) >= 2:
                shape, pts = "rectangle", box(pts)
            elif typ == 2 and len(pts) >= 2:
                b = box(pts)
                shape, pts = "ellipse", [b[0], b[2]]
            elif typ == 3:
                shape = "arrow"
            elif typ == 4:
                shape = "ruler"
            elif len(pts) == 1:
                shape = "point"
            else:
                shape = "polygon"
            out.append((shape, (r.get("Text") or "").strip(), group, col, pts))
    return out, 0


def asap(root):
    groups = {g.get("Name", ""): hexcol(g.get("Color")) for g in root.iter("Group")}
    kinds = {"Rectangle": "rectangle", "Dot": "point", "PointSet": "points", "Measurement": "ruler"}
    out = []
    for a in root.find("Annotations").iter("Annotation"):
        group = (a.get("PartOfGroup") or "").strip()
        group = "" if group == "None" else group
        col = hexcol(a.get("Color")) or groups.get(group)
        coords = []
        for i, c in enumerate(a.iter("Coordinate")):
            o, x, y = num(c.get("Order")), num(c.get("X")), num(c.get("Y"))
            if x is not None and y is not None:
                coords.append((i if o is None else o, x, y))
        coords.sort(key=lambda c: c[0])
        pts = [(c[1], c[2]) for c in coords]
        shape = kinds.get(a.get("Type", ""), "polygon")
        if shape == "rectangle" and len(pts) == 2:
            pts = box(pts)
        out.append((shape, (a.get("Name") or "").strip(), group, col, pts))
    return out, 0


def measure(shape, pts):
    if shape == "ellipse":
        b = box(pts)
        cx, cy = (b[0][0] + b[2][0]) / 2, (b[0][1] + b[2][1]) / 2
        rx, ry = (b[2][0] - b[0][0]) / 2, (b[2][1] - b[0][1]) / 2
        e = affinity.scale(Point(0, 0).buffer(1.0, quad_segs=1024), rx, ry)
        return affinity.translate(e, cx, cy).area, 0.0
    if shape in ("polygon", "rectangle"):
        return Polygon(pts).area, 0.0
    if shape in ("polyline", "ruler", "arrow"):
        return 0.0, LineString(pts).length
    return 0.0, 0.0


def main():
    args = sys.argv[1:]
    out_path = args.pop(0)
    frame = None
    if args and args[0] == "--frame":
        frame = tuple(float(v) for v in args[1:7])
        args = args[7:]
    lines = []
    for path in args:
        root = ET.fromstring(text(path))
        if root.tag == "annotations":
            items, skipped = ndpa(root, frame)
        elif root.tag == "ASAP_Annotations":
            items, skipped = asap(root)
        elif root.tag == "Annotations":
            items, skipped = aperio(root)
        else:
            continue
        name = path.rsplit("/", 1)[-1]
        k = 0
        for shape, label, group, col, pts in items:
            if len(pts) < MIN[shape]:
                skipped += 1
                continue
            area, length = measure(shape, pts)
            b = box(pts)
            p = ";".join("%.6f,%.6f" % q for q in pts)
            lines.append("\t".join([
                name, str(k), shape, label, group, col or "-", str(len(pts)),
                "%.6f" % b[0][0], "%.6f" % b[0][1], "%.6f" % b[2][0], "%.6f" % b[2][1],
                "%.6f" % area, "%.6f" % length, p,
            ]))
            k += 1
        lines.append(f"skipped\t{name}\t{skipped}")
    with open(out_path, "w", encoding="utf-8") as f:
        f.write("\n".join(lines) + "\n")


if __name__ == "__main__":
    main()
