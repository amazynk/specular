"""The whole-slide / OME corpus oracle (tools/corpus/fetch-wsi.sh).

  wsi-oracle.py zarr OUT     write the OME-Zarr stores under OUT/zarr (deterministic content)
  wsi-oracle.py oracle OUT   write OUT/expected.tsv and the reference regions under OUT/oracle

Rows of expected.tsv (tab-separated; paths relative to OUT):
  slide  FILE VENDOR W H MPPX OBJECTIVE      openslide-python's view of a slide (- when absent)
  region FILE SCALE X Y W H RAW              RGB of a region of the 1/SCALE level, in its pixels
                                             (RAW: W*H*3 bytes; pixels openslide leaves
                                             transparent are written 0,0,0 with RAW.mask 0)
  assoc  FILE KIND W H RAW                   an associated image (label, macro, thumbnail)
  image  FILE W H C Z T                      tifffile's / zarr's view of a multi-dimensional image
  plane  FILE C Z T SCALE X Y W H RAW        one channel's values (RAW: little-endian f32)
  refuse FILE                                a file the oracle cannot read either

Nothing downloaded is run: files are opened as data by openslide, tifffile and zarr.
"""

import json
import os
import shutil
import sys
import zipfile

import numpy as np

REGION = 256


def pattern(c, z, y, x):
    return ((x * 7 + y * 13 + c * 1000 + z * 50) % 4096).astype(np.uint16)


def pyramid(a, n):
    """`n` levels of `a` (..., Y, X), each the 2x2 mean of the one above (edges cropped)."""
    out = [a]
    for _ in range(n - 1):
        p = out[-1]
        h, w = p.shape[-2] // 2 * 2, p.shape[-1] // 2 * 2
        p = p[..., :h, :w].astype(np.uint32)
        q = (p[..., 0::2, 0::2] + p[..., 1::2, 0::2] + p[..., 0::2, 1::2] + p[..., 1::2, 1::2] + 2) // 4
        out.append(q.astype(a.dtype))
    return out


def multiscales(n, axes):
    sp = [a for a in axes if a['type'] == 'space']
    return [{
        'name': 'synthetic',
        'axes': axes,
        'datasets': [{
            'path': str(l),
            'coordinateTransformations': [{
                'type': 'scale',
                'scale': [1.0] * (len(axes) - len(sp)) + [2.0] * (len(sp) - 2) + [0.25 * 2 ** l] * 2,
            }],
        } for l in range(n)],
    }]


OMERO = {'channels': [
    {'label': 'DAPI', 'color': '0000FF', 'window': {'start': 0, 'end': 4095, 'min': 0, 'max': 4095}},
    {'label': 'GFP', 'color': '00FF00', 'window': {'start': 0, 'end': 4095, 'min': 0, 'max': 4095}},
    {'label': 'Cy5', 'color': 'FF00FF', 'window': {'start': 0, 'end': 4095, 'min': 0, 'max': 4095}},
]}


