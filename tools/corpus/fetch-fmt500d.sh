#!/bin/sh
# Fetch the small public corpus for the fmt500d formats (a few MB) into the folder given,
# default $CARGO_TARGET_DIR/corpus/fmt500d. Pillow test images (HPND repo, public-domain
# hopper), libgd's GD2 test file, OpenJPEG's PGX conformance file and FFmpeg's FATE samples.
#
#     tools/corpus/fetch-fmt500d.sh
#     python3 -I tools/corpus/fmt500d-oracle.py $CARGO_TARGET_DIR/corpus/fmt500d
set -eu
out=${1:-${CARGO_TARGET_DIR:?set CARGO_TARGET_DIR or pass a folder}/corpus/fmt500d}
mkdir -p "$out"
pil=https://raw.githubusercontent.com/python-pillow/Pillow/main/Tests
get() {
    # $1 URL, $2 file name
    if [ ! -s "$out/$2" ]; then
        echo "fetching $2"
        curl -fsSL --retry 3 -o "$out/$2.part" "$1" && mv "$out/$2.part" "$out/$2"
    fi
}
for f in hopper.psd gbr.gbr full_gimp_palette.gpl custom_gimp_palette.gpl gimp_gradient.ggr \
    gimp_gradient_with_name.ggr hopper.msp hopper.gd hopper.pxr hopper.im bw_gradient.imt \
    hopper.p7 ftex_dxt1.ftc ftex_uncompressed.ftu hopper.spider \
    cmx3g8_wv_1998.260_0745_mcidas.ara hopper_g4.tif hopper.png; do
    get "$pil/images/$f" "$f"
done
get https://raw.githubusercontent.com/libgd/libgd/master/tests/gd2/conv_test.gd2 conv_test.gd2
get https://raw.githubusercontent.com/uclouvain/openjpeg-data/master/baseline/conformance/c1p0_01_0.pgx c1p0_01_0.pgx
# Aseprite's own test sprites (Aseprite EULA; a gitignored dev corpus) and their README.
ase=https://raw.githubusercontent.com/aseprite/aseprite/main/tests/sprites
for f in 1empty3 2f-index-3x3 2x2tilemap2x2tile 2x3tilemap-indexed 3x2tilemap-grayscale \
    4f-index-4x4 abcd bg-index-3 cut_paste file-tests-props groups2 groups3abc link \
    point2frames point4frames slices-moving slices tags3 tags3x123reps z-order; do
    get "$ase/$f.aseprite" "$f.aseprite"
done
get "$ase/README.md" aseprite-README.md
fate=https://fate-suite.ffmpeg.org
get "$fate/apv/profile_422-10.apv" profile_422-10.apv
echo "corpus in $out"
