# GIMP 3 Python (run inside `gimp-console -i --batch-interpreter python-fu-eval`, see
# fetch-xcf.sh): writes synthetic XCF files covering layer modes, composite modes and spaces,
# blend spaces, groups, pass-through, masks, offsets, every precision, grey and indexed,
# RLE and zlib tiles. Output directory: $XCF_OUT.
#
# GIMP picks zlib only for images loaded from a zlib XCF, so every other file starts from a
# tiny zlib "seed" XCF written here.
import os
import struct
import zlib

import gi
import numpy as np

gi.require_version("Gimp", "3.0")
gi.require_version("Gegl", "0.4")
from gi.repository import Gegl, Gimp, Gio  # noqa: E402

OUT = os.environ["XCF_OUT"]
W, H = 150, 110
NI = Gimp.RunMode.NONINTERACTIVE
P = Gimp.Precision
RGB, GRAY = Gimp.ImageBaseType.RGB, Gimp.ImageBaseType.GRAY

os.makedirs(OUT, exist_ok=True)
SEED = os.path.join(OUT, ".seed-zlib.xcf")


def be32(v):
    return struct.pack(">I", v)


def write_seed():
    tile = zlib.compress(bytes([128, 128, 128, 255]))
    head = b"gimp xcf v008\0" + be32(1) + be32(1) + be32(0) + be32(150)
    props = be32(17) + be32(1) + b"\x02" + be32(0) + be32(0)
    base = len(head) + len(props) + 8 + 4
    layer = be32(1) + be32(1) + be32(1) + be32(2) + b"a\0" + be32(0) + be32(0)
    hier_at = base + len(layer) + 8
    level_at = hier_at + 20
    tile_at = level_at + 16
    layer += be32(hier_at) + be32(0)
    hier = be32(1) + be32(1) + be32(4) + be32(level_at) + be32(0)
    level = be32(1) + be32(1) + be32(tile_at) + be32(0)
    data = head + props + be32(base) + be32(0) + be32(0) + layer + hier + level + tile
    with open(SEED, "wb") as f:
        f.write(data)


counter = [0]


def new_image(prec, base=RGB, use_zlib=None):
    counter[0] += 1
    if use_zlib is None:
        use_zlib = counter[0] % 2 == 0
    if use_zlib:
        img = Gimp.file_load(NI, Gio.File.new_for_path(SEED))
        img.resize(W, H, 0, 0)
        for l in img.get_layers():
            img.remove_layer(l)
        if prec != P.U8_NON_LINEAR:
            img.convert_precision(prec)
        if base == GRAY:
            img.convert_grayscale()
    else:
        img = Gimp.Image.new_with_precision(W, H, base, prec)
    return img


