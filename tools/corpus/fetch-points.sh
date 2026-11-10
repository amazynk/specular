#!/usr/bin/env bash
# Fetch the point cloud test corpus into tools/corpus/points/ (gitignored: nothing here is
# committed). The specular-points corpus test reads it in place from SPECULAR_POINTS_CORPUS (a
# worktree has no corpus of its own), else from tools/corpus/points, and skips when it is absent.
#
# Usage: tools/corpus/fetch-points.sh [fetch] [oracle]
#   Default: both. OUT defaults to $SPECULAR_POINTS_CORPUS, else tools/corpus/points.
#   PYTHON (default python3) needs laspy with lazrs, and pye57, for the oracle only. A venv
#   under target/ is fine: python3 -m venv target/points-venv && target/points-venv/bin/pip
#   install --no-cache-dir 'laspy[lazrs]' pye57; then PYTHON=target/points-venv/bin/python.
#
# What it covers (about 160 MB), every file pinned by commit (or, for the one S3 sample, by
# URL) and checked against its size:
#   pdal      PDAL's test data: LAS 1.0-1.4 in every common point format, extra bytes, many
#             VLRs, GeoTIFF keys and WKT, broken headers and VLR counts, LAZ from LASzip,
#             laz-perf and libLAS (with padding), small COPC files, a 10 MB LAZ
#   laz-perf  laz-perf's raw sets: LAS and LAZ twins of point10, colour, GPS time and 1.4 data
#   laszip    LASzip's deliberately broken LAZ headers and a 1.4 example
#   e57       pye57's test files (libE57 samples): cartesian and spherical scans, poses,
#             normals, an embedded image
#   pcd       PCL's test clouds: ASCII, binary and binary_compressed, organised and not,
#             with RGB, intensity, normals and labels
#   copc      the Autzen COPC sample (10.6 M points, LAZ 1.4 v3) for LOD and timing
#   oracle/   laspy+lazrs and pye57 readings: counts, bounds and sampled points
#             (expected.tsv and one samples file per input; crates/formats/specular-points/gen)
#
# Downloads are untrusted data: this script only copies files, never runs anything from them
# (the oracle reads them with laspy and pye57, from a script outside the corpus, with -I).
# Stops if free space on the target filesystem drops below MIN_FREE_GB (default 6).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_POINTS_CORPUS:-$ROOT/tools/corpus/points}
MIN_FREE_GB=${MIN_FREE_GB:-6}
PYTHON=${PYTHON:-python3}
mkdir -p "$OUT"

