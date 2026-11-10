"""CorelDRAW preview oracle: the preview Specular shows first for every .cdr under a folder,
decoded by Pillow (an independent BMP/DIB/PNG reader), written beside it as
<file>.preview.png (RGBA). The specular-decode editors test compares against these.

Which preview: a ZIP package's first page preview (previews/pageN.png or
metadata/thumbnails/pageN.bmp, lowest N), else its thumbnail; a RIFF file's DISP chunk
(clipboard format + DIB, read with Pillow's DIB reader) or version 3's imhd chunk (a BMP
file). Files without one get no reference.

Usage: python3 -I tools/corpus/cdr-oracle.py DIR   (a venv with Pillow)
"""
import io
import os
import re
import struct
import sys
import zipfile

from PIL import BmpImagePlugin, Image


def riff_preview(d):
    end = min(len(d), struct.unpack_from('<I', d, 4)[0] + 8)
    stack = [(12, end, 0)]
    while stack:
        off, end, depth = stack.pop()
        while off + 8 <= end:
            cid = d[off:off + 4]
            n = struct.unpack_from('<I', d, off + 4)[0]
            body = d[off + 8:off + 8 + n]
            if cid == b'DISP':
                return BmpImagePlugin.DibImageFile(io.BytesIO(body[4:]))
            if cid == b'imhd' and body[:2] == b'BM':
                return Image.open(io.BytesIO(body))
            if cid == b'LIST' and depth < 2:
                stack.append((off + 12, min(off + 8 + n, end), depth + 1))
            off += 8 + n + (n & 1)
    return None


def zip_preview(path):
    z = zipfile.ZipFile(path)
    pages, thumb = [], None
    for name in z.namelist():
        m = re.fullmatch(r'(?:previews/(page(\d+)|thumbnail)\.png|metadata/thumbnails/(page(\d+)|thumbnail)\.bmp)', name)
        if not m:
            continue
        num = m.group(2) or m.group(4)
        if num is not None:
            pages.append((int(num), name))
        else:
            thumb = name
    pages.sort()
    name = pages[0][1] if pages else thumb
    return Image.open(io.BytesIO(z.read(name))) if name else None


root = sys.argv[1]
made = none = 0
for dirpath, _, files in os.walk(root):
    for f in sorted(files):
        if not f.lower().endswith('.cdr'):
            continue
        p = os.path.join(dirpath, f)
        d = open(p, 'rb').read()
        try:
            img = zip_preview(p) if d[:4] == b'PK\x03\x04' else (riff_preview(d) if d[:4] == b'RIFF' else None)
        except Exception as e:
            print(f'{p}: {e}')
            img = None
        if img is None:
            none += 1
            continue
        img.convert('RGBA').save(p + '.preview.png')
        made += 1
print(f'CorelDRAW preview oracle: {made} references, {none} files without a preview')
