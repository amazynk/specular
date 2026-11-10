#!/usr/bin/env bash
# Fetch the AMR / AMR-WB oracle material for specular-amr (gitignored: nothing here is
# committed) and build the 3GPP reference decoders.
#
# Usage: tools/corpus/fetch-amr.sh
#   OUT defaults to $SPECULAR_AMR_CORPUS, else $CARGO_TARGET_DIR/corpus/amr, else
#   target/corpus/amr.
#   Then: SPECULAR_AMR_CORPUS=<OUT> cargo test -p specular-amr --test vectors -- --ignored
#
# What it fetches (about 120 MB, SHA-256 pinned), all free downloads:
#   - ETSI TS 126 073 / 126 173 V19.0.0: the fixed-point C code of AMR and AMR-WB (part of
#     the specifications). Its tables are what tools/corpus/amr-tables.py transcribes;
#     its decoders are built here (storage-format input) as the oracle for
#     tools/corpus/amr-oracle.py. The code is never committed or linked.
#   - ETSI TS 126 074 / 126 174 V19.0.0: the 3GPP test sequences (codec bit streams and
#     the reference decoder outputs, all modes, DTX, mode switching, homing).
#   - opencore-amr 0.1.6 (Apache-2.0): the second source the table generator checks
#     against.
# Downloads are untrusted data: this script only copies, unzips and compiles them.
# Stops if free space on the target filesystem drops below MIN_FREE_GB (default 8).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_AMR_CORPUS:-${CARGO_TARGET_DIR:-$ROOT/target}/corpus/amr}
MIN_FREE_GB=${MIN_FREE_GB:-8}
mkdir -p "$OUT/dl"

die() { echo "fetch-amr: $*" >&2; exit 1; }

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
  if [[ ! -s "$OUT/dl/$name" ]]; then
    echo "fetch-amr: downloading $name"
    curl -fL --progress-bar --retry 3 -A 'Mozilla/5.0 (specular-corpus-fetch)' -o "$OUT/dl/$name.part" "$url" \
      || { rm -f "$OUT/dl/$name.part"; die "download failed: $url"; }
    mv "$OUT/dl/$name.part" "$OUT/dl/$name"
  fi
  local got
  got=$(sha256sum "$OUT/dl/$name" | cut -d' ' -f1)
  [[ $got == "$sum" ]] || { rm -f "$OUT/dl/$name"; die "$name: SHA-256 $got, expected $sum"; }
}

E=https://www.etsi.org/deliver/etsi_ts
get "$E/126000_126099/126073/19.00.00_60/ts_126073v190000p0.zip" ts_126073v190000p0.zip \
  22de7656c3f573c9f8201afd8092523f25bbd5b5b60ee026e79a5c1bd1512333
get "$E/126100_126199/126173/19.00.00_60/ts_126173v190000p0.zip" ts_126173v190000p0.zip \
  b51c2f49a69c404d9d519eac92995a3c70373e1b8e33e826180024137cf57572
get "$E/126000_126099/126074/19.00.00_60/ts_126074v190000p0.zip" ts_126074v190000p0.zip \
  dbbeae1f73810ba347046abdd2226a021f96640f935d289b73c7cbc80c1b6fab
get "$E/126100_126199/126174/19.00.00_60/ts_126174v190000p0.zip" ts_126174v190000p0.zip \
  a959f7b48024cf2c0ffd896f23a436c8a4993b16af765b75ad53e1cb8262e85c
get https://sourceforge.net/projects/opencore-amr/files/opencore-amr/opencore-amr-0.1.6.tar.gz/download \
  opencore-amr-0.1.6.tar.gz 483eb4061088e2b34b358e47540b5d495a96cd468e361050fae615b1809dc4a1

check_space
rm -rf "$OUT/nbc" "$OUT/wbc" "$OUT/nbv" "$OUT/wbv" "$OUT/oc" "$OUT/refbuild"
mkdir -p "$OUT/nbc" "$OUT/wbc" "$OUT/nbv" "$OUT/wbv" "$OUT/oc" "$OUT/refbuild"
(cd "$OUT/nbc" && unzip -qo ../dl/ts_126073v190000p0.zip && unzip -qo 26073-*_ANSI_C_source_code.zip && rm -f ./*.zip)
(cd "$OUT/wbc" && unzip -qo ../dl/ts_126173v190000p0.zip && unzip -qo 26173-*_ANSI-C_source_code.zip && rm -f ./*.zip)
# AMR sequences: linear PCM only (the A-law/u-law sets and encoder inputs are left out)
(cd "$OUT/nbv" && unzip -qo ../dl/ts_126074v190000p0.zip \
  && for z in TS_AMR_h01_DTX.zip TS_AMR_h01_DTX2.zip TS_AMR_h01_NODTX.zip; do unzip -qo "$z"; done \
  && for z in NODTX_h00.zip DTX_h00.zip DTX2_h00.zip; do unzip -qo "$z"; done \
  && rm -f ./*.zip ./*.doc && find . -name '*.INP' -delete)
(cd "$OUT/wbv" && unzip -qo ../dl/ts_126174v190000p0.zip && unzip -qo T.zip -x '*.inp' && rm -f ./*.zip)
tar -xzf "$OUT/dl/opencore-amr-0.1.6.tar.gz" -C "$OUT/oc"

# the 3GPP reference decoders, storage-format (MMS / MIME) input
cp -r "$OUT/nbc/c-code" "$OUT/refbuild/nb"
cp -r "$OUT/wbc/c-code" "$OUT/refbuild/wb"
for d in nb wb; do
  (cd "$OUT/refbuild/$d" && mv coder.c coder.c.off \
    && gcc -O2 -w -std=gnu89 -D__MSDOS__ -DWMOPS=0 -DVAD1 -DMMS_IO -I. ./*.c -o decoder -lm)
done

if python3 -c 'import ctypes; ctypes.CDLL("libopencore-amrnb.so.0"); ctypes.CDLL("libvo-amrwbenc.so.0")' 2>/dev/null; then
  python3 -I "$ROOT/tools/corpus/amr-oracle.py" "$OUT/fixtures" \
    --ref "$OUT/refbuild/nb/decoder" "$OUT/refbuild/wb/decoder" > /dev/null
  echo "fetch-amr: fixtures and oracle in $OUT/fixtures"
else
  echo "fetch-amr: libopencore-amrnb / libvo-amrwbenc not installed; fixtures not regenerated"
fi
echo "fetch-amr: done ($(du -sh "$OUT" | cut -f1) in $OUT)"
