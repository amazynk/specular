#!/usr/bin/env bash
# Fetch the AAC conformance corpus for specular-aac (gitignored: nothing here is committed) and
# write the oracle decodes beside each stream.
#
# Usage: tools/corpus/fetch-aac.sh
#   OUT defaults to $SPECULAR_AAC_CORPUS, else target/aac-corpus; the LATM samples go to
#   $SPECULAR_AAC_LATM, else OUT/latm.
#   Then: SPECULAR_AAC_CORPUS=<OUT> cargo test -p specular-aac --test conformance
#         SPECULAR_AAC_LATM=<OUT>/latm cargo test -p specular-aac --test latm
#         SPECULAR_AAC_CORPUS=<OUT> cargo test -p specular-video --test aac_oracle
#
# What it covers (about 15 MB), from FFmpeg's FATE suite (fate-suite.ffmpeg.org/aac, the MPEG-4
# conformance bitstreams of ISO/IEC 14496-26 plus a few encoder samples), SHA-256 pinned:
#   AAC-LC     al04_44, al04sf_48 (960-sample frames), al05_44, al06_44, al07_96, al15_44,
#              al17_44, al18_44, al22_chCfg0PCE_44 (layout from a program config element),
#              aac-sce-in-stereo, ct_faac-adts (ADTS framing)
#   AAC Main   am00_88, am05_44
#   AAC-LTP    ap05_48
#   HE-AAC v1  al_sbr_cm_48_2, al_sbr_cm_48_5.1, al_sbr_sr_48_2_fsaac48
#   HE-AAC v2  al_sbr_ps_04_new, al_sbr_ps_06_new
#   coupling   al07_96 (dependent), al15_44 (independent), am05_44
#   refused    er_ad6000np_44_ep0 (AAC-LD)
# Each conformance stream comes with its .s16 reference decode (16-bit, from the conformance
# package; NAME_reorder.s16 in the WAVE channel order for 5.1). If ffmpeg is on PATH, ffmpeg's
# float decoder output is also written as <name>.f32 (interleaved f32le at the stream's output
# rate) for the error/SNR table.
#
# LATM (LOAS in MPEG transport and program streams): FATE's latm_000000001180bc60 and
# latm_stereo_to_51 (its StreamMuxConfig changes from stereo to 5.1), and the first 3 MB of
# two DVB recordings from samples.ffmpeg.org (danish-1, dvb-aac-latm). ffmpeg takes the LOAS
# stream out (NAME.loas) and decodes it (NAME.f32, NAME.ch); without ffmpeg they are skipped.
#
# Downloads are untrusted data: this script only copies files and hands them to ffmpeg.
# Stops if free space on the target filesystem drops below MIN_FREE_GB (default 8).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_AAC_CORPUS:-$ROOT/target/aac-corpus}
MIN_FREE_GB=${MIN_FREE_GB:-8}
BASE=https://fate-suite.ffmpeg.org/aac
mkdir -p "$OUT"
: > "$OUT/.provenance.tsv"

