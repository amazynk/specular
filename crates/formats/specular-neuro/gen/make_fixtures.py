#!/usr/bin/env python3
"""Synthetic fixtures for specular-neuro (tests/data) and specular-medvol's MRtrix reader.

    python3 -I make_fixtures.py OUT_DIR [MIF_DIR]

Written with nibabel where nibabel writes the case (GIFTI in its default zlib-base64 and
ASCII, CIFTI-2 dscalar/dlabel/dtseries/pscalar/dconn, TrackVis v2 with scalars and properties,
MRtrix .tck); by hand where it does not (GIFTI in big-endian base64 and zlib-base64, column
major, mixed types and an external data file; TrackVis v1 and big-endian; MRtrix tracks in
Float32BE; MRtrix .mif/.mih images with permuted and reversed strides, a separate data file
with an offset, scaling and gzip). Every file is read back by gen/oracle.py.
"""

import base64
import gzip
import os
import struct
import sys
import zlib

import nibabel as nib
import numpy as np
from nibabel import cifti2 as ci
from nibabel.streamlines import Field, TckFile, Tractogram, TrkFile

OUT = sys.argv[1]
# The MRtrix images go to specular-medvol's tests (its reader); default: beside the rest.
MIF = sys.argv[2] if len(sys.argv) > 2 else OUT
os.makedirs(OUT, exist_ok=True)
os.makedirs(MIF, exist_ok=True)
rng = np.random.default_rng(20261012)


def path(name):
    return os.path.join(OUT, name)


# --- A small closed surface: an icosahedron subdivided once (42 vertices, 80 faces). -------
def icosphere(radius, centre):
    t = (1 + 5**0.5) / 2
    v = [(-1, t, 0), (1, t, 0), (-1, -t, 0), (1, -t, 0), (0, -1, t), (0, 1, t),
         (0, -1, -t), (0, 1, -t), (t, 0, -1), (t, 0, 1), (-t, 0, -1), (-t, 0, 1)]
    f = [(0, 11, 5), (0, 5, 1), (0, 1, 7), (0, 7, 10), (0, 10, 11), (1, 5, 9), (5, 11, 4),
         (11, 10, 2), (10, 7, 6), (7, 1, 8), (3, 9, 4), (3, 4, 2), (3, 2, 6), (3, 6, 8),
         (3, 8, 9), (4, 9, 5), (2, 4, 11), (6, 2, 10), (8, 6, 7), (9, 8, 1)]
    v = [np.array(p, float) / np.linalg.norm(p) for p in v]
    mid = {}

    def m(a, b):
        k = (min(a, b), max(a, b))
        if k not in mid:
            p = v[a] + v[b]
            v.append(p / np.linalg.norm(p))
            mid[k] = len(v) - 1
        return mid[k]

    nf = []
    for a, b, c in f:
        ab, bc, ca = m(a, b), m(b, c), m(c, a)
        nf += [(a, ab, ca), (b, bc, ab), (c, ca, bc), (ab, bc, ca)]
    pts = np.array(v) * radius * np.array([0.8, 1.0, 0.9]) + centre
    return pts.astype(np.float32), np.array(nf, np.int32)


L_PTS, FACES = icosphere(40.0, np.array([-35.0, -10.0, 15.0]))
R_PTS, _ = icosphere(40.0, np.array([35.0, -10.0, 15.0]))
NV = len(L_PTS)

XFORM = np.array([[1, 0, 0, 2.5], [0, 1, 0, -1.0], [0, 0, 1, 0.5], [0, 0, 0, 1]], float)


def surf_image(pts, faces, structure, encoding):
    coord = nib.gifti.GiftiCoordSystem(dataspace=3, xformspace=3, xform=XFORM)
    p = nib.gifti.GiftiDataArray(pts, intent='NIFTI_INTENT_POINTSET', datatype='NIFTI_TYPE_FLOAT32',
                                 encoding=encoding, coordsys=coord)
    t = nib.gifti.GiftiDataArray(faces, intent='NIFTI_INTENT_TRIANGLE', datatype='NIFTI_TYPE_INT32',
                                 encoding=encoding)
    meta = nib.gifti.GiftiMetaData({'AnatomicalStructurePrimary': structure,
                                    'AnatomicalStructureSecondary': 'MidThickness'})
    return nib.gifti.GiftiImage(darrays=[p, t], meta=meta)


