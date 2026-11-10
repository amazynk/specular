#!/usr/bin/env bash
# Fetch the public DICOM test corpus into tools/corpus/dicom/ (gitignored: nothing downloaded
# here is committed). The specular-dicom and specular-decode tests read it in place from
# SPECULAR_DICOM_CORPUS (a worktree has no corpus of its own), else from tools/corpus/dicom, and
# skip when it is absent.
#
# Usage: tools/corpus/fetch-dicom.sh [SECTION...]
#   SECTION is a function name below (e.g. src_pydicom_data). Default: all sections.
#   OUT defaults to $SPECULAR_DICOM_CORPUS, else tools/corpus/dicom.
#
# What it covers (about 140 MB): implicit / explicit VR little endian, explicit big endian,
# deflate, RLE, JPEG baseline / extended (12-bit) / lossless (SV1 and other predictors),
# JPEG-LS, JPEG 2000 (lossless and lossy), HTJ2K, palette colour, YBR_FULL / YBR_FULL_422,
# multi-frame (cine, 3D colour), Modality and VOI LUT sequences, big CR / mammography (WG04
# RG1-3, MG1), CT 512x512 (693), US, and the IHE MESA CPI grayscale softcopy presentation state
# tests (GSPS .pre files next to their images); SEG (pydicom-data, highdicom, dcmqi) and a
# TCIA PET/CT with RTSTRUCTs (about 150 MB more). Sources and licences: .provenance.tsv.
#
# Downloads are untrusted data: this script only copies files, never runs anything from them.
# Scratch space is target/agent-corpus/dicom-fetch (deleted afterwards). Stops if free space
# on the target filesystem drops below MIN_FREE_GB (default 6).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_DICOM_CORPUS:-$ROOT/tools/corpus/dicom}
SCRATCH="$ROOT/target/agent-corpus/dicom-fetch"
MIN_FREE_GB=${MIN_FREE_GB:-6}
export GIT_LFS_SKIP_SMUDGE=1 GIT_TERMINAL_PROMPT=0
mkdir -p "$OUT" "$SCRATCH"

