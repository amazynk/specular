#!/usr/bin/env bash
# Fetch the corpus of video codecs round 4 (VC-1/WMV9, Motion JPEG and MJPEG-B, DV in AVI,
# QuickTime and Matroska, FLV, JPEG 2000 in MXF, refusals) into $SPECULAR_ROUND4_CORPUS
# (default ${CARGO_TARGET_DIR:-target}/corpus/round4; gitignored, never committed) and write
# FFmpeg's decode of each beside it as NAME.yuv (the oracle: every frame up to a limit, in the
# decoder's own pixel format; JPEG with `-idct int`).
#
#   dl/        public samples from samples.ffmpeg.org and fate-suite.ffmpeg.org (test material
#              distributed for FFmpeg), checked against the pins below
#   gen-*      clips made here with ffmpeg: FATE's VC-1 conformance streams remuxed into
#              Matroska, MP4, ASF and MPEG-TS; DV (PAL 4:2:0, NTSC 4:1:1, DVCPRO50) in
#              QuickTime, AVI and Matroska
#   list.tsv   file, container, ORACLE label, tolerance (largest sample difference allowed),
#              frames compared ("refuse": must be refused in words; "play:N": N frames must
#              decode, no reference: DCI X'Y'Z' is shown as RGB, which FFmpeg doesn't do alike)
#
# Test that uses it (prints ORACLE rows):
#   SPECULAR_ROUND4_CORPUS=... cargo test -p specular-video --test round4 corpus -- --nocapture
#
# Downloads are untrusted data: each is checked against its size and SHA-256 and nothing from
# them is run. Needs curl and ffmpeg (8.x). About 35 MB downloaded, 400 MB with the references.
# Stops below MIN_FREE_GB (default 8).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_ROUND4_CORPUS:-${CARGO_TARGET_DIR:-$ROOT/target}/corpus/round4}
MIN_FREE_GB=${MIN_FREE_GB:-8}
SAMPLES=https://samples.ffmpeg.org
FATE=https://fate-suite.ffmpeg.org
mkdir -p "$OUT/dl"

for t in curl ffmpeg sha256sum; do
  command -v $t >/dev/null || { echo "fetch-videocodecs4: $t not found" >&2; exit 1; }
done

check_space() {
  local gb
  gb=$(df -Pk "$OUT" | awk 'NR==2{print int($4/1048576)}')
  [ "$gb" -ge "$MIN_FREE_GB" ] || { echo "fetch-videocodecs4: only ${gb} GB free" >&2; exit 1; }
}

