#!/usr/bin/env bash
# Fetch the microscopy and spectroscopy corpus (3DHISTECH MIRAX, Roche/Ventana BIF, Olympus
# cellSens VSI with its .ets frame files, Olympus FluoView OIR, Princeton Instruments SPE,
# Andor SIF) into tools/corpus/microscopy/ (gitignored: nothing here is committed), then write
# the oracles' expected values beside it:
#   microscopy/expected.tsv        tools/corpus/microscopy-oracle.py (openslide-python 4,
#                                  tifffile, Bio-Formats' bfconvert): read by specular-wsi's
#                                  tests/microscopy.rs
#   microscopy/expected-micro.tsv  crates/formats/specular-micro/gen/spectro-oracle.py
#                                  (spe2py / imageio, sif_parser, Bio-Formats): read by
#                                  specular-micro's tests/microscopy_corpus.rs
# Both tests skip when the corpus is absent.
#
# Usage: tools/corpus/fetch-microscopy.sh [SECTION...]
#   SECTION is a function name below (src_mrxs, src_bif, src_vsi, src_oir, src_spe, src_sif,
#   oracles). Default: all.
#   OUT defaults to $SPECULAR_MICROSCOPY_CORPUS, else tools/corpus/microscopy.
#   PY is the oracle interpreter (openslide-python, openslide-bin, tifffile, imagecodecs,
#   numpy, spe2py, imageio, sif_parser), run with -I. BFTOOLS is Bio-Formats' bftools folder
#   (needs java; downloads.openmicroscopy.org/bio-formats/8.5.0/artifacts/bftools.zip).
#
# About 780 MB downloaded (zips are unpacked and deleted, their hash kept in a marker):
#   MRXS  OpenSlide test data (Mirax/, CC0 and "distributable"): CMU-1 saved at 1/16 and 1/2,
#         Mirax2-Fluorescence-2
#   BIF   OpenSlide test data (Ventana/Ventana-1.bif, CC0, PathAI); the oracle also writes
#         two small synthetic BIF files that OpenSlide reads (joint directions RIGHT/UP)
#   VSI   OME downloads (images/CellSens): evident Image_V4.1_FL_2D (label, overview and two
#         4-channel 16-bit JPEG 2000 images), zenodo-17590655 metadataTest_01 (CC BY 4.0,
#         raw 16-bit C×Z×T)
#   OIR   OME downloads (images/Olympus-OIR): etienne (CC BY 4.0), imagesc-105684,
#         gh-4205/zenodo-12773657 and zenodo-13680725
#   SPE   imageio test_images (v2), spe2py tests (LightField v3 with ROIs and step-and-glue)
#   SIF   sif_parser testings (MIT)
#
# Every download is pinned by size and sha256. Downloads are untrusted data: this script only
# copies files, never runs anything from them, and the oracles run with python -I. Stops if
# free space on the target filesystem drops below MIN_FREE_GB (default 8).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_MICROSCOPY_CORPUS:-$ROOT/tools/corpus/microscopy}
PY=${PY:-python3}
BFTOOLS=${BFTOOLS:-}
MIN_FREE_GB=${MIN_FREE_GB:-8}
OME=https://downloads.openmicroscopy.org/images
OS=https://openslide.cs.cmu.edu/download/openslide-testdata
GH=https://raw.githubusercontent.com
mkdir -p "$OUT"

