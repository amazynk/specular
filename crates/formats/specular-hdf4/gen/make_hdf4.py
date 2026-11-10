#!/usr/bin/env python3
"""Writes the synthetic HDF4 fixtures in ../tests/data.

Needs pyhdf (0.11 or later, the binary wheel) and numpy.  The SD, V and VS interfaces come
from pyhdf; little-endian data, chunking, NBIT and the raster interfaces (DFR8, DF24, GR),
which pyhdf does not wrap, are called through ctypes on the HDF4 libraries the wheel bundles.

Every array is a deterministic pattern so the files are byte-for-byte reproducible.  The HDF-EOS2
files are made by hand: SDS and Vgroups laid out as the HDF-EOS2 library does, with
hand-written StructMetadata.0, CoreMetadata.0 and ArchiveMetadata.0.

Run:  python3 make_hdf4.py [out_dir]
"""

import ctypes
import glob
import os
import sys

import numpy as np
from pyhdf.HC import HC
from pyhdf.HDF import HDF
from pyhdf.SD import SD, SDC
import pyhdf.SD
import pyhdf.V  # noqa: F401  (HDF.vgstart needs it imported)
import pyhdf.VS  # noqa: F401

OUT = os.path.abspath(sys.argv[1] if len(sys.argv) > 1 else
                      os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'tests', 'data'))
LIBS = os.path.dirname(pyhdf.SD.__file__) + '.libs'


def lib(prefix):
    path = glob.glob(os.path.join(LIBS, prefix + '-*'))[0]
    return ctypes.CDLL(path, mode=ctypes.RTLD_GLOBAL)


DF = lib('libdf')
MF = lib('libmfhdf')

LITEND = 0x4000
NT = {  # name: (HDF number type, numpy dtype)
    'char8': (SDC.CHAR8, 'S1'),
    'uchar8': (SDC.UCHAR8, 'u1'),
    'int8': (SDC.INT8, 'i1'),
    'uint8': (SDC.UINT8, 'u1'),
    'int16': (SDC.INT16, 'i2'),
    'uint16': (SDC.UINT16, 'u2'),
    'int32': (SDC.INT32, 'i4'),
    'uint32': (SDC.UINT32, 'u4'),
    'float32': (SDC.FLOAT32, 'f4'),
    'float64': (SDC.FLOAT64, 'f8'),
}


def fresh(name):
    """A fresh file in OUT, named relative to it: main() works in OUT, and the SD interface
    records the path it was given in the file (as the name of the CDF0.0 Vgroup)."""
    if os.path.exists(name):
        os.remove(name)
    return name


def pattern(shape, dtype, seed=0):
    n = int(np.prod(shape))
    i = np.arange(n, dtype=np.int64)
    dt = np.dtype(dtype)
    if dt.kind == 'f':
        v = (i * 0.37 + seed) * np.where(i % 3 == 0, -1.0, 1.0) + np.sin(i * 0.1) * 5
        return v.astype(dt).reshape(shape)
    if dt.kind == 'S':
        return np.frombuffer(bytes((32 + (k * 7 + seed) % 90) for k in range(n)), 'S1').reshape(shape)
    info = np.iinfo(dt)
    span = int(info.max) - int(info.min) + 1
    v = (i * 2654435761 + seed * 977) % span + int(info.min)
    return v.astype(dt).reshape(shape)


def write_ctypes(sds, arr):
    """Writes a whole array through SDwritedata (any byte order, the buffer as it is)."""
    arr = np.ascontiguousarray(arr)
    rank = arr.ndim
    start = (ctypes.c_int32 * rank)(*([0] * rank))
    edge = (ctypes.c_int32 * rank)(*arr.shape)
    st = MF.SDwritedata(sds._id, start, None, edge, arr.ctypes.data_as(ctypes.c_void_p))
    assert st == 0, 'SDwritedata failed'


def set_attr_ctypes(obj_id, name, nt, arr):
    arr = np.ascontiguousarray(arr)
    st = MF.SDsetattr(obj_id, name.encode(), nt, arr.size, arr.ctypes.data_as(ctypes.c_void_p))
    assert st == 0, 'SDsetattr failed'


class ChunkDef(ctypes.Structure):
    """HDF_CHUNK_DEF, passed by value: 32 chunk lengths, then the compression or NBIT fields."""
    _fields_ = [('w', ctypes.c_int32 * 40)]


def set_chunk(sds, lengths, comp=0, level=0, nbit=None):
    """SDsetchunk: HDF_CHUNK (1) or HDF_CHUNK | HDF_COMP (3) or HDF_CHUNK | HDF_NBIT (5)."""
    cdef = ChunkDef()
    buf = cdef.w
    for k, n in enumerate(lengths):
        buf[k] = n
    flags = 1
    if comp:
        buf[32] = comp  # comp_type
        buf[33] = 0  # model_type
        buf[34] = level  # cinfo: deflate level or skphuff skp_size
        flags = 3
    if nbit:
        buf[32], buf[33], buf[34], buf[35] = nbit  # start_bit, bit_len, sign_ext, fill_one
        flags = 5
    st = MF.SDsetchunk(sds._id, cdef, flags)
    assert st == 0, 'SDsetchunk failed'


