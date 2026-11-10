#!/usr/bin/env bash
# Corpus for the fidelity round 4 runner (crates/specular-convert/tests/fidelity.rs):
# the generated option matrices (tools/corpus/fidelity-oracle.py gen) plus public suites:
# PngSuite, the OpenEXR test images, the AV1 AVIF test files,
# libheif's examples and Nokia's HEIF content. Into $SPECULAR_FIDELITY_CORPUS (default
# $CARGO_TARGET_DIR/corpus/fidelity4), gitignored, about 120 MB.
#
#   tools/corpus/fetch-fidelity.sh [gen] [png] [exr] [avif] [heic]   # default: all
#
# Round 5 (JPEG, WebP, GIF, BMP, JPEG 2000, PSD/PSB, Radiance HDR and PFM, Cineon) goes into
# $CARGO_TARGET_DIR/corpus/fidelity5 (about 40 MB): the generated matrices plus
# libjpeg-turbo's test images, libwebp's test vectors, the pygif GIF test suite (with its
# expected frames), BMP Suite 2.8 (good and questionable files, SHA-256 pinned), the JPEG 2000
# conformance codestreams (ITU-T T.803, from openjpeg-data), psd-tools' test files and
# ag-psd's read tests (about 30 MB more):
#
#   tools/corpus/fetch-fidelity.sh --round 5 [gen5] [jpeg] [webp] [gif] [bmp] [jp2] [psd] [psdag]
#
# JPEG XL conformance has its own runner with libjxl's reference decodes (specular-jxl
# tests/conformance.rs); here JPEG XL is the generated matrix.
#
# PY (default $CARGO_TARGET_DIR/venv/bin/python) needs numpy, pillow, tifffile, imagecodecs,
# pillow-heif and OpenEXR; ImageMagick and FFmpeg write the DPX matrix. Downloads are
# untrusted data: this script only copies files, never runs anything from them.
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
TARGET=${CARGO_TARGET_DIR:-$ROOT/target}
ROUND=4
if [[ ${1:-} == --round ]]; then
  ROUND=$2
  shift 2
fi
OUT=${SPECULAR_FIDELITY_CORPUS:-$TARGET/corpus/fidelity$ROUND}
PY=${PY:-$TARGET/venv/bin/python}
SCRATCH=$TARGET/corpus/.fidelity-scratch
MIN_FREE_GB=${MIN_FREE_GB:-8}
export GIT_LFS_SKIP_SMUDGE=1 GIT_TERMINAL_PROMPT=0
mkdir -p "$OUT" "$SCRATCH"

space() {
  local kb
  kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  (( kb >= MIN_FREE_GB * 1024 * 1024 )) || { echo "fetch-fidelity: under ${MIN_FREE_GB} GB free" >&2; exit 1; }
}

