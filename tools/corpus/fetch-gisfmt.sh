#!/bin/sh
# Fetch the small public corpus for the GIS raster and vector formats of the fmt500d corpus
# (about 250 KB) into the folder given, default $CARGO_TARGET_DIR/corpus/gisfmt:
# BSB/KAP charts, ER Mapper, SAGA, Arc/Info binary grids, GDAL VRT (with the GeoTIFFs they
# name), S-57 ENC cells and TopoJSON, all from GDAL's autotest data (MIT) pinned at v3.11.0.
# IGC and NMEA samples are written by the oracle (tools/corpus/gisfmt-oracle.py), which also
# writes more VRT, ER Mapper and SAGA files through rasterio and records what rasterio,
# pyogrio and pynmea2 read from every file:
#
#     tools/corpus/fetch-gisfmt.sh
#     python3 -m venv $CARGO_TARGET_DIR/venv
#     PIP_NO_CACHE_DIR=1 $CARGO_TARGET_DIR/venv/bin/pip install --only-binary=:all: \
#         rasterio==1.5.2 pyogrio==0.13.0 pynmea2 numpy
#     $CARGO_TARGET_DIR/venv/bin/python -I tools/corpus/gisfmt-oracle.py $CARGO_TARGET_DIR/corpus/gisfmt
#     SPECULAR_GISFMT_CORPUS=$CARGO_TARGET_DIR/corpus/gisfmt \
#         cargo test -p specular-gis --test gisfmt_oracle -- --ignored --nocapture
#
# Downloads are only written to disk (gitignored when under the repository), never run.
set -eu
out=${1:-${CARGO_TARGET_DIR:?set CARGO_TARGET_DIR or pass a folder}/corpus/gisfmt}
mkdir -p "$out"
base=https://raw.githubusercontent.com/OSGeo/gdal/447eb5238bb6ef2837e68bf2ec742c64007b680b/autotest
# $1 path under autotest, $2 size in bytes, $3 destination under $out.
get() {
    dest=$out/$3
    if [ -f "$dest" ] && [ "$(wc -c < "$dest")" -eq "$2" ]; then
        return
    fi
    mkdir -p "$(dirname "$dest")"
    echo "fetching $3"
    curl -fsSL --retry 3 -o "$dest.part" "$base/$1"
    got=$(wc -c < "$dest.part")
    [ "$got" -eq "$2" ] || { echo "$1: $got bytes, expected $2" >&2; exit 1; }
    mv "$dest.part" "$dest"
}
while read -r path size dest; do
    [ -n "$path" ] && get "$path" "$size" "$dest"
