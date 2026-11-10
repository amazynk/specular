#!/usr/bin/env bash
# Oracle samples: fetch public sample files for the crates that had no independent reference
# decoder, and write each oracle's reading of them beside them.
#
#   tools/corpus/fetch-oracle-samples.sh [SECTION...]     (default: all sections)
#
# Output: ${SPECULAR_ORACLE_SAMPLES_CORPUS:-$CARGO_TARGET_DIR/corpus/oracle-samples}/SECTION/{files,expect}
# and SECTION/expect.tsv (never committed). Each crate's tests/oracle.rs reads its folder;
# tools/ci/oracle-pass-rates.sh runs them. The oracles are Python packages in a venv under
# CARGO_TARGET_DIR (created here) plus ImageMagick and FFmpeg for the raster variants; see
# tools/corpus/oracle-samples.py for which reader serves which section.
#
# Every download is pinned by sha256 (a mismatch stops). Downloads are untrusted data: they
# are only copied, never run, and the oracles run with python -I.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
TARGET=${CARGO_TARGET_DIR:-$ROOT/target}
OUT=${SPECULAR_ORACLE_SAMPLES_CORPUS:-$TARGET/corpus/oracle-samples}
VENV=${VENV:-$TARGET/venv}
PY=$VENV/bin/python
MIN_FREE_GB=${MIN_FREE_GB:-8}

die() { echo "fetch-oracle-samples: $*" >&2; exit 1; }

venv() {
  [[ -x $PY ]] || python3 -m venv "$VENV"
  "$PY" -I -c 'import numpy, PIL, OpenImageIO, xisf, nptdms, flowio, olefile, pdr, imagecodecs
import pds4_tools, vicar, skimage' 2>/dev/null && return
  "$VENV/bin/pip" install -q numpy pillow OpenImageIO xisf npTDMS flowio olefile pdr pds4_tools \
    rms-vicar imagecodecs scikit-image
}

# get SECTION NAME URL SHA256: download into SECTION/files/NAME unless present with that hash.
get() {
  local sec=$1 name=$2 url=$3 want=$4 dest="$OUT/$1/files/$2"
  mkdir -p "$(dirname "$dest")"
  if [[ -f $dest ]] && [[ $(sha256sum < "$dest" | cut -c1-64) == "$want" ]]; then return; fi
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  (( avail_kb >= MIN_FREE_GB * 1024 * 1024 )) || die "less than $MIN_FREE_GB GB free; stopping"
  curl -fsSL --retry 3 --max-filesize 50000000 -o "$dest.part" "$url"
  local got
  got=$(sha256sum < "$dest.part" | cut -c1-64)
  if [[ $want == - ]]; then
    echo "$sec $name $got"
  elif [[ $got != "$want" ]]; then
    rm -f "$dest.part"
    die "$sec/$name: sha256 $got, expected $want"
  fi
  mv "$dest.part" "$dest"
}

oracle() { rm -rf "$OUT/$1/expect"; "$PY" -I "$ROOT/tools/corpus/oracle-samples.py" "$1" "$OUT/$1"; }

GH=https://raw.githubusercontent.com

# Radiance RGBE and PFM, against OpenImageIO. three.js example textures (MIT; HDRIs from Poly
# Haven, CC0), OpenImageIO-images PFM (BSD-3-Clause), Pillow's test PFMs (MIT-CMU).
radiance() {
  local t=$GH/mrdoob/three.js/r170/examples/textures
  get radiance memorial.hdr $t/memorial.hdr 18376b9219927b363127817900b1c170a0f752f52a63f5949655b4150e35061d
  get radiance spot1Lux.hdr $t/equirectangular/spot1Lux.hdr 093c8f84fd07b4f0a496b39d67aaaf8852ea195eeeb05255ad2990f606a7fd9e
  get radiance pisa-px.hdr $t/cube/pisaHDR/px.hdr 72f6b4f08e657dbf47483367a92ac1b299979fa96906591f6a5260229ef128db
  get radiance venice_sunset_1k.hdr $t/equirectangular/venice_sunset_1k.hdr 0e72ed46b5316cb5fb67fc81ff85b024a09146fd89ef3811a8d2299647ada118
  get radiance moonless_golf_1k.hdr $t/equirectangular/moonless_golf_1k.hdr 4f597078024bd81429431e872d466d8808653ad62a8bc8c61d8052af7466c3aa
  get radiance oiio-test-1.pfm $GH/AcademySoftwareFoundation/OpenImageIO-images/main/pnm/test-1.pfm b2c6cfcf843c0cf9163e80de66f8f8cbda95bc9a8a64cb60d2e55714d15e961e
  get radiance oiio-test-2.pfm $GH/AcademySoftwareFoundation/OpenImageIO-images/main/pnm/test-2.pfm 80f0f881fb2a0e5ea1d1a373bd8e6e86568c7636bd25542b143eab219cde1582
  get radiance oiio-test-3.pfm $GH/AcademySoftwareFoundation/OpenImageIO-images/main/pnm/test-3.pfm 86b06b5a51f0bbea4e36149520eec7674aa07bffbfd5b7e1f0bfb4c829cec575
  get radiance hopper.pfm $GH/python-pillow/Pillow/11.0.0/Tests/images/hopper.pfm 11395479d5209ffe1dd334595f1ad74f50f01eddbeb510c9e2679e42b988e444
  get radiance hopper_be.pfm $GH/python-pillow/Pillow/11.0.0/Tests/images/hopper_be.pfm c456a354ed836485c56be4054f1b8271a1e942d7cb05695c103fcc6e31ed443e
  oracle radiance
}

# ICNS, against Pillow's per-entry readers; ICN#, icm# and icsb against rust-icns's expected
# PNGs (MIT). Pillow's test icons (MIT-CMU), VS Code's and Electron's app icons (MIT).
icns() {
  local p=$GH/python-pillow/Pillow/11.0.0/Tests/images r=$GH/mdsteele/rust-icns/master/tests
  get icns pillow.icns $p/pillow.icns 86b320323d6064ff15606637229236eddd4f547b5a1e854494e2a6ba83e6914c
  get icns pillow2.icns $p/pillow2.icns 0da8b3b6f0dd348d07ad874a92dab3aa1399455cbcb2260cf9d6c9eb1ca84ce2
  get icns pillow3.icns $p/pillow3.icns 45c5fa821c43278dd9f3e711abc7f3de80fed6f45946efe0643f55f0c562a4e0
  get icns ic07.icns $r/icns/ic07.icns 5981a148fea4d1d2304d11264c9e73c4214fa1816891406aa4d50b278b3345e5
  get icns ic11.icns $r/icns/ic11.icns 5dc6e5309cb6496ca8d4dd14eb93169dec40ee63a458edf6241889d356cde64f
  get icns ic12.icns $r/icns/ic12.icns f3d92912f741f2342ee902846146c558f36042dec60809ea106fce58a3ab6ba1
  get icns icp4.icns $r/icns/icp4.icns 017e5a6c50ed77feebd8bac5918d3702790a445fd1276f27f676ed4e13a76c7b
  get icns icp5.icns $r/icns/icp5.icns 638a60d6b0e2bf358ecfc3b34eef1cd2423074455be3a2e02039575cc18a07d2
  get icns il32.icns $r/icns/il32.icns 5572b13999fc1175f292a75fadc5accb46e7f87928ff51744904072b15570b3c
  get icns is32.icns $r/icns/is32.icns dd75df44bd87596b8764b692224755357023f76844de0ac7489a7895940e87a8
  get icns it32.icns $r/icns/it32.icns bc18e3fb7119ef82dea74e19a8515ca208701b76b90d9edeaac341b4e82a0b50
  get icns icsb.icns $r/icns/icsb.icns dcd1640728955c3dcf865e18cc44be561a16772bf42eff678568ba88342fd040
  get icns icm#.icns $r/icns/icm%23.icns 1e56bd9791853b55330c2112deda887c269744dbf7040dd5cd547f68e0978030
  get icns icon.icns $r/icns/icon.icns c832565cb4526b44c7cf014b3617539c37c64531a008b5c65089329fb2230faf
  get icns icm#.png $r/png/icm%23.png 144f827a6cac339a7f0a0e1d40dd50b5165d131d22b0ebeb3d057b7fa2927f0b
  get icns icon.png $r/png/icon.png c09eb83de90d6f45f159da25ae7056e86173eec844cb76453ca6f2a5f701eb33
  get icns 18x18.png $r/png/18x18.png 8e40c7f3273a8a8427ea940c7315df1bbc67161a82432e8e5ad772d1c1b2e2b9
  get icns vscode-code.icns $GH/microsoft/vscode/1.95.0/resources/darwin/code.icns 9a8e62dcb766f84eef6debdb2d59d1222a1f102c18f8ffc9de157f1cdc509926
  get icns vscode-cpp.icns $GH/microsoft/vscode/1.95.0/resources/darwin/cpp.icns f96abe22e1d3432c70eef10cc79f93fd5eb6bb61312c79e1d10933b8d9c59042
  get icns electron.icns $GH/electron/electron/v33.0.0/shell/browser/resources/mac/electron.icns 5a9a78d54c157f55672afea37037464858a87fd5f276fc8206787f366ed684cf
  oracle icns
}

# XISF, against the xisf package. libXISF's test files (GPL-3.0 project; the files are only
# test data, read here) and xisf-rs's (MIT).
xisf() {
  local n=https://gitea.nouspiro.space/nou/libXISF/raw/branch/master/test
  get xisf libxisf-test.xisf $n/test.xisf 53e47dfcd2fdbd0a2f6144a80823335bed43670972c5bd30f626266b425c76e6
  get xisf libxisf-test_lz4.xisf $n/test_lz4.xisf 2886e14b4f2c3461d5b310806e388ee58b2f1981581228a09a42a166d832e0b4
  get xisf xisf-rs-2ch.xisf $GH/wrenby/xisf/HEAD/tests/files/2ch.xisf cacfe579ac10cc6793e49cc1db6af482bdcc36106e9c6c655abbeada0b2ad79b
  oracle xisf
}

