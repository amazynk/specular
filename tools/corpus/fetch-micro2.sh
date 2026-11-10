#!/usr/bin/env bash
# Fetch the micro2 corpus (ICS, FCS, Hamamatsu VMS) into tools/corpus/micro2/ (gitignored:
# nothing here is committed), then write the oracle's expected values beside it with
# tools/corpus/micro2-oracle.py (DIPlib, fcsparser, FlowIO, flowutils, openslide-python):
#   micro2/expected.tsv   read by specular-micro's tests/ics_oracle.rs, specular-fcs's
#                         tests/corpus.rs and specular-wsi's tests/vms.rs
# The oracle also writes synthetic VMS / VMU slides (micro2/vms-synth) and a VMS made of the
# real CMU-1's bottom row (micro2/vms-cut). The tests skip when the corpus is absent.
#
# Usage: tools/corpus/fetch-micro2.sh [SECTION...]
#   SECTION is a function name below (src_ics, src_fcs, src_vms, oracles). Default: all.
#   OUT defaults to $SPECULAR_MICRO2_CORPUS, else tools/corpus/micro2.
#   PY is the oracle interpreter (numpy, diplib, fcsparser, flowio, flowutils, pandas,
#   openslide-python, openslide-bin, pillow), run with -I.
#
# About 155 MB downloaded:
#   ICS  libics' test images (LGPL-2.1 repository: testim, and testim_c with a compress
#        .ids.Z), OME downloads ICS/jan (SVI Huygens result, CC BY 4.0, Jan Eglinger)
#   FCS  fcsparser's test data (MIT): Miltenyi 2.0/3.0/3.1, FACSCalibur HTS, FACSDiva,
#        Fortessa, CyFlow Cube 8, Cytek xP5, Guava Muse (four datasets), duplicate names;
#        FlowIO's test data (BSD-3-Clause): data1, 3FITC_4PE_004, variable integers, G11,
#        B01, the data start / stop offset discrepancy examples
#   VMS  OpenSlide's CMU-1 (CC0-1.0) from OME's mirror: the index, optimisation file,
#        macro, map and the two bottom-row JPEG files (the top row's are 555 MB)
#
# Every download is pinned by size and sha256. Downloads are untrusted data: this script only
# copies files, never runs anything from them, and the oracle runs with python -I. Stops if
# free space on the target filesystem drops below MIN_FREE_GB (default 8).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_MICRO2_CORPUS:-$ROOT/tools/corpus/micro2}
PY=${PY:-python3}
MIN_FREE_GB=${MIN_FREE_GB:-8}
mkdir -p "$OUT"

die() { echo "fetch-micro2: $*" >&2; exit 1; }

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

src_ics() {
  echo "ICS"
  local LIC="libics test images (LGPL-2.1)"
  pinned "https://raw.githubusercontent.com/svi-opensource/libics/master/test/testim.ics" "ics/testim.ics" 505 \
    2eb0a67d7b3288fc5205f6fb2021205587b8bd90d202bd906453b99f8639f9c4 "$LIC"
  pinned "https://raw.githubusercontent.com/svi-opensource/libics/master/test/testim.ids" "ics/testim.ids" 73500 \
    3c6d7aeaa540daf981f00aa54377a7606cbece6aca12f6e308a9f4f7fb817fad "$LIC"
  pinned "https://raw.githubusercontent.com/svi-opensource/libics/master/test/testim_c.ics" "ics/testim_c.ics" 507 \
    4a2f1d5e68a7b637be4067ad5a47a789e4146d8445ff37599da48c32b7c6477f "$LIC"
  pinned "https://raw.githubusercontent.com/svi-opensource/libics/master/test/testim_c.ids.Z" "ics/testim_c.ids.Z" 29823 \
    3d953891d72894e788b555e854d28f65b975d7b6031d648d35a0c037380fbeeb "$LIC"
  LIC="OME downloads, ICS/jan (Jan Eglinger), CC BY 4.0"
  local jan=https://downloads.openmicroscopy.org/images/ICS/jan
  local b=benchmark_v1_2018_x64y64z5c2s1t11_w1Laser4054BD4BP_5c8bc101d6559_hrm
  pinned "$jan/$b.ics" "ics/$b.ics" 4373 \
    e42cbe54f0539633b074e8f73a1e3d2786087944357e3b87d5f006fc65613778 "$LIC"
  pinned "$jan/$b.ids" "ics/$b.ids" 81920 \
    f9e240e3dd05b367832e81e3090b63d3d98a58bae326757913ae718828ed3380 "$LIC"
}

