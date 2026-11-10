"""Writes the synthetic Bruker experiments under tests/data (deterministic; numpy only).

Usage: python -I gen_fixtures.py [OUT]   (OUT defaults to tests/data beside this script)

  fid1d/        1H, int32 little-endian FID: three Lorentzian lines (7.26, 3.70, 1.20 ppm)
                delayed by a digital filter (GRPDLY 67.9856) with a 40° phase error; pdata/1
                holds its 1r processed here independently (window, zero fill, transform,
                delay and phase removed exactly), NC_proc -3, and a peaklist.xml
  fid1d_f64_be/ the same FID as big-endian float64 (DTYPA 2, BYTORDA 1), no pdata
  old_xwin/     13C, XWIN-NMR style: DSPFVS 10, DECIM 4 (66.625 points from the table), no
                GRPDLY, big-endian int32; lines at 77.0 and 128.5 ppm
  hsqc2d/       acqus, acqu2s and a small ser (8 FIDs of 256 complex points), pdata/1 with a 32 × 64 2rr in 8 × 16 submatrices whose values say
                where they belong (row · 1000 + column)
  varian/       a VnmrJ folder (procpar and fid): refused
"""
import os
import sys

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, 'data')


def params(path, title, items):
    lines = [f'##TITLE= Parameter file, {title}', '##JCAMPDX= 5.0', '##DATATYPE= Parameter Values',
             '##ORIGIN= Bruker BioSpin GmbH', '##OWNER= fixture',
             '$$ 2026-10-19 12:00:00.000 +0200  fixture@specular',
             '$$ /opt/data/fixture']
    for k, v in items:
        if isinstance(v, list):
            lines.append(f'##${k}= (0..{len(v) - 1})')
            lines.append(' '.join(str(x) for x in v))
        else:
            lines.append(f'##${k}= {v}')
    lines.append('##END=')
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, 'w') as f:
        f.write('\n'.join(lines) + '\n')


def fid(n, sw, sf, car_ppm, lines, gd, phase_deg, t2=0.08):
    """Lines (ppm, amplitude) as a FID of n complex points delayed by gd points, the way a
    digital filter delays it: a causal FID shifted by a fractional circular shift (the
    points before the delay ring a little, as the filter's output does)."""
    big = 8 * n
    t = np.arange(big) / sw
    x = np.zeros(big, complex)
    for ppm, a in lines:
        f = (ppm - car_ppm) * sf
        x += a * np.exp(2j * np.pi * f * t - t / t2)
    k = np.fft.fftfreq(big) * big
    x = np.fft.ifft(np.fft.fft(x) * np.exp(-2j * np.pi * gd * k / big))[:n]
    return x * np.exp(1j * np.radians(phase_deg))


def to_int32(x, peak=2e8):
    s = peak / np.abs(x).max()
    v = np.empty(2 * len(x))
    v[0::2] = (x * s).real
    v[1::2] = (x * s).imag
    return np.round(v).astype(np.int64).clip(-2**31, 2**31 - 1)


def process(x, size, lb, sw, gd, phase_deg):
    """The reference processing: em, zero fill, fft in Bruker order, delay and phase."""
    j = np.arange(len(x))
    y = np.zeros(size, complex)
    y[:len(x)] = x * np.exp(-np.pi * lb * j / sw)
    s = np.fft.fftshift(np.fft.fft(y))
    s = s * np.exp(2j * np.pi * gd * np.arange(size) / size)
    s = s[(size - np.arange(size)) % size]
    # The constant part of the delay's phase, and the error put in.
    s = s * np.exp(-1j * (np.radians(phase_deg) + np.pi * gd))
    return s.real


def write(path, values, dtype):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    np.asarray(values).astype(dtype).tofile(path)


