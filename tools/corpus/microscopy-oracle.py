"""The stitched-scan oracle (tools/corpus/fetch-microscopy.sh).

  microscopy-oracle.py OUT     write OUT/expected.tsv, the synthetic BIF files under OUT/bif
                               and the reference pixels under OUT/oracle

Rows of expected.tsv (tab-separated; paths relative to OUT):
  slide  FILE VENDOR W H MPPX OBJECTIVE   the oracle's view of a slide (- when absent)
  levels FILE WxH,WxH,...                 stored level sizes, full resolution first
  region FILE SCALE X Y W H RAW [MAD]     RGB of a region of the 1/SCALE level, in its pixels
                                          (RAW: W*H*3 bytes; RAW.mask marks the pixels the
                                          oracle paints fully; MAD: a looser bound where the
                                          oracle resamples)
  assoc  FILE KIND W H RAW                an associated image (label, macro, thumbnail)
  image  FILE W H C Z T                   the oracle's view of a multi-channel image
  plane  FILE C Z T SCALE X Y W H RAW TOL one channel's values (RAW: little-endian f32),
                                          equal within TOL
  refuse FILE                             a file that must not open

Oracles: openslide-python 4 (MIRAX; the synthetic BIF files, whose joints run RIGHT and UP;
OpenSlide 4.0.1 refuses Ventana-1.bif's LEFT joints), tifffile (Ventana-1.bif: its raw
levels and an independent placement of their tiles by the recorded joints, written here),
Bio-Formats' bfconvert (VSI; BFTOOLS names its folder; java runs with its home and temporary
folder under OUT). Nothing downloaded is run: files are opened as data.
"""

import os
import re
import shutil
import subprocess
import sys
import xml.etree.ElementTree as ET

import numpy as np
import openslide
import tifffile

OUT = sys.argv[1]
ORACLE = os.path.join(OUT, 'oracle')
os.makedirs(ORACLE, exist_ok=True)
rows = []


def raw(name, rgb, mask=None):
    path = os.path.join('oracle', name)
    with open(os.path.join(OUT, path), 'wb') as f:
        f.write(np.ascontiguousarray(rgb, dtype=np.uint8).tobytes())
    if mask is not None:
        with open(os.path.join(OUT, path + '.mask'), 'wb') as f:
            f.write(np.ascontiguousarray(mask, dtype=np.uint8).tobytes())
    return path


def rawf(name, values):
    path = os.path.join('oracle', name)
    with open(os.path.join(OUT, path), 'wb') as f:
        f.write(np.ascontiguousarray(values, dtype='<f4').tobytes())
    return path


def slug(s):
    return re.sub(r'[^A-Za-z0-9]+', '_', s)


# -- OpenSlide: MIRAX and synthetic BIF --------------------------------------------------

def openslide_rows(rel, vendor, regions):
    s = openslide.OpenSlide(os.path.join(OUT, rel))
    p = s.properties
    w, h = s.dimensions
    rows.append(['slide', rel, vendor, w, h, p.get('openslide.mpp-x', '-'), p.get('openslide.objective-power', '-')])
    rows.append(['levels', rel, ','.join(f'{a}x{b}' for a, b in s.level_dimensions)])
    bx = int(p.get('openslide.bounds-x', 0))
    by = int(p.get('openslide.bounds-y', 0))
    bw = int(p.get('openslide.bounds-width', w))
    bh = int(p.get('openslide.bounds-height', h))
    for k, (level, fx, fy, rw, rh) in enumerate(regions):
        if level >= s.level_count:
            continue
        ds = round(s.level_downsamples[level])
        lw, lh = s.level_dimensions[level]
        x = int((bx + bw * fx) / ds)
        y = int((by + bh * fy) / ds)
        rw, rh = min(rw, lw - x), min(rh, lh - y)
        if rw <= 0 or rh <= 0:
            continue
        img = np.asarray(s.read_region((x * ds, y * ds), level, (rw, rh)))
        name = raw(f'{slug(rel)}.r{k}.raw', img[..., :3], img[..., 3] == 255)
        row = ['region', rel, ds, x, y, rw, rh, name]
        if vendor == 'mirax' and level > 0:
            # OpenSlide resamples images at fractional positions here: a looser bound.
            row.append(16)
        rows.append(row)
    for kind, img in s.associated_images.items():
        a = np.asarray(img.convert('RGB'))
        rows.append(['assoc', rel, kind, a.shape[1], a.shape[0], raw(f'{slug(rel)}.{kind}.raw', a)])