# TDMS, against npTDMS (the JSON dump of specular-tdms's gen/make_samples.py). Files written by
# LabVIEW and NI DIAdem: ConvertTDMS's examples (BSD-2-Clause), the tdms Rust crate's (MIT),
# the Ruby tdms gem's fixtures (MIT).
tdms() {
  local m=$GH/mnaberez/tdms/HEAD/test/fixtures
  get tdms convert-simple_test.tdms $GH/humphreysb/ConvertTDMS/HEAD/exampleFiles/simple_test.tdms 3bc50b92d44771e16833f0829bec08642894492d91e9426a1e5c3434429cbd94
  get tdms convert-Raw000.tdms $GH/humphreysb/ConvertTDMS/HEAD/Raw000.tdms 4d9edca27d27c2322d8e0f1804b0318490fe84ae165bca3d1babf348bb5aa8fc
  get tdms dnoberon-big_endian.tdms $GH/DnOberon/tdms/HEAD/data/big_endian.tdms 051833770eaa2f3a0cb4dce4c3a05f8559ffeda65b24d2d9b4ef7bef8f78ce6b
  get tdms dnoberon-raw.tdms $GH/DnOberon/tdms/HEAD/data/raw.tdms f4e33424abebe7863c369a936795924001a6f0e5886e5c26604cc9f52c1ea684
  get tdms mnaberez-example.tdms $m/example.tdms 2383617b1f61ea67d61de41ff92d336f02f43a7e5e55fdc29aa0b8b05265ee3c
  get tdms mnaberez-type_01_int8_one_segment.tdms $m/type_01_int8_one_segment.tdms bea17f6d19b1c6efd8391cef699f3975a0d9320e592eada7810e08f8621a1bfc
  get tdms mnaberez-type_01_int8_three_segments.tdms $m/type_01_int8_three_segments.tdms 37ac59200101921fa99f3a03ec6c0b848fa940a46cd54e67d76175cb91c9f24e
  get tdms mnaberez-type_01_int8_two_channels_one_segment.tdms $m/type_01_int8_two_channels_one_segment.tdms fb3d218e738e6367c2fea2296a21ef5c27447b8972d1cd773b10217465c2b32c
  get tdms mnaberez-type_02_int16_one_segment.tdms $m/type_02_int16_one_segment.tdms 7b5cd673064e5139a2fe65c33902d022b85a8bf40be7ee83bb1317867d707e36
  get tdms mnaberez-type_02_int16_three_segments.tdms $m/type_02_int16_three_segments.tdms cf0f334cbbc84b751e8b938ff2c5331507ba1d6a63807a2964e7c1071aa51287
  get tdms mnaberez-type_02_int16_two_channels_one_segment.tdms $m/type_02_int16_two_channels_one_segment.tdms 9807592f30f685b8ac92f9af6e5c0434b8c649fcfebbe465bc931dcc00132f59
  get tdms mnaberez-type_03_int32_one_segment.tdms $m/type_03_int32_one_segment.tdms 832f5235673bc0e5076afaa91dd797929fbce320cb5ed9c5dc51de647851cabb
  get tdms mnaberez-type_03_int32_three_segments.tdms $m/type_03_int32_three_segments.tdms 7b1c760a85d8de14a6ccc6d00ea0485343c561f81a311e73abf9358c7797e09a
  get tdms mnaberez-type_03_int32_two_channels_one_segment.tdms $m/type_03_int32_two_channels_one_segment.tdms f4c7927c2268df4fc6363bb8c48120a4f5b5e0f5fe7416d7dc0eb9d192cc2bee
  get tdms mnaberez-type_04_int64_one_segment.tdms $m/type_04_int64_one_segment.tdms 2f3caa5bdfe78b64e005a889ce44c50e91267df755781c4516545adf116ce9f5
  get tdms mnaberez-type_04_int64_three_segments.tdms $m/type_04_int64_three_segments.tdms 1b2644b01f91fb19a840bed6534c8927212a657004f63751ebf044b7e272b1dc
  get tdms mnaberez-type_04_int64_two_channels_one_segment.tdms $m/type_04_int64_two_channels_one_segment.tdms 7cf626977c3b83306284a47f4059bd5eafbd8a62efa12b7588101640ecedc460
  get tdms mnaberez-type_05_uint8_one_segment.tdms $m/type_05_uint8_one_segment.tdms f91488efe48f6753110caad072d331f39e0a1a6b66dd923d9b907068df366057
  get tdms mnaberez-type_05_uint8_three_segments.tdms $m/type_05_uint8_three_segments.tdms b95290fa804781eb451cceb7a29c120e09858adb26effed1f1687dbc5e836cbb
  get tdms mnaberez-type_05_uint8_two_channels_one_segment.tdms $m/type_05_uint8_two_channels_one_segment.tdms c1cad26c12f28315d41b3981698aefd247462a86affec39bcb23f63d9b72eb9a
  get tdms mnaberez-type_06_uint16_one_segment.tdms $m/type_06_uint16_one_segment.tdms 1dc1f241570109976cf9050429d45b1ce6f71e7a1872e1dbbe0e29e61acb3097
  get tdms mnaberez-type_06_uint16_three_segments.tdms $m/type_06_uint16_three_segments.tdms 64d891712ffe60622b26c980dc3199004f1b501053b8c099b949c36d34fab417
  get tdms mnaberez-type_06_uint16_two_channels_one_segment.tdms $m/type_06_uint16_two_channels_one_segment.tdms 5a91bf7968da284588ab30fd339b5765d0a7e9589085641d43b39cf349a0ce82
  get tdms mnaberez-type_07_uint32_one_segment.tdms $m/type_07_uint32_one_segment.tdms f0ac18d09d858e6546bed47299f62845d1b5c51269ab1d86ceecbc3f307cc7a4
  get tdms mnaberez-type_07_uint32_three_segments.tdms $m/type_07_uint32_three_segments.tdms 16ac0041cf4913739a33d383b2f3536271c54035b236ee308f77e4622459112c
  get tdms mnaberez-type_07_uint32_two_channels_one_segment.tdms $m/type_07_uint32_two_channels_one_segment.tdms c01cc6d034ebf6493225e69d369d88018bd0392e674e830cc22e57287646f2ab
  get tdms mnaberez-type_08_uint64_one_segment.tdms $m/type_08_uint64_one_segment.tdms d6db37416bccba92f46fd696e4994fc410a480b3bf75ba4c644926320bb9dacc
  get tdms mnaberez-type_08_uint64_three_segments.tdms $m/type_08_uint64_three_segments.tdms b3a0173a229c52e70cb485c40c73a0bc06fea4d7aa3c11b67ea783e50e7d003e
  get tdms mnaberez-type_08_uint64_two_channels_one_segment.tdms $m/type_08_uint64_two_channels_one_segment.tdms c8e4a25341d54a121a1995911320d8177f63287320300552dee079b8dad64a12
  get tdms mnaberez-type_09_single_one_segment.tdms $m/type_09_single_one_segment.tdms 30ee3fd77e2b685735d7afd756a170291e4d8083a3c1fb39cf29ea1758204630
  get tdms mnaberez-type_09_single_three_segments.tdms $m/type_09_single_three_segments.tdms 769bd86746fdce0d4ff43d6e4504ee96a479270045cf80ddd570f5c1af922108
  get tdms mnaberez-type_09_single_two_channels_one_segment.tdms $m/type_09_single_two_channels_one_segment.tdms 1ffccd44e80e84b2eb14b84dc4f635b15a72f7da25b239d1c4fa7288869f2386
  get tdms mnaberez-type_0a_double_one_segment.tdms $m/type_0a_double_one_segment.tdms d09a629116f7a1cd26dd1176addc271ff88acaaac34f450ea9a376f0d96ffc16
  get tdms mnaberez-type_0a_double_three_segments.tdms $m/type_0a_double_three_segments.tdms 2e0a60102fce8aa5bd547946d73fe5b61acfd9561e3c96595ccf11ceca4061f6
  get tdms mnaberez-type_0a_double_two_channels_one_segment.tdms $m/type_0a_double_two_channels_one_segment.tdms 673d7f5453b91f0f1f6bd0ada7c20eda9a934b598ef253fdfab187e77f22a3d3
  get tdms mnaberez-type_19_single_with_unit_one_segment.tdms $m/type_19_single_with_unit_one_segment.tdms e8ab1f6dedad905be544f36022bd5f4e6935a73726491a58eec1cc3b6836a3a4
  get tdms mnaberez-type_19_single_with_unit_three_segments.tdms $m/type_19_single_with_unit_three_segments.tdms c4e9f7a99069bb6d9f6aab3fda0ec44271a3e2bd31cb6a1c770265288b7ad982
  get tdms mnaberez-type_19_single_with_unit_two_channels_one_segment.tdms $m/type_19_single_with_unit_two_channels_one_segment.tdms d0020a47c4405d6b57ad39b349db6b23f6897def1b1d5ed8b506e48e7a13944f
  get tdms mnaberez-type_1a_double_with_unit_one_segment.tdms $m/type_1a_double_with_unit_one_segment.tdms ad9bfe59bb9ed0e83c9a5b110c813ffe7c3f2d517e72819e1744e03dd03810e9
  get tdms mnaberez-type_1a_double_with_unit_three_segments.tdms $m/type_1a_double_with_unit_three_segments.tdms 06b3820b5f8b535e20e72057e54ab606d1736a7f2179e8a2d90b96c9807ddb1b
  get tdms mnaberez-type_1a_double_with_unit_two_channels_one_segment.tdms $m/type_1a_double_with_unit_two_channels_one_segment.tdms 03f04c96f9e0074fd8602f4d5c7fea112d807d3c0019a388dfd3dc050b5cc6ae
  get tdms mnaberez-type_20_double_two_channels_one_segment.tdms $m/type_20_double_two_channels_one_segment.tdms c85380a7fb3a61d3876c0abfb73cfc803c004263afc05547cf802a2371c5dfd5
  get tdms mnaberez-type_20_string_one_segment.tdms $m/type_20_string_one_segment.tdms 0b4cc4f946281b59ac4c5eb65db4850ac1bac0537bf1d5847ed008964156c108
  get tdms mnaberez-type_20_string_three_segments.tdms $m/type_20_string_three_segments.tdms 379aac0259af19526128228d6f1141a39cafcf7793cb201f4020e997c829b0ec
  get tdms mnaberez-type_20_string_two_channels_one_segment.tdms $m/type_20_string_two_channels_one_segment.tdms b2a5760c68978fd6cb652c492d03df05c69b5698154dad79c9524e8bbb5f3007
  get tdms mnaberez-type_20_string_two_segments.tdms $m/type_20_string_two_segments.tdms fdd28f77b528c8fbd679c898e9c8e0167803dff42e47025962dd9ebed5788638
  get tdms mnaberez-type_21_boolean_one_segment.tdms $m/type_21_boolean_one_segment.tdms 240c93d2fb2526e9b15701b89ef9b5967f0842d615bfd5724649014d5c88b912
  get tdms mnaberez-type_21_boolean_three_segments.tdms $m/type_21_boolean_three_segments.tdms 523741b8a31495801d27945f0794a67e35248d4a7fca6beb6dba463b3cb1828d
  get tdms mnaberez-type_21_boolean_two_channels_one_segment.tdms $m/type_21_boolean_two_channels_one_segment.tdms ab36e6ad9461d73179ed20b4d8f73f987accce19faaf3721734432f5f935d250
  get tdms mnaberez-type_44_timestamp_one_segment.tdms $m/type_44_timestamp_one_segment.tdms 75b5cb2680a7bd8c5736aa89c73f55835a889b6308dfd3421def6c35a5606517
  get tdms mnaberez-type_44_timestamp_three_segments.tdms $m/type_44_timestamp_three_segments.tdms 36e0299a4295683e8d196e2d6b47b093cab2aaaf40493d41eae564ec4fdd0724
  get tdms mnaberez-type_44_timestamp_two_channels_one_segment.tdms $m/type_44_timestamp_two_channels_one_segment.tdms cc435ea1a34768f8e1e73b17a0bc887aced952b40f1f6263c3ad3d35fa9bf342
  oracle tdms
}

