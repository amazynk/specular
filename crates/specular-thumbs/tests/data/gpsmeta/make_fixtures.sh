#!/usr/bin/env bash
# Regenerates the GPS / capture-time fixtures of tests/gpsmeta.rs: one tiny synthetic image per
# format whose Exif is found without decoding (exif_locate.rs in specular-decode). Every image
# is a 16x12 gradient; nothing here comes from a real photo.
#
# All but nogps.webp carry the same made-up Exif: Make "Specular" ("Canon" in gps.cr3), Model "Fixture Cam",
# DateTimeOriginal 2021:07:14 09:26:53 and a GPS position of 10 N, 20 E.
#
#   gps.jpg            APP1 Exif (the reference path)
#   gps.png            eXIf chunk
#   gps_rawprofile.png ImageMagick's legacy zTXt "Raw profile type exif" (hex), no eXIf
#   gps.webp           EXIF chunk in a VP8X container
#   gps.heic           `Exif` item (iinf/iloc, 4-byte TIFF header offset prefix)
#   gps.avif           the same in AVIF
#   gps.jxl            `Exif` box of a JPEG XL container (the codestream is the in-tree
#                      crates/formats/specular-jxl/tests/data/modular.jxl, wrapped by ExifTool)
#   gps_brob.jxl       gps.jxl with its Exif box Brotli-compressed (`brob`), plus a `brob` XMP
#                      box (gps_brob.xmp: a title and a rating), by specular-jxl's
#                      tests/gen_brob.py
#   gps.psd            image resource 1058 (Exif) of a Photoshop file
#   gps.tif            a big-endian 16-bit RGB TIFF: IFD0 (Make, Model) pointing at the Exif
#                      and GPS IFDs, rebuilt into a compact structure by tiff_exif.rs
#   gps.cr3            a Canon CR3 skeleton: CMT1 (IFD0, Make "Canon"), CMT2 (Exif IFD, no
#                      LensModel), CMT3 (a Canon maker note: lens "Fixture Lens 35mm F2", AI Servo,
#                      IS on, a made-up serial number 0x5E81A1) and CMT4 (GPS IFD) in the Canon
#                      `uuid` box, gps.jpg as the PRVW preview (written by the Python below; no
#                      sensor data). Only this part needs rerunning for a CR3 change.
#   nogps.webp         DateTimeOriginal but no GPS
#
# Needs ImageMagick 7 (`magick`, with libheif for HEIC/AVIF and libwebp), ExifTool and python3
# with the `brotli` module.
set -euo pipefail
cd "$(dirname "$0")"
JXL_SRC=../../../../formats/specular-jxl/tests/data/modular.jxl
# -m: wrapping the bare JXL codestream in a container is a "minor" warning.
ET=(exiftool -q -m -overwrite_original -n)
TAGS=(-Make=Specular '-Model=Fixture Cam' '-DateTimeOriginal=2021:07:14 09:26:53'
    -GPSLatitude=10 -GPSLatitudeRef=N -GPSLongitude=20 -GPSLongitudeRef=E)

work=$(mktemp -d ./.work.XXXXXX)
trap 'rm -rf "$work"' EXIT
magick -size 16x12 gradient:'#336699-#ffcc33' -strip "$work/base.png"

magick "$work/base.png" -quality 80 gps.jpg
magick "$work/base.png" -define png:exclude-chunk=date,tIME gps.png
magick "$work/base.png" -define webp:lossless=true gps.webp
magick "$work/base.png" -quality 50 gps.heic
magick "$work/base.png" -quality 50 gps.avif
cp "$JXL_SRC" gps.jxl
magick "$work/base.png" gps.psd
magick "$work/base.png" -strip -endian MSB -compress none gps.tif
for f in gps.jpg gps.png gps.webp gps.heic gps.avif gps.jxl gps.psd gps.tif; do
    "${ET[@]}" "${TAGS[@]}" "$f"
done
python3 -I ../../../../formats/specular-jxl/tests/gen_brob.py gps.jxl gps_brob.jxl --xmp gps_brob.xmp
magick "$work/base.png" -define webp:lossless=true nogps.webp
"${ET[@]}" -Make=Specular '-DateTimeOriginal=2021:07:14 09:26:53' nogps.webp