def fid1d():
    sw, sf, o1 = 5000.0, 400.13, 1900.0
    car = o1 / sf
    td = 4096
    gd = 67.9856
    lines = [(7.26, 1.0), (3.70, 0.6), (1.20, 0.8)]
    x = fid(td // 2, sw, sf, car, lines, gd, 40.0)
    v = to_int32(x)
    acq = [('TD', td), ('SW_h', sw), ('SW', sw / sf), ('SFO1', sf + o1 * 1e-6), ('BF1', sf), ('O1', o1),
           ('NUC1', '<1H>'), ('PULPROG', '<zg30>'), ('SOLVENT', '<CDCl3>'), ('DATE', 1760875200),
           ('INSTRUM', '<fixture>'), ('PROBHD', '<5 mm test probe>'), ('NS', 16), ('DS', 2),
           ('RG', 101), ('TE', 298.0), ('AQ_mod', 3), ('DIGMOD', 1), ('DSPFVS', 20), ('DECIM', 1666),
           ('GRPDLY', gd), ('BYTORDA', 0), ('DTYPA', 0), ('D', [0, 1.0, 0, 0]), ('P', [0, 10.0, 0])]
    base = os.path.join(OUT, 'fid1d')
    params(os.path.join(base, 'acqus'), 'TOPSPIN\t\tVersion 4.1.4', acq)
    write(os.path.join(base, 'fid'), v, '<i4')
    # pdata/1: 1r of the same FID processed here.
    size, lb = 8192, 0.5
    xs = (v[0::2] + 1j * v[1::2])
    r = process(xs, size, lb, sw, gd, 40.0)
    nc = -3
    r_stored = np.round(r / 2.0 ** nc / np.abs(r).max() * 1e6)
    sf_p = sf  # SR = 0
    offset = ((sf + o1 * 1e-6 - sf_p) * 1e6 + sw / 2) / sf_p
    pr = [('SI', size), ('SF', sf_p), ('SW_p', sw), ('OFFSET', round(offset, 6)), ('NC_proc', nc),
          ('BYTORDP', 0), ('DTYPP', 0), ('WDW', 1), ('LB', lb), ('PHC0', 0), ('PHC1', 0), ('XDIM', size)]
    params(os.path.join(base, 'pdata/1/procs'), 'TOPSPIN\t\tVersion 4.1.4', pr)
    write(os.path.join(base, 'pdata/1/1r'), r_stored, '<i4')
    with open(os.path.join(base, 'pdata/1/title'), 'w') as f:
        f.write('Three lines\nsynthetic fixture\n')
    peaks = ''.join(f'    <Peak1D F1="{p}" intensity="1.0" type="0"/>\n' for p, _ in lines)
    with open(os.path.join(base, 'pdata/1/peaklist.xml'), 'w') as f:
        f.write('<?xml version="1.0" encoding="UTF-8"?>\n<PeakList>\n  <PeakList1D>\n'
                '    <PeakList1DHeader creator="fixture"/>\n' + peaks + '  </PeakList1D>\n</PeakList>\n')
    # The same FID as big-endian float64.
    b2 = os.path.join(OUT, 'fid1d_f64_be')
    acq2 = [(k, {'BYTORDA': 1, 'DTYPA': 2}.get(k, val)) for k, val in acq]
    params(os.path.join(b2, 'acqus'), 'TOPSPIN\t\tVersion 4.1.4', acq2)
    write(os.path.join(b2, 'fid'), v.astype(float), '>f8')


def old_xwin():
    sw, sf, o1 = 25000.0, 100.61, 10061.0
    car = o1 / sf
    td = 4096
    gd = 66.625
    x = fid(td // 2, sw, sf, car, [(77.0, 1.0), (128.5, 0.7)], gd, -25.0, t2=0.05)
    acq = [('TD', td), ('SW_h', sw), ('SW', sw / sf), ('SFO1', sf + o1 * 1e-6), ('BF1', sf), ('O1', o1),
           ('NUC1', '<13C>'), ('PULPROG', '<zgpg30>'), ('SOLVENT', '<CDCl3>'), ('DATE', 1000000000),
           ('AQ_mod', 1), ('DIGMOD', 1), ('DSPFVS', 10), ('DECIM', 4), ('BYTORDA', 1), ('NS', 128)]
    base = os.path.join(OUT, 'old_xwin')
    params(os.path.join(base, 'acqus'), 'XWIN-NMR\t\tVersion 3.5', acq)
    write(os.path.join(base, 'fid'), to_int32(x), '>i4')


def hsqc2d():
    base = os.path.join(OUT, 'hsqc2d')
    td2, td1 = 512, 8
    acq = [('TD', td2), ('SW_h', 4000.0), ('SFO1', 500.13 + 2350.0e-6), ('BF1', 500.13), ('O1', 2350.0),
           ('NUC1', '<1H>'), ('PULPROG', '<hsqcetgp>'), ('AQ_mod', 3), ('DIGMOD', 1),
           ('GRPDLY', 67.98), ('DSPFVS', 20), ('DECIM', 2500), ('BYTORDA', 0), ('DTYPA', 0)]
    params(os.path.join(base, 'acqus'), 'TOPSPIN\t\tVersion 3.6.2', acq)
    params(os.path.join(base, 'acqu2s'), 'TOPSPIN\t\tVersion 3.6.2',
           [('TD', td1), ('SW', 160.0), ('SFO1', 125.76 + 75.0 * 125.76e-6), ('BF1', 125.76),
            ('NUC1', '<13C>')])
    rows = []
    for r in range(td1):
        x = fid(td2 // 2, 4000.0, 500.13, 4.7, [(2.0 + 0.3 * r, 1.0)], 67.98, 0.0)
        v = to_int32(x, 1e6)
        rows.append(v)
    write(os.path.join(base, 'ser'), np.concatenate(rows), '<i4')
    si2, si1, x2, x1 = 64, 32, 16, 8
    m = np.array([[r * 1000 + c for c in range(si2)] for r in range(si1)], float)
    tiles = []
    for br in range(si1 // x1):
        for bc in range(si2 // x2):
            tiles.append(m[br * x1:(br + 1) * x1, bc * x2:(bc + 1) * x2].reshape(-1))
    write(os.path.join(base, 'pdata/1/2rr'), np.concatenate(tiles), '<i4')
    params(os.path.join(base, 'pdata/1/procs'), 'TOPSPIN\t\tVersion 3.6.2',
           [('SI', si2), ('SF', 500.13), ('SW_p', 4000.0), ('OFFSET', round(2350.0 / 500.13 + 2000.0 / 500.13, 6)), ('NC_proc', 0),
            ('BYTORDP', 0), ('DTYPP', 0), ('XDIM', x2), ('WDW', 0)])
    params(os.path.join(base, 'pdata/1/proc2s'), 'TOPSPIN\t\tVersion 3.6.2',
           [('SI', si1), ('SF', 125.76), ('SW_p', 125.76 * 160.0), ('OFFSET', 155.0), ('XDIM', x1)])


def varian():
    base = os.path.join(OUT, 'varian')
    os.makedirs(base, exist_ok=True)
    with open(os.path.join(base, 'procpar'), 'w') as f:
        f.write('sfrq 1 1 1000000000 0 0 2 1 11 1 64\n1 399.9\n0\n')
    with open(os.path.join(base, 'fid'), 'wb') as f:
        f.write(bytes(64))


if __name__ == '__main__':
    fid1d()
    old_xwin()
    hsqc2d()
    varian()
    total = sum(os.path.getsize(os.path.join(d, f)) for d, _, fs in os.walk(OUT) for f in fs)
    print(f'{total} bytes under {OUT}')