# url | size | sha256
FILES=(
  "$SAMPLES/V-codecs/WMV9/sb_wmv9_samples/sb-spl-q60.avi|147456|b7d0ae05bc3aba81e5382b5720df629297338fb6bcb134908fba4aeecfa2a2d1"
  "$SAMPLES/V-codecs/WMV9/sb_wmv9_samples/sb-main-q60.avi|151552|62ac30ded41571bd83b409e4614916a59f7f040e998c1d3f189d2ac92495fc7d"
  "$SAMPLES/V-codecs/WMV9/sb_wmv9_samples/sb-adv-q60.avi|143360|2185e8bd57d7481a65b04b57e0989caa5b628202e6046889fe1e6fd41eeedb0b"
  "$SAMPLES/V-codecs/WMV9/RES_RTM-pframes/niceday.wmv|1030830|e26444e4f03c3ef585cbb19cf6f9a7237f1f109015492281bd08e63ff718c6ac"
  "$SAMPLES/V-codecs/WMV9/B-frames/bloodyback.wmv|1517638|91812851d0a5cdbc8e455e08eb25391ffb0c91dbc01e0f6bd0eb584a15c06e47"
  "$SAMPLES/V-codecs/WMV9/wmv-samples/sample-wmv9-simple-SP@ML.wmv|1157362|30ec4142ad2c38134d9fecf64a18d5108a23eb78e2afdf7fbe1397f52442645d"
  "$SAMPLES/V-codecs/WMV9/wmv-samples/sample-wmv9-main-MP@LL.wmv|1266346|da6e00011cbe9703ed46560fee73fda8975f3ff95a7db038d93afd4b0b8ccabb"
  "$SAMPLES/V-codecs/WMV9/wmv-samples/sample-wmv9-complex-CP.wmv|1229062|737dad2b71e718319044ce8d26c1797905d8185fd3ab56afd3f85cb9cfebb8f8"
  "$SAMPLES/V-codecs/WMV9/interlaced/480i30__codec_WVC1__mode_2__framerate_29.970__type_2__preproc_17.wmv|1437578|b2ef9f4580fcba8bb808244e0313a34a54976d3f9a2f81c7cbf23f4fe4aaa159"
  "$SAMPLES/V-codecs/MJPEGs/Minolta-MJPEG.mov|2286904|256cc89ab0b9e7a36dd7a2431fcbd53729bc603ba531a817aeaa165b77b99220"
  "$SAMPLES/V-codecs/MJPEGs/m_ijpg.avi|2976982|dcae0d899e0fc33af48c71ac441d6fb0958b7d779cc29fea7682801ac1629a7d"
  "$SAMPLES/V-codecs/MJPEGs/mjpa-b.mov/mjpa.mov|1462019|0820856c1a59414f84393b0a741f1481d8f071330a4a867b61cca1a4895bd8d3"
  "$SAMPLES/V-codecs/MJPEGs/mjpa-b.mov/mjpegb.mov|884461|b47e572313213284df8336257964b0b04a0f9dd64d7f4809dc23bbde94a8415f"
  "$SAMPLES/V-codecs/MJPEGs/mjpa-b.mov/mjpegb_2.mov|258547|ed17963b51fae36836ab902d1f12b82c04070d599d7a6ea0f7f11534cbd2995b"
  "$FATE/mjpegb/mjpegb_part.mov|328244|b415f02efce75ca708f832882f797d6a18f2d25b76f835e48948d1e3db461758"
  "$FATE/mjpeg/mjpeg_field_order.avi|725126|ce3e326dc7ce75311b20d85b27ab333986cd27a7f9686887019f40abf8136aaf"
  "$SAMPLES/V-codecs/MP43/Park_auto_no_driver.wmv|4035364|8265b6c147264bb0aeda8efe7d843bc8797a6e2e1705edce74135633022b8fb9"
  "$SAMPLES/FLV/flash8/tacoma.flv|721178|e5b3bb3402ab1f53cd8f378e7c43715909356e77a935b7630111b9aae0c952e7"
  "$SAMPLES/FLV/zelda.flv|601526|d7153290fcdae628aa0569c083a96d42f19142094a4da6ec3ca8bcb69bf575b3"
  "$FATE/flv/Enigma_Principles_of_Lust-part.flv|512000|a0b6d22bd69054d1e7082de11a825e72739e3cdd3409a54349365bffcd3a40fb"
  "$FATE/flv/multitrack.flv|274432|3739b85b0f4ed7847bfa07a07b0ca8829aed9b34d56f8dc3cbf605f22673c7cf"
  "$FATE/jpeg2000/chiens_dcinema2K.mxf|1160321|4e7be78f00006e8abb3bb68cb01020085b2ec9d417b2c26ac798224b442d6d4d"
  "$SAMPLES/V-codecs/DV50/dvcpro50.avi|1448332|5e01035411030b45bc3e42db5aa804bc647451fd8e6fb848adf7aa6ac0a1f914"
  "$SAMPLES/archive/video/dvvideo/avi+dvvideo+pcm_s16le++minidvbug.avi|5242880|12e34fa949d3be236ab832ee9aa752c94d6a901b294dd3cc720a1fd68222a979"
  "$FATE/vc1/SA00040.vc1|24097|898035e63cfe0ddb1cc2c8749d7dafdfcc1b12fe5dac7e5b65941e6c4e41c860"
  "$FATE/vc1/SA10091.vc1|437691|c60a0da89873b40d7e436f1c29757c6cc71f3c51f2ac2ea8c6bd023762c94b6c"
  "$FATE/vc1/SMM0015.rcv|437157|67baabd762ed35f70cd70c492e83e8744d068a5a567d9b19cc509851bf9f1d44"
)

for e in "${FILES[@]}"; do
  IFS='|' read -r url size sum <<< "$e"
  f="$OUT/dl/$(basename "$url")"
  if [ -s "$f" ] && echo "$sum  $f" | sha256sum -c --status; then
    continue
  fi
  check_space
  curl -fsSL --retry 3 --retry-all-errors -A 'specular-corpus-fetch/1 (test corpus)' -o "$f.part" "$url"
  [ "$(stat -c %s "$f.part")" = "$size" ] || { echo "fetch-videocodecs4: $url has the wrong size" >&2; exit 1; }
  echo "$sum  $f.part" | sha256sum -c --status || { echo "fetch-videocodecs4: $url hash mismatch" >&2; exit 1; }
  mv "$f.part" "$f"
done

