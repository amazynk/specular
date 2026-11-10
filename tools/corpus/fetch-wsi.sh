#!/usr/bin/env bash
# Fetch the whole-slide and OME microscopy test corpus into tools/corpus/wsi/ (gitignored: nothing
# here is committed), make the OME-Zarr stores, and write the oracle. The specular-wsi corpus test
# reads it in place from SPECULAR_WSI_CORPUS (a worktree has no corpus of its own), else from
# tools/corpus/wsi, and skips when it is absent.
#
# Usage: tools/corpus/fetch-wsi.sh [SECTION...]
#   SECTION is a function name below (src_openslide, src_ome, src_zarr, src_oracle). Default: all
#   four in that order.
#   OUT defaults to $SPECULAR_WSI_CORPUS, else tools/corpus/wsi.
#   PYTHON (default python3) must have numpy, tifffile, imagecodecs, zarr, ome-zarr and
#   openslide-python with openslide-bin for src_zarr and src_oracle. A venv under target/ is
#   fine: python3 -m venv target/wsi-venv && target/wsi-venv/bin/pip install --no-cache-dir
#   numpy tifffile imagecodecs zarr ome-zarr openslide-python openslide-bin; then
#   PYTHON=target/wsi-venv/bin/python.
#
# What it covers (about 1.2 GB):
#   openslide  OpenSlide's public test slides (CC0 / "freely redistributable", see
#              https://openslide.cs.cmu.edu/download/openslide-testdata/): Aperio SVS with JPEG
#              (CMU-1, CMU-1-Small-Region) and JPEG 2000 (JP2K-33003-1) tiles, Hamamatsu NDPI
#              (CMU-1), Leica SCN (Leica-1 brightfield, Leica-Fluorescence-1), a generic tiled pyramidal TIFF (CMU-1)
#   ome        OME's OME-TIFF samples, 2016-06 (CC BY 4.0): bioformats-artificial single and
#              multi-channel, Z, T and BigTIFF; tubhiswt-3D (one file per channel, each naming
#              the other); the multifile companion set (BinaryOnly files and a .companion.ome);
#              retina_large (a SubIFD pyramid)
#   zarr       OME-Zarr stores written by ome-zarr-py / zarr-python here (v0.4 on Zarr v2 with
#              blosc, zstd and gzip; v0.5 on Zarr v3 with zstd and sharding off; one zipped)
#   oracle/    for each file: expected.tsv (size, levels, MPP, channels) and, per file, raw RGB(A)
#              or 16-bit regions read by openslide-python / tifffile / zarr for the pixel check
#
# Downloads are untrusted data: each source gets its own directory, every file is checked against
# its SHA-256 below, and nothing from them is run (the oracle reads them with python -I from this
# repository's script). Stops if free space drops below MIN_FREE_GB (default 8).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_WSI_CORPUS:-$ROOT/tools/corpus/wsi}
MIN_FREE_GB=${MIN_FREE_GB:-8}
PYTHON=${PYTHON:-python3}
mkdir -p "$OUT"