die() { echo "fetch-aac: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

# get NAME SHA256: an empty hash prints the hash instead of checking it (for pinning).
get() {
  local name=$1 sum=$2
  check_space
  if [[ ! -s "$OUT/$name" ]]; then
    curl -fsSL --retry 3 -o "$OUT/$name.part" "$BASE/$name" || { rm -f "$OUT/$name.part"; die "download failed: $BASE/$name"; }
    mv "$OUT/$name.part" "$OUT/$name"
  fi
  local got
  got=$(sha256sum "$OUT/$name" | cut -d' ' -f1)
  if [[ -z $sum ]]; then
    echo "unpinned $name $got"
  elif [[ $got != "$sum" ]]; then
    rm -f "$OUT/$name"
    die "$name: SHA-256 $got, expected $sum"
  fi
  printf '%s\t%s\t%s\n' "$name" "$BASE/$name" "$got" >> "$OUT/.provenance.tsv"
}

# name SHA-256 (an empty hash prints the file's hash, for pinning a new entry)
while read -r name sum; do
  [[ -z $name || $name == \#* ]] && continue
  get "$name" "$sum"
done <<'EOF'
al04_44.mp4 e98388671459a44e499e3ddbfb93ffb9bc027bb4a434dbba9ee9a918600dfebf
al04_44.s16 1c4a9035d5863e0be61aea7fdb3171d2820274e21aac37225cb5dfc5555f3758
al04sf_48.mp4 7dc63a8bbf3818af6e771ff359220ab9e0fe63fb0fad8550f55ccadb2df1a40b
al04sf_48.s16 0c52376b0ae7b9078dc6fb21fdffef9ef703ce098ded4e26dd7bef0a53f8db65
al05_44.mp4 cb322e5b75b3ef3a5e421f030521ed81028dc73d565f5250df3928b86c389de4
al05_44.s16 75810beda6069fa7fdef1c23887d580f570bb3e5d79cbae05b2b4edd47640b3c
al06_44.mp4 c7c36aa972bea86c09bde15360866a60f014523291346857236b8ed7e0f932d1
al06_44.s16 ddcff5bafe096fd4876246fc2543d5fb1b4696d55a2a2ab82b25b6fa78dd763e
al07_96.mp4 90c75d790c78c2eed70bf76d5ffa7ec1d1a04613f8e9ded6daf9f2ca30c7cc0a
al07_96.s16 89d234c5b0dfe73c03d06af0c9b87b0c4079e44bea845d2e5c9cc6e5f2fe1daf
al15_44.mp4 8285a4df9afa6b54ee04f5d79e0f4f6ec9fdc2c327992f079e0ce86663d060ab
al15_44.s16 db005f27054ef6917c4f174991141a045be9090fa7769b9ae5298e10584d57dc
al17_44.mp4 9d26cd8f1066385b06f6e53f6cf2c85e2d3eb72c970e1ae50f0d46d1c1a3bc18
al17_44.s16 f8905842c7b5b6064881d127bb0b4198c7cba1a03863c91818b96f5f854658f2
al18_44.mp4 4730cf09e9b599f5376fa398e9901876b527114e08b4b857d80270375de85e21
al18_44.s16 12fe444859ba35f925aba4edd54ce2ab158e16e66f491b51b732f0fe6e13b731
al22_chCfg0PCE_44.mp4 1f343206a3323817b1d36008b0b82d2ca8a6dceddfe3b398dc0d9a20b159dcdc
aac-sce-in-stereo.mp4 2dc4e9412aa9912011893fd3fd27ff79c643b51decc3d3318f04bbb525825aa4
ct_faac-adts.aac 052f9c7c5359d642fbba887d57fc72fe5745fc75b34cddc7a7adc0a912f9cf9c
al_sbr_cm_48_2.mp4 7e7c0390c950eda21775d1a193e05ee703257bed85602884e7d30a8b3c65be90
al_sbr_hq_cm_48_2.s16 30e39d88a75e0193d1f3a2cf1c4f15e151f96a86d27dd975ac0508a9871af264
al_sbr_cm_48_5.1.mp4 c49bf6c206e015b26f34d109693ac11e2434edebc321462c75b254d27a49ccf5
al_sbr_hq_cm_48_5.1.s16 1831f4cf3ef40434d2a332b7c9b4ec42f76cc8d2c6d388057b3502cd4cfb77b7
al_sbr_sr_48_2_fsaac48.mp4 d0b286c5f9bdb47ee1722ab64775cc507de27744c6a21143ce41fc7410cdbceb
al_sbr_hq_sr_48_2_fsaac48.s16 701c77b384ec8261dea54ef65632e3db7405366f73c70d1bb7827e993240c8a0
al_sbr_ps_04_new.mp4 f5b3271f340162096a153a87e8b67c7e1b69f5c3198a6c6e525daa86ca548bb9
al_sbr_ps_04_ur.s16 3079ef561ce89d44b4288c8889ffc36e53eb46c25790e4fc2ac044bbf1d1743b
al_sbr_ps_06_new.mp4 19d241d01cc3490a133ade50f3dec58d2872ada855cf0d0f954102e774cd9841
al_sbr_ps_06_ur.s16 740ea4371031c220c6df7d86c7cf3a412d20e032da420f918c601a438c57bdb7
am00_88.mp4 83e83c47115f443a8707412c1deb2c04ddd7258c52f38595b934ba1290767d09
am00_88.s16 61862236792b5d4206a9ec6a23a6d115ebd556038892be77e0af13b4b683c774
am05_44.mp4 c2c82c447661b4e8fc1da278fffb19fc822e0fd8e0db912bfe4f7bfa2af280ae
am05_44.s16 2a9a98c8003dcfba023ee48ec0457d92a493ecb8d7d61b040ee7b22f909aca0d
ap05_48.mp4 8dc2c9a9f65c1fb7a6a797a4a9fd3574f4bffc1fa9d0b6e4550e9c530e4e2a76
ap05_48.s16 cd3e84ba521a5dcd83195eef5876df06e40188e6cf4432e708664f07b9835961
er_ad6000np_44_ep0.mp4 dcc229f067ed9c8c63617d4024f18d6259b8aeabf9ebcfd7a55e8c1f27da2f50
am05_44_reorder.s16 9ff327c85d98b867e86c5853add4c036a348159172bdbd0ae80534e76967f115
al06_44_reorder.s16 d34ce905bf224e509f23c8034ff29288fd474fc2ff62b3986b973e617bea03f4
al07_96_reorder.s16 e3e1d8e38a510acd89c7a81e71b19815f68184c26d46f8691ca5bdaf9a4e1738
al15_44_reorder.s16 ce61b0376f7a97cefe9c34c1944d78610c8ffe6c1f13c9f7984e043ed282ca0e
EOF

# LATM samples: NAME SHA-256 URL [byte range]
LATM=${SPECULAR_AAC_LATM:-$OUT/latm}
mkdir -p "$LATM"
while read -r name sum url range; do
  [[ -z $name ]] && continue
  check_space
  if [[ ! -s "$LATM/$name" ]]; then
    curl -fsSL --retry 3 ${range:+-r "$range"} -o "$LATM/$name.part" "$url" \
      || { rm -f "$LATM/$name.part"; die "download failed: $url"; }
    mv "$LATM/$name.part" "$LATM/$name"
  fi
  got=$(sha256sum "$LATM/$name" | cut -d' ' -f1)
  [[ $got == "$sum" ]] || { rm -f "$LATM/$name"; die "$name: SHA-256 $got, expected $sum"; }
  printf '%s\t%s%s\t%s\n' "$name" "$url" "${range:+ bytes $range}" "$got" >> "$OUT/.provenance.tsv"
done <<'EOF'
latm_000000001180bc60.mpg be03349733ec4e5cbe16b1e45a7fc4f687b8dfb1627c6f7e4b92f2509d6c011d https://fate-suite.ffmpeg.org/aac/latm_000000001180bc60.mpg
latm_stereo_to_51.ts 955cf489bcb62045157702986a6bf81d59bc114187252fdc598f9014131a0816 https://fate-suite.ffmpeg.org/aac/latm_stereo_to_51.ts
danish-1.m2t 6550bdb502bdd834f9237ee720193d9c91f41700b8f5939afaeb4ac0d8a4071e https://samples.ffmpeg.org/A-codecs/AAC/LATM/danish-1.m2t 0-3000000
dvb-aac-latm.m2t c509d3318789e0efaa7e77a36a56453624389b00b568fba14e1ccaa79099b5ad https://samples.ffmpeg.org/A-codecs/AAC/dvb-aac-latm.m2t 0-3000000
EOF

if command -v ffmpeg >/dev/null; then
  for f in "$OUT"/*.mp4 "$OUT"/*.aac; do
    [[ -e $f ]] || continue
    ref=${f%.*}.f32
    [[ -s $ref ]] && continue
    ffmpeg -nostdin -v error -i "$f" -map 0:a:0 -c:a pcm_f32le -f f32le "$ref.part" 2>/dev/null \
      && mv "$ref.part" "$ref" || rm -f "$ref.part"
  done
  for f in "$LATM"/*.mpg "$LATM"/*.ts "$LATM"/*.m2t; do
    [[ -e $f ]] || continue
    b=${f%.*}
    [[ -s $b.f32 ]] && continue
    ffmpeg -nostdin -v quiet -y -i "$f" -map 0:a:0 -c copy -f data "$b.loas" || continue
    ffmpeg -nostdin -v quiet -y -i "$f" -map 0:a:0 -c:a pcm_f32le -f f32le "$b.f32.part" \
      && mv "$b.f32.part" "$b.f32" || rm -f "$b.f32.part"
    ffprobe -v quiet -select_streams a:0 -show_entries stream=channels -of csv=p=0 "$f" \
      | head -1 > "$b.ch"
  done
else
  echo "fetch-aac: ffmpeg not found; skipping the float reference decodes" >&2
fi
echo "fetch-aac: corpus in $OUT, LATM samples in $LATM"
