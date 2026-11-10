#!/usr/bin/env python3
"""The oracle for specular-pcb: gerbonara's reading of every Gerber and Excellon file under a
folder, written for tests/oracle.rs to compare against.

Usage (from the worktree root, in the oracle venv with gerbonara, shapely and numpy):
    python -I crates/formats/specular-pcb/tests/oracle.py CORPUS_DIR EXPECT_DIR

For each file it writes one JSON line to EXPECT_DIR/expect.jsonl:
  Gerber:   {"path", "kind": "gerber", "bbox": [x0, y0, x1, y1] (mm, gerbonara's), "grid":
             [x0, y_top, px, w, h], "mask": "<name>.pbm", "mask_any": "any-<name>.pbm"
             (every pixel the image touches), "dark": pixels set}
            The mask is the image (dark minus clear, in order) sampled at pixel centres, with
            every primitive made a shapely polygon (circles, lines and arcs buffered with round
            caps; regions and macro outlines as polygons; arcs flattened to 1 µm).
  Excellon: {"path", "kind": "drill", "holes": n, "slots": n, "tools": [diameters mm, sorted]}
  Failures: {"path", "kind": "error", "error": "..."}
gerbonara ignores aperture transformations (LM, LR, LS) and negative images (IPNEG), does not
know primitive 22, and misplaces step-and-repeat and block aperture copies: such files, and files gerbonara cannot read, are rendered by pygerber
instead (its raster renderer at a whole number of dots per mm over its own bounding box; the
record says "oracle": "pygerber").
Draws under 10 µm wide are left out of the mask (Specular draws them as hairlines), and so
are macro circles and lines under 10 µm across (below any drawable size). Draws with
rectangle, obround and polygon apertures are swept exactly (the hull of the aperture at both
ends), where gerbonara would use a round line of the aperture's equivalent width.
"""

import hashlib
import json
import math
import os
import signal
import sys

import numpy as np
import shapely
from shapely.geometry import LineString, Point, Polygon, box
from shapely import affinity

from gerbonara import ExcellonFile, GerberFile
from gerbonara import graphic_primitives as gp
from gerbonara.utils import MM

LONG = 1024
PAD = 4
QUAD = 64
# Draws narrower than this (mm) are hairlines in Specular (gerber::HAIRLINE_MM): left out.
HAIRLINE = 0.01


def arc_points(x1, y1, x2, y2, cx, cy, clockwise, err=1e-3):
    r = math.hypot(x1 - cx, y1 - cy)
    a0 = math.atan2(y1 - cy, x1 - cx)
    a1 = math.atan2(y2 - cy, x2 - cx)
    if clockwise:
        d = (a0 - a1) % (2 * math.pi)
        if d < 1e-9:
            d = 2 * math.pi
        d = -d
    else:
        d = (a1 - a0) % (2 * math.pi)
        if d < 1e-9:
            d = 2 * math.pi
    if r < 1e-12:
        return [(x1, y1)]
    step = 2 * math.acos(max(-1.0, min(1.0, 1 - err / r))) if r > err else math.pi / 8
    n = max(8, int(abs(d) / max(step, 1e-4)) + 1)
    return [(cx + r * math.cos(a0 + d * i / n), cy + r * math.sin(a0 + d * i / n)) for i in range(n + 1)]


