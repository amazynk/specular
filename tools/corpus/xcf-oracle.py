# GIMP 3 Python (run inside `gimp-console -i --batch-interpreter python-fu-eval`, see
# fetch-xcf.sh): for every path listed in the file $XCF_LIST, loads the XCF in GIMP and writes
# GIMP's own composite of the visible layers (its projection, in the image's precision) next
# to it as `<file>.ref`:
#
#   b"XREF" | u32 width | u32 height | u32 flags (bit 0: the image has an ICC profile)
#   | width*height*4 little-endian f32: R'G'B'A, straight alpha, sRGB-coded (for images with
#   a profile: in the profile's encoding, composited as if it were sRGB)
#
# Values are not clamped (float images keep out-of-range values).
import os
import struct
import sys

import gi

gi.require_version("Gimp", "3.0")
gi.require_version("Gegl", "0.4")
from gi.repository import Gegl, Gimp, Gio  # noqa: E402

NI = Gimp.RunMode.NONINTERACTIVE

with open(os.environ["XCF_LIST"]) as f:
    paths = [l.strip() for l in f if l.strip()]

ok = bad = 0
for p in paths:
    try:
        img = Gimp.file_load(NI, Gio.File.new_for_path(p))
        w, h = img.get_width(), img.get_height()
        if img.get_base_type() == Gimp.ImageBaseType.INDEXED:
            # A layer made from the visible image would be indexed too (snapped to the
            # palette); in RGB it holds the projection as GIMP shows it.
            img.convert_rgb()
        flags = 1 if img.get_color_profile() is not None else 0
        if flags:
            # Assign (not convert to) sRGB: GIMP then composites the stored values the way a
            # decoder that leaves colour management to the viewer does, and the reference
            # stays in the profile's encoding instead of being converted to sRGB.
            img.set_color_profile(None)
        v = Gimp.Layer.new_from_visible(img, img, "oracle")
        data = v.get_buffer().get(Gegl.Rectangle.new(0, 0, w, h), 1.0, "R'G'B'A float",
                                  Gegl.AbyssPolicy.NONE)
        with open(p + ".ref", "wb") as f:
            f.write(b"XREF" + struct.pack("<III", w, h, flags))
            f.write(data)
        v.delete()
        img.delete()
        ok += 1
    except Exception as e:  # keep going: one bad file must not stop the rest
        print("xcf-oracle: %s: %s" % (p, e), file=sys.stderr)
        bad += 1
print("xcf-oracle: %d references, %d failures" % (ok, bad))
