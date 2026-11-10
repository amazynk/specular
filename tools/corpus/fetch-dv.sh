#!/usr/bin/env bash
# Fetch the DV corpus for specular-dv into $SPECULAR_DV_CORPUS (default
# ${CARGO_TARGET_DIR:-target}/corpus/dv; never committed) and make ffmpeg's references.
#
# Usage: tools/corpus/fetch-dv.sh
#
# What it holds (about 75 MB downloaded, about 150 MB after extraction and references):
#   dl/          public samples from samples.ffmpeg.org (test material distributed for FFmpeg):
#                consumer DV (PAL 4:2:0 and NTSC 4:1:1, raw .dv and AVI), a tape dropout, DVCPRO
#                (4:1:1 PAL in QuickTime) and DVCPRO50 (4:2:2 in AVI)
#   NAME.dv      raw DV frames: the downloaded .dv files, the first 40 frames of the others
#                (stream copy), and synthetic frames ffmpeg encodes from test patterns in every
#                sampling (gen-*: 525/625, 4:1:1, 4:2:0, 4:2:2, 16:9, with 48 kHz sound; *-12bit: 12-bit 32 kHz)
#   NAME.yuv     ffmpeg's decode of NAME.dv in its own pixel format (every frame)
#   NAME.pcm     ffmpeg's decode of the sound, s16le stereo, when the file has sound
#
# Test that uses it (prints ORACLE rows):
#   SPECULAR_DV_CORPUS=... cargo test -p specular-dv --test oracle -- --ignored --nocapture
#
# Downloads are untrusted data: every file is checked against its size and SHA-256 below, and
# nothing from them is run. Stops if free space drops below MIN_FREE_GB (default 6). Needs
# ffmpeg (8.x) on PATH.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_DV_CORPUS:-${CARGO_TARGET_DIR:-$ROOT/target}/corpus/dv}
MIN_FREE_GB=${MIN_FREE_GB:-6}
SAMPLES=https://samples.ffmpeg.org
mkdir -p "$OUT/dl"

die() { echo "fetch-dv: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

# path on samples.ffmpeg.org | size | sha256
FILES=(
  "DV-raw/shots0000.dv|144000|eb0ef4218cb28462d44b580bd230337f748f62343db5fde9559f0408128a548d"
  "DV-raw/small_test2.dv|10240000|98430695ac08a55e23a34c4b8102457d4c7ac379298dffe5853bf50f0a034e0a"
  "archive/video/dvvideo/dv+dvvideo+pcm_s16le++dropout.dv|4752000|85e1161405a52a337819e252666bbf7dde8738fa13acc6036b836eae73abdd68"
  "archive/video/dvvideo/dv+dvvideo+pcm_s16le++mplayer-ppc-dv-bugs-mpeg-ps.dv|11808000|f3840a94ef723e7298eabdedac56be6b07ce68f32e96a30ee7cdcfd95812ce88"
  "archive/video/dvvideo/avi+dvvideo+pcm_s16le++minidvbug.avi|5242880|12e34fa949d3be236ab832ee9aa752c94d6a901b294dd3cc720a1fd68222a979"
  "V-codecs/DVSD/pond.dv|10231808|2a8bb24d6aae593a13e322f18aeaa3760f54f370893f74230975e111d9600500"
  "V-codecs/DV50/dvcpro50.avi|1448332|5e01035411030b45bc3e42db5aa804bc647451fd8e6fb848adf7aa6ac0a1f914"
  "mov/FCP/DVCPRO.mov|30495236|1db0505a5aea407b098b0118d442fa348f9fc84a399978801ab651828eb4cac2"
)

fetch() {
  local path=$1 size=$2 sum=$3 dst
  dst="$OUT/dl/$(basename "$path")"
  if [[ -f $dst ]] && [[ $(stat -c %s "$dst") == "$size" ]] \
     && sha256sum "$dst" | grep -q "^$sum "; then
    return
  fi
  check_space
  curl -fsSL --max-time 600 -o "$dst.part" "$SAMPLES/$path"
  [[ $(stat -c %s "$dst.part") == "$size" ]] || die "$path: wrong size"
  sha256sum "$dst.part" | grep -q "^$sum " || die "$path: wrong SHA-256"
  mv "$dst.part" "$dst"
}

# Short names for the corpus files.
name_of() {
  case $(basename "$1") in
    *dropout*) echo dropout ;;
    *mpeg-ps*) echo ppc-mpeg-ps ;;
    *minidvbug*) echo minidvbug ;;
    *) basename "$1" | sed 's/\.[a-z0-9]*$//' ;;
  esac
}

refs() { # NAME: the references of OUT/NAME.dv
  local n=$1
  ffmpeg -nostdin -v error -y -f dv -i "$OUT/$n.dv" -map 0:v:0 -f rawvideo "$OUT/$n.yuv"
  if ffprobe -v error -f dv -select_streams a:0 -show_entries stream=index -of csv=p=0 \
       "$OUT/$n.dv" | grep -q .; then
    ffmpeg -nostdin -v error -y -f dv -i "$OUT/$n.dv" -map 0:a:0 -f s16le -ac 2 "$OUT/$n.pcm" \
      || rm -f "$OUT/$n.pcm"
  fi
}

for e in "${FILES[@]}"; do
  IFS='|' read -r path size sum <<< "$e"
  fetch "$path" "$size" "$sum"
  src="$OUT/dl/$(basename "$path")"
  n=$(name_of "$path")
  case $path in
    *.dv) cp "$src" "$OUT/$n.dv" ;;
    *) ffmpeg -nostdin -v error -y -i "$src" -map 0:v:0 -frames:v 40 -c copy -f rawvideo "$OUT/$n.dv" ;;
  esac
  refs "$n"
done

# Synthetic frames in every sampling, with sound.
gen() { # name size rate pixfmt [extra video filter]
  check_space
  ffmpeg -nostdin -v error -y -f lavfi -i "testsrc2=s=$2:r=$3,noise=alls=20:allf=t${5:-}" \
    -f lavfi -i "sine=f=440:r=48000" -frames:v 6 -pix_fmt "$4" -ac 2 -shortest \
    -f dv "$OUT/gen-$1.dv"
  refs "gen-$1"
}
gen ntsc411 720x480 30000/1001 yuv411p
gen pal420 720x576 25 yuv420p
gen pal411 720x576 25 yuv411p
gen ntsc422 720x480 30000/1001 yuv422p
gen pal422 720x576 25 yuv422p
gen pal420-smooth 720x576 25 yuv420p ",gblur=sigma=3"
gen ntsc411-busy 720x480 30000/1001 yuv411p ",noise=alls=80:allf=t"
# 16:9 (the VAUX display mode) from a wide source.
check_space
ffmpeg -nostdin -v error -y -f lavfi -i "testsrc2=s=1024x576:r=25" -frames:v 3 \
  -vf "scale=720:576,setdar=16/9" -pix_fmt yuv420p -f dv "$OUT/gen-pal420-wide.dv"
refs gen-pal420-wide
# 12-bit nonlinear sound at 32 kHz (no encoder writes it: tools/corpus/dv12-audio.py rewrites
# the sound of two of the files above; ffmpeg's decode is the oracle).
for n in ntsc411 pal420; do
  python3 -I "$ROOT/tools/corpus/dv12-audio.py" "$OUT/gen-$n.dv" "$OUT/gen-$n-12bit.dv"
  refs "gen-$n-12bit"
done
ls -l "$OUT" | head -40