die() { echo "fetch-dicom: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

# provenance RELPATH URL LICENCE
provenance() { printf '%s\t%s\t%s\n' "$1" "$2" "$3" >> "$OUT/.provenance.tsv"; }

# git_files OWNER/REPO COMMIT DEST LICENCE PATH...
# Sparse, blobless, depth-1 fetch of PATH... at COMMIT; copies every file except Python
# sources and READMEs to $OUT/DEST/<repo path>. Git LFS pointers are resolved via
# media.githubusercontent.com.
git_files() {
  local repo=$1 commit=$2 dest=$3 lic=$4; shift 4
  local work="$SCRATCH/${repo//\//_}"
  check_space
  rm -rf "$work"
  git init -q "$work"
  git -C "$work" remote add origin "https://github.com/$repo"
  git -C "$work" config core.sparseCheckout true
  git -C "$work" sparse-checkout set --no-cone "$@"
  git -C "$work" fetch -q --depth 1 --filter=blob:none origin "$commit"
  git -C "$work" -c advice.detachedHead=false checkout -q FETCH_HEAD
  local n=0
  while IFS= read -r -d '' f; do
    local rel=${f#"$work"/}
    mkdir -p "$OUT/$dest/$(dirname "$rel")"
    if head -c 40 "$f" | grep -q '^version https://git-lfs'; then
      curl -fsSL --retry 3 -o "$OUT/$dest/$rel" \
        "https://media.githubusercontent.com/media/$repo/$commit/$rel"
    else
      cp "$f" "$OUT/$dest/$rel"
    fi
    provenance "$dest/$rel" "https://github.com/$repo/blob/$commit/$rel" "$lic"
    n=$((n + 1))
  done < <(find "$work" -path "$work/.git" -prune -o -type f \
             ! -name '*.py' ! -name '*.rst' ! -name '*.md' ! -name '*.json' ! -name '*.dump' \
             ! -name '*.icc' ! -name '*.gz' ! -name 'README*' -print0)
  rm -rf "$work"
  echo "  $repo: $n files -> $dest"
}

# ------------------------------------------------------------------------------- sections

src_pydicom() {      # pydicom's own small test files (incl. DICOMDIR trees, deflate, big endian)
  git_files pydicom/pydicom 60cebd027861e0dd4dfac12431679cb49e0156c3 pydicom \
    'MIT (pydicom); test files from various public sources' '/src/pydicom/data/test_files/'
}
src_pydicom_charset() { # pydicom's Specific Character Set files (PS3.5 Annex H-K, all sets)
  git_files pydicom/pydicom 60cebd027861e0dd4dfac12431679cb49e0156c3 pydicom \
    'MIT (pydicom); character set examples from DICOM PS3.5 and dclunie' \
    '/src/pydicom/data/charset_files/'
}
src_pydicom_data() { # pydicom-data: WG04 RG1/RG3/US1/MR2 in every syntax, HTJ2K, JPEG-LS, LUTs
  git_files pydicom/pydicom-data abc42b90985fb6cf385aa4af766d2c9c94a257a4 pydicom-data \
    'MIT (pydicom-data); NEMA WG04 and other public samples' '/data_store/data/'
}
src_pylibjpeg_data() { # pylibjpeg-data: per transfer syntax sets (JPEG ext/lossless, MG1, RG1/2)
  git_files pydicom/pylibjpeg-data 8253566715800a7fc3d4d949abab102c8172bca0 pylibjpeg-data \
    'MIT (pylibjpeg-data); NEMA WG04 and other public samples' '/ljdata/ds/'
}

src_mesa_cpi() {     # IHE MESA CPI display tests by OFFIS: images + GSPS (.pre) + results (.res)
  # Grayscale Softcopy Presentation States exercising Modality/VOI/Presentation LUTs,
  # spatial transformation, displayed area, shutters, graphic and text annotations.
  local url=https://dcmtk.org/download/mesa_cpi/release_2004/imgdisplay_testcases.zip
  local md5=8184e7d85f665e8ba22f6c6628226d3a work="$SCRATCH/mesa_cpi"
  check_space
  rm -rf "$work"; mkdir -p "$work"
  curl -fsSL --retry 3 -o "$work/t.zip" "$url"
  [[ $(md5sum "$work/t.zip" | cut -d' ' -f1) == "$md5" ]] || die "mesa_cpi: md5 mismatch"
  unzip -q "$work/t.zip" -d "$work/x"
  mkdir -p "$OUT/mesa-cpi"
  local n=0 f
  while IFS= read -r -d '' f; do
    cp "$f" "$OUT/mesa-cpi/"
    provenance "mesa-cpi/$(basename "$f")" "$url" \
      'IHE MESA CPI test data (OFFIS, freely distributed for conformance testing)'
    n=$((n + 1))
  done < <(find "$work/x" -type f \( -name '*.dcm' -o -name '*.pre' -o -name '*.res' \) -print0)
  rm -rf "$work"
  echo "  mesa_cpi: $n files -> mesa-cpi"
}

src_highdicom() {   # highdicom: SEG binary / fractional / overlapping over one CT image
  git_files ImagingDataCommons/highdicom 847fea5946b4be9cf0c9939b8f9dcbf0bc60b24d highdicom \
    'MIT (highdicom)' '/data/test_files/ct_image.dcm' '/data/test_files/seg_image_ct_*.dcm'
}
src_dcmqi() {       # dcmqi: the three CT slices pydicom-data's liver SEG refers to, more SEGs
  git_files QIICR/dcmqi 5571494a14416ad951a0979d2329cf15b8bba019 dcmqi \
    'BSD-3-Clause (dcmqi)' '/data/segmentations/ct-3slice/' '/data/segmentations/*.dcm'
}

# tcia_series DEST SERIES_UID SHA256 LICENCE
# One public TCIA series through the NBIA REST API (no login for public collections). The zip
# is built on request, so the pin is the SHA-256 over the sorted SHA-256s of the files in it.
tcia_series() {
  local dest=$1 uid=$2 want=$3 lic=$4 work="$SCRATCH/tcia_$2"
  local url="https://services.cancerimagingarchive.net/nbia-api/services/v1/getImage?SeriesInstanceUID=$uid"
  check_space
  rm -rf "$work"; mkdir -p "$work/x"
  curl -fsSL --retry 3 -o "$work/s.zip" "$url"
  unzip -q "$work/s.zip" -d "$work/x"
  local got
  got=$(find "$work/x" -type f -name '*.dcm' -exec sha256sum {} + | cut -d' ' -f1 | sort \
        | sha256sum | cut -d' ' -f1)
  [[ $got == "$want" ]] || die "tcia $uid: content hash $got, want $want"
  mkdir -p "$OUT/$dest"
  local n=0 f
  while IFS= read -r -d '' f; do
    cp "$f" "$OUT/$dest/"
    provenance "$dest/$(basename "$f")" "$url" "$lic"
    n=$((n + 1))
  done < <(find "$work/x" -type f -name '*.dcm' -print0)
  rm -rf "$work"
  echo "  tcia $uid: $n files -> $dest"
}
src_tcia_sts() {    # TCIA Soft-tissue-Sarcoma STS_010: PET/CT sharing a Frame of Reference + RTSTRUCTs
  # GE Discovery ST, decay-corrected BQML PET (SUV, fusion), its CT (150 MB) and the
  # contours drawn on each. Licence CC BY 3.0, doi:10.7937/K9/TCIA.2015.7GO2GSKS.
  local lic='CC BY 3.0 (TCIA Soft-tissue-Sarcoma, doi:10.7937/K9/TCIA.2015.7GO2GSKS)'
  local p=1.3.6.1.4.1.14519.5.2.1.5168.1900
  tcia_series tcia-sts/pt  $p.120827366891563757522579924917 9b0ed54461ca1a1df43238975ed3cb74e70b39408b52b77aac8c2e402a15a7c0 "$lic"
  tcia_series tcia-sts/ct  $p.197754415891602187397505258429 a2907a5c7ab22cf43c254e4a021b213b83f927dfef4530899fa0c75b1005cf04 "$lic"
  tcia_series tcia-sts/rtss-pt $p.315084889191112753248709818643 d7ae8b0499eefb413fd3e069f92d8693d3dc8d5b0b35aca752aba7f0439655ed "$lic"
  tcia_series tcia-sts/rtss-ct $p.133037479497893229915237733054 4949225396f5e1bdc5c870cd9b9b6278fe467c705af89e6aba6709ba81908a3c "$lic"
}

main() {
  local sections=() s
  for a in "$@"; do
    case $a in
      -h|--help) sed -n '2,20p' "$0"; exit 0 ;;
      *) sections+=("$a") ;;
    esac
  done
  if (( ${#sections[@]} == 0 )); then
    sections+=($(declare -F | awk '{print $3}' | grep '^src_'))
  fi
  for s in "${sections[@]}"; do
    declare -F "$s" >/dev/null || die "unknown section $s"
    echo "== $s"
    "$s"
  done
  # Re-runs can repeat entries; keep the last per path.
  if [[ -f $OUT/.provenance.tsv ]]; then
    tac "$OUT/.provenance.tsv" | awk -F'\t' '!seen[$1]++' | tac > "$OUT/.provenance.tmp"
    mv "$OUT/.provenance.tmp" "$OUT/.provenance.tsv"
  fi
  rm -rf "$SCRATCH"
  du -sh "$OUT"
  df -h "$OUT" | tail -1
}
main "$@"
