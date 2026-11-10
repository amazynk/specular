#!/usr/bin/env bash
# Build the JPEG XR test corpus in $SPECULAR_LARGE_CORPUS/jxr (default tools/corpus/large/jxr;
# gitignored, nothing here is committed). The specular-jxr large-corpus test reads it from
# there and skips when it is absent.
#
# Usage: tools/corpus/fetch-jxr.sh [SECTION...]
#   SECTION is a function name below (src_kde, generate, oracle). Default: all, in that order.
#
# * src_kde: the JPEG XR files of KDE kimageformats' autotests (other encoders than jxrlib's
#   command line tools: metadata, all eight orientations, CMYK, grey, mono, 16-bit, alpha).
# * generate: 24 MP (6000x4000) files encoded by JxrEncApp from a crop of the large corpus's
#   aerial photo (tiff/Dop20rgb_*.tif): lossy RGB (the bench file), lossless, 4:2:0, half
#   float, tiled BGRA with interleaved alpha, and an 8000x8000 grey crop of the CAPELLA TIFF.
# * oracle: decodes every .jxr with JxrDecApp (jxrlib, the oracle only) and writes
#   manifest.txt: name, width, height, bytes per pixel and the FNV-1a 64 hash of the native
#   samples, the same convention as crates/formats/specular-jxr/tests/vectors.
#
# Needs curl, python3 with numpy and Pillow, and jxrlib's JxrEncApp / JxrDecApp. Downloads are
# untrusted data: this script only copies them and decodes them with JxrDecApp; nothing from
# them is executed. Scratch space is target/agent-corpus/jxr-fetch (deleted afterwards). Stops
# if free space drops below MIN_FREE_GB (default 6).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
LARGE=${SPECULAR_LARGE_CORPUS:-$ROOT/tools/corpus/large}
OUT=$LARGE/jxr
SCRATCH="$ROOT/target/agent-corpus/jxr-fetch"
GEN="$ROOT/crates/formats/specular-jxr/gen"
MIN_FREE_GB=${MIN_FREE_GB:-6}
mkdir -p "$OUT" "$SCRATCH"

