#!/usr/bin/env bash
# Fetch the vendor microscopy corpus (Leica LIF/LOF, Nikon ND2) and public whole-slide
# annotation sidecars (CAMELYON16/17, ASAP XML) into tools/corpus/vendormicro/ (gitignored:
# nothing here is committed), then write the oracles' expected values beside them:
#   vendormicro/expected/*.json   crates/formats/specular-micro/gen/oracle.py (nd2, readlif,
#                                 liffile): read by specular-micro's corpus_matches_oracles
#   vendormicro/annot/expected.tsv tools/corpus/annot-oracle.py (xml.etree, shapely): read by
#                                 specular-wsi's tests/annot.rs corpus_matches_the_oracle
# Both tests skip when the corpus is absent.
#
# Usage: tools/corpus/fetch-vendormicro.sh [SECTION...]
#   SECTION is a function name below (src_nd2, src_lif, src_annot, oracles). Default: all.
#   OUT defaults to $SPECULAR_VENDORMICRO_CORPUS, else tools/corpus/vendormicro.
#   PY is the oracle interpreter (needs nd2, readlif, liffile, numpy, shapely), run with -I.
#
# About 265 MB downloaded:
#   ND2  OME downloads (images/ND2; see each folder's COPYING): a v3 brightfield (maxime),
#        2011 v3 fluorescence (aryeh), 2009 header test (jonas), 2006 and 2007 JPEG 2000 v1/v2
#        files (aryeh)
#   LIF  OME downloads (images/Leica-LIF): mixed scan types (michael), a FRET series
#        (imagesc-30856), a Zenodo 6606445 project
#   LOF  OME downloads (images/Leica-XLEF): many channels, xyct 16-bit 2 channels 11 frames
#   annot/ CAMELYON16 and CAMELYON17 lesion annotations (ASAP XML; CC0, camelyon-dataset
#        on AWS Open Data). No public Aperio ImageScope or NDP.view sidecars with a stable URL
#        were found; those formats are covered by the synthetic fixtures.
#
# Every download is pinned by size and sha256. Downloads are untrusted data: this script only
# copies files, never runs anything from them, and the oracles run with python -I. Stops if
# free space on the target filesystem drops below MIN_FREE_GB (default 8).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_VENDORMICRO_CORPUS:-$ROOT/tools/corpus/vendormicro}
PY=${PY:-python3}
MIN_FREE_GB=${MIN_FREE_GB:-8}
OME=https://downloads.openmicroscopy.org/images
CAM=https://camelyon-dataset.s3.amazonaws.com
mkdir -p "$OUT"