def to_shape(p):
    if isinstance(p, gp.Circle):
        # Under HAIRLINE across: below any drawable size (Allegro builds outlines in macros
        # from 0.1 mil circles and vector lines); a pixel centre lands on them by chance.
        return Point(p.x, p.y).buffer(p.r, quad_segs=QUAD) if 2 * p.r >= HAIRLINE else None
    if isinstance(p, gp.Line):
        if p.width < HAIRLINE:
            return None
        if math.isclose(p.x1, p.x2, abs_tol=1e-12) and math.isclose(p.y1, p.y2, abs_tol=1e-12):
            return Point(p.x1, p.y1).buffer(p.width / 2, quad_segs=QUAD)
        return LineString([(p.x1, p.y1), (p.x2, p.y2)]).buffer(p.width / 2, quad_segs=QUAD)
    if isinstance(p, gp.Arc):
        if p.width < HAIRLINE:
            return None
        pts = arc_points(p.x1, p.y1, p.x2, p.y2, p.cx, p.cy, p.clockwise)
        if len(pts) < 2:
            return Point(p.x1, p.y1).buffer(p.width / 2, quad_segs=QUAD)
        return LineString(pts).buffer(p.width / 2, quad_segs=QUAD)
    if isinstance(p, gp.Rectangle):
        if min(p.w, p.h) < HAIRLINE:
            return None
        b = box(p.x - p.w / 2, p.y - p.h / 2, p.x + p.w / 2, p.y + p.h / 2)
        return affinity.rotate(b, p.rotation, origin=(p.x, p.y), use_radians=True)
    if isinstance(p, gp.ArcPoly):
        if not p.outline:
            return None
        pts = []
        for (x1, y1), (x2, y2), (cw, (cx, cy)) in p.segments:
            if cw is None:
                pts.append((x1, y1))
            else:
                pts.extend(arc_points(x1, y1, x2, y2, cx, cy, cw)[:-1])
        if len(pts) < 3:
            return None
        poly = Polygon(pts)
        if not poly.is_valid:
            poly = shapely.make_valid(poly)
        return poly
    raise TypeError(type(p).__name__)


def swept_line(o):
    """A straight draw with a non-circular aperture as the hull of the aperture at both ends
    (gerbonara itself makes it a round line of the aperture's equivalent width)."""
    from gerbonara.apertures import CircleAperture
    from gerbonara.graphic_objects import Line

    if not isinstance(o, Line) or o.aperture is None or isinstance(o.aperture, CircleAperture):
        return None
    c = o.converted(MM)
    shapes = []
    for x, y in (c.p1, c.p2):
        for p in o.aperture.flash(x, y, unit=MM):
            s = to_shape(p)
            if s is not None and p.polarity_dark:
                shapes.append(s)
    if not shapes:
        return None
    return shapely.union_all(shapes).convex_hull


def hairline_width(o):
    """The width (mm) of a draw with a round aperture under HAIRLINE, else None."""
    from gerbonara.apertures import CircleAperture
    from gerbonara.graphic_objects import Arc, Line

    if not isinstance(o, (Line, Arc)) or not isinstance(o.aperture, CircleAperture):
        return None
    u = o.aperture.unit or o.unit
    d = u.convert_to(MM, o.aperture.diameter) if u is not None else o.aperture.diameter
    return d if 0 < d < HAIRLINE else None


def bounds(g):
    """gerbonara's bounding box, with draws by non-circular apertures bounded by their exact
    sweep (gerbonara bounds them as round lines of the aperture's equivalent width)."""
    box_ = None
    for o in g.objects:
        swept = swept_line(o)
        if swept is not None:
            bx0, by0, bx1, by1 = swept.bounds
        else:
            (bx0, by0), (bx1, by1) = o.bounding_box(unit=MM)
            # A hairline (see HAIRLINE) is bounded by its centre line.
            w = hairline_width(o)
            if w:
                bx0, by0, bx1, by1 = bx0 + w / 2, by0 + w / 2, bx1 - w / 2, by1 - w / 2
        box_ = (bx0, by0, bx1, by1) if box_ is None else (
            min(box_[0], bx0), min(box_[1], by0), max(box_[2], bx1), max(box_[3], by1))
    if box_ is None:
        return g.bounding_box(unit=MM)
    return (box_[0], box_[1]), (box_[2], box_[3])


