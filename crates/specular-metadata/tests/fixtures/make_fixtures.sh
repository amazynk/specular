#!/usr/bin/env bash
# Regenerates the metadata test fixtures (tiny JPEGs with known EXIF/XMP/ICC).
# Needs ImageMagick 7 (`magick`), libjpeg-turbo's `cjpeg` and ExifTool.
# Embedded ICC profiles come from the icc-profiles-free package:
#   DisplayP3-v4.icc (Compact ICC Profiles, CC0) and
#   compatibleWithAdobeRGB1998.icc (Graeme Gill, public domain / Zlib).
set -euo pipefail
cd "$(dirname "$0")"
ICC_DIR=${ICC_DIR:-/usr/share/color/icc}
ET=(exiftool -q -overwrite_original -n)

base() { # $1 = output; progressive 4:2:0 YCbCr, 64x48
    magick -size 64x48 gradient:'#3a6ea5-#f2c14e' -strip -quality 85 \
        -sampling-factor 2x2 -interlace JPEG "$1"
}

# --- Lisbon: full camera set, GPS, XMP rating/label/title, v4 (mluc) ICC ------
base lisbon.jpg
"${ET[@]}" \
    -Make='Canon' -Model='Canon EOS R5' -Software='Firmware 1.8.1' \
    -SerialNumber='012345678901' -BodySerialNumber='012345678901' \
    -LensModel='RF24-70mm F2.8 L IS USM' -LensMake='Canon' -LensInfo='24 70 2.8 2.8' \
    -ExposureTime=0.004 -FNumber=2.8 -ISO=400 -FocalLength=35 -FocalLengthIn35mmFormat=52 \
    -ExposureCompensation=0.7 -ExposureProgram=3 -MeteringMode=5 -WhiteBalance=0 -EXIF:Flash=25 \
    -DateTimeOriginal='2024:05:14 18:32:10' -OffsetTimeOriginal='+01:00' \
    -CreateDate='2024:05:14 18:32:10' -ModifyDate='2024:05:20 09:15:00' \
    -GPSLatitude=38.7223 -GPSLatitudeRef=N -GPSLongitude=9.1393 -GPSLongitudeRef=W \
    -GPSAltitude=112 -GPSAltitudeRef=0 -GPSImgDirection=287 -GPSImgDirectionRef=T \
    -GPSHPositioningError=5 -GPSDateStamp='2024:05:14' -GPSTimeStamp='17:32:10' \
    -Orientation=6 -Artist='Ana Silva' -Copyright='(c) 2024 Ana Silva' \
    -XMP-xmp:Rating=4 -XMP-xmp:Label=Red -XMP-dc:Title='Tram 28 in Alfama' \
    -XMP-dc:Creator='Ana Silva' -XMP-dc:Rights='© 2024 Ana Silva' \
    -XMP-dc:Subject=tram -XMP-dc:Subject=lisbon -XMP-dc:Subject=alfama \
    "-ICC_Profile<=$ICC_DIR/DisplayP3-v4.icc" \
    lisbon.jpg

# --- Buenos Aires: Nikon, no zone in EXIF (inferred from GPS), v2 (desc) ICC ----
base buenos_aires.jpg
"${ET[@]}" \
    -Make='NIKON CORPORATION' -Model='NIKON D850' \
    -ExposureTime=2.5 -FNumber=8 -ISO=64 -FocalLength=24 -ExposureCompensation=-1.3333 \
    -ExposureProgram=1 -MeteringMode=2 -WhiteBalance=1 -LightSource=1 -EXIF:Flash=16 \
    -DateTimeOriginal='2023:11:02 20:05:00' \
    -GPSLatitude=34.6132 -GPSLatitudeRef=S -GPSLongitude=58.3772 -GPSLongitudeRef=W \
    -GPSAltitude=3 -GPSAltitudeRef=1 -GPSImgDirection=90 -GPSImgDirectionRef=M \
    -GPSDateStamp='2023:11:02' -GPSTimeStamp='23:05:02' \
    "-ICC_Profile<=$ICC_DIR/compatibleWithAdobeRGB1998.icc" \
    buenos_aires.jpg

# --- Mid-Atlantic: nearest town is far away ("near …") ------------------------
base ocean.jpg
"${ET[@]}" -Make='Apple' -Model='iPhone 15 Pro' -DateTimeOriginal='2024:07:01 12:00:00' \
    -OffsetTimeOriginal='-02:00' \
    -GPSLatitude=40.0 -GPSLatitudeRef=N -GPSLongitude=40.0 -GPSLongitudeRef=W ocean.jpg

# --- No date, no ICC, greyscale baseline (cjpeg) ------------------------------
magick -size 64x48 gradient:black-white -strip ppm:- | cjpeg -grayscale -quality 80 > no_date.jpg
"${ET[@]}" -Make='FUJIFILM' -Model='X-T5' no_date.jpg

# --- Orientation 1..8, and an EXIF/XMP conflict -------------------------------
for o in 1 2 3 4 5 6 7 8; do
    base "orientation_$o.jpg"
    "${ET[@]}" -Orientation=$o -DateTimeOriginal='2024:01:01 10:00:00' "orientation_$o.jpg"
done
base orientation_conflict.jpg
"${ET[@]}" -Orientation=6 -XMP-tiff:Orientation=1 -DateTimeOriginal='2024:01:01 10:00:00' \
    orientation_conflict.jpg

ls -l ./*.jpg
du -ch ./*.jpg | tail -1