die() { echo "fetch-vendormicro: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

provenance() { printf '%s\t%s\t%s\n' "$1" "$2" "$3" >> "$OUT/.provenance.tsv"; }

# pinned URL DEST SIZE SHA256 LICENCE: downloads once and checks the size and hash (a
# mismatch deletes the download and stops).
pinned() {
  local url=$1 dest=$OUT/$2 size=$3 sha=$4 lic=$5
  if [[ -f $dest ]] && [[ $(stat -c %s "$dest") == "$size" ]] \
    && [[ $(sha256sum "$dest" | cut -d' ' -f1) == "$sha" ]]; then
    return
  fi
  check_space
  mkdir -p "$(dirname "$dest")"
  curl -fsSL --retry 3 --max-filesize $((size + 1)) -o "$dest.part" "$url"
  local got_size got
  got_size=$(stat -c %s "$dest.part")
  if [[ $got_size != "$size" ]]; then
    rm -f "$dest.part"
    die "$2: $got_size bytes, expected $size"
  fi
  got=$(sha256sum "$dest.part" | cut -d' ' -f1)
  if [[ $got != "$sha" ]]; then
    rm -f "$dest.part"
    die "$2: sha256 $got, expected $sha"
  fi
  mv "$dest.part" "$dest"
  provenance "$2" "$url" "$lic"
  echo "  $2"
}

src_nd2() {
  echo "ND2"
  pinned "$OME/ND2/maxime/BF007.nd2" BF007.nd2 270336 \
    e3f449796ac2ce82d5734a607d8899a3ec5a9a575161a3eded2e22eedc1dce92 "OME downloads, ND2/maxime/COPYING"
  pinned "$OME/ND2/aryeh/MeOh_high_fluo_003.nd2" MeOh_high_fluo_003.nd2 13131776 \
    8e23bb594cd18314f9c18e70d736088ae46f8bc696ab7dc047784be416d7a706 "OME downloads, ND2/aryeh/COPYING"
  pinned "$OME/ND2/jonas/header_test2.nd2" header_test2.nd2 14700544 \
    48a6302fa0667b68fea455a880d4ccc8fd589fcbdfe771114af2d0034b86ad05 "OME downloads, ND2/jonas/COPYING"
  pinned "$OME/ND2/aryeh/b16_14_12.nd2" b16_14_12.nd2 24671709 \
    0ec63250700580fa2777d342aa85c0175259e127580ecc760c39e2fd78f0634b "OME downloads, ND2/aryeh/COPYING"
  pinned "$OME/ND2/aryeh/but3_cont200-1.nd2" but3_cont200-1.nd2 16137019 \
    7e8dcd14c990aae7fe3d3a59be6b7db3f0e45e648948e47d5765ffe9f69003ce "OME downloads, ND2/aryeh/COPYING"
}

src_lif() {
  echo "LIF / LOF"
  pinned "$OME/Leica-LIF/michael/PR2729_frameOrderCombinedScanTypes.lif" PR2729_frameOrderCombinedScanTypes.lif 227429 \
    17994bb1bdf93dd1a34a19ed53e5a3d7f672470a8057e2998e25d2474437a052 "OME downloads, Leica-LIF/michael/COPYING"
  pinned "$OME/Leica-LIF/imagesc-30856/20191025%20Test%20FRET%20585.%20423,%20426.lif" fret_585_423_426.lif 49313199 \
    8d4ee62868b9616b832c2eb28e7d62ec050fb032e0bc11ea0a392f5c84390c71 "OME downloads, Leica-LIF/imagesc-30856/COPYING"
  pinned "$OME/Leica-LIF/zenodo-6606445/Project007.lif" Project007.lif 111643445 \
    a4bc49b2fb9f1adf3a66872d01e1450db1d898fc7f4d2386aefa28836c2ec419 "Zenodo 6606445 via OME downloads, see COPYING"
  pinned "$OME/Leica-XLEF/rgb%20channel%20test/XLEF-LOF%20rgb%20channel%20test/many%20channels.lof" many_channels.lof 23090667 \
    5b3a958557a4418da05ba00eda660448626b10bae172a3bb98aea9151266019b "OME downloads, Leica-XLEF"
  pinned "$OME/Leica-XLEF/xyct%2016bit%202ch%20t11/XLEF-LOF%20xyct%2016bit%202ch%20t11/xyct%2016bit%202ch%20t11.lof" xyct_16bit_2ch_t11.lof 11673831 \
    3a8bc142ef49f3349a42748918970b269a31bd8bc1b1951525a71238a396e4de "OME downloads, Leica-XLEF"
}

src_annot() {
  echo "annotations"
  local lic="CAMELYON16/17, CC0 (AWS Open Data camelyon-dataset)"
  pinned "$CAM/CAMELYON16/annotations/tumor_001.xml" annot/tumor_001.xml 142178 \
    ae16fb66f57ed8a1487179c9dcce022c4666a425635617292cadab97f81d7a8c "$lic"
  pinned "$CAM/CAMELYON16/annotations/tumor_016.xml" annot/tumor_016.xml 769363 \
    78ec60137df53987dea4804b51b649d31da6a5371e94864f343d714a0fea5868 "$lic"
  pinned "$CAM/CAMELYON16/annotations/test_001.xml" annot/test_001.xml 281405 \
    b487308cb057d064891d082a3f088aa0a7cc260a95e9879c141bf8244ff29e67 "$lic"
  pinned "$CAM/CAMELYON17/annotations/patient_004_node_4.xml" annot/patient_004_node_4.xml 2647 \
    f8d9e9f7a3cf952c085a5076b1be0bd72d0db8f16550fe45f9e2d11e2a5da716 "$lic"
  pinned "$CAM/CAMELYON17/annotations/patient_009_node_1.xml" annot/patient_009_node_1.xml 28584 \
    65f6ddcd6bf87a650dca9c404d4585dc7f7461a6211be2d3f1bae334ca766b64 "$lic"
  pinned "$CAM/CAMELYON17/annotations/patient_010_node_4.xml" annot/patient_010_node_4.xml 61142 \
    0a0f0cbc79ff0cfd57c0fd4f1654eb9612cc2f984fcf3ddde0fbf29149170e74 "$lic"
}

oracles() {
  echo "oracles"
  "$PY" -I "$ROOT/crates/formats/specular-micro/gen/oracle.py" "$OUT" "$OUT/expected"
  local files=("$OUT"/annot/*.xml)
  if [[ -e ${files[0]} ]]; then
    "$PY" -I "$ROOT/tools/corpus/annot-oracle.py" "$OUT/annot/expected.tsv" "${files[@]}"
  fi
}

sections=("$@")
if (( ${#sections[@]} == 0 )); then
  sections=(src_nd2 src_lif src_annot oracles)
fi
for s in "${sections[@]}"; do
  declare -F "$s" > /dev/null || die "no section $s"
  "$s"
done
echo "done: $(du -sh "$OUT" | cut -f1) in $OUT"
