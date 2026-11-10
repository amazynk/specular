#!/usr/bin/env bash
# Fetch and build the reference coders for WavPack, Monkey's Audio and Musepack (oracle use
# only: built under the target dir, never committed, linked or bundled), then make the
# sample corpus with them.
#
# Usage: tools/corpus/fetch-lossless.sh
#   OUT defaults to $SPECULAR_LOSSLESS_CORPUS, else $CARGO_TARGET_DIR/corpus/lossless, else
#   target/corpus/lossless.
#   Then: SPECULAR_LOSSLESS_CORPUS=<OUT> cargo test -p specular-lossless --test oracle -- --ignored
#
# Sources (all BSD 3-clause decoders, SHA-256 pinned, about 4 MB):
#   - WavPack 5.8.1 (github.com/dbry/WavPack): wavpack, wvunpack.
#   - Monkey's Audio 13.43 SDK (monkeysaudio.com, BSD since 10.18, 2023-08-10): mac.
#   - Musepack r475 (musepack.net; libmpcdec BSD, mpcenc LGPL, run only): mpcenc, mpcdec.
# Downloads are untrusted data: this script only copies, unpacks and compiles them.
# Stops if free space drops below MIN_FREE_GB (default 8).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_LOSSLESS_CORPUS:-${CARGO_TARGET_DIR:-$ROOT/target}/corpus/lossless}
MIN_FREE_GB=${MIN_FREE_GB:-8}
mkdir -p "$OUT/dl" "$OUT/src" "$OUT/bin" "$OUT/samples" "$OUT/tmp"
export TMPDIR=$OUT/tmp

