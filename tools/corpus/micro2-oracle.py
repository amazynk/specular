"""The micro2 oracle (tools/corpus/fetch-micro2.sh): ICS, FCS and Hamamatsu VMS / VMU.

  micro2-oracle.py OUT     write OUT/expected.tsv, the synthetic VMS / VMU slides under
                           OUT/vms-synth, a VMS made of the real CMU-1 bottom row under
                           OUT/vms-cut and the reference pixels under OUT/oracle

Rows of expected.tsv (tab-separated; paths relative to OUT):
  ics    FILE SIZES TENSOR SUM MIN MAX     DIPlib's reading (dip.ImageReadICS): every sample's
                                           sum and extremes (complex as magnitude)
  fcs    FILE DS EVENTS NAMES SUMS FIRST LAST
                                           fcsparser's raw channel values of dataset DS
                                           (NAMES ';'-joined $PnN, SUMS / FIRST / LAST
                                           ','-joined per parameter)
  flowio FILE DS EVENTS RAW SCALE          FlowIO's values of dataset DS: per-parameter
                                           sums of the raw channels and of the scale
                                           values (gain, $PnE, $TIMESTEP applied)
  comp   FILE DS PARAM SUM                 flowutils' compensation of FlowIO's scale values
                                           through the dataset's spillover matrix
  logicle T W M A XS YS                    flowutils' logicle of XS (','-joined)
  slide  FILE W H MPPX OBJECTIVE           OpenSlide's view of a VMS / VMU
  region FILE LEVEL X Y W H RAW            OpenSlide's RGB of a region of a level (RAW:
                                           W*H*3 bytes), level downsample a power of two
  assoc  FILE KIND W H RAW                 an associated image

Oracles: diplib (libics), fcsparser 0.2, FlowIO, flowutils, openslide-python 4 with
openslide-bin; Pillow writes the synthetic JPEGs. Nothing downloaded is run.
"""

import io
import os
import struct
import sys

import numpy as np

OUT = sys.argv[1]
ORACLE = os.path.join(OUT, 'oracle')
os.makedirs(ORACLE, exist_ok=True)
rows = []


def raw(name, rgb):
    path = os.path.join('oracle', name)
    with open(os.path.join(OUT, path), 'wb') as f:
        f.write(np.ascontiguousarray(rgb, dtype=np.uint8).tobytes())
    return path


def fmt(v):
    return f'{v:.10g}'


# ---- ICS ------------------------------------------------------------------------------
import diplib as dip

for name in sorted(os.listdir(os.path.join(OUT, 'ics'))):
    if not name.endswith('.ics'):
        continue
    p = os.path.join(OUT, 'ics', name)
    try:
        img = dip.ImageReadICS(p)
    except Exception as e:  # noqa: BLE001
        print('diplib refuses', name, e)
        continue
    a = np.asarray(img)
    v = np.abs(a) if np.iscomplexobj(a) else a.astype(np.float64)
    rows.append(['ics', f'ics/{name}', 'x'.join(map(str, img.Sizes())), str(img.TensorElements()),
                 fmt(v.sum()), fmt(v.min()), fmt(v.max())])

# ---- FCS ------------------------------------------------------------------------------
import fcsparser
import flowio
import flowutils