die() { echo "fetch-jxr: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

# provenance RELPATH SOURCE LICENCE
provenance() { printf '%s\t%s\t%s\n' "$1" "$2" "$3" >> "$OUT/.provenance.tsv"; }

# ------------------------------------------------------------------------------- sections

src_kde() {
  local commit=5dafbe2d2f81e9a17712c63e2e11345af5f2b4d5 n=0 f
  local base="https://raw.githubusercontent.com/KDE/kimageformats/$commit/autotests/read/jxr"
  check_space
  mkdir -p "$OUT/kde"
  for f in abydos_bgra32 metadata orientation1 orientation2 orientation3 orientation4 \
           orientation5 orientation6 orientation7 orientation8 testcard_bgra8 testcard_cmyk8 \
           testcard_gray16 testcard_gray8 testcard_mono testcard_rgb16 testcard_rgb8 \
           testcard_rgba16 testcard_rgba8; do
    curl -fsSL --retry 3 -o "$OUT/kde/$f.jxr" "$base/$f.jxr"
    provenance "kde/$f.jxr" "$base/$f.jxr" 'kimageformats autotests (LGPL-2.1+ project; test data)'
    n=$((n + 1))
  done
  echo "  kimageformats: $n files -> kde"
}

generate() {
  command -v JxrEncApp >/dev/null || die "JxrEncApp not found"
  local photo gray
  photo=$(ls "$LARGE"/tiff/Dop20rgb_*.tif 2>/dev/null | head -1)
  gray=$(ls "$LARGE"/tiff/Sunny_Isles_Beach_CAPELLA_*.tif* 2>/dev/null | head -1)
  [[ -n $photo ]] || die "no tiff/Dop20rgb_*.tif in $LARGE (the large corpus photo)"
  check_space
  mkdir -p "$OUT/gen"
  python3 -I - "$GEN" "$photo" "${gray:-}" "$SCRATCH" <<'EOF'
import sys
import numpy as np
from PIL import Image
sys.path.insert(0, sys.argv[1])
import oracle
Image.MAX_IMAGE_PIXELS = None
photo, gray, scratch = sys.argv[2], sys.argv[3], sys.argv[4]
rgb = np.asarray(Image.open(photo).convert("RGB"))[1000:5000, 2000:8000]
oracle.write_tiff(f"{scratch}/rgb8.tif", np.ascontiguousarray(rgb), 2)
a = np.linspace(0, 255, rgb.shape[1], dtype=np.float32)[None, :] * np.ones((rgb.shape[0], 1), np.float32)
rgba = np.dstack([rgb, a.astype(np.uint8)])
oracle.write_tiff(f"{scratch}/rgba8.tif", np.ascontiguousarray(rgba), 2, extra=2)
lin = ((rgb.astype(np.float32) / 255.0) ** 2.2 * 4.0).astype(np.float32)  # scene-referred HDR
oracle.write_tiff(f"{scratch}/rgbf.tif", np.ascontiguousarray(lin), 2, sample_format=3)
if gray:
    g = np.asarray(Image.open(gray))[6000:14000, 6000:14000]
    oracle.write_tiff(f"{scratch}/gray8.tif", np.ascontiguousarray(g[..., None]), 1)
EOF
  local enc=(JxrEncApp)
  "${enc[@]}" -i "$SCRATCH/rgb8.tif" -o "$OUT/gen/photo24-q.jxr" -c 9 -q 0.85 >/dev/null
  "${enc[@]}" -i "$SCRATCH/rgb8.tif" -o "$OUT/gen/photo24-lossless.jxr" -c 9 -q 1 >/dev/null
  "${enc[@]}" -i "$SCRATCH/rgb8.tif" -o "$OUT/gen/photo24-420-l2.jxr" -c 9 -d 1 -l 2 -q 0.8 >/dev/null
  "${enc[@]}" -i "$SCRATCH/rgb8.tif" -o "$OUT/gen/photo24-spatial.jxr" -c 9 -f -q 0.85 >/dev/null
  "${enc[@]}" -i "$SCRATCH/rgbf.tif" -o "$OUT/gen/photo24-half.jxr" -c 12 -q 0.9 >/dev/null
  "${enc[@]}" -i "$SCRATCH/rgba8.tif" -o "$OUT/gen/photo24-tiles-alpha.jxr" -c 22 -U 4 4 -a 3 -Q 2 -q 0.85 >/dev/null
  if [[ -f $SCRATCH/gray8.tif ]]; then
    "${enc[@]}" -i "$SCRATCH/gray8.tif" -o "$OUT/gen/gray64-q.jxr" -c 2 -q 0.9 >/dev/null
  fi
  for f in "$OUT"/gen/*.jxr; do
    provenance "gen/$(basename "$f")" "JxrEncApp from $(basename "$photo") / $(basename "${gray:-none}")" 'see source photo'
  done
  echo "  generated: $(ls "$OUT"/gen/*.jxr | wc -l) files -> gen"
}

oracle() {
  command -v JxrDecApp >/dev/null || die "JxrDecApp not found"
  python3 -I - "$GEN" "$OUT" "$SCRATCH" <<'EOF'
import glob, os, subprocess, sys
import numpy as np
sys.path.insert(0, sys.argv[1])
import tiffraw  # noqa: E402
from make_vectors import fnv
out, scratch = sys.argv[2], sys.argv[3]
lines = ["# path width height bytes-per-pixel fnv1a64(native samples, little-endian) - fetch-jxr.sh"]
for f in sorted(glob.glob(f"{out}/**/*.jxr", recursive=True)):
    rel = os.path.relpath(f, out)
    tif = f"{scratch}/o.tif"
    if os.path.exists(tif):
        os.remove(tif)
    r = subprocess.run(["JxrDecApp", "-i", f, "-o", tif], capture_output=True)
    if r.returncode != 0 or not os.path.exists(tif):
        print(f"  oracle failed: {rel}")
        continue
    tags, raw = tiffraw.read(tif)
    w, h = tags[256][0], tags[257][0]
    if tags.get(258, [8])[0] == 1:
        raw = np.unpackbits(np.frombuffer(raw, np.uint8).reshape(h, -1), axis=1)[:, :w].tobytes()
    lines.append(f"{rel} {w} {h} {len(raw) // (w * h)} {fnv(raw):016x}")
    print("  " + lines[-1])
with open(f"{out}/manifest.txt", "w") as m:
    m.write("\n".join(lines) + "\n")
EOF
}

main() {
  local sections=() s
  for a in "$@"; do
    case $a in
      -h|--help) sed -n '2,24p' "$0"; exit 0 ;;
      *) sections+=("$a") ;;
    esac
  done
  (( ${#sections[@]} )) || sections=(src_kde generate oracle)
  for s in "${sections[@]}"; do
    declare -F "$s" >/dev/null || die "unknown section $s"
    echo "== $s"
    "$s"
  done
  if [[ -f $OUT/.provenance.tsv ]]; then
    tac "$OUT/.provenance.tsv" | awk -F'\t' '!seen[$1]++' | tac > "$OUT/.provenance.tmp"
    mv "$OUT/.provenance.tmp" "$OUT/.provenance.tsv"
  fi
  rm -rf "$SCRATCH"
  du -sh "$OUT"
}
main "$@"