def make_zarr(out):
    import numcodecs
    import zarr
    import zarr.codecs

    root = os.path.join(out, 'zarr')
    shutil.rmtree(root, ignore_errors=True)
    os.makedirs(root)
    c, z, y, x = np.meshgrid(np.arange(3), np.arange(4), np.arange(600), np.arange(800), indexing='ij')
    czyx = pattern(c, z, y, x)
    axes = [
        {'name': 'c', 'type': 'channel'},
        {'name': 'z', 'type': 'space', 'unit': 'micrometer'},
        {'name': 'y', 'type': 'space', 'unit': 'micrometer'},
        {'name': 'x', 'type': 'space', 'unit': 'micrometer'},
    ]
    levels = pyramid(czyx, 3)
    stores = [
        ('v04-blosc.ome.zarr', 2, numcodecs.Blosc(cname='lz4', clevel=5, shuffle=numcodecs.Blosc.SHUFFLE), '/'),
        ('v04-zstd.ome.zarr', 2, numcodecs.Zstd(level=3), '/'),
        ('v04-gzip.ome.zarr', 2, numcodecs.GZip(level=5), '.'),
        ('v05-zstd.ome.zarr', 3, zarr.codecs.ZstdCodec(level=3), None),
        ('v05-gzip.ome.zarr', 3, zarr.codecs.GzipCodec(level=5), None),
    ]
    for name, fmt, comp, sep in stores:
        path = os.path.join(root, name)
        g = zarr.open_group(path, mode='w', zarr_format=fmt)
        for l, a in enumerate(levels):
            kw = {'chunk_key_encoding': {'name': 'v2', 'separator': sep}} if fmt == 2 else {}
            arr = g.create_array(str(l), shape=a.shape, chunks=(1, 1, 128, 160), dtype=a.dtype,
                                 compressors=comp, fill_value=0, **kw)
            arr[...] = a
        ms = multiscales(len(levels), axes)
        if fmt == 2:
            ms[0]['version'] = '0.4'
            g.attrs['multiscales'] = ms
            g.attrs['omero'] = OMERO
        else:
            g.attrs['ome'] = {'version': '0.5', 'multiscales': ms, 'omero': OMERO}
    # A 2-D single-channel uint8 image, v0.4, uncompressed, with a partial edge chunk.
    yx = ((np.arange(301)[:, None] * 3 + np.arange(457)[None, :] * 5) % 251).astype(np.uint8)
    g = zarr.open_group(os.path.join(root, 'v04-yx-raw.ome.zarr'), mode='w', zarr_format=2)
    yl = pyramid(yx, 2)
    for l, a in enumerate(yl):
        arr = g.create_array(str(l), shape=a.shape, chunks=(64, 64), dtype=a.dtype, compressors=None,
                             fill_value=0, chunk_key_encoding={'name': 'v2', 'separator': '/'})
        arr[...] = a
    ms = multiscales(2, [{'name': 'y', 'type': 'space', 'unit': 'micrometer'},
                         {'name': 'x', 'type': 'space', 'unit': 'micrometer'}])
    ms[0]['version'] = '0.4'
    g.attrs['multiscales'] = ms
    # The blosc store zipped (stored entries, the group at the root of the archive).
    src = os.path.join(root, 'v04-blosc.ome.zarr')
    with zipfile.ZipFile(os.path.join(root, 'v04-blosc.ome.zarr.zip'), 'w', zipfile.ZIP_STORED) as zf:
        for d, _, files in sorted(os.walk(src)):
            for f in sorted(files):
                p = os.path.join(d, f)
                zf.write(p, os.path.relpath(p, src))


def rel(out, p):
    return os.path.relpath(p, out)


class Writer:
    def __init__(self, out):
        self.out = out
        self.dir = os.path.join(out, 'oracle')
        shutil.rmtree(self.dir, ignore_errors=True)
        os.makedirs(self.dir)
        self.rows = []
        self.n = 0

    def raw(self, data, mask=None):
        self.n += 1
        name = os.path.join('oracle', f'{self.n:04}.raw')
        with open(os.path.join(self.out, name), 'wb') as f:
            f.write(data)
        if mask is not None:
            with open(os.path.join(self.out, name + '.mask'), 'wb') as f:
                f.write(mask)
        return name

    def row(self, *f):
        self.rows.append('\t'.join(str(v) for v in f))


def opt(v):
    return '-' if v is None else v


