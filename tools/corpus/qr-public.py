#!/usr/bin/env python3
"""A public set of real-world QR code photos for specular-codes (not committed; run in a venv
with zxing-cpp, pillow and numpy).

Source: the ZXing project's black-box test pictures, core/src/test/resources/blackbox/
qrcode-1 … qrcode-6 (phone photos and scans of printed QR codes, each with the expected
text), Apache License 2.0 (the repository's LICENSE), pinned to commit COMMIT below.
Only the pictures and their expected texts are used; no ZXing code.

  fetch <dir>    downloads the pictures (progress: bytes, speed, ETA), writes <dir>/NAME.pgm
                 (grey) and <dir>/manifest.tsv (file, expected text as hex UTF-8, set)
  oracle <dir>   decodes every picture with zxing-cpp; writes <dir>/zxing.tsv (file, ok)

Then: SPECULAR_QR_PUBLIC_CORPUS=<dir> cargo test -p specular-codes --test qr_public -- --ignored --nocapture
"""
import io
import json
import os
import sys
import time
import urllib.request

COMMIT = '56b8a9a7b53392097891463023a26c406b422225'
REPO = 'zxing/zxing'
PREFIX = 'core/src/test/resources/blackbox/qrcode-'


def get(url):
    req = urllib.request.Request(url, headers={'User-Agent': 'specular-corpus'})
    with urllib.request.urlopen(req, timeout=60) as r:
        return r.read()


def bar(done, total, start, what):
    el = max(time.time() - start, 1e-6)
    speed = done / el
    eta = (total - done) / speed if speed > 0 else 0
    n = int(30 * done / max(total, 1))
    sys.stderr.write('\r%-28s [%s%s] %5.1f/%.1f MB %6.0f kB/s ETA %3.0f s' % (
        what[-28:], '#' * n, '.' * (30 - n), done / 1e6, total / 1e6, speed / 1e3, eta))
    sys.stderr.flush()


def fetch(d):
    from PIL import Image
    os.makedirs(d, exist_ok=True)
    print('Listing %s at %s' % (REPO, COMMIT[:12]), file=sys.stderr)
    tree = json.loads(get('https://api.github.com/repos/%s/git/trees/%s?recursive=1' % (REPO, COMMIT)))
    files = {t['path']: t.get('size', 0) for t in tree['tree']
             if t['type'] == 'blob' and t['path'].startswith(PREFIX)}
    pics = sorted(p for p in files if p.endswith(('.png', '.jpg', '.gif')))
    want = []
    for p in pics:
        stem = p.rsplit('.', 1)[0]
        if stem + '.txt' in files:
            want.append((p, stem + '.txt'))
    total = sum(files[p] + files[t] for p, t in want)
    done, start, rows = 0, time.time(), []
    for p, t in want:
        img = get('https://raw.githubusercontent.com/%s/%s/%s' % (REPO, COMMIT, p))
        txt = get('https://raw.githubusercontent.com/%s/%s/%s' % (REPO, COMMIT, t))
        done += len(img) + len(txt)
        bar(done, total, start, p.rsplit('/', 2)[-2] + '/' + p.rsplit('/', 1)[-1])
        try:
            text = txt.decode('utf-8')
        except UnicodeDecodeError:
            text = txt.decode('latin-1')
        group = p.split('/')[-2]
        # one expected text has a typo (zxing-cpp's copy of the picture has it right)
        if p.endswith('qrcode-2/13.png'):
            text = text.replace('aerial photograph of', 'aerial photography of')
        name = '%s-%s.pgm' % (group, p.rsplit('/', 1)[-1].rsplit('.', 1)[0])
        im = Image.open(io.BytesIO(img)).convert('L')
        with open(os.path.join(d, name), 'wb') as f:
            f.write(b'P5\n%d %d\n255\n' % im.size)
            f.write(im.tobytes())
        rows.append('%s\t%s\t%s' % (name, text.encode('utf-8').hex(), group))
    sys.stderr.write('\n')
    with open(os.path.join(d, 'manifest.tsv'), 'w') as f:
        f.write('\n'.join(rows) + '\n')
    print('wrote %d pictures to %s' % (len(rows), d))


def oracle(d):
    import numpy as np
    import zxingcpp
    from PIL import Image
    ok, out = 0, []
    rows = [l.rstrip('\n').split('\t') for l in open(os.path.join(d, 'manifest.tsv'))]
    for name, hx, group in rows:
        im = np.asarray(Image.open(os.path.join(d, name)))
        want = bytes.fromhex(hx).decode('utf-8')
        res = zxingcpp.read_barcodes(im, formats=zxingcpp.BarcodeFormat.QRCode, try_rotate=True,
                                     try_downscale=True, try_invert=True)
        good = any(x.text == want for x in res)
        ok += good
        out.append('%s\t%d' % (name, good))
    with open(os.path.join(d, 'zxing.tsv'), 'w') as f:
        f.write('\n'.join(out) + '\n')
    print('zxing-cpp: %d/%d decoded' % (ok, len(rows)))


if __name__ == '__main__':
    {'fetch': fetch, 'oracle': oracle}[sys.argv[1]](sys.argv[2])
