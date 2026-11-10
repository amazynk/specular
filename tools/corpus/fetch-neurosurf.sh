#!/usr/bin/env bash
# Fetch the brain surface and tractography corpus into tools/corpus/neurosurf/ (gitignored:
# nothing here is committed) and write nibabel's reading of it (the oracle,
# crates/formats/specular-neuro/gen/oracle.py) to expected.tsv beside it.
#
# Usage: tools/corpus/fetch-neurosurf.sh
#   OUT defaults to $SPECULAR_NEURO_CORPUS, else tools/corpus/neurosurf.
#   PYTHON (default python3) must have nibabel and numpy for the oracle.
#   Then: SPECULAR_NEURO_DIR=tools/corpus/neurosurf
#   SPECULAR_NEURO_ORACLE=tools/corpus/neurosurf/expected.tsv
#   cargo test -p specular-neuro --test oracle -- --nocapture
#   cargo test -p specular-medvol --test mif mif_corpus_matches_oracle -- --nocapture
#
# What it covers (about 12 MB):
#   nibabel/     nibabel 5.2.1's own test data (MIT): GIFTI in ASCII, Base64 and
#                GZipBase64, an external data file, labels (rh.aparc.annot), shapes, a
#                functional time series; TrackVis v2 (complex, big-endian, LPS, empty) and
#                MRtrix .tck (big-endian, NaN-separated, multi-line header, empty)
#   hcp/         nibabel's CIFTI-2 test data (nibabel-nitest-cifti2, ODC PDDL): Conte69
#                32k_fs_LR dscalar, dtseries, ptseries and dlabel, and ones.dscalar, beside
#                TemplateFlow's fsLR 32k midthickness surfaces (the same 32 492 vertices per
#                hemisphere, so the CIFTI files open on them)
#   mrtrix/      MRtrix3's test data (MPL-2.0): .mif images (DWI, FODs, warps, masks),
#                .mif.gz templates and a .tck tractogram
#
# Downloads are untrusted data: this script only copies files, never runs anything from them.
# Stops if free space on the target filesystem drops below MIN_FREE_GB (default 8).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_NEURO_CORPUS:-$ROOT/tools/corpus/neurosurf}
MIN_FREE_GB=${MIN_FREE_GB:-8}
PYTHON=${PYTHON:-python3}
mkdir -p "$OUT/nibabel" "$OUT/hcp" "$OUT/mrtrix"
: > "$OUT/.provenance.tsv"

die() { echo "fetch-neurosurf: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

# get URL DEST LICENCE
get() {
  local url=$1 dest=$2 lic=$3 try
  check_space
  if [[ ! -s "$OUT/$dest" ]]; then
    for try in 1 2 3 4 5; do
      if curl -fsSL --retry 3 -o "$OUT/$dest.part" "$url"; then
        break
      fi
      [[ $try == 5 ]] && die "download failed: $url"
      sleep $((try * 3))
    done
    mv "$OUT/$dest.part" "$OUT/$dest"
  fi
  printf '%s\t%s\t%s\n' "$dest" "$url" "$lic" >> "$OUT/.provenance.tsv"
}

NIB=https://raw.githubusercontent.com/nipy/nibabel/5.2.1/nibabel
for f in ascii.gii ascii_flat_data.gii base64bin.gii external.dat external.gii gzipbase64.gii \
  label.gii rh.aparc.annot.gii rh.shape.curv.gii task.func.gii; do
  get "$NIB/gifti/tests/data/$f" "nibabel/$f" "MIT (nibabel)"
done
for f in complex.trk complex_big_endian.trk simple.trk standard.trk standard.LPS.trk empty.trk \
  standard.tck simple.tck simple_big_endian.tck matlab_nan.tck multiline_header_field.tck \
  empty.tck; do
  get "$NIB/tests/data/$f" "nibabel/$f" "MIT (nibabel)"
done

CIFTI=https://raw.githubusercontent.com/demianw/nibabel-nitest-cifti2/master
for f in Conte69.MyelinAndCorrThickness.32k_fs_LR.dscalar.nii \
  Conte69.MyelinAndCorrThickness.32k_fs_LR.dtseries.nii \
  Conte69.MyelinAndCorrThickness.32k_fs_LR.ptseries.nii \
  Conte69.parcellations_VGD11b.32k_fs_LR.dlabel.nii ones.dscalar.nii; do
  get "$CIFTI/$f" "hcp/$f" "ODC PDDL (nibabel-nitest-cifti2)"
done
TF=https://templateflow.s3.amazonaws.com/tpl-fsLR
for h in L R; do
  get "$TF/tpl-fsLR_den-32k_hemi-${h}_midthickness.surf.gii" \
    "hcp/fsLR.${h}.midthickness.32k_fs_LR.surf.gii" "TemplateFlow tpl-fsLR (HCP)"
done

MRT=https://raw.githubusercontent.com/MRtrix3/test_data/master
for f in components_mask.mif dilate_master.mif dwi.mif dwi_mean.mif fod.mif fod_warp.mif \
  mask.mif noise.mif peaks.mif rotatez_warp.mif unit_warp.mif warp.mif moving.mif.gz \
  template.mif.gz tracks.tck; do
  get "$MRT/$f" "mrtrix/$f" "MPL-2.0 (MRtrix3 test data)"
done

echo "fetch-neurosurf: $(find "$OUT" -type f ! -name '.*' | wc -l) files, $(du -sh "$OUT" | cut -f1)"
"$PYTHON" -I "$ROOT/crates/formats/specular-neuro/gen/oracle.py" "$OUT" "$OUT/expected.tsv"
echo "fetch-neurosurf: oracle written to $OUT/expected.tsv"