# ---------------------------------------------------------------------------------------------
# Number types, both byte orders, attributes, dimensions, fill values, unlimited dimensions.


def make_types():
    sd = SD(fresh('sds_types.hdf'), SDC.WRITE | SDC.CREATE)
    sd.attr('title').set(SDC.CHAR8, 'Every HDF4 number type')
    sd.attr('history').set(SDC.CHAR8, 'make_hdf4.py')
    sd.attr('global_ints').set(SDC.INT32, [1, -2, 3])
    sd.attr('global_double').set(SDC.FLOAT64, 2.5)
    for k, (name, (nt, dt)) in enumerate(NT.items()):
        shape = (5, 7) if name != 'char8' else (4, 6)
        sds = sd.create('be_' + name, nt, shape)
        if dt == 'S1':
            write_ctypes(sds, pattern(shape, dt, k))
        else:
            sds[:] = pattern(shape, dt, k)
        if name not in ('char8', 'uchar8'):
            sds.attr('one_value').set(nt, pattern((1,), dt, k + 3).tolist())
            sds.attr('three_values').set(nt, pattern((3,), dt, k + 5).tolist())
        sds.attr('long_name').set(SDC.CHAR8, 'big-endian ' + name)
        sds.endaccess()
    for k, (name, (nt, dt)) in enumerate(NT.items()):
        if name in ('char8', 'uchar8'):
            continue
        shape = (3, 4, 5)
        sds = sd.create('le_' + name, nt | LITEND, shape)
        write_ctypes(sds, pattern(shape, '<' + dt, k + 20))
        set_attr_ctypes(sds._id, 'le_values', nt | LITEND, pattern((2,), '<' + dt, k + 40))
        sds.endaccess()

    # Dimensions with names, scales and attributes; a coordinate variable shared by two SDS.
    t = sd.create('temperature', SDC.FLOAT32, (4, 6))
    t[:] = pattern((4, 6), 'f4', 1)
    d0, d1 = t.dim(0), t.dim(1)
    d0.setname('lat')
    d1.setname('lon')
    d0.setscale(SDC.FLOAT32, [10.0, 20.0, 30.0, 40.0])
    d1.setscale(SDC.FLOAT64, [0.0, 1.5, 3.0, 4.5, 6.0, 7.5])
    d0.units = 'degrees_north'
    d1.units = 'degrees_east'
    t.units = 'K'
    t.setfillvalue(-999.0)
    t.setrange(-50.0, 50.0)
    t.endaccess()
    p = sd.create('pressure', SDC.INT16, (4, 6))
    p[:] = pattern((4, 6), 'i2', 2)
    p.dim(0).setname('lat')
    p.dim(1).setname('lon')
    p.setcal(0.5, 0.0, 100.0, 0.0, SDC.INT16)
    p.setfillvalue(-32768)
    p.endaccess()

    # A 1-D, a 4-D and a fill-only SDS (never written).
    one = sd.create('profile', SDC.FLOAT64, (9,))
    one[:] = pattern((9,), 'f8', 3)
    one.endaccess()
    four = sd.create('cube4', SDC.UINT16, (2, 3, 4, 5))
    four[:] = pattern((2, 3, 4, 5), 'u2', 4)
    four.endaccess()
    empty = sd.create('never_written', SDC.INT32, (3, 3))
    empty.setfillvalue(-7)
    empty.endaccess()

    # Unlimited dimension appended in several writes: the data become linked blocks.
    u = sd.create('records', SDC.INT32, (SDC.UNLIMITED, 6))
    u.dim(0).setname('time')
    for r in range(0, 30, 3):
        u[r:r + 3] = pattern((3, 6), 'i4', r)
    u.endaccess()
    u2 = sd.create('records_f', SDC.FLOAT32, (SDC.UNLIMITED,))
    u2.dim(0).setname('time2')
    for r in range(0, 50, 7):
        n = min(7, 50 - r)
        u2[r:r + n] = pattern((n,), 'f4', r)
    u2.endaccess()
    sd.end()


# ---------------------------------------------------------------------------------------------
# Compression and special elements.


