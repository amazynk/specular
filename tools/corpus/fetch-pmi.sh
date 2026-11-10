#!/usr/bin/env bash
# Fetch the NIST MBE PMI test models (CTC, FTC and STC, STEP AP242, and AP203 with PMI) into
# tools/corpus/pmi/ (gitignored: nothing here is committed). specular-decode-host's
# tests/pmi.rs reads them from SPECULAR_PMI_CORPUS, else tools/corpus/pmi, and skips when they
# are absent; tools/corpus/pmi-oracle.py writes the independent reading they are compared
# with (pmi-oracle.tsv).
#
# Usage: tools/corpus/fetch-pmi.sh
#
# Source: NIST "MBE PMI Validation and Conformance Testing", NIST-PMI-STEP-Files.zip (14 MB;
# the STEP files kept are about 60 MB). Licence: NIST, public domain ("can be used without
# any restrictions"; acknowledgement asked). The zip is checked against its SHA-256; a
# mismatch stops the script. Downloads are untrusted data: this script only copies files out
# of the zip.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_PMI_CORPUS:-$ROOT/tools/corpus/pmi}
SCRATCH=${SCRATCH:-$ROOT/target/agent-corpus/pmi-fetch}
URL=https://www.nist.gov/system/files/documents/noindex/2024/06/19/NIST-PMI-STEP-Files.zip
SHA=8fa78429e6d8d9b0d7681d223b6aa9ec98c3772185c55b1a0e3679b21c181911

die() { echo "fetch-pmi: $*" >&2; exit 1; }

mkdir -p "$OUT" "$SCRATCH"
zip=$SCRATCH/NIST-PMI-STEP-Files.zip
if [[ ! -f $zip ]] || ! echo "$SHA  $zip" | sha256sum -c --status; then
  echo "fetch-pmi: $URL"
  curl -fSL --retry 3 -m 600 --progress-bar -o "$zip.part" "$URL" || die "download failed: $URL"
  echo "$SHA  $zip.part" | sha256sum -c --status || { rm -f "$zip.part"; die "SHA-256 mismatch"; }
  mv "$zip.part" "$zip"
fi
# AP242 files at the top, AP203 with PMI under ap203/ (the geometry-only AP203 files and the
# PDF drawings are left out).
python3 -I - "$zip" "$OUT" <<'PY'
import os, sys, zipfile
z = zipfile.ZipFile(sys.argv[1])
out = sys.argv[2]
n = 0
for i in z.infolist():
    parts = i.filename.split("/")
    if not parts[-1].lower().endswith(".stp") or ".." in parts:
        continue
    if len(parts) == 2:
        dest = os.path.join(out, parts[-1])
    elif len(parts) == 3 and parts[1] == "AP203 with PMI":
        dest = os.path.join(out, "ap203", parts[-1])
    else:
        continue
    os.makedirs(os.path.dirname(dest), exist_ok=True)
    with z.open(i) as src, open(dest, "wb") as dst:
        dst.write(src.read())
    n += 1
with open(os.path.join(out, ".provenance.tsv"), "w") as f:
    f.write("*.stp\thttps://www.nist.gov/document/nist-pmi-step-files\tNIST, public domain\n")
print(f"fetch-pmi: {n} STEP files in {out}")
PY
