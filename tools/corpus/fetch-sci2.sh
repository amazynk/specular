#!/usr/bin/env bash
# Fetch the sci2 remote-sensing corpus into tools/corpus/sci2/ (gitignored: nothing here is
# committed) and write its GDAL oracle. The specular-decode test `sci2_oracle` reads it from
# SPECULAR_SCI2_CORPUS, else tools/corpus/sci2, and skips when it is absent.
#
# Usage: tools/corpus/fetch-sci2.sh
#   PYTHON: a Python with numpy and rasterio (wheels only, no system packages), e.g.
#     python3 -m venv target/sci2/venv
#     PIP_CACHE_DIR=target/sci2/pipcache target/sci2/venv/bin/pip install \
#       --only-binary=:all: numpy rasterio opencolorio
#   then PYTHON=target/sci2/venv/bin/python.
#
# What it fetches (about 120 MB):
#   gdal/  GDAL autotest files at tag v3.11.0 (MIT/X11): ENVI rasters (BIL/BIP/BSQ, big-endian
#          16-bit) and a SICD NITF (complex I/Q samples)
#   s2/    parts of two Sentinel-2 products of tile 33TUN, 2015-07-04, from the public Google
#          Cloud bucket gcp-public-data-sentinel-2 (Copernicus Sentinel data, free and open
#          licence): the L2A product's metadata, all 60 m bands, the 20 m SCL and B8A and
#          the cloud masks and its preview; the L1C product's metadata, B01, B09, B10 and MSK_CLASSI_B00.
#          The products keep their SAFE layout; files not fetched are left out.
# Then sci2-fixtures.py generates the ENVI/complex fixtures and a small synthetic SAFE
# product and writes expected/sci2.jsonl and expected/s2.jsonl, and ocio-oracle.py writes
# ocio/ (the ACES 1.3 cg and studio configs PyOpenColorIO carries, LUT files, and
# PyOpenColorIO's answers on test ramps) for the specular-ocio test.
#
# Downloads are untrusted data: this script only copies files; the oracle reads them from a
# script outside the corpus. Stops below MIN_FREE_GB free (default 8).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_SCI2_CORPUS:-$ROOT/tools/corpus/sci2}
PYTHON=${PYTHON:-python3}
MIN_FREE_GB=${MIN_FREE_GB:-8}
mkdir -p "$OUT"

die() { echo "fetch-sci2: $*" >&2; exit 1; }
check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  (( avail_kb >= MIN_FREE_GB * 1024 * 1024 )) || die "less than ${MIN_FREE_GB} GB free; stopping"
}

# get URL DEST: shows what, speed, ETA and a progress bar; skips files already there.
get() {
  local url=$1 dest=$2
  [[ -s "$dest" ]] && return 0
  check_space
  mkdir -p "$(dirname "$dest")"
  echo "  ${dest#"$OUT"/}"
  curl -fL --retry 3 --progress-bar -o "$dest.part" "$url"
  mv "$dest.part" "$dest"
}

GDAL=https://raw.githubusercontent.com/OSGeo/gdal/v3.11.0/autotest/gdrivers/data
echo "GDAL autotest files"
for f in envi/envi_rgbsmall_bil.hdr envi/envi_rgbsmall_bil.img envi/envi_rgbsmall_bip.hdr \
         envi/envi_rgbsmall_bip.img envi/envi_rgbsmall_bsq.hdr envi/envi_rgbsmall_bsq.img \
         envi/uint16_envi_bigendian.hdr envi/uint16_envi_bigendian.dat nitf/sar_sicd.ntf; do
  get "$GDAL/$f" "$OUT/gdal/${f#*/}"
done

BUCKET=https://storage.googleapis.com/gcp-public-data-sentinel-2
L2A=S2A_MSIL2A_20150704T101006_N0500_R022_T33TUN_20231012T100650.SAFE
L2A_G=GRANULE/L2A_T33TUN_A000162_20150704T101337
L2A_P=T33TUN_20150704T101006
echo "Sentinel-2 L2A (part)"
for f in MTD_MSIL2A.xml manifest.safe "$L2A_G/MTD_TL.xml" \
         "$L2A_G/QI_DATA/MSK_CLDPRB_20m.jp2" "$L2A_G/QI_DATA/MSK_CLDPRB_60m.jp2" \
         "$L2A_G/QI_DATA/${L2A_P}_PVI.jp2" \
         "$L2A_G/IMG_DATA/R20m/${L2A_P}_SCL_20m.jp2" "$L2A_G/IMG_DATA/R20m/${L2A_P}_B8A_20m.jp2"; do
  get "$BUCKET/L2/tiles/33/T/UN/$L2A/$f" "$OUT/s2/$L2A/$f"
done
for b in B01 B02 B03 B04 B05 B06 B07 B8A B09 B11 B12 SCL TCI AOT WVP; do
  f="$L2A_G/IMG_DATA/R60m/${L2A_P}_${b}_60m.jp2"
  get "$BUCKET/L2/tiles/33/T/UN/$L2A/$f" "$OUT/s2/$L2A/$f"
done

L1C=S2A_MSIL1C_20150704T101006_N0500_R022_T33TUN_20231011T134419.SAFE
L1C_G=GRANULE/L1C_T33TUN_A000162_20150704T101337
echo "Sentinel-2 L1C (part)"
for f in MTD_MSIL1C.xml manifest.safe "$L1C_G/MTD_TL.xml" "$L1C_G/QI_DATA/MSK_CLASSI_B00.jp2" \
         "$L1C_G/IMG_DATA/${L2A_P}_B01.jp2" "$L1C_G/IMG_DATA/${L2A_P}_B09.jp2" \
         "$L1C_G/IMG_DATA/${L2A_P}_B10.jp2"; do
  get "$BUCKET/tiles/33/T/UN/$L1C/$f" "$OUT/s2/$L1C/$f"
done

echo "Fixtures and oracle"
"$PYTHON" -I "$ROOT/tools/corpus/sci2-fixtures.py" "$OUT"
"$PYTHON" -I "$ROOT/tools/corpus/ocio-oracle.py" "$OUT/ocio"
du -sh "$OUT"