nib.save(surf_image(L_PTS, FACES, 'CortexLeft', 'GIFTI_ENCODING_B64GZ'), path('sub.L.midthickness.surf.gii'))
nib.save(surf_image(R_PTS, FACES, 'CortexRight', 'GIFTI_ENCODING_B64GZ'), path('sub.R.midthickness.surf.gii'))
nib.save(surf_image(L_PTS, FACES, 'CortexLeft', 'GIFTI_ENCODING_ASCII'), path('surf_ascii.surf.gii'))

# Per-vertex data for the left surface.
z = (L_PTS[:, 2] - L_PTS[:, 2].min()) / np.ptp(L_PTS[:, 2])
maps = [np.sin(6 * z).astype(np.float32), (L_PTS[:, 0] / 40).astype(np.float32),
        rng.normal(2.5, 0.4, NV).astype(np.float32)]
names = ['sin_z', 'x', 'thickness']
func = nib.gifti.GiftiImage(darrays=[
    nib.gifti.GiftiDataArray(m, intent='NIFTI_INTENT_NONE', datatype='NIFTI_TYPE_FLOAT32',
                             encoding='GIFTI_ENCODING_B64GZ',
                             meta=nib.gifti.GiftiMetaData({'Name': n}))
    for m, n in zip(maps, names)])
func.meta = nib.gifti.GiftiMetaData({'AnatomicalStructurePrimary': 'CortexLeft'})
nib.save(func, path('sub.L.thickness.func.gii'))

shape = nib.gifti.GiftiImage(darrays=[nib.gifti.GiftiDataArray(
    (L_PTS[:, 1] / 10).astype(np.float32), intent='NIFTI_INTENT_SHAPE',
    datatype='NIFTI_TYPE_FLOAT32', encoding='GIFTI_ENCODING_B64BIN')])
nib.save(shape, path('sub.L.sulc.shape.gii'))

LABELS = np.where(L_PTS[:, 0] < -35, 1, np.where(L_PTS[:, 1] > -10, 2, 0)).astype(np.int32)
LABELS[5] = 3
lt = nib.gifti.GiftiLabelTable()
for key, name, rgba in [(0, '???', (0, 0, 0, 0)), (1, 'medial', (0.9, 0.2, 0.1, 1)),
                        (2, 'anterior', (0.1, 0.5, 0.9, 1)), (3, 'spot', (1, 1, 0, 1))]:
    lab = nib.gifti.GiftiLabel(key=key, red=rgba[0], green=rgba[1], blue=rgba[2], alpha=rgba[3])
    lab.label = name
    lt.labels.append(lab)
lab_img = nib.gifti.GiftiImage(labeltable=lt, darrays=[nib.gifti.GiftiDataArray(
    LABELS, intent='NIFTI_INTENT_LABEL', datatype='NIFTI_TYPE_INT32', encoding='GIFTI_ENCODING_B64GZ')])
nib.save(lab_img, path('sub.L.aparc.label.gii'))

# --- Hand-written GIFTI: the cases nibabel does not write. --------------------------------
HEAD = ('<?xml version="1.0" encoding="UTF-8"?>\n'
        '<!DOCTYPE GIFTI SYSTEM "http://www.nitrc.org/frs/download.php/115/gifti.dtd">\n')


def encode(arr, encoding, big):
    a = arr.astype(arr.dtype.newbyteorder('>' if big else '<'))
    if encoding == 'ASCII':
        rows = a.reshape(a.shape[0], -1) if a.ndim > 1 else a.reshape(-1, 1)
        return '\n'.join(' '.join(repr(x.item()) if a.dtype.kind == 'f' else str(int(x)) for x in r)
                         for r in rows)
    raw = a.tobytes(order='C')
    if encoding == 'GZipBase64Binary':
        raw = zlib.compress(raw)
    b = base64.b64encode(raw).decode()
    # Wrapped lines, as some writers do.
    return '\n'.join(b[i:i + 76] for i in range(0, len(b), 76))


TYPES = {np.float32: 'NIFTI_TYPE_FLOAT32', np.int32: 'NIFTI_TYPE_INT32', np.uint8: 'NIFTI_TYPE_UINT8'}