done <<'EOF'
gdrivers/data/bsb/rgbsmall.kap 4982 bsb/rgbsmall.kap
gdrivers/data/bsb/rgbsmall_index.kap 5188 bsb/rgbsmall_index.kap
gdrivers/data/bsb/rgbsmall_truncated.kap 4930 bsb/rgbsmall_truncated.kap
gdrivers/data/bsb/rgbsmall_truncated2.kap 4979 bsb/rgbsmall_truncated2.kap
gdrivers/data/bsb/rgbsmall_with_line_break.kap 4983 bsb/rgbsmall_with_line_break.kap
gdrivers/data/bsb/transverse_mercator.kap 5266 bsb/transverse_mercator.kap
gdrivers/data/bsb/australia4c.kap 11863 bsb/australia4c.kap
gdrivers/data/bsb/NZ52201_truncated.KAP 14357 bsb/NZ52201_truncated.KAP
gdrivers/data/ers/srtm.ers 1241 ers/srtm.ers
gdrivers/data/ers/srtm 240 ers/srtm
gdrivers/data/ers/8s.ers 768 ers/8s.ers
gdrivers/data/ers/8s 300 ers/8s
gdrivers/data/ers/ers_dem.ers 783 ers/ers_dem.ers
gdrivers/data/ers/ers_dem 4832 ers/ers_dem
gdrivers/data/saga/4byteFloat.sdat 400 saga/4byteFloat.sdat
gdrivers/data/saga/4byteFloat.sgrd 310 saga/4byteFloat.sgrd
gdrivers/data/saga/4byteFloat.prj 417 saga/4byteFloat.prj
gdrivers/data/saga/4byteFloat.sg-grd-z 1665 saga/4byteFloat.sg-grd-z
gdrivers/data/aigrid/abc3x1/abc3x1.clr 3842 aig/abc3x1/abc3x1.clr
gdrivers/data/aigrid/abc3x1/dblbnd.adf 32 aig/abc3x1/dblbnd.adf
gdrivers/data/aigrid/abc3x1/hdr.adf 308 aig/abc3x1/hdr.adf
gdrivers/data/aigrid/abc3x1/prj.adf 161 aig/abc3x1/prj.adf
gdrivers/data/aigrid/abc3x1/sta.adf 32 aig/abc3x1/sta.adf
gdrivers/data/aigrid/abc3x1/vat.adf 24 aig/abc3x1/vat.adf
gdrivers/data/aigrid/abc3x1/w001001.adf 118 aig/abc3x1/w001001.adf
gdrivers/data/aigrid/abc3x1/w001001x.adf 108 aig/abc3x1/w001001x.adf
gdrivers/data/aigrid/ABC3X1UC/DBLBND.ADF 32 aig/ABC3X1UC/DBLBND.ADF
gdrivers/data/aigrid/ABC3X1UC/HDR.ADF 308 aig/ABC3X1UC/HDR.ADF
gdrivers/data/aigrid/ABC3X1UC/PRJ.ADF 161 aig/ABC3X1UC/PRJ.ADF
gdrivers/data/aigrid/ABC3X1UC/STA.ADF 32 aig/ABC3X1UC/STA.ADF
gdrivers/data/aigrid/ABC3X1UC/VAT.ADF 24 aig/ABC3X1UC/VAT.ADF
gdrivers/data/aigrid/ABC3X1UC/W001001.ADF 118 aig/ABC3X1UC/W001001.ADF
gdrivers/data/aigrid/ABC3X1UC/W001001X.ADF 108 aig/ABC3X1UC/W001001X.ADF
gdrivers/data/aigrid/ABC3X1UC.CLR 3842 aig/ABC3X1UC.CLR
gdrivers/data/aigrid/aigrid_sta_24bytes/teststa/dblbnd.adf 32 aig/teststa/dblbnd.adf
gdrivers/data/aigrid/aigrid_sta_24bytes/teststa/hdr.adf 308 aig/teststa/hdr.adf
gdrivers/data/aigrid/aigrid_sta_24bytes/teststa/prj.adf 114 aig/teststa/prj.adf
gdrivers/data/aigrid/aigrid_sta_24bytes/teststa/sta.adf 24 aig/teststa/sta.adf
gdrivers/data/aigrid/aigrid_sta_24bytes/teststa/vat.adf 24 aig/teststa/vat.adf
gdrivers/data/aigrid/aigrid_sta_24bytes/teststa/w001001.adf 8174 aig/teststa/w001001.adf
gdrivers/data/aigrid/aigrid_sta_24bytes/teststa/w001001x.adf 300 aig/teststa/w001001x.adf
gcore/data/byte.vrt 1542 vrt/byte.vrt
gcore/data/byte.tif 736 vrt/byte.tif
gcore/data/int16.vrt 1255 vrt/int16.vrt
gcore/data/int16.tif 1136 vrt/int16.tif
gcore/data/float32.vrt 1373 vrt/float32.vrt
gcore/data/float32.tif 1936 vrt/float32.tif
gcore/data/mfloat32.vrt 2085 vrt/mfloat32.vrt
gcore/data/uint16_3band.vrt 2472 vrt/uint16_3band.vrt
gcore/data/msubwinbyte.vrt 1777 vrt/msubwinbyte.vrt
gcore/data/utmsmall.vrt 1377 vrt/utmsmall.vrt
gcore/data/utmsmall.tif 10360 vrt/utmsmall.tif
gcore/data/byte_nearest_50pct.vrt 1558 vrt/byte_nearest_50pct.vrt
gcore/data/byte_nearest_200pct.vrt 1558 vrt/byte_nearest_200pct.vrt
gcore/data/nan32_nodata.vrt 1607 vrt/nan32_nodata.vrt
gcore/data/nan32_nodata.tif 772 vrt/nan32_nodata.tif
ogr/data/s57/1B5X02NE.000 9362 s57/1B5X02NE.000
ogr/data/s57/bug1526.000 8264 s57/bug1526.000
ogr/data/s57/bug2147_3R7D0889.000 42267 s57/bug2147_3R7D0889.000
ogr/data/s57/fake_s57.000 247 s57/fake_s57.000
ogr/data/s57/fake_s57_variant_C151.000 247 s57/fake_s57_variant_C151.000
ogr/data/topojson/topojson1.topojson 2826 topojson/topojson1.topojson
ogr/data/topojson/topojson2.topojson 354 topojson/topojson2.topojson
ogr/data/topojson/topojson3.topojson 250 topojson/topojson3.topojson
ogr/data/topojson/topojson_with_crs.topojson 440 topojson/topojson_with_crs.topojson
EOF
echo "corpus in $out"
