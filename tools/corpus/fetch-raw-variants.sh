#!/usr/bin/env bash
# Fetch the RAW variant corpus: files of already-supported RAW containers whose sensor data
# uses a less common variant (DNG lossy JPEG / JPEG XL / floating point deflate / VC-5, Canon
# sRAW and mRAW, Sony ARW1 and lossless M/S sizes, Fujifilm SuperCCD, CYGM Nikon Coolpix,
# Nikon High Efficiency, DNG tone curves). All from raw.pixls.us (CC0), pinned by path and SHA-256. About 500 MB.
#
#   tools/corpus/fetch-raw-variants.sh [DEST]
#
# DEST defaults to $RAW_VARIANTS_DIR or tools/corpus/raw-variants (gitignored). The specular-raw
# tests read it through SPECULAR_RAW_VARIANTS (see crates/formats/specular-raw/tests/variants.rs)
# and tools/quality/raw_variants_oracle.py compares the decoded sensor data with rawpy (LibRaw).
#
# Downloads are untrusted data: this script only copies files, never runs anything from them.
# Stops if free space on the destination drops below MIN_FREE_GB (default 6).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
DEST=${1:-${RAW_VARIANTS_DIR:-$ROOT/tools/corpus/raw-variants}}
MIN_FREE_GB=${MIN_FREE_GB:-6}
mkdir -p "$DEST"