die() { echo "fetch-lossless: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

# get URL NAME SHA256
get() {
  local url=$1 name=$2 sum=$3
  check_space
  if [[ ! -f "$OUT/dl/$name" ]]; then
    echo "fetch-lossless: $name"
    curl -fL --progress-bar -o "$OUT/dl/$name.part" "$url"
    mv "$OUT/dl/$name.part" "$OUT/dl/$name"
  fi
  echo "$sum  $OUT/dl/$name" | sha256sum -c --quiet - || die "$name: checksum mismatch"
}

get https://github.com/dbry/WavPack/archive/refs/tags/5.8.1.tar.gz wavpack.tar.gz \
  1228dda992cf70ddda278d0a7ead410cfa8ea7f29ba23da7c6fdcbefb74ca363
get https://files.musepack.net/source/musepack_src_r475.tar.gz musepack_src_r475.tar.gz \
  a4b1742f997f83e1056142d556a8c20845ba764b70365ff9ccf2e3f81c427b2b
get https://monkeysaudio.com/files/MAC_1343_SDK.zip MAC_1343_SDK.zip \
  3fd1c60f994daba536902f30b1bebc2235099c8b8a039d8b37d9d27d4827e0ca

# FATE samples (ffmpeg's test suite, fate-suite.ffmpeg.org; test material only, ~31 MB).
mkdir -p "$OUT/fate"
while read -r path sum; do
  name=$(basename "$path")
  get "https://fate-suite.ffmpeg.org/$path" "fate-$name" "$sum"
  cp -n "$OUT/dl/fate-$name" "$OUT/fate/$name"
done <<'EOF'
lossless-audio/NoLegacy-cut.ape 7ca0fd5ef79665e1a5679ec7ffdbf6533a80839f1c5d3b177839a4ef31bf85da
lossless-audio/luckynight-partial.ape ce0769473b9302477c224fb1425194788bf6bab2e2dad3964d2c68a3b9711a07
lossless-audio/luckynight-mac394b1-c2000.ape c17918e48df298a648bc3efa23e948bcd055af8cc2a631af183b10e6a198c1bc
lossless-audio/luckynight-partial.wv 4ee2627d9e217a600a5bd2c79fa0a5f123b8104f8a30f74a3f5bb4fa8c1f1630
musepack/inside-mp7.mpc 0e23d692f39d3c874286ef0b479ec8d1d1586fdcf93b906dee316fc11b694301
musepack/inside-mp7.pcm 1cfeff6e8cdca3588dd7d0d5f4a67a423bcd45316657dde3165166bb22f012d2
musepack/inside-mp8.mpc 62ee2237bd95e7704d9835abb6bcca694e5a29ba8ea76d34cb6ecdac0e8b5fac
musepack/inside-mp8.pcm 4934379be786b798b67bfc8a502db6c61055f469b768b60453005acb1d8a7a78
wavpack/lossless/12bit-partial.wv 8242aa9dab135b308434122af851fb7e3f9a7c2f9c10fd034fb8b1e97caf0e55
wavpack/lossless/16bit-partial.wv 4924f2a2bdf9d24627b40b94f391b6f6cedf78be6ee2782543f0717df7229026
wavpack/lossless/24bit-partial.wv 455bc50b07fa0e7d71994a4af0ff692418df6069abaa56c6e71ffe2ee0ea771b
wavpack/lossless/32bit_float-partial.wv e4496128c556114cb5e9725f40fac937c69e33eefd292f7e21a873960fe280ce
wavpack/lossless/32bit_int-partial.wv 2013fe3f70d6a01ae62a6a73ecea4ec21650f536f9428cee3778256e52ff5bba
wavpack/lossless/8bit-partial.wv 84371ea36998021884511d97e2878c3b2a3a86dac2731b46e23c8be7831872a9
wavpack/lossless/dsd.wv 90b5037f4654dbed6299ff857c58c6d78cfa3f893f51c3b5d3741cd982c2a2c0
wavpack/lossy/2.0_32-bit_float.wv b0803acfc4321987d8718e02825185d34b976843c8348ce2bfffad79222d6694
wavpack/lossy/4.0_16-bit.wv 63a362a501d4e9a26a1de96dec3d34eb94cbf477977925cc1c53c05208c1dd83
wavpack/lossy/4.0_24-bit.wv 4997ffd1f95a934e1c51f444fbabbfc33fbcce4103f37df574fd74b2ee48126a
wavpack/lossy/4.0_32-bit_int.wv 3120665c5aea945e0ef00c7924d3db2df6794969273061d814af52dc1b6fc398
wavpack/lossy/4.0_8-bit.wv 3a59c5f8f44f3bc0520b923e0efb79f80a9b6c1b913a3d5810657e2c5ed1c67b
wavpack/special/clipping.wv 3cc3652da5ec70b2a75c4c8f36564b8431a75419837de4f76c97e00898ae569f
wavpack/special/false_stereo.wv 4b7631bb0bddf2751dec1ab1ca59df37268c8bb0a8ce83f54a9d2c419ff74e65
wavpack/special/zero_lsbs.wv c63135606d9fa93f9e07c22fba556d9acd9ef21e241e592f3449314a1b88013c
wavpack/num_channels/edward_4.0_16bit-partial.wv 25982c911c78ad974dc6c3e4dfb76edb52a2e075e090a9961fccdef490b3e3a1
wavpack/num_channels/eva_2.22_6.1_16bit-partial.wv b33dfcd7bce75d7b6ab8abec8b25c6101854d2e3054305fa30c3b4fd4bd6ded2
wavpack/num_channels/mono_16bit_int.wv b4322ec56021f3f8db77d2640f0e8fdf20136fda3aa4561d1c755595632fcc4f
wavpack/num_channels/mono_float-partial.wv 9f8bdc9149e2711425eb950f653cf1842a9f6c8048d422904c34c0d09e42eee0
wavpack/num_channels/panslab_sample_5.1_16bit-partial.wv df93cc6b1106d209ad9fa7da3147f9b85f11a63003a04daf4954fbbb5d273864
wavpack/num_channels/panslab_sample_7.1_16bit-partial.wv 43f030caf922e5259371a7a62c2b05ac0513e7ae1544b4e83801d9960b33bd69
wavpack/speed_modes/default-partial.wv cdcd2f7a4b0185c88f563e0784168ff2f4c9fd79066edfe3472eab85aa1a9def
wavpack/speed_modes/fast-partial.wv 341150fb32e59a6a45da90abe1c9a9271a55322f87bfd5c84be79b85be89c75f
wavpack/speed_modes/high-partial.wv 93f6f66bd13c93c5677aa534683f0fe429980d40aff4714e1155d68e8d7a824d
wavpack/speed_modes/vhigh-partial.wv a6c8a3238f2763986d0b5262c52d97c900b7e206ce696aeb58605daeb76db4f4
EOF

SRC=$OUT/src
[[ -d $SRC/WavPack-5.8.1 ]] || tar -C "$SRC" -xzf "$OUT/dl/wavpack.tar.gz"
[[ -d $SRC/musepack_src_r475 ]] || tar -C "$SRC" -xzf "$OUT/dl/musepack_src_r475.tar.gz"
[[ -d $SRC/mac ]] || { mkdir -p "$SRC/mac" && unzip -q "$OUT/dl/MAC_1343_SDK.zip" -d "$SRC/mac"; }

BIN=$OUT/bin
if [[ ! -x $BIN/wvunpack ]]; then
  cmake -S "$SRC/WavPack-5.8.1" -B "$OUT/build/wv" -DCMAKE_BUILD_TYPE=Release -DBUILD_TESTING=OFF >/dev/null
  nice make -C "$OUT/build/wv" -j2 >/dev/null
  cp "$OUT/build/wv/wavpack" "$OUT/build/wv/wvunpack" "$BIN/"
fi
if [[ ! -x $BIN/mac ]]; then
  cmake -S "$SRC/mac" -B "$OUT/build/mac" -DCMAKE_BUILD_TYPE=Release >/dev/null
  nice make -C "$OUT/build/mac" -j2 >/dev/null
  cp "$OUT/build/mac/mac" "$BIN/"
fi
if [[ ! -x $BIN/mpcenc ]]; then
  # The tree's CMake files are too old for current CMake: compile directly.
  M=$SRC/musepack_src_r475
  F=(-O2 -w -std=gnu89 -Wno-incompatible-pointer-types -Wno-implicit-function-declaration
     -Wno-int-conversion -Wl,--allow-multiple-definition -I"$M/include")
  gcc "${F[@]}" -I"$M/libmpcdec" -I"$M/libwavformat" -o "$BIN/mpcdec" "$M/mpcdec/mpcdec.c" \
    "$M"/libmpcdec/*.c "$M"/libwavformat/*.c "$M/common/crc32.c" -lm
  gcc "${F[@]}" -I"$M/libmpcenc" -I"$M/libmpcpsy" -I"$M/mpcenc" -o "$BIN/mpcenc" \
    "$M"/mpcenc/*.c "$M"/libmpcenc/*.c "$M"/libmpcpsy/*.c "$M/common/crc32.c" \
    "$M/common/fastmath.c" "$M/common/tags.c" "$M/common/huffman-bcl.c" -lm
fi

if [[ ${SKIP_SAMPLES:-0} != 1 ]]; then
  python3 -I "$ROOT/tools/corpus/lossless-samples.py" "$OUT"
  # DSD WavPack (needs numpy): OUT/samples/dsd, for specular-wavpack's `dsd` test.
  python3 -I "$ROOT/tools/corpus/dsd-samples.py" "$OUT"
fi
echo "fetch-lossless: done in $OUT"