die() { echo "fetch-wsi: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

# provenance RELPATH URL LICENCE
provenance() { printf '%s\t%s\t%s\n' "$1" "$2" "$3" >> "$OUT/.provenance.tsv"; }

# fetch BASEURL DEST LICENCE (SHA256 REMOTE-PATH)... ; saved under DEST/ by REMOTE-PATH with
# its slashes kept as folders.
fetch() {
  local base=$1 dest=$2 lic=$3; shift 3
  while (( $# >= 2 )); do
    local want=$1 name=$2 out="$OUT/$dest/$2"; shift 2
    mkdir -p "$(dirname "$out")"
    if [[ ! -s $out ]]; then
      check_space
      curl -fsSL --retry 3 -A 'specular-corpus-fetch/1 (test corpus)' -o "$out.part" "$base/$name"
      mv "$out.part" "$out"
    fi
    if [[ $want == - ]]; then
      sha256sum "$out"
    else
      echo "$want  $out" | sha256sum -c --quiet - || die "checksum mismatch: $dest/$name"
    fi
    provenance "$dest/$name" "$base/$name" "$lic"
  done
}

src_openslide() {    # OpenSlide test data (CC0 / freely redistributable)
  fetch https://openslide.cs.cmu.edu/download/openslide-testdata openslide 'OpenSlide test data' \
    ed92d5a9f2e86df67640d6f92ce3e231419ce127131697fbbce42ad5e002c8a7 Aperio/CMU-1-Small-Region.svs \
    00a3d54482cd707abf254fe69dccc8d06b8ff757a1663f1290c23418c480eb30 Aperio/CMU-1.svs \
    6205ccf75a8fa6c32df7c5c04b7377398971a490fb6b320d50d91f7ba6a0e6fd Aperio/JP2K-33003-1.svs \
    edf4a1ccf395c7000ae93ad3b44c07d97043810e00be0c1d167dd09bbe436e46 Hamamatsu/CMU-1.ndpi \
    63a3c00fef5215497a9725cf19092a93f7f5ff855ce8561af74b087631831a97 Leica/Leica-1.scn \
    a9037f7d683dc0a95e2e5f33bceee71f0449042e70e6d1421ca580bcd97a023a Leica/Leica-Fluorescence-1.scn \
    73a7e89bc15576587c3d68e55d9bf92f09690280166240b48ff4b48230b13bcd Generic-TIFF/CMU-1.tiff
}

src_ome() {          # OME-TIFF samples, 2016-06 (CC BY 4.0, (c) the OME Consortium)
  fetch https://downloads.openmicroscopy.org/images/OME-TIFF/2016-06 ome 'CC-BY-4.0' \
    8188f23c90bbdebcac5f0788596ccad5e524cfa6f20a87682882421c94be7175 bioformats-artificial/single-channel.ome.tif \
    ff98dcffeebc6ea7011ba3ece20b7f8841bed57949566eb2460e7da389be9fd3 bioformats-artificial/multi-channel.ome.tif \
    f8cebdb6036060e433576bd0303f6f632fac6f2a71f2e7ceec08a13f34f69cba bioformats-artificial/multi-channel-z-series.ome.tif \
    caf707ca2ba6c42c40ded92245432d350a781fcdd03c0b178834f5eb5e5b96f3 bioformats-artificial/multi-channel-4D-series.ome.tif \
    529dbd3ea2a5db3a84f9647216c1ba22f42029a84db7d3475cc2b631fda6f224 bioformats-artificial/4D-series.ome.btf \
    6e77b86de28c84f275394f15aef861053bc5e66f16417e291404b62ed20d1df4 tubhiswt-3D/tubhiswt_C0.ome.tif \
    9da1b207fec196b2c46a2901b165fa1b0b39fb7be18925239d47ffa26aee78c0 tubhiswt-3D/tubhiswt_C1.ome.tif \
    8a82cf28f2ee72d0b20386af914de3df78eee7d8112ed4c4c1cba324c495be93 companion/multifile.companion.ome \
    1ed1bf3d8ed588f544636622c6615518bcb1825ad7cbb979f2b882bd39b19fa2 companion/multifile-Z1.ome.tiff \
    fc4764f5e59ace5c30a895d238caa8df249980201b06513b8219c0f5932297f1 companion/multifile-Z2.ome.tiff \
    310d65183645f36de7a6c33ef3bdeb3ee0a9a7063755fc4edeacf75ea0314ddc companion/multifile-Z3.ome.tiff \
    8caceb915f8a08ffc6c6aebbbe983704e843b7f8714099a0dbe88a285cacdfd6 companion/multifile-Z4.ome.tiff \
    6d01d236a5a17ee96bb6a2280358d1c1e6f93ac308630dab80dd5572ab7ae222 companion/multifile-Z5.ome.tiff \
    dad5c9fca085e277b6deee9ec198725513c36111f772ad01f523f6e2bde6447c sub-resolutions/Z-stack/retina_large.ome.tiff
}

src_zarr() {         # OME-Zarr stores made here (deterministic content)
  check_space
  "$PYTHON" -I "$ROOT/tools/corpus/wsi-oracle.py" zarr "$OUT"
}

src_oracle() {       # expected.tsv and reference regions
  check_space
  "$PYTHON" -I "$ROOT/tools/corpus/wsi-oracle.py" oracle "$OUT"
}

if (( $# == 0 )); then
  set -- src_openslide src_ome src_zarr src_oracle
  : > "$OUT/.provenance.tsv"
fi
for s in "$@"; do
  declare -F "$s" > /dev/null || die "no section $s"
  "$s"
done
du -sh "$OUT"
