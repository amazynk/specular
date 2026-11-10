"""Writes the video elementary stream of an encapsulated DICOM file (its pixel data fragments
joined, the basic offset table left out) for ffmpeg: python3 -I dicom-es.py IN.dcm OUT.m2v

Reads data only: finds (7FE0,0010) with an undefined length and walks its items."""
import struct
import sys

data = open(sys.argv[1], 'rb').read()
tag = b'\xe0\x7f\x10\x00'
at = data.rfind(tag + b'OB\x00\x00\xff\xff\xff\xff')
if at < 0:
    sys.exit('no encapsulated pixel data')
p = at + 12
out = bytearray()
first = True
while p + 8 <= len(data):
    group, elem, length = struct.unpack_from('<HHI', data, p)
    p += 8
    if (group, elem) == (0xFFFE, 0xE0DD):
        break
    if (group, elem) != (0xFFFE, 0xE000):
        sys.exit('unexpected element in pixel data')
    if not first:
        out += data[p:p + length]
    first = False
    p += length
open(sys.argv[2], 'wb').write(out)
