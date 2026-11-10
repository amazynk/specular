#!/bin/sh
# Regenerates tests/data/htj2k/ with OpenJPH's ojph_compress (BSD-2) from synthetic images made
# by tests/mkimg.py and tests/mkyuv.py, and writes tests/data/htj2k/MANIFEST (name, size,
# options). Usage: tests/gen_htj2k.sh <OpenJPH install prefix> <work dir>
set -e
PREFIX=$1
WORK=$2
HERE=$(cd "$(dirname "$0")" && pwd)
OUT=$HERE/data/htj2k
mkdir -p "$OUT" "$WORK"
rm -f "$OUT"/*.jph "$OUT"/*.j2c "$OUT"/*.jhc "$OUT/MANIFEST"
python3 -I "$HERE/mkimg.py" 83 61 3 "$WORK/rgb.ppm"
python3 -I "$HERE/mkimg.py" 101 77 1 "$WORK/gray.pgm"
python3 -I "$HERE/mkimg.py" 64 48 1 "$WORK/gray12.pgm" 4095
python3 -I "$HERE/mkimg.py" 77 61 1 "$WORK/gray16.pgm" 65535
python3 -I "$HERE/mkimg.py" 45 35 3 "$WORK/rgb16.ppm" 65535
python3 -I "$HERE/mkimg.py" 70 66 1 "$WORK/small.pgm"
python3 -I "$HERE/mkyuv.py" 97 75 2 2 255 "$WORK/yuv420.yuv"
python3 -I "$HERE/mkyuv.py" 96 64 2 1 1023 "$WORK/yuv422.yuv"
echo "# file bytes ojph_compress options (regenerate with tests/gen_htj2k.sh)" > "$OUT/MANIFEST"
c() {
    name=$1
    src=$2
    shift 2
    LD_LIBRARY_PATH="$PREFIX/lib" "$PREFIX/bin/ojph_compress" -i "$WORK/$src" -o "$OUT/$name" "$@" >/dev/null
    echo "$name $(wc -c < "$OUT/$name") $src $*" >> "$OUT/MANIFEST"
}
c gray8_rev.jph gray.pgm -reversible true
c gray8_97.j2c gray.pgm -qstep 0.01
c gray12_rev.jhc gray12.pgm -reversible true
c gray12_97.jph gray12.pgm -qstep 0.002
c gray16_rev.jph gray16.pgm -reversible true
c gray16_97.j2c gray16.pgm -qstep 0.0005
c rgb_rev.jph rgb.ppm -reversible true
c rgb_rev_nomct.j2c rgb.ppm -reversible true -colour_trans false
c rgb_97.jph rgb.ppm -qstep 0.008
c rgb_97_nomct.jhc rgb.ppm -qstep 0.02 -colour_trans false
c rgb16_rev.jph rgb16.ppm -reversible true
c rgb16_97.j2c rgb16.ppm -qstep 0.001
c rgb_cb16.j2c rgb.ppm -reversible true -block_size "{16,16}"
c rgb_cb_wide.j2c rgb.ppm -qstep 0.01 -block_size "{8,128}"
c rgb_cb_tall.j2c rgb.ppm -reversible true -block_size "{128,4}"
c rgb_prec.j2c rgb.ppm -reversible true -precincts "{8,8},{16,16},{32,32}" -block_size "{16,16}"
c rgb_nl0.j2c rgb.ppm -reversible true -num_decomps 0
c rgb_nl1.jph rgb.ppm -qstep 0.01 -num_decomps 1
c small_nl6.j2c small.pgm -reversible true -num_decomps 6
for p in LRCP RLCP PCRL CPRL; do
    c "rgb_$p.j2c" rgb.ppm -reversible true -prog_order $p -precincts "{16,16}" -block_size "{16,16}"
done
c rgb_tiles.j2c rgb.ppm -reversible true -tile_size "{32,24}"
c rgb_tiles_97.jph rgb.ppm -qstep 0.01 -tile_size "{48,40}" -tileparts RC -tlm_marker true
c rgb_offsets.j2c rgb.ppm -reversible true -image_offset "{9,13}" -tile_offset "{7,11}" -tile_size "{30,30}"
c gray_tileparts.j2c gray.pgm -qstep 0.02 -tile_size "{48,40}" -tileparts R -prog_order RLCP
c yuv420_rev.j2c yuv420.yuv -reversible true -dims "{97,75}" -num_comps 3 -signed false,false,false -bit_depth 8,8,8 -downsamp "{1,1},{2,2},{2,2}"
c yuv422_97.j2c yuv422.yuv -qstep 0.005 -dims "{96,64}" -num_comps 3 -signed false,false,false -bit_depth 10,10,10 -downsamp "{1,1},{2,1},{2,1}"