def pattern(kind, w, h, over=False):
    """Straight-alpha R'G'B'A floats."""
    y, x = np.mgrid[0:h, 0:w].astype(np.float64)
    if kind == 0:  # backdrop: smooth colours, a hole and a soft bottom edge
        r, g = x / max(w - 1, 1), y / max(h - 1, 1)
        b = 0.5 + 0.5 * np.sin(x * 0.1) * np.cos(y * 0.13)
        a = np.where(y > h - 25, np.clip((h - y) / 25, 0, 1), 1.0)
        a[(x >= 30) & (x < 50) & (y >= 20) & (y < 40)] = 0.0
    elif kind == 1:  # waves with an alpha ramp, clear and solid areas
        r = 0.5 + 0.5 * np.sin(0.07 * x + 0.05 * y)
        g = 0.5 + 0.5 * np.cos(0.11 * y)
        b = ((x * y) % 37) / 36
        a = np.clip(x / (w * 0.7), 0, 1)
        a[((x // 23 + y // 17) % 5) == 0] = 1.0
    else:  # blocks: saturated, grey, black and white, varied alpha
        cols = np.array([(1, 0, 0), (0, 1, 0), (0, 0, 1), (1, 1, 0), (0.5, 0.5, 0.5),
                         (0, 0, 0), (1, 1, 1), (0.2, 0.6, 0.9), (0.9, 0.4, 0.1)])
        k = ((x // 16 + 3 * (y // 16)) % len(cols)).astype(int)
        r, g, b = cols[k, 0], cols[k, 1], cols[k, 2]
        a = np.array([1.0, 0.6, 0.3, 0.0, 0.85])[((x // 16 + y // 16) % 5).astype(int)]
    if over:
        r, g, b = r * 1.4 - 0.1, g * 1.3 - 0.15, b * 1.2
    return np.stack([r, g, b, a], axis=-1).astype("<f4").tobytes()


def mask_pattern(w, h):
    y, x = np.mgrid[0:h, 0:w].astype(np.float64)
    d = np.hypot(x - w * 0.5, y - h * 0.45) / (0.5 * min(w, h))
    return np.clip(1.2 - d, 0, 1).astype("<f4").tobytes()


def layer_type(img, alpha):
    if img.get_base_type() == GRAY:
        return Gimp.ImageType.GRAYA_IMAGE if alpha else Gimp.ImageType.GRAY_IMAGE
    return Gimp.ImageType.RGBA_IMAGE if alpha else Gimp.ImageType.RGB_IMAGE


def add(img, name, parent=None, kind=0, x=0, y=0, w=W, h=H, mode=28, opacity=100.0,
        alpha=True, mask=False, comp=None, cspace=None, bspace=None, visible=True, over=False):
    l = Gimp.Layer.new(img, name, w, h, layer_type(img, alpha), opacity, Gimp.LayerMode(mode))
    img.insert_layer(l, parent, 0)
    l.set_offsets(x, y)
    buf = l.get_buffer()
    buf.set(Gegl.Rectangle.new(0, 0, w, h), "R'G'B'A float", pattern(kind, w, h, over))
    buf.flush()
    setup(l, mode, comp, cspace, bspace, visible, mask, w, h)
    return l


def setup(l, mode, comp, cspace, bspace, visible, mask, w, h):
    l.set_mode(Gimp.LayerMode(mode))
    if comp is not None:
        l.set_composite_mode(Gimp.LayerCompositeMode(comp))
    if cspace is not None:
        l.set_composite_space(Gimp.LayerColorSpace(cspace))
    if bspace is not None:
        l.set_blend_space(Gimp.LayerColorSpace(bspace))
    l.set_visible(visible)
    if mask:
        m = l.create_mask(Gimp.AddMaskType.WHITE)
        l.add_mask(m)
        mb = m.get_buffer()
        mb.set(Gegl.Rectangle.new(0, 0, w, h), "Y float", mask_pattern(w, h))
        mb.flush()


def group(img, name, parent=None, mode=28, opacity=100.0, mask=False, visible=True):
    g = Gimp.GroupLayer.new(img, name)
    img.insert_layer(g, parent, 0)
    return g, (mode, opacity, mask, visible)


def finish_group(g, cfg):
    mode, opacity, mask, visible = cfg
    g.set_opacity(opacity)
    w, h = g.get_width(), g.get_height()
    setup(g, mode, None, None, None, visible, mask, w, h)


def save(img, name):
    Gimp.file_save(NI, img, Gio.File.new_for_path(os.path.join(OUT, name + ".xcf")), None)
    img.delete()


def three(img, mode, opacity=85.0, mask=False, comp=None, cspace=None, bspace=None, over=False):
    add(img, "backdrop", kind=0, over=over)
    add(img, "middle", kind=2, x=20, y=15, w=100, h=80, opacity=70.0)
    add(img, "top", kind=1, x=-20, y=30, w=140, h=100, mode=mode, opacity=opacity, mask=mask,
        comp=comp, cspace=cspace, bspace=bspace, over=over)


def modes():
    for tag, prec in (("u8nl", P.U8_NON_LINEAR), ("floatl", P.FLOAT_LINEAR)):
        for m in range(64):
            if m == 61:  # pass-through: groups only
                continue
            img = new_image(prec)
            three(img, m, mask=m % 2 == 1)
            save(img, "mode-%s-%02d" % (tag, m))


def composites():
    for m in (28, 30, 31, 57, 58, 59, 60, 62, 63, 29, 1, 24, 47, 22, 3):
        for c in (1, 2, 3, 4):
            img = new_image(P.U8_NON_LINEAR)
            three(img, m, comp=c)
            save(img, "comp-%02d-%d" % (m, c))
    for m in (28, 30, 31, 63, 1, 57):
        for s in (1, 2, 4):
            img = new_image(P.U16_LINEAR)
            three(img, m, cspace=s)
            save(img, "cspace-%02d-%d" % (m, s))
    for m in (30, 41, 45, 52, 23):
        for s in (1, 2, 3, 4):
            img = new_image(P.FLOAT_LINEAR)
            three(img, m, bspace=s)
            save(img, "bspace-%02d-%d" % (m, s))


def mixed(img, depth_extra=True):
    add(img, "backdrop", kind=0)
    g, cfg = group(img, "group", opacity=80.0)
    add(img, "g-bottom", g, kind=2, x=10, y=10, w=90, h=70)
    add(img, "g-multiply", g, kind=1, x=40, y=30, w=100, h=70, mode=30, mask=True)
    finish_group(g, cfg)
    add(img, "screen", kind=1, x=-10, y=-5, w=80, h=60, mode=31, opacity=60.0)
    if depth_extra:
        add(img, "hidden", kind=2, mode=28, visible=False)
        add(img, "overlay-masked", kind=2, x=60, y=40, w=90, h=70, mode=23, mask=True)


def structure():
    img = new_image(P.U8_NON_LINEAR)
    mixed(img)
    save(img, "group-basic")

    img = new_image(P.U8_NON_LINEAR)
    add(img, "backdrop", kind=0)
    g, cfg = group(img, "masked group", opacity=90.0, mask=True)
    add(img, "a", g, kind=2, x=30, y=20, w=100, h=80)
    add(img, "b", g, kind=1, x=50, y=40, w=90, h=60, mode=33)
    finish_group(g, cfg)
    save(img, "group-mask")

    img = new_image(P.U16_LINEAR)
    add(img, "backdrop", kind=0)
    g1, c1 = group(img, "outer", mode=31, opacity=90.0)
    add(img, "o1", g1, kind=2, x=5, y=5, w=120, h=90)
    g2, c2 = group(img, "inner", g1, mode=30, opacity=75.0)
    add(img, "i1", g2, kind=1, x=20, y=20, w=90, h=70)
    g3, c3 = group(img, "innermost", g2, mode=44)
    add(img, "ii1", g3, kind=2, x=40, y=10, w=80, h=80, mode=28, mask=True)
    add(img, "ii2", g3, kind=1, x=30, y=50, w=70, h=50, mode=47)
    finish_group(g3, c3)
    add(img, "invisible", g2, kind=2, visible=False)
    finish_group(g2, c2)
    g4, c4 = group(img, "hidden group", g1, visible=False)
    add(img, "h1", g4, kind=2)
    finish_group(g4, c4)
    finish_group(g1, c1)
    save(img, "group-nested")

    for tag, op, mask in (("plain", 100.0, False), ("opacity", 60.0, False), ("mask", 100.0, True)):
        img = new_image(P.U8_NON_LINEAR)
        add(img, "backdrop", kind=0)
        g, cfg = group(img, "pass", mode=61, opacity=op, mask=mask)
        add(img, "p-multiply", g, kind=1, x=10, y=10, w=110, h=80, mode=30)
        add(img, "p-screen", g, kind=2, x=40, y=30, w=100, h=70, mode=31, opacity=70.0)
        add(img, "p-erase", g, kind=1, x=70, y=0, w=60, h=60, mode=58, opacity=40.0)
        finish_group(g, cfg)
        save(img, "pass-%s" % tag)

    img = new_image(P.FLOAT_LINEAR)
    g, cfg = group(img, "pass at bottom", mode=61, opacity=80.0)
    add(img, "pb1", g, kind=2, x=10, y=10, w=110, h=80)
    add(img, "pb2", g, kind=1, x=40, y=30, w=100, h=70, mode=30)
    finish_group(g, cfg)
    save(img, "pass-bottom")

    img = new_image(P.U8_NON_LINEAR)
    add(img, "backdrop", kind=0)
    g, cfg = group(img, "normal", opacity=85.0)
    add(img, "n1", g, kind=2, x=0, y=0, w=120, h=90)
    p, pcfg = group(img, "pass inside", g, mode=61)
    add(img, "pi1", p, kind=1, x=20, y=20, w=100, h=80, mode=45)
    p2, p2cfg = group(img, "pass in pass", p, mode=61, opacity=70.0)
    add(img, "pp1", p2, kind=2, x=50, y=40, w=80, h=60, mode=32)
    finish_group(p2, p2cfg)
    finish_group(p, pcfg)
    finish_group(g, cfg)
    save(img, "pass-nested")

    for tag, m in (("multiply", 30), ("dissolve", 1), ("anti-erase", 63), ("legacy", 3),
                   ("behind", 29), ("lch", 26)):
        img = new_image(P.U8_NON_LINEAR)
        add(img, "bottom", kind=1, x=10, y=5, w=120, h=90, mode=m, opacity=80.0, mask=True)
        add(img, "top", kind=2, x=30, y=30, w=100, h=70, mode=30)
        save(img, "bottom-%s" % tag)

    img = new_image(P.U8_NON_LINEAR)
    add(img, "backdrop", kind=0)
    g, cfg = group(img, "first is multiply", opacity=90.0)
    add(img, "gm", g, kind=1, x=10, y=10, w=100, h=80, mode=30)
    add(img, "gs", g, kind=2, x=40, y=30, w=100, h=70, mode=31)
    finish_group(g, cfg)
    save(img, "group-bottom-multiply")

    img = new_image(P.U8_NON_LINEAR)
    add(img, "backdrop", kind=0)
    g, cfg = group(img, "dissolving group", mode=1, opacity=70.0)
    add(img, "d1", g, kind=2, x=37, y=21, w=90, h=70)
    d2 = add(img, "d2", g, kind=1, x=13, y=45, w=80, h=50, mode=1, opacity=60.0)
    finish_group(g, cfg)
    save(img, "dissolve-group")

    img = new_image(P.U8_NON_LINEAR)
    add(img, "backdrop", kind=0)
    g, cfg = group(img, "empty")
    finish_group(g, cfg)
    add(img, "off canvas", kind=2, x=400, y=300, w=50, h=50)
    add(img, "zero opacity", kind=1, opacity=0.0)
    add(img, "legacy group child", kind=2, x=20, y=20, w=60, h=60, mode=13)
    save(img, "edge-cases")

    img = new_image(P.U8_NON_LINEAR)
    add(img, "opaque", kind=0, alpha=False)
    add(img, "multiply", kind=1, mode=30, opacity=50.0)
    save(img, "opaque")


def precisions():
    for tag, prec in (("u8nl", P.U8_NON_LINEAR), ("u8l", P.U8_LINEAR), ("u8p", P.U8_PERCEPTUAL),
                      ("u16nl", P.U16_NON_LINEAR), ("u16l", P.U16_LINEAR),
                      ("u16p", P.U16_PERCEPTUAL), ("u32l", P.U32_LINEAR),
                      ("u32nl", P.U32_NON_LINEAR), ("halfl", P.HALF_LINEAR),
                      ("halfnl", P.HALF_NON_LINEAR), ("floatl", P.FLOAT_LINEAR),
                      ("floatnl", P.FLOAT_NON_LINEAR), ("floatp", P.FLOAT_PERCEPTUAL),
                      ("doublel", P.DOUBLE_LINEAR), ("doublenl", P.DOUBLE_NON_LINEAR)):
        for z in (False, True):
            img = new_image(prec, use_zlib=z)
            mixed(img)
            save(img, "prec-%s-%s" % (tag, "zlib" if z else "rle"))

    img = new_image(P.FLOAT_LINEAR)
    three(img, 33, over=True)
    add(img, "multiply over", kind=2, mode=30, opacity=80.0, over=True)
    save(img, "float-overshoot")


def grey_indexed():
    for tag, prec in (("u8nl", P.U8_NON_LINEAR), ("u16l", P.U16_LINEAR), ("floatl", P.FLOAT_LINEAR)):
        img = new_image(prec, GRAY)
        mixed(img)
        save(img, "grey-%s" % tag)
    img = new_image(P.U8_NON_LINEAR, GRAY)
    add(img, "opaque", kind=0, alpha=False)
    save(img, "grey-opaque")

    for tag, alpha in (("alpha", True), ("opaque", False)):
        img = new_image(P.U8_NON_LINEAR)
        add(img, "backdrop", kind=0, alpha=alpha)
        add(img, "blocks", kind=2, x=20, y=15, w=100, h=80, mode=30 if alpha else 28,
            alpha=alpha, mask=alpha)
        img.convert_indexed(Gimp.ConvertDitherType.NONE, Gimp.ConvertPaletteType.GENERATE,
                            200, False, False, "")
        save(img, "indexed-%s" % tag)


def channels():
    # Visible channels: GIMP draws them over the layers in their colour.
    for tag, prec, shown in (("u8nl", P.U8_NON_LINEAR, False), ("u8nl-masked", P.U8_NON_LINEAR, True),
                             ("floatl", P.FLOAT_LINEAR, False), ("u16nl-masked", P.U16_NON_LINEAR, True)):
        img = new_image(prec)
        mixed(img)
        for name, col, op, visible in (("orange", "#ff8020", 55.0, True),
                                       ("blue", "#2040e0", 35.0, True),
                                       ("hidden", "#00ff00", 100.0, False)):
            c = Gimp.Channel.new(img, name, W, H, op, Gegl.Color.new(col))
            img.insert_channel(c, None, 0)
            c.get_buffer().set(Gegl.Rectangle.new(0, 0, W, H), "Y float",
                               mask_pattern(W, H) if name != "blue" else pattern_y(W, H))
            c.get_buffer().flush()
            c.set_show_masked(shown)
            c.set_visible(visible)
        save(img, "chan-%s" % tag)


def pattern_y(w, h):
    y, x = np.mgrid[0:h, 0:w].astype(np.float64)
    return (x / max(w - 1, 1)).astype("<f4").tobytes()


def bench():
    global W, H
    W, H = 2400, 1600
    img = new_image(P.U8_NON_LINEAR, use_zlib=True)
    add(img, "backdrop", kind=0)
    g, cfg = group(img, "group", opacity=85.0)
    add(img, "g1", g, kind=2, x=100, y=80, w=1800, h=1200)
    add(img, "g2", g, kind=1, x=600, y=500, w=1700, h=1000, mode=30, mask=True)
    finish_group(g, cfg)
    add(img, "screen", kind=1, x=-200, y=-100, w=1500, h=1100, mode=31, opacity=60.0)
    add(img, "overlay", kind=2, x=900, y=300, w=1400, h=1200, mode=23)
    add(img, "softlight", kind=1, x=0, y=700, w=2400, h=900, mode=45, opacity=50.0)
    save(img, "bench-2400x1600")
    W, H = 150, 110


write_seed()
which = os.environ.get("XCF_SECTIONS", "modes composites structure precisions grey_indexed channels bench")
for s in which.split():
    globals()[s]()
    print("xcf-generate: %s done" % s)
os.remove(SEED)