def darray(arr, intent, encoding, big, column_major=False, name=None, xform=None, external=None):
    dims = ''.join(f' Dim{i}="{d}"' for i, d in enumerate(arr.shape))
    order = 'ColumnMajorOrder' if column_major else 'RowMajorOrder'
    # Column-major storage is the C order of the transpose. In ASCII it is written as one
    # flat run: nibabel reshapes the rows loadtxt returns in Fortran order, which reads a
    # column-major file written as rows of the transpose wrongly (other readers take the
    # values as a flat stream, as this crate does).
    payload = np.ascontiguousarray(arr.T) if column_major else arr
    if column_major and encoding == 'ASCII':
        payload = payload.reshape(-1)
    meta = f'<MetaData><MD><Name>Name</Name><Value><![CDATA[{name}]]></Value></MD></MetaData>' if name else '<MetaData/>'
    xf = ''
    if xform is not None:
        xf = ('<CoordinateSystemTransformMatrix><DataSpace><![CDATA[NIFTI_XFORM_SCANNER_ANAT]]></DataSpace>'
              '<TransformedSpace><![CDATA[NIFTI_XFORM_MNI_152]]></TransformedSpace><MatrixData>'
              + ' '.join(repr(float(x)) for x in xform.ravel()) + '</MatrixData></CoordinateSystemTransformMatrix>')
    if external:
        fname, offset = external
        ext = f' ExternalFileName="{os.path.basename(fname)}" ExternalFileOffset="{offset}"'
        data = '<Data></Data>'
    else:
        ext = ' ExternalFileName="" ExternalFileOffset=""'
        data = f'<Data>{encode(payload, encoding, big)}</Data>'
    return (f'<DataArray Intent="{intent}" DataType="{TYPES[arr.dtype.type]}" ArrayIndexingOrder="{order}"'
            f' Dimensionality="{arr.ndim}"{dims} Encoding="{encoding}"'
            f' Endian="{"BigEndian" if big else "LittleEndian"}"{ext}>\n{meta}{xf}\n{data}\n</DataArray>\n')


def gifti(name, arrays, labels=''):
    with open(path(name), 'w') as f:
        f.write(HEAD + f'<GIFTI Version="1.0" NumberOfDataArrays="{len(arrays)}">\n<MetaData>'
                '<MD><Name>AnatomicalStructurePrimary</Name><Value>CortexLeft</Value></MD></MetaData>\n'
                + labels + ''.join(arrays) + '</GIFTI>\n')


gifti('surf_b64_be.surf.gii', [darray(L_PTS, 'NIFTI_INTENT_POINTSET', 'Base64Binary', True, xform=XFORM),
                               darray(FACES, 'NIFTI_INTENT_TRIANGLE', 'Base64Binary', True, column_major=True)])
gifti('surf_gz_be.surf.gii', [darray(L_PTS, 'NIFTI_INTENT_POINTSET', 'GZipBase64Binary', True, column_major=True),
                              darray(FACES, 'NIFTI_INTENT_TRIANGLE', 'GZipBase64Binary', True)])
gifti('surf_ascii_cm.surf.gii', [darray(L_PTS, 'NIFTI_INTENT_POINTSET', 'ASCII', False, column_major=True),
                                 darray(FACES, 'NIFTI_INTENT_TRIANGLE', 'ASCII', False)])
U8 = (np.arange(NV) * 7 % 256).astype(np.uint8)
I32 = (np.arange(NV, dtype=np.int32) - 20) * 1000003
gifti('mixed.func.gii', [
    darray(maps[0], 'NIFTI_INTENT_NONE', 'ASCII', False, name='ascii f32'),
    darray(maps[1], 'NIFTI_INTENT_NONE', 'Base64Binary', True, name='b64 f32 BE'),
    darray(I32, 'NIFTI_INTENT_NONE', 'Base64Binary', True, name='b64 i32 BE'),
    darray(I32, 'NIFTI_INTENT_NONE', 'GZipBase64Binary', False, name='gz i32 LE'),
    darray(U8, 'NIFTI_INTENT_NONE', 'GZipBase64Binary', True, name='gz u8'),
    darray(U8, 'NIFTI_INTENT_NONE', 'ASCII', False, name='ascii u8'),
    darray(maps[2], 'NIFTI_INTENT_NONE', 'GZipBase64Binary', True, name='gz f32 BE'),
])
# Big-endian float32 data in a separate file, 32 bytes in.
with open(path('external.dat'), 'wb') as f:
    f.write(b'\0' * 32 + maps[2].astype('>f4').tobytes())
gifti('external.shape.gii', [darray(maps[2], 'NIFTI_INTENT_SHAPE', 'ExternalFileBinary', True,
                                    external=('external.dat', 32))])