die() { echo "fetch-points: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

# DEST<TAB>SIZE<TAB>URL<TAB>LICENCE
MANIFEST=$(cat <<'END'
pdal/las/1.2-with-color.las	36439	https://raw.githubusercontent.com/PDAL/PDAL/64c11506cccdf790b9677458d5ee884bdf4e754b/test/data/las/1.2-with-color.las	BSD-3-Clause (PDAL test data)
pdal/las/100-points.las	3627	https://raw.githubusercontent.com/PDAL/PDAL/64c11506cccdf790b9677458d5ee884bdf4e754b/test/data/las/100-points.las	BSD-3-Clause (PDAL test data)
pdal/las/4_1.las	5572238	https://raw.githubusercontent.com/PDAL/PDAL/64c11506cccdf790b9677458d5ee884bdf4e754b/test/data/las/4_1.las	BSD-3-Clause (PDAL test data)
pdal/las/4_6.las	5971566	https://raw.githubusercontent.com/PDAL/PDAL/64c11506cccdf790b9677458d5ee884bdf4e754b/test/data/las/4_6.las	BSD-3-Clause (PDAL test data)
pdal/las/autzen_trim.las	3742038	https://raw.githubusercontent.com/PDAL/PDAL/64c11506cccdf790b9677458d5ee884bdf4e754b/test/data/las/autzen_trim.las	BSD-3-Clause (PDAL test data)
pdal/las/extrabytes.las	66354	https://raw.githubusercontent.com/PDAL/PDAL/64c11506cccdf790b9677458d5ee884bdf4e754b/test/data/las/extrabytes.las	BSD-3-Clause (PDAL test data)
pdal/las/epsg_4326.las	108453	https://raw.githubusercontent.com/PDAL/PDAL/64c11506cccdf790b9677458d5ee884bdf4e754b/test/data/las/epsg_4326.las	BSD-3-Clause (PDAL test data)
pdal/las/test1_4.las	32305	https://raw.githubusercontent.com/PDAL/PDAL/64c11506cccdf790b9677458d5ee884bdf4e754b/test/data/las/test1_4.las	BSD-3-Clause (PDAL test data)
pdal/las/no-points.las	859	https://raw.githubusercontent.com/PDAL/PDAL/64c11506cccdf790b9677458d5ee884bdf4e754b/test/data/las/no-points.las	BSD-3-Clause (PDAL test data)
pdal/las/simple.las	36437	https://raw.githubusercontent.com/PDAL/PDAL/64c11506cccdf790b9677458d5ee884bdf4e754b/test/data/las/simple.las	BSD-3-Clause (PDAL test data)
pdal/las/spec_3.las	898	https://raw.githubusercontent.com/PDAL/PDAL/64c11506cccdf790b9677458d5ee884bdf4e754b/test/data/las/spec_3.las	BSD-3-Clause (PDAL test data)
pdal/las/lots_of_vlr.las	81919	https://raw.githubusercontent.com/PDAL/PDAL/64c11506cccdf790b9677458d5ee884bdf4e754b/test/data/las/lots_of_vlr.las	BSD-3-Clause (PDAL test data)
pdal/las/garbage_nVariableLength.las	14601	https://raw.githubusercontent.com/PDAL/PDAL/64c11506cccdf790b9677458d5ee884bdf4e754b/test/data/las/garbage_nVariableLength.las	BSD-3-Clause (PDAL test data)
pdal/las/bad_vlr_count.las	769	https://raw.githubusercontent.com/PDAL/PDAL/64c11506cccdf790b9677458d5ee884bdf4e754b/test/data/las/bad_vlr_count.las	BSD-3-Clause (PDAL test data)
pdal/las/gps-time-nan.las	255	https://raw.githubusercontent.com/PDAL/PDAL/64c11506cccdf790b9677458d5ee884bdf4e754b/test/data/las/gps-time-nan.las	BSD-3-Clause (PDAL test data)
pdal/las/mvk-thin.las	179154	https://raw.githubusercontent.com/PDAL/PDAL/64c11506cccdf790b9677458d5ee884bdf4e754b/test/data/las/mvk-thin.las	BSD-3-Clause (PDAL test data)
pdal/las/sample_c.las	490099	https://raw.githubusercontent.com/PDAL/PDAL/64c11506cccdf790b9677458d5ee884bdf4e754b/test/data/las/sample_c.las	BSD-3-Clause (PDAL test data)
pdal/las/warsaw_small.las	102284	https://raw.githubusercontent.com/PDAL/PDAL/64c11506cccdf790b9677458d5ee884bdf4e754b/test/data/las/warsaw_small.las	BSD-3-Clause (PDAL test data)
pdal/las/prec3.las	4459	https://raw.githubusercontent.com/PDAL/PDAL/64c11506cccdf790b9677458d5ee884bdf4e754b/test/data/las/prec3.las	BSD-3-Clause (PDAL test data)
pdal/las/interesting.las	37698	https://raw.githubusercontent.com/PDAL/PDAL/64c11506cccdf790b9677458d5ee884bdf4e754b/test/data/las/interesting.las	BSD-3-Clause (PDAL test data)
pdal/las/1.2-empty-geotiff-vlrs.las	9860	https://raw.githubusercontent.com/PDAL/PDAL/64c11506cccdf790b9677458d5ee884bdf4e754b/test/data/las/1.2-empty-geotiff-vlrs.las	BSD-3-Clause (PDAL test data)
pdal/las/bad-geotiff-keys.las	1039	https://raw.githubusercontent.com/PDAL/PDAL/64c11506cccdf790b9677458d5ee884bdf4e754b/test/data/las/bad-geotiff-keys.las	BSD-3-Clause (PDAL test data)
pdal/las/test_utm16.las	793	https://raw.githubusercontent.com/PDAL/PDAL/64c11506cccdf790b9677458d5ee884bdf4e754b/test/data/las/test_utm16.las	BSD-3-Clause (PDAL test data)
pdal/las/utm17.las	769	https://raw.githubusercontent.com/PDAL/PDAL/64c11506cccdf790b9677458d5ee884bdf4e754b/test/data/las/utm17.las	BSD-3-Clause (PDAL test data)
pdal/las/test_epsg_4047.las	1666	https://raw.githubusercontent.com/PDAL/PDAL/64c11506cccdf790b9677458d5ee884bdf4e754b/test/data/las/test_epsg_4047.las	BSD-3-Clause (PDAL test data)
pdal/las/hextest.las	387	https://raw.githubusercontent.com/PDAL/PDAL/64c11506cccdf790b9677458d5ee884bdf4e754b/test/data/las/hextest.las	BSD-3-Clause (PDAL test data)
pdal/las/synthetic_test.las	261	https://raw.githubusercontent.com/PDAL/PDAL/64c11506cccdf790b9677458d5ee884bdf4e754b/test/data/las/synthetic_test.las	BSD-3-Clause (PDAL test data)
pdal/las/permutations/1.0_0.las	1027	https://raw.githubusercontent.com/PDAL/PDAL/64c11506cccdf790b9677458d5ee884bdf4e754b/test/data/las/permutations/1.0_0.las	BSD-3-Clause (PDAL test data)
pdal/las/permutations/1.0_1.las	1035	https://raw.githubusercontent.com/PDAL/PDAL/64c11506cccdf790b9677458d5ee884bdf4e754b/test/data/las/permutations/1.0_1.las	BSD-3-Clause (PDAL test data)
pdal/las/permutations/1.1_0.las	1025	https://raw.githubusercontent.com/PDAL/PDAL/64c11506cccdf790b9677458d5ee884bdf4e754b/test/data/las/permutations/1.1_0.las	BSD-3-Clause (PDAL test data)
pdal/las/permutations/1.1_1.las	1033	https://raw.githubusercontent.com/PDAL/PDAL/64c11506cccdf790b9677458d5ee884bdf4e754b/test/data/las/permutations/1.1_1.las	BSD-3-Clause (PDAL test data)
pdal/las/permutations/1.2_0.las	1025	https://raw.githubusercontent.com/PDAL/PDAL/64c11506cccdf790b9677458d5ee884bdf4e754b/test/data/las/permutations/1.2_0.las	BSD-3-Clause (PDAL test data)
pdal/las/permutations/1.2_1.las	1033	https://raw.githubusercontent.com/PDAL/PDAL/64c11506cccdf790b9677458d5ee884bdf4e754b/test/data/las/permutations/1.2_1.las	BSD-3-Clause (PDAL test data)
pdal/las/permutations/1.2_2.las	1031	https://raw.githubusercontent.com/PDAL/PDAL/64c11506cccdf790b9677458d5ee884bdf4e754b/test/data/las/permutations/1.2_2.las	BSD-3-Clause (PDAL test data)
pdal/las/permutations/1.2_3.las	1039	https://raw.githubusercontent.com/PDAL/PDAL/64c11506cccdf790b9677458d5ee884bdf4e754b/test/data/las/permutations/1.2_3.las	BSD-3-Clause (PDAL test data)
pdal/las/permutations/1.2-no-points.las	229	https://raw.githubusercontent.com/PDAL/PDAL/64c11506cccdf790b9677458d5ee884bdf4e754b/test/data/las/permutations/1.2-no-points.las	BSD-3-Clause (PDAL test data)
pdal/laz/autzen_trim.laz	603353	https://raw.githubusercontent.com/PDAL/PDAL/64c11506cccdf790b9677458d5ee884bdf4e754b/test/data/laz/autzen_trim.laz	BSD-3-Clause (PDAL test data)
pdal/laz/las_with_several_extra_byte_bloc.laz	3240041	https://raw.githubusercontent.com/PDAL/PDAL/64c11506cccdf790b9677458d5ee884bdf4e754b/test/data/laz/las_with_several_extra_byte_bloc.laz	BSD-3-Clause (PDAL test data)
pdal/laz/simple-laszip-compressor-version-1.2r0.laz	22149	https://raw.githubusercontent.com/PDAL/PDAL/64c11506cccdf790b9677458d5ee884bdf4e754b/test/data/laz/simple-laszip-compressor-version-1.2r0.laz	BSD-3-Clause (PDAL test data)
pdal/laz/simple.laz	18217	https://raw.githubusercontent.com/PDAL/PDAL/64c11506cccdf790b9677458d5ee884bdf4e754b/test/data/laz/simple.laz	BSD-3-Clause (PDAL test data)
pdal/laszip/laszip-generated.laz	18219	https://raw.githubusercontent.com/PDAL/PDAL/64c11506cccdf790b9677458d5ee884bdf4e754b/test/data/laszip/laszip-generated.laz	BSD-3-Clause (PDAL test data)
pdal/laszip/liblas-generated.laz	18249	https://raw.githubusercontent.com/PDAL/PDAL/64c11506cccdf790b9677458d5ee884bdf4e754b/test/data/laszip/liblas-generated.laz	BSD-3-Clause (PDAL test data)
pdal/laszip/laszip-generated_with2bytespadding.laz	18217	https://raw.githubusercontent.com/PDAL/PDAL/64c11506cccdf790b9677458d5ee884bdf4e754b/test/data/laszip/laszip-generated_with2bytespadding.laz	BSD-3-Clause (PDAL test data)
pdal/laszip/basefile.las	36439	https://raw.githubusercontent.com/PDAL/PDAL/64c11506cccdf790b9677458d5ee884bdf4e754b/test/data/laszip/basefile.las	BSD-3-Clause (PDAL test data)
pdal/copc/1.2-with-color.copc.laz	33684	https://raw.githubusercontent.com/PDAL/PDAL/64c11506cccdf790b9677458d5ee884bdf4e754b/test/data/copc/1.2-with-color.copc.laz	BSD-3-Clause (PDAL test data)
pdal/copc/lone-star.copc.laz	2705193	https://raw.githubusercontent.com/PDAL/PDAL/64c11506cccdf790b9677458d5ee884bdf4e754b/test/data/copc/lone-star.copc.laz	BSD-3-Clause (PDAL test data)
pdal/draco/red-rocks.laz	10188197	https://raw.githubusercontent.com/PDAL/PDAL/64c11506cccdf790b9677458d5ee884bdf4e754b/test/data/draco/red-rocks.laz	BSD-3-Clause (PDAL test data)
pdal/autzen/autzen-thin-srs.las	363585	https://raw.githubusercontent.com/PDAL/PDAL/64c11506cccdf790b9677458d5ee884bdf4e754b/test/data/autzen/autzen-thin-srs.las	BSD-3-Clause (PDAL test data)
pdal/ept/source/lone-star.laz	2382514	https://raw.githubusercontent.com/PDAL/PDAL/64c11506cccdf790b9677458d5ee884bdf4e754b/test/data/ept/source/lone-star.laz	BSD-3-Clause (PDAL test data)
laz-perf/1815.las	35436	https://raw.githubusercontent.com/hobuinc/laz-perf/f2e7491902dc06bf4ce0a4577d3af3b98f280b2a/cpp/test/raw-sets/1815.las	Apache-2.0 (laz-perf test data)
laz-perf/1815.laz	16177	https://raw.githubusercontent.com/hobuinc/laz-perf/f2e7491902dc06bf4ce0a4577d3af3b98f280b2a/cpp/test/raw-sets/1815.laz	Apache-2.0 (laz-perf test data)
laz-perf/autzen_trim.las	3742038	https://raw.githubusercontent.com/hobuinc/laz-perf/f2e7491902dc06bf4ce0a4577d3af3b98f280b2a/cpp/test/raw-sets/autzen_trim.las	Apache-2.0 (laz-perf test data)
laz-perf/autzen_trim.laz	603353	https://raw.githubusercontent.com/hobuinc/laz-perf/f2e7491902dc06bf4ce0a4577d3af3b98f280b2a/cpp/test/raw-sets/autzen_trim.laz	Apache-2.0 (laz-perf test data)
laz-perf/extrabytes.las	66206	https://raw.githubusercontent.com/hobuinc/laz-perf/f2e7491902dc06bf4ce0a4577d3af3b98f280b2a/cpp/test/raw-sets/extrabytes.las	Apache-2.0 (laz-perf test data)
laz-perf/extrabytes.laz	28936	https://raw.githubusercontent.com/hobuinc/laz-perf/f2e7491902dc06bf4ce0a4577d3af3b98f280b2a/cpp/test/raw-sets/extrabytes.laz	Apache-2.0 (laz-perf test data)
laz-perf/no-points-1.3.las	235	https://raw.githubusercontent.com/hobuinc/laz-perf/f2e7491902dc06bf4ce0a4577d3af3b98f280b2a/cpp/test/raw-sets/no-points-1.3.las	Apache-2.0 (laz-perf test data)
laz-perf/no-points-1.3.laz	345	https://raw.githubusercontent.com/hobuinc/laz-perf/f2e7491902dc06bf4ce0a4577d3af3b98f280b2a/cpp/test/raw-sets/no-points-1.3.laz	Apache-2.0 (laz-perf test data)
laz-perf/point-color-time.las	37417	https://raw.githubusercontent.com/hobuinc/laz-perf/f2e7491902dc06bf4ce0a4577d3af3b98f280b2a/cpp/test/raw-sets/point-color-time.las	Apache-2.0 (laz-perf test data)
laz-perf/point-color-time.las.laz	18558	https://raw.githubusercontent.com/hobuinc/laz-perf/f2e7491902dc06bf4ce0a4577d3af3b98f280b2a/cpp/test/raw-sets/point-color-time.las.laz	Apache-2.0 (laz-perf test data)
laz-perf/point-color.las	28897	https://raw.githubusercontent.com/hobuinc/laz-perf/f2e7491902dc06bf4ce0a4577d3af3b98f280b2a/cpp/test/raw-sets/point-color.las	Apache-2.0 (laz-perf test data)
laz-perf/point-color.las.laz	13913	https://raw.githubusercontent.com/hobuinc/laz-perf/f2e7491902dc06bf4ce0a4577d3af3b98f280b2a/cpp/test/raw-sets/point-color.las.laz	Apache-2.0 (laz-perf test data)
laz-perf/point-time-1.4.las.laz	16111	https://raw.githubusercontent.com/hobuinc/laz-perf/f2e7491902dc06bf4ce0a4577d3af3b98f280b2a/cpp/test/raw-sets/point-time-1.4.las.laz	Apache-2.0 (laz-perf test data)
laz-perf/point-time.las	31027	https://raw.githubusercontent.com/hobuinc/laz-perf/f2e7491902dc06bf4ce0a4577d3af3b98f280b2a/cpp/test/raw-sets/point-time.las	Apache-2.0 (laz-perf test data)
laz-perf/point-time.las.laz	15633	https://raw.githubusercontent.com/hobuinc/laz-perf/f2e7491902dc06bf4ce0a4577d3af3b98f280b2a/cpp/test/raw-sets/point-time.las.laz	Apache-2.0 (laz-perf test data)
laz-perf/point10-1.0.las	247	https://raw.githubusercontent.com/hobuinc/laz-perf/f2e7491902dc06bf4ce0a4577d3af3b98f280b2a/cpp/test/raw-sets/point10-1.0.las	Apache-2.0 (laz-perf test data)
laz-perf/point10-1.0.laz	366	https://raw.githubusercontent.com/hobuinc/laz-perf/f2e7491902dc06bf4ce0a4577d3af3b98f280b2a/cpp/test/raw-sets/point10-1.0.laz	Apache-2.0 (laz-perf test data)
laz-perf/point10-1.1.las	247	https://raw.githubusercontent.com/hobuinc/laz-perf/f2e7491902dc06bf4ce0a4577d3af3b98f280b2a/cpp/test/raw-sets/point10-1.1.las	Apache-2.0 (laz-perf test data)
laz-perf/point10-1.1.laz	366	https://raw.githubusercontent.com/hobuinc/laz-perf/f2e7491902dc06bf4ce0a4577d3af3b98f280b2a/cpp/test/raw-sets/point10-1.1.laz	Apache-2.0 (laz-perf test data)
laz-perf/point10.las	22507	https://raw.githubusercontent.com/hobuinc/laz-perf/f2e7491902dc06bf4ce0a4577d3af3b98f280b2a/cpp/test/raw-sets/point10.las	Apache-2.0 (laz-perf test data)
laz-perf/point10.las.laz	10988	https://raw.githubusercontent.com/hobuinc/laz-perf/f2e7491902dc06bf4ce0a4577d3af3b98f280b2a/cpp/test/raw-sets/point10.las.laz	Apache-2.0 (laz-perf test data)
laz-perf/rand1_6.las	1500375	https://raw.githubusercontent.com/hobuinc/laz-perf/f2e7491902dc06bf4ce0a4577d3af3b98f280b2a/cpp/test/raw-sets/rand1_6.las	Apache-2.0 (laz-perf test data)
laszip/broken_coder.laz	1000	https://raw.githubusercontent.com/LASzip/LASzip/9b635836400f6cc5b8aba95985de3ed84685d7e2/data/broken_coder.laz	Apache-2.0 (LASzip test data)
laszip/broken_compressor.laz	1000	https://raw.githubusercontent.com/LASzip/LASzip/9b635836400f6cc5b8aba95985de3ed84685d7e2/data/broken_compressor.laz	Apache-2.0 (LASzip test data)
laszip/broken_size.laz	1000	https://raw.githubusercontent.com/LASzip/LASzip/9b635836400f6cc5b8aba95985de3ed84685d7e2/data/broken_size.laz	Apache-2.0 (LASzip test data)
laszip/broken_type.laz	1000	https://raw.githubusercontent.com/LASzip/LASzip/9b635836400f6cc5b8aba95985de3ed84685d7e2/data/broken_type.laz	Apache-2.0 (LASzip test data)
laszip/broken_version.laz	1000	https://raw.githubusercontent.com/LASzip/LASzip/9b635836400f6cc5b8aba95985de3ed84685d7e2/data/broken_version.laz	Apache-2.0 (LASzip test data)
laszip/5points_14.las	1086	https://raw.githubusercontent.com/LASzip/LASzip/9b635836400f6cc5b8aba95985de3ed84685d7e2/example/5points_14.las	Apache-2.0 (LASzip test data)
e57/pumpAVisualReferenceImage.e57	4110336	https://raw.githubusercontent.com/davidcaron/pye57/b8d20999f3037c580bf1fe1dcb239c50a8f764ad/tests/test_data/pumpAVisualReferenceImage.e57	MIT (pye57 test data, libE57 samples)
e57/test.e57	13723648	https://raw.githubusercontent.com/davidcaron/pye57/b8d20999f3037c580bf1fe1dcb239c50a8f764ad/tests/test_data/test.e57	MIT (pye57 test data, libE57 samples)
e57/testSpherical.e57	5168128	https://raw.githubusercontent.com/davidcaron/pye57/b8d20999f3037c580bf1fe1dcb239c50a8f764ad/tests/test_data/testSpherical.e57	MIT (pye57 test data, libE57 samples)
e57/testWithNormals.e57	3072	https://raw.githubusercontent.com/davidcaron/pye57/b8d20999f3037c580bf1fe1dcb239c50a8f764ad/tests/test_data/testWithNormals.e57	MIT (pye57 test data, libE57 samples)
e57/translationWithoutRotation.e57	3072	https://raw.githubusercontent.com/davidcaron/pye57/b8d20999f3037c580bf1fe1dcb239c50a8f764ad/tests/test_data/translationWithoutRotation.e57	MIT (pye57 test data, libE57 samples)
pcd/bunny.pcd	10878	https://raw.githubusercontent.com/PointCloudLibrary/pcl/66e1d0ea8cc3fa54820eb38e18ad0adac6345b94/test/bunny.pcd	BSD-3-Clause (PCL test data)
pcd/bun0.pcd	32101	https://raw.githubusercontent.com/PointCloudLibrary/pcl/66e1d0ea8cc3fa54820eb38e18ad0adac6345b94/test/bun0.pcd	BSD-3-Clause (PCL test data)
pcd/milk_color.pcd	146087	https://raw.githubusercontent.com/PointCloudLibrary/pcl/66e1d0ea8cc3fa54820eb38e18ad0adac6345b94/test/milk_color.pcd	BSD-3-Clause (PCL test data)
pcd/milk.pcd	92940	https://raw.githubusercontent.com/PointCloudLibrary/pcl/66e1d0ea8cc3fa54820eb38e18ad0adac6345b94/test/milk.pcd	BSD-3-Clause (PCL test data)
pcd/table_scene_mug_stereo_textured.pcd	1726160	https://raw.githubusercontent.com/PointCloudLibrary/pcl/66e1d0ea8cc3fa54820eb38e18ad0adac6345b94/test/table_scene_mug_stereo_textured.pcd	BSD-3-Clause (PCL test data)
pcd/colored_cloud.pcd	32241	https://raw.githubusercontent.com/PointCloudLibrary/pcl/66e1d0ea8cc3fa54820eb38e18ad0adac6345b94/test/colored_cloud.pcd	BSD-3-Clause (PCL test data)
pcd/cturtle.pcd	788633	https://raw.githubusercontent.com/PointCloudLibrary/pcl/66e1d0ea8cc3fa54820eb38e18ad0adac6345b94/test/cturtle.pcd	BSD-3-Clause (PCL test data)
pcd/five_people.pcd	2007625	https://raw.githubusercontent.com/PointCloudLibrary/pcl/66e1d0ea8cc3fa54820eb38e18ad0adac6345b94/test/five_people.pcd	BSD-3-Clause (PCL test data)
pcd/office1.pcd	1878399	https://raw.githubusercontent.com/PointCloudLibrary/pcl/66e1d0ea8cc3fa54820eb38e18ad0adac6345b94/test/office1.pcd	BSD-3-Clause (PCL test data)
pcd/pcl_logo.pcd	29714	https://raw.githubusercontent.com/PointCloudLibrary/pcl/66e1d0ea8cc3fa54820eb38e18ad0adac6345b94/test/pcl_logo.pcd	BSD-3-Clause (PCL test data)
pcd/car6.pcd	65638	https://raw.githubusercontent.com/PointCloudLibrary/pcl/66e1d0ea8cc3fa54820eb38e18ad0adac6345b94/test/car6.pcd	BSD-3-Clause (PCL test data)
pcd/lamppost.pcd	47146	https://raw.githubusercontent.com/PointCloudLibrary/pcl/66e1d0ea8cc3fa54820eb38e18ad0adac6345b94/test/lamppost.pcd	BSD-3-Clause (PCL test data)
pcd/octants.pcd	279	https://raw.githubusercontent.com/PointCloudLibrary/pcl/66e1d0ea8cc3fa54820eb38e18ad0adac6345b94/test/octants.pcd	BSD-3-Clause (PCL test data)
pcd/sac_plane_test.pcd	250720	https://raw.githubusercontent.com/PointCloudLibrary/pcl/66e1d0ea8cc3fa54820eb38e18ad0adac6345b94/test/sac_plane_test.pcd	BSD-3-Clause (PCL test data)
pcd/stereo_left.pcd	521238	https://raw.githubusercontent.com/PointCloudLibrary/pcl/66e1d0ea8cc3fa54820eb38e18ad0adac6345b94/test/stereo_left.pcd	BSD-3-Clause (PCL test data)
pcd/rops_cloud.pcd	822795	https://raw.githubusercontent.com/PointCloudLibrary/pcl/66e1d0ea8cc3fa54820eb38e18ad0adac6345b94/test/rops_cloud.pcd	BSD-3-Clause (PCL test data)
pcd/lena.pcd	158514	https://raw.githubusercontent.com/PointCloudLibrary/pcl/66e1d0ea8cc3fa54820eb38e18ad0adac6345b94/test/2d/lena.pcd	BSD-3-Clause (PCL test data)
pcd/curve3d.pcd	3889	https://raw.githubusercontent.com/PointCloudLibrary/pcl/66e1d0ea8cc3fa54820eb38e18ad0adac6345b94/test/curve3d.pcd	BSD-3-Clause (PCL test data)
pcd/frame_20121214T142255.814212.pcd	1837321	https://raw.githubusercontent.com/PointCloudLibrary/pcl/66e1d0ea8cc3fa54820eb38e18ad0adac6345b94/test/grabber_sequences/pcd/frame_20121214T142255.814212.pcd	BSD-3-Clause (PCL test data)
copc/autzen-classified.copc.laz	81123042	https://s3.amazonaws.com/hobu-lidar/autzen-classified.copc.laz	public domain (Oregon LiDAR Consortium Autzen sample, as distributed by Hobu)
END
)

fetch() {
  local n=0 got=0
  : > "$OUT/.provenance.tsv"
  while IFS=$'\t' read -r dest size url lic; do
    [[ -z $dest ]] && continue
    n=$((n + 1))
    local f="$OUT/$dest"
    if [[ ! -f $f || $(stat -c %s "$f") != "$size" ]]; then
      check_space
      mkdir -p "$(dirname "$f")"
      curl -fsSL --retry 3 --max-filesize $((size + 1)) -o "$f.part" "$url" \
        || die "download failed: $url"
      [[ $(stat -c %s "$f.part") == "$size" ]] || { rm -f "$f.part"; die "size mismatch: $url"; }
      mv "$f.part" "$f"
      got=$((got + 1))
    fi
    printf '%s\t%s\t%s\t%s\t%s\n' "$dest" "$size" "$(sha256sum "$f" | cut -d' ' -f1)" "$url" "$lic" \
      >> "$OUT/.provenance.tsv"
  done <<< "$MANIFEST"
  echo "  $n files ($got downloaded) in $OUT"
}

oracle() {
  if ! "$PYTHON" -I -c "import laspy, lazrs, pye57" 2>/dev/null; then
    echo "  $PYTHON lacks laspy, lazrs or pye57; no oracle (see the header for a venv)"
    return
  fi
  "$PYTHON" -I "$ROOT/crates/formats/specular-points/gen/oracle.py" "$OUT"
}

main() {
  (( $# == 0 )) && set -- fetch oracle
  for s in "$@"; do
    case $s in
      fetch | oracle) echo "$s"; "$s" ;;
      *) die "unknown step $s" ;;
    esac
  done
}

main "$@"
