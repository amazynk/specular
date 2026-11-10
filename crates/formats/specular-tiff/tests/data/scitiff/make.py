#!/usr/bin/env python3
"""Writes the scientific TIFF fixtures of this directory and their tifffile oracle.

Each fixture `NAME.EXT` gets `NAME.json`: the shape and axes tifffile reads, the pixel size in
metres, the axis steps, the display ranges, LUTs and labels from the ImageJ metadata, the
instrument pixel size, and every plane's values (null for NaN) keyed by its C/Z/T index. The
specular-decode test `scitiff` compares the session against these files.

The LUT files (`*.lut`) are ImageJ lookup tables in the three forms ImageJ reads (raw 768 bytes,
NIH ICOL 800 bytes, text); `luts.json` holds their tables.

Run with a Python that has numpy and tifffile (the oracle venv):
    target/scitiff/venv/bin/python -I crates/formats/specular-tiff/tests/data/scitiff/make.py
Everything is deterministic; rerunning rewrites identical files.
"""

import json
import os
import struct
import sys

import numpy
import tifffile

OUT = os.path.dirname(os.path.abspath(__file__))
H, W = 12, 20
RNG = numpy.random.default_rng(20261011)

UNIT_METRES = {
    'm': 1.0, 'mm': 1e-3, 'um': 1e-6, 'µm': 1e-6, 'micron': 1e-6, 'microns': 1e-6,
    '\\u00B5m': 1e-6, 'nm': 1e-9, 'cm': 1e-2,
}


def path(name):
    return os.path.join(OUT, name)


def values(plane):
    out = []
    for v in numpy.asarray(plane, dtype=numpy.float64).ravel():
        out.append(None if numpy.isnan(v) else float(numpy.float32(v)))
    return out


def dump(name, oracle):
    with open(path(name), 'w', encoding='utf-8') as f:
        json.dump(oracle, f, separators=(',', ':'), sort_keys=True)
        f.write('\n')


def planes_of(arr, axes):
    """Every plane of `arr` (axes ending in YX, optionally S last) keyed by C/Z/T."""
    lead = [a for a in axes if a not in 'YXS']
    if axes.endswith('S'):
        arr = numpy.moveaxis(arr, -1, 0)
        lead = ['C', *lead]
        axes = 'C' + axes[:-1]
    out = []
    for idx in numpy.ndindex(*arr.shape[: len(lead)]):
        key = {a: int(i) for a, i in zip(lead, idx, strict=True)}
        out.append({'index': key, 'values': values(arr[idx])})
    return out