labels = ('<LabelTable><Label Key="0" Red="1" Green="1" Blue="1" Alpha="0"><![CDATA[none]]></Label>'
          '<Label Key="7" Red="0.2" Green="0.8" Blue="0.3" Alpha="1">seven &amp; up</Label></LabelTable>\n')
gifti('u8labels.label.gii', [darray(np.where(U8 > 128, 7, 0).astype(np.uint8), 'NIFTI_INTENT_LABEL',
                                    'Base64Binary', False)], labels)

# --- CIFTI-2 ---------------------------------------------------------------------------------
VOL_SHAPE = (10, 12, 8)
VOL_AFF = np.array([[-2, 0, 0, 9], [0, 2, 0, -12], [0, 0, 2, -8], [0, 0, 0, 1]], float)
lmask = np.ones(NV, bool)
lmask[[3, 17]] = False  # medial wall: two vertices left out
thal = np.zeros(VOL_SHAPE, bool)
thal[2:5, 3:7, 2:4] = True
put = np.zeros(VOL_SHAPE, bool)
put[6:8, 5:9, 4:6] = True
bm = (ci.BrainModelAxis.from_mask(lmask, 'CortexLeft')
      + ci.BrainModelAxis.from_mask(np.ones(NV, bool), 'CortexRight')
      + ci.BrainModelAxis.from_mask(thal, 'ThalamusLeft', affine=VOL_AFF)
      + ci.BrainModelAxis.from_mask(put, 'PutamenRight', affine=VOL_AFF))
nb = len(bm)
xyz = np.concatenate([L_PTS[lmask], R_PTS, (np.argwhere(thal) * 4.0 - 20).astype(np.float32),
                      (np.argwhere(put) * 4.0 - 20).astype(np.float32)])
dscalar = np.stack([np.sin(xyz[:, 2] / 15), xyz[:, 0] / 50]).astype(np.float32)
img = ci.Cifti2Image(dscalar, header=(ci.ScalarAxis(['sin z', 'x over 50']), bm))
img.nifti_header.set_intent('ConnDenseScalar')
nib.save(img, path('sub.dscalar.nii'))

keys = np.where(xyz[:, 0] < 0, 1, 2).astype(np.float32)
keys[xyz[:, 2] > 30] = 3
la = ci.LabelAxis(['parts'], [{0: ('???', (0, 0, 0, 0)), 1: ('left', (1, 0, 0, 1)),
                              2: ('right', (0, 0, 1, 1)), 3: ('top', (0, 1, 0, 1))}])
img = ci.Cifti2Image(keys[None, :], header=(la, bm))
img.nifti_header.set_intent('ConnDenseLabel')
nib.save(img, path('sub.dlabel.nii'))

ts = np.stack([np.sin(xyz[:, 1] / 10 + t) for t in range(5)]).astype(np.float32)
img = ci.Cifti2Image(ts, header=(ci.SeriesAxis(start=0.0, step=0.72, size=5, unit='second'), bm))
img.nifti_header.set_intent('ConnDenseSeries')
nib.save(img, path('sub.dtseries.nii'))

parcels = ci.ParcelsAxis.from_brain_models([
    ('lateral', ci.BrainModelAxis.from_mask(L_PTS[:, 0] < -35, 'CortexLeft')),
    ('right', ci.BrainModelAxis.from_mask(np.ones(NV, bool), 'CortexRight')),
    ('deep', ci.BrainModelAxis.from_mask(thal, 'ThalamusLeft', affine=VOL_AFF)),
])
img = ci.Cifti2Image(np.array([[1.5, -2.0, 3.25], [0.5, 0.25, 0.125]], np.float32),
                     header=(ci.ScalarAxis(['a', 'b']), parcels))
img.nifti_header.set_intent('ConnParcelScalr')
nib.save(img, path('sub.pscalar.nii'))

# A surface-only dscalar whose surfaces are not beside it.
img = ci.Cifti2Image(dscalar[:, :NV - 2][:1], header=(ci.ScalarAxis(['only left']),
                                                       ci.BrainModelAxis.from_mask(lmask, 'CortexLeft')))
os.makedirs(path('alone'), exist_ok=True)
nib.save(img, os.path.join(OUT, 'alone', 'left_only.dscalar.nii'))

