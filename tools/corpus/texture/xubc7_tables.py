"""Emit compact BC7 partition/anchor tables for xubc7.rs from bcn.rs's PARTITION_SETS (bc7 fn).

Usage: python -I xubc7_tables.py path/to/bcn.rs
"""

import re
import sys

src = open(sys.argv[1]).read()
src = src[src.index('pub(crate) fn bc7('):]
rows = re.findall(r'\[\[(\d+), (\d+), (\d+), (\d+)\], \[(\d+), (\d+), (\d+), (\d+)\], '
                  r'\[(\d+), (\d+), (\d+), (\d+)\], \[(\d+), (\d+), (\d+), (\d+)\]\]', src)
rows = [[int(v) for v in r] for r in rows[:128]]
assert len(rows) == 128
p2, p3 = rows[:64], rows[64:]
masks2, masks3, a2, a31, a32 = [], [], [], [], []
for r in p2:
    m = 0
    for i, v in enumerate(r):
        s = v & 3
        assert s < 2
        m |= s << i
        if v == 129:
            a2.append(i)
    masks2.append(m)
for r in p3:
    m = 0
    for i, v in enumerate(r):
        s = v & 3
        assert s < 3
        m |= s << (2 * i)
        if v == 129:
            a31.append(i)
        if v == 130:
            a32.append(i)
    masks3.append(m)
assert len(a2) == 64 and len(a31) == 64 and len(a32) == 64


def emit(name, ty, vals, fmt, per):
    print('static %s: [%s; %d] = [' % (name, ty, len(vals)))
    for i in range(0, len(vals), per):
        print('    ' + ', '.join(fmt % v for v in vals[i:i + per]) + ',')
    print('];')


emit('PART2', 'u16', masks2, '0x%04x', 8)
emit('PART3', 'u32', masks3, '0x%08x', 6)
emit('ANCHOR2', 'u8', a2, '%d', 16)
emit('ANCHOR3_1', 'u8', a31, '%d', 16)
emit('ANCHOR3_2', 'u8', a32, '%d', 16)
