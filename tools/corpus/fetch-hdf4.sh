#!/usr/bin/env bash
# Fetch a small public HDF4 / HDF-EOS2 corpus into tools/corpus/hdf4/ (gitignored: nothing here
# is committed). crates/formats/specular-hdf4/gen/oracle.py compares what
# `cargo run -p specular-hdf4 --example hdf4dump` reads from these files with pyhdf.
#
# Usage: tools/corpus/fetch-hdf4.sh [SECTION...]
#   SECTION is gdal, hdflib or unidata. Default: all of them.
#   OUT defaults to $SPECULAR_HDF4_CORPUS, else tools/corpus/hdf4.
#
# About 100 MB once decompressed (20 MB downloaded):
#   gdal/     GDAL autotest HDF4 files (MIT): SDS of every number type, 2-D and 3-D, unlimited
#             dimensions, GR images with and without palettes, a UTM-georeferenced SDS
#   hdflib/   The HDF Group's HDF4 library test and example files (BSD-style): RIG / RI8 /
#             RI24 images with palettes, compressed images (RLE, IMCOMP, JPEG), compressed and
#             chunked SDS, Vdatas and Vgroups, long names, many empty datasets
#   unidata/  NASA HDF-EOS2 products that netCDF-C tests with, mirrored by Unidata: AMSR-E L2
#             rain and L3 daily land (swath and grid), MODIS MOD29/MYD29 sea-ice swaths
#
# Not fetched: MODIS land sinusoidal tiles (MOD09A1, MOD13, MCD43 ...), MODIS L1B, AIRS, MISR,
# CERES and other EOS products. Every NASA DAAC (LP DAAC, LAADS, NSIDC, GES DISC, LaRC) now needs
# an Earthdata login to download, so the MODIS-like grid is checked with the generated fixture
# crates/formats/specular-hdf4/tests/data/eos_grid.hdf instead.
#
# GitHub files come from pinned commits and are checked by size; Unidata files by size and
# sha256. Downloads are untrusted data: this script only copies and decompresses files, never
# runs anything from them. Stops if free space drops below MIN_FREE_GB (default 8).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_HDF4_CORPUS:-$ROOT/tools/corpus/hdf4}
MIN_FREE_GB=${MIN_FREE_GB:-8}
mkdir -p "$OUT"

