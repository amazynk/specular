#!/usr/bin/env bash
# Fetch the public conformance / feature-coverage corpus (tools/corpus/conformance/) and the
# large performance files (tools/corpus/large/). Both directories are gitignored: nothing
# downloaded here is committed. tools/corpus/conformance/MANIFEST.md lists every file with
# its source and SHA-256.
#
# Usage: tools/corpus/fetch-conformance.sh [--large|--no-large] [SECTION...]
#   SECTION is a function name below (e.g. src_pngsuite, large_raw). Default: all sections.
#   --no-large skips the large_* sections (RAW ~0.7 GB, Adobe-profile source RAW ~1.5 GB, large
#   files ~0.6 GB, JPEG XL conformance references ~0.6 GB); --large runs only those.
#
# Downloads are untrusted data: this script only copies files, never runs anything from them.
# Scratch space is target/corpus/ (deleted afterwards). Stops if free space on the
# target filesystem drops below MIN_FREE_GB (default 6).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
CONF="$ROOT/tools/corpus/conformance"
LARGE="$ROOT/tools/corpus/large"
SCRATCH="$ROOT/target/corpus/fetch"
MIN_FREE_GB=${MIN_FREE_GB:-6}
export GIT_LFS_SKIP_SMUDGE=1 GIT_TERMINAL_PROMPT=0
mkdir -p "$CONF" "$LARGE" "$SCRATCH"

IMG_EXT='png|apng|jpg|jpeg|jpe|mpo|webp|gif|bmp|dib|ico|cur|ani|qoi|pbm|pgm|ppm|pnm|pam|pfm|tga|svg|svgz|tif|tiff|btf|dds|ktx|ktx2|basis|astc|psd|psb|ff|ras|sun|sgi|rgb|rgba|bw|fli|flc|pcx|dcx|xpm|xbm|wbmp|jxl|avif|avifs|heic|heif|hif|heics|jp2|j2k|j2c|jpx|jpf|jph|jhc|jpm|exr|hdr|pic|icns|ora|kra|xcf|blp|im|eps|fits'