die() { echo "fetch-microscopy: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

provenance() { printf '%s\t%s\t%s\n' "$1" "$2" "$3" >> "$OUT/.provenance.tsv"; }

# fetch URL DEST SIZE SHA256: download to DEST.part and check the size and hash (a mismatch
# deletes the download and stops).
fetch() {
  local url=$1 dest=$2 size=$3 sha=$4
  check_space
  mkdir -p "$(dirname "$dest")"
  curl -fsSL --retry 3 --max-filesize $((size + 1)) -o "$dest.part" "$url"
  local got_size got
  got_size=$(stat -c %s "$dest.part")
  if [[ $got_size != "$size" ]]; then
    rm -f "$dest.part"
    die "$dest: $got_size bytes, expected $size"
  fi
  got=$(sha256sum "$dest.part" | cut -d' ' -f1)
  if [[ $got != "$sha" ]]; then
    rm -f "$dest.part"
    die "$dest: sha256 $got, expected $sha"
  fi
  mv "$dest.part" "$dest"
}

# pinned URL DEST SIZE SHA256 LICENCE: downloads once.
pinned() {
  local url=$1 dest=$OUT/$2 size=$3 sha=$4 lic=$5
  if [[ -f $dest ]] && [[ $(stat -c %s "$dest") == "$size" ]] \
    && [[ $(sha256sum "$dest" | cut -d' ' -f1) == "$sha" ]]; then
    return
  fi
  fetch "$url" "$dest" "$size" "$sha"
  provenance "$2" "$url" "$lic"
  echo "  $2"
}

# zipped URL DIR NAME SIZE SHA256 LICENCE: downloads NAME.zip once, unpacks it into DIR and
# deletes it, keeping DIR/.NAME.zip.sha256 as the marker.
zipped() {
  local url=$1 dir=$OUT/$2 name=$3 size=$4 sha=$5 lic=$6
  local marker=$dir/.$name.zip.sha256
  if [[ -f $marker ]] && [[ $(cat "$marker") == "$sha" ]]; then
    return
  fi
  fetch "$url" "$dir/$name.zip" "$size" "$sha"
  (cd "$dir" && unzip -q -o "$name.zip")
  rm -f "$dir/$name.zip"
  echo "$sha" > "$marker"
  provenance "$2/$name.zip" "$url" "$lic"
  echo "  $2/$name (unzipped)"
}

src_mrxs() {
  echo "MIRAX"
  zipped "$OS/Mirax/CMU-1-Saved-1_16.zip" mrxs CMU-1-Saved-1_16 3754715 \
    5bf66d612475e30bdc46e0f16e8e88558e02fb1fd3cb83bfd804366462b842bc "OpenSlide test data, CC0-1.0"
  zipped "$OS/Mirax/CMU-1-Saved-1_2.zip" mrxs CMU-1-Saved-1_2 150691097 \
    4c6067e2950855986058648f1241c6c0c73d8a6dd0ec872e391e3d0531159e0c "OpenSlide test data, CC0-1.0"
  zipped "$OS/Mirax/Mirax2-Fluorescence-2.zip" mrxs Mirax2-Fluorescence-2 41439819 \
    0a72728e8ba5243f3f38f90b7f60b114678d6477ace0b3d334deb161c850da32 "OpenSlide test data, distributable (Yves Sucaet)"
}

src_bif() {
  echo "BIF"
  pinned "$OS/Ventana/Ventana-1.bif" bif/Ventana-1.bif 227377284 \
    2eb05cd8f628c36408c3fde09beeeebaaae90181c4f840d5475ec7594bdcbbb6 "OpenSlide test data, CC0-1.0 (PathAI)"
}

src_vsi() {
  echo "VSI"
  local ev=$OME/CellSens/evident lic="OME downloads, CellSens/evident"
  pinned "$ev/Image_V4.1_FL_2D.vsi" vsi/Image_V4.1_FL_2D.vsi 981224 \
    92582ee8a636bd0e55c75820cf5ce13065b098012071bb5c5f2613d0857d0bf9 "$lic"
  pinned "$ev/_Image_V4.1_FL_2D_/stack1/frame_t.ets" vsi/_Image_V4.1_FL_2D_/stack1/frame_t.ets 32583122 \
    17963eb065bc820eaf1d369a71fea85ff1e03608b6e9582f2f6b3993351d8274 "$lic"
  pinned "$ev/_Image_V4.1_FL_2D_/stack10000/frame_t.ets" vsi/_Image_V4.1_FL_2D_/stack10000/frame_t.ets 29477786 \
    70da374b968689a2c7bdacf2ec92757b4c91d23fa310a38d7d6779d1bd890149 "$lic"
  pinned "$ev/_Image_V4.1_FL_2D_/stack10000/blob_21_f.meta" vsi/_Image_V4.1_FL_2D_/stack10000/blob_21_f.meta 11250 \
    81d1c52e87419bf675940c3f5594c1db631cfbffffb7e6e1ca7c0287313a9930 "$lic"
  pinned "$ev/_Image_V4.1_FL_2D_/stack10000/blob_21_f_Frame%230.ets" "vsi/_Image_V4.1_FL_2D_/stack10000/blob_21_f_Frame#0.ets" 2557600 \
    9b3b05d16b514f68a80262c6340d43f2606dffa38a94ccfb8595e3acd4a407fa "$lic"
  pinned "$ev/_Image_V4.1_FL_2D_/stack10002/frame_t_0.ets" vsi/_Image_V4.1_FL_2D_/stack10002/frame_t_0.ets 87799619 \
    a8404d8b9c8408ec291eb5b32895dae2fa420b16b6240e483dad1250aa2937ed "$lic"
  pinned "$ev/_Image_V4.1_FL_2D_/stack10005/frame_t_0.ets" vsi/_Image_V4.1_FL_2D_/stack10005/frame_t_0.ets 86776904 \
    f35d9b7e09688ecd046795a9a673eb1e39cddad9422635463068fc8b12788262 "$lic"
  local zn=$OME/CellSens/zenodo-17590655 zl="Zenodo 17590655 (Aafke Gros), CC BY 4.0"
  pinned "$zn/metadataTest_01.vsi" vsi/metadataTest_01.vsi 109714 \
    abefda78a992220e816b809ce07c4f148c7a1a9e3fc6b41de04f13d212a7eebd "$zl"
  pinned "$zn/_metadataTest_01_/stack1/frame_t_0.ets" vsi/_metadataTest_01_/stack1/frame_t_0.ets 20225812 \
    9cca4926768fc1249612e92f7a986b0a82f2314dabf86393a6666c0b50761206 "$zl"
}

src_oir() {
  echo "OIR"
  pinned "$OME/Olympus-OIR/etienne/amy%20slice%20z%20stack_0001.oir" oir/amy_slice_z_stack_0001.oir 25957525 \
    2c530b25c188baec977147850cf4c71d70da0a9a45d949e44aeeba7d5aa1c94d "OME downloads, Olympus-OIR/etienne, CC BY 4.0"
  pinned "$OME/Olympus-OIR/imagesc-105684/1202-interval_10sec_sequence_frame.oir" oir/1202-interval_10sec_sequence_frame.oir 17517607 \
    3796258be91390c370388fcbccb77cd14f0ed90fc5b53c055aa577c843ce0b1c "OME downloads, Olympus-OIR/imagesc-105684/COPYING"
  pinned "$OME/Olympus-OIR/gh-4205/zenodo-12773657/DAPI_mCherry_4Z_5Lambda-420-630-w10nm-s50nm.oir" oir/DAPI_mCherry_4Z_5Lambda.oir 14595535 \
    73c4160baf79043f301fe470cfa3832aaf17d789f99ab35b9b736d4af2ace67d "OME downloads, Olympus-OIR/gh-4205/COPYING"
  pinned "$OME/Olympus-OIR/gh-4205/zenodo-13680725/Map_A01.oir" oir/Map_A01.oir 1701149 \
    a5d5cdfb8401da9d47586150ca7edab8f61b70aa50e4d36f148bc8cdbf703188 "OME downloads, Olympus-OIR/gh-4205/COPYING"
}

src_spe() {
  echo "SPE"
  local io=$GH/imageio/test_images/master sp=$GH/ashirsch/spe2py/master/tests/test_files
  pinned "$io/test_000_.SPE" spe/test_000_.SPE 8196 \
    ff84f294c301b43ef5fcdd8dc37d5de57460fc6f6f023e87de0bdc5654f4e93e "imageio test_images"
  pinned "$io/test_v0501_000_.SPE" spe/test_v0501_000_.SPE 6500 \
    0a7a6dedc694e1a7561293f9a0e8f92c45fc0df352b04dae95415d2123445d9e "imageio test_images"
  pinned "$sp/full_sensor_image.spe" spe/full_sensor_image.spe 2137722 \
    baa727352edab985e74b7687f9ac74ac6e75a76e918ae07b5be58c51a61ca4b9 "spe2py tests"
  pinned "$sp/one_dimensional_spectrum.spe" spe/one_dimensional_spectrum.spe 44175 \
    867e3fdbf27c9be2a79b5ea6a2d1dc2f7c1151440e0439ad08b5c2bb965da7b8 "spe2py tests"
  pinned "$sp/small_roi.spe" spe/small_roi.spe 940126 \
    7e5a89a12a5af219a288f1346a4f71a1b2a69f4323fc5fb9b5ef396e551c6588 "spe2py tests"
  pinned "$sp/step_and_glue.spe" spe/step_and_glue.spe 6465263 \
    664badb0c2ba06456d4c427a75787d026c6245adf34a304ba2bba377adfb252b "spe2py tests"
  pinned "$sp/ten_frames_two_rois_different_binning.spe" spe/ten_frames_two_rois_different_binning.spe 2492473 \
    75c0562350706f3169544daf296afa8045c6d90d069f192f0dbde660edae595b "spe2py tests"
  pinned "$sp/two_rectangular_rois_different_binning.spe" spe/two_rectangular_rois_different_binning.spe 949299 \
    b05ffcaafda7e8153137bc83cfb9f4503f60854ac82c8f2f0fbd4ec8e8c068b1 "spe2py tests"
  pinned "$sp/two_rectangular_rois_noBinning.spe" spe/two_rectangular_rois_noBinning.spe 1010911 \
    070fd438aeff0296d935c4cf9e1334c4038f822460a0211cc7ce18c96093d617 "spe2py tests"
}

src_sif() {
  echo "SIF"
  local t=$GH/fujiisoup/sif_parser/master/testings lic="sif_parser testings (MIT)"
  pinned "$t/public_testdata/image.sif" sif/image.sif 2102426 \
    9ec78ef1ba44d57963ec22840318410740f0ce47701d28732d775a72f0cdffa3 "$lic"
  pinned "$t/examples_with_calibration/raman1.sif" sif/raman1.sif 8390 \
    b7899121e24b2307fd9a1bf2c4e46976823ea00313cab5f5a1c71dcb7ff22d35 "$lic"
  pinned "$t/examples_with_calibration/nd_lum_image_no_glue.sif" sif/nd_lum_image_no_glue.sif 8700159 \
    adf94206d113bd457649f177b0b73cec8644c90f82401e436678e28cefaf7334 "$lic"
  pinned "$t/raman_data/DD58_785_1_Fe2O3_5x10s.sif" sif/DD58_785_1_Fe2O3_5x10s.sif 8605 \
    91d440d2c5ad4b3db7f8d213dc65c2aa9954de80689be37eb408131ac757cdb7 "$lic"
  pinned "$t/raman_data/rubpy3%20in%20ethanolpulsed.sif" sif/rubpy3_in_ethanolpulsed.sif 8639 \
    a98b95fa5909aeb97808eaa3b6c1c631a572417f1eb20e8e70592973a377c14b "$lic"
  pinned "$t/issue27/test.sif" sif/issue27.sif 264898 \
    cec885387ff2df969a8e71c24cb1400042d79fc80b5ff1b5b491dcb7a96c9567 "$lic"
  pinned "$t/issue33/measurement.sif" sif/issue33.sif 88340 \
    306c02d2f31b6017fa95b8ce451ce2c7c2726635484b5cda364f513369938e93 "$lic"
  pinned "$t/issue37/shot25-Ar-delay%20480us-width%202us-gain%203900.sif" sif/issue37_gain3900.sif 4198476 \
    f7eb14468a45b5b1a7d92a1d5237bcf2cfad341d2a65b7880423092f916dbbda "$lic"
  pinned "$t/echelle/boron_0.05_1us_750ns_5.sif" sif/echelle_boron.sif 94326 \
    5d3baf659801eb8cbce4adf18ebc1ffbe4bbb04f692e6e72fe5acf38f1338b87 "$lic"
  pinned "$t/step_and_glue/step_and_glue.sif" sif/step_and_glue.sif 46752 \
    43573815c44c9cc81047e730f617ff1fcc504da73bf1c1301db47f2ff3da606d "$lic"
}

oracles() {
  echo "oracles"
  BFTOOLS=$BFTOOLS "$PY" -I "$ROOT/tools/corpus/microscopy-oracle.py" "$OUT"
  BFTOOLS=$BFTOOLS "$PY" -I "$ROOT/crates/formats/specular-micro/gen/spectro-oracle.py" "$OUT"
}

sections=("$@")
if (( ${#sections[@]} == 0 )); then
  sections=(src_mrxs src_bif src_vsi src_oir src_spe src_sif oracles)
fi
for s in "${sections[@]}"; do
  declare -F "$s" > /dev/null || die "no section $s"
  "$s"
done
echo "done: $(du -sh "$OUT" | cut -f1) in $OUT"
