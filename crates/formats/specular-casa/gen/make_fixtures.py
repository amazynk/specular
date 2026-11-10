"""Writes the CASA image fixtures in tests/data with python-casacore.

    python gen/make_fixtures.py tests/data

Big-endian tables need casacore's `table.endianformat: big` setting, read once per
process, so the script reruns itself for those with CASARCFILES pointing at a settings
file it writes next to the fixtures (and removes).
"""
import math
import os
import shutil
import subprocess
import sys

import numpy as np
import casacore.images as ci
import casacore.tables as ct

OUT = os.path.abspath(sys.argv[1])
BIG = len(sys.argv) > 2 and sys.argv[2] == '--big'


def fresh(name):
    path = os.path.join(OUT, name)
    shutil.rmtree(path, ignore_errors=True)
    return path


def tclean_coords(cs, ra_deg, dec_deg, cell_arcsec, rot_deg, freq, dfreq, rest,
                  stokes=None, system='J2000', projection='SIN', telescope='ALMA',
                  observer='Ada Lovelace', mjd=59000.25):
    """Rewrites a default coordinate record the way tclean's look: radians, a rotated pc,
    a spectral axis with a rest frequency."""
    d = cs['direction0']
    d['system'] = system
    d['projection'] = projection
    d['units'] = ['rad', 'rad']
    d['crval'] = np.array([math.radians(ra_deg), math.radians(dec_deg)])
    c = math.radians(cell_arcsec / 3600.0)
    d['cdelt'] = np.array([-c, c])
    d['crpix'] = np.array([d['crpix'][0] + 0.5, d['crpix'][1] - 1.25])
    r = math.radians(rot_deg)
    d['pc'] = np.array([[math.cos(r), -math.sin(r)], [math.sin(r), math.cos(r)]])
    d['conversionSystem'] = system
    if projection == 'SIN':
        d['projection_parameters'] = np.array([0.0, 0.0])
    cs['telescope'] = telescope
    cs['observer'] = observer
    cs['obsdate'] = {'type': 'epoch', 'refer': 'UTC', 'm0': {'value': mjd, 'unit': 'd'}}
    for k in list(cs):
        if k.startswith('stokes') and stokes is not None:
            cs[k]['stokes'] = stokes
        if k.startswith('spectral'):
            s = cs[k]
            s['system'] = 'LSRK'
            s['restfreq'] = rest
            s['restfreqs'] = np.array([rest])
            s['wcs']['crval'] = freq
            s['wcs']['cdelt'] = dfreq
            s['wcs']['crpix'] = 1.0
    return cs


def beam(major, minor, pa, minor_unit='arcsec'):
    return {'major': {'value': major, 'unit': 'arcsec'},
            'minor': {'value': minor, 'unit': minor_unit},
            'positionangle': {'value': pa, 'unit': 'deg'}}


def finish(path, coords=None, info=None, units=None):
    t = ct.table(path, readonly=False, ack=False)
    if coords is not None:
        cs = t.getkeyword('coords')
        t.putkeyword('coords', coords(cs))
    if info is not None:
        t.putkeyword('imageinfo', info)
    if units is not None:
        t.putkeyword('units', units)
    t.close()


def cube(nra, ndec, nstokes, nfreq, dtype, seed):
    rng = np.random.default_rng(seed)
    shape = (nfreq, nstokes, ndec, nra)  # numpy order: casacore's reversed
    a = rng.normal(size=shape)
    a += np.arange(nra)[None, None, None, :] * 0.25
    if np.issubdtype(dtype, np.complexfloating):
        a = a + 1j * rng.normal(size=shape)
    return a.astype(dtype)


def mask_for(a, seed):
    rng = np.random.default_rng(seed)
    m = rng.random(a.shape) < 0.15
    m[..., 0, 0] = True
    return m


def shape_image(path, a, tile, units, coords):
    """A one-row image table stored by a TiledShapeStMan (the row is the cube's last axis)."""
    src = path + '.src'
    shutil.rmtree(src, ignore_errors=True)
    im = ci.image(src, values=a)
    del im
    s = ct.table(src, ack=False)
    kw = s.getkeywords()
    s.close()
    vt = {np.dtype('float32'): 'float', np.dtype('float64'): 'double'}[a.dtype]
    td = ct.maketabdesc([ct.makearrcoldesc('map', 0.0, ndim=a.ndim, valuetype=vt,
                                           datamanagertype='TiledShapeStMan',
                                           datamanagergroup='map')])
    ct.tabledefinehypercolumn(td, 'map', a.ndim + 1, ['map'])
    dm = {'*1': {'TYPE': 'TiledShapeStMan', 'NAME': 'map',
                 'SPEC': {'DEFAULTTILESHAPE': np.array(list(tile) + [1], dtype=np.int32)},
                 'COLUMNS': ['map']}}
    t = ct.table(path, td, nrow=1, dminfo=dm, ack=False)
    t.putcell('map', 0, a)
    shutil.move(os.path.join(src, 'logtable'), os.path.join(path, 'logtable'))
    shutil.rmtree(src)
    kw['coords'] = coords(kw['coords'])
    kw['units'] = units
    t.putkeywords(kw)
    t.putinfo({'type': 'Image', 'subType': ''})
    t.close()


