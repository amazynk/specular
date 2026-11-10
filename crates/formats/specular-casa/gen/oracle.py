"""python-casacore's reading of CASA images, as rows for tests/oracle.rs.

    python gen/oracle.py DIR [DIR...] > expected.tsv

Every image directory under each DIR (or DIR itself) gives, all in casacore axis order
(first axis fastest) and 0-based pixels:

    img    NAME  DTYPE  SHAPE(a,b,..)  HASMASK
    plane  NAME  INDEX  SUM_RE  SUM_IM  VALID     (masked pixels left out)
    val    NAME  PIXEL  RE  IM                    (masked: nan)
    world  NAME  PIXEL  W0,W1,..                  (degrees, Hz, Stokes codes)
    beam   NAME  CHAN  STOKES  MAJOR  MINOR  PA   (arcsec, arcsec, degrees)
    meta   NAME  KEY  VALUE
    error  NAME  MESSAGE                          (python-casacore refused it)
"""
import datetime
import itertools
import math
import os
import sys

import numpy as np
import casacore.images as ci

DEG = {'deg': 1.0, 'rad': 180.0 / math.pi, "'": 1.0 / 60, 'arcmin': 1.0 / 60,
       '"': 1.0 / 3600, 'arcsec': 1.0 / 3600}
HZ = {'Hz': 1.0, 'kHz': 1e3, 'MHz': 1e6, 'GHz': 1e9}
MAX_PLANES = 64


def out(*f):
    print('\t'.join(str(x) for x in f))


def num(x):
    x = float(x)
    if math.isnan(x):
        return 'nan'
    return repr(x)


def images(root):
    if os.path.exists(os.path.join(root, 'table.dat')):
        yield root
        return
    for d in sorted(os.listdir(root)):
        p = os.path.join(root, d)
        if os.path.exists(os.path.join(p, 'table.dat')):
            yield p


def axis_scales(im):
    """Per casacore axis: the factor to degrees / Hz of python-casacore's world values."""
    cs = im.coordinates()
    d = cs.dict()
    n = len(im.shape())
    scale = [1.0] * n
    for key, rec in d.items():
        if not isinstance(rec, dict):
            continue
        idx = key.rstrip('0123456789')
        num_ = key[len(idx):]
        if idx not in ('direction', 'spectral', 'stokes', 'linear', 'tabular') or not num_:
            continue
        pmap = list(d['pixelmap' + num_])
        if idx == 'direction':
            for k, p in enumerate(pmap):
                scale[p] = DEG.get(rec['units'][k], 1.0)
        elif idx == 'spectral':
            scale[pmap[0]] = HZ.get(rec.get('unit', 'Hz'), 1.0)
    return scale


def run(path):
    name = os.path.basename(path.rstrip('/'))
    try:
        im = ci.image(path)
    except Exception as e:
        out('error', name, str(e).splitlines()[0] if str(e) else 'error')
        return
    shape = list(reversed(im.shape()))
    data = im.getdata()
    mask = im.getmask()
    has_mask = bool(mask.any())
    out('img', name, im.datatype(), ','.join(map(str, shape)), int(has_mask))
    # numpy order -> casacore order
    data = np.transpose(data)
    mask = np.transpose(mask)
    cplx = np.iscomplexobj(data)
    rest = shape[2:]
    for k, idx in enumerate(itertools.product(*[range(n) for n in rest])):
        if k >= MAX_PLANES:
            break
        sl = (slice(None), slice(None)) + idx if len(shape) > 1 else (slice(None),)
        p = data[sl].astype(np.complex128 if cplx else np.float64)
        m = mask[sl]
        valid = ~m
        out('plane', name, ','.join(map(str, idx)), num(np.real(p[valid]).sum()),
            num(np.imag(p[valid]).sum() if cplx else 0.0), int(valid.sum()))
        rng = np.random.default_rng(k)
        picks = [(0, 0), (shape[0] - 1, shape[1] - 1)] + [
            (int(rng.integers(shape[0])), int(rng.integers(shape[1]))) for _ in range(4)]
        for (x, y) in picks:
            at = (x, y) + idx
            v = data[at]
            if mask[at]:
                re, imv = 'nan', 'nan'
            else:
                re, imv = num(np.real(v)), num(np.imag(v) if cplx else 0.0)
            out('val', name, ','.join(map(str, at)), re, imv)
    scale = axis_scales(im)
    rng = np.random.default_rng(99)
    corners = [[0] * len(shape), [n - 1 for n in shape]]
    for _ in range(6):
        corners.append([float(rng.uniform(-0.5, n - 0.5)) for n in shape])
    for p in corners:
        w = im.toworld(list(reversed(p)))
        w = list(reversed(w))
        w = [x * s for x, s in zip(w, scale)]
        out('world', name, ','.join(num(x) for x in p), ','.join(num(x) for x in w))
    info = im.imageinfo()
    arcsec = {'arcsec': 1.0, 'arcmin': 60.0, 'deg': 3600.0, 'rad': 180 / math.pi * 3600}
    degs = {'deg': 1.0, 'rad': 180 / math.pi}

    def beam(b):
        return (b['major']['value'] * arcsec[b['major']['unit']],
                b['minor']['value'] * arcsec[b['minor']['unit']],
                b['positionangle']['value'] * degs[b['positionangle']['unit']])
    if 'perplanebeams' in info:
        pb = info['perplanebeams']
        nc, ns = pb['nChannels'], pb['nStokes']
        for s in range(ns):
            for c in range(nc):
                b = beam(pb['*%d' % (c + s * nc)])
                out('beam', name, c, s, num(b[0]), num(b[1]), num(b[2]))
    elif 'restoringbeam' in info and info['restoringbeam']:
        b = beam(info['restoringbeam'])
        out('beam', name, -1, -1, num(b[0]), num(b[1]), num(b[2]))
    cs = im.coordinates()
    d = cs.dict()
    if d.get('telescope'):
        out('meta', name, 'telescope', d['telescope'])
    if d.get('observer'):
        out('meta', name, 'observer', d['observer'])
    mjd = d.get('obsdate', {}).get('m0', {}).get('value', 0.0)
    if mjd:
        t = datetime.datetime(1858, 11, 17) + datetime.timedelta(days=mjd)
        out('meta', name, 'date', t.strftime('%Y-%m-%dT%H:%M:%S.') + '%03d' % (t.microsecond // 1000))
    if im.unit():
        out('meta', name, 'units', im.unit())
    if info.get('objectname'):
        out('meta', name, 'object', info['objectname'])


print('# python-casacore oracle; columns per gen/oracle.py')
for root in sys.argv[1:]:
    for p in images(root):
        run(p)