die() { echo "fetch-conformance: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$ROOT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

# provenance DESTROOT RELPATH URL LICENCE
provenance() { printf '%s\t%s\t%s\n' "$2" "$3" "$4" >> "$1/.provenance.tsv"; }

# fetch_url DESTROOT RELPATH URL LICENCE [SHA256]
fetch_url() {
  local root=$1 rel=$2 url=$3 lic=$4 want=${5:-}
  local out="$root/$rel"
  mkdir -p "$(dirname "$out")"
  if [[ ! -s $out ]]; then
    check_space
    curl -fsSL --retry 3 -A 'specular-corpus-fetch/1 (test corpus)' -o "$out.part" "$url"
    mv "$out.part" "$out"
  fi
  if [[ -n $want ]]; then
    echo "$want  $out" | sha256sum -c --quiet - || die "checksum mismatch: $rel"
  fi
  provenance "$root" "$rel" "$url" "$lic"
}

# git_files OWNER/REPO COMMIT DEST LICENCE EXT_REGEX PATH...
# Sparse, blobless, depth-1 fetch of PATH... at COMMIT; copies files whose extension matches
# EXT_REGEX (case-insensitive) to $CONF/DEST/<repo path>. Git LFS pointers are resolved via
# media.githubusercontent.com.
git_files() {
  local repo=$1 commit=$2 dest=$3 lic=$4 ext=$5; shift 5
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
    mkdir -p "$CONF/$dest/$(dirname "$rel")"
    if head -c 40 "$f" | grep -q '^version https://git-lfs'; then
      curl -fsSL --retry 3 -o "$CONF/$dest/$rel" \
        "https://media.githubusercontent.com/media/$repo/$commit/$rel"
    else
      cp "$f" "$CONF/$dest/$rel"
    fi
    provenance "$CONF" "$dest/$rel" "https://github.com/$repo/blob/$commit/$rel" "$lic"
    n=$((n + 1))
  done < <(find "$work" -path "$work/.git" -prune -o -type f -regextype posix-extended \
             -iregex ".*\.($ext)" -print0)
  rm -rf "$work"
  echo "  $repo: $n files -> conformance/$dest"
}

# ---------------------------------------------------------------- conformance (small)

src_pngsuite() {   # PngSuite + APNG/iCCP/sBIT/regression files from image-png
  git_files image-rs/image-png b2f10c6a7e3127d4aaf231f3eae731a33e6eb0bc image-png \
    'MIT OR Apache-2.0; PngSuite (c) Willem van Schaik, free use' 'png|apng' \
    '/tests/pngsuite/' '/tests/pngsuite-extra/' '/tests/animated/' '/tests/iccp/' \
    '/tests/sbit/' '/tests/bugfixes/' '/tests/text_chunk_examples/'
}
src_pillow() {     # Pillow's test images: DDS, FLI, SGI, Sun raster, PCX, XBM/XPM, MPO, TGA, APNG, ...
  git_files python-pillow/Pillow 52d303ae51f8fecdd54b6fd44256e37c93665493 pillow \
    'MIT-CMU (HPND); individual images vary' "$IMG_EXT" '/Tests/images/'
}
src_sdl_image() {
  git_files libsdl-org/SDL_image f7ec8b3631f5eeb4497bf4a581547409dfaa55c4 sdl_image 'Zlib' \
    "$IMG_EXT" '/test/'
}
src_image_rs() {   # image-rs test images except PNG (PngSuite covers PNG)
  git_files image-rs/image 075e65d618a323e5cb48c1d8710a2eca25d690c6 image-rs \
    'MIT OR Apache-2.0' "${IMG_EXT/png|apng|/apng|}" '/tests/images/'
}
src_jpeg_decoder() {
  git_files image-rs/jpeg-decoder eb2d7c0f6a2d0298aba7a7f8b9ca1440353e8f8c jpeg-decoder \
    'MIT OR Apache-2.0; imagetestsuite and mozilla images keep their own terms' 'jpg|jpeg' \
    '/tests/reftest/images/' '/tests/crashtest/images/'
}
src_libjpeg_turbo() {
  git_files libjpeg-turbo/libjpeg-turbo 21be2ba777b530ead205e78dc7a57a8efddb14cb libjpeg-turbo \
    'IJG / BSD-3-Clause' 'jpg' '/testimages/'
}
src_libultrahdr() {
  git_files google/libultrahdr 66821e0a261aa3a06c0e7c889f52eced52850be1 libultrahdr \
    'Apache-2.0' 'jpg' '/tests/data/'
}
src_jxl() {
  git_files libjxl/conformance b1d0f990b03e57bf6d137c365cd5dc8b470b9191 jxl-conformance \
    'BSD-3-Clause' 'jxl|json' '/testcases/'
  git_files libjxl/testdata 73695d303670c90e4d506ea89d9901b081385089 jxl-testdata \
    'BSD-3-Clause (per-file notes in repo)' 'jxl' '/*'
}
src_avif() {
  git_files AOMediaCodec/av1-avif bf4c18d1f3971069b75e87d6ee469790589f4f09 av1-avif \
    'BSD-2-Clause / per-folder licences' 'avif|avifs' '/testFiles/'
  git_files AOMediaCodec/libavif 0061e03104b959e3aaad73f7d48e968469fff16b libavif \
    'BSD-2-Clause (tests/data/README.md lists sources)' 'avif|avifs|jpg|heic' '/tests/data/'
}
src_heif() {
  git_files strukturag/libheif bb5e04a1c93d5e421e0936374fc11e01930f6768 libheif \
    'LGPL-3.0 repository; test data unknown' 'heic|heif|hif|avif' \
    '/examples/' '/tests/data/' '/fuzzing/data/'
  git_files nokiatech/heif 897fdf20ec788a406c3e7dee5225caf9e0dcf79a nokia-heif \
    'Nokia HEIF licence (evaluation use)' 'heic|heif' '/content/'
  git_files MPEGGroup/FileFormatConformance 767a5bcb9fa4841d1b3bb0ca112a6e7d4c6bf067 mpeg-ffc \
    'MPEG conformance (see repo)' 'heic|heif|avif' '/data/file_features/published/heif/'
  git_files etemesi254/zune-image 52300d398488e907283786da5196e7f1797cfd81 zune-image \
    'MIT OR Apache-2.0 OR Zlib; images vary' 'heic|hdr|qoi|psd' \
    '/crates/zune-heic/fuzz-samples/' '/test-images/hdr/' '/test-images/psd/' '/test-images/qoi/'
  # An iPhone HEIC with Apple's HDR gain-map auxiliary image (Git LFS)
  git_files johncf/apple-hdr-heic e64716c29abc91a3b40543d7c47fb0f526608982 apple-hdr-heic \
    'MIT' 'heic' '/tests/data/'
}
src_webp() {
  git_files webmproject/libwebp-test-data 06ddd96e276c2c638a72d39d3c0f340afd61978c libwebp-test-data \
    'BSD-3-Clause' 'webp' '/*.webp'
}
src_gif() {
  git_files image-rs/image-gif a5f89ddc2b29424334244c97456b4a26e208aa7d image-gif \
    'MIT OR Apache-2.0' 'gif' '/tests/'
}
src_tiff() {       # includes libtiff pics (tests/libtiffpic) and BigTIFF samples
  git_files image-rs/image-tiff 3302936b08e2260d4c66f40f51170efbb1fded4c image-tiff \
    'MIT; libtiffpic images keep libtiff terms' 'tif|tiff|btf' '/tests/'
}
src_exr() {
  git_files AcademySoftwareFoundation/openexr-images e38ffb0790f62f05a6f083a6fa4cac150b3b7452 \
    openexr-images 'BSD-3-Clause (per-image notes)' 'exr' '/*'
}
src_jp2() {
  git_files uclouvain/openjpeg-data 39524bd3a601d90ed8e0177559400d23945f96a9 openjpeg-data \
    'BSD-2-Clause repo; ITU-T T.803 conformance streams' 'j2k|jp2|j2c|jph|jhc|jpx|jpf|jpm' '/input/'
}
# WG1 HTJ2K conformance (gitlab.com/wg1/htj2k-codestreams, ~70 MB): HT and HTMIXED transcodes of
# the Part 4 profile 0/1 streams, the HTJ2K .jph files, their syntax descriptions and the Part 4
# class 0/1 reference PGX/PGM decodes they are checked against.
src_htj2k() {
  local commit=f6b9ede094a0bd6e1e0427e12721e3f3ee1b704b rel
  local api="https://gitlab.com/api/v4/projects/wg1%2Fhtj2k-codestreams/repository/tree"
  local list="$SCRATCH/htj2k_tree.txt"
  python3 -I -c 'import json, sys, urllib.request
out, page = [], 1
while True:
    d = json.load(urllib.request.urlopen(f"{sys.argv[1]}?ref={sys.argv[2]}&per_page=100&recursive=true&page={page}"))
    if not d: break
    out += [e["path"] for e in d if e["type"] == "blob"]
    page += 1
print("\n".join(out))' "$api" "$commit" > "$list"
  while IFS= read -r rel; do
    fetch_url "$CONF" "htj2k-codestreams/$rel" \
      "https://gitlab.com/wg1/htj2k-codestreams/-/raw/$commit/$rel" \
      'ISO/IEC 15444-4 / ITU-T T.803 conformance material (see COPYRIGHT.txt)'
  done < <(grep -E '^(COPYRIGHT\.txt|README\.(txt|licence)|htj2k_bsets_profile[01]/.*\.j2k|testfiles_jph/.*\.jph|codestreams_hifi/hifi_ht1_02\.j2k|reference_class(0|1|1HF)_profile[01]/.*\.pgx|reference_jph/.*\.pgm|descriptions_htj2k_profile[01]/.*syntax\.txt|descriptions_jph/.*\.desc)$' "$list")
  echo "  wg1/htj2k-codestreams -> conformance/htj2k-codestreams"
}
# OpenJPH's decoder test streams (Kakadu-encoded .jph: code-block sizes, 5/3 and 9/7, 16-bit,
# tiles, all progressions, low-latency CAUSAL), ~10 MB; the OpenJPH-encoded .j2c files are
# regenerated locally instead.
src_openjph() {
  git_files aous72/jp2k_test_codestreams 1ccd349d12dced98eafabe641e5dac8c42a7c909 openjph-tests \
    'BSD-2-Clause' 'jph' '/openjph/simple_dec_*'
}
src_psd() {
  git_files psd-tools/psd-tools 1b8c1d404d1b976748e7dbbca33ed5898663371b psd-tools 'MIT' \
    'psd|psb' '/tests/psd_files/'
}
src_krita() {      # Krita documents (.kra): tests (layers, 16-bit, HDR, animation) and templates
  git_files KDE/krita 3ee2270d8ead36966a055ab72265b8d767d162b8 krita 'GPL-2.0-or-later' 'kra' \
    '/libs/ui/tests/data/load_test.kra' '/libs/ui/tests/data/flood_fill_16bit.kra' \
    '/plugins/impex/libkra/tests/data/*.kra' '/plugins/impex/jxl/tests/data/results/cmyk-layers.kra' \
    '/krita/data/templates/texture/Texture256x2568bitsrgb.kra' \
    '/krita/data/templates/texture/Texture1k32bitscalar.kra'
}
src_mypaint() {    # OpenRaster documents (.ora) from MyPaint's tests
  git_files mypaint/mypaint fb6f7d6b12238d125294496b105f5f36e658bc61 mypaint 'GPL-2.0-or-later' 'ora' \
    '/tests/*.ora'
}
src_twelvemonkeys() {  # IFF ILBM/PBM (HAM, EHB, deep, masks) and Apple PICT (v1/v2, QuickTime JPEG)
  git_files haraldk/TwelveMonkeys d9da033312af7461b4a99cbafe2a79f1d6d66e7b twelvemonkeys \
    'BSD-3-Clause repo; sample images keep their own terms' 'iff|ilbm|lbm|ham|ham8|pbm|deep|acbm|pict|pct|pic' \
    '/imageio/imageio-iff/src/test/resources/' '/imageio/imageio-pict/src/test/resources/'
}
src_ffmpeg_iffpict() {  # Real-world ILBM / PBM and PICT files from FFmpeg's sample collection
  local f u=https://samples.ffmpeg.org/image-samples
  for f in 24.iff ASH.LBM test.iff 20071025155910162.pct 8_PriceFixingCartoon.pct \
    BKK_Logo_normal.pct JDSnowyBlog.pct circlemask.pct image003.pct tess19.pct; do
    fetch_url "$CONF" "ffmpeg-samples/$f" "$u/$f" 'unknown (samples.ffmpeg.org image samples)'
  done
}
src_ffmpeg_iffx() {  # Rarer IFF: Atari ST VDAT, DEEP (TVDC), RGB8/RGBN and ANIM (ops 3-8, J, l)
  local f u=https://samples.ffmpeg.org
  for f in image-samples/atarist/iff/MEDUSAB2.IFF image-samples/atarist/iff/MEDUSABL.IFF \
    image-samples/iff-deep-tvdc/tvpaint_delta.deep image-samples/iff-rgbx/1.rgb8 \
    image-samples/iff-rgbx/1.rgbn anim/anim3_1bpp.anim anim/anim3_zeus.anim \
    anim/anim5_1bpp.anim anim/anim5_ham6.anim anim/anim7l_1bpp.anim anim/anim7s_1bpp.anim \
    anim/anim8l_1bpp.anim anim/anim8s_1bpp.anim anim/animJ_5bpp.anim anim/animJ_ham6.anim \
    anim/anim74_spigot 'anim/anim108_ElGato(maybe_anim_1).anim'; do
    fetch_url "$CONF" "ffmpeg-iffx/${f##*/}" "$u/${f// /%20}" \
      'unknown (samples.ffmpeg.org samples)'
  done
}
src_svg() {        # resvg test suite (SVG inputs only; reference PNGs stay in the repo)
  git_files linebender/resvg 1fe3cfe88bc692612f1ccd4403ba3476b2cdd813 resvg \
    'Apache-2.0 OR MIT' 'svg|svgz' '/crates/resvg/tests/'
}
src_ktx() {        # Git LFS: KTX/KTX2/Basis test textures (BC, ASTC, ETC, UASTC, zstd, cubemaps, arrays)
  git_files KhronosGroup/KTX-Software d0ea84532cd3b01715638ab2dc69670f8acc66bd ktx-software \
    'Apache-2.0' 'ktx|ktx2|basis|dds|astc' \
    '/tests/resources/ktx2/' '/tests/resources/ktx/' '/tests/resources/basis/' \
    '/external/astc-encoder/Test/Images/Small/LDR-L/'
}
src_archive() {    # libarchive reader tests (uuencoded) -> rar4/rar5/7z/zip/zipx containers
  local work="$SCRATCH/libarchive" c=8bb3bbdc7b117a1e22086a2260f2087aafa90687 f
  local names=(rar.rar rar5_stored.rar rar5_compressed.rar rar5_solid.rar rar5_solid_encrypted.rar
    7zip_copy.7z 7zip_lzma2.7z 7zip_bzip2.7z 7zip_deflate.7z 7zip_ppmd_bcj.7z 7zip_zstd.7z
    zip_bzip2.zipx zip_lzma.zipx zip_zstd.zipx zip_comment_stored_1.zip
    zip_traditional_encryption_data.zip tar_empty_filename.tar
    rar_encryption_data.rar rar_encryption_header.rar)
  mkdir -p "$work" "$CONF/libarchive"
  for f in "${names[@]}"; do
    local url="https://raw.githubusercontent.com/libarchive/libarchive/$c/libarchive/test/test_read_format_$f.uu"
    check_space
    curl -fsSL --retry 3 -o "$work/$f.uu" "$url"
    uudecode -o "$CONF/libarchive/$f" "$work/$f.uu"
    provenance "$CONF" "libarchive/$f" "$url (uudecoded)" 'BSD-2-Clause'
  done
  rm -rf "$work"
}
src_bmpsuite() {
  local work="$SCRATCH/bmpsuite" url='https://entropymine.com/jason/bmpsuite/releases/bmpsuite-2.8.zip'
  mkdir -p "$work"; check_space
  curl -fsSL --retry 3 -o "$work/b.zip" "$url"
  unzip -q -o "$work/b.zip" -d "$work/x"
  find "$work/x" -type f -iregex '.*\.\(bmp\|txt\|html\)' | while read -r f; do
    local rel=bmpsuite/${f#"$work/x/"}
    mkdir -p "$CONF/$(dirname "$rel")"; cp "$f" "$CONF/$rel"
    provenance "$CONF" "$rel" "$url" 'bmpsuite (see readme.txt)'
  done
  rm -rf "$work"
}
src_qoi() {
  local work="$SCRATCH/qoi" url='https://qoiformat.org/qoi_test_images.zip'
  mkdir -p "$work"; check_space
  curl -fsSL --retry 3 -o "$work/q.zip" "$url"
  unzip -q -o "$work/q.zip" -d "$work/x"
  find "$work/x" -type f -iname '*.qoi' | while read -r f; do
    local rel=qoi/${f#"$work/x/"}
    mkdir -p "$CONF/$(dirname "$rel")"; cp "$f" "$CONF/$rel"
    provenance "$CONF" "$rel" "$url" 'unknown (qoiformat.org test set)'
  done
  rm -rf "$work"
}
src_kodak() {
  local i
  for i in $(seq -w 1 24); do
    fetch_url "$CONF" "kodak/kodim$i.png" "https://r0k.us/graphics/kodak/kodak/kodim$i.png" \
      'Kodak, released for unrestricted use'
  done
}

# ---------------------------------------------------------------- large (performance)

# RAW: one or two full-size files per vendor / body family / compression mode from
# raw.pixls.us (CC0 unless the site marks otherwise). SHA-256 from the site's filelist.sha256.
RAW_FILES=(
  'Canon/EOS 5DS R/_DSR2002.CR2'                                           # 50 MP CR2, 61 MB
  'Canon/EOS R5/Canon_EOS_R5_CRAW_ISO_100_nocrop_dual.CR3'                 # CR3 CRAW + dual pixel
  'Canon/EOS 300D/CRW_8395.CRW'                                            # CIFF
  'Nikon/D850/Nikon-D850-14bit-lossless-compressed.NEF'                    # 45 MP NEF
  'Nikon/Z 9/Nikon_-_NIKON_Z_9_-_14bit_compressed_(Lossy_High_Efficiency).NEF'  # HE (TicoRAW)
  'Sony/ILCE-7RM5/7RM5-LosslessCompressedLarge.ARW'                        # 61 MP, lossless (LJ92 tiles)
  'Sony/DSC-RX100M7/DSC00601.ARW'                                          # classic compressed ARW
  'Fujifilm/GFX100S/Fujifilm-GFX100S-14bits-losslescompressed-4_3.RAF'     # 102 MP, 110 MB
  'Fujifilm/X-T5/DSCF0021.RAF'                                             # 40 MP X-Trans
  'Fujifilm/X-H1/fuji_xh1_compressed.RAF'                                  # lossy-compressed X-Trans
  'Fujifilm/X-H1/fuji_xh1_uncompressed.RAF'                                # uncompressed X-Trans
  'Fujifilm/X-S10/Fujifilm-X-S10-lossless-compressed.RAF'                  # lossless X-Trans IV
  'Fujifilm/X-A5/DSCF0617.RAF'                                             # Bayer Fuji body
  'OM System/OM-1/OM_Systems_OM1_RAW_Image_20mpx_P1010025.ORF'
  'Olympus/E-M1MarkII/Olympus_EM1mk2_Standard_20MP.ORF'                   # Olympus compressed ORF
  'Panasonic/DC-GH5/_T012010.RW2'
  'Panasonic/DC-G9M2/P1000019.RW2'                                         # RW2 v8 (Huffman)
  'Panasonic/DC-GH5S/P1022081.RW2'                                         # RW2 v5, 14-bit
  'Panasonic/DC-S5/dc-s5_6k4k.RW2'                                         # RW2 v6, 14-bit
  'Panasonic/DC-GH5M2/Panasonic_-_DC-GH5M2_-_1:1.RW2'                      # RW2 v6, 12-bit
  'Pentax/K-3 Mark III/IMGP9854.PEF'
  'Adobe DNG Converter/Canon EOS 5D Mark III/5G4A9394-compressed-lossy.DNG'  # lossy DNG
  'Apple/iPhone 12 Pro/IMG_1361.DNG'                                       # ProRAW (linear DNG)
  'Hasselblad/CFV/RAW_HASSELBLAD_CFV.3FR'
  'Samsung/NX1/2016-07-23-142101_sam_9364.srw'
  'Sigma/SIGMA SD14/20120818-SDIM4579.X3F'                                 # Foveon
  'Minolta/DiMAGE A2/PICT0881.MRW'
  'Kodak/DCS Pro 14nx/D7465857.DCR'
  'Epson/R-D1s/EPSN3575.ERF'
  'GoPro/HERO11 Black/GOPR0123.GPR'                                        # VC-5
  # CR2 across generations (about 162 MB)
  'Canon/EOS 40D/_MG_0154.CR2'                                             # 2007, sRAW (not full raw), 6 MB
  'Canon/EOS 5D Mark II/08.canon.raw.cr2'                                  # 21 MP, 26 MB
  'Canon/EOS 5D Mark II/09.canon.sraw1.cr2'                                # sRAW1 (YCbCr), 15 MB
  'Canon/EOS 7D/RAW_CANON_EOS_7D-mraw.CR2'                                 # mRAW, 16 MB
  'Canon/EOS-1D Mark IV/IMG_5398.CR2'                                      # sRAW/mRAW (not full raw), 11 MB
  'Canon/EOS 80D/IMG_0096.CR2'                                             # sRAW/mRAW (not full raw), 19 MB
  'Canon/EOS 1300D/IMG_0778.CR2'                                           # 21 MB
  'Canon/EOS-1D X Mark II/AZ1I2270.CR2'                                    # 24 MB
  'Canon/PowerShot G7 X Mark II/IMG_0166.CR2'                              # compact CR2, 25 MB
  # Colour-profile sources and bodies newly covered by tools/dng-colour-sources.tsv (about 250 MB)
  'Pentax/K-3 Mark III/_IMG0114.DNG'                                       # camera DNG: K-3 III profile, 31 MB
  'Pentax/K-1 Mark II/_IMG2387.DNG'                                        # camera DNG: K-1 II profile, 44 MB
  'Pentax/K-1 Mark II/_IMG2388.PEF'                                        # 44 MB
  'Olympus/E-M1MarkIII/_3160531.ORF'                                       # 17 MB
  'OM System/OM-5/C1291096.ORF'                                            # 18 MB
  'Nikon/Z 7/8-Nikon-Z7-RAW-12bit-lossless-compressed-S.NEF'               # 24 MB
  'Nikon/Z 50/DSC_0441.NEF'                                                # 25 MB
  'Canon/EOS RP/Canon_EOS_RP_CRAW_ISO_100_nocrop_nodual.CR3'               # 15 MB
  'Canon/EOS 250D/Canon250D_cRAW-3:2.CR3'                                  # 14 MB
  'Canon/EOS R/Canon_EOS_R_CRAW_ISO_100.CR3'                               # 16 MB
)
# Optional extras (not fetched; disk budget). Add to RAW_FILES as space allows:
#   'Phase One/IQ4 150MP/IQ4_150MP_L_14.iiq'                    151 MP, 168 MB
#   'Hasselblad/X2D 100C/B0000079.3FR'                          100 MP, 213 MB
#   'OM System/OM-1/OM_Systems_OM1_RAW_image_HiRes_80mpx_P1010027.ORF'  80 MP, 66 MB
#   'Panasonic/DC-S1R/P1000077.RW2'                             47 MP, 70 MB
#   'Pentax/645Z/IMGP2837.PEF'                                  51 MP, 70 MB
#   'Canon/EOS 5D Mark III/hdrmerge-bayer-fp16-uncompressed.dng' float DNG, 49 MB
#   'Pentax/K-3 Mark III Monochrome/K3III.DNG'                  monochrome DNG, 38 MB
#   'Google/Pixel 8 Pro/PXL_20240415_103400204.RAW-02.ORIGINAL.dng'  45 MB
#   'Leaf/Aptus 75/RAW_APTUS_75.MOS'                            43 MB
#   'Canon/EOS R50/Canon_R50_ISO100_RAW_mechanicShutter.CR3'    non-CRAW CR3, 32 MB
#   'Nikon/D850/Nikon-D850-small.NEF'                           sRAW, 23 MB
#   'Fujifilm/FinePix S5Pro/2018_06150003.RAF'                  SuperCCD, 26 MB
large_raw() { rpu_fetch "${RAW_FILES[@]}"; }

# Source raws for the Adobe DNG Converter colour profiles in tools/dng-colour-sources.tsv:
# the smallest raw.pixls.us file of each body the dcraw table predates.
# tools/corpus/adobe-dng.sh converts them; bodies already in RAW_FILES are converted from there
# (Canon EOS R5/R/RP/250D, Nikon Z 7/Z 50/Z 9, Fujifilm X-T5/X-S10/GFX100S, Panasonic
# DC-S5/DC-GH5M2/DC-G9M2, Olympus E-M1MarkIII, OM System OM-1/OM-5). About 1.47 GB.
RAW_ADOBE_FILES=(
  'Canon/EOS R3/_C3A1200.CR3'                                              # 32 MB
  'Canon/EOS R6/Canon_EOS_R6_CRAW_ISO_100_crop_nodual.CR3'                 # 5 MB
  'Canon/EOS R6 Mark II/canon-r6m2-img_0359-craw-normal-crop.cr3'          # 8 MB
  'Canon/EOS R7/443A0159.CR3'                                              # 14 MB
  'Canon/EOS R8/CRAW_FULL_FRAME.CR3'                                       # 15 MB
  'Canon/EOS R10/IMG_4361.CR3'                                             # 30 MB
  'Canon/EOS R50/Canon_R50_ISO100_cRAW_mechanicShutter.CR3'                # 19 MB
  'Canon/Canon EOS R5m2/APS-C_CRAW.CR3'                                    # 8 MB
  'Canon/EOS 90D/Canon-90D-cRAW-ISO-100.CR3'                               # 20 MB
  'Canon/EOS M6 Mark II/IMG_CRAW_3_2.CR3'                                  # 32 MB
  'Canon/EOS-1D X Mark III/Canon_EOS-1D_X_Mark_III_CRAW_ISO_100_nocrop_nodual.CR3'  # 10 MB
  'Canon/EOS 850D/IMG_1837.CR3'                                            # 28 MB
  'Canon/Canon EOS R100/_MG_2195[1].CR3'                                   # 16 MB
  'Canon/Canon EOS R6 Mark III/IMG_4244.CR3'                               # 7 MB
  'Nikon/D780/DSC_0336.NEF'                                                # 25 MB
  'Nikon/D6/D06_0316.NEF'                                                  # 9 MB
  'Nikon/Z 5/DSC_0519.NEF'                                                 # 22 MB
  'Nikon/Z 6/DSC_0757.NEF'                                                 # 14 MB
  'Nikon/Z 6_2/CZP_0303.NEF'                                               # 29 MB
  'Nikon/Z 8/Nikon_Z8_high_efficiency_low.NEF'                             # HE (TicoRAW), 24 MB
  'Nikon/Z f/DSC_0043.NEF'                                                 # HE (TicoRAW), 13 MB
  'Nikon/Z 30/_DSC0028.NEF'                                                # 20 MB
  'Nikon/Z 7_2/Z72_0030.NEF'                                               # 38 MB
  'Nikon/Z fc/_DSC0070.nef'                                                # 20 MB
  'Fujifilm/X-T3/AFXT2721.RAF'                                             # 27 MB
  'Fujifilm/X-T4/DSCF0268.RAF'                                             # 27 MB
  'Fujifilm/X-H2/Fuji-X-H2-Compressed-Raw.RAF'                             # 29 MB
  'Fujifilm/X-H2S/IMG-20221005-0667.raf'                                   # 21 MB
  'Fujifilm/X-S20/DSCF2529.RAF'                                            # 21 MB
  'Fujifilm/X100V/DSCF0734.RAF'                                            # 32 MB
  'Fujifilm/X100VI/DSCF0076.RAF'                                           # 30 MB
  'Fujifilm/X-T30/DSCF0066.RAF'                                            # 32 MB
  'Fujifilm/X-T30 II/compressed_X-T30II.RAF'                               # 19 MB
  'Fujifilm/X-E4/fuji_xe4_32_lossy.RAF'                                    # 20 MB
  'Fujifilm/GFX 100/_DSF3980.RAF'                                          # 105 MB
  'Panasonic/DC-S1/P1033119.RW2'                                           # 36 MB
  'Panasonic/DC-S1R/P1000077.RW2'                                          # 70 MB
  'Panasonic/DC-S5M2/P1126460_burst_electronic.RW2'                        # 37 MB
  'Panasonic/DC-GH6/1-1.RW2'                                               # 45 MB
  'Olympus/E-M5 Mark III/PB240241.ORF'                                     # 67 MB
  'Olympus/E-M10 Mark IV/PA040002.ORF'                                     # 18 MB
  'OM System/OM-1 Mark II/20mp-12bit.orf'                                  # 22 MB
  'Hasselblad/X1D/B9999910.3FR'                                            # 110 MB
  'Hasselblad/X1D II 50C/x1d-II-sample-02.fff'                             # 78 MB
  'Hasselblad/X2D 100C/Job_0080.fff'                                       # 161 MB
)
large_raw_adobe() { rpu_fetch "${RAW_ADOBE_FILES[@]}"; }

# rpu_fetch PATH...: raw.pixls.us files (CC0) into $LARGE/raw/, checked against its filelist.
rpu_fetch() {
  local list="$SCRATCH/rpu_filelist.sha256" p sha enc
  [[ -s $list ]] || curl -fsSL -o "$list" https://raw.pixls.us/data/filelist.sha256
  for p in "$@"; do
    sha=$(grep -F " *$p" "$list" | head -1 | cut -d' ' -f1)
    [[ -n $sha ]] || die "not in raw.pixls.us filelist: $p"
    enc=$(python3 -I -c 'import sys,urllib.parse;print(urllib.parse.quote(sys.argv[1]))' "$p")
    fetch_url "$LARGE" "raw/$p" "https://raw.pixls.us/data/$enc" 'CC0-1.0 (raw.pixls.us)' "$sha"
  done
}

# Large single files: (dest, URL, licence). Sizes from the source APIs in Oct 2026.
LARGE_FILES=(
  # JPEG: 40000x6223 (249 MP) real stitched panorama, 48 MB
  'jpeg/Riga_Skyline_Panorama_Diliff.jpg|https://upload.wikimedia.org/wikipedia/commons/7/71/Riga_Skyline_Panorama%2C_Latvia_-_Diliff.jpg|CC BY-SA 3.0 (Diliff, Wikimedia Commons)'
  # PNG: 38400x38400 (1.47 GP), 83 MB
  'png/Historische_Flurkarte_kernen.png|https://upload.wikimedia.org/wikipedia/commons/b/b7/Historische_Flurkarte_kernen.png|Public domain (Wikimedia Commons)'
  # PNG: 36000x18000 (648 MP) NASA SVS, 47 MB
  'png/NEX_GDDP_CMIP6_tasmax_SVS5343.png|https://upload.wikimedia.org/wikipedia/commons/1/14/NEX_GDDP_CMIP6_Historical_and_Predicted_Global_Maximum_Monthly_Temperature_from_1950_-_2100_%28SVS5343_-_relief_tasmax_monthly_ssp126_120_01692%29.png|Public domain (NASA)'
  # TIFF: 10000x10000 (100 MP) orthophoto, 19 MB
  'tiff/Dop20rgb_32_418_5822_2_ni_2017-05-26.tif|https://upload.wikimedia.org/wikipedia/commons/d/df/Dop20rgb_32_418_5822_2_ni_2017-05-26.tif|CC BY 4.0 (LGLN Niedersachsen)'
  # TIFF: 22009x21937 (483 MP) Capella SAR GeoTIFF, 158 MB
  'tiff/Sunny_Isles_Beach_CAPELLA_C13_preview.tiff|https://upload.wikimedia.org/wikipedia/commons/f/f9/Sunny_Isles_Beach%2C_Miami-Dade_County%2C_Florida%2C_United_States_%28CAPELLA_C13_SP_GEO_HH_20251005113102_20251005113110_preview%29.tiff|CC BY 4.0 (Capella Space)'
  # GIF: long colour-cycling animation, 600x600, 31 MB
  'gif/Mandelbrot_Set_Color_Cycling_Animation_600px_8.gif|https://upload.wikimedia.org/wikipedia/commons/b/b6/Mandelbrot_Set_Color_Cycling_Animation_600px_8.gif|CC0'
  # WebP: 10886x16383 (178 MP, near the 16383 limit), 40 MB
  'webp/Macau_Morpheus_Hotel_2024_dllu.webp|https://upload.wikimedia.org/wikipedia/commons/e/e7/Macau_Morpheus_Hotel_2024_dllu.webp|CC BY-SA 4.0 (dllu, Wikimedia Commons)'
  # SVG: heavy geographic map, 13.6 MB of paths
  'svg/Physical_map_of_Ancient_Greece-ru.svg|https://upload.wikimedia.org/wikipedia/commons/d/d4/Physical_map_of_Ancient_Greece-ru.svg|CC BY-SA 4.0 (Wikimedia Commons)'
  # Radiance HDR: 8192x4096 equirect, 74 MB
  'hdr/kloofendal_48d_partly_cloudy_puresky_8k.hdr|https://dl.polyhaven.org/file/ph-assets/HDRIs/hdr/8k/kloofendal_48d_partly_cloudy_puresky_8k.hdr|CC0 (Poly Haven)'
  # OpenEXR: 4096x2048 float, 75 MB
  'exr/kloofendal_48d_partly_cloudy_puresky_4k.exr|https://dl.polyhaven.org/file/ph-assets/HDRIs/exr/4k/kloofendal_48d_partly_cloudy_puresky_4k.exr|CC0 (Poly Haven)'
)
large_files() {
  local e dest url lic
  for e in "${LARGE_FILES[@]}"; do
    IFS='|' read -r dest url lic <<< "$e"
    fetch_url "$LARGE" "$dest" "$url" "$lic"
  done
}

# libjxl conformance reference decodes (~0.6 GB) next to the inputs and test.json files from
# src_jxl: reference.icc (the colour space of the reference) and reference_image.npy (float32,
# frames x height x width x channels). The repository holds only their SHA-256 (test.json
# "sha256sums"); the objects live in the project's public bucket, addressed by hash. The *_5
# variants (looser limits) are skipped: their images duplicate the strict ones. Used by
# crates/formats/specular-jxl/tests/conformance.rs.
JXL_CONF_OBJECTS='https://storage.googleapis.com/storage/v1/b/jxl-conformance/o/objects%2F'
large_jxl_refs() {
  local dir=$CONF/jxl-conformance/testcases t name sha n=0
  [[ -d $dir ]] || src_jxl
  for t in "$dir"/*/test.json; do
    [[ $t == *_5/test.json ]] && continue
    while IFS=$'\t' read -r name sha; do
      [[ $name == reference.icc || $name == reference_image.npy ]] || continue
      [[ $sha =~ ^[0-9a-f]{64}$ ]] || die "bad sha256 for $name in $t"
      fetch_url "$CONF" "jxl-conformance/testcases/$(basename "$(dirname "$t")")/$name" \
        "$JXL_CONF_OBJECTS$sha?alt=media" 'BSD-3-Clause' "$sha"
      n=$((n + 1))
    done < <(python3 -I -c 'import json, sys
for k, v in json.load(open(sys.argv[1]))["sha256sums"].items(): print(f"{k}\t{v}")' "$t")
  done
  echo "  libjxl/conformance references: $n files -> conformance/jxl-conformance"
}

# ---------------------------------------------------------------- driver
# (keep last: sections are discovered with declare -F)
main() {
  local mode=all sections=() s
  for a in "$@"; do
    case $a in
      --large) mode=large ;;
      --no-large) mode=small ;;
      -h|--help) sed -n '2,15p' "$0"; exit 0 ;;
      *) sections+=("$a") ;;
    esac
  done
  if (( ${#sections[@]} == 0 )); then
    [[ $mode != large ]] && sections+=($(declare -F | awk '{print $3}' | grep '^src_'))
    [[ $mode != small ]] && sections+=($(declare -F | awk '{print $3}' | grep '^large_'))
  fi
  for s in "${sections[@]}"; do
    declare -F "$s" >/dev/null || die "unknown section $s"
    echo "== $s"
    "$s"
  done
  # Provenance logs can repeat entries after re-runs; keep the last entry per path.
  for d in "$CONF" "$LARGE"; do
    [[ -f $d/.provenance.tsv ]] || continue
    tac "$d/.provenance.tsv" | awk -F'\t' '!seen[$1]++' | tac > "$d/.provenance.tmp"
    mv "$d/.provenance.tmp" "$d/.provenance.tsv"
  done
  rm -rf "$SCRATCH"
  # Rehash every file and regenerate the per-file tables in MANIFEST.md.
  python3 -I "$ROOT/tools/corpus/conformance-manifest.py"
  df -h "$ROOT" | tail -1
}
main "$@"
