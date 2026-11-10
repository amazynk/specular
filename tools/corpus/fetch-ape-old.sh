#!/usr/bin/env bash
# Fetch Monkey's Audio files from versions 3.80-3.94 for specular-ape's `before395` test, and
# write the reference decodes beside them (gitignored: nothing here is committed).
#
# Usage: tools/corpus/fetch-ape-old.sh [OUT]
#   OUT defaults to $SPECULAR_APE_OLD, else target/ape-old.
#   MAC is the Monkey's Audio 13.43 reference decoder (BSD), default
#   $SPECULAR_LOSSLESS_CORPUS/bin/mac as tools/corpus/fetch-lossless.sh builds it.
#   Then: SPECULAR_APE_OLD=<OUT> cargo test -p specular-ape --test before395 -- --nocapture
#
# The files are FFmpeg's FATE samples (fate-suite.ffmpeg.org/lossless-audio, test material
# only, about 4 MB): the same music encoded by Monkey's Audio 3.80, 3.88, 3.89 beta 1, 3.91
# beta 1, 3.92 beta 2 and 3.94 beta 1 at the normal (c2000) and extra high (c4000) levels,
# each cut to 328000 bytes; and luckynight-partial (3.99, cut to 1 MB) for the frame at the
# cut, which mac gives up on and ffmpeg decodes. Each gets NAME.mac.wav (`mac -d`) and, with ffmpeg on PATH,
# NAME.ff.raw (16-bit); the two agree over every sample both decode, and the script stops
# if they don't. Downloads are untrusted data: they are only hashed and handed to decoders.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${1:-${SPECULAR_APE_OLD:-$ROOT/target/ape-old}}
MAC=${MAC:-${SPECULAR_LOSSLESS_CORPUS:-${CARGO_TARGET_DIR:-$ROOT/target}/corpus/lossless}/bin/mac}
mkdir -p "$OUT"
die() { echo "fetch-ape-old: $*" >&2; exit 1; }
avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
(( avail_kb > 8 * 1024 * 1024 )) || die "less than 8 GB free; stopping"
[[ -x $MAC ]] || die "no reference decoder at $MAC (run tools/corpus/fetch-lossless.sh, or set MAC)"

while read -r name sum; do
  [[ -z $name ]] && continue
  if [[ ! -s $OUT/$name ]]; then
    curl -fsSL --retry 3 -o "$OUT/$name.part" "https://fate-suite.ffmpeg.org/lossless-audio/$name" \
      || { rm -f "$OUT/$name.part"; die "download failed: $name"; }
    mv "$OUT/$name.part" "$OUT/$name"
  fi
  echo "$sum  $OUT/$name" | sha256sum -c --quiet - || { rm -f "$OUT/$name"; die "$name: checksum mismatch"; }
  b=$OUT/${name%.ape}
  # the files are cut: mac stops with an error at the cut, after writing the whole frames
  [[ -s $b.mac.wav ]] || "$MAC" "$OUT/$name" "$b.mac.wav" -d >/dev/null 2>&1 || true
  [[ -s $b.mac.wav ]] || die "$name: mac wrote nothing"
  if command -v ffmpeg >/dev/null && [[ ! -s $b.ff.raw ]]; then
    ffmpeg -nostdin -v quiet -y -i "$OUT/$name" -f s16le "$b.ff.raw" || true
  fi
done <<'EOF'
luckynight-mac380-c2000.ape 0e7ebad37905f2d52c578acf44ffcff799016640068d552e12a2a810cb16e5b7
luckynight-mac380-c4000.ape a82adead4ea0473d4691a1fc33e0660068d359077b6cba20005e0d94520c1b61
luckynight-mac388-c2000.ape 73d7dd1e452e2f02745e00e90a501a7f4612fde2b3448fcf9dfa3fa692aef42a
luckynight-mac388-c4000.ape 747c0142d75c07850b1bd0a261bac24724c43c29eef10196fb2e46cdc0594225
luckynight-mac389b1-c2000.ape 59cb325b9be8c8f7c6137ab6c4d3446b4288ea60574b8e688441f046308f4945
luckynight-mac389b1-c4000.ape f1d55016c6b5bc49e55967e8987bb1a5d35f44fb6eb4689f70ad23f7ead29f3f
luckynight-mac391b1-c2000.ape 2a1063f472abab3d6a3669d05c426d2fbe8e9c7f4124725087dc248ae331b954
luckynight-mac391b1-c4000.ape 20295fb9146b4e80acb39b27dd75fa3028924b3a41fa2ccb6dbf99dd33123124
luckynight-mac392b2-c2000.ape ecaa78325ed4c4d70ab63625dd1848ba78d728944dd9f797a53b7b25fdfecabc
luckynight-mac392b2-c4000.ape 4a3f5ef731b31975db4d473cbfb9dd46ddfb2e760ad3b5705b79180873a38abe
luckynight-mac394b1-c2000.ape c17918e48df298a648bc3efa23e948bcd055af8cc2a631af183b10e6a198c1bc
luckynight-mac394b1-c4000.ape 208fb2b75cdfb7ef9ae7dedda0df8ebbc121ef59a78dcdb2f907b977c0e62eec
luckynight-partial.ape ce0769473b9302477c224fb1425194788bf6bab2e2dad3964d2c68a3b9711a07
EOF

# mac and ffmpeg must agree wherever both decoded
if command -v ffmpeg >/dev/null; then
  python3 -I - "$OUT" <<'PY' || die "mac and ffmpeg disagree"
import glob, os, sys
def data(w):
    at = 12
    while at + 8 <= len(w):
        n = int.from_bytes(w[at + 4:at + 8], "little")
        if w[at:at + 4] == b"data":
            return w[at + 8:at + 8 + n]
        at += 8 + n + (n & 1)
    return b""
bad = 0
for f in sorted(glob.glob(os.path.join(sys.argv[1], "luckynight-*.mac.wav"))):
    m = data(open(f, "rb").read())
    r = open(f[:-8] + ".ff.raw", "rb").read()
    n = min(len(m), len(r))
    if m[:n] != r[:n] or n == 0:
        print("differ:", os.path.basename(f)); bad += 1
sys.exit(1 if bad else 0)
PY
fi
echo "fetch-ape-old: corpus in $OUT"
