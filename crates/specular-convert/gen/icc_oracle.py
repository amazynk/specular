"""ICC oracle for export recipes (tests/recipe_icc_oracle.rs).

Usage: icc_oracle.py REFERENCE.rgb TOLERANCE FILE.png...

Forward: the reference sRGB pixels (raw 8-bit RGB, the PNGs' size) are converted by
LittleCMS (Pillow's ImageCms, relative colorimetric) from its built-in sRGB into each PNG's
embedded profile and compared with the PNG's pixels: this checks that our pixels are what our
profile says. Back: each PNG converted through its profile to sRGB, compared with the
reference (8-bit steps of a wide space cost a few codes near black). Prints one line per
file: name, forward worst difference in codes, forward samples off by more than TOLERANCE,
back worst difference.
"""

import io
import sys

from PIL import Image, ImageCms


def main():
    ref = open(sys.argv[1], "rb").read()
    tol = int(sys.argv[2])
    srgb = ImageCms.createProfile("sRGB")
    for path in sys.argv[3:]:
        im = Image.open(path)
        im.load()
        icc = im.info.get("icc_profile")
        if not icc:
            print(f"{path} no-profile")
            continue
        src = ImageCms.ImageCmsProfile(io.BytesIO(icc))
        out = ImageCms.profileToProfile(
            im.convert("RGB"),
            src,
            srgb,
            renderingIntent=ImageCms.Intent.RELATIVE_COLORIMETRIC,
            outputMode="RGB",
        )
        back = out.tobytes()
        if len(back) != len(ref):
            print(f"{path} size-mismatch {len(back)} {len(ref)}")
            continue
        ref_img = Image.frombytes("RGB", im.size, ref)
        fwd = ImageCms.profileToProfile(
            ref_img,
            srgb,
            src,
            renderingIntent=ImageCms.Intent.RELATIVE_COLORIMETRIC,
            outputMode="RGB",
        ).tobytes()
        ours = im.convert("RGB").tobytes()
        worst = 0
        over = 0
        for a, b in zip(fwd, ours):
            d = abs(a - b)
            worst = max(worst, d)
            if d > tol:
                over += 1
        back_worst = max(abs(a - b) for a, b in zip(back, ref))
        print(f"{path} {worst} {over} {back_worst}")


if __name__ == "__main__":
    main()