# Dense connectivity (brain models × brain models): a correlation-like matrix that falls off
# with distance, symmetric, over the same brain models (sub.dconn.nii, shown on the surfaces
# beside it); and one over voxels only whose rows (seeds) are the thalamus alone and whose
# columns are both structures, in float64 (alone/vox.dconn.nii, shown as a volume).
dist = np.linalg.norm(xyz[:, None, :] - xyz[None, :, :], axis=2)
dconn = np.exp(-dist / 25.0) * np.cos(dist / 40.0)
img = ci.Cifti2Image(dconn.astype(np.float32), header=(bm, bm))
img.nifti_header.set_intent('ConnDense')
nib.save(img, path('sub.dconn.nii'))

vox = (ci.BrainModelAxis.from_mask(thal, 'ThalamusLeft', affine=VOL_AFF)
       + ci.BrainModelAxis.from_mask(put, 'PutamenRight', affine=VOL_AFF))
seeds = ci.BrainModelAxis.from_mask(thal, 'ThalamusLeft', affine=VOL_AFF)
vxyz = np.concatenate([np.argwhere(thal), np.argwhere(put)]).astype(float)
sxyz = np.argwhere(thal).astype(float)
# nibabel's axes are (rows, columns): the seeds are CIFTI dimension 1, its columns.
vd = np.linalg.norm(vxyz[:, None, :] - sxyz[None, :, :], axis=2)
img = ci.Cifti2Image(1.0 - vd / 10.0, header=(vox, seeds))
img.nifti_header.set_intent('ConnDense')
img.set_data_dtype(np.float64)
nib.save(img, os.path.join(OUT, 'alone', 'vox.dconn.nii'))

# --- Tractography ----------------------------------------------------------------------------
def streamlines(n, seed):
    r = np.random.default_rng(seed)
    out = []
    for i in range(n):
        m = int(r.integers(2, 30))
        start = r.uniform(-40, 40, 3)
        step = r.normal(0, 1, 3)
        step = step / np.linalg.norm(step) * 1.5
        bend = r.normal(0, 0.05, 3)
        pts = [start + k * step + k * k * bend for k in range(m)]
        out.append(np.array(pts, np.float32))
    return out


S = streamlines(40, 1)
dpp = {'fa': [np.abs(np.sin(s[:, :1])).astype(np.float32) for s in S],
       'colour': [np.abs(s[:, :3] / 40).astype(np.float32) for s in S]}
dps = {'length': np.array([[len(s) * 1.5] for s in S], np.float32),
       'id': np.arange(len(S), dtype=np.float32)[:, None]}
aff = np.array([[-1.25, 0, 0, 90], [0, 1.25, 0, -126], [0, 0, 1.25, -72], [0, 0, 0, 1]], float)
hdr = {Field.VOXEL_TO_RASMM: aff, Field.VOXEL_SIZES: np.array([1.25, 1.25, 1.25], np.float32),
       Field.DIMENSIONS: np.array([145, 174, 145], np.int16), Field.VOXEL_ORDER: b'LAS'}
t = Tractogram(S, data_per_point=dpp, data_per_streamline=dps, affine_to_rasmm=np.eye(4))
TrkFile(t, header=hdr).save(path('scalars.trk'))
t2 = Tractogram(streamlines(25, 2), affine_to_rasmm=np.eye(4))
TckFile(t2).save(path('plain.tck'))

# TrackVis by hand: version 1 (no matrix; LPS order) and version 2 big-endian with a
# rotation, so both header paths and both byte orders are covered.
def trk_by_hand(name, sls, version, big, vox_to_ras, order, vs=(2.0, 2.0, 2.0), dim=(64, 64, 40)):
    e = '>' if big else '<'
    h = bytearray(1000)
    h[0:6] = b'TRACK\0'
    struct.pack_into(e + '3h', h, 6, *dim)
    struct.pack_into(e + '3f', h, 12, *vs)
    struct.pack_into(e + 'h', h, 36, 1)
    h[38:38 + 6] = b'curv\0\0'
    struct.pack_into(e + 'h', h, 238, 0)
    if vox_to_ras is not None:
        struct.pack_into(e + '16f', h, 440, *np.asarray(vox_to_ras, float).ravel())
    h[948:951] = order
    struct.pack_into(e + '3i', h, 988, len(sls), version, 1000)
    body = bytearray()
    for s in sls:
        body += struct.pack(e + 'i', len(s))
        for k, p in enumerate(s):
            body += struct.pack(e + '4f', *p, float(k) / len(s))
    with open(path(name), 'wb') as f:
        f.write(bytes(h) + bytes(body))


