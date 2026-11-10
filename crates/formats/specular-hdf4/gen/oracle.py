#!/usr/bin/env python3
"""The pyhdf oracle for specular-hdf4.

  oracle.py expected OUT.tsv FILE...      writes what pyhdf reads from each file as TSV
  oracle.py compare DUMP_DIR FILE...      compares hdf4dump output (see examples/hdf4dump.rs)

The TSV has one line per item:  file  kind  name  type  shape  values
kinds:  sds (an SDS's stored values), sattr (an SDS attribute, name "sds/attr"), gattr (a file
attribute), vdata (a Vdata field, name "vdata/field"), vattr (a Vdata attribute), ri (a raster
image read through the GR interface, values in [y][x][component] order).
Numbers are printed exactly (floats by repr of the float64 value); a text attribute's value is
the string with backslash, tab and newline escaped.

`compare` runs on a dump directory made by `hdf4dump FILE DIR` for each file (DIR/<file
basename>/manifest.tsv plus raw little-endian f64 files) and prints mismatch counts.
"""

import ctypes
import glob
import os
import sys

import numpy as np
from pyhdf.HDF import HDF, HC
from pyhdf.SD import SD, SDC
import pyhdf.SD
import pyhdf.V  # noqa: F401
import pyhdf.VS  # noqa: F401

LIBS = os.path.dirname(pyhdf.SD.__file__) + '.libs'
DF = ctypes.CDLL(glob.glob(os.path.join(LIBS, 'libdf-*'))[0], mode=ctypes.RTLD_GLOBAL)
MF = ctypes.CDLL(glob.glob(os.path.join(LIBS, 'libmfhdf-*'))[0], mode=ctypes.RTLD_GLOBAL)

TYPES = {3: 'uchar8', 4: 'char8', 5: 'float32', 6: 'float64', 20: 'int8', 21: 'uint8', 22: 'int16',
         23: 'uint16', 24: 'int32', 25: 'uint32', 26: 'int64', 27: 'uint64'}
DTYPES = {3: 'u1', 4: 'u1', 5: 'f4', 6: 'f8', 20: 'i1', 21: 'u1', 22: 'i2', 23: 'u2', 24: 'i4',
          25: 'u4'}


def esc(s):
    return s.replace('\\', '\\\\').replace('\t', '\\t').replace('\n', '\\n').replace('\r', '\\r')


def num(v):
    if isinstance(v, (float, np.floating)):
        return repr(float(v))
    return str(int(v))


def values(v, nt):
    t = nt & 0xff
    if t in (3, 4) and isinstance(v, str):
        return 's:' + esc(v)
    if isinstance(v, (bytes, np.bytes_)):
        return 's:' + esc(v.decode('latin-1'))
    arr = np.asarray(v)
    if arr.dtype.kind in 'SU':
        return 's:' + esc(''.join(str(x) for x in arr.ravel()))
    return ' '.join(num(x) for x in arr.ravel())


def tname(nt):
    return TYPES.get(nt & 0xff, str(nt)) + ('le' if nt & 0x4000 else '')


def np_type(nt):
    dt = np.dtype(DTYPES[nt & 0xff])
    return dt.newbyteorder('<') if nt & 0x4000 else dt


def attrs_ctypes(obj_id, natt):
    """Every attribute through SDattrinfo/SDreadattr (pyhdf refuses little-endian types)."""
    out = []
    for k in range(natt):
        name = ctypes.create_string_buffer(256)
        nt, count = ctypes.c_int32(), ctypes.c_int32()
        if MF.SDattrinfo(obj_id, k, name, ctypes.byref(nt), ctypes.byref(count)) != 0:
            continue
        nt = nt.value
        buf = np.zeros(count.value, np_type(nt))
        MF.SDreadattr(obj_id, k, buf.ctypes.data_as(ctypes.c_void_p))
        if nt & 0xff in (3, 4):
            v = 's:' + esc(buf.tobytes().decode('latin-1'))
        else:
            v = ' '.join(num(x) for x in buf)
        out.append((name.value.decode('latin-1'), tname(nt), str(count.value), v))
    return sorted(out)


