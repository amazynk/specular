# GIMP 3 Python (run inside `gimp-console -i --batch-interpreter python-fu-eval`, see
# fetch-editors-psp.sh): for every path listed in the file $PSP_LIST, loads the PaintShop Pro
# file with GIMP's importer and writes `<file>.gref` next to it:
#
#   b"PGRF" | u32 width | u32 height | u32 layer count
#   | width*height*4 bytes: the visible layers flattened, R'G'B'A 8-bit, straight alpha
#   | per layer, top-most first (groups expanded depth first, a group itself has no pixels):
#       u32 name length | name (UTF-8) | i32 x | i32 y | u32 w | u32 h | u8 visible
#       | u8 group | u8 opacity (0..255) | u8 mode (GIMP's LayerMode value, truncated)
#       | w*h*4 bytes R'G'B'A 8-bit of the layer itself (its own alpha; mask not applied)
import os
import struct
import sys

import gi

gi.require_version("Gimp", "3.0")
gi.require_version("Gegl", "0.4")
from gi.repository import Gegl, Gimp, Gio  # noqa: E402

NI = Gimp.RunMode.NONINTERACTIVE

with open(os.environ["PSP_LIST"]) as f:
    paths = [l.strip() for l in f if l.strip()]


def walk(items, out):
    for it in items:
        out.append(it)
        if it.is_group():
            walk(it.get_children(), out)


ok = bad = 0
for p in paths:
    try:
        img = Gimp.file_load(NI, Gio.File.new_for_path(p))
        if img.get_base_type() != Gimp.ImageBaseType.RGB:
            img.convert_rgb()
        img.set_color_profile(None)
        w, h = img.get_width(), img.get_height()
        layers = []
        walk(img.get_layers(), layers)
        v = Gimp.Layer.new_from_visible(img, img, "oracle")
        comp = v.get_buffer().get(Gegl.Rectangle.new(0, 0, w, h), 1.0, "R'G'B'A u8",
                                  Gegl.AbyssPolicy.NONE)
        parts = [b"PGRF" + struct.pack("<III", w, h, len(layers)), bytes(comp)]
        for l in layers:
            name = l.get_name().encode("utf-8")
            ok_, x, y = l.get_offsets()
            group = l.is_group()
            lw, lh = (0, 0) if group else (l.get_width(), l.get_height())
            parts.append(struct.pack("<I", len(name)) + name)
            parts.append(struct.pack("<iiIIBBBB", x, y, lw, lh, 1 if l.get_visible() else 0,
                                     1 if group else 0,
                                     int(round(l.get_opacity() * 2.55)),
                                     int(l.get_mode()) & 0xFF))
            if not group:
                buf = l.get_buffer().get(Gegl.Rectangle.new(0, 0, lw, lh), 1.0,
                                         "R'G'B'A u8", Gegl.AbyssPolicy.NONE)
                parts.append(bytes(buf))
        with open(p + ".gref", "wb") as f:
            f.write(b"".join(parts))
        v.delete()
        img.delete()
        ok += 1
    except Exception as e:  # keep going: one bad file must not stop the rest
        print("psp-oracle: %s: %s" % (p, e), file=sys.stderr)
        bad += 1
print("psp-oracle: %d references, %d failures" % (ok, bad))
