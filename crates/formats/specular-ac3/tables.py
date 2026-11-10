import re, sys
# Usage: python3 -I tables.py A52-SPEC.txt (the ATSC A/52 specification as plain text)
L = open(sys.argv[1]).read().split('\n')


def find(title):
    for i in range(1000, len(L)):
        if title in L[i]:
            return i
    raise SystemExit('missing ' + title)


def grid(title, n, conv):
    """Rows 'A=k v0 v1 ...' after the title."""
    i = find(title)
    vals = []
    while len(vals) < n:
        i += 1
        m = re.match(r'\s*A=(\d+)\s+(.*)', L[i])
        if m:
            vals += [conv(t) for t in m.group(2).split()]
    return vals[:n]


out = []


def emit(name, ty, vals, per=16):
    out.append(f'pub(crate) const {name}: [{ty}; {len(vals)}] = [')
    for k in range(0, len(vals), per):
        out.append('    ' + ', '.join(str(v) for v in vals[k:k + per]) + ',')
    out.append('];')


masktab = grid('Table 7.13 Bin Number to Band Number Table', 256, int)
latab = grid('Table 7.14 Log-Addition Table', 256, lambda t: int(t, 16))
i = find('Table 7.15 Hearing Threshold Table')
hth = [[None] * 50 for _ in range(3)]
cnt = 0
while cnt < 25:
    i += 1
    t = L[i].split()
    if len(t) == 8 and t[0].isdigit() and t[1].startswith('0x'):
        a, b = int(t[0]), int(t[4])
        for f in range(3):
            hth[f][a] = int(t[1 + f], 16)
            hth[f][b] = int(t[5 + f], 16)
        cnt += 1
i = find('Table 7.16 Bit Allocation Pointer Table')
bap = [None] * 64
cnt = 0
while cnt < 32:
    i += 1
    t = L[i].split()
    if len(t) == 4 and all(x.isdigit() for x in t):
        bap[int(t[0])] = int(t[1])
        bap[int(t[2])] = int(t[3])
        cnt += 1
i = find('Table E3.1 High Efficiency Bit Allocation Pointers')
hebap = [None] * 64
cnt = 0
while cnt < 32:
    i += 1
    t = L[i].split()
    if len(t) == 4 and all(x.isdigit() for x in t):
        hebap[int(t[0])] = int(t[1])
        hebap[int(t[2])] = int(t[3])
        cnt += 1
i = find('Table E3.14 Spectral Extension Attenuation Table')
att = []
while len(att) < 32:
    i += 1
    t = L[i].split()
    if len(t) == 4 and t[0].isdigit() and t[1].startswith('0.'):
        assert int(t[0]) == len(att)
        att.append([float(x) for x in t[1:]])
i = find('Table E3.6 Large Mantissa Inverse Quantization')
remap = {}
cur = None
while len(remap) < 12 * 2:
    i += 1
    t = L[i].replace('≥', '>=').split()
    if not t:
        continue
    if t[0].isdigit() and len(t) > 2 and t[1].startswith('x'):
        cur = int(t[0])
        t = t[1:]
    if t and t[0].startswith('x') and cur is not None:
        sign = 0 if '>=' in t[0] else 1
        v = [int(x, 16) if x.startswith('0x') else None for x in t[1:]]
        while len(v) < 6:
            v.append(None)
        remap[(cur, sign)] = v
vq = []
for k, n in zip(range(1, 8), [4, 8, 16, 32, 128, 256, 512]):
    i = find(f'Table E4.{k} VQ Table for hebap {k}')
    i += 1
    rows = []
    while len(rows) < n:
        i += 1
        t = L[i].split()
        if len(t) == 7 and t[0].isdigit() and t[1].startswith('0x'):
            assert int(t[0]) == len(rows), (k, t)
            rows.append([int(x, 16) - (65536 if int(x, 16) >= 32768 else 0) for x in t[1:]])
    vq.append(rows)

assert None not in bap and None not in hebap
assert all(None not in r for r in hth)
out.append("#![allow(clippy::excessive_precision)]")
out.append("//! Tables of ATSC A/52:2012, typed in from the standard (generated from its text by')
out.append("//! `tables.py` beside this crate's tests): the bit allocation tables of section 7.2.3, E-AC-3's")
out.append('//! high efficiency bit allocation pointers (Table E3.1), large mantissa remapping (E3.6),')
out.append('//! spectral extension attenuation (E3.14) and the AHT vector quantization tables (E4.1 to E4.7).')
out.append('')
out.append('/// Table 7.13: band of each bin.')
emit('MASKTAB', 'u8', masktab, 20)
out.append('/// Table 7.14: log-addition.')
emit('LATAB', 'u8', latab, 20)
out.append('/// Table 7.15: hearing threshold by `fscod` (48, 44.1, 32 kHz) and band.')
out.append('pub(crate) const HTH: [[u16; 50]; 3] = [')
for f in range(3):
    out.append('    [' + ', '.join(f'0x{v:04x}' for v in hth[f]) + '],')
out.append('];')
out.append('/// Table 7.16: bit allocation pointers.')
emit('BAPTAB', 'u8', bap, 32)
out.append('/// Table E3.1: high efficiency bit allocation pointers.')
emit('HEBAPTAB', 'u8', hebap, 32)
out.append('/// Table E3.14: spectral extension attenuation by `spxattencod` and bin.')
out.append('pub(crate) const SPXATTENTAB: [[f32; 3]; 32] = [')
for r in att:
    out.append('    [' + ', '.join(repr(x) for x in r) + '],')
out.append('];')
out.append('/// Table E3.6: large mantissa remapping constants `(a, b)` as 16-bit fractions, by hebap')
out.append('/// 8..=19, sign of x (non-negative, negative) and gain (1, 2, 4); zero where not applicable.')
out.append('pub(crate) const GAQ_REMAP: [[[(i16, i16); 3]; 2]; 12] = [')


def s16(v):
    return v - 65536 if v >= 32768 else v


for h in range(8, 20):
    rows = []
    for sign in range(2):
        v = remap[(h, sign)]
        pairs = []
        for g in range(3):
            a, b = v[2 * g], v[2 * g + 1]
            pairs.append(f'({s16(a) if a is not None else 0}, {s16(b) if b is not None else 0})')
        rows.append('[' + ', '.join(pairs) + ']')
    out.append('    [' + ', '.join(rows) + '],')
out.append('];')
for k, rows in enumerate(vq, 1):
    out.append(f'/// Table E4.{k}: vectors for hebap {k}.')
    out.append(f'pub(crate) const VQ{k}: [[i16; 6]; {len(rows)}] = [')
    for r in rows:
        out.append('    [' + ', '.join(str(x) for x in r) + '],')
    out.append('];')
open(sys.argv[1], 'w').write('\n'.join(out) + '\n')
print('ok', masktab[:30], latab[-5:], hth[0][:3], bap[:8], hebap[60:], att[0], remap[(8, 1)], vq[6][511])