die() { echo "fetch-hdf4: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

# get URL DEST SIZE [SHA256]: download unless DEST already has that size (and hash).
get() {
  local url=$1 dest=$2 size=$3 sha=${4:-}
  if [[ -f $dest && $(stat -c %s "$dest") == "$size" ]]; then
    return 0
  fi
  check_space
  mkdir -p "$(dirname "$dest")"
  curl -fsSL --retry 3 -o "$dest.part" "$url" || die "download failed: $url"
  [[ $(stat -c %s "$dest.part") == "$size" ]] || die "$url: size $(stat -c %s "$dest.part"), expected $size"
  if [[ -n $sha ]]; then
    [[ $(sha256sum "$dest.part" | cut -d' ' -f1) == "$sha" ]] || die "$url: sha256 mismatch"
  fi
  mv "$dest.part" "$dest"
  echo "fetch-hdf4: $(basename "$dest")"
}

GDAL=https://raw.githubusercontent.com/OSGeo/gdal/0d57017e566dfa576f46bfb3e4cac3dabd91d3a8
gdal() {
  local d=$OUT/gdal
  while read -r path size; do
    get "$GDAL/$path" "$d/$(basename "$path")" "$size"
  done <<'EOF'
autotest/gcore/data/General_RImages.hdf 1210
autotest/gcore/data/Image_with_Palette.hdf 1252
autotest/gcore/data/SDS.hdf 4613
autotest/gcore/data/SDSUNLIMITED.hdf 5741
autotest/gcore/data/byte_2.hdf 3970
autotest/gcore/data/byte_3.hdf 4109
autotest/gcore/data/float32_2.hdf 5296
autotest/gcore/data/float32_3.hdf 5436
autotest/gcore/data/float64_2.hdf 6896
autotest/gcore/data/float64_3.hdf 6896
autotest/gcore/data/int16_2.hdf 4494
autotest/gcore/data/int16_3.hdf 4634
autotest/gcore/data/int32_2.hdf 5294
autotest/gcore/data/int32_3.hdf 5434
autotest/gcore/data/uint16_2.hdf 4495
autotest/gcore/data/uint16_3.hdf 4634
autotest/gcore/data/uint32_2.hdf 5295
autotest/gcore/data/uint32_3.hdf 5435
autotest/gcore/data/utmsmall_2.hdf 13697
autotest/gcore/data/utmsmall_3.hdf 13837
EOF
}

HDFLIB=https://raw.githubusercontent.com/HDFGroup/hdf4/2c46ad56bea5aab05e996e473a81dcbd9a66b125
hdflib() {
  local d=$OUT/hdflib
  while read -r path size; do
    # Several directories hold files of the same name: keep the directory in the name.
    local name
    name=$(echo "$path" | tr '/' '_')
    get "$HDFLIB/$path" "$d/$name" "$size"
  done <<'EOF'
HDF4Examples/C/AN/testfiles/General_HDFobjects.hdf 469
HDF4Examples/C/GR/testfiles/General_RImages.hdf 1599
HDF4Examples/C/GR/testfiles/Image_with_Palette.hdf 1261
HDF4Examples/C/VD/testfiles/General_Vdatas.hdf 554
HDF4Examples/C/VD/testfiles/Packed_Vdata.hdf 636
HDF4Examples/C/VD/testfiles/Two_Vdatas.hdf 373
HDF4Examples/C/VG/testfiles/General_Vgroups.hdf 625
hdf/test/test_files/grtdfui83.hdf 2603
hdf/util/testfiles/fp2hdf/pal.hdf 970
hdf/util/testfiles/jet2.hdf 121010
hdf/util/testfiles/ntcheck.hdf 3066
hdf/util/testfiles/skull.hdf 804543
hdf/util/testfiles/storm110.hdf 3487
hdf/util/testfiles/test.hdf 30275
hdf/util/testfiles/tvattr.hdf 2256
mfhdf/hdfimport/SDSfloat2.hdf 3116
mfhdf/hdfimport/SDSfloat3.hdf 3451
mfhdf/hdiff/testfiles/hdifftst1.hdf 3992
mfhdf/hdiff/testfiles/hdifftst3.hdf 1074414
mfhdf/hdiff/testfiles/hdifftst5.hdf 2966
mfhdf/hdiff/testfiles/hdifftst7.hdf 364
mfhdf/hdp/testfiles/Example6.hdf 2615
mfhdf/hdp/testfiles/IMCOMP.hdf 1155
mfhdf/hdp/testfiles/Image_with_Palette.hdf 2537
mfhdf/hdp/testfiles/LongDataset.hdf 3555
mfhdf/hdp/testfiles/SDSlongname.hdf 3203
mfhdf/hdp/testfiles/Tables.hdf 711
mfhdf/hdp/testfiles/VGlongname.hdf 753
mfhdf/hdp/testfiles/ctxtr2r.hdf 434
mfhdf/hdp/testfiles/grtdfi322.hdf 1678
mfhdf/hdp/testfiles/grtdfui162.hdf 2447
mfhdf/hdp/testfiles/grtdfui82.hdf 2332
mfhdf/hdp/testfiles/grtdfui84.hdf 2447
mfhdf/hdp/testfiles/sds1_dim1_samename.hdf 3126
mfhdf/hdp/testfiles/sds2_dim1_samename.hdf 3354
mfhdf/hdp/testfiles/sds_compressed.hdf 3945
mfhdf/hdp/testfiles/sds_empty_many.hdf 21705
mfhdf/hdp/testfiles/star.hdf 174066
mfhdf/hdp/testfiles/swf32.hdf 573
mfhdf/hdp/testfiles/swf32_fileattr.hdf 25796
mfhdf/hdp/testfiles/swi16.hdf 488
mfhdf/hdp/testfiles/swi8.hdf 452
mfhdf/hdp/testfiles/tdata.hdf 7190
mfhdf/hdp/testfiles/tdf24.hdf 2137
mfhdf/hdp/testfiles/tdfr8f.hdf 67111
mfhdf/hdp/testfiles/tvset.hdf 75925
mfhdf/hdp/testfiles/vslongname.hdf 639
EOF
}

UNIDATA=https://resources.unidata.ucar.edu/netcdf/sample_data/hdf4
unidata() {
  local d=$OUT/unidata
  while read -r name size sha; do
    local gz=$d/.gz/$name.gz
    get "$UNIDATA/$name.gz" "$gz" "$size" "$sha"
    if [[ ! -f $d/$name ]]; then
      gzip -dc "$gz" > "$d/$name.part"
      mv "$d/$name.part" "$d/$name"
    fi
  done <<'EOF'
AMSR_E_L2_Rain_V10_200905312326_A.hdf 5148066 a7e8bb2c10fdc8ede191244e990f118b204634e31cd4634396fd9783de2b57c7
AMSR_E_L3_DailyLand_V06_20020619.hdf 8170718 cd22decc317d85981990994212746c749b426287de98977b4b33695b409495ab
MOD29.A2000055.0005.005.2006267200024.hdf 825105 7c68b002f5c3b837b45cf5865dcb57974a4dc1bc9898122a85c12cef89f99096
MYD29.A2002185.0000.005.2007160150627.hdf 2233673 2ad21a956a595cc46b9d149acdf100e7f6d4666b75c1fcb2f06f557ef33ff2fa
MYD29.A2009152.0000.005.2009153124331.hdf 2429720 43614eb456a71d3349c812ba196399d765095aa6d558d66af622f0d023307f5f
EOF
}

sections=("$@")
(( ${#sections[@]} )) || sections=(gdal hdflib unidata)
for s in "${sections[@]}"; do
  case $s in
    gdal | hdflib | unidata) "$s" ;;
    *) die "unknown section $s (gdal, hdflib, unidata)" ;;
  esac
done
echo "fetch-hdf4: done, $(find "$OUT" -name '*.hdf' -not -path '*/.gz/*' | wc -l) files in $OUT"
