#!/usr/bin/env python3
"""Aztec code test corpus and oracle for specular-codes (not committed; run in a venv with
zxing-cpp (3.x, whose writer makes the symbols), pillow and numpy).

  gen <dir> [count]   writes <dir>/az-NNNN.pgm and <dir>/manifest.tsv (file, expected text
                      as hex UTF-8, size, payload kind, distortion): compact and full-range
                      symbols of every layer count, runes, every character mode, binary,
                      ECI and GS1 data; rotations, perspective, noise, blur, small modules,
                      light-on-dark and mirrored symbols
  oracle <dir>        decodes every image with zxing-cpp; prints decoded/total and writes
                      <dir>/zxing.tsv (file, ok)

The expected text is what was encoded; GS1 symbols expect the plain text with GS (0x1D)
between fields, as a clean symbol reads in zxing-cpp's plain text mode.
"""
import math
import os
import random
import sys

import numpy as np
from PIL import Image, ImageFilter

UPPER = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ '
LOWER = 'abcdefghijklmnopqrstuvwxyz '
MIXED = '@\\^_`|~\x01\x07\x0d\x1b\x7f '
PUNCT = '!"#$%&\'()*+,-./:;<=>?[]{}\r'
DIGITS = '0123456789,.'


def payload(r, i, cap):
    """(content for the writer, expected text, kind, writer options)."""
    k = i % 10
    n = r.randint(1, cap)
    if k == 0:
        t = ''.join(r.choice(UPPER) for _ in range(n))
        return t, t, 'upper', {}
    if k == 1:
        t = ''.join(r.choice(LOWER + UPPER) for _ in range(n))
        return t, t, 'lower', {}
    if k == 2:
        t = ''.join(r.choice(MIXED + LOWER) for _ in range(n))
        return t, t, 'mixed', {}
    if k == 3:
        t = ''.join(r.choice(PUNCT + UPPER + '. , : ') for _ in range(n))
        return t, t, 'punct', {}
    if k == 4:
        t = ''.join(r.choice(DIGITS + ' ') for _ in range(n * 2))
        return t, t, 'digit', {}
    if k == 5:
        b = bytes(r.randrange(256) for _ in range(n))
        return b, b.decode('latin-1'), 'binary', {}
    if k == 6:
        t = 'Grüße ' + ''.join(r.choice('äöüßéèñ שלום') for _ in range(max(1, n // 3)))
        return t, t, 'eci', {'eci': 26}
    if k == 7:
        t = 'https://example.org/' + ''.join(r.choice('abcdefghijklmnopqrstuvwxyz0123456789/?=&') for _ in range(n))
        return t, t, 'url', {}
    if k == 8:
        lot = ''.join(r.choice('ABCDEFGH0123456789') for _ in range(r.randint(1, 10)))
        t = '(01)09521234543213(10)%s(17)271231' % lot
        return t, '010952123454321310%s\x1d17271231' % lot, 'gs1', {'gs1': True}
    t = ''.join(chr(r.randint(32, 126)) for _ in range(n))
    return t, t, 'ascii', {}


def make(r, i):
    import zxingcpp
    rune = i % 23 == 11
    if rune:
        v = r.randrange(256)
        b = zxingcpp.create_barcode(str(v), zxingcpp.BarcodeFormat.AztecRune)
        content, want, pk, size = str(v), str(v), 'rune', 'rune'
    else:
        # a spread of sizes: small symbols mostly, now and then a big full-range one
        cap = r.choice([8, 20, 40, 80, 160, 400] + ([900, 1500] if i % 7 == 0 else []))
        content, want, pk, opts = payload(r, i, cap)
        version = r.choice([0, 0, 0, r.randint(1, 4), r.randint(5, 36)])
        b = None
        for v in ([version] if version else []) + [0]:
            try:
                b = zxingcpp.create_barcode(content, zxingcpp.BarcodeFormat.Aztec, **opts, **({'version': v} if v else {}))
                break
            except Exception:
                continue
        if b is None:
            content, want, pk = 'A', 'A', 'upper'
            b = zxingcpp.create_barcode(content, zxingcpp.BarcodeFormat.Aztec)
        size = ''
    m = np.array(b.to_image(scale=1, add_quiet_zones=False)) < 128
    n = m.shape[0]
    if not size:
        size = '%dx%d' % (n, n)
    mod = r.choice([2, 3, 4, 5, 6, 8]) if n < 60 else r.choice([2, 3, 4])
    border = 3
    img = np.full(((n + 2 * border) * mod,) * 2, 255, np.uint8)
    for y in range(n):
        for x in range(n):
            if m[y, x]:
                img[(y + border) * mod:(y + border + 1) * mod, (x + border) * mod:(x + border + 1) * mod] = 0
    kind = []
    if i % 11 == 7:
        img = img[:, ::-1].copy()
        kind.append('mirror')
    if i % 13 == 5:
        img = 255 - img
        kind.append('invert')
    bg = 0 if 'invert' in kind else 255
    im = Image.fromarray(img)
    pad = im.width // 2
    canvas = Image.new('L', (im.width + 2 * pad, im.height + 2 * pad), bg)
    canvas.paste(im, (pad, pad))
    im = canvas
    t = i % 5
    if t in (1, 3):
        ang = r.uniform(0, 360)
        im = im.rotate(ang, resample=Image.BILINEAR, fillcolor=bg)
        kind.append('rot%d' % ang)
    if t in (2, 3):
        w, h = im.size
        d = 0.12
        src = [(0, 0), (w, 0), (w, h), (0, h)]
        dst = [(x + r.uniform(-d, d) * w, y + r.uniform(-d, d) * h) for x, y in src]
        im = im.transform((w, h), Image.PERSPECTIVE, find_coeffs(dst, src), Image.BILINEAR, fillcolor=bg)
        kind.append('persp')
    if i % 4 == 1:
        im = im.filter(ImageFilter.GaussianBlur(r.uniform(0.5, 1.2) * mod / 3))
        kind.append('blur')
    a = np.asarray(im).astype(np.float32)
    if i % 3 == 2:
        s = r.uniform(10, 40)
        a = a + np.random.default_rng(i).normal(0, s, a.shape)
        kind.append('noise%d' % s)
    a = np.clip(a, 0, 255).astype(np.uint8)
    return a, want, size, pk, '+'.join(kind) or 'plain'


def find_coeffs(pa, pb):
    mat = []
    for p1, p2 in zip(pa, pb):
        mat.append([p1[0], p1[1], 1, 0, 0, 0, -p2[0] * p1[0], -p2[0] * p1[1]])
        mat.append([0, 0, 0, p1[0], p1[1], 1, -p2[1] * p1[0], -p2[1] * p1[1]])
    A = np.array(mat, dtype=np.float64)
    B = np.array(pb, dtype=np.float64).reshape(8)
    return np.linalg.solve(A, B).tolist()


def gen(d, count):
    os.makedirs(d, exist_ok=True)
    r = random.Random(24778)
    rows = []
    for i in range(count):
        a, want, size, pk, kind = make(r, i)
        name = 'az-%04d.pgm' % i
        h, w = a.shape
        with open(os.path.join(d, name), 'wb') as f:
            f.write(b'P5\n%d %d\n255\n' % (w, h))
            f.write(a.tobytes())
        rows.append('%s\t%s\t%s\t%s\t%s' % (name, want.encode('utf-8').hex(), size, pk, kind))
    with open(os.path.join(d, 'manifest.tsv'), 'w') as f:
        f.write('\n'.join(rows) + '\n')
    print('wrote', count)


def oracle(d):
    import zxingcpp
    ok = 0
    out = []
    rows = [l.rstrip('\n').split('\t') for l in open(os.path.join(d, 'manifest.tsv'))]
    for name, hx, size, pk, kind in rows:
        im = np.asarray(Image.open(os.path.join(d, name)))
        want = bytes.fromhex(hx).decode('utf-8')
        res = zxingcpp.read_barcodes(im, formats=zxingcpp.barcode_formats_from_str('Aztec,AztecRune'), try_rotate=True,
                                     try_downscale=True, try_invert=True, text_mode=zxingcpp.TextMode.Plain)
        good = any(x.text == want or (pk == 'rune' and x.text.lstrip('0') == want.lstrip('0')) for x in res)
        ok += good
        out.append('%s\t%d' % (name, good))
    with open(os.path.join(d, 'zxing.tsv'), 'w') as f:
        f.write('\n'.join(out) + '\n')
    print('zxing-cpp: %d/%d decoded' % (ok, len(rows)))


if __name__ == '__main__':
    if sys.argv[1] == 'gen':
        gen(sys.argv[2], int(sys.argv[3]) if len(sys.argv) > 3 else 300)
    else:
        oracle(sys.argv[2])
