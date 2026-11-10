#!/usr/bin/env bash
# Fetch the diffraction detector corpus (CBF/miniCBF, ESRF EDF, ADSC and d*TREK SMV, Bruker
# v86/v100, MarCCD, Rigaku R-AXIS) into tools/corpus/xtal/ (gitignored: nothing here is
# committed), then write the fabio/pyFAI oracle output into tools/corpus/xtal/expected/.
# specular-xtal's tests/corpus.rs reads SPECULAR_XTAL_CORPUS (a worktree has no corpus of
# its own), else tools/corpus/xtal, and skips when it is absent.
#
# Usage: tools/corpus/fetch-xtal.sh [--no-oracle]
#   OUT defaults to $SPECULAR_XTAL_CORPUS, else tools/corpus/xtal.
#   PYTHON is a python with fabio, pyFAI and numpy (default: $CARGO_TARGET_DIR/venv/bin/python;
#   make one with `python3 -m venv $CARGO_TARGET_DIR/venv && $CARGO_TARGET_DIR/venv/bin/pip
#   install fabio pyFAI`).
#
# About 120 MB: FabIO's public test images (http://www.silx.org/pub/fabio/testimages/, the
# images FabIO's own test suite uses; contributed by beamlines and vendors for testing):
#   CBF     Pilatus 1M/2M/6M miniCBFs, an XDS-written CBF, a Mar CBF with 16-bit elements
#           counting past 65535; four of them also kept bzip2-compressed
#   EDF     multi-frame SAXS (Dummy, Center, PSize), gzip and zlib compressed u16, Oxford
#           Diffraction (TY1 header), Frelon, id13 with bad padding, Pilatus 2M
#   SMV     ADSC Q315 (3072², big-endian) and Rigaku HyPix-3000 d*TREK
#   Bruker  v86 frame with ASCII overflow records, v100 .sfrm with underflow/overflow tables
#   MarCCD  two .mccd frames; R-AXIS IV mgzn-20hpt.img
#
# Every download is pinned by size and sha256. Downloads are untrusted data: this script
# only copies and decompresses files, never runs anything from them. Stops if free space on
# the target filesystem drops below MIN_FREE_GB (default 8).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_XTAL_CORPUS:-$ROOT/tools/corpus/xtal}
MIN_FREE_GB=${MIN_FREE_GB:-8}
PYTHON=${PYTHON:-${CARGO_TARGET_DIR:-$ROOT/target}/venv/bin/python}
mkdir -p "$OUT/dl"