die() { echo "fetch-raw-variants: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$DEST" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

# SHA-256 | path under https://raw.pixls.us/data/   # variant, size
FILES=(
  # DNG lossy JPEG (34892), LinearRaw 8-bit tiles, Adobe DNG Converter
  'b22f1e36331f679abb8b13e433b9bbde3988723adf10fee7aa0dda6381016a98|Adobe DNG Converter/Canon EOS 5D Mark III/5G4A9395-compressed-lossy.DNG'  # 2 MB
  '159326856c29073e845c3c5a9ecf98c6474f43ca15798a88ad5e2baecd0664b7|Adobe DNG Converter/Canon EOS 5D Mark III/5G4A9394-compressed-lossy.DNG'  # 6 MB
  # DNG deflate (8), floating point CFA with predictor 34894 (HDRMerge), and an uncompressed float
  '6275226bfa2d86ba479130999d045ea0b8da6be4b011130c9eb91cd6a0eb2233|Canon/EOS 5D Mark III/hdrmerge-bayer-fp16-w-pred-deflate.dng'  # 20 MB
  '94993fefdac13a68a40f0c4f47479256ff53790d8af8ad01ecf6b78d7d2b553f|Canon/EOS 5D Mark III/hdrmerge-bayer-fp32-w-pred-deflate.dng'  # 54 MB
  '9bc7f4afefc1b9787d6c9ac01337cd9ac7250f9cc98aeb58ce854229a4d23e87|Canon/EOS 5D Mark III/hdrmerge-bayer-fp16-uncompressed.dng'  # 47 MB
  # DNG 1.7 JPEG XL (52546): LinearRaw 16-bit in one strip (Samsung) and an Enhanced image in tiles
  '8f0ddc0a672503a840995a6901b16d3650f4c41b8c775e66a553b30222a69864|samsung/SM-S901E/20250305_173647.dng'  # 41 MB
  '090e01f9a7a860b2f2f0ead770b8a9ba3b17ea98dc1997835c6bda2fc0a2ae54|Canon/EOS 7D Mark II/0P9A0410-Enhanced-NR.dng'  # 36 MB
  # DNG ProfileToneCurve
  '6492b7e0ac8e012524c0c83606a104cc9bf3b68f30fa60c7b73ffbe4dc584ab1|Samsung/SM-G973U/20200402_181931.dng'  # 1 MB
  '4c65b8cda205087cfb94d8931811e53e15eb4df538ad67c1b4a3e76c1185b277|Blackmagic/Micro Cinema Camera/CAM1_2000-01-01_1707_C0003_000100.dng'  # 1 MB
  # DNG VC-5 (GoPro GPR, compression 9)
  '191901b64412083146256ccebeacd954783f6c40a4556ef355a067915136159b|GoPro/HERO5 Black/GOPR0020.GPR'  # 4 MB
  'f7d8a5b693bc173a777164dbad48921c8d145d602d5422925a13090404514b66|GoPro/HERO7 Black/GOPR0055.GPR'  # 3 MB
  'bf6f67d0c7096f347b864310f6d58be35237c3d0e715b47a1b30e650e9d785b1|GoPro/HERO9 Black/GOPR0017.GPR'  # 8 MB
  'f3eb9413ed30c92ac6e1834b8228ecf5c9a546c142e8e7ec01146c13035cb532|GoPro/FUSION/GPBK1322.GPR'  # 4 MB
  # Canon CR2 sRAW / mRAW (YCbCr lossless JPEG)
  '3789b1ba5d880613104637c40a06619cd9f3b54b0cf62d2d95b9f9e885edcf6e|Canon/EOS 7D/RAW_CANON_EOS_7D-sraw.CR2'  # 11 MB
  '25c746a58363e910e6046a96cf2915fff45fe2995676eb2c97842d0c156d03a7|Canon/EOS 7D/RAW_CANON_EOS_7D-mraw.CR2'  # 16 MB
  'ba644e7dd2abe74eca260e67f0206ff113bf0f62e710f8130611e964d6be5bf1|Canon/EOS 40D/_MG_0154.CR2'  # 6 MB
  '25d928f9e9a65525ac796bdff13e93b7b36b627568d7a17338e12dc44424078a|Canon/EOS 5D Mark IV/B13A0732.CR2'  # 15 MB
  # Sony ARW1 and lossless compressed M / S sizes
  '4df7981ac4baec9648b4f106751ea4b09a6e27bc70a15f8ef09bec051fec362d|Sony/DSLR-A100/_DSC0258.ARW'  # 10 MB
  'd453005714327addd75bcb99c1c6223173dc92f2a59542d82e6761ef0e0e7571|Sony/ILCE-7M4/ILCE-7M4_DSC06675_FullFrame-LossLess-Compressed-Medium.ARW'  # 31 MB
  'cbbd0930c7d8706dff84c68a2004454266e6fd0d8354f5f76a106b5d776e0223|Sony/ILCE-7M4/ILCE-7M4_DSC06676_FullFrame-LossLess-Compressed-Small.ARW'  # 22 MB
  '006b477808b3edaaa41e58ce082653a36e544c4ddc0bfe1bc8a6ef39d93d8369|SONY/ILCE-1M2/A1_07633_lossless_compressed_small_apsc.ARW'  # 24 MB
  # Fujifilm SuperCCD (rotated 45 degree layouts)
  'aca2b2ebcf90d248b4f822ed78bfeb73e79f1b64164a87dc2051a96726cdfa0c|Fujifilm/FinePix S5Pro/2018_06150003.RAF'  # 24 MB
  'c3686a66f9159043bd27afb0d749d566c2912a0374319e38f68fa6255ea565b4|Fujifilm/FinePix F700/RAW_FUJI_F700.RAF'  # 13 MB
  '07adee4a49ae922c89786c96c2f86ce3c130fa128e18b25d0e498fa881d51b94|Fujifilm/FinePix E900/RAW_FUJI_E900.RAF'  # 18 MB
  'dabd5e74521a6980156be9fd4b88d0c37b0fe4d0e0e6f5c12db8cffff1b76297|Fujifilm/FinePix S5000/2007_0912Image0022.RAF'  # 7 MB
  # CYGM colour filter arrays (Nikon Coolpix NEF)
  '6efeb5049cb4957f0586c47478caed2c5cfdce23eed6b65f5e65ab50ab7c1294|Nikon/E5700/20080115-DSCN1758.NEF'  # 8 MB
  # X-Trans (demosaic quality against LibRaw)
  'f756ad5be551a8b3e1b503a8a6f7e8e637fba519c0b0f8c79dd58e94b0287602|Fujifilm/X-T2/20170114_174341_TFW04727.RAF'  # 22 MB
  # Nikon High Efficiency (HE / HE*, intoPIX TicoRAW): decoded (tools/corpus/fetch-nikon-he.sh has the oracle)
  '82df041542b8d328738443bb5ff3373b396e406ecc8cbe7dc3b35252790c53e7|Nikon/Z 8/Nikon_Z8_high_efficiency_low.NEF'  # 23 MB
  'e113deb305d84153b26251ce259d1f839be1f36b048cbe83a4548cbda806c0f5|Nikon/Z 9/Nikon_-_NIKON_Z_9_-_14bit_compressed_(Lossy_High_Efficiency_Star).NEF'  # 34 MB
)

for entry in "${FILES[@]}"; do
  sha=${entry%%|*}
  rel=${entry#*|}
  out="$DEST/$rel"
  if [[ ! -s $out ]] || ! echo "$sha  $out" | sha256sum -c --quiet - 2>/dev/null; then
    check_space
    mkdir -p "$(dirname "$out")"
    enc=$(python3 -I -c 'import sys,urllib.parse;print(urllib.parse.quote(sys.argv[1]))' "$rel")
    curl -fsSL --retry 3 -A 'specular-corpus-fetch/1 (test corpus)' -o "$out.part" \
      "https://raw.pixls.us/data/$enc"
    mv "$out.part" "$out"
    echo "$sha  $out" | sha256sum -c --quiet - || die "checksum mismatch: $rel"
  fi
done
echo "fetch-raw-variants: ${#FILES[@]} files in $DEST"