def pattern(w, h, seed):
    """A smooth colour field with sharp edges, so misplaced tiles show."""
    y, x = np.mgrid[0:h, 0:w]
    r = (x * 255 // max(w - 1, 1)).astype(np.uint8)
    g = (y * 255 // max(h - 1, 1)).astype(np.uint8)
    b = (((x // 37 + y // 29 + seed) % 2) * 160 + 40).astype(np.uint8)
    return np.dstack([r, g, b])


def serpentine(c, r, cols, rows_):
    """The serpentine number (from 1, bottom-left) of the tile at column c, row r (from the top)."""
    rb = rows_ - 1 - r
    cc = c if rb % 2 == 0 else cols - 1 - c
    return rb * cols + cc + 1


def synthetic_bif(rel, cols, rows_, tile, ox, oy, levels):
    """A BIF of cols×rows tiles cut from one picture with overlaps ox, oy, its coarser levels
    made from the unstitched tiles, with joints recorded RIGHT and UP."""
    w0 = (cols - 1) * (tile - ox) + tile
    h0 = (rows_ - 1) * (tile - oy) + tile
    base = pattern(w0, h0, 1)
    grid = np.zeros((rows_ * tile, cols * tile, 3), np.uint8)
    for r in range(rows_):
        for c in range(cols):
            y0, x0 = r * (tile - oy), c * (tile - ox)
            grid[r * tile:(r + 1) * tile, c * tile:(c + 1) * tile] = base[y0:y0 + tile, x0:x0 + tile]
    joints = []
    for r in range(rows_):
        for c in range(cols):
            if c + 1 < cols:
                joints.append(f'<TileJointInfo FlagJoined="1" Confidence="100" Direction="RIGHT" Tile1="{serpentine(c, r, cols, rows_)}" Tile2="{serpentine(c + 1, r, cols, rows_)}" OverlapX="{ox}" OverlapY="0"/>')
            if r > 0:
                joints.append(f'<TileJointInfo FlagJoined="1" Confidence="100" Direction="UP" Tile1="{serpentine(c, r, cols, rows_)}" Tile2="{serpentine(c, r - 1, cols, rows_)}" OverlapX="0" OverlapY="{oy}"/>')
    iscan = '<iScan Mode="brightfield" Magnification="40" ScanRes="0.25" ScannerModel="Synthetic" Z-layers="1"/>'
    xmp0 = f'<?xml version="1.0" encoding="utf-8"?><Metadata>{iscan}</Metadata>'.encode()
    enc = (f'<?xml version="1.0" encoding="utf-8"?><EncodeInfo Ver="2"><SlideInfo>{iscan}</SlideInfo>'
           f'<SlideStitchInfo><ImageInfo AOIScanned="1" AOIIndex="0" NumRows="{rows_}" NumCols="{cols}" '
           f'Width="{tile}" Height="{tile}" Pos-X="0" Pos-Y="0">{"".join(joints)}</ImageInfo></SlideStitchInfo>'
           f'<AoiOrigin><AOI0 OriginX="0" OriginY="0"/></AoiOrigin></EncodeInfo>').encode()
    path = os.path.join(OUT, rel)
    with tifffile.TiffWriter(path, bigtiff=True) as tw:
        tw.write(pattern(96, 64, 0), description='Label_Image', photometric='rgb', metadata=None,
                 extratags=[(700, 1, len(xmp0), xmp0, True)])
        for lv in range(levels):
            ds = 2 ** lv
            sub = tile // ds
            lc, lr = -(-cols // ds), -(-rows_ // ds)
            img = np.full((lr * tile, lc * tile, 3), 255, np.uint8)
            for r in range(rows_):
                for c in range(cols):
                    t = grid[r * tile:(r + 1) * tile, c * tile:(c + 1) * tile].astype(np.float64)
                    t = t.reshape(sub, ds, sub, ds, 3).mean(axis=(1, 3)).round().astype(np.uint8)
                    tr, tc = r // ds, c // ds
                    y, x = tr * tile + (r % ds) * sub, tc * tile + (c % ds) * sub
                    img[y:y + sub, x:x + sub] = t
            extra = [(700, 1, len(enc), enc, True)] if lv == 0 else []
            tw.write(img, tile=(tile, tile), compression='jpeg', compressionargs={'level': 95}, photometric='rgb', metadata=None,
                     description=f'level={lv} mag={40 / ds:g} quality=95', extratags=extra)


# -- Ventana-1.bif: tifffile and an independent placement ---------------------------------

def bif_rows(rel):
    path = os.path.join(OUT, rel)
    with tifffile.TiffFile(path) as t:
        pages = list(t.pages)
        level = {}
        for i, p in enumerate(pages):
            d = p.tags.get('ImageDescription')
            m = d and re.search(r'level=(\d+) mag=([\d.]+)', d.value)
            if m:
                level[int(m.group(1))] = p
            elif d and d.value in ('Label Image', 'Label_Image'):
                a = p.asarray()
                rows.append(['assoc', rel, 'macro', a.shape[1], a.shape[0], raw(f'{slug(rel)}.macro.raw', a)])
        p0 = level[0]
        x = p0.tags['XMP'].value
        x = x.decode() if isinstance(x, bytes) else x
        root = ET.fromstring(x.rstrip('\0 \n').encode())
        info = root.find('.//SlideStitchInfo/ImageInfo')
        cols, rws = int(info.get('NumCols')), int(info.get('NumRows'))
        tw, th = p0.tilewidth, p0.tilelength
        left = {}
        top = {}

        def coords(n):
            i = n - 1
            c, r = i % cols, i // cols
            if r % 2:
                c = cols - 1 - c
            return c, rws - 1 - r

        sums = [0.0, 0.0]
        wts = [0.0, 0.0]
        for j in info.iter('TileJointInfo'):
            a, b = coords(int(j.get('Tile1'))), coords(int(j.get('Tile2')))
            conf = float(j.get('Confidence', 0))
            known = conf > 0 and j.get('FlagJoined', '1') != '0'
            if a[1] == b[1] and abs(a[0] - b[0]) == 1:
                ov = float(j.get('OverlapX'))
                left[(max(a[0], b[0]), a[1])] = ov if known else None
                sums[0] += conf * ov
                wts[0] += conf
            elif a[0] == b[0] and abs(a[1] - b[1]) == 1:
                ov = float(j.get('OverlapY'))
                top[(a[0], max(a[1], b[1]))] = ov if known else None
                sums[1] += conf * ov
                wts[1] += conf
        mean = [sums[k] / wts[k] if wts[k] else 0.0 for k in (0, 1)]
        off = {}
        for r in range(rws):
            for c in range(cols):
                ovx = left.get((c, r))
                ovy = top.get((c, r))
                xx = 0.0 if c == 0 else off[(c - 1, r)][0] + tw - (mean[0] if ovx is None else ovx)
                yy = 0.0 if r == 0 else off[(c, r - 1)][1] + th - (mean[1] if ovy is None else ovy)
                off[(c, r)] = (xx, yy)
        width = int(np.ceil(max(o[0] for o in off.values()) + tw))
        height = int(np.ceil(max(o[1] for o in off.values()) + th))
        x0 = pages[0].tags['XMP'].value
        x0 = x0.decode() if isinstance(x0, bytes) else x0
        scan = ET.fromstring(x0.rstrip('\0 \n').encode()).find('.//iScan')
        rows.append(['slide', rel, 'ventana', width, height, scan.get('ScanRes'), scan.get('Magnification')])
        for lv, (fx, fy, rw, rh) in [(3, (0.45, 0.45, 512, 512)), (5, (0.0, 0.0, 733, 672))]:
            ds = 2 ** lv
            img = level[lv].asarray()
            lw, lh = -(-width // ds), -(-height // ds)
            canvas = np.full((lh, lw, 3), 255, np.uint8)
            sub = tw // ds
            for r in range(rws):
                for c in range(cols):
                    ox, oy = off[(c, r)]
                    px, py = int(np.floor(ox / ds + 0.5)), int(np.floor(oy / ds + 0.5))
                    sy, sx = (r // ds) * th + (r % ds) * sub, (c // ds) * tw + (c % ds) * sub
                    part = img[sy:sy + sub, sx:sx + sub]
                    hh, ww = min(part.shape[0], lh - py), min(part.shape[1], lw - px)
                    if hh > 0 and ww > 0:
                        canvas[py:py + hh, px:px + ww] = part[:hh, :ww]
            x0, y0 = int(lw * fx), int(lh * fy)
            rw, rh = min(rw, lw - x0), min(rh, lh - y0)
            reg = canvas[y0:y0 + rh, x0:x0 + rw]
            rows.append(['region', rel, ds, x0, y0, rw, rh, raw(f'{slug(rel)}.l{lv}.raw', reg)])


# -- Bio-Formats: VSI ---------------------------------------------------------------------

BFTOOLS = os.environ.get('BFTOOLS', '')


def java_env():
    home = os.path.join(OUT, '.java')
    tmp = os.path.join(OUT, '.java', 'tmp')
    os.makedirs(tmp, exist_ok=True)
    env = dict(os.environ)
    env['_JAVA_OPTIONS'] = f'-Duser.home={home} -Djava.io.tmpdir={tmp}'
    return env


def bf(tool, *args):
    return subprocess.run([os.path.join(BFTOOLS, tool), '-no-upgrade', *args], env=java_env(),
                          capture_output=True, text=True, check=True).stdout


def bf_series(path):
    """Per unflattened series: (resolutions, width, height, C, Z, T, rgb, type)."""
    out = bf('showinf', '-nopix', '-noflat', '-nometa', path)
    series = []
    for block in re.split(r'Series #\d+ :', out)[1:]:
        g = lambda k: re.search(rf'{k} = (\S+)', block)
        series.append(dict(res=int(g('Resolutions').group(1)) if g('Resolutions') else 1,
                           w=int(g('Width').group(1)), h=int(g('Height').group(1)),
                           c=int(g('SizeC').group(1)), z=int(g('SizeZ').group(1)), t=int(g('SizeT').group(1)),
                           rgb=g('RGB').group(1) == 'true', type=g('Pixel type').group(1)))
    return series


def bf_crop(path, series, x, y, w, h, c=None, z=None, t=None):
    tmp = os.path.join(OUT, '.java', 'crop.tif')
    if os.path.exists(tmp):
        os.remove(tmp)
    args = ['-overwrite', '-series', str(series), '-crop', f'{x},{y},{w},{h}']
    for k, v in (('-channel', c), ('-z', z), ('-timepoint', t)):
        if v is not None:
            args += [k, str(v)]
    bf('bfconvert', *args, path, tmp)
    a = tifffile.imread(tmp)
    os.remove(tmp)
    return a


def vsi_rows(rel, main, planes, regions_rgb, tol):
    """`main`: the unflattened series that is the slide. `planes`: (res, c, z, t, fx, fy, w, h)."""
    path = os.path.join(OUT, rel)
    ser = bf_series(path)
    first = [sum(s['res'] for s in ser[:i]) for i in range(len(ser))]
    m = ser[main]
    rows.append(['image', rel, m['w'], m['h'], 1 if m['rgb'] else m['c'], m['z'], m['t']])
    sizes = [(m['w'], m['h'])]
    for _ in range(1, m['res']):
        pw, ph = sizes[-1]
        sizes.append((pw // 2 + pw % 2, ph // 2 + ph % 2))
    for k, (res, c, z, t, fx, fy, w, h) in enumerate(planes):
        lw, lh = sizes[res]
        x, y = int(lw * fx), int(lh * fy)
        w, h = min(w, lw - x), min(h, lh - y)
        a = bf_crop(path, first[main] + res, x, y, w, h, c, z, t).astype(np.float32)
        a = a.reshape(h, w)
        rows.append(['plane', rel, c, z, t, 2 ** res, x, y, w, h, rawf(f'{slug(rel)}.p{k}.raw', a), tol])
    for k, (s, kind) in enumerate(regions_rgb):
        a = bf_crop(path, first[s], 0, 0, ser[s]['w'], ser[s]['h'])
        if a.ndim == 3 and a.shape[0] == 3:
            a = np.moveaxis(a, 0, -1)
        rows.append(['assoc', rel, kind, a.shape[1], a.shape[0], raw(f'{slug(rel)}.a{k}.raw', a)])


def main():
    # MIRAX
    regions = [(0, 0.5, 0.5, 512, 512), (0, 0.31, 0.62, 400, 300), (1, 0.5, 0.4, 512, 512),
               (3, 0.0, 0.0, 512, 512), (5, 0.0, 0.0, 1024, 1024)]
    for rel in ['mrxs/CMU-1-Saved-1_16.mrxs', 'mrxs/CMU-1-Saved-1_2.mrxs', 'mrxs/Mirax2-Fluorescence-2.mrxs']:
        if os.path.exists(os.path.join(OUT, rel)):
            openslide_rows(rel, 'mirax', regions)
    # Synthetic BIF through OpenSlide
    synthetic_bif('bif/synthetic-right.bif', 4, 3, 256, 16, 8, 3)
    synthetic_bif('bif/synthetic-wide.bif', 7, 5, 128, 24, 12, 4)
    for rel in ['bif/synthetic-right.bif', 'bif/synthetic-wide.bif']:
        openslide_rows(rel, 'ventana', [(0, 0.0, 0.0, 2048, 2048), (0, 0.3, 0.4, 200, 150), (1, 0.0, 0.0, 1024, 1024),
                                         (2, 0.0, 0.0, 512, 512)])
    if os.path.exists(os.path.join(OUT, 'bif/Ventana-1.bif')):
        bif_rows('bif/Ventana-1.bif')
    # VSI through Bio-Formats
    if BFTOOLS and os.path.exists(os.path.join(OUT, 'vsi/metadataTest_01.vsi')):
        vsi_rows('vsi/metadataTest_01.vsi', 0,
                 [(0, 0, 0, 0, 0, 0, 260, 216), (0, 1, 17, 4, 0, 0, 260, 216), (0, 1, 9, 2, 0.25, 0.3, 100, 80)],
                 [], 0)
    if BFTOOLS and os.path.exists(os.path.join(OUT, 'vsi/Image_V4.1_FL_2D.vsi')):
        vsi_rows('vsi/Image_V4.1_FL_2D.vsi', 2,
                 [(0, 0, 0, 0, 0.45, 0.45, 512, 512), (0, 3, 0, 0, 0.6, 0.3, 300, 200), (2, 1, 0, 0, 0.3, 0.3, 512, 512),
                  (5, 2, 0, 0, 0.0, 0.0, 341, 171)],
                 [(4, 'thumbnail')], 1)
    rows.append(['refuse', 'vsi/missing-frames.vsi'])
    shutil.copy(os.path.join(OUT, 'vsi/metadataTest_01.vsi'), os.path.join(OUT, 'vsi/missing-frames.vsi'))
    with open(os.path.join(OUT, 'expected.tsv'), 'w') as f:
        f.write('# written by tools/corpus/microscopy-oracle.py\n')
        for r in rows:
            f.write('\t'.join(str(v) for v in r) + '\n')
    print(f'{len(rows)} rows')


main()