def little():
    # A 4-D float cube, masked: RA × Dec × Stokes (I, V) × frequency, tiles that do not
    # divide the shape.
    p = fresh('cube_le.image')
    a = cube(13, 11, 2, 5, np.float32, 1)
    im = ci.image(p, values=np.ma.masked_array(a, mask_for(a, 2)), tileshape=(2, 1, 4, 5))
    del im
    finish(p, lambda cs: tclean_coords(cs, 83.82, -5.39, 0.5, 20.0, 230.5e9, -1.0e6,
                                       230.538e9, stokes=['I', 'V']),
           {'imagetype': 'Intensity', 'objectname': 'Orion KL',
            'restoringbeam': beam(1.25, 0.75, -35.0)}, 'Jy/beam')

    # Complex, masked, three Stokes in a linear-feed basis.
    p = fresh('cplx.image')
    a = cube(6, 5, 3, 2, np.complex64, 3)
    im = ci.image(p, values=np.ma.masked_array(a, mask_for(a, 4)), tileshape=(1, 2, 2, 4))
    del im
    finish(p, lambda cs: tclean_coords(cs, 201.365, -43.019, 2.0, 0.0, 1.4e9, 2.0e6,
                                       1420405751.786, stokes=['XX', 'YY', 'XY'],
                                       telescope='MeerKAT', observer='', mjd=58000.0),
           None, 'Jy/beam')

    # Double-precision complex.
    p = fresh('dcplx.image')
    a = cube(4, 3, 1, 2, np.complex128, 5)
    im = ci.image(p, values=a)
    del im
    finish(p, None, None, 'Jy')

    # A 2-D residual, galactic, TAN.
    p = fresh('flat.residual')
    rng = np.random.default_rng(6)
    a = rng.normal(size=(9, 7)).astype(np.float32)
    im = ci.image(p, values=a, tileshape=(4, 3))
    del im

    def gal(cs):
        d = cs['direction0']
        d['system'] = 'GALACTIC'
        d['projection'] = 'TAN'
        d['projection_parameters'] = np.array([], dtype=np.float64)
        d['units'] = ['deg', 'deg']
        d['crval'] = np.array([120.5, -2.25])
        d['cdelt'] = np.array([-0.01, 0.01])
        d['crpix'] = np.array([3.0, 4.0])
        d['conversionSystem'] = 'GALACTIC'
        cs['telescope'] = 'VLA'
        return cs
    finish(p, gal, {'imagetype': 'Intensity', 'objectname': '',
                    'restoringbeam': beam(45.0, 0.6, 10.0, 'arcmin')}, 'Jy/beam')

    # A TiledShapeStMan model.
    p = fresh('shape_le.model')
    a = cube(7, 6, 1, 4, np.float32, 7)
    shape_image(p, a, (3, 2, 1, 2), 'Jy/pixel',
                lambda cs: tclean_coords(cs, 10.0, 41.0, 1.5, -10.0, 100e9, 5e6, 0.0,
                                         stokes=['I'], system='ICRS', mjd=0.0))


def big():
    # A big-endian double cube with per-plane beams.
    p = fresh('cube_be.image')
    a = cube(9, 8, 2, 3, np.float64, 8)
    im = ci.image(p, values=a, tileshape=(3, 2, 3, 4))
    del im
    pb = {'nChannels': 3, 'nStokes': 2}
    k = 0
    for s in range(2):
        for c in range(3):
            pb['*%d' % k] = beam(1.0 + 0.5 * k, 0.5 + 0.25 * k, 5.0 * k)
            k += 1
    finish(p, lambda cs: tclean_coords(cs, 266.4, -29.0, 0.1, 0.0, 345.796e9, 0.5e6,
                                       345.79599e9, stokes=['RR', 'LL'], system='B1950',
                                       projection='SIN'),
           {'imagetype': 'Intensity', 'objectname': 'Sgr A*', 'perplanebeams': pb},
           'Jy/beam')

    # A big-endian, masked TiledShapeStMan image.
    p = fresh('shape_be.pb')
    a = cube(5, 9, 1, 3, np.float32, 9)
    shape_image(p, a, (2, 1, 4, 5), '',
                lambda cs: tclean_coords(cs, 150.1, 2.2, 3.0, 45.0, 3.0e9, -16e6, 0.0,
                                         stokes=['I']))

    # Big-endian with a mask (bits are bits whatever the byte order).
    p = fresh('masked_be.image')
    a = cube(10, 6, 1, 2, np.float32, 10)
    im = ci.image(p, values=np.ma.masked_array(a, mask_for(a, 11)), tileshape=(1, 1, 4, 3))
    del im
    finish(p, None, None, 'K')


if BIG:
    big()
else:
    os.makedirs(OUT, exist_ok=True)
    little()
    rc = os.path.join(OUT, 'casarc')
    with open(rc, 'w') as f:
        f.write('table.endianformat: big\n')
    env = dict(os.environ, CASARCFILES=rc)
    subprocess.run([sys.executable, os.path.abspath(__file__), OUT, '--big'], env=env,
                   check=True)
    os.remove(rc)