# FCS, against FlowIO: every dataset's channel values, scale values ($PnE, $PnG, $TIMESTEP)
# and compensated values (FlowIO's scale values times the inverse spillover matrix: the
# density plots' inputs). fcsparser's test data (MIT) and FlowIO's (BSD-3-Clause), as in
# fetch-cytometry.sh.
fcs() {
  local fp=$GH/eyurtsev/fcsparser/master/fcsparser/tests/data/FlowCytometers
  local fio=$GH/whitews/FlowIO/master/data/fcs_files
  get fcs miltenyi_2.0.fcs "$fp/MiltenyiBiotec/FCS2.0/EY_2013-07-19_PBS_FCS_2.0_Custom_Without_Add_Well_A1.001.fcs" \
    28c442794bf920c72cb7c84cdca0e972d5cbace49727ddedde9d717c8b049128
  get fcs facscalibur_hts_2.0.fcs "$fp/FACSCaliburHTS/Sample_Well_A02.fcs" \
    bf3fb0140c63ca583a01ec5d6af46df1d00f2e2cd6151f5533ab13a7d7f34e27
  get fcs miltenyi_3.0.fcs "$fp/MiltenyiBiotec/FCS3.0/FCS3.0_Custom_Compatible.fcs" \
    9a56d8825ea533356443c3b849bef82c6027ca3b4eea9e52a4b74e2be25219ff
  get fcs facs_diva_3.0.fcs "$fp/FACS_Diva/facs_diva_test.fcs" \
    8d0a72d1d219c880d9120bac0b7501afa23a08b4ecd2b074803e73eaa411802a
  get fcs fortessa_3.0.fcs "$fp/Fortessa/FCS_3.0_Fortessa_PBS_Specimen_001_A1_A01.fcs" \
    fa9011c86e8ad043ab623656646f329aea907e9655e20f94ade97eea4b9dc177
  get fcs cyflow_cube_8_3.0.fcs "$fp/cyflow_cube_8/cyflow_cube_8.fcs" \
    08b8ef7fc49d34c56543551cc4c16bbf50c85af49d89b3abbe40d6f6a5d6a8d9
  get fcs cytek_xp5_3.0.fcs "$fp/Cytek_xP5/Cytek_xP5.fcs" \
    dd6aaa9a034c40ab5f3a2ce1ea4c8eab7d00a053426917e07a09f8d8d76ca51a
  get fcs guava_muse_3.0.fcs "$fp/GuavaMuse/Guava%20Muse.fcs" \
    f9b0ca844ecee33c935a6c5e6a7d81c5afe155ce40f0ed663bc51f0d9c98f0e9
  get fcs miltenyi_3.1.fcs "$fp/MiltenyiBiotec/FCS3.1/EY_2013-07-19_PBS_FCS_3.1_Well_A1.001.fcs" \
    8cfedc08f387a53cd492c98b5150c3a85ad704499aec07190de5bb249897598e
  get fcs duplicate_names_3.1.fcs "$fp/MiltenyiBiotec/FCS3.1/SG_2014-09-26_Duplicate_Names.fcs" \
    7ee7e214e54161ebea46aea860ef6c050dfdb1e093cd1d97684fbfd4bcb87cc4
  get fcs flowio_data1_2.0.fcs "$fio/data1.fcs" \
    cba7812f5bbce700786d22d97cf9858c1301878aa840c64eadcd263a3a0a5508
  get fcs flowio_3FITC_4PE_004_2.0.fcs "$fio/3FITC_4PE_004.fcs" \
    240e76eba7e04c7e9116c8c3ad8388ccb2faf58105d21b831d122764a3621e53
  get fcs flowio_variable_int_3.0.fcs "$fio/variable_int_example.fcs" \
    88d2933d52686b061f36d8596e0826c9d2e75eb3d654d570f4ad3f1153f9beb2
  get fcs flowio_G11_3.1.fcs "$fio/G11.fcs" \
    40ed9f6d4293a87077cab07a34c68b2497d4d69988adcb64a5f718098f48b56f
  get fcs flowio_B01_3.1.fcs "$fio/B01%20KC-A-W---91-US.fcs" \
    33736fb7a40a4f571837c231d342df5fd4b20b5dd70439b74b3610f94b6c2a36
  get fcs flowio_data_start_discrepancy.fcs "$fio/data_start_offset_discrepancy_example.fcs" \
    414fdccae26a65996324261ab028fb1c762e448313ebcf6207894b4dc43eef47
  get fcs flowio_data_stop_discrepancy.fcs "$fio/data_stop_offset_discrepancy_example.fcs" \
    49956b68a5131af1bba423a26fdaabd9fdf109b51d5f421a42971c09c920cc9d
  oracle fcs
}

