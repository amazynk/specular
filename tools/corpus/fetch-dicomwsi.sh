#!/usr/bin/env bash
# Fetch a few small DICOM whole-slide series from NCI's Imaging Data Commons into
# tools/corpus/dicomwsi/ (gitignored: nothing here is committed) and write the wsidicom oracle
# beside them. specular-wsi's DICOM WSI corpus test reads it in place from
# SPECULAR_DICOMWSI_CORPUS (a worktree has no corpus of its own), else from tools/corpus/dicomwsi,
# and skips when it is absent.
#
# Usage: tools/corpus/fetch-dicomwsi.sh [SECTION...]
#   SECTION is src_idc or src_oracle (default: both), or `pin` to print the current object
#   listing with SHA-256s for the table below (downloads everything once).
#   OUT defaults to $SPECULAR_DICOMWSI_CORPUS, else tools/corpus/dicomwsi.
#   PYTHON (default python3) must have numpy, pydicom, wsidicom and highdicom for src_oracle,
#   e.g. a venv under target/: python3 -m venv target/dicomwsi-venv &&
#   target/dicomwsi-venv/bin/pip install --no-cache-dir numpy pydicom wsidicom highdicom
#   imagecodecs; then PYTHON=target/dicomwsi-venv/bin/python.
#
# What it covers (about 105 MB, plus 45 MB of oracle regions), from IDC's public bucket s3://idc-open-data (https), one folder
# per series, named after the collection and what it exercises:
#   kirp-thumbnail      TCGA-KIRP, a series of one uncompressed RGB THUMBNAIL instance, no
#                       VOLUME (wsidicom refuses it; no oracle) (CC BY 3.0)
#   htan-fluorescence   HTAN-WUSTL, 28 MONOCHROME2 instances: optical paths and levels as
#                       separate instances (CC BY 4.0)
#   hnsc-jpeg2000       TCGA-HNSC, JPEG 2000 (YBR_ICT) levels (CC BY 3.0)
#   hcmi-jpeg           HCMI-CMDC, JPEG baseline RGB levels (CC BY 4.0)
#   gbm-jpeg2000-422    TCGA-GBM, JPEG 2000 and YBR_FULL_422 (CC BY 3.0)
#   blgsp-jpeg-ybr      CGCI-BLGSP, JPEG baseline YBR_FULL_422 (CC BY 4.0)
#   luad-jpeg-levels    CPTAC-LUAD, JPEG baseline, a 27 MB base and its levels (CC BY 4.0)
# IDC's DICOM conversions are licensed per collection as above (see https://portal.imaging.
# datacommons.cancer.gov and each collection's DOI); they carry no real patient identity.
#
#   oracle/<series>.tsv and oracle/<series>/L<i>.rgb: tools/corpus/dicomwsi-oracle.py --raw
#   (wsidicom: slide size, levels, MPP, a 1024 x 1024 region at each level's centre).
#
# Downloads are untrusted data: each series gets its own directory, every file is checked
# against its SHA-256 below, and nothing from them is run (the oracle reads them with python -I
# from this repository's script). Stops if free space drops below MIN_FREE_GB (default 8).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_DICOMWSI_CORPUS:-$ROOT/tools/corpus/dicomwsi}
MIN_FREE_GB=${MIN_FREE_GB:-8}
PYTHON=${PYTHON:-python3}
BUCKET=https://idc-open-data.s3.amazonaws.com
mkdir -p "$OUT"

