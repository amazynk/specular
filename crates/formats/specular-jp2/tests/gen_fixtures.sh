#!/bin/sh
# Regenerates tests/data/ with OpenJPEG's opj_compress (BSD-2). Sources are synthetic images
# made by tests/mkimg.py. Usage: tests/gen_fixtures.sh <dir with opj_compress> <work dir>
set -e
BIN=$1
WORK=$2
HERE=$(cd "$(dirname "$0")" && pwd)
OUT=$HERE/data
mkdir -p "$OUT" "$WORK"
python3 -I "$HERE/mkimg.py" 203 157 3 "$WORK/rgb.ppm"
python3 -I "$HERE/mkimg.py" 131 97 1 "$WORK/gray.pgm"
python3 -I "$HERE/mkimg.py" 120 90 1 "$WORK/gray12.pgm" 4095
python3 -I "$HERE/mkimg.py" 70 66 3 "$WORK/small.ppm"
python3 -I "$HERE/mkimg.py" 77 61 1 "$WORK/gray16.pgm" 65535
python3 -I "$HERE/mkimg.py" 90 70 4 "$WORK/rgba.pam"
python3 -I "$HERE/mkimg.py" 64 50 2 "$WORK/graya.pam"
python3 -I "$HERE/mkimg.py" 48 40 2 "$WORK/graya16.pam" 65535
c() { name=$1; src=$2; shift 2; "$BIN/opj_compress" -i "$WORK/$src" -o "$OUT/$name" "$@" >/dev/null; }
c rgb_rev.j2k rgb.ppm
c rgb_97.j2k rgb.ppm -I -r 12
c rgb_layers.jp2 rgb.ppm -r 50,20,8,1
c rgb_97_layers.j2k rgb.ppm -I -r 60,25,10
c rgb_tiles.j2k rgb.ppm -t 64,48
c rgb_tiles_97.jp2 rgb.ppm -t 80,70 -I -r 10
c rgb_tileorigin.j2k rgb.ppm -t 50,50 -T 7,11 -d 9,13
c rgb_prec.j2k rgb.ppm -c [32,32],[32,32],[16,16] -b 16,16
c rgb_rlcp.j2k rgb.ppm -p RLCP -r 30,10,1 -c [32,32],[16,16]
c rgb_rpcl.j2k rgb.ppm -p RPCL -r 30,10,1 -c [32,32],[16,16] -t 100,90
c rgb_pcrl.j2k rgb.ppm -p PCRL -r 30,10,1 -c [64,64],[32,32] -t 128,128
c rgb_cprl.j2k rgb.ppm -p CPRL -r 30,10,1 -c [32,32],[16,16]
c rgb_poc.j2k rgb.ppm -r 30,10,1 -POC T0=0,0,3,6,3,LRCP/T0=0,0,3,6,3,RPCL -n 5
c rgb_sop_eph.j2k rgb.ppm -SOP -EPH -r 20,5
c rgb_plt_tlm.j2k rgb.ppm -PLT -TLM -t 100,100
c rgb_tileparts.j2k rgb.ppm -t 96,96 -u R -r 20,5
c rgb_cb_small.j2k rgb.ppm -b 8,8 -n 3
c rgb_cb_wide.j2k rgb.ppm -b 128,8 -I
for m in 1 2 4 8 16 32 63; do c "rgb_mode$m.j2k" rgb.ppm -M $m -r 20,4,1; done
c rgb_mode63_97.j2k rgb.ppm -M 63 -I -r 15
c rgb_mode1_lossless.j2k rgb.ppm -M 1
c rgb_roi.j2k rgb.ppm -ROI c=0,U=4 -I -r 30
c rgb_nl1.j2k rgb.ppm -n 1
c rgb_nl8.j2k small.ppm -n 6
c rgb_sub.jp2 small.ppm -s 2,2
c rgb_nomct.j2k rgb.ppm -mct 0
c gray_rev.jp2 gray.pgm
c gray_97.j2k gray.pgm -I -r 15
c gray_tiles.j2k gray.pgm -t 32,32 -p CPRL
c gray12_rev.j2k gray12.pgm
c gray12_97.jp2 gray12.pgm -I -r 20
c small_rev.j2k small.ppm -n 4 -b 4,4
c gray16_rev.j2k gray16.pgm
c gray16_97.j2k gray16.pgm -I -r 10
c rgba_rev.jp2 rgba.pam
c rgba_97.jp2 rgba.pam -I -r 12
c graya_rev.jp2 graya.pam
c graya16_rev.jp2 graya16.pam
c index.j2k gray.pgm
python3 -I "$HERE/mkpal.py" "$OUT/index.j2k" "$OUT/palette.jp2"
rm "$OUT/index.j2k"
c sop_eph_tiles.j2k rgb.ppm -SOP -EPH -t 96,96 -u R -r 20,5,1
python3 -I "$HERE/mkppx.py" ppt "$OUT/sop_eph_tiles.j2k" "$OUT/rgb_ppt.j2k"
python3 -I "$HERE/mkppx.py" ppm "$OUT/sop_eph_tiles.j2k" "$OUT/rgb_ppm.j2k"
rm "$OUT/sop_eph_tiles.j2k"