q=(-nostdin -loglevel error -y)
cd "$OUT"
# The oracle: FFmpeg's decode of the first `frames` pictures.
ref() {
  local src=$1 frames=$2
  ffmpeg "${q[@]}" -threads 1 -flags +bitexact -idct int -i "$src" -map 0:v:0 -fps_mode passthrough \
    -frames:v "$frames" -f rawvideo "${src%.*}.yuv"
}
# VC-1 conformance streams in the containers that carry VC-1 (one reference per source).
ffmpeg "${q[@]}" -i dl/SA00040.vc1 -c copy gen-vc1-a40.mkv
ffmpeg "${q[@]}" -i dl/SA10091.vc1 -c copy gen-vc1-a91.ts
ffmpeg "${q[@]}" -i dl/SA10091.vc1 -c copy gen-vc1-a91.mp4
ffmpeg "${q[@]}" -i dl/SMM0015.rcv -c copy gen-vc1-main.mkv
ffmpeg "${q[@]}" -i dl/SMM0015.rcv -c copy gen-vc1-main.asf
# DV: PAL 4:2:0 in QuickTime, NTSC 4:1:1 in AVI, DVCPRO50 4:2:2 in Matroska.
src=(-f lavfi -i "testsrc2=size=720x576:rate=25,trim=duration=0.4" -f lavfi -i "sine=frequency=600:sample_rate=48000:duration=0.4")
ffmpeg "${q[@]}" "${src[@]}" -c:v dvvideo -pix_fmt yuv420p -c:a pcm_s16le -ac 2 gen-dv-pal.mov
ffmpeg "${q[@]}" -f lavfi -i "testsrc2=size=720x480:rate=30000/1001,trim=duration=0.4" -c:v dvvideo -pix_fmt yuv411p gen-dv-ntsc.avi
ffmpeg "${q[@]}" -f lavfi -i "testsrc2=size=720x576:rate=25,trim=duration=0.4" -c:v dvvideo -pix_fmt yuv422p gen-dv50.mkv

# file | container | label | tolerance | frames ("refuse": refused in words)
cat > list.tsv <<'EOF'
# file	container	label	tolerance	frames
dl/sb-spl-q60.avi	avi	VC-1	0	60
dl/sb-main-q60.avi	avi	VC-1	0	60
dl/sb-adv-q60.avi	avi	VC-1	0	refuse
dl/niceday.wmv	asf	VC-1	0	60
dl/bloodyback.wmv	asf	VC-1	0	60
dl/sample-wmv9-simple-SP@ML.wmv	asf	VC-1	0	60
dl/sample-wmv9-main-MP@LL.wmv	asf	VC-1	0	60
dl/sample-wmv9-complex-CP.wmv	asf	VC-1	0	refuse
dl/480i30__codec_WVC1__mode_2__framerate_29.970__type_2__preproc_17.wmv	asf	VC-1	0	refuse
gen-vc1-a40.mkv	mkv	VC-1	0	60
gen-vc1-a91.ts	ts	VC-1	0	60
gen-vc1-a91.mp4	mp4	VC-1	0	60
gen-vc1-main.mkv	mkv	VC-1	0	60
gen-vc1-main.asf	asf	VC-1	0	60
dl/Minolta-MJPEG.mov	mov	MJPEG	1	30
dl/m_ijpg.avi	avi	MJPEG	1	20
dl/mjpa.mov	mov	MJPEG	1	30
dl/mjpegb.mov	mov	MJPEG	1	27
dl/mjpegb_2.mov	mov	MJPEG	1	6
dl/mjpegb_part.mov	mov	MJPEG	1	10
dl/mjpeg_field_order.avi	avi	MJPEG	1	9
dl/Park_auto_no_driver.wmv	asf	MS-MPEG4	0	refuse
dl/tacoma.flv	flv	FLV	0	refuse
dl/zelda.flv	flv	FLV	0	60
dl/Enigma_Principles_of_Lust-part.flv	flv	FLV	0	60
dl/multitrack.flv	flv	FLV	0	30
dl/chiens_dcinema2K.mxf	mxf	JPEG2000	0	play:2
dl/dvcpro50.avi	avi	DV	2	5
dl/avi+dvvideo+pcm_s16le++minidvbug.avi	avi	DV	2	30
gen-dv-pal.mov	mov	DV	2	10
gen-dv-ntsc.avi	avi	DV	2	12
gen-dv50.mkv	mkv	DV	2	10
EOF
while IFS=$'\t' read -r f _ _ _ n; do
  case "$f" in '#'*|'') continue ;; esac
  case "$n" in refuse|play:*) continue ;; esac
  check_space
  ref "$f" "$n"
done < list.tsv
# DV 4:1:1 is shown with each chroma sample doubled (4:2:2): the reference likewise.
python3 -I - gen-dv-ntsc.yuv 720 480 <<'PY'
import sys
path, w, h = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
d = open(path, "rb").read()
cw = w // 4
n = w * h + 2 * cw * h
out = bytearray()
for k in range(len(d) // n):
    f = d[k * n:(k + 1) * n]
    out += f[:w * h]
    for c in (f[w * h:w * h + cw * h], f[w * h + cw * h:]):
        for s in c:
            out += bytes((s, s))
open(path, "wb").write(out)
PY
echo "fetch-videocodecs4: corpus in $OUT ($(du -sh "$OUT" | cut -f1))"