die() { echo "fetch-xtal: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

FABIO=http://www.silx.org/pub/fabio/testimages

# pinned NAME SIZE SHA256: downloads $FABIO/NAME once into dl/ and checks size and hash.
pinned() {
  local name=$1 size=$2 sha=$3 dest=$OUT/dl/$1
  if [[ -f $dest ]] && [[ $(stat -c %s "$dest") == "$size" ]] \
    && [[ $(sha256sum "$dest" | cut -d' ' -f1) == "$sha" ]]; then
    return
  fi
  check_space
  curl -fsSL --retry 3 --max-filesize $((size + 1)) -o "$dest.part" "$FABIO/$name"
  [[ $(stat -c %s "$dest.part") == "$size" ]] || { rm -f "$dest.part"; die "$name: wrong size"; }
  local got
  got=$(sha256sum "$dest.part" | cut -d' ' -f1)
  [[ $got == "$sha" ]] || { rm -f "$dest.part"; die "$name: sha256 $got, expected $sha"; }
  mv "$dest.part" "$dest"
  printf '%s\t%s\t%s\n' "dl/$name" "$FABIO/$name" "FabIO test images" >> "$OUT/.provenance.tsv"
}

# unbz NAME SIZE SHA256: NAME.bz2 (already fetched) decompressed beside it, checked.
unbz() {
  local name=$1 size=$2 sha=$3 dest=$OUT/dl/$1
  if [[ -f $dest ]] && [[ $(sha256sum "$dest" | cut -d' ' -f1) == "$sha" ]]; then
    return
  fi
  check_space
  bzip2 -dc "$dest.bz2" > "$dest.part"
  [[ $(stat -c %s "$dest.part") == "$size" ]] || { rm -f "$dest.part"; die "$name: wrong size"; }
  [[ $(sha256sum "$dest.part" | cut -d' ' -f1) == "$sha" ]] || { rm -f "$dest.part"; die "$name: bad hash"; }
  mv "$dest.part" "$dest"
}

# CBF
pinned Pilatus1M.cbf.bz2 777097 cfcfa09e85d065b92e21703862d4526845ff1822cef29d99e39b91afb1c2e1fa
unbz Pilatus1M.cbf 1128447 c6fc6157ae1fd660146a78155d0a08ee863e84e387015cb57b2fb1b1244e9e6b
pinned XDS.cbf.bz2 913556 bda0053a67fb6a69884f6a3eaafd80ecbb5ab2af8d7ecbd3ba2404eea46bb099
unbz XDS.cbf 2478080 079d8b55583fc3809767053d62333f353c749718d6d0205d35bbb1a51469a6ed
pinned testcbf.cbf.bz2 152020 834bcd6a2fd2622a2ead4308c0f60e13a5c544acdf7951bf70c1c2900c04a4dc
unbz testcbf.cbf 200377 f5c0b33b5ea1e54f130cf8d58002d20a7efb5ed66f5c4c90ee3cd726bf20c36f
pinned ref-test_5_0001.cbf.bz2 2074687 f01cc65196248651ea940d73c9a8bf8c9c1c8345c8893fe970be9d09c30668ba
unbz ref-test_5_0001.cbf 6229254 bb3590e6a104fd7d41edc2c5ad7e9adf9bd0a36e0405699c92869c0c371a8fa7
pinned v1_Nickel_Al2_00001.cbf.bz2 5045069 dea065f8e4e1922b807de60148ff7d7ab421014b1873fbf4a36e59933202f16b
unbz v1_Nickel_Al2_00001.cbf 8497012 6ae1d9ba2deaed9ad8a1057a75e089b90800953e403e71586396ead262208fb5
pinned run2_1_00148.cbf 6230840 6322bcd33dd2faed4dcac284c54ad6f318dee50829bb2e523a8537d2094eac37

# EDF
pinned multiframes.edf.bz2 380 e599331bb8c4454f0b9ed77eb0dfab28965cd51ab49808defd131bc544bfde4c
unbz multiframes.edf 16896 cedf23ec1063afb8da0125497a6a5eeef7e57d98afef84d3a4cc209d43f4f86a
pinned MultiFrame.edf 8202240 0d5acff8565b30b08d219b821b711c9f40f3441a2da65b6bfe85d37fca862a48
pinned edfGzip_U16.edf 3669504 32d7acea21eb405c7f9ccf7d26a9d736855e7455b905d13dc9236ccae0b12866
pinned edfCompressed_U16.edf 3669504 447bb96f21505c22273b126e5b3e2136bec29151c7391e546c58f8f7140fa35c
pinned F2K_Seb_Lyso0675.edf 8391680 d94042e57327476c461a90ce4d12f02a185aef4f27a888cffce4e633d400b924
pinned id13_badPadding.edf 1049600 c447da95d27a5eccfde377028161273d4ca0c62659040705288c58ac97a79ece
pinned pilatus2M.edf 9906612 105dca571da6fc8834eff3745ee15790bbcbb8bd580737b2066c8c1bb84cb63e
pinned d80_60s.edf 16779776 64992b70b52b78335fc12dc0e0174ef47a360ae8579be9e1d5d3e558674d700f

# SMV
pinned mb_LP_1_001.img 18874880 d14cae27c7b1a815a437a50b772feee6027691b4adeed1f690b370c7bc86b7fe
pinned HSA_1_5mg_C1_0004.img 1199132 c5c867da5838642b53eb17378bcc3c7954810fe31ecc5cb1fd227d43f858a2a3

# Bruker, MarCCD, R-AXIS
pinned Cr8F8140k103.0026 532480 702bc232bda07ba445eebee7993927748856d0379ea54823ce400c4c0e4f67dd
pinned NaCl_10_01_0009.sfrm 270016 ff03fa579c6cabb928cadb9b5f9e55cb3308346f555b6d58a9fb20f20871c37b
pinned corkcont2_H_0089.mccd 8392704 b1dad2815c8ec103a9f12a4a44250a09b9314f89eaf7922c2c9474e26fb8e11a
pinned somedata_0001.mccd 2101248 093c0cf1210d3fa8a4b41e3a17af313b4f67d242294f559e063c1116da847ff5
pinned mgzn-20hpt.img 5892600 67a4ae749ff05c3a1f8939cfd7f6690f91f463865940826b222dc1370c1b5fcb

if [[ ${1:-} != --no-oracle ]]; then
  [[ -x $PYTHON ]] || die "no python at $PYTHON (set PYTHON, or pass --no-oracle)"
  "$PYTHON" -I "$ROOT/crates/formats/specular-xtal/gen/oracle.py" "$OUT"
fi
du -sh "$OUT"
