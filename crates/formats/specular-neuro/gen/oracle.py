#!/usr/bin/env python3
"""nibabel's reading of GIFTI, CIFTI-2, TrackVis and MRtrix tracks, the oracle for
specular-neuro (tests/oracle.rs); and an independent numpy reading of MRtrix images, the
oracle for specular-medvol's (tests/mif.rs). nibabel is never linked; it runs here, from a
venv.

    python3 -I oracle.py DIR OUT.tsv

Every *.gii, *.dscalar.nii / *.dlabel.nii / *.dtseries.nii / *.pscalar.nii / *.ptseries.nii /
*.dconn.nii,
*.trk, *.tck, *.mif, *.mif.gz and *.mih under DIR (recursively) gives rows (tab separated;
NAME is the path relative to DIR):
  gifti   NAME narrays nlabels
  garray  NAME i intent dtype dims count sum sumsq     (dims comma separated; row-major)
  gname   NAME i name                                  (the array's Name metadata)
  gval    NAME i index value                           (up to MAX_VALUES, strided)
  gxform  NAME i k dataspace xformspace m00..m33
  glabel  NAME key name r g b a
  cifti   NAME shape0 shape1 kind0 kind1
  cmodel  NAME offset count structure type total checksum
                                (type surface|voxels; checksum: sum of vertex indices, or of
                                 i + 1000 j + 1000000 k)
  cvol    NAME d0 d1 d2 m00..m33                       (in millimetres)
  cmapname NAME i name
  cmap    NAME i count sum sumsq                       (map i over the brain dimension; up
                                                        to MAX_ROWS, strided, for dconn)
  cval    NAME i index value                           (strided)
  cmodel0 NAME offset count structure type total checksum   (dimension 0's, both brain models)
  crow    NAME r count sum sumsq                       (row r: dimension 0 at index r of
                                                        dimension 1; up to MAX_ROWS, strided)
  crowval NAME r index value                           (strided)
  clabel  NAME i key name r g b a
  cparcel NAME i name nvertices nvoxels
  tract   NAME streamlines points
  tsum    NAME sumx sumy sumz                          (RAS millimetres)
  tpt     NAME index x y z                             (up to MAX_POINTS, strided)
  tscalar NAME column sum                              (per-point values, columns in order)
  tprop   NAME column sum
  mif     NAME dims vox intercept slope
  mvox    NAME i j k v value x y z                     (image index, scaled value, RAS mm)
  error   NAME words                                   (the reader refused the file)
Values print with repr (shortest round trip).
"""

import gzip
import os
import sys
import warnings

import nibabel as nib
import numpy as np

MAX_VALUES = 64
MAX_POINTS = 256
CIFTI = ('.dscalar.nii', '.dlabel.nii', '.dtseries.nii', '.pscalar.nii', '.ptseries.nii',
         '.dconn.nii')
MAX_ROWS = 8

warnings.simplefilter('ignore')


def clean(s):
    return str(s).replace('\t', ' ').replace('\n', '\\n').replace('\r', '\\r')


def fmt(x):
    x = float(x)
    if x != x:
        return 'nan'
    if x in (float('inf'), float('-inf')):
        return 'inf' if x > 0 else '-inf'
    return repr(x)


def strided(n, m):
    if n <= m:
        return list(range(n))
    return sorted({round(i * (n - 1) / (m - 1)) for i in range(m)})


def gifti(name, path, out):
    g = nib.load(path)
    labels = [lab for lab in g.labeltable.labels]
    out.append(['gifti', name, len(g.darrays), len(labels)])
    for i, d in enumerate(g.darrays):
        a = np.asarray(d.data)
        f = a.astype(np.float64).ravel()
        dtype = {np.dtype('float32'): 'NIFTI_TYPE_FLOAT32', np.dtype('int32'): 'NIFTI_TYPE_INT32',
                 np.dtype('uint8'): 'NIFTI_TYPE_UINT8'}.get(a.dtype.newbyteorder('='), str(a.dtype))
        out.append(['garray', name, i, nib.nifti1.intent_codes.niistring[d.intent], dtype,
                    ','.join(map(str, a.shape)), f.size, fmt(f.sum()), fmt((f * f).sum())])
        if d.meta.get('Name'):
            out.append(['gname', name, i, clean(d.meta['Name'])])
        for k in strided(f.size, MAX_VALUES):
            out.append(['gval', name, i, k, fmt(f[k])])
        for k, c in enumerate(d.coordsys if isinstance(d.coordsys, list) else [d.coordsys]):
            if c is None:
                continue
            out.append(['gxform', name, i, k, nib.nifti1.xform_codes.niistring[c.dataspace],
                        nib.nifti1.xform_codes.niistring[c.xformspace]]
                       + [fmt(v) for v in np.asarray(c.xform, float).ravel()])
    for lab in labels:
        out.append(['glabel', name, lab.key, clean(lab.label or ''), fmt(lab.red), fmt(lab.green),
                    fmt(lab.blue), fmt(lab.alpha)])


