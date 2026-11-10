#!/usr/bin/env bash
# Fetch the DICOM structured report, waveform and radiotherapy corpus into tools/corpus/dicomrt/
# (gitignored: nothing here is committed), and write its oracle tables.
#
# Usage: tools/corpus/fetch-dicomrt.sh [SECTION...]
#   SECTION is a function name below. Default: all, in order. OUT defaults to
#   $SPECULAR_DICOMRT_CORPUS, else tools/corpus/dicomrt. src_oracle needs PYTHON (default
#   python3) with pydicom, numpy and dicompyler-core, e.g. the venv of
#   crates/formats/specular-dicom/gen/dicomrt_make.py: PYTHON=target/dicomrt/venv/bin/python.
#
# What it covers (about 35 MB):
#   pydicom     pydicom's RT Dose, Structure Set and Plan, its Comprehensive SR and its 12-lead
#               ECG (MIT)
#   highdicom   highdicom's TID 1500 measurement reports (MIT)
#   dcmqi       dcmqi's TID 1500 example (BSD-3-Clause)
#   dicompyler  dicompyler-core's example plan: a 3D dose, a structure set and a plan (BSD)
#   tcia        TCIA Pancreatic-CT-CBCT-SEG patient 027: an Eclipse dose and its structure sets
#               (CC BY 4.0, doi:10.7937/TCIA.ESHQ-4D90)
#   oracle      oracle.tsv: dicompyler-core's DVH of every ROI over the doses with a structure
#               set of the same frame of reference, pydicom's SR content items and waveform
#               samples (gen/dicomrt_oracle.py), read by specular-dicom's tests/dicomrt.rs
# No public Ophthalmic Tomography sample was found to pin; OCT is covered by the synthetic
# fixture only.
#
# Downloads are untrusted data: each source gets its own directory, every file is checked
# against its size and SHA-256 below, and nothing from them is run (the oracle reads them with
# python -I from this repository's scripts). Stops if free space drops below MIN_FREE_GB
# (default 6).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_DICOMRT_CORPUS:-$ROOT/tools/corpus/dicomrt}
SCRATCH="$ROOT/target/corpus/dicomrt-fetch"
MIN_FREE_GB=${MIN_FREE_GB:-6}
PYTHON=${PYTHON:-python3}
GEN=$ROOT/crates/formats/specular-dicom/gen
mkdir -p "$OUT"