def make_comp():
    sd = SD(fresh('sds_comp.hdf'), SDC.WRITE | SDC.CREATE)
    smooth = (np.add.outer(np.arange(40), np.arange(30)) // 3).astype('i2')
    runs = np.repeat(np.arange(12, dtype='u1'), 100).reshape(30, 40)

    def one(name, nt, arr, comp, value=0):
        sds = sd.create(name, nt, arr.shape)
        sds.setcompress(comp, value)
        sds[:] = arr
        sds.endaccess()

    one('deflate_i16', SDC.INT16, smooth, SDC.COMP_DEFLATE, 6)
    one('deflate_f64', SDC.FLOAT64, pattern((20, 10), 'f8', 7), SDC.COMP_DEFLATE, 9)
    one('rle_u8', SDC.UINT8, runs, SDC.COMP_RLE)
    one('rle_i32', SDC.INT32, np.repeat(pattern((40,), 'i4', 3), 10).reshape(20, 20), SDC.COMP_RLE)
    one('skphuff_i16', SDC.INT16, smooth, SDC.COMP_SKPHUFF, 2)
    one('skphuff_u8', SDC.UINT8, runs, SDC.COMP_SKPHUFF, 1)
    one('skphuff_f32', SDC.FLOAT32, pattern((16, 16), 'f4', 9), SDC.COMP_SKPHUFF, 4)

    # NBIT: 12 significant bits starting at bit 13 of an int16 (signed, sign-extended) and
    # 5 bits at bit 6 of a uint8 (zero-filled).
    nb = sd.create('nbit_i16', SDC.INT16, (12, 10))
    MF.SDsetnbitdataset(nb._id, 13, 12, 1, 0)
    nb[:] = ((np.arange(120, dtype='i4') * 37 % 4096) - 2048).astype('i2').reshape(12, 10)
    nb.endaccess()
    nb = sd.create('nbit_u8', SDC.UINT8, (9, 9))
    MF.SDsetnbitdataset(nb._id, 6, 5, 0, 0)
    nb[:] = (np.arange(81, dtype='u1') % 32).reshape(9, 9)
    nb.endaccess()

    # Chunked: plain, deflated and skipping-Huffman chunks; a partly written one whose
    # missing chunks read as fill; ragged edge chunks.
    ch = sd.create('chunked_plain', SDC.FLOAT32, (25, 18))
    set_chunk(ch, (10, 8))
    ch[:] = pattern((25, 18), 'f4', 11)
    ch.endaccess()
    ch = sd.create('chunked_deflate', SDC.UINT16, (32, 32))
    set_chunk(ch, (16, 16), comp=4, level=5)
    ch[:] = pattern((32, 32), 'u2', 12) // 64
    ch.endaccess()
    ch = sd.create('chunked_skphuff', SDC.INT32, (12, 12))
    set_chunk(ch, (6, 6), comp=3, level=4)
    ch[:] = pattern((12, 12), 'i4', 13)
    ch.endaccess()
    ch = sd.create('chunked_partial', SDC.INT16, (20, 20))
    ch.setfillvalue(-5)
    set_chunk(ch, (10, 10))
    ch[0:10, 0:10] = pattern((10, 10), 'i2', 14)
    ch[10:20, 10:20] = pattern((10, 10), 'i2', 15)
    ch.endaccess()
    ch = sd.create('chunked_3d', SDC.UINT8, (4, 9, 7))
    set_chunk(ch, (2, 4, 4), comp=4, level=1)
    ch[:] = pattern((4, 9, 7), 'u1', 16)
    ch.endaccess()
    sd.end()

    # External data in the same folder.
    ext = fresh('sds_ext.dat')
    sd = SD(fresh('sds_ext.hdf'), SDC.WRITE | SDC.CREATE)
    e = sd.create('external_i32', SDC.INT32, (6, 5))
    e.setexternalfile('sds_ext.dat', 16)
    e[:] = pattern((6, 5), 'i4', 17)
    e.endaccess()
    e = sd.create('inline_u8', SDC.UINT8, (3, 3))
    e[:] = pattern((3, 3), 'u1', 18)
    e.endaccess()
    sd.end()
    assert os.path.exists(ext)


# ---------------------------------------------------------------------------------------------
# Vgroups, Vdatas and their attributes.


def make_vgroups():
    p = fresh('vgroups.hdf')
    sd = SD(p, SDC.WRITE | SDC.CREATE)
    a = sd.create('in_group', SDC.FLOAT32, (3, 4))
    a[:] = pattern((3, 4), 'f4', 21)
    ref = a.ref()
    a.endaccess()
    sd.end()

    f = HDF(p, HC.WRITE)
    vs = f.vstart()
    vd = vs.create('Table', (('id', HC.INT32, 1), ('pos', HC.FLOAT32, 3), ('label', HC.CHAR8, 6),
                             ('flag', HC.UINT8, 1), ('big', HC.FLOAT64, 1), ('s16', HC.INT16, 2)))
    vd._class = 'Measurements'
    for k in range(8):
        vd.write([[k, [k * 0.5, -k * 1.5, k * 2.25], 'row%03d' % k, k % 2, k * 1e10, [k, -k]]])
    vd.attr('vd_note').set(HC.CHAR8, 'a vdata attribute')
    vd.field('id').attr('units').set(HC.CHAR8, 'count')
    vd.detach()
    vd = vs.create('Scalars', (('value', HC.UINT16, 1),))
    for k in range(5):
        vd.write([[k * 1000]])
    vd.detach()
    vs.end()

    v = f.vgstart()
    top = v.create('Top Group')
    top._class = 'Collection'
    top.attr('vg_note').set(HC.CHAR8, 'a vgroup attribute')
    top.attr('vg_numbers').set(HC.FLOAT32, [1.5, 2.5])
    child = v.create('Child Group')
    child._class = 'Leaf'
    top.insert(child)
    child.add(HC.DFTAG_NDG, ref)
    vs = f.vstart()
    t = vs.attach('Table')
    child.insert(t)
    t.detach()
    vs.end()
    grand = v.create('Grandchild')
    child.insert(grand)
    grand.detach()
    child.detach()
    top.detach()
    v.end()
    f.close()


# ---------------------------------------------------------------------------------------------
# Raster images: DFR8 (with palettes, RLE, JPEG), DF24 (three interlaces, JPEG) and GR.


class JpegInfo(ctypes.Structure):
    _fields_ = [('quality', ctypes.c_int), ('force_baseline', ctypes.c_int)]


def rgb_image(w, h, seed):
    y, x = np.mgrid[0:h, 0:w]
    r = (x * 255 // max(w - 1, 1)).astype('u1')
    g = (y * 255 // max(h - 1, 1)).astype('u1')
    b = ((x + y + seed) * 9 % 256).astype('u1')
    return np.stack([r, g, b], axis=-1)


def grey_image(w, h, seed):
    y, x = np.mgrid[0:h, 0:w]
    return (((x // 4 + y // 3) * 17 + seed) % 256).astype('u1')


def make_raster():
    p = fresh('raster.hdf').encode()
    pal = np.zeros((256, 3), 'u1')
    pal[:, 0] = np.arange(256)
    pal[:, 1] = 255 - np.arange(256)
    pal[:, 2] = (np.arange(256) * 3) % 256
    pal = np.ascontiguousarray(pal)
    DF.DFR8setpalette(pal.ctypes.data_as(ctypes.c_void_p))
    img = np.ascontiguousarray(grey_image(40, 30, 0))
    assert DF.DFR8addimage(p, img.ctypes.data_as(ctypes.c_void_p), 40, 30, 0) == 0
    img2 = np.ascontiguousarray(np.repeat(grey_image(8, 30, 5), 5, axis=1))
    assert DF.DFR8addimage(p, img2.ctypes.data_as(ctypes.c_void_p), 40, 30, 11) == 0  # RLE
    DF.DFR8setpalette(None)
    q = JpegInfo(90, 1)
    DF.DFR8setcompress(2, ctypes.byref(q))  # COMP_JPEG
    img3 = np.ascontiguousarray(grey_image(48, 32, 9))
    assert DF.DFR8addimage(p, img3.ctypes.data_as(ctypes.c_void_p), 48, 32, 2) == 0

    for il in (0, 1, 2):
        rgb = rgb_image(20, 12, il)
        if il == 1:
            buf = np.ascontiguousarray(rgb.transpose(0, 2, 1))  # line: y, c, x
        elif il == 2:
            buf = np.ascontiguousarray(rgb.transpose(2, 0, 1))  # plane: c, y, x
        else:
            buf = np.ascontiguousarray(rgb)
        DF.DF24setil(il)
        assert DF.DF24addimage(p, buf.ctypes.data_as(ctypes.c_void_p), 20, 12) == 0
    DF.DF24setil(0)
    DF.DF24setcompress(2, ctypes.byref(JpegInfo(85, 1)))
    rgb = np.ascontiguousarray(rgb_image(32, 24, 3))
    assert DF.DF24addimage(p, rgb.ctypes.data_as(ctypes.c_void_p), 32, 24) == 0

    fid = DF.Hopen(p, 3, 0)  # DFACC_RDWR
    assert fid > 0
    gr = DF.GRstart(fid)

    def create(name, ncomp, nt, w, h, il=0):
        dims = (ctypes.c_int32 * 2)(w, h)
        ri = DF.GRcreate(gr, name.encode(), ncomp, nt, il, dims)
        assert ri > 0
        return ri

    def write(ri, arr, w, h):
        arr = np.ascontiguousarray(arr)
        start = (ctypes.c_int32 * 2)(0, 0)
        edge = (ctypes.c_int32 * 2)(w, h)
        assert DF.GRwriteimage(ri, start, None, edge, arr.ctypes.data_as(ctypes.c_void_p)) == 0

    ri = create('gr_rgb', 3, SDC.UINT8, 24, 16)
    write(ri, rgb_image(24, 16, 7), 24, 16)
    DF.GRendaccess(ri)

    ri = create('gr_u16_lut', 1, SDC.UINT16, 16, 10)
    write(ri, pattern((10, 16), 'u2', 30), 16, 10)
    lut = DF.GRgetlutid(ri, 0)
    assert DF.GRwritelut(lut, 3, SDC.UINT8, 0, 256, pal.ctypes.data_as(ctypes.c_void_p)) == 0
    val = np.array([1.25], 'f4')
    DF.GRsetattr(ri, b'gain', SDC.FLOAT32, 1, val.ctypes.data_as(ctypes.c_void_p))
    DF.GRendaccess(ri)

    ri = create('gr_deflate', 1, SDC.INT16, 30, 20)
    lvl = (ctypes.c_int * 8)(6)
    assert DF.GRsetcompress(ri, 4, lvl) == 0
    write(ri, pattern((20, 30), 'i2', 31) // 256, 30, 20)
    DF.GRendaccess(ri)

    ri = create('gr_jpeg', 3, SDC.UINT8, 32, 32)
    assert DF.GRsetcompress(ri, 7, ctypes.byref(JpegInfo(95, 1))) == 0
    write(ri, rgb_image(32, 32, 1), 32, 32)
    DF.GRendaccess(ri)

    ri = create('gr_float', 1, SDC.FLOAT32, 9, 7)
    write(ri, pattern((7, 9), 'f4', 32), 9, 7)
    DF.GRendaccess(ri)

    note = b'raster fixture'
    DF.GRsetattr(gr, b'gr_note', SDC.CHAR8, len(note), note)
    DF.GRend(gr)
    DF.Hclose(fid)


# ---------------------------------------------------------------------------------------------
# HDF-EOS2: a MODIS-like sinusoidal grid, a projection zoo and a swath with dimension maps.

CORE = '''
GROUP                  = INVENTORYMETADATA
  GROUPTYPE            = MASTERGROUP

  GROUP                  = ECSDATAGRANULE

    OBJECT                 = LOCALGRANULEID
      NUM_VAL              = 1
      VALUE                = "MOD09A1.A2020001.h08v05.061.2020010041358.hdf"
    END_OBJECT             = LOCALGRANULEID

    OBJECT                 = PRODUCTIONDATETIME
      NUM_VAL              = 1
      VALUE                = "2020-01-10T04:13:58.000Z"
    END_OBJECT             = PRODUCTIONDATETIME

  END_GROUP              = ECSDATAGRANULE

  GROUP                  = COLLECTIONDESCRIPTIONCLASS

    OBJECT                 = SHORTNAME
      NUM_VAL              = 1
      VALUE                = "MOD09A1"
    END_OBJECT             = SHORTNAME

    OBJECT                 = VERSIONID
      NUM_VAL              = 1
      VALUE                = 61
    END_OBJECT             = VERSIONID

  END_GROUP              = COLLECTIONDESCRIPTIONCLASS

  GROUP                  = RANGEDATETIME

    OBJECT                 = RANGEBEGINNINGDATE
      NUM_VAL              = 1
      VALUE                = "2020-01-01"
    END_OBJECT             = RANGEBEGINNINGDATE

    OBJECT                 = RANGEBEGINNINGTIME
      NUM_VAL              = 1
      VALUE                = "00:00:00.000000"
    END_OBJECT             = RANGEBEGINNINGTIME

    OBJECT                 = RANGEENDINGDATE
      NUM_VAL              = 1
      VALUE                = "2020-01-08"
    END_OBJECT             = RANGEENDINGDATE

    OBJECT                 = RANGEENDINGTIME
      NUM_VAL              = 1
      VALUE                = "23:59:59.999999"
    END_OBJECT             = RANGEENDINGTIME

  END_GROUP              = RANGEDATETIME

  GROUP                  = ASSOCIATEDPLATFORMINSTRUMENTSENSOR

    OBJECT                 = ASSOCIATEDPLATFORMINSTRUMENTSENSORCONTAINER
      CLASS                = "1"

      OBJECT                 = ASSOCIATEDSENSORSHORTNAME
        CLASS                = "1"
        NUM_VAL              = 1
        VALUE                = "MODIS"
      END_OBJECT             = ASSOCIATEDSENSORSHORTNAME

      OBJECT                 = ASSOCIATEDPLATFORMSHORTNAME
        CLASS                = "1"
        NUM_VAL              = 1
        VALUE                = "Terra"
      END_OBJECT             = ASSOCIATEDPLATFORMSHORTNAME

    END_OBJECT             = ASSOCIATEDPLATFORMINSTRUMENTSENSORCONTAINER

  END_GROUP              = ASSOCIATEDPLATFORMINSTRUMENTSENSOR

  GROUP                  = ADDITIONALATTRIBUTES

    OBJECT                 = ADDITIONALATTRIBUTESCONTAINER
      CLASS                = "1"
      OBJECT                 = PARAMETERVALUE
        NUM_VAL              = 1
        CLASS                = "1"
        VALUE                = ("H08", "V05",
            "tile")
      END_OBJECT             = PARAMETERVALUE
    END_OBJECT             = ADDITIONALATTRIBUTESCONTAINER

  END_GROUP              = ADDITIONALATTRIBUTES

END_GROUP              = INVENTORYMETADATA

END
'''

ARCHIVE = '''
GROUP                  = ARCHIVEDMETADATA
  GROUPTYPE            = MASTERGROUP

  OBJECT                 = CHARACTERISTICBINANGULARSIZE
    NUM_VAL              = 1
    VALUE                = 15.0 /* arc seconds */
  END_OBJECT             = CHARACTERISTICBINANGULARSIZE

  OBJECT                 = NORTHBOUNDINGCOORDINATE
    NUM_VAL              = 1
    VALUE                = 39.9999999964079
  END_OBJECT             = NORTHBOUNDINGCOORDINATE

END_GROUP              = ARCHIVEDMETADATA

END
'''


def struct_grids(grids):
    out = ['GROUP=SwathStructure', 'END_GROUP=SwathStructure', 'GROUP=GridStructure']
    for k, g in enumerate(grids, 1):
        out += ['\tGROUP=GRID_%d' % k, '\t\tGridName="%s"' % g['name'], '\t\tXDim=%d' % g['x'],
                '\t\tYDim=%d' % g['y'], '\t\tUpperLeftPointMtrs=(%.6f,%.6f)' % g['ul'],
                '\t\tLowerRightMtrs=(%.6f,%.6f)' % g['lr'], '\t\tProjection=%s' % g['proj']]
        if g.get('zone') is not None:
            out.append('\t\tZoneCode=%d' % g['zone'])
        if g.get('params') is not None:
            out.append('\t\tProjParams=(%s)' % ','.join(repr(float(v)) if v else '0' for v in g['params']))
        out += ['\t\tSphereCode=%d' % g.get('sphere', -1)]
        if 'origin' in g:
            out.append('\t\tGridOrigin=%s' % g['origin'])
        if 'pixreg' in g:
            out.append('\t\tPixelRegistration=%s' % g['pixreg'])
        out += ['\t\tGROUP=Dimension', '\t\tEND_GROUP=Dimension', '\t\tGROUP=DataField']
        for j, (fname, nt, dims) in enumerate(g['fields'], 1):
            out += ['\t\t\tOBJECT=DataField_%d' % j, '\t\t\t\tDataFieldName="%s"' % fname,
                    '\t\t\t\tDataType=%s' % nt,
                    '\t\t\t\tDimList=(%s)' % ','.join('"%s"' % d for d in dims),
                    '\t\t\tEND_OBJECT=DataField_%d' % j]
        out += ['\t\tEND_GROUP=DataField', '\t\tGROUP=MergedFields', '\t\tEND_GROUP=MergedFields',
                '\tEND_GROUP=GRID_%d' % k]
    out += ['END_GROUP=GridStructure', 'GROUP=PointStructure', 'END_GROUP=PointStructure', 'END', '']
    return '\n'.join(out)


def eos_vgroups(p, kind, structs):
    """Lays out the HDF-EOS2 Vgroups: one per grid or swath, holding field Vgroups."""
    f = HDF(p, HC.WRITE)
    v = f.vgstart()
    for name, parts in structs:
        top = v.create(name)
        top._class = kind
        for part, refs in parts:
            g = v.create(part)
            g._class = kind + ' Vgroup'
            for ref in refs:
                g.add(HC.DFTAG_NDG, ref)
            top.insert(g)
            g.detach()
        attrs = v.create(('Grid' if kind == 'GRID' else 'Swath') + ' Attributes')
        attrs._class = kind + ' Vgroup'
        top.insert(attrs)
        attrs.detach()
        top.detach()
    v.end()
    f.close()


# MODIS h08v05 tile corners on the 6371007.181 m sphere.
UL = (-11119505.196667, 4447802.078667)
LR = (-10007554.677000, 3335851.559333)


def make_eos_grid():
    p = fresh('eos_grid.hdf')
    sd = SD(p, SDC.WRITE | SDC.CREATE)
    x, y = 48, 40
    ul, lr = UL, (UL[0] + (LR[0] - UL[0]) * x / 48, UL[1] + (LR[1] - UL[1]) * y / 48)
    refs = []
    yy, xx = np.mgrid[0:y, 0:x]
    refl = (xx * 300 + yy * 37 + 120).astype('i2')
    refl[5:8, 10:14] = -28672
    refl[0, 0:3] = -150  # below valid_range
    s = sd.create('sur_refl_b01', SDC.INT16, (y, x))
    s.dim(0).setname('YDim:MOD_Grid_500m_Surface_Reflectance')
    s.dim(1).setname('XDim:MOD_Grid_500m_Surface_Reflectance')
    s.setcompress(SDC.COMP_DEFLATE, 5)
    s[:] = refl
    s.long_name = 'Surface_reflectance_for_band_1'
    s.units = 'reflectance'
    s.attr('valid_range').set(SDC.INT16, [-100, 16000])
    s.attr('_FillValue').set(SDC.INT16, -28672)
    s.setcal(0.0001, 0.0, 0.0, 0.0, SDC.INT32)
    refs.append(s.ref())
    s.endaccess()
    qa = sd.create('sur_refl_qc_500m', SDC.UINT32, (y, x))
    qa.dim(0).setname('YDim:MOD_Grid_500m_Surface_Reflectance')
    qa.dim(1).setname('XDim:MOD_Grid_500m_Surface_Reflectance')
    qa.setcompress(SDC.COMP_DEFLATE, 5)
    qa[:] = ((xx * 7 + yy * 13) % 1024).astype('u4')
    qa.long_name = '500m Reflectance Band Quality'
    qa.attr('_FillValue').set(SDC.UINT32, 787410671)
    refs.append(qa.ref())
    qa.endaccess()
    # MODIS calibration with a non-zero offset: value = scale * (stored - offset).
    lst = sd.create('LST_Day', SDC.UINT16, (y, x))
    lst.dim(0).setname('YDim:MOD_Grid_500m_Surface_Reflectance')
    lst.dim(1).setname('XDim:MOD_Grid_500m_Surface_Reflectance')
    lst[:] = (14000 + xx * 10 + yy * 3).astype('u2')
    lst.units = 'K'
    lst.attr('valid_range').set(SDC.UINT16, [7500, 65535])
    lst.attr('_FillValue').set(SDC.UINT16, 0)
    lst.setcal(0.02, 0.0, 100.0, 0.0, SDC.UINT16)
    refs.append(lst.ref())
    lst.endaccess()
    meta = struct_grids([{
        'name': 'MOD_Grid_500m_Surface_Reflectance', 'x': x, 'y': y, 'ul': ul, 'lr': lr,
        'proj': 'GCTP_SNSOID', 'params': [6371007.181] + [0] * 12, 'sphere': -1,
        'origin': 'HDFE_GD_UL',
        'fields': [('sur_refl_b01', 'DFNT_INT16', ('YDim', 'XDim')),
                   ('sur_refl_qc_500m', 'DFNT_UINT32', ('YDim', 'XDim')),
                   ('LST_Day', 'DFNT_UINT16', ('YDim', 'XDim'))]}])
    sd.attr('HDFEOSVersion').set(SDC.CHAR8, 'HDFEOS_V2.19')
    sd.attr('StructMetadata.0').set(SDC.CHAR8, meta)
    sd.attr('CoreMetadata.0').set(SDC.CHAR8, CORE)
    sd.attr('ArchiveMetadata.0').set(SDC.CHAR8, ARCHIVE)
    sd.end()
    eos_vgroups(p, 'GRID', [('MOD_Grid_500m_Surface_Reflectance', [('Data Fields', refs)])])


def dms(deg):
    """Packed DMS (DDDMMMSSS.SS) for a decimal-degree value."""
    sign = -1 if deg < 0 else 1
    a = abs(deg)
    d = int(a)
    m = int((a - d) * 60 + 1e-9)
    s = (a - d - m / 60) * 3600
    return sign * (d * 1000000 + m * 1000 + s)


def make_eos_proj():
    p = fresh('eos_proj.hdf')
    sd = SD(p, SDC.WRITE | SDC.CREATE)
    grids = [
        # MODIS climate-modelling grid: GEO with packed-DMS corners.
        {'name': 'CMG_GEO', 'x': 12, 'y': 6, 'proj': 'GCTP_GEO', 'sphere': 0,
         'ul': (dms(-180.0), dms(90.0)), 'lr': (dms(180.0), dms(-90.0)), 'params': None},
        # NSIDC-like north polar stereographic, true scale at 70 N, central meridian -45.
        {'name': 'NP_PS', 'x': 8, 'y': 10, 'proj': 'GCTP_PS', 'sphere': 12,
         'ul': (-3850000.0, 5850000.0), 'lr': (3750000.0, -5350000.0),
         'params': [0, 0, 0, 0, dms(-45.0), dms(70.0), 0, 0, 0, 0, 0, 0, 0]},
        {'name': 'UTM33', 'x': 10, 'y': 8, 'proj': 'GCTP_UTM', 'zone': 33, 'sphere': 12,
         'ul': (300000.0, 5900000.0), 'lr': (400000.0, 5820000.0), 'params': None},
        # EASE-Grid north: Lambert azimuthal on the 6371228 m sphere.
        {'name': 'EASE_N', 'x': 9, 'y': 9, 'proj': 'GCTP_LAMAZ', 'sphere': -1,
         'ul': (-9036842.762, 9036842.762), 'lr': (9036842.762, -9036842.762),
         'params': [6371228.0, 0, 0, 0, 0, dms(90.0), 0, 0, 0, 0, 0, 0, 0]},
        # CONUS Albers equal area on GRS 80.
        {'name': 'CONUS_ALBERS', 'x': 12, 'y': 8, 'proj': 'GCTP_ALBERS', 'sphere': 8,
         'ul': (-2400000.0, 3200000.0), 'lr': (2300000.0, 200000.0),
         'params': [0, 0, dms(29.5), dms(45.5), dms(-96.0), dms(23.0), 0, 0, 0, 0, 0, 0, 0]},
        {'name': 'LCC', 'x': 6, 'y': 5, 'proj': 'GCTP_LAMCC', 'sphere': 12,
         'ul': (-500000.0, 400000.0), 'lr': (500000.0, -400000.0),
         'params': [0, 0, dms(33.0), dms(45.0), dms(-97.0), dms(40.0), 0, 0, 0, 0, 0, 0, 0]},
        {'name': 'MERC', 'x': 6, 'y': 5, 'proj': 'GCTP_MERCAT', 'sphere': 12,
         'ul': (-1000000.0, 1000000.0), 'lr': (1000000.0, -1000000.0),
         'params': [0, 0, 0, 0, dms(10.0), 0, 0, 0, 0, 0, 0, 0, 0]},
        {'name': 'TM', 'x': 6, 'y': 5, 'proj': 'GCTP_TM', 'sphere': 12,
         'ul': (400000.0, 600000.0), 'lr': (600000.0, 400000.0),
         'params': [0, 0, 0.9996, 0, dms(9.0), dms(0.0), 500000.0, 0, 0, 0, 0, 0, 0]},
        {'name': 'ISIN', 'x': 6, 'y': 5, 'proj': 'GCTP_ISINUS', 'sphere': -1,
         'ul': (-1000000.0, 1000000.0), 'lr': (1000000.0, -1000000.0),
         'params': [6371007.181, 0, 0, 0, 0, 0, 0, 0, 86400.0, 0, 1, 0, 0]},
    ]
    for k, g in enumerate(grids):
        name = 'Field_' + g['name']
        s = sd.create(name, SDC.UINT8, (g['y'], g['x']))
        s.dim(0).setname('YDim:' + g['name'])
        s.dim(1).setname('XDim:' + g['name'])
        s[:] = pattern((g['y'], g['x']), 'u1', k)
        g['ref'] = s.ref()
        g['fields'] = [(name, 'DFNT_UINT8', ('YDim', 'XDim'))]
        s.endaccess()
    sd.attr('StructMetadata.0').set(SDC.CHAR8, struct_grids(grids))
    sd.end()
    eos_vgroups(p, 'GRID', [(g['name'], [('Data Fields', [g['ref']])]) for g in grids])


SWATH = '''GROUP=SwathStructure
	GROUP=SWATH_1
		SwathName="Swath_Test"
		GROUP=Dimension
			OBJECT=Dimension_1
				DimensionName="GeoTrack"
				Size=6
			END_OBJECT=Dimension_1
			OBJECT=Dimension_2
				DimensionName="GeoXtrack"
				Size=5
			END_OBJECT=Dimension_2
			OBJECT=Dimension_3
				DimensionName="DataTrack"
				Size=12
			END_OBJECT=Dimension_3
			OBJECT=Dimension_4
				DimensionName="DataXtrack"
				Size=10
			END_OBJECT=Dimension_4
			OBJECT=Dimension_5
				DimensionName="Band"
				Size=3
			END_OBJECT=Dimension_5
		END_GROUP=Dimension
		GROUP=DimensionMap
			OBJECT=DimensionMap_1
				GeoDimension="GeoTrack"
				DataDimension="DataTrack"
				Offset=0
				Increment=2
			END_OBJECT=DimensionMap_1
			OBJECT=DimensionMap_2
				GeoDimension="GeoXtrack"
				DataDimension="DataXtrack"
				Offset=1
				Increment=2
			END_OBJECT=DimensionMap_2
		END_GROUP=DimensionMap
		GROUP=IndexDimensionMap
		END_GROUP=IndexDimensionMap
		GROUP=GeoField
			OBJECT=GeoField_1
				GeoFieldName="Latitude"
				DataType=DFNT_FLOAT32
				DimList=("GeoTrack","GeoXtrack")
			END_OBJECT=GeoField_1
			OBJECT=GeoField_2
				GeoFieldName="Longitude"
				DataType=DFNT_FLOAT32
				DimList=("GeoTrack","GeoXtrack")
			END_OBJECT=GeoField_2
		END_GROUP=GeoField
		GROUP=DataField
			OBJECT=DataField_1
				DataFieldName="Brightness"
				DataType=DFNT_UINT16
				DimList=("DataTrack","DataXtrack")
			END_OBJECT=DataField_1
			OBJECT=DataField_2
				DataFieldName="Cloud_Mask"
				DataType=DFNT_INT8
				DimList=("GeoTrack","GeoXtrack")
			END_OBJECT=DataField_2
			OBJECT=DataField_3
				DataFieldName="Radiance"
				DataType=DFNT_FLOAT32
				DimList=("Band","DataTrack","DataXtrack")
			END_OBJECT=DataField_3
		END_GROUP=DataField
		GROUP=MergedFields
		END_GROUP=MergedFields
	END_GROUP=SWATH_1
END_GROUP=SwathStructure
GROUP=GridStructure
END_GROUP=GridStructure
GROUP=PointStructure
END_GROUP=PointStructure
END
'''


def make_eos_swath():
    p = fresh('eos_swath.hdf')
    sd = SD(p, SDC.WRITE | SDC.CREATE)
    t, x = np.mgrid[0:6, 0:5]
    lat = (50.0 - t * 0.5 + x * 0.05).astype('f4')
    lon = (5.0 + x * 0.8 + t * 0.1).astype('f4')
    geo, data = [], []

    def field(name, nt, arr, dims, out):
        s = sd.create(name, nt, arr.shape)
        for k, d in enumerate(dims):
            s.dim(k).setname(d + ':Swath_Test')
        s[:] = arr
        out.append(s.ref())
        return s

    s = field('Latitude', SDC.FLOAT32, lat, ('GeoTrack', 'GeoXtrack'), geo)
    s.units = 'degrees_north'
    s.endaccess()
    s = field('Longitude', SDC.FLOAT32, lon, ('GeoTrack', 'GeoXtrack'), geo)
    s.units = 'degrees_east'
    s.endaccess()
    yy, xx = np.mgrid[0:12, 0:10]
    s = field('Brightness', SDC.UINT16, (yy * 100 + xx * 7).astype('u2'), ('DataTrack', 'DataXtrack'), data)
    s.scale_factor = 0.01
    s.add_offset = 0.0
    s.endaccess()
    s = field('Cloud_Mask', SDC.INT8, ((t + x) % 3 - 1).astype('i1'), ('GeoTrack', 'GeoXtrack'), data)
    s.endaccess()
    s = field('Radiance', SDC.FLOAT32, pattern((3, 12, 10), 'f4', 5),
              ('Band', 'DataTrack', 'DataXtrack'), data)
    s.endaccess()
    sd.attr('HDFEOSVersion').set(SDC.CHAR8, 'HDFEOS_V2.19')
    sd.attr('StructMetadata.0').set(SDC.CHAR8, SWATH)
    sd.end()
    eos_vgroups(p, 'SWATH', [('Swath_Test', [('Geolocation Fields', geo), ('Data Fields', data)])])


def main():
    os.makedirs(OUT, exist_ok=True)
    os.chdir(OUT)  # external files are written relative to the working directory
    make_types()
    make_comp()
    make_vgroups()
    make_raster()
    make_eos_grid()
    make_eos_proj()
    make_eos_swath()
    for f in sorted(os.listdir(OUT)):
        if f.endswith('.hdf') or f.endswith('.dat'):
            print('%8d  %s' % (os.path.getsize(f), f))


if __name__ == '__main__':
    main()
