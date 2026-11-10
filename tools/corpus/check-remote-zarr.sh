#!/bin/sh
# Dev-only: opens a few public OME-Zarr samples of the Image Data Resource (EBI's S3-compatible
# store) over HTTPS and as s3:// against its endpoint, through the remote Zarr reader, and
# prints what was listed, opened and decoded with request counts and times. Fetches only a
# few chunks; nothing is kept. Never run by the tests.
#
#   tools/corpus/check-remote-zarr.sh [URL…]
#
# Build env as for any cargo command (CARGO_TARGET_DIR etc.).
set -eu
cd "$(dirname "$0")/../.."
if [ "$#" -eq 0 ]; then
    set -- \
        https://uk1s3.embassy.ebi.ac.uk/idr/zarr/v0.4/idr0062A/6001240.zarr \
        https://uk1s3.embassy.ebi.ac.uk/idr/zarr/v0.4/idr0001A/2551.zarr \
        https://uk1s3.embassy.ebi.ac.uk/idr/zarr/v0.5/idr0062A/6001240_labels.zarr \
        s3://idr/zarr/v0.4/idr0062A/6001240.zarr
fi
# s3:// addresses go to EBI's endpoint, unsigned.
export AWS_ENDPOINT_URL="${AWS_ENDPOINT_URL:-https://uk1s3.embassy.ebi.ac.uk}"
export AWS_NO_SIGN_REQUEST=true
nice cargo run --offline -q -j 2 -p specular-decode --example zarr_remote_check -- "$@"