VM = [np.abs(s) + 1 for s in streamlines(12, 3)]
trk_by_hand('v1.trk', VM, 1, False, None, b'LPS')
c, s_ = np.cos(0.3), np.sin(0.3)
rot = np.array([[2 * c, -2 * s_, 0, -60], [2 * s_, 2 * c, 0, -70], [0, 0, -2, 30], [0, 0, 0, 1]])
trk_by_hand('be_oblique.trk', VM, 2, True, rot, b'RAI')

# MRtrix tracks in Float32BE, with an empty streamline (skipped by readers).
sl = streamlines(9, 4)
hdr = b'mrtrix tracks\ncount: 9\ndatatype: Float32BE\nstep_size: 1.5\nfile: . 128\nEND\n'
body = b''.join(s.astype('>f4').tobytes() + np.full(3, np.nan, '>f4').tobytes() for s in sl)
body = np.full(3, np.nan, '>f4').tobytes() + body + np.full(3, np.inf, '>f4').tobytes()
with open(path('be.tck'), 'wb') as f:
    f.write(hdr.ljust(128, b'\0') + body)


# --- MRtrix images ---------------------------------------------------------------------------
def mif(name, arr, layout, dtype, vox, transform, sep=None, offset=None, scaling=None, gz=False):
    """arr is indexed by image axes (i, j, k[, v]). layout: per axis, (sign, rank)."""
    nd = arr.ndim
    # Storage order: ranks ascending (rank 0 fastest); a '-' axis is stored reversed.
    a = arr
    for ax, (sign, _) in enumerate(layout):
        if sign < 0:
            a = np.flip(a, ax)
    order = sorted(range(nd), key=lambda ax: layout[ax][1])
    stored = np.transpose(a, order[::-1]).astype(dtype)  # slowest first, for C order
    lay = ','.join(('+' if sg > 0 else '-') + str(rk) for sg, rk in layout)
    dt = {'<i2': 'Int16LE', '>i2': 'Int16BE', '<f4': 'Float32LE', '>f4': 'Float32BE', '|u1': 'UInt8',
          '<u2': 'UInt16LE', '<f8': 'Float64LE', '>i4': 'Int32BE'}[np.dtype(dtype).str]
    lines = ['mrtrix image', 'dim: ' + ','.join(map(str, arr.shape)),
             'vox: ' + ','.join(map(str, vox)), 'layout: ' + lay, 'datatype: ' + dt]
    lines += ['transform: ' + ','.join(repr(float(x)) for x in row) for row in transform[:3]]
    if scaling:
        lines.append('scaling: %r,%r' % scaling)
    lines.append('comments: synthetic fixture')
    data = stored.tobytes()
    if sep:
        lines.append(f'file: {sep} {offset}')
        head = ('\n'.join(lines) + '\nEND\n').encode()
        with open(os.path.join(MIF, sep), 'wb') as f:
            f.write(b'\xee' * offset + data)
    else:
        base = len('\n'.join(lines)) + 30
        off = (base + 15) // 16 * 16
        lines.append(f'file: . {off}')
        head = ('\n'.join(lines) + '\nEND\n').encode().ljust(off, b'\0')
        head = head + data
    with (gzip.open if gz else open)(os.path.join(MIF, name), 'wb') as f:
        f.write(head)


A = (np.arange(5 * 4 * 3).reshape(5, 4, 3) * 3 - 50).astype(np.int16)
rotm = np.array([[0.0, 0, 1, -10], [1, 0, 0, 20], [0, 1, 0, -30]])
mif('neg.mif', A, [(-1, 1), (1, 0), (1, 2)], '<i2', (1.5, 2.0, 2.5), rotm)
B = rng.normal(0, 100, (6, 5, 4, 3)).astype(np.float32)
c, s_ = np.cos(0.2), np.sin(0.2)
obl = np.array([[c, -s_, 0, 4], [s_, c, 0, -8], [0, 0, 1, 12]])
mif('vol4d.mih', B, [(1, 3), (-1, 2), (1, 1), (1, 0)], '>f4', (2.0, 2.0, 3.0, 1.0), obl,
    sep='vol4d.dat', offset=40)
C = (np.arange(4 * 3 * 2).reshape(4, 3, 2) % 200).astype(np.uint8)
mif('scaled.mif.gz', C, [(1, 0), (1, 1), (-1, 2)], '|u1', (1.0, 1.0, 1.0), np.eye(4)[:3],
    scaling=(-1.5, 0.25), gz=True)
print('fixtures written to', OUT)
