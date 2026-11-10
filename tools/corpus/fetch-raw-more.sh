#!/usr/bin/env bash
# Fetch the camera RAW corpus for the formats beyond the big vendors' current ones (Minolta
# MRW, Canon CRW, Sigma X3F, Hasselblad 3FR/FFF, Phase One IIQ, Leaf MOS, Mamiya MEF, Kodak
# DCR/KDC, Epson ERF, compressed Samsung SRW, Panasonic RW2 raw formats 7 and 8, Leica RWL,
# ARRI ARI, Sony SRF) into tools/corpus/raw-more/ (gitignored: nothing here is committed).
# A few samples per format from raw.pixls.us (CC0 unless the site marks otherwise), each
# pinned by URL and SHA-256 (from the site's filelist.sha256 in October 2026). About 700 MB.
#
# Usage: tools/corpus/fetch-raw-more.sh
#   OUT defaults to $SPECULAR_RAW_MORE_CORPUS, else tools/corpus/raw-more.
# Then, for the comparison with LibRaw (rawpy) of the unpacked sensor values and levels:
#   python3 -m venv $CARGO_TARGET_DIR/venv && $CARGO_TARGET_DIR/venv/bin/pip install rawpy numpy
#   SPECULAR_RAW_MORE_CORPUS=tools/corpus/raw-more SPECULAR_RAWPY=$CARGO_TARGET_DIR/venv/bin/python \
#     cargo test -p specular-raw --test raw_more -- --ignored --nocapture
#
# Downloads are untrusted data: this script only copies them. Stops if free space drops below
# MIN_FREE_GB (default 8).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_RAW_MORE_CORPUS:-$ROOT/tools/corpus/raw-more}
MIN_FREE_GB=${MIN_FREE_GB:-8}
mkdir -p "$OUT"
: > "$OUT/.provenance.tsv"