def ramp(colour):
    t = numpy.arange(256, dtype=numpy.uint16)
    lut = numpy.zeros((3, 256), dtype=numpy.uint8)
    for k in range(3):
        lut[k] = (t * colour[k] // 255).astype(numpy.uint8)
    return lut


# ----- a minimal little-endian TIFF writer for the hand-built formats ------------------------


class Tiff:
    def __init__(self):
        self.buf = bytearray(b'II*\x00\x00\x00\x00\x00')
        self.last_next = 4

    def put(self, blob):
        if len(self.buf) % 2:
            self.buf += b'\x00'
        off = len(self.buf)
        self.buf += blob
        return off

    def ifd(self, entries):
        """`entries`: (tag, type, count, value bytes); values over 4 bytes go out of line."""
        out = []
        for tag, typ, count, data in sorted(entries):
            if len(data) <= 4:
                out.append((tag, typ, count, data.ljust(4, b'\x00')))
            else:
                out.append((tag, typ, count, struct.pack('<I', self.put(data))))
        body = struct.pack('<H', len(out))
        for tag, typ, count, v in out:
            body += struct.pack('<HHI', tag, typ, count) + v
        body += b'\x00\x00\x00\x00'
        off = self.put(body)
        struct.pack_into('<I', self.buf, self.last_next, off)
        self.last_next = off + len(body) - 4
        return off

    def save(self, name):
        with open(path(name), 'wb') as f:
            f.write(self.buf)


def short(tag, *v):
    return (tag, 3, len(v), struct.pack(f'<{len(v)}H', *v))


def long(tag, *v):
    return (tag, 4, len(v), struct.pack(f'<{len(v)}I', *v))


def image_tags(width, height, bits, samples, offsets, counts, planar=1):
    return [
        long(256, width),
        long(257, height),
        short(258, *([bits] * samples)),
        short(259, 1),
        short(262, 1),
        long(273, *offsets),
        short(277, samples),
        long(278, height),
        long(279, *counts),
        short(284, planar),
    ]


# ----- fixtures ------------------------------------------------------------------------------


def hyperstack():
    """ImageJ hyperstack, T2 Z3 C2, 16-bit, composite, µm, LUTs, ranges, labels."""
    t, z, c = 2, 3, 2
    data = RNG.integers(0, 4096, size=(t, z, c, H, W), dtype=numpy.uint16)
    labels = [f't{i}z{j}c{k}' for i in range(t) for j in range(z) for k in range(c)]
    luts = [ramp((255, 0, 0)), ramp((0, 255, 0))]
    tifffile.imwrite(
        path('hyperstack.tif'),
        data,
        imagej=True,
        resolution=(4.0, 4.0),
        metadata={
            'axes': 'TZCYX',
            'spacing': 0.5,
            'unit': 'um',
            'finterval': 2.0,
            'mode': 'composite',
            'LUTs': luts,
            'Ranges': (0.0, 4000.0, 100.0, 3000.0),
            'Labels': labels,
        },
    )
    with tifffile.TiffFile(path('hyperstack.tif')) as f:
        s = f.series[0]
        arr = s.asarray()
        ij = f.imagej_metadata
        xres = f.pages.first.tags['XResolution'].value
        oracle = {
            'shape': list(s.shape),
            'axes': s.axes,
            'pixel': [xres[1] / xres[0] * 1e-6] * 2,
            'unit': 'µm',
            'z_step': ij['spacing'],
            't_step': ij['finterval'],
            'ranges': [list(ij['Ranges'][i : i + 2]) for i in (0, 2)],
            'luts': [numpy.asarray(l).T.tolist() for l in ij['LUTs']],
            'labels': ij['Labels'],
            'channel_axis': True,
            'planes': planes_of(arr, s.axes),
        }
    dump('hyperstack.json', oracle)


def signed_cf():
    """ImageJ 16-bit signed: stored unsigned with a linear calibration c0 = -32768."""
    stored = RNG.integers(30000, 36000, size=(H, W), dtype=numpy.uint16)
    desc = (
        'ImageJ=1.54f\nimages=1\nunit=micron\ncf=0\nc0=-32768.0\nc1=1.0\n'
        'vunit=Gray Value\nmin=31768.0\nmax=34768.0\n'
    )
    tifffile.imwrite(
        path('signed.tif'), stored, description=desc, metadata=None, resolution=(2.0, 2.0)
    )
    with tifffile.TiffFile(path('signed.tif')) as f:
        ij = f.imagej_metadata
        raw = f.pages.first.asarray().astype(numpy.float64)
        # ImageJ's calibration function 0 (straight line): c0 + c1·x.
        val = ij['c0'] + ij['c1'] * raw
        xres = f.pages.first.tags['XResolution'].value
        oracle = {
            'shape': [H, W],
            'axes': 'YX',
            'pixel': [xres[1] / xres[0] * 1e-6] * 2,
            'unit': 'µm',
            'vunit': ij['vunit'],
            'ranges': [[ij['c0'] + ij['min'], ij['c0'] + ij['max']]],
            'planes': [{'index': {}, 'values': values(val)}],
        }
    dump('signed.json', oracle)


def float32():
    """Plain float32 with a NaN and a resolution of 10 000 pixels per centimetre."""
    a = RNG.normal(0.0, 1.0, size=(H, W)).astype(numpy.float32)
    a[3, 4] = numpy.nan
    tifffile.imwrite(
        path('float.tif'), a, metadata=None, resolution=(10000.0, 10000.0),
        resolutionunit='CENTIMETER',
    )
    with tifffile.TiffFile(path('float.tif')) as f:
        p = f.pages.first
        xres = p.tags['XResolution'].value
        oracle = {
            'shape': list(p.shape),
            'axes': p.axes,
            'pixel': [xres[1] / xres[0] * 1e-2] * 2,
            'unit': 'µm',
            'planes': [{'index': {}, 'values': values(p.asarray())}],
        }
    dump('float.json', oracle)


def multiband():
    """Five 16-bit samples per pixel, contiguous, MinIsBlack."""
    a = RNG.integers(0, 65535, size=(H, W, 5), dtype=numpy.uint16)
    tifffile.imwrite(
        path('multiband.tif'), a, photometric='minisblack', planarconfig='contig',
        metadata=None,
    )
    with tifffile.TiffFile(path('multiband.tif')) as f:
        s = f.series[0]
        oracle = {
            'shape': list(s.shape),
            'axes': s.axes,
            'pixel': None,
            'planes': planes_of(s.asarray(), s.axes),
        }
    dump('multiband.json', oracle)


def lsm():
    """Zeiss LSM, Z3 C2 T2, 16-bit planar, with thumbnails, colours, names and time stamps."""
    z, c, t = 3, 2, 2
    data = RNG.integers(0, 4096, size=(t, z, c, H, W), dtype=numpy.uint16)
    f = Tiff()
    colours = b''.join(bytes(v) for v in ((255, 0, 0, 0), (0, 255, 0, 0)))
    names = b''.join(struct.pack('<I', len(n) + 1) + n + b'\x00' for n in (b'Ch1-T1', b'Ch2-T2'))
    cc_head = 24
    cc = struct.pack('<IIIIII', cc_head + len(colours) + len(names), 2, 2, cc_head,
                     cc_head + len(colours), 0) + colours + names
    cc_off = f.put(cc)
    stamps = [100.0, 101.5]
    ts_off = f.put(struct.pack('<ii', 8 + 8 * t, t) + struct.pack(f'<{t}d', *stamps))
    info = bytearray(224)
    struct.pack_into('<IiiiiiiiII', info, 0, 0x0400494C, len(info), W, H, z, c, t, 2, 4, 3)
    struct.pack_into('<ddd', info, 40, 0.2e-6, 0.2e-6, 1.0e-6)
    struct.pack_into('<HHI', info, 88, 6, 0, 0)
    struct.pack_into('<I', info, 108, cc_off)
    struct.pack_into('<d', info, 112, 1.5)
    struct.pack_into('<I', info, 132, ts_off)
    for ti in range(t):
        for zi in range(z):
            offs, counts = [], []
            for ci in range(c):
                blob = data[ti, zi, ci].astype('<u2').tobytes()
                offs.append(f.put(blob))
                counts.append(len(blob))
            tags = image_tags(W, H, 16, c, offs, counts, planar=2)
            # As LSM writers do: two BitsPerSample values out of line.
            tags[2] = (258, 3, c, struct.pack('<I', f.put(struct.pack(f'<{c}H', *[16] * c))))
            tags.append(long(254, 0))
            if ti == 0 and zi == 0:
                tags.append((34412, 1, len(info), bytes(info)))
            f.ifd(tags)
            thumb = numpy.zeros((3, 4), dtype=numpy.uint8).tobytes()
            toff = f.put(thumb)
            ttags = image_tags(4, 3, 8, 1, [toff], [len(thumb)])
            ttags.append(long(254, 1))
            f.ifd(ttags)
    f.save('stack.lsm')
    with tifffile.TiffFile(path('stack.lsm')) as tf:
        s = tf.series[0]
        m = tf.lsm_metadata
        oracle = {
            'shape': list(s.shape),
            'axes': s.axes,
            'pixel': [m['VoxelSizeX'], m['VoxelSizeY']],
            'unit': 'µm',
            'z_step': m['VoxelSizeZ'] * 1e6,
            't_step': float(m['TimeStamps'][1] - m['TimeStamps'][0]),
            'labels': m['ChannelColors']['ColorNames'],
            'colours': [col[:3] for col in m['ChannelColors']['Colors']],
            'channel_axis': True,
            'planes': planes_of(s.asarray(), s.axes),
        }
    dump('stack.json', oracle)


def stk():
    """MetaMorph STK, 4 planes 0.25 µm apart, calibrated 0.1 µm, all in one strip run."""
    n = 4
    data = RNG.integers(0, 65535, size=(n, H, W), dtype=numpy.uint16)
    f = Tiff()
    pix = f.put(data.astype('<u2').tobytes())
    plane = H * W * 2
    uic2 = b''.join(struct.pack('<6I', 1, 4, 2460000, 1000 * i, 2460000, 1000 * i)
                    for i in range(n))
    uic3 = b''.join(struct.pack('<2I', 520, 1) for _ in range(n))
    xcal = f.put(struct.pack('<2I', 1, 10))
    ycal = f.put(struct.pack('<2I', 1, 10))
    units = f.put(struct.pack('<I', 3) + b'um\x00')
    name = f.put(struct.pack('<I', 6) + b'stack\x00')
    uic1 = struct.pack('<10I', 3, 1, 4, xcal, 5, ycal, 6, units, 7, name)
    tags = image_tags(W, H, 16, 1, [pix], [plane])
    tags += [(33628, 4, 5, uic1), (33629, 5, n, uic2), (33630, 5, n, uic3)]
    f.ifd(tags)
    f.save('stack.stk')
    with tifffile.TiffFile(path('stack.stk')) as tf:
        s = tf.series[0]
        m = tf.stk_metadata
        scale = UNIT_METRES[m['CalibrationUnits']]
        oracle = {
            'shape': list(s.shape),
            'axes': s.axes,
            'pixel': [m['XCalibration'] * scale, m['YCalibration'] * scale],
            'unit': 'µm',
            'z_step': float(s.coords['Z'][1] - s.coords['Z'][0]) if s.coords else None,
            'title': m['Name'],
            'planes': planes_of(s.asarray(), s.axes),
        }
    dump('stack-stk.json', oracle)


def fluoview():
    """Olympus FluoView, C2 Z3 16-bit, one page per plane, MM_Header on the first."""
    c, z = 2, 3
    data = RNG.integers(0, 4096, size=(z, c, H, W), dtype=numpy.uint16)
    dims = [(b'X', W, 0.0, 0.125, b'um'), (b'Y', H, 0.0, 0.125, b'um'),
            (b'Ch', c, 0.0, 1.0, b''), (b'Z', z, 2.0, 0.5, b'um')]
    head = bytearray(1454)
    struct.pack_into('<hB', head, 0, 1, 1)
    head[3:3 + 9] = b'fluoview\x00'
    for k in range(10):
        name, size, origin, step, unit = dims[k] if k < len(dims) else (b'', 1, 0.0, 0.0, b'')
        at = 284 + 100 * k
        head[at:at + len(name)] = name
        struct.pack_into('<idd', head, at + 16, size, origin, step)
        head[at + 36:at + 36 + len(unit)] = unit
    f = Tiff()
    stamp = struct.pack('<8d', *range(8))
    for zi in range(z):
        for ci in range(c):
            blob = data[zi, ci].astype('<u2').tobytes()
            off = f.put(blob)
            tags = image_tags(W, H, 16, 1, [off], [len(blob)])
            tags.append((34362, 12, 8, stamp))
            if zi == 0 and ci == 0:
                tags.append((34361, 1, len(head), bytes(head)))
            f.ifd(tags)
    f.save('fluoview.tif')
    with tifffile.TiffFile(path('fluoview.tif')) as tf:
        s = tf.series[0]
        m = tf.fluoview_metadata
        d = {n: (o, st, u) for n, _, o, st, u in m['Dimensions']}
        oracle = {
            'shape': [n for n in s.shape],
            'axes': s.axes,
            'pixel': [d['X'][1] * UNIT_METRES[d['X'][2]], d['Y'][1] * UNIT_METRES[d['Y'][2]]],
            'unit': 'µm',
            'z_step': d['Z'][1] * UNIT_METRES[d['Z'][2]] * 1e6,
            'title': m['ImageName'],
            'channel_axis': True,
            'planes': planes_of(s.asarray(), s.axes),
        }
    dump('fluoview.json', oracle)


def gel():
    """Molecular Dynamics GEL: square-root coded 16-bit with a scale of 1/21025."""
    stored = RNG.integers(0, 2000, size=(H, W), dtype=numpy.uint16)
    tifffile.imwrite(
        path('scan.gel'),
        stored,
        metadata=None,
        extratags=[
            (33445, 'I', 1, 2, True),
            (33446, '2I', 1, (1, 21025), True),
            (33448, 's', 0, 'Lab', True),
            (33449, 's', 0, 'Western blot', True),
            (33452, 's', 0, 'Counts', True),
        ],
    )
    with tifffile.TiffFile(path('scan.gel')) as f:
        s = f.series[0]
        oracle = {
            'shape': list(s.shape),
            'axes': s.axes,
            'pixel': None,
            'vunit': f.mdgel_metadata['FileUnits'],
            'title': f.mdgel_metadata['SampleInfo'],
            'planes': [{'index': {}, 'values': values(s.asarray())}],
        }
    dump('scan.json', oracle)


ZEISS = (
    '\r\n'.join([
        '0', '0',
        'AP_PIXEL_SIZE', 'Image Pixel Size = 2.5 nm',
        'AP_ACTUALKV', 'EHT = 5.00 kV',
        'AP_MAG', 'Mag = 50.00 K X',
        'AP_WD', 'WD = 4.9 mm',
        'DP_DETECTOR_CHANNEL', 'Signal A = InLens',
        'DP_COLUMN_TYPE', 'Column Type = GEMINI',
        'AP_DATE', 'Date :11 Oct 2026',
        'AP_TIME', 'Time :10:20:30',
    ])
    + '\r\n'
)

FEI = '\r\n'.join([
    '[User]', 'Date=10/11/2026', 'Time=10:20:30 AM',
    '[System]', 'SystemType=Helios G4',
    '[Beam]', 'HV=5000', 'Beam=EBeam',
    '[EBeam]', 'HV=5000', 'WD=0.004',
    '[Scan]', 'PixelWidth=1.5e-09', 'PixelHeight=1.5e-09',
    '[Detectors]', 'Name=TLD', 'Mode=SE',
    '[Stage]', 'WorkingDistance=0.004',
]) + '\r\n'

TESCAN = '\r\n'.join([
    '[MAIN]', 'Device=MIRA3', 'Date=2026-10-11', 'Time=10:20:30', 'HV=10000.0',
    'PixelSizeX=3.0e-08', 'PixelSizeY=3.0e-08', 'Magnification=5000', 'WD=0.01',
    'Detector=SE',
]) + '\r\n'


def sem():
    """SEM images: Zeiss 8-bit (a picture), FEI 16-bit (science), TESCAN 8-bit (a picture)."""
    a8 = RNG.integers(0, 255, size=(H, W), dtype=numpy.uint8)
    a16 = RNG.integers(0, 65535, size=(H, W), dtype=numpy.uint16)
    tifffile.imwrite(path('zeiss.tif'), a8, metadata=None,
                     extratags=[(34118, 's', 0, ZEISS, True)])
    tifffile.imwrite(path('fei.tif'), a16, metadata=None,
                     extratags=[(34682, 's', 0, FEI, True)])
    tifffile.imwrite(path('tescan.tif'), a8, metadata=None,
                     extratags=[(50431, 's', 0, TESCAN, True)])
    with tifffile.TiffFile(path('zeiss.tif')) as f:
        m = f.sem_metadata
        v, unit = m['ap_pixel_size'][1], m['ap_pixel_size'][2]
        kv = m['ap_actualkv'][1] * 1000.0
        dump('zeiss.json', {
            'shape': [H, W], 'axes': 'YX', 'picture': True,
            'pixel': [v * UNIT_METRES[unit]] * 2, 'voltage': kv, 'vendor': 'Zeiss',
            'planes': [{'index': {}, 'values': values(f.pages.first.asarray())}],
        })
    with tifffile.TiffFile(path('fei.tif')) as f:
        m = f.fei_metadata
        dump('fei.json', {
            'shape': [H, W], 'axes': 'YX',
            'pixel': [m['Scan']['PixelWidth'], m['Scan']['PixelHeight']],
            'voltage': float(m['Beam']['HV']), 'vendor': 'Thermo Fisher (FEI)',
            'planes': [{'index': {}, 'values': values(f.pages.first.asarray())}],
        })
    with tifffile.TiffFile(path('tescan.tif')) as f:
        # tifffile has no TESCAN parser: the tag's INI text read by hand.
        text = f.pages.first.tags[50431].value
        kv = dict(l.split('=', 1) for l in text.splitlines() if '=' in l)
        dump('tescan.json', {
            'shape': [H, W], 'axes': 'YX', 'picture': True,
            'pixel': [float(kv['PixelSizeX']), float(kv['PixelSizeY'])],
            'voltage': float(kv['HV']), 'vendor': 'TESCAN',
            'planes': [{'index': {}, 'values': values(f.pages.first.asarray())}],
        })


HITACHI = '\r\n'.join([
    '[SemImageFile]', 'InstructName=SU8230', 'SerialNumber=1234', 'DataNumber=1',
    'SampleName=grid', 'Format=tif', 'ImageName=hitachi.tif', 'Date=2026/10/11',
    'Time=10:20:30', 'AcceleratingVoltage=3000 Volt', 'Magnification=100000',
    'WorkingDistance=8000 um', 'SignalName=SE(U)', 'PixelSize=1.27',
]) + '\r\n'


def hitachi():
    """An 8-bit Hitachi SEM image with its `.txt` sidecar (pixel size in nanometres)."""
    a8 = RNG.integers(0, 255, size=(H, W), dtype=numpy.uint8)
    tifffile.imwrite(path('hitachi.tif'), a8, metadata=None)
    with open(path('hitachi.txt'), 'w', encoding='ascii', newline='') as f:
        f.write(HITACHI)
    # tifffile does not read sidecars: the INI text by hand.
    kv = dict(l.split('=', 1) for l in HITACHI.splitlines() if '=' in l)
    dump('hitachi.json', {
        'shape': [H, W], 'axes': 'YX', 'picture': True,
        'pixel': [float(kv['PixelSize']) * 1e-9] * 2, 'voltage': 3000.0, 'vendor': 'Hitachi',
        'planes': [{'index': {}, 'values': values(a8)}],
    })


def luts():
    """ImageJ LUT files: raw 768, NIH ICOL 800 and text."""
    i = numpy.arange(256)
    table = numpy.stack([i, (i * 7) % 256, 255 - i]).astype(numpy.uint8)
    with open(path('raw.lut'), 'wb') as f:
        f.write(table.tobytes())
    head = bytearray(32)
    head[0:4] = b'ICOL'
    struct.pack_into('>HHHH', head, 4, 1, 256, 0, 255)
    with open(path('icol.lut'), 'wb') as f:
        f.write(bytes(head) + table[:, ::-1].tobytes())
    with open(path('text.lut'), 'w', encoding='ascii') as f:
        f.write('Index\tRed\tGreen\tBlue\n')
        for k in range(256):
            f.write(f'{k}\t{table[0, 255 - k]}\t{table[1, k]}\t{table[2, k]}\n')
    dump('luts.json', {
        'raw.lut': table.T.tolist(),
        'icol.lut': table[:, ::-1].T.tolist(),
        'text.lut': numpy.stack([table[0, ::-1], table[1], table[2]]).T.tolist(),
    })


def banded():
    """ImageJ composite whose channels are the samples of each page (16-bit RGB), Z2."""
    z, c = 2, 3
    data = RNG.integers(0, 65535, size=(z, H, W, c), dtype=numpy.uint16)
    desc = (
        f'ImageJ=1.54f\nimages={z * c}\nchannels={c}\nslices={z}\nhyperstack=true\n'
        'mode=composite\nunit=micron\nspacing=1.5\n'
    )
    tifffile.imwrite(
        path('banded.tif'), data, photometric='rgb', planarconfig='contig',
        description=desc, metadata=None, resolution=(5.0, 5.0),
    )
    with tifffile.TiffFile(path('banded.tif')) as f:
        # tifffile's ImageJ series wants images = pages; read the pages themselves.
        arr = numpy.stack([p.asarray() for p in f.pages])
        xres = f.pages.first.tags['XResolution'].value
        ij = f.imagej_metadata
        oracle = {
            'shape': [z, H, W, c],
            'axes': 'ZYXS',
            'pixel': [xres[1] / xres[0] * 1e-6] * 2,
            'unit': 'µm',
            'z_step': ij['spacing'],
            'channel_axis': True,
            'planes': planes_of(arr, 'ZYXS'),
        }
    dump('banded.json', oracle)


def old_composite():
    """Old ImageJ composite: three separate 16-bit planes back to back under one strip."""
    c = 3
    data = RNG.integers(0, 4096, size=(c, H, W), dtype=numpy.uint16)
    desc = b'ImageJ=1.38n\nchannels=3\nmin=33.0\nmax=3931.0\x00'
    t = Tiff()
    off = t.put(data.tobytes())
    tags = image_tags(W, H, 16, c, [off], [data.nbytes], planar=2)
    tags.append((270, 2, len(desc), desc))
    t.ifd(tags)
    t.save('composite-old.tif')
    with tifffile.TiffFile(path('composite-old.tif')) as f:
        s = f.series[0]
        dump('composite-old.json', {
            'shape': list(s.shape),
            'axes': s.axes,
            'pixel': None,
            'planes': planes_of(s.asarray(), s.axes),
        })


def main():
    if len(sys.argv) > 1:
        sys.exit('usage: make.py (writes next to itself)')
    hyperstack()
    signed_cf()
    float32()
    multiband()
    lsm()
    stk()
    gel()
    sem()
    hitachi()
    luts()
    banded()
    old_composite()
    fluoview()


if __name__ == '__main__':
    main()