def image(objs):
    geom = shapely.Polygon()
    batch, dark = [], True

    def flush():
        nonlocal geom, batch
        if not batch:
            return
        u = shapely.union_all(batch)
        geom = shapely.union(geom, u) if dark else shapely.difference(geom, u)
        batch = []

    for o in objs:
        swept = swept_line(o)
        if swept is not None:
            if o.polarity_dark != dark:
                flush()
                dark = o.polarity_dark
            batch.append(swept)
            continue
        # One object's primitives (a macro flash) make one shape first: an exposure-off
        # primitive clears only what the macro drew before it, never the image underneath
        # (Gerber spec 4.5.1, "Exposure"; gerbonara gives such primitives the opposite
        # polarity of the object, which would clear the image).
        shape, prims = None, list(o.to_primitives(unit=MM))
        if len(prims) == 1:
            if prims[0].polarity_dark == o.polarity_dark:
                shape = to_shape(prims[0])
        else:
            for p in prims:
                s = to_shape(p)
                if s is None or s.is_empty:
                    continue
                if p.polarity_dark == o.polarity_dark:
                    shape = s if shape is None else shapely.union(shape, s)
                elif shape is not None:
                    shape = shapely.difference(shape, s)
        if shape is None or shape.is_empty:
            continue
        if o.polarity_dark != dark:
            flush()
            dark = o.polarity_dark
        batch.append(shape)
    flush()
    return geom


def write_pbm(path, mask):
    h, w = mask.shape
    with open(path, "wb") as f:
        f.write(f"P4\n{w} {h}\n".encode())
        f.write(np.packbits(mask, axis=1).tobytes())


class Timeout(Exception):
    pass


def alarm(_sig, _frm):
    raise Timeout()


def sniff(data):
    head = data[:16384]
    if b"\0" in head:
        return None
    t = head.decode("latin-1")
    if "*" in t and ("%FS" in t or "%MO" in t or "%ADD" in t or "%TF." in t or "D01*" in t or "D03*" in t):
        return "gerber"
    lines = [l.strip() for l in t.splitlines()]
    if "M48" in lines or (any(l[:1] == "T" and l[1:2].isdigit() for l in lines) and "*" not in t):
        return "drill"
    return None


def pygerber_record(full):
    import io

    from PIL import Image
    from pygerber.gerberx3.api.v2 import ColorScheme, GerberFile as PGFile
    from pygerber.common.rgba import RGBA

    scheme = ColorScheme(
        background_color=RGBA.from_hex("#000000"),
        clear_color=RGBA.from_hex("#000000"),
        solid_color=RGBA.from_hex("#FFFFFF"),
        clear_region_color=RGBA.from_hex("#000000"),
        solid_region_color=RGBA.from_hex("#FFFFFF"),
    )
    pf = PGFile.from_file(full).parse()
    info = pf.get_info()
    x0, y0, x1, y1 = (float(info.min_x_mm), float(info.min_y_mm), float(info.max_x_mm), float(info.max_y_mm))
    dpmm = max(1, int(LONG / max(x1 - x0, y1 - y0, 1e-3)))
    buf = io.BytesIO()
    pf.render_raster(buf, color_scheme=scheme, dpmm=dpmm, image_format=_png())
    img = np.asarray(Image.open(io.BytesIO(buf.getvalue())).convert("L"))
    mask = img > 127
    gh, gw = mask.shape
    return dict(bbox=[x0, y0, x1, y1], grid=[x0, y1, 1.0 / dpmm, gw, gh], oracle="pygerber"), mask


def _png():
    from pygerber.gerberx3.api.v2 import ImageFormatEnum

    return ImageFormatEnum.PNG


def needs_pygerber(data):
    t = data.decode("latin-1")
    if any(c in t for c in ("%LM", "%LR", "%LS", "%SR", "%AB", "%IPNEG")):
        return True
    if "%AM" in t:
        for block in t.split("%AM")[1:]:
            body = block.split("%")[0]
            if any(st.strip().startswith("22,") for st in body.split("*")):
                return True
    return False