def read_sds(sd):
    out = []
    items = sd.datasets()
    for name in sorted(items):
        idx = items[name][3]
        s = sd.select(idx)
        sname, rank, dims, nt, natt = s.info()
        if not isinstance(dims, list):
            dims = [dims]
        if dims and dims[0] == 0:
            continue  # empty record variable
        try:
            if nt & 0x4000:
                data = np.zeros(dims, np_type(nt))
                start = (ctypes.c_int32 * rank)()
                edge = (ctypes.c_int32 * rank)(*dims)
                assert MF.SDreaddata(s._id, start, None, edge, data.ctypes.data_as(ctypes.c_void_p)) == 0
            else:
                data = s.get()
        except Exception:
            data = None
        if data is not None:
            arr = np.asarray(data)
            if arr.dtype.kind in 'SUO':
                arr = np.frombuffer(b''.join(x if isinstance(x, bytes) else str(x).encode('latin-1')
                                             for x in arr.ravel().tolist()), 'u1')
            out.append(('sds', sname, tname(nt), ','.join(str(d) for d in dims),
                        ' '.join(num(x) for x in arr.ravel())))
        for an, at, ac, av in attrs_ctypes(s._id, natt):
            out.append(('sattr', sname + '/' + an, at, ac, av))
        s.endaccess()
    for an, at, ac, av in attrs_ctypes(sd._id, sd.info()[1]):
        out.append(('gattr', an, at, ac, av))
    return out


def read_vdata(path):
    out = []
    f = HDF(path, HC.READ)
    vs = f.vstart()
    ref = -1
    while True:
        try:
            ref = vs.next(ref)
        except Exception:
            break
        vd = vs.attach(ref)
        name, cls = vd._name, vd._class
        nrec = vd._nrecs
        if not name or nrec == 0 or cls.startswith('Attr0.') or cls.startswith('Dim') or cls in ('_HDF_CHK_TBL_0',) \
                or cls.startswith('RIATTR') or cls.startswith('DimVal') or cls.startswith('Var') \
                or cls.startswith('CDF') or cls.startswith('_HDF_') or name.startswith('_HDF_'):
            vd.detach()
            continue
        fields = vd.inquire()[2]
        recs = vd.read(nrec) if nrec else []
        for k, fname in enumerate(fields):
            col = [r[k] for r in recs]
            info = vd.field(fname)
            ft = info._type
            if ft in (3, 4):
                v = 's:' + esc(''.join(c if isinstance(c, str) else ''.join(map(str, c)) for c in col))
            else:
                flat = []
                for c in col:
                    flat.extend(c if isinstance(c, list) else [c])
                v = ' '.join(num(x) for x in flat)
            out.append(('vdata', name + '/' + fname, tname(ft), '%d,%d' % (nrec, info._order), v))
        for an, (ant, acount, aval, _size) in sorted(vd.attrinfo().items()):
            out.append(('vattr', name + '/' + an, tname(ant), str(acount), values(aval, ant)))
        vd.detach()
    vs.end()
    f.close()
    return out


