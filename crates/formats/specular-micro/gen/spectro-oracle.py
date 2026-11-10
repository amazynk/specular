"""Oracle for OIR, SPE and SIF (tools/corpus/fetch-microscopy.sh).

  spectro-oracle.py OUT    write OUT/expected-micro.tsv and reference values under OUT/oracle

Rows (tab-separated; paths relative to OUT):
  pages  FILE N                     the number of pages (frames × regions)
  page   FILE K W H RAW             page K's values (RAW: little-endian f32, W*H)
  wl     FILE K RAW                 page K's column wavelengths, nm (little-endian f64), or -
  fact   FILE KEY VALUE             exposure (s) or temperature (°C)
  image  FILE W H C Z T             an OIR's size (C counts channels × λ, as Bio-Formats)
  plane  FILE C Z T RAW             an OIR plane (Bio-Formats' C index)

Oracles: spe2py's spe_loader (LightField 3.0) and imageio's SPE plugin (2.x), sif_parser,
Bio-Formats' bfconvert for OIR (BFTOOLS names its folder). Downloads are opened as data only.
"""

import glob
import os
import re
import subprocess
import sys

import numpy as np

OUT = sys.argv[1]
ORACLE = os.path.join(OUT, 'oracle')
os.makedirs(ORACLE, exist_ok=True)
rows = []


def slug(s):
    return re.sub(r'[^A-Za-z0-9]+', '_', s)


def save(name, a, dtype):
    path = os.path.join('oracle', name)
    np.ascontiguousarray(a, dtype=dtype).tofile(os.path.join(OUT, path))
    return path


def page_rows(rel, pages, wls):
    rows.append(['pages', rel, len(pages)])
    for k, p in enumerate(pages):
        if k > 3 and k != len(pages) - 1:
            continue
        p = np.asarray(p)
        if p.ndim == 1:
            p = p[None, :]
        rows.append(['page', rel, k, p.shape[1], p.shape[0], save(f'{slug(rel)}.{k}.f32', p, '<f4')])
        wl = wls[k] if k < len(wls) else None
        rows.append(['wl', rel, k, '-' if wl is None else save(f'{slug(rel)}.{k}.wl', wl, '<f8')])


def spe_rows(rel):
    path = os.path.join(OUT, rel)
    with open(path, 'rb') as f:
        b = f.read()
    ver = np.frombuffer(b[1992:1996], '<f4')[0]
    if ver >= 3:
        import spe_loader
        s = spe_loader.SpeFile(path)
        pages, wls = [], []
        wl = s.wavelength
        for fr in range(s.nframes):
            for r in range(s.nroi):
                pages.append(s.data[fr][r])
                w = s.data[fr][r].shape[-1]
                wls.append(wl if wl is not None and len(wl) == w else None)
        page_rows(rel, pages, wls)
    else:
        import imageio.v3 as iio
        a = iio.imread(path, index=..., plugin='SPE')
        meta = iio.immeta(path, plugin='SPE', sdt_control=False)
        page_rows(rel, list(a), [])
        if meta.get('exposure_sec'):
            rows.append(['fact', rel, 'exposure', meta['exposure_sec']])


def sif_rows(rel):
    import sif_parser
    from sif_parser.utils import extract_calibration
    data, info = sif_parser.np_open(os.path.join(OUT, rel))
    cal = extract_calibration(info)
    pages = list(np.asarray(data))
    wls = []
    for k in range(len(pages)):
        c = None
        if cal is not None:
            c = cal[k] if cal.ndim == 2 else cal
            # The identity (pixel numbers) is no calibration.
            if np.allclose(c, np.arange(1, len(c) + 1)):
                c = None
        wls.append(c)
    page_rows(rel, pages, wls)
    rows.append(['fact', rel, 'exposure', info['ExposureTime']])
    rows.append(['fact', rel, 'temperature', info['DetectorTemperature']])


BFTOOLS = os.environ.get('BFTOOLS', '')


def bf(tool, *args):
    home = os.path.join(OUT, '.java')
    os.makedirs(os.path.join(home, 'tmp'), exist_ok=True)
    env = dict(os.environ, _JAVA_OPTIONS=f'-Duser.home={home} -Djava.io.tmpdir={home}/tmp')
    return subprocess.run([os.path.join(BFTOOLS, tool), '-no-upgrade', *args], env=env,
                          capture_output=True, text=True, check=True).stdout


def oir_rows(rel):
    import tifffile
    path = os.path.join(OUT, rel)
    out = bf('showinf', '-nopix', '-nometa', path)
    g = lambda k: int(re.search(rf'{k} = (\d+)', out).group(1))
    w, h, c, z, t = g('Width'), g('Height'), g('SizeC'), g('SizeZ'), g('SizeT')
    rows.append(['image', rel, w, h, c, z, t])
    for k, (cc, zz, tt) in enumerate({(0, 0, 0), (c - 1, z - 1, t - 1), (c // 2, z // 2, t // 2)}):
        tmp = os.path.join(OUT, '.java', 'plane.tif')
        if os.path.exists(tmp):
            os.remove(tmp)
        bf('bfconvert', '-overwrite', '-channel', str(cc), '-z', str(zz), '-timepoint', str(tt), path, tmp)
        a = tifffile.imread(tmp).astype(np.float32).reshape(h, w)
        os.remove(tmp)
        rows.append(['plane', rel, cc, zz, tt, save(f'{slug(rel)}.p{k}.f32', a, '<f4')])


def main():
    for p in sorted(glob.glob(os.path.join(OUT, 'spe', '*'))):
        spe_rows(os.path.relpath(p, OUT))
    for p in sorted(glob.glob(os.path.join(OUT, 'sif', '*.sif'))):
        sif_rows(os.path.relpath(p, OUT))
    if BFTOOLS:
        for p in sorted(glob.glob(os.path.join(OUT, 'oir', '*.oir'))):
            rel = os.path.relpath(p, OUT)
            try:
                oir_rows(rel)
            except subprocess.CalledProcessError:
                rows.append(['refuse', rel])
    with open(os.path.join(OUT, 'expected-micro.tsv'), 'w') as f:
        f.write('# written by crates/formats/specular-micro/gen/spectro-oracle.py\n')
        for r in rows:
            f.write('\t'.join(str(v) for v in r) + '\n')
    print(f'{len(rows)} rows')


main()