def oracle_slide(w, out, path):
    import openslide
    r = rel(out, path)
    try:
        s = openslide.OpenSlide(path)
    except Exception:
        w.row('refuse', r)
        return
    p = s.properties
    # A Leica collection with one main image: Specular shows that image, OpenSlide the whole
    # collection with the image at its bounds, so regions are taken in the image's frame.
    ox, oy, fw, fh = 0, 0, s.dimensions[0], s.dimensions[1]
    if p.get('openslide.vendor') == 'leica' and 'openslide.region[1].x' not in p \
            and 'openslide.region[0].x' in p:
        ox, oy, fw, fh = (int(p[f'openslide.region[0].{k}']) for k in ('x', 'y', 'width', 'height'))
    w.row('slide', r, p.get('openslide.vendor'), fw, fh,
          opt(p.get('openslide.mpp-x')), opt(p.get('openslide.objective-power')))
    for lv, ds in enumerate(s.level_downsamples):
        scale = round(ds)
        if abs(ds - scale) > 0.002 * scale or scale & (scale - 1):
            continue
        lw, lh = fw // scale, fh // scale
        # The image's origin in this level's pixels: when it falls within a hair of a pixel
        # boundary (CMU's Leica-1 at 1/64: 548.996), OpenSlide's rounding places the image a
        # row off from the level's own pixels; such levels are not compared.
        if any(abs(o / ds - round(o / ds)) < 0.02 and o / ds % 1 for o in (ox, oy)):
            continue
        # The middle and a corner-ish area (bounds where content usually is), plus the far
        # bottom-right edge of the level.
        spots = {(lw // 2, lh // 2), (lw // 3, lh // 5), (max(lw - REGION, 0), max(lh - REGION, 0))}
        for (x, y) in sorted(spots):
            rw, rh = min(REGION, lw - x), min(REGION, lh - y)
            img = np.asarray(s.read_region((ox + x * scale, oy + y * scale), lv, (rw, rh)))
            a = img[..., 3]
            rgb = img[..., :3].copy()
            # Un-premultiply nothing: openslide's RGBA is opaque where there is tissue data;
            # partial alpha only at region edges of SCN/Leica collections.
            mask = (a == 255).astype(np.uint8)
            rgb[a != 255] = 0
            name = w.raw(rgb.tobytes(), None if mask.all() else mask.tobytes())
            w.row('region', r, scale, x, y, rw, rh, name)
    for kind, img in s.associated_images.items():
        a = np.asarray(img.convert('RGB'))
        name = w.raw(a.tobytes())
        w.row('assoc', r, kind, a.shape[1], a.shape[0], name)


def five(series):
    """The series' levels as (T, C, Z, Y, X) arrays (memory maps where possible)."""
    out = []
    for lv in series.levels:
        a = lv.asarray()
        axes = lv.axes
        if 'S' in axes:
            return None
        for ax in 'TCZ':
            if ax not in axes:
                a = a[np.newaxis]
                axes = ax + axes
        order = [axes.index(ax) for ax in 'TCZYX']
        out.append(np.transpose(a, order))
    return out


def planes(w, r, levels, scales):
    t_, c_, z_, h, wd = levels[0].shape
    w.row('image', r, wd, h, c_, z_, t_)
    for lv, (a, scale) in enumerate(zip(levels, scales)):
        lh, lw = a.shape[-2:]
        for (c, z, t) in sorted({(0, 0, 0), (c_ - 1, z_ - 1, t_ - 1), (c_ // 2, z_ // 2, 0)}):
            x, y = lw // 3, lh // 4
            rw, rh = min(REGION, lw - x), min(REGION, lh - y)
            v = np.ascontiguousarray(a[t, c, z, y:y + rh, x:x + rw]).astype('<f4')
            name = w.raw(v.tobytes())
            w.row('plane', r, c, z, t, scale, x, y, rw, rh, name)


def oracle_companion(w, out, path, meta):
    """A BinaryOnly file of an OME-TIFF set: the image its companion OME-XML describes, its
    planes read by tifffile from the files the TiffData elements name (same folder)."""
    import tifffile
    import xml.etree.ElementTree as ET
    ns = {'o': 'http://www.openmicroscopy.org/Schemas/OME/2016-06'}
    folder = os.path.dirname(path)
    px = ET.parse(os.path.join(folder, meta)).getroot().find('o:Image/o:Pixels', ns)
    size = {k: int(px.get('Size' + k)) for k in 'XYZCT'}
    a = None
    for td in px.findall('o:TiffData', ns):
        name = td.find('o:UUID', ns).get('FileName')
        if os.path.basename(name) != name:
            continue
        v = tifffile.imread(os.path.join(folder, name), key=int(td.get('IFD', 0)))
        if a is None:
            a = np.zeros((size['T'], size['C'], size['Z']) + v.shape, v.dtype)
        a[int(td.get('FirstT', 0)), int(td.get('FirstC', 0)), int(td.get('FirstZ', 0))] = v
    planes(w, rel(out, path), [a], [1])


def oracle_tiff(w, out, path):
    import tifffile
    r = rel(out, path)
    try:
        with tifffile.TiffFile(path) as t:
            desc = t.pages[0].description or ''
            if '<BinaryOnly' in desc:
                import re
                meta = re.search(r'MetadataFile="([^"]+)"', desc).group(1)
                oracle_companion(w, out, path, meta)
                return
            s = t.series[0]
            levels = five(s)
            if levels is None:
                w.row('refuse', r)
                return
            base = levels[0].shape[-1]
            scales = [round(base / a.shape[-1]) for a in levels]
            keep = [(a, sc) for a, sc in zip(levels, scales) if sc & (sc - 1) == 0]
            planes(w, r, [a for a, _ in keep], [sc for _, sc in keep])
    except Exception as e:
        print(f'{r}: {e}', file=sys.stderr)
        w.row('refuse', r)


def oracle_zarr(w, out, path):
    import zarr
    r = rel(out, path)
    src = path
    if path.endswith('.zip'):
        src = zarr.storage.ZipStore(path, mode='r')
    g = zarr.open_group(src, mode='r')
    attrs = dict(g.attrs)
    ms = attrs.get('ome', attrs).get('multiscales')[0]
    names = [a['name'] for a in ms['axes']]
    levels, scales = [], []
    for l, d in enumerate(ms['datasets']):
        a = g[d['path']][...]
        missing = [ax for ax in 'tcz' if ax not in names]
        a = a.reshape((1,) * len(missing) + a.shape)
        full = missing + names
        a = np.transpose(a, [full.index(ax) for ax in 'tczyx'])
        levels.append(a)
        scales.append(2 ** l)
    planes(w, r, levels, scales)


def oracle(out):
    w = Writer(out)
    for d in sorted(os.listdir(os.path.join(out, 'openslide'))):
        for f in sorted(os.listdir(os.path.join(out, 'openslide', d))):
            p = os.path.join(out, 'openslide', d, f)
            oracle_slide(w, out, p)
            if f.endswith('.scn'):
                oracle_tiff(w, out, p)
    for root, _, files in sorted(os.walk(os.path.join(out, 'ome'))):
        for f in sorted(files):
            if '.ome.' in f and not f.endswith('.companion.ome'):
                oracle_tiff(w, out, os.path.join(root, f))
    zroot = os.path.join(out, 'zarr')
    if os.path.isdir(zroot):
        for f in sorted(os.listdir(zroot)):
            oracle_zarr(w, out, os.path.join(zroot, f))
    with open(os.path.join(out, 'expected.tsv'), 'w') as f:
        f.write('# written by tools/corpus/wsi-oracle.py; see its docstring for the rows\n')
        f.write('\n'.join(w.rows) + '\n')
    print(f'{len(w.rows)} rows, {w.n} reference regions')


def main():
    mode, out = sys.argv[1], sys.argv[2]
    if mode == 'zarr':
        make_zarr(out)
    elif mode == 'oracle':
        oracle(out)
    else:
        sys.exit(f'unknown mode {mode}')


if __name__ == '__main__':
    main()
