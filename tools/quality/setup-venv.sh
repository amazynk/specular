#!/usr/bin/env bash
# Python oracles for the quality runners, in a venv under CARGO_TARGET_DIR (default
# target/quality): pydicom (DICOM display pipeline), colour-science (HDR transfer
# functions, GSDF, CIEDE2000), rawpy (LibRaw) and numpy/scipy. Prints the venv's python.
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
venv=${CARGO_TARGET_DIR:-$root/target/quality}/venv
[ -x "$venv/bin/python" ] || python3 -m venv "$venv"
"$venv/bin/pip" install -q numpy scipy pillow pydicom==3.0.2 colour-science==0.4.7 rawpy==0.27.1
echo "$venv/bin/python"
