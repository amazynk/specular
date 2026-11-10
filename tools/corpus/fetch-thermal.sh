#!/usr/bin/env bash
# Fetch the thermal-still test corpus into tools/corpus/thermal/ (gitignored: nothing here is
# committed) and write the independent readings of it (the oracle) to expected.tsv beside it.
#
# Usage: tools/corpus/fetch-thermal.sh
#   OUT defaults to $SPECULAR_THERMAL_CORPUS, else tools/corpus/thermal.
#   PYTHON (default python3) must have numpy, Pillow and infiray_irg for the oracle; a venv
#   under target/ is fine: python3 -m venv target/thermal/venv &&
#   target/thermal/venv/bin/pip install --no-cache-dir numpy pillow infiray_irg; then
#   PYTHON=target/thermal/venv/bin/python. exiftool reads the FLIR files; java and javac
#   (with the `irb` reader's sources and gson, fetched below) read the InfraTec files.
#   Then: SPECULAR_THERMAL_DIR=tools/corpus/thermal
#   SPECULAR_THERMAL_ORACLE=tools/corpus/thermal/expected.tsv
#   cargo test -p specular-fmt-thermal --test oracle corpus_matches_the_oracles -- --nocapture
#   and SPECULAR_THERMAL_DIR=tools/corpus/thermal cargo test -p specular-decode
#   thermal_corpus_timings -- --nocapture for open and first-frame timings.
#
# What it covers (about 21 MB):
#   flir/   R-JPEGs: a FLIR AX8 (PNG raw data, 80 x 60), a FLIR camera with the Iron palette (PNG,
#           320 x 240) and a DJI Zenmuse XT-R (raw counts, 640 x 512) from FlirImageExtractor's
#           examples (MIT); a FLIR SC660 (raw counts, 640 x 480) from the Thermimage R package's
#           example data (GPL-3 package; the image is only read)
#   irg/    an InfiRay C201 file from infiray_irg (MIT)
#   irb/    InfraTec VarioCAM and ImageIR stills, uncompressed and delta-coded, from the `irb`
#           reader's test data (Apache-2.0)
#
# Downloads are untrusted data: this script only copies them, except the `irb` Java reader
# (sources at a pinned commit) and gson (checksummed), which are compiled and run as the
# oracle under OUT/.oracle. Stops if free space drops below MIN_FREE_GB (default 8).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_THERMAL_CORPUS:-$ROOT/tools/corpus/thermal}
MIN_FREE_GB=${MIN_FREE_GB:-8}
PYTHON=${PYTHON:-python3}
FIE=abf48d359fddd42c3165bc6cd6caac2ad8304853
THERMIMAGE=cb244dac8fffaac50b1ba8bbbf58b7d37b5d96e1
IRG=eb0f779403665012b3b29eb3b849cdc4a6d84ae1
IRB=845fe5bd9beff99d360b7d3a752731ebd5eb217f
GSON=https://repo1.maven.org/maven2/com/google/code/gson/gson/2.10.1/gson-2.10.1.jar
RAW=https://raw.githubusercontent.com
mkdir -p "$OUT/flir" "$OUT/irg" "$OUT/irb" "$OUT/.oracle"
: > "$OUT/.provenance.tsv"

