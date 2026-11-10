"""Wraps the G4 strip of a one-strip bilevel TIFF as a DjVu Smmr chunk body."""
import struct, sys
d = open(sys.argv[1], 'rb').read()
assert d[:4] == b'II*\x00'
off = struct.unpack_from('<I', d, 4)[0]
n = struct.unpack_from('<H', d, off)[0]
tags = {}
for i in range(n):
    tag, typ, cnt, val = struct.unpack_from('<HHII', d, off + 2 + 12 * i)
    if typ == 3 and cnt == 1:
        val &= 0xffff
    tags[tag] = (typ, cnt, val)
w, h = tags[256][2], tags[257][2]
assert tags[259][2] == 4, 'not G4'
assert tags[273][1] == 1, 'one strip only'
start, size = tags[273][2], tags[279][2]
photometric = tags.get(262, (0, 0, 0))[2]
flags = 1 if photometric == 1 else 0  # BlackIsZero: inverted
open(sys.argv[2], 'wb').write(b'MMR' + bytes([flags]) + struct.pack('>HH', w, h) + d[start:start + size])
print(w, h, photometric, size)