# CFB, against olefile: every storage and stream (path, size, FNV-1a 64 of the bytes).
# Apache POI's test data (Apache-2.0: Word, Excel, Visio, Project, Outlook files, 512- and
# 4096-byte sectors, fuzzer cases) and olefile's own test file (BSD-2-Clause).
cfb() {
  local p=$GH/apache/poi/trunk/test-data
  get cfb olefile-test-ole-file.doc $GH/decalage2/olefile/master/tests/images/test-ole-file.doc a9645cd22f59f2314d0d0cf7ded60a536fb8422ed6b26115d2faa5442fad3336
  get cfb poi-20-Force-on-a-current-S00.doc $p/poifs/20-Force-on-a-current-S00.doc cecacf88f03237b59e187cce039522ede4e1013a7dead9e9f28c5d24ef4903c6
  get cfb poi-Notes.ole2 $p/poifs/Notes.ole2 613cfc4e02d7066465c88e2c9d34e163b8e143a44f3a8d7806daf9850555f087
  get cfb poi-ReferencesInvalidSectors.mpp $p/poifs/ReferencesInvalidSectors.mpp 42240fb6a4371d10eea27d1bbd3d82fa85da86346a0a9ea62f92fada08347e1c
  get cfb poi-only-zero-byte-streams.ole2 $p/poifs/only-zero-byte-streams.ole2 7a799f02b7f0d7fb51dcbd0a2dbd24e3c25c0d38c2c86e4023c1ee8bb181e028
  get cfb poi-unknown_properties.msg $p/poifs/unknown_properties.msg f14e95e2d02bb5aff56eba522e683541eb15d012eba4dcd5e276102e20a0f7d3
  get cfb poi-MailSentPropertyMultiple.msg $p/poifs/MailSentPropertyMultiple.msg af1e2789f0b73384c84acf3616fb1c3a6af89e7150029505f223367e434e474a
  get cfb poi-60256.bin $p/poifs/60256.bin dd22765657ba5364bc40946eb3688fed02012f12e412e481999268feb45ab70d
  get cfb poi-61300.bin $p/poifs/61300.bin 69ed9206b5d117e4a830f2a0689bd7401a9dbf34116bbbfebc72af6197230b8c
  get cfb poi-BlockSize4096.zvi $p/poifs/BlockSize4096.zvi bb3c9df7aadd500813f4510643fd5426f98908798b8c547f6ff0c24adda2a56c
  get cfb poi-BlockSize512.zvi $p/poifs/BlockSize512.zvi 4454d6470f6c6c750b9494094f239cf7e70be5c9cb13a0adfb42fcc3a395c49c
  get cfb poi-deep-data.bin $p/poifs/deep-data.bin 36f554325f22ea77da18d3feca5ae1e4ceb5565ade54e66b67558141d0e70e7a
  get cfb poi-oleObject1.bin $p/poifs/oleObject1.bin 46edb31d7679e9903ebef3acc6c902c8ff559ff5c13e2920e40e736bc34dfc14
  get cfb poi-64322.ole2 $p/poifs/64322.ole2 836fbe1328f3f68c21175aa68f2a43a31e805ca5580d95d4e26d535085d47611
  get cfb poi-TestBug44375.xls $p/hpsf/TestBug44375.xls 0260f4ff37a49f271a37b77ed93696c91db3be6a3b0bdd484bfe42bef90d03ce
  get cfb poi-TestBug52117.doc $p/hpsf/TestBug52117.doc 39dc33af3d370cb9ad5f1742298c5d657e8ed65d4c2251e789a38172e51735ff
  get cfb poi-TestChineseProperties.doc $p/hpsf/TestChineseProperties.doc 59bd71be14fef14b69d760204daaf7614d335163448d5448fa2b844adade9687
  get cfb poi-TestMickey.doc $p/hpsf/TestMickey.doc 407c896983b5a885af800c45be40c4dc49d41eb3ea7b8433ef3677c4fb51e14e
  get cfb poi-TestNon4ByteBoundary.doc $p/hpsf/TestNon4ByteBoundary.doc 01e9c68e492e5dbdc42e5f7a1e6c2ee53a1afed7462a733596dc8f44f1798928
  get cfb poi-TestSectionDictionary.doc $p/hpsf/TestSectionDictionary.doc 9c5f2ee44e43effff471751b50657e42156cd8063076a4f52a49869d15b50bc5
  get cfb poi-TestThumbnail.xls $p/hpsf/TestThumbnail.xls 2848f3f7bdac3307955aa475f6a43275a2ba6bfbaee5693f9cb1166ecece2645
  get cfb poi-TestUnicode.xls $p/hpsf/TestUnicode.xls d6e56522b14ff1be8c51270dfc612eebe5ee13b0588b189817ecd0f6193b6dc7
  get cfb poi-TestVisioWithCodepage.vsd $p/hpsf/TestVisioWithCodepage.vsd 31de63a4d06b699c2f4b64d66efd8c2e6e91ebcfb88f72dd2556026f8ce3807e
  get cfb poi-TestZeroLengthCodePage.mpp $p/hpsf/TestZeroLengthCodePage.mpp 9b18f64acb87a8fb00e157db010f0d6fd247aff0667f104f43b6aebee939cdac
  get cfb poi-51873.msg $p/hsmf/51873.msg 21ccc86ef46abbacfbb9b49a7e5e510fa65befdf2116cd34cd4de8bdd57087a8
  get cfb poi-58214_with_attachment.msg $p/hsmf/58214_with_attachment.msg 06a7b97925615e2296fa166db26da8b57d14f2eb3332da36dd6dd23a7a259517
  get cfb poi-attachment_msg_pdf.msg $p/hsmf/attachment_msg_pdf.msg 439a1fb117edd4c84b780fcf44bf1d5167166694130ced5743a70132a83c6970
  get cfb poi-blank.msg $p/hsmf/blank.msg b9e5b5d8fac07e7be9604a3f9b039b9f824978d8b59cdeb2768d5182a03825c0
  get cfb poi-chinese-traditional.msg $p/hsmf/chinese-traditional.msg b84c12748155dc57b41de02c6d43aa47e82b879020df070290633e9fd5ce7fdd
  get cfb poi-cyrillic_message.msg $p/hsmf/cyrillic_message.msg 8378273915639c4affdbe74a07105a653e3da752cd61882444deb5df771562c8
  get cfb poi-clusterfuzz-testcase-minimized-POIHSMFFuzzer-4735011465854976.msg $p/hsmf/clusterfuzz-testcase-minimized-POIHSMFFuzzer-4735011465854976.msg 56a323fbcf78dfc31b50735ed708721f85763e429df89db2f532157eca5040a0
  get cfb poi-clusterfuzz-testcase-minimized-POIHSMFFuzzer-5336473854148608.msg $p/hsmf/clusterfuzz-testcase-minimized-POIHSMFFuzzer-5336473854148608.msg 134d86d865dabae10bd8ce2dbd8cdd8c02d261de69b47cece7102aa29ad735b7
  get cfb poi-44501.vsd $p/diagram/44501.vsd 41665798154d7103adaa25295b269fecde246f840b26b79c9f3d3ee9d7cc690b
  get cfb poi-v5_Connection_Types.vsd $p/diagram/v5_Connection_Types.vsd c289c529db8d49781012a0ea49447ab1b4962d3cbf0cb72ddcd35eef20456d2c
  get cfb poi-visio_with_embeded.vsd $p/diagram/visio_with_embeded.vsd ce31a269ff6ca0b8a25394c8ae81114895a2eafb4328d72fc9b9450d79a48cf5
  get cfb poi-clusterfuzz-testcase-minimized-POIHDGFFuzzer-6478389109981184.vsd $p/diagram/clusterfuzz-testcase-minimized-POIHDGFFuzzer-6478389109981184.vsd 87a3ccc9d4b786e6545bea6a7c1ed2cd8c855f2299e90164dface445ffac9f17
  get cfb poi-clusterfuzz-testcase-minimized-POIVisioFuzzer-4537225637134336.vsd $p/diagram/clusterfuzz-testcase-minimized-POIVisioFuzzer-4537225637134336.vsd 4f85a2be8a3f2e9f9e8f8aa6abd5bf46e7c11ed675c7c67586f40f9258b99bb0
  get cfb poi-62624.bin $p/spreadsheet/62624.bin 03350dd3d52c7b8ff8970f40f46e547ab0254ec686c1a6ad6c6adbd8c8e39955
  get cfb poi-quick.doc $p/hmef/quick-contents/quick.doc 1240639edc264abf046523eed4bd0a154b0c4e487a9ec8b74be9d0c51b7de124
  oracle cfb
}

# PDS3 and PDS4 against pdr (Million Concepts' Planetary Data Reader, with pds4_tools), VICAR
# against rms-vicar (the PDS Ring-Moon Systems Node's reader). Real products: Voyager ISS raw
# and geometrically corrected frames and a Cassini ISS frame from the Ring-Moon Systems Node
# (VICAR with detached PDS3 labels: read both ways), the LCROSS NIR2 camera from the PDS
# Geosciences Node (PDS4); GDAL's autotest files (MIT): VICAR in every sample type,
# interleave, VAX reals and BASIC compression, PDS4 Cartography labels.
planetary() {
  local r=https://pds-rings.seti.org/holdings/volumes
  local g=$GH/OSGeo/gdal/v3.10.0/autotest/gdrivers/data
  local l=https://pds-geosciences.wustl.edu/Lunar/lcross-e_l-mir1-2-raw-v1/lcro_0001/data/20091009104100_preimpact/nir2/raw
  get planetary C1385455_RAW.IMG $r/VGISS_5xxx/VGISS_5101/DATA/C13854XX/C1385455_RAW.IMG be744cfd941f0b60dced1aaa928e10f546c7d2026524714ddc97029cdf1f2162
  get planetary C1385455_RAW.LBL $r/VGISS_5xxx/VGISS_5101/DATA/C13854XX/C1385455_RAW.LBL 911772036862aa093daa33f5d0d304c7d6beb13a3eebc0cb309ff04761f8e079
  get planetary C1385455_GEOMED.IMG $r/VGISS_5xxx/VGISS_5101/DATA/C13854XX/C1385455_GEOMED.IMG c24dcd76412793985f3d896930bbb80e84a51e3be89aa8f43c9f99e9a91bdc86
  get planetary C1385455_GEOMED.LBL $r/VGISS_5xxx/VGISS_5101/DATA/C13854XX/C1385455_GEOMED.LBL ef60465f7038d789b19afc5f4061b3dc266a64db1bb615572c106d6a48f9595c
  get planetary N1454725799_1.IMG $r/COISS_2xxx/COISS_2001/data/1454725799_1455008789/N1454725799_1.IMG 35b8eb50df68ebda657e6ca3e15b81bfa455de54ecbfbcecbd44bf67926e5c93
  get planetary N1454725799_1.LBL $r/COISS_2xxx/COISS_2001/data/1454725799_1455008789/N1454725799_1.LBL a6f6e3ee53707f2f3b2c0d8f0d63a11ebbbd7691c2bb56b822f281e2e901af05
  get planetary lcross_nir2_raw_20091009104207183.xml $l/lcross_nir2_raw_20091009104207183.xml b294a16d6d0d91fa1ba968f878831bcf8caaabb0ff4dc9405f31f5f613d13de3
  get planetary lcross_nir2_raw_20091009104207183.img $l/lcross_nir2_raw_20091009104207183.img 303de13754e133af9443caaf3607c1aafae8b40b184ded239727b158035995df
  get planetary vicar_all_ones_basic2.vic $g/vicar/vicar_all_ones_basic2.vic d25cd6dfee61d3828e80d3155540d5d9a319a49dc4feb22b7fcc9549f283b17b
  get planetary vicar_bigendian_float32.vic $g/vicar/vicar_bigendian_float32.vic 0ea1d40a9fa425edb33cfd6f6bb1c910e1dfc1548e5225a06c5365d599c6a7e0
  get planetary vicar_bigendian_int16.vic $g/vicar/vicar_bigendian_int16.vic 3986dde4c5f287f6e8ca903f9d0cda504fb4b7803f67289b4524e56edd70ee06
  get planetary vicar_binary_prefix.vic $g/vicar/vicar_binary_prefix.vic 115d86e07cf713da8000de4aeb30770d22579d7dc6659c8ab5d416e0342e1603
  get planetary vicar_byte.vic $g/vicar/vicar_byte.vic 424757bb8668de97abf73a07bc480d8701b81b0ee1fdc1c74416d009fce8013f
  get planetary vicar_byte_basic.vic $g/vicar/vicar_byte_basic.vic decbf18ae65be14e4fb99662b94a026c19642522d7f9db3b8eba38f5c53fa04a
  get planetary vicar_byte_basic2.vic $g/vicar/vicar_byte_basic2.vic 04d95eae22636d7eae26623aaf6b498367765c8ab729e385f5d27b74597c1ce9
  get planetary vicar_cfloat32.vic $g/vicar/vicar_cfloat32.vic aaf86e86e20ac9d44c0297239387800f68ddf00533ce88195e4400b81a721e14
  get planetary vicar_float32_bil.vic $g/vicar/vicar_float32_bil.vic df6cdc7a53c4613de18e57407591d02b04a60bed3b118be532c30cb5a1510b5c
  get planetary vicar_float32_bip.vic $g/vicar/vicar_float32_bip.vic 7b6b8629bedf07b466c33418fac561763b5e4fc4ceae28a50cf42ea3c4a398c5
  get planetary vicar_float32_bsq.vic $g/vicar/vicar_float32_bsq.vic 29908cd6d34c15628557e090e5d38afe3a6a8b675ac85bdcf29af7f0319343d2
  get planetary vicar_float64.vic $g/vicar/vicar_float64.vic c63b08f55d6b13327fdb4055f450e295116a193eed51d8e44e35abe9d0a60c4b
  get planetary vicar_int16.vic $g/vicar/vicar_int16.vic 3a635673be00d23d88035f19844751febc1355ac4aa6fb7ab3d94dab1126a352
  get planetary vicar_int16_basic2.vic $g/vicar/vicar_int16_basic2.vic 4c8fb9ad12a61db63f876b8a6ce78975b48afe1211fbdf0e245e42957d7ef953
  get planetary vicar_int32.vic $g/vicar/vicar_int32.vic bcb59c4cd6e101a891a4df5aba811bc1e6847c14d26f336abf047f0d036cdc95
  get planetary vicar_vax_cfloat32.vic $g/vicar/vicar_vax_cfloat32.vic 1cd5667af569a47ec842fe1355d12f18172b70732896860ce869d316196d50cf
  get planetary vicar_vax_float32.vic $g/vicar/vicar_vax_float32.vic 8aa8ab9c7500a13515e55ccdab321992812d371dc70f8ebdbbf668d9e1b00173
  get planetary vicar_vax_float64.vic $g/vicar/vicar_vax_float64.vic 8f9e95d3533e9e410e2a7e5b8170901293fc69c12995b34a10ca1ae762512671
  # GDAL's expected checksums of these files (its autotest), for what rms-vicar misreads.
  get planetary gdal-vicar-autotest.py $g/../vicar.py f71a7eb553346b77bb3f92860bdd154c2ce494540bd587167fb5543785f5f2f1
  get planetary byte_pds4_cart_1700.xml $g/pds4/byte_pds4_cart_1700.xml 19763ef58cae862892ba19497667eaa3db1825c877e607724fd6ed1f76813d5d
  get planetary byte_pds4_cart_1700.img $g/pds4/byte_pds4_cart_1700.img b55a841b7b95be907f6bb0d358b8d10c9dce6e485381eb9accb71e653597d9a1
  get planetary byte_pds4_cart_1b00.xml $g/pds4/byte_pds4_cart_1b00.xml 600aac241ad55aef6d7fb2168acd90e2d4e037de248ca9cd3e89226ed950512e
  get planetary byte_pds4_cart_1b00.img $g/pds4/byte_pds4_cart_1b00.img b55a841b7b95be907f6bb0d358b8d10c9dce6e485381eb9accb71e653597d9a1
  get planetary byte_pds4_cart_1d00_1933.xml $g/pds4/byte_pds4_cart_1d00_1933.xml 7ffe8d6564110eb09ca3b1dab49616708a61ac10856990edd6ddfab31945d7f3
  get planetary byte_pds4_cart_1d00_1933.img $g/pds4/byte_pds4_cart_1d00_1933.img b55a841b7b95be907f6bb0d358b8d10c9dce6e485381eb9accb71e653597d9a1
  oracle planetary
}

