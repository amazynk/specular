#!/usr/bin/env bash
# Fetch the CERN ROOT test corpus into tools/corpus/root/ (gitignored: nothing here is committed)
# and write uproot's reading of it (the oracle) to expected.tsv beside it.
#
# Usage: tools/corpus/fetch-root.sh
#   OUT defaults to $SPECULAR_ROOT_CORPUS, else tools/corpus/root.
#   PYTHON (default python3) must have uproot and numpy for the oracle (lz4, xxhash and
#   zstandard for those compressions); a venv under target/ is fine:
#   python3 -m venv target/root/venv &&
#   target/root/venv/bin/pip install --no-cache-dir uproot numpy lz4 xxhash zstandard;
#   then PYTHON=target/root/venv/bin/python.
#   Then: SPECULAR_ROOT_DIR=tools/corpus/root SPECULAR_ROOT_ORACLE=tools/corpus/root/expected.tsv
#   cargo test -p specular-root --test oracle corpus_matches_uproot
#
# What it covers (about 200 MB):
#   skhep/     every ROOT file of scikit-hep-testdata (BSD-3-Clause) at a pinned version no
#              larger than MAX_FILE_MB: uproot's histogram files (TH1/TH2/TH3, TProfile,
#              variable bins, labels), the HZZ and Zmumu trees in zlib, LZ4, LZMA and ZSTD, the
#              uproot-sample files written by ROOT 5.23 to 6.20 (uncompressed, zlib, LZMA, LZ4),
#              nested directories, RNTuples (listed, not read) and the issue files
#   tutorials/ ROOT's hsimple.root (hpx, hpxpy, hprof and the ntuple), from root.cern
#
# Downloads are untrusted data: this script only copies files, never runs anything from them.
# Stops if free space on the target filesystem drops below MIN_FREE_GB (default 8).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_ROOT_CORPUS:-$ROOT/tools/corpus/root}
MIN_FREE_GB=${MIN_FREE_GB:-8}
MAX_FILE_MB=${MAX_FILE_MB:-40}
PYTHON=${PYTHON:-python3}
TESTDATA=v0.6.8
BASE=https://raw.githubusercontent.com/scikit-hep/scikit-hep-testdata/$TESTDATA/src/skhep_testdata/data
[[ -n $OUT ]] || exit 1
mkdir -p "$OUT/skhep" "$OUT/tutorials"
: > "$OUT/.provenance.tsv"