def read_gr(path):
    out = []
    fid = DF.Hopen(path.encode(), 1, 0)
    if fid < 0:
        return out
    gr = DF.GRstart(fid)
    n, na = ctypes.c_int32(), ctypes.c_int32()
    DF.GRfileinfo(gr, ctypes.byref(n), ctypes.byref(na))
    for i in range(n.value):
        ri = DF.GRselect(gr, i)
        name = ctypes.create_string_buffer(256)
        ncomp, nt, il, nattr = ctypes.c_int32(), ctypes.c_int32(), ctypes.c_int32(), ctypes.c_int32()
        dims = (ctypes.c_int32 * 2)()
        DF.GRgetiminfo(ri, name, ctypes.byref(ncomp), ctypes.byref(nt), ctypes.byref(il), dims,
                       ctypes.byref(nattr))
        w, h, c = dims[0], dims[1], ncomp.value
        dt = np.dtype(DTYPES[nt.value & 0xff]).newbyteorder('=')
        buf = np.zeros((h, w, c), dt)
        DF.GRreqimageil(ri, 0)
        start = (ctypes.c_int32 * 2)(0, 0)
        edge = (ctypes.c_int32 * 2)(w, h)
        st = DF.GRreadimage(ri, start, None, edge, buf.ctypes.data_as(ctypes.c_void_p))
        if st == 0 and il.value != 0 and c == 3 and buf.itemsize == 1:
            # GRreadimage ignores GRreqimageil for some line- and plane-interlaced RIG
            # images (the HDF4 library's grtdfui*.hdf, tdf24.hdf): it returns the stored
            # layout. Read 24-bit images through the DF24 interface, which converts.
            ref = DF.GRidtoref(ri)
            if DF.DF24readref(path.encode(), ref) == 0:
                DF.DF24reqil(0)
                st = DF.DF24getimage(path.encode(), buf.ctypes.data_as(ctypes.c_void_p), w, h)
        if st == 0:
            out.append(('ri', name.value.decode('latin-1'), tname(nt.value), '%d,%d,%d' % (h, w, c),
                        ' '.join(num(x) for x in buf.ravel())))
        DF.GRendaccess(ri)
    DF.GRend(gr)
    DF.Hclose(fid)
    return out


def oracle(path):
    out = []
    try:
        sd = SD(path, SDC.READ)
        out += read_sds(sd)
        sd.end()
    except Exception as e:
        print('sd failed', path, e, file=sys.stderr)
    try:
        out += read_vdata(path)
    except Exception as e:
        print('vs failed', path, e, file=sys.stderr)
    out += read_gr(path)
    return out


def expected(dst, files):
    with open(dst, 'w') as f:
        for p in files:
            base = os.path.basename(p)
            for row in oracle(p):
                f.write(base + '\t' + '\t'.join(row) + '\n')


def parse(vals, typ):
    if vals.startswith('s:'):
        return vals
    if not vals:
        return np.zeros(0)
    return np.array([float(x) for x in vals.split(' ')])


def compare(dump, files):
    """Counts mismatching values between pyhdf and hdf4dump's output."""
    total_items = total_values = bad_items = bad_values = 0
    for p in files:
        base = os.path.basename(p)
        d = os.path.join(dump, base)
        man = os.path.join(d, 'manifest.tsv')
        if not os.path.exists(man):
            print('NO DUMP', base)
            continue
        ours = {}
        for line in open(man, encoding='utf-8'):
            kind, name, typ, shape, vals = line.rstrip('\n').split('\t')
            ours[(kind, name)] = (typ, shape, vals)
        for kind, name, typ, shape, vals in oracle(p):
            if kind == 'vdata' or kind == 'vattr':
                continue  # compared on the fixtures only
            total_items += 1
            o = ours.get((kind, name))
            if o is None:
                print('MISSING', base, kind, name)
                bad_items += 1
                continue
            if vals.startswith('s:'):
                n = 1
                bad = int(vals.rstrip('\\n\x00 ') != o[2].rstrip('\\n\x00 '))
            else:
                exp = parse(vals, typ)
                if o[2].startswith('f:'):
                    got = np.fromfile(os.path.join(d, o[2][2:]), '<f8')
                else:
                    got = parse(o[2], typ)
                n = exp.size
                if got.size != exp.size:
                    bad = n
                elif kind == 'ri' and ('jpeg' in name.lower() or o[0].endswith('jpeg')):
                    bad = int(np.count_nonzero(np.abs(got - exp) > 2))
                else:
                    same = (got == exp) | (np.isnan(got) & np.isnan(exp))
                    bad = int(np.count_nonzero(~same))
            total_values += n
            if bad:
                bad_items += 1
                bad_values += bad
                print('MISMATCH', base, kind, name, bad, 'of', n)
    print('items %d (mismatching %d), values %d (mismatching %d)' %
          (total_items, bad_items, total_values, bad_values))


if __name__ == '__main__':
    if sys.argv[1] == 'expected':
        expected(sys.argv[2], sys.argv[3:])
    else:
        compare(sys.argv[2], sys.argv[3:])
