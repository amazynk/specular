# GIMP 3 Python (run inside `gimp-console -i --batch-interpreter python-fu-eval`, see
# fetch-xcf.sh, section `filters`): writes XCF files with one layer carrying GIMP 3 layer
# effects (non-destructive GEGL filters), one file per filter and setting, into $XCF_OUT.
# The `oracle` section then writes GIMP's composite of each (filters applied) as `.ref`.
import os

import gi
import numpy as np

gi.require_version("Gimp", "3.0")
gi.require_version("Gegl", "0.4")
from gi.repository import Gegl, Gimp, Gio  # noqa: E402

OUT = os.environ["XCF_OUT"]
W, H = 96, 64
NI = Gimp.RunMode.NONINTERACTIVE
os.makedirs(OUT, exist_ok=True)


def pattern(w, h):
    y, x = np.mgrid[0:h, 0:w].astype(np.float64)
    r = x / (w - 1)
    g = y / (h - 1)
    b = 0.5 + 0.5 * np.sin(x * 0.2) * np.cos(y * 0.15)
    a = np.ones_like(r)
    a[(x >= 60) & (x < 80) & (y >= 20) & (y < 40)] = 0.0
    a[y >= h - 8] = 0.5
    return np.stack([r, g, b, a], axis=-1).astype("<f4").tobytes()


def make(name, filters, prec=Gimp.Precision.U8_NON_LINEAR, opacity=100.0, mode=None):
    img = Gimp.Image.new_with_precision(W, H, Gimp.ImageBaseType.RGB, prec)
    back = Gimp.Layer.new(img, "back", W, H, Gimp.ImageType.RGBA_IMAGE, 100.0,
                          Gimp.LayerMode.NORMAL)
    img.insert_layer(back, None, 0)
    buf = back.get_buffer()
    y, x = np.mgrid[0:H, 0:W]
    chk = (((x // 8 + y // 8) % 2) * 0.5 + 0.25).astype(np.float64)
    buf.set(Gegl.Rectangle.new(0, 0, W, H), "R'G'B'A float",
            np.stack([chk, chk, chk, np.ones_like(chk)], -1).astype("<f4").tobytes())
    buf.flush()
    l = Gimp.Layer.new(img, "top", W, H, Gimp.ImageType.RGBA_IMAGE, opacity,
                       Gimp.LayerMode.NORMAL if mode is None else mode)
    img.insert_layer(l, None, 0)
    buf = l.get_buffer()
    buf.set(Gegl.Rectangle.new(0, 0, W, H), "R'G'B'A float", pattern(W, H))
    buf.flush()
    for op, props, extra in filters:
        f = Gimp.DrawableFilter.new(l, op, op)
        cfg = f.get_config()
        for k, v in props.items():
            cfg.set_property(k, v)
        if "opacity" in extra:
            f.set_opacity(extra["opacity"])
        if "visible" in extra:
            f.set_visible(extra["visible"])
        if "mode" in extra:
            f.set_blend_mode(extra["mode"])
        f.update()
        l.append_filter(f)
    Gimp.file_save(NI, img, Gio.File.new_for_path(os.path.join(OUT, name + ".xcf")), None)
    img.delete()
    print("xcf-filters:", name)


CASES = [
    ("invert", [("gegl:invert-gamma", {}, {})]),
    ("invert-linear", [("gegl:invert-linear", {}, {})]),
    ("blur", [("gegl:gaussian-blur", {"std-dev-x": 3.0, "std-dev-y": 2.0}, {})]),
    ("desaturate", [("gimp:desaturate", {}, {})]),
    ("threshold", [("gimp:threshold", {"low": 0.4, "high": 1.0}, {})]),
    ("brightness-contrast", [("gimp:brightness-contrast", {"brightness": 0.2, "contrast": 0.3}, {})]),
    ("levels", [("gimp:levels", {}, {})]),
    ("hue-saturation", [("gimp:hue-saturation", {}, {})]),
    ("posterize", [("gimp:posterize", {"levels": 4}, {})]),
    ("color-to-alpha", [("gegl:color-to-alpha", {}, {})]),
    ("pixelize", [("gegl:pixelize", {"size-x": 8, "size-y": 8}, {})]),
    ("dropshadow", [("gegl:dropshadow", {"x": 4.0, "y": 4.0, "radius": 3.0}, {})]),
    ("mono-mixer", [("gegl:mono-mixer", {}, {})]),
    ("opacity-half", [("gegl:invert-gamma", {}, {"opacity": 0.5})]),
    ("hidden", [("gegl:invert-gamma", {}, {"visible": False})]),
    ("two", [("gegl:invert-gamma", {}, {}), ("gegl:gaussian-blur", {"std-dev-x": 2.0, "std-dev-y": 2.0}, {})]),
    ("invert-16", [("gegl:invert-gamma", {}, {})], Gimp.Precision.U16_LINEAR),
]

for c in CASES:
    try:
        make(c[0], c[1], *(c[2:]))
    except Exception as e:  # keep going
        print("xcf-filters: failed", c[0], e)