die() { echo "fetch-thermal: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

# get URL DEST LICENCE [MD5]
get() {
  local url=$1 dest=$2 lic=$3 md5=${4:-}
  check_space
  if [[ ! -s "$OUT/$dest" ]]; then
    mkdir -p "$(dirname "$OUT/$dest")"
    curl -fsSL --retry 3 -o "$OUT/$dest.part" "$url" || die "download failed: $url"
    if [[ -n $md5 ]]; then
      [[ $(md5sum "$OUT/$dest.part" | cut -d' ' -f1) == "$md5" ]] || die "checksum mismatch: $url"
    fi
    mv "$OUT/$dest.part" "$OUT/$dest"
  fi
  printf '%s\t%s\t%s\n' "$dest" "$url" "$lic" >> "$OUT/.provenance.tsv"
}

E="$RAW/nationaldronesau/FlirImageExtractor/$FIE/examples"
get "$E/ax8.jpg" flir/ax8.jpg "MIT (FlirImageExtractor)" 4bf464fc1a6ededfe5760c5e347c987c
get "$E/flir_example.jpg" flir/flir_example.jpg "MIT (FlirImageExtractor)" \
    991e6522e50547ce1d32fb107ef3c009
get "$E/zenmuse_xtr.jpg" flir/zenmuse_xtr.jpg "MIT (FlirImageExtractor)" \
    9df2ec0589e6cddbe740677ecb057a21
get "$RAW/gtatters/Thermimage/$THERMIMAGE/inst/extdata/IR_2412.jpg" flir/IR_2412.jpg \
    "GPL-3 package data (Thermimage)" 44151b6294d270eb42e46c02c04436b4
get "$RAW/jaseg/infiray_irg/$IRG/example.irg" irg/example.irg "MIT (infiray_irg)" \
    456a574155bb63acdf378c544c5bd647

T="$RAW/jonathanschilling/irb/$IRB/src/test/resources/de/labathome/irb"
while read -r md5 f; do
  get "$T/$f" "irb/$f" "Apache-2.0 (irb test data)" "$md5"
done <<'LIST'
3ce223b55383e78ddd1c0cbbfa002844 140114AA/AA011400.irb
4da96406bb571a9f1ea9730ed02339fa 140114AA/AA011401.irb
4a6574416471593bfee3e46692b6b289 140115AA/AA011500.irb
b77d796a394e9c35adf9656ecd2a2892 140115AA/AA011501.irb
a24926df076804b1e15f6bd6e0ba7fed 140115AA/AA011502.irb
9c2f567346a0f3dcbfed935f042a6484 140115AA/AA011503.irb
7ecdf55c393bb7826f3e9e402f3b5a17 140202AA/AA020200.irb
235f614dd4379cb26ab69dc982571b4d 140203AA/AA020300.irb
e62e385230f3a4fdfa7daf49ad44ace4 140203AA/AA020301.irb
4e1c9261e9b4e7b29b4f5158bed2821a 140203AA/AA020302.irb
d0fbe1724db4f5dd7b06e7a9f0ae9145 140203AA/AA020303.irb
9a3b9a62281ea922c8fd1067a89022d1 140203AA/AA020304.irb
69fdf64135be7d01875b9d7bec9cdb61 140203AB/AB020300.irb
75ca2dc838cdf9bb1622993277c338be 140203AB/AB020301.irb
3a685b18645d49da0060ef20e0e2a985 140203AB/AB020302.irb
15e1dc9470f4a382a9e0992736687229 image.irb
LIST

# The IRB oracle: the `irb` reader's own sources at the pinned commit, and gson for its
# annotations, compiled under OUT/.oracle.
O="$OUT/.oracle"
J="$RAW/jonathanschilling/irb/$IRB/src/main/java/de/labathome/irb"
mkdir -p "$O/src" "$O/classes" "$O/dump"
for c in IrbBlockType IrbFile IrbFileHeader IrbFileType IrbFrameHeader IrbHeader IrbHeaderBlock \
         IrbImage IrbPreview IrbTextInfo; do
  [[ -s "$O/src/$c.java" ]] || curl -fsSL --retry 3 -o "$O/src/$c.java" "$J/$c.java" \
    || die "download failed: $J/$c.java"
done
[[ -s "$O/gson.jar" ]] || curl -fsSL --retry 3 -o "$O/gson.jar" "$GSON" || die "download failed: $GSON"
[[ $(sha1sum "$O/gson.jar" | cut -d' ' -f1) == b3add478d4382b78ea20b1671390a858002feb6c ]] \
  || die "gson checksum mismatch"
javac -nowarn -d "$O/classes" -cp "$O/gson.jar" "$O"/src/*.java

GEN="$ROOT/crates/formats/specular-fmt-thermal/gen/oracle.py"
# Every 37th pixel (a prime, so the samples walk across columns): ~10 MB of readings, not 270.
THERMAL_ORACLE_STRIDE=${THERMAL_ORACLE_STRIDE:-37} "$PYTHON" -I "$GEN" "$OUT" "$OUT/expected.tsv" "$O/classes:$O/gson.jar" "$O/dump"
du -sh "$OUT"