# SER, against a reader written here from Grischa Hahn's SER format description (v3, the
# oracle-samples.py `ser` section). PlanetarySystemStacker's sample videos (GPL-3.0 project; the
# videos are only test data, read here): 8-bit and 16-bit mono captures with timestamps.
ser() {
  local p=$GH/Rolf-Hempel/PlanetarySystemStacker/master/planetary_system_stacker/Videos
  get ser pss-8bit_mono.ser $p/8bit_mono.ser 8214442ce5d93ccd5118429081dd027e44f1603d645b5f394c16b19e9e0dbbb5
  get ser pss-16bit_mono.ser $p/16bit_mono.ser 43820987b37e545ebb2c3fa724699e3dbb70835a474400f3cadcaa53544f8aa0
  oracle ser
}

# CCSDS 121.0-B adaptive entropy coding, against libaec (through imagecodecs) and the CCSDS
# reference data itself: the 121.0-B-2 test data libaec ships (BSD-2-Clause project; data
# from CCSDS), every option, low-entropy set and the 32-bit SAR extended-parameter set.
aec() {
  local a=$GH/MathisRosenhauer/libaec/HEAD/data/121B2TestData
  get aec test_p256n01-basic.rz $a/AllOptions/test_p256n01-basic.rz 3f98ebb411dd1e84fcc3fc2c72ea8732b1d39064afc0362f30d163d57e8dd132
  get aec test_p256n01-restricted.rz $a/AllOptions/test_p256n01-restricted.rz c7edc76a4c7a71fcdc7a03809b430fa7c258879ea21be68235237138089449a4
  get aec test_p256n01.dat $a/AllOptions/test_p256n01.dat d72d231fa10ff04f39322ecb3f72fea8c62e2d63ec80490ba3bf12ba07d04204
  get aec test_p256n02-basic.rz $a/AllOptions/test_p256n02-basic.rz 5756b4aa50dd54aaff255cc6cf7acb83d4f7c5fd9d5d39d159af88e64d4421ee
  get aec test_p256n02-restricted.rz $a/AllOptions/test_p256n02-restricted.rz bdd27c357ac1c699f1aa16cb75ce2082b2ad6638abb9c6c43466817e08819595
  get aec test_p256n02.dat $a/AllOptions/test_p256n02.dat cdf8b818d9d2367e48d4800d1a9f28d0b5ac301c99d642ffa031e897f7746de3
  get aec test_p256n03-basic.rz $a/AllOptions/test_p256n03-basic.rz af33226374fc713fb07a3a85a8cca3251032458841e24b4d4c6de75941051208
  get aec test_p256n03-restricted.rz $a/AllOptions/test_p256n03-restricted.rz 2326a451d843a5ea58b0cd6371513eeaba20e65197a8a5be1d561c693a1a1bf9
  get aec test_p256n03.dat $a/AllOptions/test_p256n03.dat 84529d8d998d344ce39f66f580231e0c3ab2ab1e8b128a4cd4e5dc725bb0cdda
  get aec test_p256n04-basic.rz $a/AllOptions/test_p256n04-basic.rz 7e0e01a8a0e632480963b7d205d0ab6a4fa0ec7dbe6fc69d4b61c5bd336f4d5f
  get aec test_p256n04-restricted.rz $a/AllOptions/test_p256n04-restricted.rz 61dd238fa142df0df00df45ffc8e6dac4d79d85cb8378295a12632fc1a521291
  get aec test_p256n04.dat $a/AllOptions/test_p256n04.dat dfc6815ecfc1147309a55d67650601da4113c6ccf2934d0da162de7781e3f21b
  get aec test_p256n05.dat $a/AllOptions/test_p256n05.dat 6336a7d758a4741e8169b6a9abd6a1b99e27093ff7421d7ee7c88ed77e328b86
  get aec test_p256n05.rz $a/AllOptions/test_p256n05.rz a265ce9d9b709a0d946a622de63d3ff70667b8b821868c86400177cb8341d8b7
  get aec test_p256n06.dat $a/AllOptions/test_p256n06.dat 0aa4c1163521cd4d0e753a812422219255990d85c4efeee2d0e56f12a72892ee
  get aec test_p256n06.rz $a/AllOptions/test_p256n06.rz 57bf087ae42ecc32ce1f32a0dd881f5339c33a9d8a6c12303434fe2baa3b126b
  get aec test_p256n07.dat $a/AllOptions/test_p256n07.dat 88dd2932c1c86d1f9049cf721172f983483fa347bb1fdfa3e507e17c9e28353a
  get aec test_p256n07.rz $a/AllOptions/test_p256n07.rz 9542d531f454a4d8bc3d130051f41bbeaf232410f12c8403da73ee79a095183b
  get aec test_p256n08.dat $a/AllOptions/test_p256n08.dat db888931097f3015be1383feedcd99b4aeb7db7d188cea1fb22b5b38c89205f6
  get aec test_p256n08.rz $a/AllOptions/test_p256n08.rz 66b1c8549af47fe21f00229d427b84b2c50b9fa2268593336a5596f8f1b12456
  get aec test_p256n09.dat $a/AllOptions/test_p256n09.dat 6ae31947af7a16245128de7f400ef5820530e1268d59eca742b223b16325d2b2
  get aec test_p256n09.rz $a/AllOptions/test_p256n09.rz baeb98828995da7f42930bd1cd6a3c9939911499966f8614e2ad5a8e8306fe87
  get aec test_p256n10.dat $a/AllOptions/test_p256n10.dat c8a91fd7d4537b7a4d65d1d3347ec3fab35c3eaf3e0e58088d2f0b4e741409db
  get aec test_p256n10.rz $a/AllOptions/test_p256n10.rz 62a7c02b115457a32b4f1f5569a8156fc73cb086e67c64e1f53dd20291c2078a
  get aec test_p256n11.dat $a/AllOptions/test_p256n11.dat 4b5afa68a5e8982d69d8c13dce1fa655132229b867c18a6f8f1b1b7bd70d818d
  get aec test_p256n11.rz $a/AllOptions/test_p256n11.rz 0337b22d1c565eb8704f7e0f8f5135dd4fcec2c9f7daeeb947c1dddcb4733644
  get aec test_p256n12.dat $a/AllOptions/test_p256n12.dat c6d6b4ec2a9980f5edf26b8b0b1cf4155b155ee2d1eb3bd33294291b9f1d2332
  get aec test_p256n12.rz $a/AllOptions/test_p256n12.rz afb1a236d3c8724ecbf115d0dcfb7cea8a74194d96bdc46de3aa48dedeaa8612
  get aec test_p256n13.dat $a/AllOptions/test_p256n13.dat 9eb3abb1eff1b6423d23c3e8651f178a44ee73149b45270e78fa4e00754e196a
  get aec test_p256n13.rz $a/AllOptions/test_p256n13.rz 7016d2504c12f891904aab0036cd2b68a41c3f8b113196463d2d62438231af95
  get aec test_p256n14.dat $a/AllOptions/test_p256n14.dat 223db1a939846651de75940ff114e6e27eb62cea43fd0ffc51647738860cadea
  get aec test_p256n14.rz $a/AllOptions/test_p256n14.rz 211407ec0c95adc502660a949e26329e78fe078b8544fcb8e52b83ab4a7c5cd6
  get aec test_p256n15.dat $a/AllOptions/test_p256n15.dat 0305e3fa06fc731a55261a443c9d1a592a3cafa3e7e11cfa04d7f4b328d7d79c
  get aec test_p256n15.rz $a/AllOptions/test_p256n15.rz a52aaafa3c9916bf2c6baf552305f950557a0b3fb0a9c5412bc7d729041a54ce
  get aec test_p256n16.dat $a/AllOptions/test_p256n16.dat 566ac528028e6335b7b1bd957f98cf5d61e51e2b80e461392747a3164863b741
  get aec test_p256n16.rz $a/AllOptions/test_p256n16.rz 2463bcb83fb8756681bdcf24cac8b68a9a4f0a3a39814e46b11b60c652de82e8
  get aec test_p512n17.dat $a/AllOptions/test_p512n17.dat 975183f2d34951f053bd84f154fb4ca5959f1f9b4a8bd772e64cd82db4e82fd4
  get aec test_p512n17.rz $a/AllOptions/test_p512n17.rz 6bccbdadcf93fd1e15d76dba654ba221eb9e6f0e4d75a78c5058e13ca344ba3a
  get aec test_p512n18.dat $a/AllOptions/test_p512n18.dat de34f058909a43d6bd5cb491935f6dae1d2ad1d5d5be23b26a44f02ccc5d39ee
  get aec test_p512n18.rz $a/AllOptions/test_p512n18.rz 324d67ca39744794e422e8b26d83675f85ea0db31f6bb458996df16c80792479
  get aec test_p512n19.dat $a/AllOptions/test_p512n19.dat 76f2c869878d4665b4c0f58bc9f0587ba4fb1e4424a66773c7fa7ffb05e0f3ce
  get aec test_p512n19.rz $a/AllOptions/test_p512n19.rz b0c1e5c6c4aef699d5045e65a570c681a535ef6e725d0da37261f50cb1ad57f4
  get aec test_p512n20.dat $a/AllOptions/test_p512n20.dat 2240fb49b459e0552a6ad03303263543d5241d66fbfe9417be585cb93cdaf415
  get aec test_p512n20.rz $a/AllOptions/test_p512n20.rz 34c96e3e02809341f40ed328087a6c1c693603bb7866db3cddbeae172b393d24
  get aec test_p512n21.dat $a/AllOptions/test_p512n21.dat 4120f052e4f8f907d26996e77427ef9751e6d87619121dd812a92953117a067f
  get aec test_p512n21.rz $a/AllOptions/test_p512n21.rz c8f8d8fe4e89367ba77689bdc11ff34dfdee4b9ee45bd42052e2e94ee99b5b0a
  get aec test_p512n22.dat $a/AllOptions/test_p512n22.dat 8302782b0c82def975b54643cb99853a9b4799a76625b981d1dbbc3a14516e8c
  get aec test_p512n22.rz $a/AllOptions/test_p512n22.rz 3e68e05dcffaaaf72068360b0c7dd87faa26c532236411fe63e9b5253cde159e
  get aec test_p512n23.dat $a/AllOptions/test_p512n23.dat bf4ee35128b4cc7c64f91c9a438bdb0888a53e1e39a23f6b65e414f3966d6d68
  get aec test_p512n23.rz $a/AllOptions/test_p512n23.rz 44a0ccbe99a81f46cea7d947c8458848253f219c795b3c24afc12c3f5bc01f39
  get aec test_p512n24.dat $a/AllOptions/test_p512n24.dat ad60d838fc09b3e630e4f2612180c1346262d4fc19fef1ace803cc13c36b9592
  get aec test_p512n24.rz $a/AllOptions/test_p512n24.rz cde5f8a532aa5e1cea6136fe7f082b91c9966b36cf258289c46088ee322ee027
  get aec test_p512n25.dat $a/AllOptions/test_p512n25.dat abbe1c80570be700a2ae87fd824a349209c400dc0761be83725b55f3bd3d4002
  get aec test_p512n25.rz $a/AllOptions/test_p512n25.rz 5cce5d36e62495adc1bc362d9fdda2355ca4421040a936ecd4146cd8fd9459c9
  get aec test_p512n26.dat $a/AllOptions/test_p512n26.dat ae86ce2a794393c05937909d9802c4144a9b61ceabe302647f6064a0a3d5c8e3
  get aec test_p512n26.rz $a/AllOptions/test_p512n26.rz e55e02db506088a8d09ed08859dd42ad9d22ccc99b365fb80822a9ff585ce1a8
  get aec test_p512n27.dat $a/AllOptions/test_p512n27.dat 183129f792a90203a1717d5947a3712b71dd771969d6dbfad12cdd14620f639d
  get aec test_p512n27.rz $a/AllOptions/test_p512n27.rz 6534d8fd0e6b2cdc76f74eb6b3b4354b96f4ccb5c5ddf1303af67dfdcd9934c2
  get aec test_p512n28.dat $a/AllOptions/test_p512n28.dat 1d7ce0c08a336e78d9da99b4b1404277c221f8fb04ad842b3bb7b39be2b2e013
  get aec test_p512n28.rz $a/AllOptions/test_p512n28.rz 3e25ae450d34fe321544a6f878a2c297b19d49d1a4689bded06ca96b4ed4192c
  get aec test_p512n29.dat $a/AllOptions/test_p512n29.dat dd35e60933c712379a6e46f506a9d2638079dd15ac862b352c1410ea392a8b3d
  get aec test_p512n29.rz $a/AllOptions/test_p512n29.rz 7261e80da7f09dfd20fb0199dcca921dab129b4fa0ccfac9828a3ae9e3e8ce9c
  get aec test_p512n30.dat $a/AllOptions/test_p512n30.dat 0fe61add1706ba90ac89cac5907d69fa168c9013671336bfd6920b11976ba1ae
  get aec test_p512n30.rz $a/AllOptions/test_p512n30.rz d5f7c3153e802df345ffba10f98f083bf9a602734f59d84352dd96f293cd2abf
  get aec test_p512n31.dat $a/AllOptions/test_p512n31.dat c6c3ebf06e7d03111328cfa80a6e17013884d1764d0f1455ae33e83bbcab95c8
  get aec test_p512n31.rz $a/AllOptions/test_p512n31.rz 39b1583f6103db6ad47dfb1b29084501ad56e99ec46365c460791b49fb033bca
  get aec test_p512n32.dat $a/AllOptions/test_p512n32.dat 2bb0f6111ad079e043e0d08bfb1f73a9603cbbaa8c5a51bda8e75f8e92dd8f24
  get aec test_p512n32.rz $a/AllOptions/test_p512n32.rz 6b805c326b20216314a938f955e07ab199633718646ee73dccfd6ef2473d68c7
  get aec sar32bit.dat $a/ExtendedParameters/sar32bit.dat 7455f4e5f75cf7bbe9b6c792a06569ebf028ceb029c059a8cb0c8ca94ae07461
  get aec sar32bit.j16.r256.rz $a/ExtendedParameters/sar32bit.j16.r256.rz 15e56af8ca1b8b4821befa6d78a37f84afbe063aeb3b7406f074459ec945d8ef
  get aec sar32bit.j64.r4096.rz $a/ExtendedParameters/sar32bit.j64.r4096.rz 836566c5f735b4916cc4bd8e99c60614f4dae75e8d42e361279ee80033418fb0
  get aec Lowset1_8bit.dat $a/LowEntropyOptions/Lowset1_8bit.dat 7f7531cec41dcc2d5841666c2059ba54a9ecdd7f7782a4fc195e6bc6e25efbcb
  get aec Lowset1_8bit.n01-basic.rz $a/LowEntropyOptions/Lowset1_8bit.n01-basic.rz ec4e67e44c81362ef2afddbd31d516b55e45c532447b3b04fd9be1e8af5d0386
  get aec Lowset1_8bit.n01-restricted.rz $a/LowEntropyOptions/Lowset1_8bit.n01-restricted.rz a46dc4c6a370116a93e6f5531063faf6aad2a6e8c0c6ae7459f4e274f36d7490
  get aec Lowset1_8bit.n02-basic.rz $a/LowEntropyOptions/Lowset1_8bit.n02-basic.rz 6455e410fc42448ab9d86e73ede068524d51ff952504a049085c0a1c854dc660
  get aec Lowset1_8bit.n02-restricted.rz $a/LowEntropyOptions/Lowset1_8bit.n02-restricted.rz afb23eee1f4e94df0b1a0427e9bbe8fbc30fdfca852f04883739a53706f707fb
  get aec Lowset1_8bit.n03-basic.rz $a/LowEntropyOptions/Lowset1_8bit.n03-basic.rz b9b59a7a3311521915931d8775b16386f942e4fbe1023a322a0128fe5fd34f93
  get aec Lowset1_8bit.n03-restricted.rz $a/LowEntropyOptions/Lowset1_8bit.n03-restricted.rz b6a2e0ccd547f7c275395329188de4d516ac10f42f0abc5ef4c10b161effa81d
  get aec Lowset1_8bit.n04-basic.rz $a/LowEntropyOptions/Lowset1_8bit.n04-basic.rz 36c90bf178c2ac152200d354317f44305673b3b1b5f4722174e04b8a3f4212d9
  get aec Lowset1_8bit.n04-restricted.rz $a/LowEntropyOptions/Lowset1_8bit.n04-restricted.rz 40572e327eeb593d987f73a0f02e5bf87aafc59ca1df98f42d932016fd3aff46
  get aec Lowset1_8bit.n05.rz $a/LowEntropyOptions/Lowset1_8bit.n05.rz 99c1fcafeb2851f8dfba1670f0a60060c4e6de7ea30c147770eb072c703f4730
  get aec Lowset1_8bit.n06.rz $a/LowEntropyOptions/Lowset1_8bit.n06.rz 5506615ba7ee52bcc3e5ddb0b5dfa5e587a24a7afeea860e342d7c16cb2db644
  get aec Lowset1_8bit.n07.rz $a/LowEntropyOptions/Lowset1_8bit.n07.rz 29896806d0f146c1b2709de01361b2f534c231b68b319c71f28e157baec516e8
  get aec Lowset1_8bit.n08.rz $a/LowEntropyOptions/Lowset1_8bit.n08.rz 95d81c7d59d1a15ecc006762331cc2622a28fae767624424cad27b0133b1d0f1
  get aec Lowset2_8bit.dat $a/LowEntropyOptions/Lowset2_8bit.dat 74c74428f74ebc73f547e3bd6ebe3d6df9430943e7bc1fcd1488a075fcf20221
  get aec Lowset2_8bit.n01-basic.rz $a/LowEntropyOptions/Lowset2_8bit.n01-basic.rz 1a5f2627ebfd9b2d94ae612bf3d82574785eb79fcde08d79d9d1e22e495f54ea
  get aec Lowset2_8bit.n01-restricted.rz $a/LowEntropyOptions/Lowset2_8bit.n01-restricted.rz 5c570ffa03bf75d366308ff414da31758063da59fed0ede219ca3435372483cf
  get aec Lowset2_8bit.n02-basic.rz $a/LowEntropyOptions/Lowset2_8bit.n02-basic.rz 74156486f9a890745db174c67dacb06eb1ebb1ca09c7fee8e4d59c3a1438a278
  get aec Lowset2_8bit.n02-restricted.rz $a/LowEntropyOptions/Lowset2_8bit.n02-restricted.rz 7cdb9d371da3bb9e32baf66312c933b951fbfcef1729bbb0ab053a1275899818
  get aec Lowset2_8bit.n03-basic.rz $a/LowEntropyOptions/Lowset2_8bit.n03-basic.rz 5d345353854fcb164a2e07084e6722527a6494adda22ea07e7bb0d9abd4dcd44
  get aec Lowset2_8bit.n03-restricted.rz $a/LowEntropyOptions/Lowset2_8bit.n03-restricted.rz 315979c6f0ad58520c4f16d27fafaeaccdba37e36094ababfc658baf8fb9a705
  get aec Lowset2_8bit.n04-basic.rz $a/LowEntropyOptions/Lowset2_8bit.n04-basic.rz df26b2546e854ae4f3953c707434a0c83b1afeefe78142eefa6923b4e40307fb
  get aec Lowset2_8bit.n04-restricted.rz $a/LowEntropyOptions/Lowset2_8bit.n04-restricted.rz 23a32bed2a202c2c9164ae6742d1b1ce3d78951fd72f233d7020a4a8c8fc7322
  get aec Lowset2_8bit.n05.rz $a/LowEntropyOptions/Lowset2_8bit.n05.rz 8fa5811ea33711fd5caa71a2b7f8d85cdf7e39a2ab95ce0612a5e9c6be09e773
  get aec Lowset2_8bit.n06.rz $a/LowEntropyOptions/Lowset2_8bit.n06.rz 068c1a96890c96e376e17977316bda419458d0d18af0d8215669c5aa6046729d
  get aec Lowset2_8bit.n07.rz $a/LowEntropyOptions/Lowset2_8bit.n07.rz 5fd171964f49f9cc4ca50643272880fa604882d9fb3da000f02a77f5b4161dd3
  get aec Lowset2_8bit.n08.rz $a/LowEntropyOptions/Lowset2_8bit.n08.rz ab56ce56ea58f0f3561d1faa396f276d94b5e903bd579dbceee34696967023df
  get aec Lowset3_8bit.dat $a/LowEntropyOptions/Lowset3_8bit.dat a6f236edb0c9c6bd9232b430488d221d4824999f9fc503c518ad6e7db5b1b9ac
  get aec Lowset3_8bit.n01-basic.rz $a/LowEntropyOptions/Lowset3_8bit.n01-basic.rz 6669d9c971825412dcfa25481eab3d9f2b8e2166b123162bc45c18e155b08c5d
  get aec Lowset3_8bit.n01-restricted.rz $a/LowEntropyOptions/Lowset3_8bit.n01-restricted.rz 7777b74592520de6ff4b2f09154a30c7c72ba447caefb08935985e3469a8fab5
  get aec Lowset3_8bit.n02-basic.rz $a/LowEntropyOptions/Lowset3_8bit.n02-basic.rz 9b8c072259503e3c406fbff2c0ab2dabc353f9a7e0edf7cbda129979d3f3c9bd
  get aec Lowset3_8bit.n02-restricted.rz $a/LowEntropyOptions/Lowset3_8bit.n02-restricted.rz 442df8e3507265f907586b7afc509a6a56b22492b908faf94d897e527004ace8
  get aec Lowset3_8bit.n03-basic.rz $a/LowEntropyOptions/Lowset3_8bit.n03-basic.rz d1d284bc78ad6acbd942a680ad0d22dbac5d5dab6994e4fb88f71276cfc5c247
  get aec Lowset3_8bit.n03-restricted.rz $a/LowEntropyOptions/Lowset3_8bit.n03-restricted.rz 08665dadfa1f0300649f67037ea3b5fdadf6b04f44dceb7d22d920531a32f393
  get aec Lowset3_8bit.n04-basic.rz $a/LowEntropyOptions/Lowset3_8bit.n04-basic.rz fafdae2498a72308d85741c2db532544368d4f4d80bcff2551b12ea8926d6b7f
  get aec Lowset3_8bit.n04-restricted.rz $a/LowEntropyOptions/Lowset3_8bit.n04-restricted.rz 4a9edfe6153564ed36c91c8798a9ea9016efc22de1a874010cc8bfb054a9bd1b
  get aec Lowset3_8bit.n05.rz $a/LowEntropyOptions/Lowset3_8bit.n05.rz ac3dfbb4965be3a5d6ecc1754c238b6613b77155b2db55773abac4aac1f7c316
  get aec Lowset3_8bit.n06.rz $a/LowEntropyOptions/Lowset3_8bit.n06.rz 426248b24d86472f9f871cf3a8f54681bfbdc7d91a5a2e0d4347098708df3f37
  get aec Lowset3_8bit.n07.rz $a/LowEntropyOptions/Lowset3_8bit.n07.rz 52c6552276dc081232929a83bd45fbf72b839c791a48bbc1b07040a2192c411d
  get aec Lowset3_8bit.n08.rz $a/LowEntropyOptions/Lowset3_8bit.n08.rz 1ebc7d857a043728500140d3c1560ffcaf84a0e84c3e490736ae51c45fc01a33
  oracle aec
}

