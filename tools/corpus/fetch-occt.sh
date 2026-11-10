#!/usr/bin/env bash
# Fetch the STEP and IGES test corpus into tools/corpus/occt/ (gitignored: nothing here is
# committed). The OpenCASCADE tests (specular-decode `tests/occt.rs`) read it from
# SPECULAR_OCCT_CORPUS, else tools/corpus/occt, and skip when it is absent.
#
# Usage: tools/corpus/fetch-occt.sh
#   OUT defaults to $SPECULAR_OCCT_CORPUS, else tools/corpus/occt.
#
# What it covers (a few MB):
#   occt/     OCCT 7.9.3's own sample models (data/step, data/iges) from the pinned release
#             tarball (LGPL-2.1 with the OCCT exception)
#   kicad/    KiCad library 3D models: coloured STEP parts (CC-BY-SA 4.0 with the KiCad
#             libraries exception)
#   pythonocc/ the AS1 assembly (AP214 with colours and an assembly tree) and small parts
#             from pythonocc-demos (LGPL-3.0)
# Generated models (boxes, cylinders, a coloured assembly, AP203/AP214/AP242 and IGES
# variants, a large instanced assembly) and the oracle values come from the C++ harness
# crates/specular-decode/tests/occt/oracle.cpp, built against the same OCCT (see
# tests/occt.rs).
#
# Downloads are untrusted data: this script only copies files, never runs anything from them.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_OCCT_CORPUS:-$ROOT/tools/corpus/occt}
SCRATCH=${SCRATCH:-$ROOT/target/agent-corpus/occt-fetch}
mkdir -p "$OUT" "$SCRATCH"

die() { echo "fetch-occt: $*" >&2; exit 1; }

# provenance RELPATH URL LICENCE
provenance() { printf '%s\t%s\t%s\n' "$1" "$2" "$3" >> "$OUT/.provenance.tsv"; }

# get URL DEST SHA256 LICENCE: downloads one file and checks its hash.
get() {
  local url=$1 dest=$2 sha=$3 lic=$4
  mkdir -p "$(dirname "$OUT/$dest")"
  if [[ -f "$OUT/$dest" ]] && echo "$sha  $OUT/$dest" | sha256sum -c --status; then
    return
  fi
  curl -fsSL --retry 3 -m 300 -o "$SCRATCH/dl" "$url" || die "download failed: $url"
  local got
  got=$(sha256sum "$SCRATCH/dl" | cut -d' ' -f1)
  [[ "$got" == "$sha" ]] || die "$dest: sha256 $got, expected $sha"
  mv "$SCRATCH/dl" "$OUT/$dest"
  provenance "$dest" "$url" "$lic"
}

OCCT_URL=https://codeload.github.com/Open-Cascade-SAS/OCCT/tar.gz/refs/tags/V7_9_3
OCCT_SHA=5ecf094ec6b12d5413dfb851d8c3590c354058aee556e32e408bdfbf8c357d57

src_occt() {
  local tgz=$SCRATCH/occt-7.9.3.tar.gz
  if [[ ! -f $tgz ]] || ! echo "$OCCT_SHA  $tgz" | sha256sum -c --status; then
    curl -fsSL --retry 3 -m 600 -o "$tgz" "$OCCT_URL" || die "download failed: $OCCT_URL"
    echo "$OCCT_SHA  $tgz" | sha256sum -c --status || die "OCCT tarball hash mismatch"
  fi
  mkdir -p "$OUT/occt"
  tar -xzf "$tgz" -C "$OUT/occt" --strip-components=3 \
    OCCT-7_9_3/data/step/screw.step OCCT-7_9_3/data/step/linkrods.step \
    OCCT-7_9_3/data/iges/bearing.iges OCCT-7_9_3/data/iges/hammer.iges
  for f in screw.step linkrods.step bearing.iges hammer.iges; do
    provenance "occt/$f" "$OCCT_URL" "LGPL-2.1-only WITH OCCT-exception-1.0"
  done
}

KICAD=https://gitlab.com/kicad/libraries/kicad-packages3D/-/raw/9.0.0
src_kicad() {
  get "$KICAD/Package_DIP.3dshapes/DIP-8_W7.62mm.step" kicad/DIP-8_W7.62mm.step \
    4a377a56a8d9e9c0b86c095ef9ecc1869ac19e7f5069a3247b294250bf4f235d "CC-BY-SA-4.0 WITH KiCad-libraries-exception"
  get "$KICAD/Connector_PinHeader_2.54mm.3dshapes/PinHeader_1x04_P2.54mm_Vertical.step" \
    kicad/PinHeader_1x04_P2.54mm_Vertical.step a74079b8d06bda78847623b39ee1855ffd1d9a8c1e896b349e4f1dc2056d44d9 \
    "CC-BY-SA-4.0 WITH KiCad-libraries-exception"
  get "$KICAD/Package_TO_SOT_SMD.3dshapes/SOT-23.step" kicad/SOT-23.step \
    9fe9af3cc6cf3d3bc3bd3da71731af2037c67f690245102a20e1e39a33e9ade4 "CC-BY-SA-4.0 WITH KiCad-libraries-exception"
}

PYOCC=https://raw.githubusercontent.com/tpaviot/pythonocc-demos/7.7.2/assets/models
src_pythonocc() {
  get "$PYOCC/as1-oc-214.stp" pythonocc/as1-oc-214.stp 038be659c54b16c9f3da8d7b2da7b63e3fd8879d3abe5b7826108336a7c0bae9 "LGPL-3.0"
  get "$PYOCC/face_recognition_sample_part.stp" pythonocc/face_recognition_sample_part.stp \
    de7b8a513ae923cd11de2323e798ac084cc94f9aa7d7a2a8adab65fc592f8d0b "LGPL-3.0"
}

rm -f "$OUT/.provenance.tsv"
src_occt
src_kicad
src_pythonocc
rm -rf "$SCRATCH"
echo "fetch-occt: corpus in $OUT ($(du -sh "$OUT" | cut -f1))"