die() { echo "fetch-dicomrt: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

# provenance RELPATH URL LICENCE
provenance() { printf '%s\t%s\t%s\n' "$1" "$2" "$3" >> "$OUT/.provenance.tsv"; }

# fetch URL DEST NAME SHA256 SIZE LICENCE
fetch() {
  local url=$1 dest=$2 name=$3 want=$4 size=$5 lic=$6 out="$OUT/$2/$3"
  mkdir -p "$OUT/$dest"
  if [[ ! -s $out ]]; then
    check_space
    curl -fsSL --retry 3 --max-filesize $((size + 1)) -A 'specular-corpus-fetch/1 (test corpus)' \
      -o "$out.part" "$url"
    mv "$out.part" "$out"
  fi
  [[ $(stat -c %s "$out") == "$size" ]] || die "size mismatch: $dest/$name"
  echo "$want  $out" | sha256sum -c --quiet - || die "checksum mismatch: $dest/$name"
  provenance "$dest/$name" "$url" "$lic"
}

# tcia_series DEST SERIES_UID SHA256 LICENCE
# One public TCIA series through the NBIA REST API (no login for public collections). The zip
# is built on request, so the pin is the SHA-256 over the sorted SHA-256s of the files in it.
tcia_series() {
  local dest=$1 uid=$2 want=$3 lic=$4 work="$SCRATCH/tcia_$2"
  local url="https://services.cancerimagingarchive.net/nbia-api/services/v1/getImage?SeriesInstanceUID=$uid"
  if compgen -G "$OUT/$dest/*.dcm" > /dev/null; then
    provenance "$dest" "$url" "$lic"
    return
  fi
  check_space
  rm -rf "$work"; mkdir -p "$work/x"
  curl -fsSL --retry 3 -o "$work/s.zip" "$url"
  unzip -q "$work/s.zip" -d "$work/x"
  local got
  got=$(find "$work/x" -type f -name '*.dcm' -exec sha256sum {} + | cut -d' ' -f1 | sort \
        | sha256sum | cut -d' ' -f1)
  [[ $got == "$want" ]] || die "tcia $uid: content hash $got, want $want"
  mkdir -p "$OUT/$dest"
  local f
  while IFS= read -r -d '' f; do
    cp "$f" "$OUT/$dest/"
  done < <(find "$work/x" -type f -name '*.dcm' -print0)
  provenance "$dest" "$url" "$lic"
  rm -rf "$work"
}

PYDICOM=https://raw.githubusercontent.com/pydicom/pydicom/60cebd027861e0dd4dfac12431679cb49e0156c3/src/pydicom/data/test_files
HIGHDICOM=https://raw.githubusercontent.com/ImagingDataCommons/highdicom/847fea5946b4be9cf0c9939b8f9dcbf0bc60b24d/data/test_files
DCMQI=https://raw.githubusercontent.com/QIICR/dcmqi/5571494a14416ad951a0979d2329cf15b8bba019/data/sr-example
DICOMPYLER=https://raw.githubusercontent.com/dicompyler/dicompyler-core/0d5729d7a2412e1c3f1e4cb4f6b07f7407eb8b06/tests/testdata/example_data

src_pydicom() {      # pydicom test files at 60cebd0 (MIT)
  fetch $PYDICOM/rtdose.dcm pydicom rtdose.dcm \
    1d6cc092146d093e086a6bcccef4ebb7d097941343f5cd3b6395d157b64e37e4 7568 MIT
  fetch $PYDICOM/rtstruct.dcm pydicom rtstruct.dcm \
    40c41bdf871fd8553396b02476a66024ed23c04927c0dc53fd10ecd3472cd0d3 2534 MIT
  fetch $PYDICOM/rtplan.dcm pydicom rtplan.dcm \
    18585dbbd6f7c5d1b7e749d6976d72251802ad89d65bccd31c03006f95aab89b 2672 MIT
  fetch $PYDICOM/test-SR.dcm pydicom test-SR.dcm \
    eebf00a37e97503b5a65022f9c2f89db6e8dac4cc632682aa3456aee1b6c177e 6796 MIT
  fetch $PYDICOM/waveform_ecg.dcm pydicom waveform_ecg.dcm \
    72f1cb0e65e8023321acdaa5425c44125cd507f5aaa148f7fe10516e1d2e688a 291088 MIT
}

src_highdicom() {    # highdicom test files at 847fea5 (MIT)
  fetch $HIGHDICOM/sr_document.dcm highdicom sr_document.dcm \
    1c291a931a91f096dcef26847e9dff7be2f356d752219d5e07136bc41efd6372 5004 MIT
  fetch $HIGHDICOM/sr_document_with_multiple_groups.dcm highdicom sr_document_with_multiple_groups.dcm \
    766a928d63f54a4ab3a12e88114ebcf00f9a8a5fd47103733d484ae5818a8f2d 8600 MIT
}

src_dcmqi() {        # dcmqi example at 5571494 (BSD-3-Clause)
  fetch $DCMQI/sr.dcm dcmqi sr.dcm \
    356a3f8f2765de9e59627b77a82590dbd0088a676466ac2ee42c52a58efbb975 77530 BSD-3-Clause
}

src_dicompyler() {   # dicompyler-core example plan at 0d5729d (BSD)
  fetch $DICOMPYLER/rtdose.dcm dicompyler rtdose.dcm \
    a78d4d7723e280b1baf8153a43583fda384a681428eca306b53ada37ef7d3123 10014610 BSD
  fetch $DICOMPYLER/rtss.dcm dicompyler rtss.dcm \
    8fe3e3a20d1acf911f5c284dc40288d46f97acd43e4a63753cd6e3e1dac398cb 1940626 BSD
  fetch $DICOMPYLER/rtplan.dcm dicompyler rtplan.dcm \
    d518fc976a225cbf05f8747d0067b52e7b1faa147da8e53b2b0bce01eaa21977 305836 BSD
}

src_tcia() {         # TCIA Pancreatic-CT-CBCT-SEG, Pancreas-CT-CB_027 (CC BY 4.0)
  local lic='CC BY 4.0 (TCIA Pancreatic-CT-CBCT-SEG, doi:10.7937/TCIA.ESHQ-4D90)'
  local p=1.3.6.1.4.1.14519.5.2.1
  tcia_series tcia/rtdose $p.337132488476568438964794321553967230469 46236a5ce645c81109401ca168d772128d4cb80101d79298402a8d7a48ea1082 "$lic"
  tcia_series tcia/rtstruct-1 $p.37321107941673873577248410871552238163 dd111783393344060998643da2a8f136d623eb0d4a7f70eda33caf4830881555 "$lic"
  tcia_series tcia/rtstruct-2 $p.331979158525040947095894924383156196531 41b6e16902f891883ec36925107e364396b0fcca613f7c9c5104b4f01f32f7ef "$lic"
}

src_oracle() {       # dicompyler-core and pydicom over the corpus
  "$PYTHON" -I "$GEN/dicomrt_oracle.py" --corpus "$OUT" > "$OUT/oracle.tsv.part"
  mv "$OUT/oracle.tsv.part" "$OUT/oracle.tsv"
}

main() {
  if (( $# == 0 )); then
    set -- src_pydicom src_highdicom src_dcmqi src_dicompyler src_tcia src_oracle
  fi
  : > "$OUT/.provenance.tsv"
  for s in "$@"; do
    declare -F "$s" > /dev/null || die "no section $s"
    "$s"
  done
  echo "fetch-dicomrt: done ($OUT)"
}

main "$@"
