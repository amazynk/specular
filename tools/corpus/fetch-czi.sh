#!/usr/bin/env bash
# Fetch the Zeiss CZI corpus into tools/corpus/czi/ (gitignored: nothing here is committed),
# optionally build libCZI from a pinned source tarball, then write the oracles' expected
# values beside the corpus:
#   czi/expected.tsv  tools/corpus/czi-oracle.py (pylibCZIrw, OpenSlide, the metadata XML,
#                     libCZI's CZIcmd): read by crates/specular-decode-host/tests/czi.rs and
#                     crates/specular-decode/tests/czi.rs
# Both tests skip when the corpus is absent (and when the host has no libCZI bridge).
#
# Usage: tools/corpus/fetch-czi.sh [SECTION...]
#   SECTION is a function name below (src_bioio, src_aics, src_openslide, libczi, oracles).
#   Default: the three sources and oracles.
#   OUT defaults to $SPECULAR_CZI_CORPUS, else tools/corpus/czi.
#   PY is the oracle interpreter (pylibCZIrw 6.1, numpy, optionally openslide-python with
#   openslide-bin 4.0), run with -I. CZICMD is libCZI's CZIcmd (attachments; the `libczi`
#   section builds one).
#   `libczi` builds libCZI (ZEISS/libczi at a pinned commit; the project tags no releases)
#   into LIBCZI_PREFIX (default target/libczi/install): shared library, headers and CZIcmd,
#   with the system's Eigen3 and a private zstd. Then build Specular with
#   SPECULAR_LIBCZI_DIR=$LIBCZI_PREFIX so the decode host gets its libCZI bridge. Needs cmake,
#   ninja and a C++17 compiler; downloads zstd and RapidJSON (CZIcmd) at configure time.
#
# About 215 MB downloaded:
#   bioio-czi test resources (GitHub LFS, BSD-3-Clause repository): single and multi-scene,
#     mosaic with T/Z/C, BGR24, a gray16 overview scan with pyramid and slide preview
#   aicspylibczi test resources (GPL-3.0 repository, files used as data only): 3 scenes with a
#     pyramid, a two-tile mosaic
#   OpenSlide test data (Zeiss/, CC0-1.0, Venklab UTHSCSA): a JPEG XR brightfield slide with
#     two scenes, five pyramid layers, label and slide preview; its BGR48 slide preview with
#     JPEG XR, zstd0 and zstd1 (HiLo) sub-blocks
#
# Every download is pinned by size and sha256. Downloads are untrusted data: this script only
# copies files, never runs anything from them, and the oracles run with python -I. Stops if
# free space on the target filesystem drops below MIN_FREE_GB (default 8).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_CZI_CORPUS:-$ROOT/tools/corpus/czi}
PY=${PY:-python3}
CZICMD=${CZICMD:-}
LIBCZI_PREFIX=${LIBCZI_PREFIX:-$ROOT/target/libczi/install}
MIN_FREE_GB=${MIN_FREE_GB:-8}
LFS=https://media.githubusercontent.com/media/bioio-devs/bioio-czi/main/bioio_czi/tests/resources
AICS=https://raw.githubusercontent.com/bioio-devs/aicspylibczi/main/aicspylibczi/tests/resources
OS=https://openslide.cs.cmu.edu/download/openslide-testdata/Zeiss
# libCZI 0.70.0 (ZEISS/libczi main, 2026-10-12): the source archive GitHub serves for it.
LIBCZI_COMMIT=a1f2751521677e4080795032d4433ac6e31dc7c8
LIBCZI_SHA256=f66a1c387d076bf3da70f973a660e71f3ee5a117dc08ab36f47657f9b20bdb56
mkdir -p "$OUT"

die() { echo "fetch-czi: $*" >&2; exit 1; }

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

# pinned URL NAME SIZE SHA256 LICENCE: downloads once.
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

src_bioio() {
  echo "bioio-czi"
  local lic="bioio-czi test resources (BSD-3-Clause repository)"
  pinned "$LFS/s_1_t_1_c_1_z_1.czi" s_1_t_1_c_1_z_1.czi 1000800 \
    aeb4e582a1ad39fd10bf4ee1811ee8f7aafcf9d6aa3c9ca508ba71defd461b02 "$lic"
  pinned "$LFS/s_3_t_1_c_3_z_5.czi" s_3_t_1_c_3_z_5.czi 14673984 \
    9f6a1a154385de9c5655f14df997d2d2460a7655714cc7248d11160331429c27 "$lic"
  pinned "$LFS/RGB-8bit.czi" RGB-8bit.czi 1749952 \
    44593e6210f2f9066f8608c53c31806f4d173d1d26bb3bb5c32b182fb0c0a43e "$lic"
  pinned "$LFS/RGB-8bit-with-non-xy-dims.czi" RGB-8bit-with-non-xy-dims.czi 1731744 \
    53f7448aef78ce58bf29d34b2e364df7fdaef8ba034c3624af3a38f68dda2293 "$lic"
  pinned "$LFS/S%3D2_4x2_T%3D2%3DZ%3D3_CH%3D2.czi" "S=2_4x2_T=2=Z=3_CH=2.czi" 31927744 \
    b9a25666e495d73299b706f503c20e5f914bec3c62185f10c73ac716553e7a7d "$lic"
  pinned "$LFS/OverViewScan.czi" OverViewScan.czi 82106592 \
    bff62579396bce3e702abb58aa59c68112ecc970136a4406d548c80d3102143f "$lic"
}

