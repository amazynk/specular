#!/usr/bin/env bash
# Generate an AAC-LC test corpus for specular-aac with the installed encoders (gitignored:
# nothing here is committed), with ffmpeg's float decode of each stream beside it.
#
# Usage: tools/corpus/gen-aac.sh
#   OUT defaults to $SPECULAR_AAC_GEN, else target/aac-gen.
#   Then: SPECULAR_AAC_GEN=<OUT> cargo test -p specular-aac --test oracle (ADTS streams)
#   and   SPECULAR_AAC_GEN=<OUT> cargo test -p specular-video --test aac_oracle (.m4a)
#
# Sources: the desktop's sound theme (/usr/share/sounds, concatenated into a 10 s clip), a
# chirp, noise bursts and clicks (short windows), and a 5.1 mix of them. Encoders: ffmpeg's
# own AAC encoder (all its tool switches: PNS, intensity, M/S, TNS, PCE) and faac when
# installed. Each stream is NAME.aac (ADTS) or NAME.m4a, with NAME.f32 (interleaved f32le,
# ffmpeg) and NAME.ch (channel count).
# Stops if free space on the target filesystem drops below MIN_FREE_GB (default 8).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_AAC_GEN:-$ROOT/target/aac-gen}
MIN_FREE_GB=${MIN_FREE_GB:-8}
mkdir -p "$OUT"
command -v ffmpeg >/dev/null || { echo "gen-aac: ffmpeg is needed" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    echo "gen-aac: only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping" >&2
    exit 1
  fi
}

ff() { ffmpeg -nostdin -v error -y "$@"; }

# Source clips (48 kHz float WAV).
src=$OUT/src
mkdir -p "$src"
if [[ ! -s $src/music.wav ]]; then
  list=$src/list.txt
  : > "$list"
  n=0
  for f in /usr/share/sounds/*.ogg /usr/share/sounds/*/stereo/*.oga /usr/share/sounds/*/stereo/*.ogg; do
    [[ -e $f ]] || continue
    echo "file '$f'" >> "$list"
    n=$((n + 1))
    (( n >= 40 )) && break
  done
  if (( n > 0 )); then
    ff -v fatal -f concat -safe 0 -i "$list" -t 10 -ar 48000 -ac 2 -c:a pcm_f32le "$src/music.wav"
  else
    ff -f lavfi -i "sine=f=440:d=10" -ar 48000 -ac 2 -c:a pcm_f32le "$src/music.wav"
  fi
fi
[[ -s $src/chirp.wav ]] || ff -f lavfi -i "aevalsrc='0.5*sin(2*PI*(50+t*1000)*t)|0.4*sin(2*PI*(80+t*900)*t+1)':s=48000:d=10" -c:a pcm_f32le "$src/chirp.wav"
# Clicks and noise bursts make encoders switch to short windows.
[[ -s $src/clicks.wav ]] || ff -f lavfi -i "aevalsrc='0.8*lt(mod(t,0.25),0.002)*sin(2*PI*3000*t)+0.3*lt(mod(t,1),0.05)*(random(0)-0.5)|0.7*lt(mod(t+0.1,0.3),0.003)':s=48000:d=8" -c:a pcm_f32le "$src/clicks.wav"
[[ -s $src/noise.wav ]] || ff -f lavfi -i "anoisesrc=d=6:c=pink:r=48000:a=0.3" -ac 2 -c:a pcm_f32le "$src/noise.wav"
[[ -s $src/surround.wav ]] || ff -i "$src/music.wav" -i "$src/chirp.wav" -i "$src/clicks.wav" -filter_complex \
  "[0:a]channelsplit=channel_layout=stereo[l][r];[1:a]channelsplit=channel_layout=stereo[c][x];[2:a]channelsplit=channel_layout=stereo[ls][rs];[x]volume=0.3,lowpass=f=120[lfe];[l][r][c][lfe][ls][rs]join=inputs=6:channel_layout=5.1[a]" \
  -map "[a]" -t 8 -c:a pcm_f32le "$src/surround.wav"

# enc NAME SOURCE CONTAINER(aac|m4a) ffmpeg-args...
enc() {
  local name=$1 source=$2 ext=$3
  shift 3
  check_space
  local f=$OUT/$name.$ext
  if [[ ! -s $f ]]; then
    ff -i "$src/$source.wav" "$@" -c:a aac "$f"
  fi
  ref "$f"
}

# ref FILE: ffmpeg's float decode and the channel count.
ref() {
  local f=$1 base=${1%.*}
  [[ -s $base.f32 ]] || ff -i "$f" -map 0:a:0 -c:a pcm_f32le -f f32le "$base.f32"
  ffprobe -v error -select_streams a:0 -show_entries stream=channels -of csv=p=0 "$f" > "$base.ch"
}

for rate in 8000 11025 12000 16000 22050 24000 32000 44100 48000 64000 88200 96000; do
  enc "lc_${rate}_2" music aac -ar "$rate" -b:a 128k
  enc "lc_${rate}_1" music aac -ar "$rate" -ac 1 -b:a 64k
done
enc lc_7350_1 music aac -ar 7350 -ac 1 -b:a 16k
enc lc_low_2 music aac -b:a 32k
enc lc_high_2 music aac -b:a 320k
enc lc_fast_2 music aac -aac_coder fast -b:a 96k
enc lc_nopns_2 music aac -aac_pns 0 -b:a 64k
enc lc_pns_2 noise aac -aac_pns 1 -b:a 48k
enc lc_pns_1 noise aac -aac_pns 1 -ac 1 -b:a 24k
enc lc_is_2 music aac -aac_is 1 -b:a 48k
enc lc_nois_2 music aac -aac_is 0 -b:a 48k
enc lc_ms_2 music aac -aac_ms 1 -b:a 96k
enc lc_notns_2 clicks aac -aac_tns 0 -b:a 96k
enc lc_tns_2 clicks aac -aac_tns 1 -b:a 96k
enc lc_clicks_1 clicks aac -ac 1 -b:a 64k
enc lc_chirp_2 chirp aac -b:a 96k
enc lc_pce_2 music aac -aac_pce 1 -b:a 128k
enc lc_51 surround aac -b:a 384k
enc lc_51_pce surround aac -aac_pce 1 -b:a 384k
enc lc_51_low surround aac -b:a 160k
enc lc_51_44k surround aac -ar 44100 -b:a 256k
enc m4a_lc_2 music m4a -b:a 128k
enc m4a_lc_51 surround m4a -b:a 320k

# faac: a second encoder with its own choices of tools and windows.
if command -v faac >/dev/null; then
  for s in music clicks chirp; do
    f=$OUT/faac_$s.aac
    [[ -s $f ]] || faac -q 100 -o "$f" "$src/$s.wav" >/dev/null 2>&1 || { rm -f "$f"; continue; }
    ref "$f"
  done
  f=$OUT/faac_music_1.aac
  if [[ ! -s $f ]]; then
    ff -i "$src/music.wav" -ac 1 -ar 32000 -c:a pcm_s16le "$src/music_1.wav"
    faac -b 48 -o "$f" "$src/music_1.wav" >/dev/null 2>&1 || rm -f "$f"
  fi
  [[ -s $f ]] && ref "$f"
fi
echo "gen-aac: corpus in $OUT ($(du -sh "$OUT" | cut -f1))"