src_fcs() {
  echo "FCS"
  local LIC="fcsparser test data (MIT)"
  local fp=https://raw.githubusercontent.com/eyurtsev/fcsparser/master/fcsparser/tests/data/FlowCytometers
  pinned "$fp/MiltenyiBiotec/FCS2.0/EY_2013-07-19_PBS_FCS_2.0_Custom_Without_Add_Well_A1.001.fcs" "fcs/miltenyi_2.0.fcs" 643582 \
    28c442794bf920c72cb7c84cdca0e972d5cbace49727ddedde9d717c8b049128 "$LIC"
  pinned "$fp/FACSCaliburHTS/Sample_Well_A02.fcs" "fcs/facscalibur_hts_2.0.fcs" 601136 \
    bf3fb0140c63ca583a01ec5d6af46df1d00f2e2cd6151f5533ab13a7d7f34e27 "$LIC"
  pinned "$fp/MiltenyiBiotec/FCS3.0/FCS3.0_Custom_Compatible.fcs" "fcs/miltenyi_3.0.fcs" 641900 \
    9a56d8825ea533356443c3b849bef82c6027ca3b4eea9e52a4b74e2be25219ff "$LIC"
  pinned "$fp/FACS_Diva/facs_diva_test.fcs" "fcs/facs_diva_3.0.fcs" 4007061 \
    8d0a72d1d219c880d9120bac0b7501afa23a08b4ecd2b074803e73eaa411802a "$LIC"
  pinned "$fp/Fortessa/FCS_3.0_Fortessa_PBS_Specimen_001_A1_A01.fcs" "fcs/fortessa_3.0.fcs" 512210 \
    fa9011c86e8ad043ab623656646f329aea907e9655e20f94ade97eea4b9dc177 "$LIC"
  pinned "$fp/cyflow_cube_8/cyflow_cube_8.fcs" "fcs/cyflow_cube_8_3.0.fcs" 58393 \
    08b8ef7fc49d34c56543551cc4c16bbf50c85af49d89b3abbe40d6f6a5d6a8d9 "$LIC"
  pinned "$fp/Cytek_xP5/Cytek_xP5.fcs" "fcs/cytek_xp5_3.0.fcs" 1043904 \
    dd6aaa9a034c40ab5f3a2ce1ea4c8eab7d00a053426917e07a09f8d8d76ca51a "$LIC"
  pinned "$fp/GuavaMuse/Guava%20Muse.fcs" "fcs/guava_muse_3.0.fcs" 8488938 \
    f9b0ca844ecee33c935a6c5e6a7d81c5afe155ce40f0ed663bc51f0d9c98f0e9 "$LIC"
  pinned "$fp/MiltenyiBiotec/FCS3.1/EY_2013-07-19_PBS_FCS_3.1_Well_A1.001.fcs" "fcs/miltenyi_3.1.fcs" 887613 \
    8cfedc08f387a53cd492c98b5150c3a85ad704499aec07190de5bb249897598e "$LIC"
  pinned "$fp/MiltenyiBiotec/FCS3.1/SG_2014-09-26_Duplicate_Names.fcs" "fcs/duplicate_names_3.1.fcs" 294908 \
    7ee7e214e54161ebea46aea860ef6c050dfdb1e093cd1d97684fbfd4bcb87cc4 "$LIC"
  LIC="FlowIO test data (BSD-3-Clause)"
  local fio=https://raw.githubusercontent.com/whitews/FlowIO/master/data/fcs_files
  pinned "$fio/data1.fcs" "fcs/flowio_data1_2.0.fcs" 216432 \
    cba7812f5bbce700786d22d97cf9858c1301878aa840c64eadcd263a3a0a5508 "$LIC"
  pinned "$fio/3FITC_4PE_004.fcs" "fcs/flowio_3FITC_4PE_004_2.0.fcs" 757929 \
    240e76eba7e04c7e9116c8c3ad8388ccb2faf58105d21b831d122764a3621e53 "$LIC"
  pinned "$fio/variable_int_example.fcs" "fcs/flowio_variable_int_3.0.fcs" 6189 \
    88d2933d52686b061f36d8596e0826c9d2e75eb3d654d570f4ad3f1153f9beb2 "$LIC"
  pinned "$fio/G11.fcs" "fcs/flowio_G11_3.1.fcs" 285872 \
    40ed9f6d4293a87077cab07a34c68b2497d4d69988adcb64a5f718098f48b56f "$LIC"
  pinned "$fio/B01%20KC-A-W---91-US.fcs" "fcs/flowio_B01_3.1.fcs" 93402 \
    33736fb7a40a4f571837c231d342df5fd4b20b5dd70439b74b3610f94b6c2a36 "$LIC"
  pinned "$fio/data_start_offset_discrepancy_example.fcs" "fcs/flowio_data_start_discrepancy.fcs" 6263 \
    414fdccae26a65996324261ab028fb1c762e448313ebcf6207894b4dc43eef47 "$LIC"
  pinned "$fio/data_stop_offset_discrepancy_example.fcs" "fcs/flowio_data_stop_discrepancy.fcs" 6263 \
    49956b68a5131af1bba423a26fdaabd9fdf109b51d5f421a42971c09c920cc9d "$LIC"
}