die() { echo "fetch-dicomwsi: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

# provenance RELPATH URL LICENCE
provenance() { printf '%s\t%s\t%s\n' "$1" "$2" "$3" >> "$OUT/.provenance.tsv"; }

# get URL FILE: download once (into FILE.part, then renamed).
get() {
  if [[ ! -s $2 ]]; then
    check_space
    curl -fsSL --retry 3 -A 'specular-corpus-fetch/1 (test corpus)' -o "$2.part" "$1"
    mv "$2.part" "$2"
  fi
}

# series NAME UUID LICENCE (SHA256 OBJECT)... : the objects of one series into OUT/NAME/.
series() {
  local name=$1 uuid=$2 lic=$3; shift 3
  mkdir -p "$OUT/$name"
  while (( $# >= 2 )); do
    local want=$1 obj=$2 out="$OUT/$name/$2"; shift 2
    get "$BUCKET/$uuid/$obj" "$out"
    echo "$want  $out" | sha256sum -c --quiet - || die "checksum mismatch: $name/$obj"
    provenance "$name/$obj" "$BUCKET/$uuid/$obj" "$lic"
  done
}

SERIES=(
  "kirp-thumbnail 7ea7112f-4c2b-48f7-8556-b7a7248856eb CC-BY-3.0"
  "htan-fluorescence 76cce1d6-61fa-4ced-bb8f-3695eb76758e CC-BY-4.0"
  "hnsc-jpeg2000 835947a4-e852-49b0-be10-a111408a9e14 CC-BY-3.0"
  "hcmi-jpeg f094a699-6ba7-4d34-9d1f-21d981975419 CC-BY-4.0"
  "gbm-jpeg2000-422 dd8ce01f-ff0e-44ff-95f9-4e212dd39b7d CC-BY-3.0"
  "blgsp-jpeg-ybr c039a2ba-9930-4f17-86a7-e8179ec55b38 CC-BY-4.0"
  "luad-jpeg-levels 6bf7d011-861a-41ee-8dd0-64750d400f0f CC-BY-4.0"
)

pin() {              # print the table below from the bucket's current listing
  local s name uuid lic obj
  for s in "${SERIES[@]}"; do
    read -r name uuid lic <<< "$s"
    mkdir -p "$OUT/$name"
    echo "  series $name $uuid $lic \\"
    for obj in $(curl -fsSL "$BUCKET/?list-type=2&prefix=$uuid/" | grep -o '<Key>[^<]*</Key>' |
      sed -e 's|<Key>[^/]*/||' -e 's|</Key>||'); do
      get "$BUCKET/$uuid/$obj" "$OUT/$name/$obj"
      echo "    $(sha256sum "$OUT/$name/$obj" | cut -d' ' -f1) $obj \\"
    done
    echo
  done
}

src_idc() {          # IDC whole-slide series (pinned below; regenerate with `pin`)
  series kirp-thumbnail 7ea7112f-4c2b-48f7-8556-b7a7248856eb CC-BY-3.0 \
    d34332af4076d683656a18c5d5e0225fbf38b08c73ec4b14273fae1571ddb757 ddedaf33-e4ea-4765-b95b-28f8972843e3.dcm
  series htan-fluorescence 76cce1d6-61fa-4ced-bb8f-3695eb76758e CC-BY-4.0 \
    15b197d3e4afafbabdc62279c5808465a9031b5e3cce771a5988d8afb466afa8 01327859-3380-423b-8ed7-cd7285bcc5c9.dcm \
    0170067ef02402a96a9999ba1cd04cc49674fa7b6e9ce848bedbc9cacb660ca6 06489932-3488-4ebc-9df0-28aaf228b4f3.dcm \
    e795b73e3fb9728738ffa82e23f22404b41494eb1aa751a4d1ce3ad317e260dc 08ef09a9-198f-434a-b7bb-4caefcd88a1b.dcm \
    25fd2c30f1cefe81bfec0dba6c74f273abd7a601f61f263cf991aad691ad6447 0b8b3c11-7840-4139-a610-5939b5e90c04.dcm \
    6d457a4e6031a29ba9622e58f99aeab8f885c2981a83ebfc8ce10db39acc9707 0fc58a47-2b0b-42a0-9451-14548812edfa.dcm \
    ad0f0f95fd8752eec47d0ded6a5813142fa68ccc3b3cac15ccc014310dda2048 301c16e6-c193-4210-a841-1ae46f1f648e.dcm \
    cf4637a4b0be48c04d6b227eeb007d79703cffc5a030ed84c45b4308e3b1cea5 343750fe-3d1b-4b43-99fc-12e21bab4a30.dcm \
    2c51fb0ca145167f21e981b147d17e3bb3141e5a4890c407d448e0faa419f369 3800779c-900a-4084-81c1-aaaa574f57ce.dcm \
    34dfa1ecf4a88a8e790e618dd65fa96207702a9aeab0e090a7f434b5f0374467 3a688fbf-3895-448a-b154-72ac3e62f267.dcm \
    da6fefb05478446a0bc21a4ad5d4d6112bc7a65d28ce266fe94c21ca3709cbf9 4a2f7891-8b25-4267-be2e-83dfdb15146b.dcm \
    424736aa995de3e988a1ca32a207a3be80cf51860cea3bd466e9e968c8571700 55eda235-4309-4b10-b03c-c295cf1e2d84.dcm \
    0bac1c79fe102bcdcf2815fb3c088f6663d0ee2e8ca296edf172e1695ebeeaa6 569c1c09-6b31-47a5-893d-ef0083f83851.dcm \
    9e9433d835c8977231c5755b469b3230169e14fe52d6562fa32d93ee2ba2ac6b 5cf2fa99-385e-4273-b2d3-1788625bdc5a.dcm \
    4cdb90faad500dee9cba247b63c16fb42ffc2a014dc9118dc6fb29f84b033b93 5ec7a28b-23de-4d1f-810f-124a171ff07a.dcm \
    a542994807300e5f9ce9ecb6ffb9a7a0ae5ac2362611233107180fdad8f86100 68553600-7df7-494a-aedf-a75b38d5b2b1.dcm \
    b4f95579fd27c51e80e8de97ced19edc853e3ee0da3b57524b8b4e4fd3e3b8a2 736ff290-8b81-4bfa-a7d7-11c1b4fa4f98.dcm \
    cd058ac9cab1b9244faba056b3c4ffbf433249e948040cada8dda47fa957b1db 7cd43b6b-a442-4b51-8935-7974e8fd0228.dcm \
    03003d6f008e51355dfc82c144f363d60744de460dfab5ffeb12a7eb4eb80abe 7f456d33-6d0e-4a27-ac3b-6ffdbf9ee6c7.dcm \
    16fa2c260004d711a981b1f49ada75ef40f5a99871176756f65d1fad37f11f5f 82e0a591-62d2-4ebe-a8e0-154e4e6301e9.dcm \
    33321ac75c52a3d9beb6ce86ca197b270565ce887971f953eceb89ff8b6d08ab 85822e73-cac3-4796-8bd6-0d4ad02058c6.dcm \
    51d94d38f2f337538ad2d2211897efab32ff82cdaed90c0a9e81b83888ea5c42 95988349-d1a8-4375-9c94-5bb1a1511295.dcm \
    7292d4169f83c38fd9dea25a14791a6ee4894b6e12c197607c4e2cc8085dd2e1 a65d56b6-30ad-4841-8dc0-70469879fd8f.dcm \
    30738b58ad254e04acde8744cf396daff7b28e52807e42eec9ee72863fb7d160 a7e9d78b-15ce-406d-a25b-709ccf2dc4c3.dcm \
    8f3dab6e26cb0603eaf99e709471c7ec73b2a2a39fafeb0ee95db811f2d22616 ade0b966-9670-4c73-9506-83cb90a8fede.dcm \
    75e7f4bdf3178457b20a7e4381292fa040ed46cb57fa4c5d9d31b2f6e13b9256 c29f0081-a60d-41b1-b9c7-e4a7a05d4fd0.dcm \
    dc92d1eb19728a3ae4d768fe40bb8a22592fc1a7557fed3011c8b033f6152acb e01326b8-461e-417c-844e-8dea53d983db.dcm \
    43c0375d9eca0a81ba2b7f1c97153688f95b32156f2ad89e92b97d8b21516d7e e49c34ab-b6ee-4460-9436-261d33ebd805.dcm \
    3f48f01acf9fefc09f10dc98580dfb6e0faac5f8a2c751c69bed0ab3251f1a7d f29bbad6-95be-4314-8e4b-f9925501a6ba.dcm
  series hnsc-jpeg2000 835947a4-e852-49b0-be10-a111408a9e14 CC-BY-3.0 \
    4d851bc70fadc22f6c275132b2adc39649fbf8caff8d964343e1da8c0e4e9828 76d3ae04-b6bc-4446-9b66-79dbd7e94e5b.dcm \
    43adefc0f6cc26e245e40c64a2ff17d12e1a8261d5756aa1c4571320fd3b1f8a bae2bddf-93d6-4638-beeb-2364c6935888.dcm \
    cf8c4d9b3f7054dbf2bb7e4bc15f30dff6afd76030c19a05d926c90f07572d26 e9b35731-6972-491f-9938-b5fcd0454bbe.dcm
  series hcmi-jpeg f094a699-6ba7-4d34-9d1f-21d981975419 CC-BY-4.0 \
    1be95efda0d777a8ff428fa1de59c984d7ec949ffdbbad1ab7a294f6b9869657 5a2feadf-4a54-43d5-ae50-f9e2ea21dd6e.dcm \
    c5f7129b445a7a36ea95494718f4e0bb96edc78601a570fad42b3c2e371e9257 706eafb8-8e26-45e0-971c-bc054a1190e3.dcm \
    1cf8fd37fc0f8af6c413b2787f1fa4bb1c3ea938f69b18f476d13e2f87ae8a80 738f6d94-c45e-4d0a-be15-6562d65e0f57.dcm
  series gbm-jpeg2000-422 dd8ce01f-ff0e-44ff-95f9-4e212dd39b7d CC-BY-3.0 \
    e05a27d100e17b5ceea4d49b9054437613e6c2e51f6d5b89806d1cff9c9abffc 45323799-e7cc-495f-ad40-dc87f2b3b8b5.dcm \
    4c210d00a4178403967ac492540930628cb9a6229e51f1bcf317a706144bc422 66ccbdd3-3b19-4ced-95a8-346f7896536d.dcm \
    3f723f1c660d69f0aecec6a9748a9cce2482bcabdba92ff9a56929e2dde9f6fb d66f9cf0-bc98-458a-a16f-2955bbb6010b.dcm \
    d242de618747bf75e258d904e03f3c8221b718495f5f4af8d7d29d2c4b3e687a ea722e0a-21fc-48b5-b53c-880c3b9429d1.dcm
  series blgsp-jpeg-ybr c039a2ba-9930-4f17-86a7-e8179ec55b38 CC-BY-4.0 \
    4b42ee3dda701e5f3c83f576ede9eff4d7b2035c78dcc1b15129181c47e25c66 3080ccb2-aef9-49b2-bf29-22e12e59900e.dcm \
    a805e6cc26c1b7bd258e8f3b683d6a26a163e3c22f19b6fd35503120a8e90f1d 6184a7b7-41ea-4521-b83e-c60844f347b6.dcm \
    f1ea5efd1edf5323d98a4f3f5c17af6cb400d52e48903efb38125ad8fd25b667 70510aa9-8986-4d66-acff-95386a65419e.dcm \
    0a4daf16a6293a23040b22786dfcebc01bc7679cec26caf468900fb2a9fef2dd 7698581a-77d2-40dc-9cc6-da9595de572e.dcm \
    05a85798a43c86f527274b065584d4f4108fe7e418e7989c56cf3a628b28f674 7810fd12-b139-4d70-b738-7115ad00dfa7.dcm \
    58b805bee55abf87c37a94837a8033e7429dfbe9d7b095325f3977b051ef1d20 9ab3db8c-446e-48b2-982b-551a99245e11.dcm \
    51e478a047bdd68729af4f81160504964cf9f80d479448405a3c8287f9f34a47 aab4d95a-5dee-454d-a60a-4225d4432102.dcm \
    372c5c938a461f5830e21047ff426c1496d9b2b78a2adf2233ad2a8f38a1c5df b3f412ed-734f-409d-bdcd-10e37b1d7186.dcm
  series luad-jpeg-levels 6bf7d011-861a-41ee-8dd0-64750d400f0f CC-BY-4.0 \
    256b504b9ee1e178d8ecfd5b6f426f057a2cc3b72cd07b2c94dccfb14ed4b278 48f35a4a-d833-49d4-8a17-da02b719da65.dcm \
    e5a072cd9d454bd7b9210d634a6ffc45765f6c38b51f2011108a85d967913c70 526c2f8a-f236-431d-907c-1d0c79f1a724.dcm \
    5c02f26955291195125fb4c0973890a94e16c960386dbe03aa24ee31583ddc4f 965e0f4e-e798-4c6e-a883-de0788c2178b.dcm \
    848217d9e9a1533fdfa72febcae0d9260f71e019345bd3526c1b06916e87e770 f59238b1-c772-4f7f-ad81-b93d17354a8f.dcm \
    2a7b4fb911ad140b8d77553976f5c3edbc6c3ac290dfbef480b4f326db766c6e fdbc4b54-bc99-4a61-a89e-c556452adc26.dcm
}

src_oracle() {       # oracle/<series>.tsv and reference regions
  local d
  check_space
  mkdir -p "$OUT/oracle"
  for d in "$OUT"/*/; do
    d=${d%/}
    [[ $(basename "$d") == oracle ]] && continue
    "$PYTHON" -I "$ROOT/tools/corpus/dicomwsi-oracle.py" --raw "$OUT/oracle/$(basename "$d")" \
      "$d" "$OUT/oracle/$(basename "$d").tsv" || echo "fetch-dicomwsi: no oracle for $d" >&2
  done
}

if (( $# == 0 )); then
  set -- src_idc src_oracle
  : > "$OUT/.provenance.tsv"
fi
for s in "$@"; do
  declare -F "$s" > /dev/null || die "no section $s"
  "$s"
done
du -sh "$OUT" >&2