# CorelDRAW previews (the crate reads previews only), against Python's own RIFF walk and
# zipfile plus Pillow: which preview is shown first and its exact bytes (a DISP chunk's DIB,
# an imhd BMP, a ZIP package's page or thumbnail member), which Pillow must decode. Sembiance's
# sample collection (unknown licence; as in fetch-editors.sh).
cdr() {
  local s=https://sembiance.com/fileFormatSamples/image/cdr
  get cdr 11ARMRD0.CDR $s/11ARMRD0.CDR a9732855cbc532b6d4f4b6052e661ce47868ca7fe625db5c1a2416f688c39ee2
  get cdr 70DIVTR1.CDR $s/70DIVTR1.CDR 70746a57cf1204a7fa5ba9273f037b014eab1b312d7fa3ebca71a37178a5d7df
  get cdr ARMYSEAL.CDR $s/ARMYSEAL.CDR c7ad78f0af29204b74d19ebbd3b53a252720e412abd78a8d039dc0cb210f477a
  get cdr ARMYSTF1.CDR $s/ARMYSTF1.CDR ced75cfddd0339b50efb46ffb69777249ad7ca0045a6478107accc645a9da741
  get cdr ARROW5.CDR $s/ARROW5.CDR c411d91339a4148e8c27385aabe8a959706a65006f93e36746688bffbdf71a4e
  get cdr BALLET.CDR $s/BALLET.CDR 9f0a478e772de444483501afd14d0f74fe3b60ce515e7c8748190b62bf067e1a
  get cdr DALI.CDR $s/DALI.CDR 8dd7244ab048066a25b37dfb765cd3ec96d6485b3ef0a930e3131526e2fc0585
  get cdr DRINKS.CDR $s/DRINKS.CDR e6f3a9b16026e94e2c1e09b03555d37020afd90d2f779b11547545d2d1bcbc20
  get cdr FLAG.CDR $s/FLAG.CDR c058d643f64c2cdcac6af177ba41007d04c6c27f91ed655f12eae179138b8289
  get cdr FOX.CDR $s/FOX.CDR d16289fb20cab63fd3920695b51a625eed384405bafd31f343d77c1cd4d8dc5a
  get cdr MENU.CDR $s/MENU.CDR edca707f697a903e4fa20d05408ca71938e6ba06d69d6350a7a6e7906639a0a6
  get cdr TAKE_ONE.CDR $s/TAKE_ONE.CDR 7c83c672b3adf0c50c729c76a1f4fb333eef50c64596127822163f4f7fa45a81
  get cdr WI.CDR $s/WI.CDR c904cbf1de6b634e0557bb2b81a1af86a317941df17632650245b08e06ffe311
  get cdr example.cdr $s/example.cdr 33491adf9a9b6804cef20992a33a5d9deb6f2cf982dde741fb93bba684a08144
  get cdr test.cdr $s/test.cdr c783ad102dc0209d15bd61bcb719d1ec23bcf363750186a4eb594d6f9e9d377d
  get cdr web_cd.cdr $s/web_cd.cdr 67acd97ed0f7f94ec14eeb8f1edc3be61d2d28cccfd0a4a8d31f96985922eb90
  oracle cdr
}