src_vms() {
  echo "VMS"
  local LIC="OpenSlide test data CMU-1 (CC0-1.0), OME downloads mirror"
  local u="https://downloads.openmicroscopy.org/images/Hamamatsu-VMS/openslide/CMU-1/CMU-1-40x%20-%202010-01-12%2013.24.05"
  local n="vms/CMU-1-40x - 2010-01-12 13.24.05"
  pinned "$u.vms" "$n.vms" 711 \
    7f84e0efdc10453441e923b6d805de8c997a692bea860f92af06352b0a6286da "$LIC"
  pinned "$u.opt" "$n.opt" 762840 \
    8c341612c54a512ec5ee3808d691e500cffec8ca8e7e655453aa5a12fe558ad7 "$LIC"
  pinned "${u}_macro.jpg" "${n}_macro.jpg" 44802 \
    1ab418304d04c35a9ee74b9bd22da98df9a1c9c4fa1d18f5ed98ebe5645009ce "$LIC"
  pinned "${u}_map2.jpg" "${n}_map2.jpg" 12448223 \
    afb63ee5509770229f1bf8ce115202766eea3ae40fd0192c503380639be8593d "$LIC"
  pinned "$u%280%2C1%29.jpg" "$n(0,1).jpg" 55761574 \
    45ee406c035bdfb2749224f7a57263a82ce7547af3c739ffa3c7d66ba352afcb "$LIC"
  pinned "$u%281%2C1%29.jpg" "$n(1,1).jpg" 22798062 \
    9a418eaca8afa936e2d833f6877ce353ffaff5bd65b3b143db02ebcf16ce92e4 "$LIC"
}

oracles() {
  echo "oracles"
  "$PY" -I "$ROOT/tools/corpus/micro2-oracle.py" "$OUT"
}

sections=("$@")
if (( ${#sections[@]} == 0 )); then
  sections=(src_ics src_fcs src_vms oracles)
fi
for s in "${sections[@]}"; do
  declare -F "$s" > /dev/null || die "no section $s"
  "$s"
done
echo "done: $(du -sh "$OUT" | cut -f1) in $OUT"
