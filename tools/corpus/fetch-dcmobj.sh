#!/usr/bin/env bash
# Fetch and generate the DICOM video and encapsulated-document corpus into tools/corpus/dcmobj/
# (gitignored: nothing here is committed), and run its oracle.
#
# Usage: tools/corpus/fetch-dcmobj.sh [SECTION...]
#   SECTION is a function name below (src_fodicom, src_ohif, src_gen, src_oracle). Default: all
#   four in that order. OUT defaults to $SPECULAR_DCMOBJ_CORPUS, else tools/corpus/dcmobj.
#   PYTHON (default python3) must have pydicom, numpy and av (PyAV); ffmpeg with libx264,
#   libx265, mpeg2video and mpeg4 must be on PATH for src_gen. A venv under target/ is fine:
#   python3 -m venv target/dcmobj/venv &&
#   target/dcmobj/venv/bin/pip install --no-cache-dir pydicom numpy av;
#   then PYTHON=target/dcmobj/venv/bin/python.
#   VIDEO_FRAMES is Specular's decoder for the oracle, default
#   target/release/examples/video_frames (cargo build --release -p specular-dicom
#   --example video_frames).
#
# What it covers (about 65 MB):
#   fodicom   fo-dicom's ETIAM_video_002.dcm (MS-PL): MPEG-2 Main, 352x288, 450 frames (refused)
#   ohif      OHIF viewer-testdata (MIT): H.264 BD-compatible 1920x1080, 3360 frames at 16.17 ms,
#             stored as an MP4 file in the pixel data (not an elementary stream)
#   gen       generated with ffmpeg + pydicom (crates/formats/specular-dicom/gen/dcmobj_make.py):
#             1080p30 H.264 High with B-frames, 720p in 8 KiB fragments (.102.1, open GOP),
#             576i MBAFF, H.264 in MP4, HEVC Main and Main 10, MPEG-2 and MPEG-4 Part 2
#             (refused), an encapsulated PDF and CDA
#   oracle.tsv  PyAV (FFmpeg) against Specular: frame count, and frames within 2 levels
#
# Downloads are untrusted data: each source gets its own new directory, every file is checked
# against its size and SHA-256 below, and nothing from them is run (the oracle reads them with
# python -I from this repository's scripts). Stops if free space drops below MIN_FREE_GB
# (default 6).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_DCMOBJ_CORPUS:-$ROOT/tools/corpus/dcmobj}
MIN_FREE_GB=${MIN_FREE_GB:-6}
PYTHON=${PYTHON:-python3}
VIDEO_FRAMES=${VIDEO_FRAMES:-$ROOT/target/release/examples/video_frames}
GEN=$ROOT/crates/formats/specular-dicom/gen
mkdir -p "$OUT"

die() { echo "fetch-dcmobj: $*" >&2; exit 1; }

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

src_fodicom() {      # fo-dicom test data at f45aedd (MS-PL)
  fetch "https://raw.githubusercontent.com/fo-dicom/fo-dicom/f45aedda1e2489f37618364bd61c76885b8d8504/Tests/FO-DICOM.Tests/Test%20Data/ETIAM_video_002.dcm" \
    fodicom ETIAM_video_002.dcm \
    c69e83192ab81687bc4ca1163ca139bab3d37e6cec10ed59851c3a24241ca8d4 9073558 MS-PL
}

src_ohif() {         # OHIF viewer-testdata at d8416ad (MIT)
  fetch "https://raw.githubusercontent.com/OHIF/viewer-testdata/d8416adf6478a583a2147f22a7b47b7406544dd8/dcm/mixedVideo/video/OT.1.3.6.1.4.1.5962.99.3.434174349.6916.500660925.201.dcm" \
    ohif h264_1080p_mp4_3360.dcm \
    c2f5704d51e1d080c7edb8ac3e71801c529f34d50cbb13f3e7a4683545d891bb 47364694 MIT
}

src_gen() {          # ffmpeg + pydicom, generated here
  check_space
  "$PYTHON" -I "$GEN/dcmobj_make.py" "$OUT/gen"
  provenance gen "crates/formats/specular-dicom/gen/dcmobj_make.py" generated
}

src_oracle() {       # PyAV against Specular over every video file
  [[ -x $VIDEO_FRAMES ]] || die "no $VIDEO_FRAMES (cargo build --release -p specular-dicom --example video_frames)"
  "$PYTHON" -I "$GEN/dcmobj_oracle.py" "$VIDEO_FRAMES" "$OUT/oracle.tsv" "$OUT"
}

main() {
  if (( $# == 0 )); then
    set -- src_fodicom src_ohif src_gen src_oracle
  fi
  : > "$OUT/.provenance.tsv"
  for s in "$@"; do
    declare -F "$s" > /dev/null || die "no section $s"
    "$s"
  done
  echo "fetch-dcmobj: done ($OUT)"
}

main "$@"