die() { echo "fetch-root: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

# get URL DEST LICENCE: skipped when missing upstream, larger than MAX_FILE_MB or not a ROOT
# file (an HTML error page).
get() {
  local url=$1 dest=$2 lic=$3 len part
  check_space
  part="$OUT/$dest.part"
  if [[ ! -s "$OUT/$dest" ]]; then
    len=$(curl -sIL --retry 3 "$url" | tr -d '\r' \
      | awk 'tolower($1)=="content-length:"{n=$2} END{print n+0}')
    if (( len > MAX_FILE_MB * 1024 * 1024 )); then
      echo "fetch-root: skipping $dest ($((len / 1024 / 1024)) MB)" >&2
      return 0
    fi
    if ! curl -fsSL --retry 3 -o "$part" "$url"; then
      [[ -f $part ]] && unlink "$part"
      echo "fetch-root: missing $url" >&2
      return 0
    fi
    if [[ $(head -c 4 "$part") != root ]]; then
      unlink "$part"
      echo "fetch-root: not a ROOT file: $url" >&2
      return 0
    fi
    mv "$part" "$OUT/$dest"
  fi
  printf '%s\t%s\t%s\n' "$dest" "$url" "$lic" >> "$OUT/.provenance.tsv"
}

for f in \
  Run2012BC_DoubleMuParked_Muons_1000evts_rntuple_v1-0-0-0.root \
  cmsopendata2015_ttbar_19980_NANOAOD_RNTupleImporter_rntuple_v1-0-0-1.root issue367b.root \
  nanoAOD_2015_CMS_Open_Data_ttbar.root ntpl001_staff_rntuple_v1-0-0-0.root \
  ntpl001_staff_rntuple_v1-0-1-0.root rntviewer-testfile-multiple-rntuples-v1-0-0-0.root \
  rntviewer-testfile-uncomp-single-rntuple-v1-0-0-0.root string-example.root \
  test_1jag_int_float_rntuple_v1-0-0-0.root test_atomic_bitset_rntuple_v1-0-0-0.root \
  test_bit_rntuple_v1-0-0-0.root test_class_inheritance_rntuple_v1-0-0-1.root \
  test_emptystruct_invalidvar_rntuple_v1-0-0-0.root test_extension_columns_rntuple_v1-0-0-0.root \
  test_float_types_rntuple_v1-0-0-0.root test_index_multicluster_rntuple_v1-0-0-0.root \
  test_int_5e4_rntuple_v1-0-0-0.root test_int_float_rntuple_v1-0-0-0.root \
  test_int_multicluster_rntuple_v1-0-0-0.root test_int_vfloat_tlv_vtlv_rntuple_v1-0-0-0.root \
  test_multiple_cluster_groups_rntuple_v1-0-0-0.root \
  test_multiple_representations_rntuple_v1-0-0-0.root \
  test_nested_structs_rntuple_v1-0-0-0.root test_split_3e4_rntuple_v1-0-0-0.root \
  test_splitint_rntuple_v1-0-1-0.root test_stl_containers_rntuple_v1-0-0-0.root uproot-Event.root \
  uproot-FCCDelphesOutput.root uproot-HZZ-lz4.root uproot-HZZ-lzma.root uproot-HZZ-objects.root \
  uproot-HZZ-uncompressed.root uproot-HZZ-zlib.root uproot-HZZ-zstd.root uproot-HZZ.root \
  uproot-Zmumu-lz4.root uproot-Zmumu-lzma.root uproot-Zmumu-uncompressed.root \
  uproot-Zmumu-zlib.root uproot-Zmumu-zstd.root uproot-Zmumu.root uproot-delphes-pr442.root \
  uproot-demo-double32.root uproot-double32-float16.root uproot-empty.root uproot-foriter.root \
  uproot-foriter2.root uproot-from-geant4.root uproot-hepdata-example.root \
  uproot-histograms.root uproot-issue-1043.root uproot-issue-1070.root uproot-issue-1221.root \
  uproot-issue-1229.root uproot-issue-123a.root uproot-issue-123b.root uproot-issue-1275.root \
  uproot-issue-1502.root uproot-issue-1610.root uproot-issue-172.root uproot-issue-181.root \
  uproot-issue-208.root uproot-issue-210.root uproot-issue-214.root uproot-issue-222.root \
  uproot-issue-227a.root uproot-issue-227b.root uproot-issue-240.root uproot-issue-250.root \
  uproot-issue-268.root uproot-issue-275-nosplit.root uproot-issue-275-split1.root \
  uproot-issue-308.root uproot-issue-350.root uproot-issue-359.root uproot-issue-398.root \
  uproot-issue-40.root uproot-issue-400.root uproot-issue-407.root uproot-issue-418.root \
  uproot-issue-513.root uproot-issue-569.root uproot-issue-586.root uproot-issue-589.root \
  uproot-issue-60.root uproot-issue-607.root uproot-issue-60a.root uproot-issue-60b.root \
  uproot-issue-643.root uproot-issue-697.root uproot-issue-707.root uproot-issue-722.root \
  uproot-issue-74.root uproot-issue-75.root uproot-issue-750.root uproot-issue-76.root \
  uproot-issue-791.root uproot-issue-798.root uproot-issue-816.root uproot-issue-85.root \
  uproot-issue-861.root uproot-issue-880.root uproot-issue-888.root uproot-issue-951.root \
  uproot-issue-tbranch-of-th2.root uproot-issue121.root uproot-issue124.root \
  uproot-issue126a.root uproot-issue126b.root uproot-issue187.root uproot-issue209.root \
  uproot-issue21.root uproot-issue213.root uproot-issue232.root uproot-issue243-new.root \
  uproot-issue243.root uproot-issue261.root uproot-issue283.root uproot-issue30.root \
  uproot-issue31.root uproot-issue327.root uproot-issue33.root uproot-issue335.root \
  uproot-issue367.root uproot-issue367b.root uproot-issue371.root uproot-issue38a.root \
  uproot-issue38b.root uproot-issue38c.root uproot-issue390.root uproot-issue399.root \
  uproot-issue403.root uproot-issue404.root uproot-issue407.root uproot-issue408.root \
  uproot-issue412.root uproot-issue413.root uproot-issue429.root uproot-issue431.root \
  uproot-issue431b.root uproot-issue433-splitlevel2.root uproot-issue433-splitlevel4.root \
  uproot-issue434.root uproot-issue443.root uproot-issue452.root uproot-issue46.root \
  uproot-issue465-flat.root uproot-issue465-nested.root uproot-issue468.root \
  uproot-issue475.root uproot-issue475b.root uproot-issue485.root uproot-issue486.root \
  uproot-issue49.root uproot-issue494.root uproot-issue498.root uproot-issue501.root \
  uproot-issue510b.root uproot-issue513.root uproot-issue519.root uproot-issue57.root \
  uproot-issue60.root uproot-issue63.root uproot-issue64.root uproot-issue66.root \
  uproot-issue70.root uproot-issue74.root uproot-issue76.root uproot-issue79.root \
  uproot-issue96.root uproot-leaflist.root uproot-mc10events.root \
  uproot-metadata-performance.root uproot-nesteddirs.root \
  uproot-physlite-rntuple_v1-0-0-0.root uproot-sample-5.23.02-uncompressed.root \
  uproot-sample-5.23.02-zlib.root uproot-sample-5.24.00-uncompressed.root \
  uproot-sample-5.24.00-zlib.root uproot-sample-5.25.02-uncompressed.root \
  uproot-sample-5.25.02-zlib.root uproot-sample-5.26.00-uncompressed.root \
  uproot-sample-5.26.00-zlib.root uproot-sample-5.27.02-uncompressed.root \
  uproot-sample-5.27.02-zlib.root uproot-sample-5.28.00-uncompressed.root \
  uproot-sample-5.28.00-zlib.root uproot-sample-5.29.02-uncompressed.root \
  uproot-sample-5.29.02-zlib.root uproot-sample-5.30.00-lzma.root \
  uproot-sample-5.30.00-uncompressed.root uproot-sample-5.30.00-zlib.root \
  uproot-sample-6.08.04-lzma.root uproot-sample-6.08.04-uncompressed.root \
  uproot-sample-6.08.04-zlib.root uproot-sample-6.10.05-lz4.root uproot-sample-6.10.05-lzma.root \
  uproot-sample-6.10.05-uncompressed.root uproot-sample-6.10.05-zlib.root \
  uproot-sample-6.14.00-lz4.root uproot-sample-6.14.00-lzma.root \
  uproot-sample-6.14.00-uncompressed.root uproot-sample-6.14.00-zlib.root \
  uproot-sample-6.16.00-lz4.root uproot-sample-6.16.00-lzma.root \
  uproot-sample-6.16.00-uncompressed.root uproot-sample-6.16.00-zlib.root \
  uproot-sample-6.18.00-lz4.root uproot-sample-6.18.00-lzma.root \
  uproot-sample-6.18.00-uncompressed.root uproot-sample-6.18.00-zlib.root \
  uproot-sample-6.20.04-lz4.root uproot-sample-6.20.04-lzma.root \
  uproot-sample-6.20.04-uncompressed.root uproot-sample-6.20.04-zlib.root uproot-simple.root \
  uproot-small-dy-nooffsets.root uproot-small-dy-withoffsets.root \
  uproot-small-evnt-tree-fullsplit.root uproot-small-evnt-tree-nosplit.root \
  uproot-small-flat-tree.root uproot-stl_containers.root uproot-vectorVectorDouble.root \
  ; do
  get "$BASE/$f" "skhep/$f" "BSD-3-Clause (scikit-hep-testdata $TESTDATA)"
done

get https://root.cern/files/tutorials/hsimple.root tutorials/hsimple.root \
  "LGPL-2.1-or-later (output of the ROOT tutorial hsimple.C, test data only)"

# The oracle: uproot's reading of every file, paths relative to OUT.
"$PYTHON" -I "$ROOT/crates/formats/specular-root/gen/oracle.py" "$OUT" "$OUT/expected.tsv" \
  || die "oracle failed (does PYTHON have uproot and numpy?)"
du -sh "$OUT"