# Clip Studio Paint, against Python's sqlite3 and Pillow: the canvas size, the preview blob
# (CanvasPreview.ImageData) and its decoded size, the layer names. clip_to_psd's and
# lycoris0731/clip's test files (MIT), as in fetch-design2-clip.sh.
clip() {
  get clip clip_to_psd-all-features.clip \
    $GH/dobrokot/clip_to_psd/HEAD/tests/test_export_all_features.clip \
    5e342dd8b397ed6c915b701bf77151cccafd93cc684bbe09439b91d8102c4e8e
  get clip lycoris-sample.clip $GH/lycoris0731/clip/HEAD/tests/assets/sample.clip \
    a313134c226fe80a756a36ba87657b028321b7d2f23971bb2ede86d847f784bf
  oracle clip
}

# 2-D spectrum contours (specular-spectra's chart), against scikit-image's marching squares:
# the contour length at twelve levels of the oracle's choosing, on synthetic seeded 2-D
# spectra written by oracle-samples.py (nothing downloaded).
spectra() {
  oracle spectra
}

# IFF ILBM colour cycling, against FFmpeg's ILBM decoder (the still image) and the cycling the
# file's CRNG / CCRT chunks define, stepped tick by tick in oracle-samples.py (no maintained reader
# plays it). Sembiance's ILBM collection (unknown licence): Deluxe Paint art with forward and
# backward ranges, several ranges at different rates; two DRNG files (no oracle models DRNG).
iff() {
  local s=https://sembiance.com/fileFormatSamples/image/iffILBM
  get iff AH_Dan.iff $s/AH_Dan.iff 12f2b811da46fa96b14dc1227983f2c818ce3b82fe9ea07ca3c981966098d340
  get iff AH_Eye.iff $s/AH_Eye.iff 54d5a96392ca8d01804dc38be97801db74e1a5bfa29c209acfebb39245063439
  get iff DECKER-BattleMech.lbm $s/DECKER-BattleMech.lbm 95148de6c8d5061be95bc240761273ed544f8bf4ce35336d85d5f542b5056edd
  get iff Danny_SkyTravellers.iff $s/Danny_SkyTravellers.iff 4827a325f1d0b5057aa1b858bf4959895eb87615e6a84fef4bf30e8d8f6594fc
  get iff V26.LBM $s/V26.LBM 2f4433a4444a541adb61a8275118fc608e2ec70399cbf23d956513c769a0ed8f
  get iff Watch.iff $s/Watch.iff 39224f805dd105dc0ae5ea89893f03b5ac122c5be70ab09445cd44c32208e345
  get iff alteq.lbm $s/alteq.lbm 6db17d5797ce280c300a40b28534c0eafef3c19c16a1dccbfbddc89203d39544
  get iff dalton-big-wheel-640x512.ilbm $s/dalton-big-wheel-640x512.ilbm 72107020c369f4cd1fd42e65c498e3887543234166c824d456781d06c137cd3d
  get iff moons.lbm $s/moons.lbm 13b1ed73c91454aa20c9b84de30281bb29026cc5d423432104a0d6135e00c18b
  get iff rahow.lbm $s/rahow.lbm 77354f193ea1a610c2c67acd1702c468bcbe3d25351b603b7067560ca8ac1970
  get iff water.lbm $s/water.lbm 96a85abb8ee8482bf58a54c335a14d089206fa9ff52f9ee0a553f35aed2650f0
  oracle iff
}

