#!/usr/bin/env bash
# Fetch the video-codec corpus (MPEG-1/2, MPEG-4 Part 2, H.263, MPEG audio) into
# tools/corpus/videocodecs/ (gitignored: nothing here is committed) and make ffmpeg's
# references beside each file.
#
# Usage: tools/corpus/fetch-videocodecs.sh [SECTION...]
#   SECTION is a function name below (src_fate, src_dicom, src_refs). Default: all three. OUT defaults to
#   $SPECULAR_VIDEOCODECS_CORPUS, else tools/corpus/videocodecs. ffmpeg (8.0) must be on PATH
#   for src_refs.
#
# What it covers (about 100 MB with the references):
#   fate-mpeg2   FFmpeg's FATE suite samples (fate-suite.ffmpeg.org, test material distributed
#                with FFmpeg): DVD programme streams, field pictures with dual prime, the HHI
#                4:2:2 and Sony / TCELA conformance-style streams, XDCAM 4:2:2 in TS, VCD .mpg
#   fate-mpeg4   FATE MPEG-4 Part 2 and H.263 samples: DivX/Xvid AVI, quarter-pel, GMC,
#                interlaced ASP, B-VOPs, 3GP short-header
#   refs         per file: NAME.yuv (`ffmpeg -idct int`, every frame as decoded) and, for the
#                bare streams, NAME.m2v / NAME.m4v with NAME.framemd5 for the crate tests
#
# Tests that use it: SPECULAR_VIDEOCODECS_CORPUS=tools/corpus/videocodecs
#   cargo test -p specular-video --test mpeg fate_samples_in_their_containers
#   SPECULAR_MPEG2_CLIPS=tools/corpus/videocodecs/es cargo test -p specular-mpeg2 --test fixtures
#
# Downloads are untrusted data: every file is checked against its size and SHA-256 below, and
# nothing from them is run. Stops if free space drops below MIN_FREE_GB (default 6).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_VIDEOCODECS_CORPUS:-$ROOT/tools/corpus/videocodecs}
MIN_FREE_GB=${MIN_FREE_GB:-6}
FATE=https://fate-suite.ffmpeg.org
mkdir -p "$OUT"

