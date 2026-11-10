"""Writes the synthetic JCAMP-DX and SPC fixtures under tests/data, with what each holds in
tests/data/expect.json (page titles, x and y of the first trace of each page, peaks, and
for refusals the word the message must contain). Deterministic: run it again and nothing
changes.

  python3 -I crates/formats/specular-jcamp/tests/gen_fixtures.py
"""
import json
import math
import os
import struct

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, 'data')
EXPECT = {}


def ys(n, seed=1):
    """Integer ordinates: a smooth curve with peaks, runs of equal values and steps."""
    out = []
    for i in range(n):
        v = 2000 + int(1500 * math.sin(i * 0.05 * seed)) + int(4000 * math.exp(-((i - n * 0.6) / 4.0) ** 2))
        if 10 <= i < 16:
            v = 777  # a run: DUP
        out.append(v)
    return out


# ---------------------------------------------------------------------------------------------
# JCAMP-DX encoders.

SQZ_POS = '@ABCDEFGHI'
SQZ_NEG = '@abcdefghi'
DIF_POS = '%JKLMNOPQR'
DIF_NEG = '%jklmnopqr'
DUP = 'STUVWXYZs'


def sqz(v):
    s = str(abs(v))
    return (SQZ_NEG if v < 0 else SQZ_POS)[int(s[0])] + s[1:]


def dif(v):
    s = str(abs(v))
    return (DIF_NEG if v < 0 else DIF_POS)[int(s[0])] + s[1:]


def dup(n):
    s = str(n)
    return DUP[int(s[0]) - 1] + s[1:]


def with_dup(tokens):
    """Folds runs of equal tokens into DUP counts."""
    out = []
    i = 0
    while i < len(tokens):
        j = i
        while j + 1 < len(tokens) and tokens[j + 1] == tokens[i]:
            j += 1
        out.append(tokens[i])
        if j > i:
            out.append(dup(j - i + 1))
        i = j + 1
    return out


def encode(y, form, per_line, x0, step):
    """Data lines of an (X++(Y..Y)) table: x written as the abscissa of the line's first
    ordinate (of its check value in DIF forms)."""
    lines = []
    i = 0
    n = len(y)
    while i < n:
        chunk = y[i:i + per_line]
        if form == 'AFFN':
            line = '%g ' % (x0 + step * i) + ' '.join(str(v) for v in chunk)
        elif form == 'PAC':
            line = '%g' % (x0 + step * i) + ''.join(('+' if v >= 0 else '') + str(v) for v in chunk)
        elif form == 'SQZ':
            line = '%g' % (x0 + step * i) + ''.join(sqz(v) for v in chunk)
        elif form == 'SQZDUP':
            line = '%g' % (x0 + step * i) + ''.join(with_dup([sqz(v) for v in chunk]))
        elif form in ('DIF', 'DIFDUP'):
            # The line starts with an absolute value; after a line ending in DIF form, that is
            # the previous line's last ordinate again (the Y-check), at its abscissa.
            if i == 0:
                start, toks = 0, [sqz(chunk[0])] + [dif(chunk[k] - chunk[k - 1]) for k in range(1, len(chunk))]
            else:
                start = i - 1
                prev = y[i - 1]
                toks = [sqz(prev)] + [dif(chunk[0] - prev)] + [dif(chunk[k] - chunk[k - 1]) for k in range(1, len(chunk))]
            if form == 'DIFDUP':
                toks = [toks[0]] + with_dup(toks[1:])
            line = '%g' % (x0 + step * start) + ''.join(toks)
        else:
            raise ValueError(form)
        lines.append(line)
        i += per_line
    return lines


def jdx(name, text, expect, raw=False):
    data = text if raw else text.encode('ascii')
    with open(os.path.join(OUT, name), 'wb') as f:
        f.write(data)
    EXPECT[name] = expect