KIND = {'CIFTI_INDEX_TYPE_BRAIN_MODELS': 'brain-models', 'CIFTI_INDEX_TYPE_PARCELS': 'parcels',
        'CIFTI_INDEX_TYPE_SCALARS': 'scalars', 'CIFTI_INDEX_TYPE_LABELS': 'labels',
        'CIFTI_INDEX_TYPE_SERIES': 'series'}


def cifti(name, path, out):
    img = nib.load(path)
    data = np.asarray(img.get_fdata(), np.float64)
    m = img.header.matrix
    kinds = ['', '']
    maps = {}
    for mim in m:
        for d in mim.applies_to_matrix_dimension:
            kinds[d] = KIND[mim.indices_map_to_data_type]
            maps[d] = mim
    out.append(['cifti', name, data.shape[0], data.shape[1], kinds[0], kinds[1]])
    spatial = 1 if kinds[1] in ('brain-models', 'parcels') or kinds[0] not in ('brain-models', 'parcels') else 0
    md = 1 - spatial
    sm = maps[spatial]
    if sm.volume is not None:
        v = sm.volume
        aff = np.asarray(v.transformation_matrix_voxel_indices_ijk_to_xyz.matrix, float).copy()
        aff[:3] *= 10.0 ** (v.transformation_matrix_voxel_indices_ijk_to_xyz.meter_exponent + 3)
        out.append(['cvol', name] + list(v.volume_dimensions) + [fmt(x) for x in aff.ravel()])

    def models(sm, kind):
        for b in sm.brain_models:
            if b.model_type == 'CIFTI_MODEL_TYPE_SURFACE':
                vi = np.asarray(b.vertex_indices, np.int64)
                out.append([kind, name, b.index_offset, b.index_count, b.brain_structure, 'surface',
                            b.surface_number_of_vertices, int(vi.sum())])
            else:
                ijk = np.asarray(b.voxel_indices_ijk, np.int64).reshape(-1, 3)
                out.append([kind, name, b.index_offset, b.index_count, b.brain_structure, 'voxels',
                            0, int((ijk[:, 0] + 1000 * ijk[:, 1] + 1000000 * ijk[:, 2]).sum())])

    if kinds[spatial] == 'brain-models':
        models(sm, 'cmodel')
    else:
        for i, p in enumerate(sm.parcels):
            nv = sum(len(v) for v in p.vertices)
            nx = len(p.voxel_indices_ijk) if p.voxel_indices_ijk is not None else 0
            out.append(['cparcel', name, i, clean(p.name), nv, nx])
    if kinds == ['brain-models', 'brain-models']:
        models(maps[0], 'cmodel0')
        for r in strided(data.shape[1], MAX_ROWS):
            row = data[:, r]
            out.append(['crow', name, r, row.size, fmt(row.sum()), fmt((row * row).sum())])
            for k in strided(row.size, MAX_VALUES):
                out.append(['crowval', name, r, k, fmt(row[k])])
    mm = maps[md]
    if kinds[md] in ('scalars', 'labels'):
        for i, nm in enumerate(mm.named_maps):
            out.append(['cmapname', name, i, clean(nm.map_name or '')])
            if kinds[md] == 'labels':
                for key, lab in sorted(nm.label_table.items()):
                    out.append(['clabel', name, i, key, clean(lab.label), fmt(lab.red), fmt(lab.green),
                                fmt(lab.blue), fmt(lab.alpha)])
    dense = kinds == ['brain-models', 'brain-models']
    for i in strided(data.shape[md], MAX_ROWS) if dense else range(data.shape[md]):
        row = data[i, :] if md == 0 else data[:, i]
        out.append(['cmap', name, i, row.size, fmt(row.sum()), fmt((row * row).sum())])
        for k in strided(row.size, MAX_VALUES):
            out.append(['cval', name, i, k, fmt(row[k])])


def tracts(name, path, out):
    t = nib.streamlines.load(path)
    sl = t.streamlines
    pts = np.asarray(sl.get_data(), np.float64).reshape(-1, 3)
    out.append(['tract', name, len(sl), len(pts)])
    out.append(['tsum', name] + [fmt(v) for v in pts.sum(0)])
    for k in strided(len(pts), MAX_POINTS):
        out.append(['tpt', name, k] + [fmt(v) for v in pts[k]])
    tg = t.tractogram
    col = 0
    for key in tg.data_per_point.keys():
        a = np.asarray(tg.data_per_point[key].get_data(), np.float64).reshape(len(pts), -1)
        for c in range(a.shape[1]):
            out.append(['tscalar', name, col, fmt(a[:, c].sum())])
            col += 1
    col = 0
    for key in tg.data_per_streamline.keys():
        a = np.asarray(tg.data_per_streamline[key], np.float64).reshape(len(sl), -1)
        for c in range(a.shape[1]):
            out.append(['tprop', name, col, fmt(a[:, c].sum())])
            col += 1


