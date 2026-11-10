#!/usr/bin/env python3
"""Write tifffile's reading of the scientific-TIFF corpus for the specular-decode corpus test.

Usage: scitiff-oracle.py CORPUS_DIR

For every TIFF-family file under CORPUS_DIR/dl (ImageJ hyperstacks, Zeiss LSM, MetaMorph STK,
SEM images) writes CORPUS_DIR/expected/<name>.json: the series' axes and shape, the pixel size
in metres where tifffile reads one (ImageJ unit and resolution, LSM voxel size, Zeiss SEM
pixel size), the ImageJ display ranges, and per plane (keyed by its C/Z/T position) the
sample count, sum, NaN count, minimum, maximum and every 97th value.

The corpus is untrusted data: run this with python -I from outside it.
"""

import json
import os
import sys

import numpy
import tifffile

STRIDE = 97
UNITS = {
    'm': 1.0, 'mm': 1e-3, 'um': 1e-6, 'µm': 1e-6, 'micron': 1e-6, 'microns': 1e-6,
    '\\u00B5m': 1e-6, 'nm': 1e-9, 'cm': 1e-2, 'pm': 1e-12,
}
SKIP = ('.zip', '.lut', '.json', '.txt', '.part')


def pixel(f):
    """The pixel size in metres tifffile reads, or None."""
    page = f.pages.first
    if f.is_lsm:
        m = f.lsm_metadata
        if m['VoxelSizeX'] > 0:
            return [m['VoxelSizeX'], m['VoxelSizeY'] or m['VoxelSizeX']]
        return None
    if f.is_imagej:
        unit = f.imagej_metadata.get('unit')
        res = page.tags.get('XResolution')
        if unit in UNITS and res is not None and res.value[0]:
            yres = page.tags['YResolution'].value
            return [res.value[1] / res.value[0] * UNITS[unit],
                    yres[1] / yres[0] * UNITS[unit]]
        return None
    if f.is_sem:
        m = f.sem_metadata
        v = m.get('ap_image_pixel_size') or m.get('ap_pixel_size')
        if isinstance(v, tuple) and len(v) >= 3 and v[2] in UNITS:
            s = float(v[1]) * UNITS[v[2]]
            return [s, s]
        return None
    return None


def planes(arr, axes):
    """(index dict, plane) for each plane of `arr` (axes ending YX, optionally S last)."""
    if axes.endswith('S'):
        arr = numpy.moveaxis(arr, -1, 0)
        axes = 'C' + axes[:-1]
    lead = axes[:-2]
    for idx in numpy.ndindex(*arr.shape[: len(lead)]):
        yield {a: int(i) for a, i in zip(lead, idx, strict=True)}, arr[idx]


def stats(plane):
    v = numpy.asarray(plane, dtype=numpy.float64).ravel()
    finite = v[~numpy.isnan(v)]
    sample = v[::STRIDE]
    return {
        'count': int(v.size),
        'sum': float(finite.sum()),
        'nan': int(v.size - finite.size),
        'min': float(finite.min()) if finite.size else None,
        'max': float(finite.max()) if finite.size else None,
        'sample': [None if numpy.isnan(x) else float(x) for x in sample],
    }


def oracle(fn):
    with tifffile.TiffFile(fn) as f:
        s = f.series[0]
        arr = s.asarray()
        out = {
            'axes': s.axes,
            'shape': list(s.shape),
            'dtype': str(s.dtype),
            'pixel': pixel(f),
            'planes': [{'index': i, **stats(p)} for i, p in planes(arr, s.axes)],
        }
        if f.is_imagej:
            ij = f.imagej_metadata
            r = ij.get('Ranges')
            if r:
                out['ranges'] = [list(r[i : i + 2]) for i in range(0, len(r), 2)]
            if ij.get('spacing'):
                out['z_step'] = ij['spacing']
        return out


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    root = sys.argv[1]
    src = os.path.join(root, 'dl')
    dst = os.path.join(root, 'expected')
    os.makedirs(dst, exist_ok=True)
    for name in sorted(os.listdir(src)):
        if name.lower().endswith(SKIP) or name.startswith('.'):
            continue
        o = oracle(os.path.join(src, name))
        with open(os.path.join(dst, name + '.json'), 'w', encoding='utf-8') as out:
            json.dump(o, out, separators=(',', ':'), sort_keys=True)
            out.write('\n')
        print(f'  {name}: {o["axes"]} {o["shape"]} {o["dtype"]} pixel {o["pixel"]}')


if __name__ == '__main__':
    main()