# The legacy form: the Exif of gps.png as a hex "Raw profile type exif" zTXt chunk, which
# ImageMagick wrote before PNG had eXIf.
exiftool -b -EXIF gps.png > "$work/exif.bin"
python3 -I - "$work/base.png" "$work/exif.bin" gps_rawprofile.png <<'PY'
import struct, sys, zlib
png, exif, out = open(sys.argv[1], 'rb').read(), open(sys.argv[2], 'rb').read(), sys.argv[3]
if not exif.startswith(b'Exif\0\0'):
    exif = b'Exif\0\0' + exif
hexed = exif.hex()
lines = '\n'.join(hexed[i:i + 72] for i in range(0, len(hexed), 72))
text = '\nexif\n%8d\n%s\n' % (len(exif), lines)
data = b'Raw profile type exif\0\0' + zlib.compress(text.encode(), 9)
chunk = struct.pack('>I', len(data)) + b'zTXt' + data
chunk += struct.pack('>I', zlib.crc32(b'zTXt' + data) & 0xffffffff)
i = png.index(b'IDAT') - 4
open(out, 'wb').write(png[:i] + chunk + png[i:])
PY

# The CR3 skeleton. Each CMT block is a TIFF structure of its own (little-endian here).
python3 -I - gps.jpg gps.cr3 <<'PY'
import struct, sys
jpg, out = open(sys.argv[1], 'rb').read(), sys.argv[2]
def bx(kind, body):
    return struct.pack('>I', len(body) + 8) + kind + body
def tiff(entries):
    n = len(entries)
    at = 8 + 2 + 12 * n + 4
    head, tail = b'II*\0' + struct.pack('<IH', 8, n), b''
    for tag, typ, count, v in sorted(entries):
        if len(v) <= 4:
            head += struct.pack('<HHI', tag, typ, count) + v.ljust(4, b'\0')
        else:
            head += struct.pack('<HHII', tag, typ, count, at + len(tail))
            tail += v + b'\0' * (len(v) % 2)
    return head + bytes(4) + tail
def asc(s):
    return (2, len(s) + 1, s.encode() + b'\0')
def rat(*v):
    return (5, len(v), b''.join(struct.pack('<II', a, b) for a, b in v))
cmt1 = tiff([(0x10F, *asc('Canon')), (0x110, *asc('Fixture Cam')), (0x112, 3, 1, struct.pack('<H', 1))])
cmt2 = tiff([(0x9003, *asc('2021:07:14 09:26:53')), (0x8827, 3, 1, struct.pack('<H', 100))])
cmt4 = tiff([(0x0, 1, 4, bytes([2, 3, 0, 0])), (0x1, *asc('N')), (0x2, *rat((10, 1), (0, 1), (0, 1))),
             (0x3, *asc('E')), (0x4, *rat((20, 1), (0, 1), (0, 1)))])
# The maker note: CameraSettings (element 0 its length in bytes; 7 FocusMode = AI Servo,
# 23/24 focal max/min = 35, 25 focal units = 1, 26 MaxAperture = 64 (f/2), 34
# ImageStabilization = On), LensModel padded with junk after the NUL as cameras do, and a
# made-up serial number (0x000C) that must never show.
cs = [0] * 40
cs[0], cs[7], cs[23], cs[24], cs[25], cs[26], cs[34] = 80, 1, 35, 35, 1, 64, 1
cmt3 = tiff([(0x1, 3, 40, struct.pack('<40H', *cs)), (0xC, 4, 1, struct.pack('<I', 0x5E81A1)),
             (0x95, 2, 32, b'Fixture Lens 35mm F2\0'.ljust(32, b'\x07'))])
canon = bytes.fromhex('85c0b687820f11e08111f4ce462b6a48') + bx(b'CNCV', b'CanonCR3_001/00.10.00/00.00.00')
canon += bx(b'CMT1', cmt1) + bx(b'CMT2', cmt2) + bx(b'CMT3', cmt3)
canon += bx(b'CMT4', cmt4)
moov = bx(b'mvhd', bytes(100)) + bx(b'uuid', canon)
prvw = bytes.fromhex('eaf42b5e1c984b88b9fbb7dc406e4d16') + bytes(8) + bx(b'PRVW', bytes(16) + jpg)
f = bx(b'ftyp', b'crx \0\0\0\x01crx isom') + bx(b'moov', moov) + bx(b'uuid', prvw) + bx(b'mdat', b'')
open(out, 'wb').write(f)
PY
ls -l gps* nogps*