src_aics() {
  echo "aicspylibczi"
  local lic="aicspylibczi test resources (GPL-3.0 repository; data only)"
  pinned "$AICS/Multiscene_CZI_3Scenes.czi" Multiscene_CZI_3Scenes.czi 6113888 \
    92869e25e363e8dd426113a46e85c9b9760f8e53ca8c0d06a7421f3f42fb421f "$lic"
  pinned "$AICS/mosaic_test.czi" mosaic_test.czi 2791008 \
    0f9287bfc0e6bdf701186cd2408423fe9dc40b4a44af3e066159c85806f5034a "$lic"
}

src_openslide() {
  echo "OpenSlide"
  local lic="OpenSlide test data, CC0-1.0 (Venklab, Pathology, UTHSCSA)"
  pinned "$OS/Zeiss-5-JXR.czi" Zeiss-5-JXR.czi 68811968 \
    c202ddf7b0bd473cdbe29977aee07c10c207077779485c0b1f876e8c00da77f7 "$lic"
  pinned "$OS/Zeiss-5-SlidePreview-JXR.czi" Zeiss-5-SlidePreview-JXR.czi 1767968 \
    f15d2ac93d97516feb44d072710e1ede49a8fe13e5ff4b81e661f14757906062 "$lic"
  pinned "$OS/Zeiss-5-SlidePreview-Zstd0.czi" Zeiss-5-SlidePreview-Zstd0.czi 1955488 \
    920c10f45b781591d7a46759e692e18e14278c2d41b069b518f81fda0748dd22 "$lic"
  pinned "$OS/Zeiss-5-SlidePreview-Zstd1-HiLo.czi" Zeiss-5-SlidePreview-Zstd1-HiLo.czi 1885344 \
    fd5cea806a557954e2ff63c2eb0dd23c83fe05d4900aac9d81a4af2a412288b0 "$lic"
}

libczi() {
  echo "libCZI $LIBCZI_COMMIT"
  local base src build
  base=$(dirname "$LIBCZI_PREFIX")
  src=$base/src
  build=$base/build
  mkdir -p "$base/dl" "$src" "$build"
  local tarball=$base/dl/libczi-$LIBCZI_COMMIT.tar.gz
  if [[ ! -f $tarball ]] || [[ $(sha256sum "$tarball" | cut -d' ' -f1) != "$LIBCZI_SHA256" ]]; then
    check_space
    curl -fsSL --retry 3 -o "$tarball.part" \
      "https://codeload.github.com/ZEISS/libczi/tar.gz/$LIBCZI_COMMIT"
    [[ $(sha256sum "$tarball.part" | cut -d' ' -f1) == "$LIBCZI_SHA256" ]] \
      || { rm -f "$tarball.part"; die "libCZI tarball: sha256 mismatch"; }
    mv "$tarball.part" "$tarball"
  fi
  tar -xzf "$tarball" -C "$src"
  cmake -G Ninja -S "$src/libczi-$LIBCZI_COMMIT" -B "$build" -DCMAKE_BUILD_TYPE=Release \
    -DLIBCZI_BUILD_UNITTESTS=OFF -DLIBCZI_BUILD_CZICMD=ON -DLIBCZI_BUILD_DYNLIB=ON \
    -DLIBCZI_ENABLE_INSTALL=ON -DLIBCZI_BUILD_PREFER_EXTERNALPACKAGE_EIGEN3=ON \
    -DLIBCZI_BUILD_PREFER_EXTERNALPACKAGE_RAPIDJSON=OFF -DCMAKE_INSTALL_PREFIX="$LIBCZI_PREFIX"
  cmake --build "$build" -j "${JOBS:-4}"
  cmake --install "$build"
  CZICMD=$LIBCZI_PREFIX/bin/CZIcmd
  echo "  libCZI in $LIBCZI_PREFIX (build with SPECULAR_LIBCZI_DIR=$LIBCZI_PREFIX)"
}

oracles() {
  echo "oracles"
  if [[ -z $CZICMD && -x $LIBCZI_PREFIX/bin/CZIcmd ]]; then
    CZICMD=$LIBCZI_PREFIX/bin/CZIcmd
  fi
  "$PY" -I "$ROOT/tools/corpus/czi-oracle.py" "$OUT" ${CZICMD:+"$CZICMD"}
}

sections=("$@")
if (( ${#sections[@]} == 0 )); then
  sections=(src_bioio src_aics src_openslide oracles)
fi
for s in "${sections[@]}"; do
  declare -F "$s" > /dev/null || die "no section $s"
  "$s"
done
echo "done: $(du -sh "$OUT" | cut -f1) in $OUT"
