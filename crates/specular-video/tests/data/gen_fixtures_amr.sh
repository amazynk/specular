#!/bin/sh
# AMR and AMR-WB sound in its containers, cut from specular-amr's fixtures (made by
# tools/corpus/amr-oracle.py with libopencore-amrnb / libvo-amrwbenc), with the 3GPP
# reference decoders' output as the oracle: .pcm (s16le, mono, 8 or 16 kHz).
# Run from this directory with the reference decoders that tools/corpus/fetch-amr.sh
# builds:  REF=<corpus>/refbuild sh gen_fixtures_amr.sh
#   snd-amr.amr   AMR 12.2 kbit/s storage file, 0.3 s
#   snd-awb.awb   AMR-WB storage file with mode switching and comfort noise, 0.3 s
#   amr-nb.3gp    MPEG-4 Part 2 with AMR (`samr`), 0.5 s
#   amr-wb.3gp    MPEG-4 Part 2 with AMR-WB (`sawb`), 0.5 s
#   amr-wb.mkv    Matroska AMR-WB as ffmpeg writes it (A_MS/ACM, format tag 0x58)
#   amr-wbid.mkv  the same with the codec ID A_AMR/WB (oracle: amr-wb.pcm)
set -e
: "${REF:?set REF to the refbuild folder of tools/corpus/fetch-amr.sh}"
src=../../../formats/specular-amr/tests/data
q="-nostdin -loglevel error -y"
cut() { python3 -I -c "
import sys
d = open(sys.argv[1], 'rb').read()
magic = int(sys.argv[2]); first = int(sys.argv[3]); count = int(sys.argv[4])
wb = magic == 9
size = ([17, 23, 32, 36, 40, 46, 50, 58, 60, 5] if wb else [12, 13, 15, 17, 19, 20, 26, 31, 5]) + [0] * 7
out, at, k = [d[:magic]], magic, 0
while at < len(d):
    n = 1 + size[(d[at] >> 3) & 15]
    if first <= k < first + count:
        out.append(d[at:at + n])
    at += n; k += 1
open(sys.argv[5], 'wb').write(b''.join(out))
" "$@"; }
cut $src/nb-122.amr 6 0 15 snd-amr.amr
cut $src/wb-switch-dtx.awb 9 30 15 snd-awb.awb
cut $src/nb-122.amr 6 0 25 amr-nb.tmp.amr
cut $src/wb-2385.awb 9 0 25 amr-wb.tmp.awb
"$REF/nb/decoder" snd-amr.amr snd-amr.pcm > /dev/null 2>&1
"$REF/wb/decoder" -mime snd-awb.awb snd-awb.pcm > /dev/null 2>&1
"$REF/nb/decoder" amr-nb.tmp.amr amr-nb.pcm > /dev/null 2>&1
"$REF/wb/decoder" -mime amr-wb.tmp.awb amr-wb.pcm > /dev/null 2>&1
pic="-f lavfi -i testsrc2=size=96x64:rate=10:duration=0.5"
ffmpeg $q $pic -i amr-nb.tmp.amr -c:v mpeg4 -q:v 6 -c:a copy -shortest amr-nb.3gp
ffmpeg $q $pic -i amr-wb.tmp.awb -c:v mpeg4 -q:v 6 -c:a copy -shortest amr-wb.3gp
ffmpeg $q -i amr-wb.tmp.awb -c:a copy amr-wb.mkv
python3 -I -c "
import sys
d = open(sys.argv[1], 'rb').read()
assert d.count(b'A_MS/ACM') == 1
open(sys.argv[2], 'wb').write(d.replace(b'A_MS/ACM', b'A_AMR/WB'))
" amr-wb.mkv amr-wbid.mkv
rm -f amr-nb.tmp.amr amr-wb.tmp.awb