die() { echo "fetch-videocodecs: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

# fetch URL DEST NAME SHA256 SIZE
fetch() {
  local url=$1 dest=$2 name=$3 want=$4 size=$5 out="$OUT/$2/$3"
  mkdir -p "$OUT/$dest"
  if [[ ! -s $out ]]; then
    check_space
    curl -fsSL --retry 3 --max-filesize $((size + 1)) -A 'specular-corpus-fetch/1 (test corpus)' \
      -o "$out.part" "$url" || die "download failed: $url"
    mv "$out.part" "$out"
  fi
  local got
  got=$(sha256sum "$out" | cut -d' ' -f1)
  [[ $got == "$want" ]] || { rm -f "$out"; die "$name: SHA-256 $got, expected $want"; }
  [[ $(stat -c %s "$out") == "$size" ]] || die "$name: size differs"
}

src_fate() {
  local d=fate-mpeg2
  fetch "$FATE/mpeg2/dvd_single_frame.vob" $d dvd_single_frame.vob 3054d653e9e575ba0592de2ce223c813d9b8bc593981a1fc184ff6f49790942b 122880
  fetch "$FATE/mpeg2/dvd_still_frame.vob" $d dvd_still_frame.vob 651dfc3a55a96de26cf6eb4a3851a8d8b5fdb57a8ab8dc8adb14e6c302fb409a 878592
  fetch "$FATE/mpeg2/hhi_burst_422_short.bits" $d hhi_burst_422_short.bits bd7b917cb5cf2871c4a3bdd8e0ac9c973792627ee64f6b0afa7b4e4a78e2ddb1 930186
  fetch "$FATE/mpeg2/matrixbench_mpeg2.lq1.mpg" $d matrixbench_mpeg2.lq1.mpg 85e49a80cd97b0c91a548ffa7ab980575d55c8e6e2c9148a1d7bd8c2ce9b77d1 210944
  fetch "$FATE/mpeg2/mpeg2_field_encoding.ts" $d mpeg2_field_encoding.ts 9688333a145b9e2c2139b9cab6cd4966acd19788fd620b4b31db5c2891547cb8 800000
  fetch "$FATE/mpeg2/sony-ct3.bs" $d sony-ct3.bs fa622b9d9a0fe24dee7eea5ebe653bd3f98d5605a4257d16c11985022eaf2875 139144
  fetch "$FATE/mpeg2/t.mpg" $d t.mpg d13177688337e32079eee871a703420e6cb503743cb6e331c961c0de0aea82ff 1349276
  fetch "$FATE/mpeg2/tcela-6.bits" $d tcela-6.bits 3b267fd327ce97ace05119022e5eda0a222d9cd65c0e360c46c8a2cd0664049a 311007
  fetch "$FATE/mpeg2/xdcam8mp2-1s_small.ts" $d xdcam8mp2-1s_small.ts c3429a5975b1e1b47602bb15cb90e72c2b8831e6349acd85f30ec834363bcb20 507788
  src_fate_mpeg4
}

# fo-dicom's MPEG-2 endoscopy video (MS-PL): 352x288, 450 frames, MPEG-2 Main Profile.
src_dicom() {
  fetch "https://raw.githubusercontent.com/fo-dicom/fo-dicom/f45aedda1e2489f37618364bd61c76885b8d8504/Tests/FO-DICOM.Tests/Test%20Data/ETIAM_video_002.dcm" \
    dicom ETIAM_video_002.dcm c69e83192ab81687bc4ca1163ca139bab3d37e6cec10ed59851c3a24241ca8d4 9073558
  local d="$OUT/dicom/ETIAM_video_002"
  if [[ ! -s $d.rgb ]] && command -v ffmpeg > /dev/null; then
    python3 -I "$ROOT/tools/corpus/dicom-es.py" "$d.dcm" "$d.m2v"
    ffmpeg -nostdin -loglevel error -y -idct int -i "$d.m2v" -fps_mode passthrough \
      -sws_flags accurate_rnd+full_chroma_int -pix_fmt rgb24 -f rawvideo "$d.rgb"
    # The bare stream for the decoder's own frame-exact test.
    mkdir -p "$OUT/es"
    cp "$d.m2v" "$OUT/es/"
    ffmpeg -nostdin -loglevel error -idct int -i "$d.m2v" -fps_mode passthrough -f framemd5 - \
      | grep -v '^#' > "$OUT/es/ETIAM_video_002.framemd5"
  fi
}

# FATE MPEG-4 Part 2 / H.263 samples.
src_fate_mpeg4() {
  local d=fate-mpeg4
  fetch "$FATE/mpeg4/demo.m4v" $d demo.m4v da1983816306e0e4003da606e139896fe9394564df17fc9be1171f1f5c7e2441 1160073
  fetch "$FATE/mpeg4/packed_bframes.avi" $d packed_bframes.avi 450a89377f024ce11d176d2be0fa3756df005aa45294866b383fee18a17cbf48 62476
  fetch "$FATE/mpeg4/xvid_vlc_trac7411.h263" $d xvid_vlc_trac7411.h263 aee58db19e76c334b126b2390aed661e6134995b7fdc614ea652222926153689 33890
  fetch "$FATE/mpeg4/resize_down-up.h263" $d resize_down-up.h263 a1a40010f9865f9beadee8d63c36202d827d31c0058503a28081555cba0766e3 439655
  fetch "$FATE/mpeg4/fragmented.mp4" $d fragmented.mp4 e8e916c5c9e166137ead5bf68413f9c7e5b215cc300a230d29c1eb438f045528 305277
  fetch "$FATE/mpeg4/mpeg4_sstp_dpcm.m4v" $d mpeg4_sstp_dpcm.m4v c7389f15ceb3381be18ab21c932ef3cda952466bcd2ade824faf36a880ea860e 253110
}

# MPEG-4 references: in containers NAME.yuv; bare streams es/NAME.EXT + es/NAME.framemd5. One
# thread: libavcodec's frame-threaded decode of field direct mode differs from its own.
refs_mpeg4() {
  local f b
  mkdir -p "$OUT/es"
  for f in "$OUT"/fate-mpeg4/*; do
    b=$(basename "$f")
    case $b in
      *.yuv) continue ;;
      *.avi | *.mp4)
        [[ -s ${f%.*}.yuv ]] || ffmpeg -nostdin -loglevel error -y -threads 1 -flags +bitexact \
          -idct int -i "$f" -map 0:v:0 -fps_mode passthrough -f rawvideo "${f%.*}.yuv" || true ;;
      *)
        cp "$f" "$OUT/es/$b"
        [[ -s $OUT/es/${b%.*}.framemd5 ]] || ffmpeg -nostdin -loglevel error -threads 1 \
          -flags +bitexact -idct int -i "$f" -fps_mode passthrough -noautoscale -f framemd5 - 2>/dev/null \
          | grep -v '^#' > "$OUT/es/${b%.*}.framemd5" || true ;;
    esac
  done
}

# refs_for FILE ES_FORMAT ES_EXT: FILE.yuv, and es/NAME.ES_EXT with es/NAME.framemd5.
refs_for() {
  local f=$1 fmt=$2 ext=$3 b
  b=$(basename "${f%.*}")
  mkdir -p "$OUT/es"
  [[ -s ${f%.*}.yuv ]] || ffmpeg -nostdin -loglevel error -y -idct int -i "$f" -map 0:v:0 \
    -fps_mode passthrough -f rawvideo "${f%.*}.yuv"
  if [[ ! -s $OUT/es/$b.framemd5 ]]; then
    ffmpeg -nostdin -loglevel error -y -i "$f" -map 0:v:0 -c copy -f "$fmt" "$OUT/es/$b.$ext"
    ffmpeg -nostdin -loglevel error -idct int -i "$OUT/es/$b.$ext" -fps_mode passthrough \
      -f framemd5 - | grep -v '^#' > "$OUT/es/$b.framemd5"
  fi
}

src_refs() {
  command -v ffmpeg > /dev/null || die "ffmpeg is needed for the references"
  check_space
  local f
  for f in "$OUT"/fate-mpeg2/*; do
    case $f in *.yuv) continue ;; esac
    refs_for "$f" mpeg2video m2v
  done
  refs_mpeg4
  # The field-coded sample is cut off inside its last frame: its bare stream stops before.
  local fe="$OUT/es/mpeg2_field_encoding"
  if [[ -s $fe.framemd5 && $(wc -l < "$fe.framemd5") == 31 ]]; then
    ffmpeg -nostdin -loglevel error -y -i "$fe.m2v" -c copy -frames:v 30 -f mpeg2video "$fe.cut.m2v"
    mv "$fe.cut.m2v" "$fe.m2v"
    head -30 "$fe.framemd5" > "$fe.cut" && mv "$fe.cut" "$fe.framemd5"
  fi
}

if (( $# == 0 )); then
  set -- src_fate src_dicom src_refs
fi
for s in "$@"; do
  "$s"
done
echo "fetch-videocodecs: corpus ready in $OUT"
