#!/usr/bin/env bash
# Fetch the FITS table corpus (HEALPix maps, gravitational-wave sky maps, X-ray event lists,
# VOTables) into tools/corpus/fitstables/ (gitignored: nothing here is committed), and write
# its oracle values. The tests read it in place from SPECULAR_FITSTABLES_CORPUS (a worktree
# has no corpus of its own), else from tools/corpus/fitstables, and skip when it is absent.
#
# Usage: tools/corpus/fetch-fitstables.sh
#   OUT defaults to $SPECULAR_FITSTABLES_CORPUS, else tools/corpus/fitstables.
#   PYTHON (default python3) must have numpy, astropy and astropy-healpix for the oracle. A
#   venv under target/ is fine: python3 -m venv target/fitstables/venv &&
#   target/fitstables/venv/bin/pip install --no-cache-dir astropy astropy-healpix;
#   then PYTHON=target/fitstables/venv/bin/python.
#
# What it holds (about 200 MB unpacked):
#   planck-freefree-0256.fits   Planck PR2 Commander free-free map, Nside 256 (IRSA)
#   S200115j-multiorder.fits    LIGO/Virgo multi-order sky map (UNIQ / PROBDENSITY), GraceDB
#   S200115j-flat.fits          the same event's BAYESTAR flat sky map, Nside 512 NESTED (PROB, DISTMU…)
#   chandra-635-evt2.fits       Chandra ACIS level-2 event list, ObsID 635 (HEASARC mirror)
#   xmm-0123700101-m1.fits      XMM-Newton EPIC MOS1 event list, ObsID 0123700101 (HEASARC)
#   vizier.vot                  VizieR result with hundreds of TABLEs (astroquery test data)
#   gaia-binary2.vot            Gaia archive BINARY2 result (astroquery test data)
#   expected.tsv                the oracle (crates/formats/specular-fits/gen/fitstables_oracle.py)
#
# Every download is pinned by SHA-256. Downloads are untrusted data: this script only copies
# and gunzips them, never runs anything from them. Scratch space is
# target/corpus/fitstables-fetch (deleted afterwards). Stops if free space on the target
# filesystem drops below MIN_FREE_GB (default 8).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_FITSTABLES_CORPUS:-$ROOT/tools/corpus/fitstables}
SCRATCH="$ROOT/target/corpus/fitstables-fetch"
MIN_FREE_GB=${MIN_FREE_GB:-8}
PYTHON=${PYTHON:-python3}
mkdir -p "$OUT" "$SCRATCH"

die() { echo "fetch-fitstables: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

# provenance RELPATH URL LICENCE
provenance() { printf '%s\t%s\t%s\n' "$1" "$2" "$3" >> "$OUT/.provenance.tsv"; }

# fetch NAME URL SHA256 LICENCE [gz]
# Downloads URL (checking the SHA-256 of what is downloaded) to $OUT/NAME, gunzipping when
# the fifth argument is "gz". Skips files already present.
fetch() {
  local name=$1 url=$2 sum=$3 lic=$4 gz=${5:-}
  if [[ -s "$OUT/$name" ]]; then
    provenance "$name" "$url" "$lic"
    echo "  $name: present"
    return
  fi
  check_space
  local tmp="$SCRATCH/$name.download"
  curl -fsSL --retry 3 --max-time 1800 -o "$tmp" "$url"
  local got
  got=$(sha256sum "$tmp" | awk '{print $1}')
  [[ "$got" == "$sum" ]] || die "$name: SHA-256 $got, expected $sum ($url)"
  if [[ "$gz" == gz ]]; then
    gzip -dc "$tmp" > "$OUT/$name.part"
    rm -f "$tmp"
  else
    mv "$tmp" "$OUT/$name.part"
  fi
  mv "$OUT/$name.part" "$OUT/$name"
  provenance "$name" "$url" "$lic"
  echo "  $name: $(du -h "$OUT/$name" | awk '{print $1}')"
}

echo "fetch-fitstables -> $OUT"
: > "$OUT/.provenance.tsv"

fetch planck-freefree-0256.fits \
  "https://irsa.ipac.caltech.edu/data/Planck/release_2/all-sky-maps/maps/component-maps/foregrounds/COM_CompMap_freefree-commander_0256_R2.00.fits" \
  eafb25208b08511e39deb4e432da126ea1e6d930ede0b81ea4a35caf084db3e3 "Planck Legacy Archive (ESA), public"
fetch S200115j-multiorder.fits \
  "https://gracedb.ligo.org/api/v2/superevents/S200115j/files/LALInference.multiorder.fits,0" \
  bee712c435c398243bdcc5ddd80343de7e719cdc6a8e958a2f5ad6865f8d518f "LIGO/Virgo public alert products, CC BY 4.0"
fetch S200115j-flat.fits \
  "https://gracedb.ligo.org/api/v2/superevents/S200115j/files/bayestar.fits.gz,0" \
  75bc3f018f1e97c75483a577be8026add24aefacfc13cf7441536cebc3eb3d19 "LIGO/Virgo public alert products, CC BY 4.0" gz
fetch chandra-635-evt2.fits \
  "https://heasarc.gsfc.nasa.gov/FTP/chandra/data/byobsid/5/635/primary/acisf00635N005_evt2.fits.gz" \
  5d15229e80e45fb17a17833e14840d50d3a024660d077d9a916c9a4b655db657 "Chandra Data Archive (CXC), public" gz
fetch xmm-0123700101-m1.fits \
  "https://heasarc.gsfc.nasa.gov/FTP/xmm/data/rev0/0123700101/PPS/P0123700101M1S001MIEVLI0000.FTZ" \
  3d4249acdebc57d96ee357a99859a6368699449fb843f1f815ede89ef0715342 "XMM-Newton Science Archive (ESA), public" gz
fetch vizier.vot \
  "https://raw.githubusercontent.com/astropy/astroquery/v0.4.7/astroquery/vizier/tests/data/viz.xml" \
  cbc1b27524e215d08e433b83bf26cb4dc13f62a2beaeba53c474c5e45ad1c378 "astroquery test data, BSD-3-Clause"
fetch gaia-binary2.vot \
  "https://raw.githubusercontent.com/astropy/astroquery/v0.4.7/astroquery/gaia/tests/data/job_1.vot" \
  897556f83b5afd5d8eb736323cdbd99ab217d2bf42bbc06aa8552e7d22751c61 "astroquery test data, BSD-3-Clause"

rm -rf "$SCRATCH"

if "$PYTHON" -I -c "import astropy, astropy_healpix, numpy" 2>/dev/null; then
  "$PYTHON" -I "$ROOT/crates/formats/specular-fits/gen/fitstables_oracle.py" "$OUT"
else
  echo "fetch-fitstables: $PYTHON lacks astropy / astropy-healpix; no oracle written" >&2
fi
echo "done: $(du -sh "$OUT" | awk '{print $1}')"