die() { echo "fetch-raw-more: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

# SHA-256 and raw.pixls.us path.
FILES=(
  # Minolta MRW: packed 12-bit (A2), unpacked 16-bit (7D), the first DiMAGE 7
  '8655e3428461377dd7c6b28ae98a0b23d659314997ed16333985278f99c9a4b4 Minolta/DiMAGE A2/PICT0881.MRW'
  '713819e33e5302ec864d576d57223cfc8dcf46e8d45e0ae744d1fec40290a89f Minolta/Dynax 7D/PICT3410.MRW'
  '4c5d4274bebb548f5f7b3001770903bc4023e7bfe4aeeebbd9d6fc08e81b03bb Minolta/DiMAGE 7/RAW_MINOLTA_DIMAGE7.MRW'
  # Canon CRW (CIFF): EOS 300D and D30, PowerShot G2, Pro70 (the oldest), G6
  '9e7f62239fe728046af28d0d4a8a722d35d40146a79794a5bda1775306af4383 Canon/EOS 300D/CRW_8395.CRW'
  '4ee482f2b6f9f1a8d370703ea04e9f2abaa49356973a7f56b229af93a78e8a2e Canon/EOS D30/CRW_2444.CRW'
  '05bdd52c725dcd1aedb0f642731c7397711bdb91ed596f0bc12e1642b5ece98a Canon/PowerShot G2/RAW_CANON_G2.CRW'
  '3e583286dd5e63b03aca3de6671b7921fc9fd814a7f4cbed50a20c028728c802 Canon/PowerShot Pro70/RAW_CANON_PRO70_SRGB.CRW'
  '244378e1f5660b6cc6cefc93a75ed4928d0b28aa6304ff327b61c748538f4fef Canon/PowerShot G6/CRW_2097.CRW'
  # Sigma X3F: SD10 (old Huffman), SD14 and DP1 (TRUE I), SD1 Merrill (TRUE II), DP0 Quattro
  'ea436ba459a7478d7b338428656fae905469de269f928749f3388db2b7b0d53f Sigma/Sigma SD10/RAW_SIGMA_SD10.X3F'
  'c32f247eb372c3e66ad3b433518ec11f898fdc317c8432ebe025b46d4917bcf3 Sigma/SIGMA SD14/20120818-SDIM4579.X3F'
  '44508f7df4191aacaecb6461e1841dae002e5b71a4f95a00aa3f52d1c6d6bc5f Sigma/DP1/RAW_SIGMA_DP1.X3F'
  '74c0421f53e82b6a37a9ed63c7963f7cf34e49b5a51e70ff0ea7b41f726b2141 Sigma/SIGMA SD1 Merrill/SDIM0042.X3F'
  '9e3095dd20237d35117faaaf1c66e22d0c271ab931f60775b3538519b49f54aa Sigma/DP0 Quattro/_SDI0263.X3F'
  # Hasselblad 3FR and FFF (Hasselblad lossless)
  'f7c0f941bd542ec638073bfa9febdc4efa01f4a4855d73c5d9218b975d1de91a Hasselblad/CFV/RAW_HASSELBLAD_CFV.3FR'
  '209d464e13702de5f4fdb830ac26ac140c98a8d0dc3e81c5ba61d93677330832 Hasselblad/H5D-40/20151114_CCSG_on_Hasselblad_H5D40-0726.fff'
  # Phase One IIQ
  'b0391cb29ebb0773ef53553572df2abffc9093b37d919aeab76d15e72751569d Phase One/P25+/CF028662.IIQ'
  '672a1bf9c2b9b43484f0549ce23a6bf8963efd4338cbc08592a2694c8a463eb6 Phase One/H 25/H25_Outdoor_.IIQ'
  '2dd56f2a7a57a7fc2123301009c1a7a10be002ed84a18470fc4d37e41760a66d Phase One/IQ3 100MP/Phase_One_IQ3_100MP_IIQ_S.IIQ'
  # Leaf MOS, Mamiya MEF
  '8a1a101cb3940b0648af54a721d92e6fb8612dd29a0909036ea2753196a00ec2 Leaf/Aptus 22/L_003172.mos'
  'bcd63507c3c4cc3ea1bad2945e3f88cb4677a1a3c762e1acc38df4445714eb7c Mamiya/ZD/RAW_MAMIYA_ZD.MEF'
  # Kodak DCR and KDC
  '0850ca7337169b609cef369f0de1e71952646d922b50cf1752b7ed6a2ef81ed5 Kodak/DCS Pro 14nx/D7465857.DCR'
  'd0e6bd0a3339daff884e73a08fed8e75b20fe05bba438fc185e3f0bd019d0da8 Kodak/DCS760C/86L57188.DCR'
  '021bfd67147207deb31fd4be19ad23604e5f548581747160a1f5318ae65b475e Kodak/DC120/RAW_KODAK_DC120.KDC'
  '37e290dbd0053f00e508d02a6b3a2a990432dad1eb74c40a52ca899f0f225ecc Kodak/DC50/RAW_KODAK_DC50.KDC'
  '8138d068d14e9ce90c6877b175759fc92358024303b8cd5e9b9eb5ae11e8c4b1 Kodak/P880/100_3710.KDC'
  'e169de008dd708e9c1cc57cf9bde907dee5043caca658f41bdaa127a212f9df3 Kodak/Z1015 IS/RAW_KODAK_EASYSHARE_Z1015-IS.KDC'
  # Epson ERF
  '45bb0081baf1c813c7cd9e05b3e6ae734b90aba408dd1195118dc94dc6abbbb9 Epson/R-D1s/EPSN3575.ERF'
  # Samsung SRW, compressed
  '2087d4b351134e307c1c5e7a4fbf5691998a4c7b20451ba46f8f577d87b38052 Samsung/NX300/nx300.SRW'
  'ec57f83c23094766b723ba9446cdf79388c527934d6689c1ac5b1b6ab3ea8378 Samsung/NX1/2016-07-23-142101_sam_9364.srw'
  '8e1bd19fff8c3b458f63bd6cf0ceda97e708efaf576424cf6c8ebf58eb0c3fc3 Samsung/NX mini/SAM_0295.SRW'
  # Panasonic RW2 raw format 8 (G9 II) and 7 (S5), Leica RWL
  'dd4bc19ed6cf6625608a1ff14ce1a9d16692e1bf1d50993a3fc076e18905b662 Panasonic/DC-G9M2/P1000019.RW2'
  '3615c85207097180b231f0c89d018eb3f1a71988780612921eb0a05246bca706 Panasonic/DC-S5/P1047510.RW2'
  '4f6db6492da83f7f2a5012591e370557249d1ed44d4c948784016605b799327c Leica/D-Lux 7/L1023208.RWL'
  # ARRI ARI, Sony SRF
  '4d487a43d1259049d6f7145049ec33da378681dab1278a8eb3941ac231cc844a Arri/Alexa Mini/A004C002_190722_R20I.478.ari'
  '07739f94bcab0c7e198d418ac65172d0dce04bb84a7683ecffd0bfe4b32f6d79 Sony/DSC-F828/DSC06227.SRF'
)

for entry in "${FILES[@]}"; do
  sha=${entry%% *}
  rel=${entry#* }
  out="$OUT/$rel"
  url="https://raw.pixls.us/data/$(python3 -I -c 'import sys,urllib.parse;print(urllib.parse.quote(sys.argv[1]))' "$rel")"
  mkdir -p "$(dirname "$out")"
  if [[ ! -s $out ]]; then
    check_space
    curl -fsSL --retry 3 -A 'specular-corpus-fetch/1 (test corpus)' -o "$out.part" "$url" ||
      die "download failed: $url"
    mv "$out.part" "$out"
  fi
  echo "$sha  $out" | sha256sum -c --quiet - || die "checksum mismatch: $rel"
  printf '%s\t%s\t%s\n' "$rel" "$url" 'CC0-1.0 (raw.pixls.us)' >> "$OUT/.provenance.tsv"
done
echo "fetch-raw-more: $(du -sh "$OUT" | cut -f1) in $OUT"