def xydata_file(name, form, n=60, yfactor=0.5, per_line=10, extra=''):
    y = ys(n)
    x0, step = 4000.0, -2.0
    lines = encode(y, form, per_line, x0, step)
    text = '\n'.join([
        '##TITLE= %s' % name,
        '##JCAMP-DX= 4.24 $$ fixture',
        '##DATA TYPE= INFRARED SPECTRUM',
        '##ORIGIN= Specular fixtures',
        '##OWNER= public domain',
        '##XUNITS= 1/CM',
        '##YUNITS= ABSORBANCE',
        '##XFACTOR= 1',
        '##YFACTOR= %g' % yfactor,
        '##FIRSTX= %g' % x0,
        '##LASTX= %g' % (x0 + step * (n - 1)),
        '##NPOINTS= %d' % n,
        '##FIRSTY= %g' % (y[0] * yfactor),
        extra.rstrip('\n'),
        '##XYDATA= (X++(Y..Y))',
    ] + lines + ['##END=', ''])
    text = text.replace('\n\n', '\n')
    jdx(name, text, {'pages': [{
        'x': [x0 + step * i for i in range(n)],
        'y': [v * yfactor for v in y],
        'xunit': 'cm⁻¹',
    }], 'warnings': 0})


def jcamp_fixtures():
    for form in ('AFFN', 'PAC', 'SQZ', 'SQZDUP', 'DIF', 'DIFDUP'):
        xydata_file('ir_%s.jdx' % form.lower(), form)

    # A Y-check that fails: the check value is off by one.
    y = ys(30)
    lines = encode(y, 'DIF', 10, 0, 1)
    # Line 2 starts with the check value of point 9, one too high.
    lines[1] = '9' + sqz(y[9] + 1) + ''.join(dif(y[k] - y[k - 1]) for k in range(10, 20))
    text = '\n'.join(['##TITLE= ycheck', '##JCAMP-DX= 5.01', '##XUNITS= NANOMETERS', '##YUNITS= ARBITRARY UNITS',
                      '##FIRSTX= 0', '##LASTX= 29', '##NPOINTS= 30', '##XYDATA= (X++(Y..Y))'] + lines + ['##END=', ''])
    jdx('ycheck_fail.jdx', text, {'pages': [{'x': list(range(30)), 'y': y}], 'warning': 'Y-check'})

    # XYPOINTS with XFACTOR and YFACTOR, comma and semicolon separators.
    pts = [(1.5, 10), (2.25, 20), (4.0, -5), (8.0, 1e3)]
    body = ' '.join('%g,%g;' % (x / 0.25, y / 2) for x, y in pts)
    text = '\n'.join(['##TITLE= points', '##JCAMP-DX= 5.00', '##DATA TYPE= UV/VIS SPECTRUM', '##XUNITS= NANOMETERS',
                      '##YUNITS= ABSORBANCE', '##XFACTOR= 0.25', '##YFACTOR= 2', '##NPOINTS= 4',
                      '##XYPOINTS= (XY..XY)', body, '##END=', ''])
    jdx('uv_xypoints.jdx', text, {'pages': [{'x': [p[0] for p in pts], 'y': [p[1] for p in pts], 'xunit': 'nm'}]})

    # A mass spectrum as a peak table (XYW..XYW): sticks, the largest peaks labelled.
    pk = [(15, 120, 1), (29, 999, 1), (31, 450, 2), (45, 60, 1), (46, 210, 1)]
    text = '\n'.join(['##TITLE= ethanol ms', '##JCAMP-DX= 5.01', '##DATA TYPE= MASS SPECTRUM', '##DATA CLASS= PEAK TABLE',
                      '##XUNITS= M/Z', '##YUNITS= RELATIVE ABUNDANCE', '##NPOINTS= 5', '##PEAK TABLE= (XYW..XYW)']
                     + ['%d,%d,%d' % p for p in pk] + ['##END=', ''])
    jdx('ms_peaks.jdx', text, {'pages': [{'x': [p[0] for p in pk], 'y': [p[1] for p in pk], 'sticks': True,
                                           'peak0': 29}]})

    # NTUPLES NMR spectrum: real and imaginary pages, Hz shown in ppm.
    n = 32
    re = ys(n, 2)
    im = [v // 3 - 500 for v in ys(n, 3)]
    first, last, mhz = 3200.0, 0.0, 400.0
    text = '\n'.join([
        '##TITLE= ntuples nmr', '##JCAMP-DX= 5.00', '##DATA TYPE= NMR SPECTRUM', '##DATA CLASS= NTUPLES',
        '##.OBSERVE FREQUENCY= %g' % mhz, '##.OBSERVE NUCLEUS= ^1H', '##.SOLVENT NAME= CDCl3',
        '##NTUPLES= NMR SPECTRUM',
        '##VAR_NAME= FREQUENCY, SPECTRUM/REAL, SPECTRUM/IMAG, PAGE NUMBER',
        '##SYMBOL= X, R, I, N', '##VAR_TYPE= INDEPENDENT, DEPENDENT, DEPENDENT, PAGE',
        '##VAR_FORM= AFFN, ASDF, ASDF, AFFN', '##VAR_DIM= %d, %d, %d, 2' % (n, n, n),
        '##UNITS= HZ, ARBITRARY UNITS, ARBITRARY UNITS,',
        '##FIRST= %g, %d, %d, 1' % (first, re[0], im[0]), '##LAST= %g, %d, %d, 2' % (last, re[-1], im[-1]),
        '##FACTOR= 1, 0.5, 0.25, 1',
        '##PAGE= N=1', '##DATA TABLE= (X++(R..R)), XYDATA'] + encode(re, 'DIFDUP', 8, first, (last - first) / (n - 1))
        + ['##PAGE= N=2', '##DATA TABLE= (X++(I..I)), XYDATA'] + encode(im, 'DIF', 8, first, (last - first) / (n - 1))
        + ['##END NTUPLES= NMR SPECTRUM', '##END=', ''])
    step = (last - first) / (n - 1)
    jdx('nmr_ntuples.jdx', text, {'pages': [{'x': [(first + step * i) / mhz for i in range(n)],
                                              'y': [v * 0.5 for v in re], 'xunit': 'ppm'}],
                                   'nucleus': '1H', 'solvent': 'CDCl3'})

    # NTUPLES mass spectra: a page of peaks per retention time.
    scans = [[(28, 100), (32, 25)], [(18, 40), (28, 90), (44, 15)]]
    lines = ['##TITLE= gc-ms', '##JCAMP-DX= 5.01', '##DATA TYPE= MASS SPECTRUM', '##DATA CLASS= NTUPLES',
             '##NTUPLES= MASS SPECTRUM', '##VAR_NAME= MASS, INTENSITY, RETENTION TIME', '##SYMBOL= X, Y, T',
             '##VAR_TYPE= INDEPENDENT, DEPENDENT, INDEPENDENT', '##VAR_FORM= AFFN, AFFN, AFFN',
             '##VAR_DIM= 3, 3, 2', '##UNITS= M/Z, RELATIVE ABUNDANCE, SECONDS', '##FACTOR= 1, 1, 1']
    for k, sc in enumerate(scans):
        lines += ['##PAGE= T=%g' % (1.5 * (k + 1)), '##NPOINTS= %d' % len(sc), '##DATA TABLE= (XY..XY), PEAKS']
        lines += ['%d, %d' % p for p in sc]
    lines += ['##END NTUPLES= MASS SPECTRUM', '##END=', '']
    jdx('ms_ntuples.jdx', '\n'.join(lines), {'pages': [
        {'title': 'T=1.5', 'x': [p[0] for p in scans[0]], 'y': [p[1] for p in scans[0]], 'sticks': True},
        {'title': 'T=3', 'x': [p[0] for p in scans[1]], 'y': [p[1] for p in scans[1]], 'sticks': True}]})

    # LINK: two spectra in one file, with CR-only line ends (old Macintosh) and Latin-1 text.
    y1, y2 = ys(20), ys(20, 4)
    blk = lambda t, y: ['##TITLE= %s' % t, '##JCAMP-DX= 4.24', '##DATA TYPE= INFRARED SPECTRUM', '##XUNITS= 1/CM',
                        '##YUNITS= TRANSMITTANCE', '##FIRSTX= 1000', '##LASTX= 1019', '##NPOINTS= 20',
                        '##XYDATA= (X++(Y..Y))'] + encode(y, 'SQZ', 7, 1000, 1) + ['##END=']
    text = '\r'.join(['##TITLE= link file', '##JCAMP-DX= 4.24', '##DATA TYPE= LINK', '##BLOCKS= 2',
                      '##ORIGIN= Lab B\xe9ziers'] + blk('first', y1) + blk('second', y2) + ['##END=', ''])
    jdx('link_cr_latin1.jdx', text.encode('latin-1'), {'pages': [
        {'title': 'first', 'x': list(range(1000, 1020)), 'y': y1},
        {'title': 'second', 'x': list(range(1000, 1020)), 'y': y2}], 'origin': 'Lab Béziers'}, raw=True)

    # UTF-16 with a byte order mark.
    t = '##TITLE= utf16\r\n##XUNITS= 1/CM\r\n##FIRSTX= 1\r\n##LASTX= 3\r\n##NPOINTS= 3\r\n##XYDATA= (X++(Y..Y))\r\n1 5 6 7\r\n##END=\r\n'
    jdx('utf16.jdx', b'\xff\xfe' + t.encode('utf-16-le'), {'pages': [{'x': [1, 2, 3], 'y': [5, 6, 7]}]}, raw=True)

    # Refusals.
    jdx('structure_only.jdx', '##TITLE= benzene\n##JCAMP-CS= 3.7\n##MOLFORM= C 6 H 6\n##ATOMLIST=\n##END=\n',
        {'refused': 'JCAMP-CS'})
    jdx('no_data.jdx', '##TITLE= nothing\n##JCAMP-DX= 5.01\n##DATA TYPE= INFRARED SPECTRUM\n##END=\n',
        {'refused': 'no spectrum'})
    # Cut short: no ##END=, fewer points than declared.
    y = ys(25)
    text = '\n'.join(['##TITLE= cut', '##XUNITS= 1/CM', '##FIRSTX= 0', '##LASTX= 24', '##NPOINTS= 25',
                      '##XYDATA= (X++(Y..Y))'] + encode(y, 'DIF', 10, 0, 1)[:2] + [''])
    jdx('truncated.jdx', text, {'pages': [{'x': list(range(20)), 'y': y[:20]}], 'warning': '##END'})


# ---------------------------------------------------------------------------------------------
# SPC.

def spc_header(flags, exp, npts, first, last, nsub, xt=1, yt=2, zt=0, exper=4, ver=0x4B, be=False,
               catxt=b'', logoff=0, cmnt=b'Specular fixture', zinc=0.0):
    e = '>' if be else '<'
    h = bytearray(512)
    h[0] = flags
    h[1] = ver
    h[2] = exper
    h[3] = exp & 0xFF
    struct.pack_into(e + 'I', h, 4, npts)
    struct.pack_into(e + 'd', h, 8, first)
    struct.pack_into(e + 'd', h, 16, last)
    struct.pack_into(e + 'I', h, 24, nsub)
    h[28], h[29], h[30] = xt, yt, zt
    struct.pack_into(e + 'I', h, 32, (2021 << 20) | (6 << 16) | (15 << 11) | (9 << 6) | 30)
    h[36:36 + 4] = b'4 cm'
    h[45:45 + 7] = b'FT-IR 1'
    h[88:88 + len(cmnt)] = cmnt
    h[218:218 + len(catxt)] = catxt
    struct.pack_into(e + 'I', h, 248, logoff)
    struct.pack_into(e + 'f', h, 312, zinc)
    return h


def subhdr(exp=0, idx=0, time=0.0, nxt=0.0, npts=0, be=False):
    e = '>' if be else '<'
    s = bytearray(32)
    s[1] = exp & 0xFF
    struct.pack_into(e + 'H', s, 2, idx)
    struct.pack_into(e + 'f', s, 4, time)
    struct.pack_into(e + 'f', s, 8, nxt)
    struct.pack_into(e + 'I', s, 16, npts)
    return s


def ints(vals, exp, short=False, be=False):
    e = '>' if be else '<'
    if exp == -128:
        return struct.pack(e + '%df' % len(vals), *vals), [float(struct.unpack('<f', struct.pack('<f', v))[0]) for v in vals]
    bits = 16 if short else 32
    raw = [int(round(v / 2.0 ** (exp - bits))) for v in vals]
    data = struct.pack(e + ('%dh' if short else '%di') % len(raw), *raw)
    return data, [r * 2.0 ** (exp - bits) for r in raw]


def f32(v):
    return struct.unpack('<f', struct.pack('<f', v))[0]


def spc(name, data, expect):
    with open(os.path.join(OUT, name), 'wb') as f:
        f.write(bytes(data))
    EXPECT[name] = expect


def linspace(a, b, n):
    return [a + (b - a) / (n - 1) * i for i in range(n)]


def spc_fixtures():
    n = 40
    vals = [v / 1000.0 for v in ys(n)]
    # Floats, evenly spaced, with a log block.
    log_text = b'Operator=Ada\r\nSample=polystyrene film\r\n\x00'
    body, y = ints(vals, -128)
    logoff = 512 + 32 + len(body)
    logblk = struct.pack('<IIIII44s', 64 + len(log_text), 64 + len(log_text), 64, 0, 0, b'') + log_text
    d = spc_header(0, -128, n, 4000.0, 400.0, 1, logoff=logoff) + subhdr(-128) + body + logblk
    spc('float_log.spc', d, {'pages': [{'x': linspace(4000.0, 400.0, n), 'y': y, 'xunit': 'cm⁻¹'}],
                             'field': ['Operator', 'Ada']})
    # 32-bit integers with an exponent, and 16-bit ones (TSPREC).
    body, y = ints(vals, 4)
    spc('int32.spc', spc_header(0, 4, n, 400.0, 800.0, 1, xt=3, yt=128, exper=6) + subhdr(3) + body,
        {'pages': [{'x': linspace(400.0, 800.0, n), 'y': y, 'xunit': 'nm'}]})
    body, y = ints(vals, 4, short=True)
    spc('int16.spc', spc_header(0x01, 4, n, 400.0, 800.0, 1, xt=3) + subhdr(4) + body,
        {'pages': [{'x': linspace(400.0, 800.0, n), 'y': y}]})
    # A negative exponent (the specification's signed char).
    body, y = ints([v / 100 for v in vals], -2)
    spc('negexp.spc', spc_header(0, -2, n, 0.0, 39.0, 1, xt=0) + subhdr(-2) + body,
        {'pages': [{'x': linspace(0.0, 39.0, n), 'y': y}]})
    # One X array for all (TXVALS).
    xs = [f32(100 + i * i * 0.5) for i in range(n)]
    body, y = ints(vals, -128)
    spc('txvals.spc', spc_header(0x80, -128, n, 0, 0, 1, xt=13, exper=10) + struct.pack('<%df' % n, *xs) + subhdr(-128) + body,
        {'pages': [{'x': xs, 'y': y, 'xunit': 'cm⁻¹'}]})
    # Multi-file, evenly spaced Z, each subfile its own exponent (TMULTI).
    d = spc_header(0x04, 0, n, 1000.0, 2000.0, 3, zt=5, zinc=0.5)
    pages = []
    for k, exp in enumerate((4, 5, -128)):
        body, y = ints([v * (k + 1) for v in vals], exp)
        d += subhdr(exp, k) + body
        pages.append({'title': 'Time %g min' % (0.5 * k), 'x': linspace(1000.0, 2000.0, n), 'y': y})
    spc('multi_even.spc', d, {'pages': pages})
    # Multi-file with Z values per subfile (TMULTI | TRANDM).
    d = spc_header(0x0C, -128, n, 1000.0, 2000.0, 2, zt=4)
    pages = []
    for k, t in enumerate((3.25, 1.5)):
        body, y = ints([v - k for v in vals], -128)
        d += subhdr(-128, k, t) + body
        pages.append({'title': 'Time %g s' % t, 'x': linspace(1000.0, 2000.0, n), 'y': y})
    spc('multi_random.spc', d, {'pages': pages})
    # X and Y per subfile (TXYXYS), sequential and with a directory.
    for name, directory in (('xyxy.spc', False), ('xyxy_dir.spc', True)):
        subs = []
        for k, m in enumerate((5, 8)):
            xs = [f32(10.0 + 7 * i + k) for i in range(m)]
            body, y = ints([float(i * i + k) for i in range(m)], -128)
            subs.append((subhdr(-128, k, npts=m), struct.pack('<%df' % m, *xs), body, xs, y))
        if directory:
            dir_at = 512
            pos = dir_at + 12 * len(subs)
            d = spc_header(0x44 | 0x80 * 0, -128, dir_at, 0, 0, len(subs), xt=9, exper=8)
            dirb = b''
            for s in subs:
                size = 32 + len(s[1]) + len(s[2])
                dirb += struct.pack('<iif', pos, size, 0.0)
                pos += size
            d += dirb
        else:
            d = spc_header(0x44, -128, 0, 0, 0, len(subs), xt=9, exper=8)
        for s in subs:
            d += s[0] + s[1] + s[2]
        spc(name, d, {'pages': [{'x': s[3], 'y': s[4], 'sticks': False} for s in subs]})
    # Axis labels of their own (TALABS).
    body, y = ints(vals, -128)
    spc('talabs.spc', spc_header(0x20, -128, n, 1.0, 40.0, 1, catxt=b'Distance (mm)\x00Height\x00\x00') + subhdr(-128) + body,
        {'pages': [{'x': linspace(1.0, 40.0, n), 'y': y, 'xtitle': 'Distance', 'xunit': 'mm'}]})
    # Most significant byte first (0x4C).
    body, y = ints(vals, 4, be=True)
    spc('msb.spc', spc_header(0, 4, n, 4000.0, 400.0, 1, ver=0x4C, be=True) + subhdr(4, be=True) + body,
        {'pages': [{'x': linspace(4000.0, 400.0, n), 'y': y}]})
    # Old format (0x4D): header 256 bytes with the first subfile header inside; integers with
    # their 16-bit halves swapped. Two subfiles.
    def old(nsub):
        h = bytearray(256)
        h[0] = 0x04 if nsub > 1 else 0
        h[1] = 0x4D
        struct.pack_into('<h', h, 2, 6)
        struct.pack_into('<fff', h, 4, float(n), 500.0, 1500.0)
        h[16], h[17] = 3, 2
        struct.pack_into('<H', h, 18, 1994)
        h[20:24] = bytes([5, 17, 12, 0])
        h[64:64 + 10] = b'old format'
        out = bytes(h[:224])
        pages = []
        for k in range(nsub):
            raw = [int(round(v * (k + 1) / 2.0 ** (6 - 32))) for v in vals]
            data = b''.join(bytes([be[1], be[0], be[3], be[2]]) for be in (struct.pack('>i', r) for r in raw))
            # Check the swap against the reader's rule: value = b1<<24 | b0<<16 | b3<<8 | b2.
            for r, i in zip(raw, range(0, len(data), 4)):
                b = data[i:i + 4]
                v = struct.unpack('>i', bytes([b[1], b[0], b[3], b[2]]))[0]
                assert v == r, (v, r)
            sub = bytearray(32)
            sub[1] = 6
            out += bytes(sub) + data
            pages.append({'x': linspace(500.0, 1500.0, n), 'y': [r * 2.0 ** (6 - 32) for r in raw]})
        return out, pages
    d, pages = old(1)
    spc('old.spc', d, {'pages': pages})
    d, pages = old(2)
    spc('old_multi.spc', d, {'pages': pages})
    # Refused: a Shimadzu .spc (an OLE compound document).
    spc('shimadzu.spc', bytes([0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]) + bytes(504), {'refused': 'Shimadzu'})
    # Cut short inside the data.
    body, y = ints(vals, -128)
    spc('truncated.spc', spc_header(0, -128, n, 0.0, 1.0, 1) + subhdr(-128) + body[:40], {'refused': 'cut short'})


def main():
    os.makedirs(OUT, exist_ok=True)
    jcamp_fixtures()
    spc_fixtures()
    with open(os.path.join(OUT, 'expect.json'), 'w') as f:
        json.dump(EXPECT, f, indent=1, sort_keys=True, ensure_ascii=False)
        f.write('\n')
    print('%d fixtures, %d bytes' % (len(EXPECT), sum(os.path.getsize(os.path.join(OUT, k)) for k in EXPECT)))


main()