for name in sorted(os.listdir(os.path.join(OUT, 'fcs'))):
    p = os.path.join(OUT, 'fcs', name)
    rel = f'fcs/{name}'
    try:
        if name == 'flowio_data_start_discrepancy.fcs':
            # fcsparser takes the HEADER's data offset, which points into TEXT here (FlowIO
            # and we take $BEGINDATA, whose segment holds exactly the events).
            raise ValueError('header offset into TEXT')
        meta, data = fcsparser.parse(p, reformat_meta=True, channel_naming='$PnN')
        arr = data.to_numpy(dtype=np.float64)
        names = list(meta['_channel_names_'])
        rows.append(['fcs', rel, '0', str(arr.shape[0]), ';'.join(names),
                     ','.join(fmt(s) for s in arr.sum(axis=0)),
                     ','.join(fmt(s) for s in arr[0]), ','.join(fmt(s) for s in arr[-1])])
    except Exception as e:  # noqa: BLE001
        print('fcsparser refuses', name, type(e).__name__, e)
    try:
        try:
            sets = [flowio.FlowData(p, ignore_offset_discrepancy=True, ignore_offset_error=True)]
        except flowio.exceptions.MultipleDataSetsError:
            sets = flowio.read_multiple_data_sets(p, ignore_offset_discrepancy=True,
                                                  ignore_offset_error=True)
        for ds, f in enumerate(sets):
            ev = f.as_array(preprocess=False)
            try:
                sc = f.as_array(preprocess=True)
            except ValueError as e:
                # FlowIO's own examples carry a $TIMESTEP of 'xxxxxxxxx'.
                print('flowio cannot scale', name, e)
                sc = None
            rows.append(['flowio', rel, str(ds), str(ev.shape[0]),
                         ','.join(fmt(s) for s in ev.sum(axis=0)),
                         '-' if sc is None else ','.join(fmt(s) for s in sc.sum(axis=0))])
            if sc is None:
                continue
            text = {k.lower(): v for k, v in f.text.items()}
            spill = text.get('spillover') or text.get('spill') or text.get('$spill')
            if spill and len(spill.split(',')) == 1 + int(spill.split(',')[0]) * (1 + int(spill.split(',')[0])):
                parts = [s.strip() for s in spill.split(',')]
                n = int(parts[0])
                labels = parts[1:1 + n]
                m = np.array([float(x) for x in parts[1 + n:]]).reshape(n, n)
                pnn = [f.channels[i + 1]['pnn'] for i in range(f.channel_count)]
                idx = [pnn.index(l) for l in labels]
                comp = flowutils.compensate.compensate(sc.copy(), m, idx)
                for i in idx:
                    rows.append(['comp', rel, str(ds), str(i), fmt(comp[:, i].sum())])
    except Exception as e:  # noqa: BLE001
        print('flowio refuses', name, type(e).__name__, e)

xs = np.array([-1000.0, -50.0, 0.0, 1.0, 10.0, 100.0, 1000.0, 10000.0, 100000.0, 262144.0, 400000.0])
for (t, w, m, a) in [(262144.0, 0.5, 4.5, 0.0), (10000.0, 1.0, 4.0, 0.0), (262144.0, 0.0, 4.5, 0.0)]:
    ys = flowutils.transforms.logicle(xs.reshape(-1, 1).copy(), [0], t=t, w=w, m=m, a=a)[:, 0]
    rows.append(['logicle', fmt(t), fmt(w), fmt(m), fmt(a), ','.join(fmt(v) for v in xs),
                 ','.join(fmt(v) for v in ys)])

# ---- VMS / VMU ------------------------------------------------------------------------
import openslide
from PIL import Image

SYN = os.path.join(OUT, 'vms-synth')
os.makedirs(SYN, exist_ok=True)