# fetch REPO COMMIT DEST EXT_REGEX MAX_KB PATH...: sparse depth-1 checkout at COMMIT, files
# matching EXT_REGEX and at most MAX_KB copied flat-ish to $OUT/DEST/<repo path>.
fetch() {
  local repo=$1 commit=$2 dest=$3 ext=$4 max=$5; shift 5
  local work="$SCRATCH/${repo//[\/:]/_}" n=0 url=$repo
  [[ $url == *://* ]] || url="https://github.com/$repo"
  space
  rm -rf "$work"
  git init -q "$work"
  git -C "$work" remote add origin "$url"
  git -C "$work" config core.sparseCheckout true
  git -C "$work" sparse-checkout set --no-cone "$@"
  git -C "$work" fetch -q --depth 1 --filter=blob:none origin "$commit"
  git -C "$work" -c advice.detachedHead=false checkout -q FETCH_HEAD
  while IFS= read -r -d '' f; do
    head -c 40 "$f" | grep -q '^version https://git-lfs' && continue
    local rel=${f#"$work"/}
    mkdir -p "$OUT/$dest/$(dirname "$rel")"
    cp "$f" "$OUT/$dest/$rel"
    n=$((n + 1))
  done < <(find "$work" -path "$work/.git" -prune -o -type f -size -"${max}"k -regextype posix-extended \
             -iregex ".*\.($ext)" -print0)
  rm -rf "$work"
  echo "  $repo: $n files -> $dest"
}

want() { [[ $# -eq 0 ]] || [[ " $* " == *" $SEL "* ]]; }
run() {
  local SEL=$1; shift
  want "$@"
}

# zipget URL SHA256 DEST PATH_REGEX: a pinned archive; members matching PATH_REGEX are copied
# under their archive path to $OUT/DEST (Python's zipfile only reads them).
zipget() {
  local url=$1 sum=$2 dest=$3 re=$4 z="$SCRATCH/archive.zip"
  space
  curl -fSL --retry 3 --progress-bar -o "$z" "$url"
  echo "$sum  $z" | sha256sum -c --quiet - || { echo "fetch-fidelity: $url checksum differs" >&2; exit 1; }
  python3 -I -c '
import os, re, sys, zipfile
z, out, rx = sys.argv[1], sys.argv[2], re.compile(sys.argv[3])
n = 0
with zipfile.ZipFile(z) as f:
    for m in f.infolist():
        name = m.filename
        if m.is_dir() or not rx.search(name) or ".." in name or name.startswith("/"):
            continue
        dst = os.path.join(out, name)
        os.makedirs(os.path.dirname(dst), exist_ok=True)
        with f.open(m) as src, open(dst, "wb") as o:
            o.write(src.read())
        n += 1
print(f"  {n} files -> {out}")
' "$z" "$OUT/$dest" "$re"
  rm -f "$z"
}

if [[ $ROUND == 5 ]]; then
  if run gen5 "$@"; then
    echo "generating round 5 option matrices"
    "$PY" -I "$ROOT/tools/corpus/fidelity-oracle.py" gen5 "$OUT"
  fi
  if run jpeg "$@"; then
    fetch libjpeg-turbo/libjpeg-turbo 36956793b60deb2d1169d1432be6d19871f5ecb1 jpeg/libjpeg-turbo jpg 2000 \
      '/testimages/'
  fi
  if run webp "$@"; then
    fetch https://chromium.googlesource.com/webm/libwebp-test-data 06ddd96e276c2c638a72d39d3c0f340afd61978c \
      webp/libwebp-test-data webp 3000 '/*.webp'
  fi
  if run gif "$@"; then
    fetch robert-ancell/pygif 741b282047e0dde112e5ac2df0c7f8f923cb5df6 gif/pygif 'gif|rgba|conf' 3000 '/test-suite/'
  fi
  if run bmp "$@"; then
    zipget https://entropymine.com/jason/bmpsuite/releases/bmpsuite-2.8.zip \
      f1b1f18c4f310b76bf317378e64f25b84f875a3d0976fab33ff10befdb35d13f bmp/bmpsuite '/(g|q)/[^/]+\.bmp$|/html/[^/]+\.(png|html)$'
  fi
  if run jp2 "$@"; then
    fetch uclouvain/openjpeg-data 39524bd3a601d90ed8e0177559400d23945f96a9 jp2/openjpeg-data 'j2k|j2c|jp2' 3000 \
      '/input/conformance/'
  fi
  if run psd "$@"; then
    fetch psd-tools/psd-tools a1fcb7d38d688ce39db54cb241f27af4a2e3a001 psd/psd-tools 'psd|psb' 2000 \
      '/tests/psd_files/*.psd' '/tests/psd_files/*.psb' '/tests/psd_files/colormodes/' \
      '/tests/psd_files/colorprofiles/' '/tests/psd_files/layers-minimal/' \
      '/tests/psd_files/transparency/' '/tests/psd_files/third-party-psds/'
  fi
  if run psdag "$@"; then
    # ag-psd's read tests (MIT): Photoshop files with adjustment layers, layer effects,
    # vector strokes and artboards, each with Photoshop's merged image.
    fetch Agamnentzar/ag-psd a74425db6584cc07b1cc135e370916cb4eccfb32 psd/ag-psd 'psd|psb' 3000 \
      '/test/read/*/src.psd' '/test/read/*/src.psb' '/test/read-write/*/src.psd'
  fi
  rm -rf "$SCRATCH"
  du -sh "$OUT"
  exit 0
fi

if run gen "$@"; then
  echo "generating option matrices"
  "$PY" -I "$ROOT/tools/corpus/fidelity-oracle.py" gen "$OUT"
fi
if run png "$@"; then
  fetch image-rs/image-png b2f10c6a7e3127d4aaf231f3eae731a33e6eb0bc png/pngsuite png 512 \
    '/tests/pngsuite/' '/tests/iccp/' '/tests/sbit/'
fi
if run exr "$@"; then
  fetch AcademySoftwareFoundation/openexr-images e38ffb0790f62f05a6f083a6fa4cac150b3b7452 \
    exr/openexr-images exr 6000 '/ScanLines/' '/Tiles/' '/TestImages/' '/Chromaticities/' \
    '/DisplayWindow/' '/Beachball/singlepart.0001.exr' '/v2/LowResLeftView/'
  # The luminance-chroma reference: OpenEXR's RGBA interface against the system library
  # (ImageMagick's dependency, OpenEXR 3.1); skipped where it is not installed.
  mkdir -p "$TARGET/bin"
  if c++ -O2 -o "$TARGET/bin/exr-rgba-oracle" "$ROOT/tools/corpus/exr-rgba-oracle.cc" \
      -l:libOpenEXR-3_1.so.30; then
    echo "  exr-rgba-oracle -> $TARGET/bin"
  else
    echo "  exr-rgba-oracle: no OpenEXR 3.1 library; luminance-chroma files stay without a reference" >&2
  fi
fi
if run avif "$@"; then
  fetch AOMediaCodec/av1-avif bf4c18d1f3971069b75e87d6ee469790589f4f09 avif/av1-avif 'avif' 3000 \
    '/testFiles/'
fi
if run heic "$@"; then
  fetch strukturag/libheif bb5e04a1c93d5e421e0936374fc11e01930f6768 heic/libheif 'heic|heif' 3000 \
    '/examples/'
  fetch nokiatech/heif 897fdf20ec788a406c3e7dee5225caf9e0dcf79a heic/nokia 'heic' 3000 '/content/images/'
fi
rm -rf "$SCRATCH"
du -sh "$OUT"