# TIM and XWD, against ImageMagick (as fetch-raster-legacy2.sh): real PlayStation TIM files
# (4- and 8-bit CLUT, 15- and 24-bit direct) and X11 screen dumps (1, 8 and 24 bits) from
# Sembiance's collection (unknown licence), plus 2-bit XWD and a mixed-mode TIM written by
# oracle-samples.py (no public sample has them; ImageMagick refuses mixed-mode TIM).
legacy2() {
  local s=https://sembiance.com/fileFormatSamples/image
  get legacy2 sembiance-123.tim $s/tim/123.tim 7bf1268a16291349221dbc0353d3ae21b2f0cec72a40ffe61b2b8637a887e045
  get legacy2 sembiance-EM048.tim $s/tim/EM048.TIM dfe718cd421f95006a97a8412fef30d53b8fc7eaedac02388e9c95f575f2ba66
  get legacy2 sembiance-FILE001.tim $s/tim/FILE001.TIM 03298fb2a12eb436b7475640a841ae3cc7b293caf83b81a85e8e8178e5e2e41f
  get legacy2 sembiance-MAP01.tim $s/tim/MAP01.TIM e4e92e4fb358f5acd9a71be8547b2c62b60b8f1b4f173a45746f4ac6bd4c2372
  get legacy2 sembiance-MAP07.tim $s/tim/MAP07.TIM 65907541f5aadb94ac6acea2e588f5d4c7d8459ba8bef8393e421d6c353a024b
  get legacy2 sembiance-MAP0E.tim $s/tim/MAP0E.TIM ece2fe795d7403b9903ab9ca799ac1b81ac076cb8a821a4a5781a584af5155af
  get legacy2 sembiance-PARTS.tim $s/tim/PARTS.TIM 1b2af850538efcc08cba9ba3afb0f533fbb20867f2323958e7ec88e44b045858
  get legacy2 sembiance-SHADOW1.tim $s/tim/SHADOW1.TIM a4b550c9f9e3b68bd69db5996a2f5e0d468ce33c2507808bf4880594d50d9a94
  get legacy2 sembiance-SILLY2.tim $s/tim/SILLY2.TIM 9291b1d22927b8614542e199e3ece2932a737b6327a673952409a12b2e53ef99
  get legacy2 sembiance-TH.tim $s/tim/TH.TIM 180de05ca8e93775f2a58413b0c36d49e486cb42f035d87d993d9df321718275
  get legacy2 sembiance-TOFULOGO.tim $s/tim/TOFULOGO.TIM 537ca2aa81cd3d3e9d4ed66c22e75d03049e7964b842500d85f0aa42131f3ab8
  get legacy2 sembiance-TYPE00.tim $s/tim/TYPE00.TIM 2aa60bf8546ba791f5d51dac45d68d4a1ee326dee2a7bf4ace85c764032eac3f
  get legacy2 sembiance-VRAM0.tim $s/tim/VRAM0.TIM 6a71b2babb29975565d2848e0e73727d0e6cadaebc4fd68baadbfe2e13ed44cb
  get legacy2 sembiance-input.tim $s/tim/input.tim 7af737f7f6fbf0747e73e20b1e0ec406331360874a336bb9cdb7e860468d185e
  get legacy2 sembiance-jopt06.tim $s/tim/jopt06.tim 4b758eb44739f8b6f1ed6991e980b4d6e963895f3469c74232e7246e88600308
  get legacy2 sembiance-opt11.tim $s/tim/opt11.tim 11960eba4cdbda76e6badbbcce877c03cb056a995f7c675792a0b28315c2e8b3
  get legacy2 sembiance-optkey03.tim $s/tim/optkey03.tim f37a0e48ac7627ee937c17001dd29f2d689d0afa3cd3138ae7e53d09f84ded8b
  get legacy2 sembiance-MARBLES.xwd $s/xwd/MARBLES.XWD 32619d6752a6d0c6cefe793c6a096b54caab4af372b53368eed31b969f312254
  get legacy2 sembiance-abydos.xwd $s/xwd/abydos.xwd 3bf5cd70528495558a21d80e3e5274a53589152cefcca07a40e51b57aace8237
  get legacy2 sembiance-fig41.xwd $s/xwd/fig41.xwd 6da082f9eda92a4dd0b73091a3827d4b10b1421b52d947f581982ebf56b7e426
  get legacy2 sembiance-input.xwd $s/xwd/input.xwd d56b679ef1138b36e0fbaffaeec64bcbfcf15acde83426ac6edb5f17a909d65c
  get legacy2 sembiance-screen.xwd $s/xwd/screen.xwd 031318c379c3e7e394586a98059377b35194a50075d7046cceba0cd61b0f9727
  get legacy2 sembiance-welcome.xwd $s/xwd/welcome.xwd 062b194f523fd59146e2869ddbe95741e486c7c4498643fb922766dff164c2c0
  get legacy2 sembiance-xyztool.xwd $s/xwd/xyztool.xwd 3ea41b101c3f1bab36be8b91685e0b24ec44d0dc6dd73ab9d238f589018ea167
  get legacy2 sembiance-betyboop-dmp.xwd $s/xwd/betyboop.dmp f2f77c493aaf6f77fd8e70e3896b2e628769c8c2434a02e01d3347840ebd224e
  get legacy2 sembiance-cccgirl1-dmp.xwd $s/xwd/cccgirl1.dmp 7fea3363d1736e7bbc43faa17460f61c32862c9df77392a1d36706c3f1aa3c73
  get legacy2 sembiance-woman-with-ban-dmp.xwd $s/xwd/woman-with-ban.dmp 3750563ac812b384f638a472838396b0322108c2586e43e747ea284bf7fafaad
  oracle legacy2
}

# Luma-only DPX (descriptor 6), against ImageMagick and OpenImageIO (as fetch-raster-film.sh):
# Sembiance's 8- and 16-bit single-channel samples (unknown licence), plus luma files that
# OpenImageIO and ImageMagick write at 8, 10, 12 and 16 bits (oracle-samples.py).
film() {
  local s=https://sembiance.com/fileFormatSamples/image/dpx
  get film sembiance-DLAD_8b_1c_big.dpx $s/DLAD_8b_1c_big.dpx c71dfc65509d37ed6683de9a18f0cfe15cd460de321344e9a23010a8936accf2
  get film sembiance-DLAD_16b_1c_big.dpx $s/DLAD_16b_1c_big.dpx b63444ccb9d470da8dde353fab04af5986c9ee50dbcb455789acd3ef70bbafac
  oracle film
}

ALL=(radiance icns xisf tdms fcs cfb planetary ser aec cdr clip spectra iff legacy2 film)
mkdir -p "$OUT"
venv
for s in "${@:-${ALL[@]}}"; do
  echo "== $s" >&2
  "$s"
done
