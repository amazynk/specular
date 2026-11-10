#!/usr/bin/env bash
# Fetch the ITU-T JVT conformance streams for the H.264 tools beyond Main profile that the
# FFmpeg mirror cannot check (FMO, ASO, data partitioning, SP/SI slices: libavcodec implements
# none of them), with the reference decoder's YUV from each zip as the oracle.
#
#   $OUT/zip/<name>.zip       as published (sha256-pinned below)
#   $OUT/jvt/<name>.<ext>     the bitstream
#   $OUT/jvt/<name>.framemd5  per-frame MD5s of the zip's reference YUV
#   $OUT/jvt/manifest.tsv     name, file, width, height, pix_fmt, field_order, frames
#
# OUT defaults to $SPECULAR_H264X_CORPUS, else $CARGO_TARGET_DIR/corpus/h264x. About 105 MB of
# zips; the YUVs are not kept. Then:
#   SPECULAR_H264_CORPUS=$OUT/jvt cargo test -p specular-h264 --test conformance
# Downloads are untrusted data: checked against their size-and-hash pins, unpacked by
# tools/corpus/h264x-refs.py (python -I, base names only), never run. Stops below MIN_FREE_GB.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_H264X_CORPUS:-${CARGO_TARGET_DIR:-$ROOT/target}/corpus/h264x}
BASE=https://www.itu.int/wftp3/av-arch/jvt-site/draft_conformance/AVCv1
MIN_FREE_GB=${MIN_FREE_GB:-8}
for t in curl python3 ffprobe sha256sum; do
  command -v $t >/dev/null || { echo "fetch-h264x: $t not found" >&2; exit 1; }
done
mkdir -p "$OUT/zip" "$OUT/jvt"

# name sha256 — FMO/ASO (Baseline/Extended), data partitioning, SP/SI and slice streams.
PINS=(
  "BA1_FT_C f21e1b0b7b820ac6f078bc5e9aa3284e068d2fdf1192e6c5c42509a070b8bb84"
  "BA3_SVA_C 1c7003ff7b4507204f50e0b279149fd183c0890ee2a12c4c2d008db387299b82"
  "CI1_FT_B b44b82586b4be8232d77e7d0971691004f7aed5ef7d8730b20f81405fbc77d1a"
  "FM1_BT_B 9e8de6bc7e87eef28f603a089438eb50c7af542d3958253cbf2c1f1ef05e816f"
  "FM1_FT_E 6474ac25708195b297d7f284a7531be5f181c5a6e0f1e65f79ee8e2fdae3a9cb"
  "FM2_SVA_C 7e27a8776c70e47e4b4441809a6ec23c8a62e1a06eea364a6407839fc8650d4f"
  "MR1_BT_A a76d4e199e6ffb27292d1a07c516e5adea251e63e3f756de490c153b23d4684d"
  "SL1_SVA_B 60d61046c3aee6b9aa6b3dd4fad4081be9c959affec2bd641808632edffed1b2"
  "SP1_BT_A aa9a1fa4fedad3d0437b8d77caefbf1b3cfb60add57b231dc67ebbded60d9d8a"
  "sp2_bt_b 61190770cec9376fe0ece9067670074570683f4547528d6a5fa6386af08a4b8a"
  "SVA_FM1_E 9db49088a67d960d1363605c40a603d38c87361f3b83e730e6541c5a1db4c10f"
)

check_space() {
  local gb
  gb=$(df -Pk "$OUT" | awk 'NR==2{print int($4/1048576)}')
  [ "$gb" -ge "$MIN_FREE_GB" ] || { echo "fetch-h264x: only ${gb} GB free" >&2; exit 1; }
}

: > "$OUT/jvt/manifest.tsv.part"
for pin in "${PINS[@]}"; do
  read -r name sum <<< "$pin"
  zip="$OUT/zip/$name.zip"
  if [ ! -s "$zip" ] || ! echo "$sum  $zip" | sha256sum -c --status; then
    check_space
    curl -fsSL --retry 3 -A 'specular-corpus-fetch/1 (test corpus)' -o "$zip.part" "$BASE/$name.zip"
    mv "$zip.part" "$zip"
  fi
  echo "$sum  $zip" | sha256sum -c --status || { echo "fetch-h264x: $name.zip hash mismatch" >&2; exit 1; }
  python3 -I "$ROOT/tools/corpus/h264x-refs.py" "$zip" "$OUT/jvt" "$name" >> "$OUT/jvt/manifest.tsv.part"
done
mv "$OUT/jvt/manifest.tsv.part" "$OUT/jvt/manifest.tsv"
echo "fetch-h264x: $(wc -l < "$OUT/jvt/manifest.tsv") streams in $OUT/jvt"
