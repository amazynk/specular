"""Wrap an 8-bit greyscale codestream into a JP2 with a 3-column palette (pclr + cmap).

Usage: mkpal.py in.j2k out.jp2. OpenJPEG cannot write palettes, so the index image is coded
by opj_compress and the boxes are added here."""
import struct, sys

cs = open(sys.argv[1], "rb").read()
w, h = struct.unpack(">II", cs[8:16])  # SIZ Xsiz, Ysiz (origin 0)


def box(t, payload):
    return struct.pack(">I", 8 + len(payload)) + t + payload


ihdr = box(b"ihdr", struct.pack(">IIHBBBB", h, w, 3, 7, 7, 0, 0))
colr = box(b"colr", struct.pack(">BBBI", 1, 0, 0, 16))
n = 256
pal = bytearray(struct.pack(">HB", n, 3) + bytes([7, 7, 7]))
for i in range(n):
    pal += bytes([i, (i * 7) & 255, 255 - i])
pclr = box(b"pclr", bytes(pal))
cmap = box(b"cmap", b"".join(struct.pack(">HBB", 0, 1, col) for col in range(3)))
jp2h = box(b"jp2h", ihdr + colr + pclr + cmap)
sig = box(b"jP  ", b"\r\n\x87\n")
ftyp = box(b"ftyp", b"jp2 " + struct.pack(">I", 0) + b"jp2 ")
open(sys.argv[2], "wb").write(sig + ftyp + jp2h + box(b"jp2c", cs))