def one(args):
    root, out, full = args
    import warnings

    warnings.simplefilter("ignore")
    signal.signal(signal.SIGALRM, alarm)
    rel = os.path.relpath(full, root)
    with open(full, "rb") as f:
        data = f.read()
    kind = sniff(data)
    if kind is None:
        return None
    rec = {"path": rel, "kind": kind}
    signal.alarm(120)
    try:
        if kind == "gerber" and needs_pygerber(data):
            extra, mask = pygerber_record(full)
            x0, y0, x1, y1 = extra["bbox"]
            if not (x1 > x0 or y1 > y0) or not mask.any():
                raise ValueError("pygerber drew nothing")
            # Pixels next to a set one: pygerber's raster has no finer reading.
            touched = mask.copy()
            touched[1:] |= mask[:-1]
            touched[:-1] |= mask[1:]
            touched[:, 1:] |= touched[:, :-1].copy()
            touched[:, :-1] |= touched[:, 1:].copy()
            mname = hashlib.sha1(rel.encode()).hexdigest()[:16] + ".pbm"
            write_pbm(os.path.join(out, "masks", mname), mask.astype(np.uint8))
            write_pbm(os.path.join(out, "masks", "any-" + mname), touched.astype(np.uint8))
            rec.update(extra, mask=mname, mask_any="any-" + mname, dark=int(mask.sum()))
        elif kind == "gerber":
            g = GerberFile.open(full)
            (x0, y0), (x1, y1) = bounds(g)
            geom = image(g.objects)
            w, h = x1 - x0, y1 - y0
            px = max(max(w, h) / LONG, 0.002)
            gw = int(math.ceil(w / px)) + 2 * PAD
            gh = int(math.ceil(h / px)) + 2 * PAD
            ox, oy = x0 - PAD * px, y1 + PAD * px
            xs = ox + (np.arange(gw) + 0.5) * px
            ys = oy - (np.arange(gh) + 0.5) * px
            X, Y = np.meshgrid(xs, ys)
            mask = shapely.contains_xy(geom, X, Y) if not geom.is_empty else np.zeros((gh, gw), bool)
            # Pixels the image touches at all (within half a pixel diagonal of a centre).
            near = geom.buffer(px * 0.7072, quad_segs=4) if not geom.is_empty else geom
            touched = shapely.contains_xy(near, X, Y) if not near.is_empty else mask
            mname = hashlib.sha1(rel.encode()).hexdigest()[:16] + ".pbm"
            write_pbm(os.path.join(out, "masks", mname), mask.astype(np.uint8))
            write_pbm(os.path.join(out, "masks", "any-" + mname), (touched | mask).astype(np.uint8))
            rec.update(bbox=[x0, y0, x1, y1], grid=[ox, oy, px, gw, gh], mask=mname,
                       mask_any="any-" + mname, dark=int(mask.sum()))
        else:
            e = ExcellonFile.open(full)
            holes = slots = 0
            tools = set()
            for o in e.objects:
                d = o.tool.unit.convert_to(MM, o.tool.diameter)
                tools.add(round(d, 4))
                if type(o).__name__ == "Flash":
                    holes += 1
                else:
                    slots += 1
            rec.update(holes=holes, slots=slots, tools=sorted(tools))
    except Timeout:
        rec = {"path": rel, "kind": "error", "error": "oracle timeout"}
    except Exception as ex:  # noqa: BLE001 - every failure is recorded
        rec = {"path": rel, "kind": "error", "error": f"{type(ex).__name__}: {ex}"[:300]}
    finally:
        signal.alarm(0)
    return json.dumps(rec)


def main():
    import multiprocessing

    root, out = sys.argv[1], sys.argv[2]
    jobs = int(os.environ.get("JOBS", "2"))
    os.makedirs(os.path.join(out, "masks"), exist_ok=True)
    todo = []
    for dirpath, dirs, files in os.walk(root):
        dirs.sort()
        for name in sorted(files):
            full = os.path.join(dirpath, name)
            if os.path.getsize(full) <= 64 << 20:
                todo.append((root, out, full))
    with multiprocessing.Pool(jobs) as pool:
        lines = [l for l in pool.imap(one, todo, chunksize=1) if l]
    with open(os.path.join(out, "expect.jsonl"), "w") as f:
        f.write("\n".join(lines) + "\n")
    print(len(lines), "records", file=sys.stderr)


if __name__ == "__main__":
    main()