def pattern(w, h, seed):
    y, x = np.mgrid[0:h, 0:w]
    r = (x * 255 // max(w, 1)).astype(np.uint8)
    g = (y * 255 // max(h, 1)).astype(np.uint8)
    b = (((x + y + seed * 40) % 256) // 2 + 64).astype(np.uint8)
    # Some texture so JPEG blocks differ.
    b = (b + ((x // 7 + y // 5) % 3) * 20).astype(np.uint8)
    return np.dstack([r, g, b])


def jpeg(arr, restart_blocks=0, sub=0):
    b = io.BytesIO()
    kw = {'quality': 92, 'subsampling': sub}
    if restart_blocks:
        kw['restart_marker_blocks'] = restart_blocks
    Image.fromarray(arr).save(b, 'JPEG', **kw)
    return b.getvalue()


def row_starts(j, per_row):
    """Offsets after each restart marker that starts an MCU row, the first the scan data."""
    i = 2
    while True:
        m = j[i + 1]
        n = struct.unpack('>H', j[i + 2:i + 4])[0]
        if m == 0xDA:
            start = i + 2 + n
            break
        i += 2 + n
    out = [start]
    k = start
    count = 0
    while k < len(j) - 1:
        if j[k] == 0xFF and 0xD0 <= j[k + 1] <= 0xD7:
            count += 1
            if count % per_row == 0:
                out.append(k + 2)
            k += 2
            continue
        k += 1
    return out


def synth_vms(name, cols, rows_, cw, ch, restart, with_opt, sub=0):
    w, h = sum(cw), sum(ch)
    full = pattern(w, h, 5)
    lines = ['[Virtual Microscope Specimen]', 'NoLayers=1', f'NoJpegColumns={cols}',
             f'NoJpegRows={rows_}']
    opt = b''
    y0 = 0
    for r in range(rows_):
        x0 = 0
        for c in range(cols):
            part = full[y0:y0 + ch[r], x0:x0 + cw[c]]
            j = jpeg(part, restart, sub)
            fn = f'{name}.jpg' if (c, r) == (0, 0) else f'{name}({c},{r}).jpg'
            open(os.path.join(SYN, fn), 'wb').write(j)
            lines.append(f'ImageFile={fn}' if (c, r) == (0, 0) else f'ImageFile({c},{r})={fn}')
            mcu = 8 if sub == 0 else 16
            per_row = cw[c] // mcu // restart
            for s in row_starts(j, per_row)[:ch[r] // mcu]:
                opt += struct.pack('<Q', s) + b'\0' * 32
            x0 += cw[c]
        y0 += ch[r]
    if with_opt:
        open(os.path.join(SYN, f'{name}.opt'), 'wb').write(opt)
        lines.append(f'OptimisationFile={name}.opt')
    # The map: the slide at 1/8 (OpenSlide needs one).
    mw, mh = (w + 7) // 8, (h + 7) // 8
    mp = np.asarray(Image.fromarray(full).resize((mw, mh), Image.BOX))
    open(os.path.join(SYN, f'{name}_map.jpg'), 'wb').write(jpeg(mp, 2))
    lines.append(f'MapFile={name}_map.jpg')
    mac = pattern(120, 40, 2)
    open(os.path.join(SYN, f'{name}_macro.jpg'), 'wb').write(jpeg(mac))
    lines += [f'MacroImage={name}_macro.jpg', 'SourceLens=20.000000',
              f'PhysicalWidth={w * 460}', f'PhysicalHeight={h * 460}',
              'XOffsetFromSlideCentre=1500', 'YOffsetFromSlideCentre=-700', f'Reference={name}']
    open(os.path.join(SYN, f'{name}.vms'), 'w').write('\r\n'.join(lines) + '\r\n')
    return f'vms-synth/{name}.vms'


def ngr_file(path, rgb8, col):
    h, w = rgb8.shape[:2]
    rgb = (rgb8.astype(np.uint16) << 4) | 0x7
    hdr = bytearray(32)
    hdr[0:2] = b'GN'
    struct.pack_into('<III', hdr, 4, w, h, col)
    struct.pack_into('<I', hdr, 24, 32)
    body = bytearray()
    for c0 in range(0, w, col):
        body += rgb[:, c0:c0 + col, :].astype('<u2').tobytes()
    open(path, 'wb').write(bytes(hdr) + bytes(body))


def synth_vmu(name, w, h, col):
    full = pattern(w, h, 1)
    ngr_file(os.path.join(SYN, f'{name}.ngr'), full, col)
    # The map: an NGR of the slide at 1/8.
    mw, mh = w // 8, h // 8
    mp = np.asarray(Image.fromarray(full).resize((mw, mh), Image.BOX))
    ngr_file(os.path.join(SYN, f'{name}_map.ngr'), mp, mw)
    lines = ['[Uncompressed Virtual Microscope Specimen]', 'NoLayers=1', f'ImageFile={name}.ngr',
             f'MapFile={name}_map.ngr',
             'BitsPerPixel=36', 'PixelOrder=RGB', 'SourceLens=10', f'PhysicalWidth={w * 1000}',
             f'PhysicalHeight={h * 1000}']
    open(os.path.join(SYN, f'{name}.vmu'), 'w').write('\r\n'.join(lines) + '\r\n')
    return f'vms-synth/{name}.vmu'


slides = [
    synth_vms('grid', 3, 2, [512, 512, 256], [256, 128], 8, True),
    synth_vms('noopt', 2, 2, [384, 128], [192, 64], 4, False),

    synth_vmu('uncomp', 384, 200, 128),
]

# A VMS of the real CMU-1's bottom row (its (0,1) and (1,1) files, their optimisation runs).
REAL = os.path.join(OUT, 'vms')
CUT = os.path.join(OUT, 'vms-cut')
stem = 'CMU-1-40x - 2010-01-12 13.24.05'
if os.path.isfile(os.path.join(REAL, f'{stem}(1,1).jpg')):
    os.makedirs(CUT, exist_ok=True)
    for src, dst in [(f'{stem}(0,1).jpg', 'cut.jpg'), (f'{stem}(1,1).jpg', 'cut(1,0).jpg'),
                     (f'{stem}_macro.jpg', 'cut_macro.jpg')]:
        d = os.path.join(CUT, dst)
        if not os.path.exists(d):
            os.symlink(os.path.join('..', 'vms', src), d)
    # The map's bottom 1856 rows are the bottom row at 1/8.
    mp = Image.open(os.path.join(REAL, f'{stem}_map2.jpg'))
    mp = np.asarray(mp.crop((0, mp.height - 14848 // 8, mp.width, mp.height)).convert('RGB'))
    open(os.path.join(CUT, 'cut_map.jpg'), 'wb').write(jpeg(mp, 64))
    o = open(os.path.join(REAL, f'{stem}.opt'), 'rb').read()
    recs = [o[k:k + 40] for k in range(0, len(o), 40)]
    vals = [struct.unpack('<Q', r[:8])[0] for r in recs]
    runs, cur = [], []
    for v, r in zip(vals, recs):
        if cur and v <= struct.unpack('<Q', cur[-1][:8])[0]:
            runs.append(cur)
            cur = []
        cur.append(r)
    runs.append(cur)
    open(os.path.join(CUT, 'cut.opt'), 'wb').write(b''.join(runs[2] + runs[3]))
    vms = open(os.path.join(REAL, f'{stem}.vms')).read()
    keep = [l for l in vms.splitlines() if l.split('=')[0] in
            ('SourceLens', 'XOffsetFromSlideCentre', 'YOffsetFromSlideCentre', 'Reference')]
    # The bottom row is 14848 of the slide's 76288 rows and 102400 columns wide.
    lines = ['[Virtual Microscope Specimen]', 'NoLayers=1', 'NoJpegColumns=2', 'NoJpegRows=1',
             'ImageFile=cut.jpg', 'ImageFile(1,0)=cut(1,0).jpg', 'OptimisationFile=cut.opt',
             'MapFile=cut_map.jpg',
             'MacroImage=cut_macro.jpg', 'PhysicalWidth=23367500',
             f'PhysicalHeight={round(17357904 * 14848 / 76288)}'] + keep
    open(os.path.join(CUT, 'cut.vms'), 'w').write('\r\n'.join(lines) + '\r\n')
    slides.append('vms-cut/cut.vms')

for rel in slides:
    s = openslide.OpenSlide(os.path.join(OUT, rel))
    w, h = s.dimensions
    mpp = s.properties.get('openslide.mpp-x', '-')
    obj = s.properties.get('openslide.objective-power', '-')
    rows.append(['slide', rel, str(w), str(h), mpp, obj])
    slug = rel.replace('/', '_').replace(' ', '_')
    for lv in range(s.level_count):
        ds = s.level_downsamples[lv]
        if abs(ds - round(ds)) > 1e-9 or int(round(ds)) & (int(round(ds)) - 1):
            continue
        lw, lh = s.level_dimensions[lv]
        regions = [(0, 0, min(lw, 300), min(lh, 200)),
                   (max(0, lw // 2 - 150), max(0, lh // 2 - 90), min(lw, 300), min(lh, 180)),
                   (max(0, lw - 260), max(0, lh - 140), min(lw, 260), min(lh, 140))]
        for (x, y, rw, rh) in regions:
            ds_i = int(round(ds))
            img = np.asarray(s.read_region((x * ds_i, y * ds_i), lv, (rw, rh)).convert('RGB'))
            rows.append(['region', rel, str(ds_i), str(x), str(y), str(rw), str(rh),
                         raw(f'{slug}_l{lv}_{x}_{y}.rgb', img)])
    for kind, im in s.associated_images.items():
        a = np.asarray(im.convert('RGB'))
        rows.append(['assoc', rel, kind, str(a.shape[1]), str(a.shape[0]), raw(f'{slug}_{kind}.rgb', a)])

with open(os.path.join(OUT, 'expected.tsv'), 'w') as f:
    for r in rows:
        f.write('\t'.join(r) + '\n')
print('wrote', len(rows), 'rows to', os.path.join(OUT, 'expected.tsv'))
