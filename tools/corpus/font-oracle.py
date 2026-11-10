"""fontTools' decompression of the WOFF/WOFF2 files of tools/corpus/fonts, for the
specular-font corpus test to compare table bytes with:

    python -I tools/corpus/font-oracle.py tools/corpus/fonts <out dir>
    SPECULAR_FONT_ORACLE=<out dir> cargo test -p specular-font --test corpus -- --nocapture

Writes <out dir>/<name>.sfnt for each.
"""

import os
import sys

from fontTools.ttLib import TTFont


def main():
    src, out = sys.argv[1], sys.argv[2]
    os.makedirs(out, exist_ok=True)
    for name in sorted(os.listdir(src)):
        if not name.endswith((".woff", ".woff2")):
            continue
        f = TTFont(os.path.join(src, name))
        f.flavor = None
        f.save(os.path.join(out, name + ".sfnt"), reorderTables=True)
        print(name)


if __name__ == "__main__":
    main()