MIF_TYPES = {'Int8': 'i1', 'UInt8': 'u1', 'Int16': 'i2', 'UInt16': 'u2', 'Int32': 'i4', 'UInt32': 'u4',
             'Int64': 'i8', 'UInt64': 'u8', 'Float32': 'f4', 'Float64': 'f8'}


def mif(name, path, out):
    """MRtrix images, read from MRtrix's format documentation (independently of the Rust)."""
    raw = open(path, 'rb').read()
    if raw[:2] == b'\x1f\x8b':
        raw = gzip.decompress(raw)
    end = raw.index(b'\nEND\n')
    head = raw[:end].decode('latin1').split('\n')
    assert head[0] == 'mrtrix image'
    kv = {}
    transform = []
    for line in head[1:]:
        k, v = line.split(':', 1)
        k, v = k.strip(), v.strip()
        if k == 'transform':
            transform.append([float(x) for x in v.split(',')])
        else:
            kv[k] = v
    dims = [int(x) for x in kv['dim'].split(',')]
    vox = [float(x) for x in kv['vox'].split(',')]
    lay = kv['layout'].split(',')
    signs = [-1 if s.strip()[0] == '-' else 1 for s in lay]
    ranks = [int(s.strip().lstrip('+-')) for s in lay]
    dt = kv['datatype']
    base = dt[:-2] if dt.endswith(('LE', 'BE')) else dt
    order = '>' if dt.endswith('BE') else '<'
    dtype = np.dtype(order + MIF_TYPES[base])
    fname, off = kv['file'].split()
    off = int(off)
    if fname == '.':
        blob = raw[off:]
    else:
        blob = open(os.path.join(os.path.dirname(path), fname), 'rb').read()[off:]
    n = int(np.prod(dims))
    flat = np.frombuffer(blob[:n * dtype.itemsize], dtype).astype(np.float64)
    # Strides in elements, from the ranks; a reversed axis starts at its far end.
    strides = [0] * len(dims)
    s = 1
    for ax in sorted(range(len(dims)), key=lambda a: ranks[a]):
        strides[ax] = s
        s *= dims[ax]
    offset, slope = 0.0, 1.0
    if 'scaling' in kv:
        offset, slope = (float(x) for x in kv['scaling'].split(','))
    out.append(['mif', name, ','.join(map(str, dims)), ','.join(map(fmt, vox)), fmt(offset), fmt(slope)])
    T = np.array(transform + [[0, 0, 0, 1]], float)
    nd = len(dims)
    total = n
    for idx in strided(total, 512):
        image = list(np.unravel_index(idx, dims, order='F'))
        at = 0
        for ax in range(nd):
            i = image[ax]
            if signs[ax] < 0:
                i = dims[ax] - 1 - i
            at += i * strides[ax]
        val = flat[at] * slope + offset
        ijk = np.array([image[a] * vox[a] if a < nd else 0.0 for a in range(3)] + [1.0])
        xyz = T @ ijk
        v = image[3] if nd > 3 else 0
        out.append(['mvox', name, image[0], image[1], image[2] if nd > 2 else 0, v, fmt(val)]
                   + [fmt(c) for c in xyz[:3]])


def main():
    root, dest = sys.argv[1], sys.argv[2]
    rows = []
    for dirpath, _, files in os.walk(root):
        for f in sorted(files):
            path = os.path.join(dirpath, f)
            name = os.path.relpath(path, root)
            low = f.lower()
            try:
                if low.endswith('.gii'):
                    gifti(name, path, rows)
                elif low.endswith(CIFTI):
                    cifti(name, path, rows)
                elif low.endswith(('.trk', '.tck')):
                    tracts(name, path, rows)
                elif low.endswith(('.mif', '.mif.gz', '.mih')):
                    mif(name, path, rows)
            except Exception as e:  # noqa: BLE001 - every refusal is a row
                rows.append(['error', name, clean(f'{type(e).__name__}: {e}')])
    rows.sort(key=lambda r: (r[1], r[0]))
    with open(dest, 'w') as f:
        f.write('# written by gen/oracle.py with nibabel ' + nib.__version__ + '\n')
        for r in rows:
            f.write('\t'.join(str(x) for x in r) + '\n')


main()
