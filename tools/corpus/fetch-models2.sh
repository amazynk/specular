#!/usr/bin/env bash
# Fetch the IFC, X3D/VRML, Rhino, AMF and PTX test corpus into tools/corpus/models2/
# (gitignored: nothing here is committed). The specular-mesh and specular-points corpus tests
# read it from SPECULAR_MODELS2_CORPUS (a worktree has no corpus of its own), else from
# tools/corpus/models2, and skip when it is absent.
#
# Usage: tools/corpus/fetch-models2.sh [SECTION...]
#   SECTION is a function name below (e.g. src_ifc). Default: every section, then the
#   oracles (src_oracle) when PYTHON has them.
#   OUT defaults to $SPECULAR_MODELS2_CORPUS, else tools/corpus/models2.
#   PYTHON (default python3) needs ifcopenshell, rhino3dm and numpy for the oracle and the
#   generated files only. A venv under target/ is fine:
#     python3 -m venv target/models2/venv
#     PIP_NO_CACHE_DIR=1 target/models2/venv/bin/pip install ifcopenshell rhino3dm numpy
#   then PYTHON=target/models2/venv/bin/python.
#
# Downloads are untrusted data: this script only copies files, never runs anything from them
# (the oracles read them from scripts outside the corpus, with python -I). Scratch space is
# target/agent-corpus/models2-fetch (deleted afterwards). Stops if free space on the target
# filesystem drops below MIN_FREE_GB (default 6).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_MODELS2_CORPUS:-$ROOT/tools/corpus/models2}
SCRATCH="$ROOT/target/agent-corpus/models2-fetch"
MIN_FREE_GB=${MIN_FREE_GB:-6}
PYTHON=${PYTHON:-python3}
mkdir -p "$OUT" "$SCRATCH"

die() { echo "fetch-models2: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

# provenance RELPATH URL LICENCE
provenance() { printf '%s\t%s\t%s\n' "$1" "$2" "$3" >> "$OUT/.provenance.tsv"; }

# get URL DEST SHA256 LICENCE: downloads one file and checks its hash.
get() {
  local url=$1 dest=$2 sha=$3 lic=$4
  check_space
  mkdir -p "$(dirname "$OUT/$dest")"
  if [[ -f "$OUT/$dest" ]] && echo "$sha  $OUT/$dest" | sha256sum -c --quiet - 2>/dev/null; then
    echo "  $dest (cached)"
  else
    curl -fsSL --retry 3 -o "$OUT/$dest.part" "$url"
    echo "$sha  $OUT/$dest.part" | sha256sum -c --quiet - || die "$dest: sha256 mismatch"
    mv "$OUT/$dest.part" "$OUT/$dest"
    echo "  $dest"
  fi
  provenance "$dest" "$url" "$lic"
}

have_python() {
  "$PYTHON" -I -c "import $1" 2>/dev/null || {
    echo "  $PYTHON lacks $1; skipped (see the header for a venv)"
    return 1
  }
}

# ------------------------------------------------------------------------------- sections

src_ifc() {          # buildingSMART sample and certification models, web-ifc's test models
  # buildingSMART Certification-datasets: IFC2x3, IFC4 and IFC4x3 simple scenes and the
  # IFC4 Reference View examples; IFC4.3.x-sample-models: one model per geometry concept.
  get "https://raw.githubusercontent.com/buildingSMART/Certification-datasets/80d976a9b193a26a8e928c3e79bff67af1de68a8/IFC%202.3.0.1%20%28IFC%202x3%20TC1%29/Simple-Scene/Building-Architecture.ifc" \
    ifc/cert/ifc2x3/scene/Building-Architecture.ifc \
    c4db65ba847f6b369a95d6c54fa11f4750cbe6d59f021934e8923d8d578e5885 "buildingSMART Certification-datasets (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/Certification-datasets/80d976a9b193a26a8e928c3e79bff67af1de68a8/IFC%202.3.0.1%20%28IFC%202x3%20TC1%29/Simple-Scene/Building-Hvac.ifc" \
    ifc/cert/ifc2x3/scene/Building-Hvac.ifc \
    f39478ce12ae2029eed8b565551412a3a4b518d10a8b501bc5e9c2a8a6d78bdb "buildingSMART Certification-datasets (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/Certification-datasets/80d976a9b193a26a8e928c3e79bff67af1de68a8/IFC%202.3.0.1%20%28IFC%202x3%20TC1%29/Simple-Scene/Building-Landscaping.ifc" \
    ifc/cert/ifc2x3/scene/Building-Landscaping.ifc \
    5ca6782566f4a263fa742154f05ab6a30608835734fd8cbae38472f91ae8dbaf "buildingSMART Certification-datasets (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/Certification-datasets/80d976a9b193a26a8e928c3e79bff67af1de68a8/IFC%202.3.0.1%20%28IFC%202x3%20TC1%29/Simple-Scene/Building-Structural.ifc" \
    ifc/cert/ifc2x3/scene/Building-Structural.ifc \
    25c09f1ea35dbf51e88b8829c423b9ce13a8264e69c620e932859772937aa25b "buildingSMART Certification-datasets (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/Certification-datasets/80d976a9b193a26a8e928c3e79bff67af1de68a8/IFC%202.3.0.1%20%28IFC%202x3%20TC1%29/Simple-Scene/Infra-Bridge.ifc" \
    ifc/cert/ifc2x3/scene/Infra-Bridge.ifc \
    50e4aa79c42281b6410ea47da16750c78d24acb3913a90780d582ec3b01e50bd "buildingSMART Certification-datasets (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/Certification-datasets/80d976a9b193a26a8e928c3e79bff67af1de68a8/IFC%202.3.0.1%20%28IFC%202x3%20TC1%29/Simple-Scene/Infra-Electrical.ifc" \
    ifc/cert/ifc2x3/scene/Infra-Electrical.ifc \
    60b55dfa3513076df211e3232b4dee29fd1f361abc7908f7c87f47d3b5412e98 "buildingSMART Certification-datasets (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/Certification-datasets/80d976a9b193a26a8e928c3e79bff67af1de68a8/IFC%202.3.0.1%20%28IFC%202x3%20TC1%29/Simple-Scene/Infra-Landscaping.ifc" \
    ifc/cert/ifc2x3/scene/Infra-Landscaping.ifc \
    4d868f360b99dfdeef3b79057bd5d3cd41071d959d46196b36297f80750396bc "buildingSMART Certification-datasets (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/Certification-datasets/80d976a9b193a26a8e928c3e79bff67af1de68a8/IFC%202.3.0.1%20%28IFC%202x3%20TC1%29/Simple-Scene/Infra-Plumbing.ifc" \
    ifc/cert/ifc2x3/scene/Infra-Plumbing.ifc \
    1724daf0cf0ca43797f44e2cdcd18d273c432afc6b411012cc9c1d382408d149 "buildingSMART Certification-datasets (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/Certification-datasets/80d976a9b193a26a8e928c3e79bff67af1de68a8/IFC%202.3.0.1%20%28IFC%202x3%20TC1%29/Simple-Scene/Infra-Rail.ifc" \
    ifc/cert/ifc2x3/scene/Infra-Rail.ifc \
    8dc7c13ba9f5758a764c2322e4e585f29651b10eec69dd0be6701f191ebcf223 "buildingSMART Certification-datasets (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/Certification-datasets/80d976a9b193a26a8e928c3e79bff67af1de68a8/IFC%202.3.0.1%20%28IFC%202x3%20TC1%29/Simple-Scene/Infra-Road.ifc" \
    ifc/cert/ifc2x3/scene/Infra-Road.ifc \
    45efd6b6d2a26daf17a3f83a331c780234faf0b73069f0410155ffa7ead222f4 "buildingSMART Certification-datasets (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/Certification-datasets/80d976a9b193a26a8e928c3e79bff67af1de68a8/IFC%204.0.2.1%20%28IFC%204%20ADD2%20TC1%29/ISO%20Spec%20-%20ReferenceView_V1.2/basin-tessellation.ifc" \
    ifc/cert/ifc4/refview/basin-tessellation.ifc \
    7278769ec5ef35d388819d2f197519061c7f0f68dd43a733f333ddeb74578121 "buildingSMART Certification-datasets (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/Certification-datasets/80d976a9b193a26a8e928c3e79bff67af1de68a8/IFC%204.0.2.1%20%28IFC%204%20ADD2%20TC1%29/ISO%20Spec%20-%20ReferenceView_V1.2/column-straight-rectangle-tessellation.ifc" \
    ifc/cert/ifc4/refview/column-straight-rectangle-tessellation.ifc \
    58bb9b2cae96edf2c368de95f526650f3f282e4e7fc37c0065c501bdbeed00a1 "buildingSMART Certification-datasets (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/Certification-datasets/80d976a9b193a26a8e928c3e79bff67af1de68a8/IFC%204.0.2.1%20%28IFC%204%20ADD2%20TC1%29/ISO%20Spec%20-%20ReferenceView_V1.2/tessellated-item.ifc" \
    ifc/cert/ifc4/refview/tessellated-item.ifc \
    758974f7d558f11b8d8121816954179def32594ed7eafcd1faae70ee0a8e5946 "buildingSMART Certification-datasets (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/Certification-datasets/80d976a9b193a26a8e928c3e79bff67af1de68a8/IFC%204.0.2.1%20%28IFC%204%20ADD2%20TC1%29/ISO%20Spec%20-%20ReferenceView_V1.2/tessellation-with-individual-colors.ifc" \
    ifc/cert/ifc4/refview/tessellation-with-individual-colors.ifc \
    3ab1290207d8538da9d8ebc14826b769eeb79fce7d75aaa85c35ed7a5a8dbd92 "buildingSMART Certification-datasets (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/Certification-datasets/80d976a9b193a26a8e928c3e79bff67af1de68a8/IFC%204.0.2.1%20%28IFC%204%20ADD2%20TC1%29/ISO%20Spec%20-%20ReferenceView_V1.2/wall-with-opening-and-window.ifc" \
    ifc/cert/ifc4/refview/wall-with-opening-and-window.ifc \
    73b0e45d931d5dc13bfee5fdc7bd80f796526445458b2de74c4168d209097832 "buildingSMART Certification-datasets (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/Certification-datasets/80d976a9b193a26a8e928c3e79bff67af1de68a8/IFC%204.0.2.1%20%28IFC%204%20ADD2%20TC1%29/Simple-Scene/Building-Architecture.ifc" \
    ifc/cert/ifc4/scene/Building-Architecture.ifc \
    8790a1e193e82b8e7e7f337ec2633cd40f2120590317a1443503a25b079e2e80 "buildingSMART Certification-datasets (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/Certification-datasets/80d976a9b193a26a8e928c3e79bff67af1de68a8/IFC%204.0.2.1%20%28IFC%204%20ADD2%20TC1%29/Simple-Scene/Building-Hvac.ifc" \
    ifc/cert/ifc4/scene/Building-Hvac.ifc \
    2c17ecad2b0fbd3335420ee42ba48963b5a395294e86bcdcfc786ee559f9a344 "buildingSMART Certification-datasets (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/Certification-datasets/80d976a9b193a26a8e928c3e79bff67af1de68a8/IFC%204.0.2.1%20%28IFC%204%20ADD2%20TC1%29/Simple-Scene/Building-Landscaping.ifc" \
    ifc/cert/ifc4/scene/Building-Landscaping.ifc \
    937dc9c0e013b0dbbf56dc0fa0dc435efe16a2234cd364af820d2cf1deaa3d7f "buildingSMART Certification-datasets (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/Certification-datasets/80d976a9b193a26a8e928c3e79bff67af1de68a8/IFC%204.0.2.1%20%28IFC%204%20ADD2%20TC1%29/Simple-Scene/Building-Structural.ifc" \
    ifc/cert/ifc4/scene/Building-Structural.ifc \
    903b5a005901397aa2b235daf6a60f46c099eae286cb01c815abf0a341707432 "buildingSMART Certification-datasets (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/Certification-datasets/80d976a9b193a26a8e928c3e79bff67af1de68a8/IFC%204.0.2.1%20%28IFC%204%20ADD2%20TC1%29/Simple-Scene/Infra-Bridge.ifc" \
    ifc/cert/ifc4/scene/Infra-Bridge.ifc \
    6012bb5e0c33d9923d08fd04e561f0c2d897674a8f3066808494addcadae1276 "buildingSMART Certification-datasets (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/Certification-datasets/80d976a9b193a26a8e928c3e79bff67af1de68a8/IFC%204.0.2.1%20%28IFC%204%20ADD2%20TC1%29/Simple-Scene/Infra-Electrical.ifc" \
    ifc/cert/ifc4/scene/Infra-Electrical.ifc \
    18ec714203238bc33ef767306cfcaa8209fcda186a8fe8ad2db7f204bc6b5a49 "buildingSMART Certification-datasets (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/Certification-datasets/80d976a9b193a26a8e928c3e79bff67af1de68a8/IFC%204.0.2.1%20%28IFC%204%20ADD2%20TC1%29/Simple-Scene/Infra-Landscaping.ifc" \
    ifc/cert/ifc4/scene/Infra-Landscaping.ifc \
    aefdccddd57d9c73d7395b0caaface371e548b4dad08cca4f415b3e1c291396b "buildingSMART Certification-datasets (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/Certification-datasets/80d976a9b193a26a8e928c3e79bff67af1de68a8/IFC%204.0.2.1%20%28IFC%204%20ADD2%20TC1%29/Simple-Scene/Infra-Plumbing.ifc" \
    ifc/cert/ifc4/scene/Infra-Plumbing.ifc \
    f44c401de3bbc1b08d7717807b5add12a1c319f98b42a6c633a69fbcf61d83d1 "buildingSMART Certification-datasets (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/Certification-datasets/80d976a9b193a26a8e928c3e79bff67af1de68a8/IFC%204.0.2.1%20%28IFC%204%20ADD2%20TC1%29/Simple-Scene/Infra-Rail.ifc" \
    ifc/cert/ifc4/scene/Infra-Rail.ifc \
    d284d418d19489a4cfecb4134ee1145d1bc3f3ee9148da3919a4e741e3e63ef0 "buildingSMART Certification-datasets (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/Certification-datasets/80d976a9b193a26a8e928c3e79bff67af1de68a8/IFC%204.0.2.1%20%28IFC%204%20ADD2%20TC1%29/Simple-Scene/Infra-Road.ifc" \
    ifc/cert/ifc4/scene/Infra-Road.ifc \
    ebc5c85f6da454de08dc63675af242b6572682692682dff1a064a71b6bcd73e8 "buildingSMART Certification-datasets (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/Certification-datasets/80d976a9b193a26a8e928c3e79bff67af1de68a8/IFC%204.3.2.0%20%28IFC%204.3%20ADD2%29/Simple-Scene/Building-Architecture.ifc" \
    ifc/cert/ifc4x3/scene/Building-Architecture.ifc \
    96e7103812b15b65eb1fd7802d36b9962833485dce5ead4f440bbf529530881d "buildingSMART Certification-datasets (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/Certification-datasets/80d976a9b193a26a8e928c3e79bff67af1de68a8/IFC%204.3.2.0%20%28IFC%204.3%20ADD2%29/Simple-Scene/Building-Hvac.ifc" \
    ifc/cert/ifc4x3/scene/Building-Hvac.ifc \
    22891586f153793d098811caec8a510cd404f6c2f1979fb329eeeebc3f475642 "buildingSMART Certification-datasets (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/Certification-datasets/80d976a9b193a26a8e928c3e79bff67af1de68a8/IFC%204.3.2.0%20%28IFC%204.3%20ADD2%29/Simple-Scene/Building-Landscaping.ifc" \
    ifc/cert/ifc4x3/scene/Building-Landscaping.ifc \
    93253acebf6c2d9bfeb1300c3c8d2fdf663ea24746e5cddb8392aaf50f9a0ca8 "buildingSMART Certification-datasets (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/Certification-datasets/80d976a9b193a26a8e928c3e79bff67af1de68a8/IFC%204.3.2.0%20%28IFC%204.3%20ADD2%29/Simple-Scene/Building-Structural.ifc" \
    ifc/cert/ifc4x3/scene/Building-Structural.ifc \
    db64895488aad4eab082c8292f5632145b0fc864c0642d4fe667404882bacf27 "buildingSMART Certification-datasets (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/Certification-datasets/80d976a9b193a26a8e928c3e79bff67af1de68a8/IFC%204.3.2.0%20%28IFC%204.3%20ADD2%29/Simple-Scene/Infra-Bridge.ifc" \
    ifc/cert/ifc4x3/scene/Infra-Bridge.ifc \
    97e84cb93e07bfbedbc1bdc57aef161f4998e09accd77175dd0561af2ab4ffab "buildingSMART Certification-datasets (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/Certification-datasets/80d976a9b193a26a8e928c3e79bff67af1de68a8/IFC%204.3.2.0%20%28IFC%204.3%20ADD2%29/Simple-Scene/Infra-Electrical.ifc" \
    ifc/cert/ifc4x3/scene/Infra-Electrical.ifc \
    39eebb4612c303e5db5adbab54cdf31ab5f04e02b1887ee49953257fa903a544 "buildingSMART Certification-datasets (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/Certification-datasets/80d976a9b193a26a8e928c3e79bff67af1de68a8/IFC%204.3.2.0%20%28IFC%204.3%20ADD2%29/Simple-Scene/Infra-Landscaping.ifc" \
    ifc/cert/ifc4x3/scene/Infra-Landscaping.ifc \
    378f25564c4fc969225370e322e8ec0d16ce52239e68dd5572e4681879e33fca "buildingSMART Certification-datasets (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/Certification-datasets/80d976a9b193a26a8e928c3e79bff67af1de68a8/IFC%204.3.2.0%20%28IFC%204.3%20ADD2%29/Simple-Scene/Infra-Plumbing.ifc" \
    ifc/cert/ifc4x3/scene/Infra-Plumbing.ifc \
    a6ac7a0f4c80c91f04a1a66b73899be064f9c3992b2e886c92f0e3145c0ca50f "buildingSMART Certification-datasets (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/Certification-datasets/80d976a9b193a26a8e928c3e79bff67af1de68a8/IFC%204.3.2.0%20%28IFC%204.3%20ADD2%29/Simple-Scene/Infra-Rail.ifc" \
    ifc/cert/ifc4x3/scene/Infra-Rail.ifc \
    c9073051e17796b8b59f96cf268207f4011c08eedeaa791774480c33e9defbb3 "buildingSMART Certification-datasets (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/Certification-datasets/80d976a9b193a26a8e928c3e79bff67af1de68a8/IFC%204.3.2.0%20%28IFC%204.3%20ADD2%29/Simple-Scene/Infra-Road.ifc" \
    ifc/cert/ifc4x3/scene/Infra-Road.ifc \
    a3cb29433d7176bd31ff7535a596b5d7cd4eb7f2fb0eb6eee451612063c1f167 "buildingSMART Certification-datasets (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/IFC4.3.x-sample-models/ef22440ec501551ccc9689309560bbc7e26aa9d7/models/advanced-geometric-shape/basin-advanced-brep/basin-advanced-brep.ifc" \
    ifc/ifc43/advanced-geometric-shape/basin-advanced-brep.ifc \
    e947de66cadaefe167e77da75eaaeca864cfcc47f7d1ed197b760b466ae081a9 "buildingSMART IFC4.3.x-sample-models (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/IFC4.3.x-sample-models/ef22440ec501551ccc9689309560bbc7e26aa9d7/models/advanced-geometric-shape/basin-faceted-brep/basin-faceted-brep.ifc" \
    ifc/ifc43/advanced-geometric-shape/basin-faceted-brep.ifc \
    d2bdf9d5f485ff77ce9723a24ceb75a5532b51d1788b9a5f3f5145944d32065f "buildingSMART IFC4.3.x-sample-models (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/IFC4.3.x-sample-models/ef22440ec501551ccc9689309560bbc7e26aa9d7/models/advanced-geometric-shape/basin-tessellation/basin-tessellation.ifc" \
    ifc/ifc43/advanced-geometric-shape/basin-tessellation.ifc \
    466944a98ab0208bd4266d096678233007520915b80bcc14532e64345a1968ef "buildingSMART IFC4.3.x-sample-models (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/IFC4.3.x-sample-models/ef22440ec501551ccc9689309560bbc7e26aa9d7/models/advanced-geometric-shape/bath-csg-solid/bath-csg-solid.ifc" \
    ifc/ifc43/advanced-geometric-shape/bath-csg-solid.ifc \
    4592f3029faa12625494ada734fcfe46e4fc91a33d001198e9e3144721870b95 "buildingSMART IFC4.3.x-sample-models (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/IFC4.3.x-sample-models/ef22440ec501551ccc9689309560bbc7e26aa9d7/models/advanced-geometric-shape/cube-advanced-brep/cube-advanced-brep.ifc" \
    ifc/ifc43/advanced-geometric-shape/cube-advanced-brep.ifc \
    c3c5b0e6f4d659cef6efe7725ff6023d22126e3c471601804b7462f166624161 "buildingSMART IFC4.3.x-sample-models (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/IFC4.3.x-sample-models/ef22440ec501551ccc9689309560bbc7e26aa9d7/models/alignment-geometries-and-linear-positioning/fixed-reference-swept-area-solid/fixed-reference-swept-area-solid.ifc" \
    ifc/ifc43/alignment-geometries-and-linear-positioning/fixed-reference-swept-area-solid.ifc \
    d3e6694756815da907bfc278490ce17596a4fac271905b17a9f3e8c0b4dd6d37 "buildingSMART IFC4.3.x-sample-models (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/IFC4.3.x-sample-models/ef22440ec501551ccc9689309560bbc7e26aa9d7/models/alignment-geometries-and-linear-positioning/linear-placement-of-signal/linear-placement-of-signal.ifc" \
    ifc/ifc43/alignment-geometries-and-linear-positioning/linear-placement-of-signal.ifc \
    3c6b97f7a82f1a1d114233702e876f387a03701be0d8a621685e5773ce664c97 "buildingSMART IFC4.3.x-sample-models (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/IFC4.3.x-sample-models/ef22440ec501551ccc9689309560bbc7e26aa9d7/models/alignment-geometries-and-linear-positioning/sectioned-solid-horizontal/sectioned-solid-horizontal.ifc" \
    ifc/ifc43/alignment-geometries-and-linear-positioning/sectioned-solid-horizontal.ifc \
    89a424cc27f58130d40642b369b93a9b5fb04df2804f1f52ec3fac457f34c025 "buildingSMART IFC4.3.x-sample-models (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/IFC4.3.x-sample-models/ef22440ec501551ccc9689309560bbc7e26aa9d7/models/alignment-geometries-and-linear-positioning/segmented-reference-curve/segmented-reference-curve.ifc" \
    ifc/ifc43/alignment-geometries-and-linear-positioning/segmented-reference-curve.ifc \
    d4320f2fcc7e9c385f89c56c3002a54127f8d9fb4c5a5a447bd7238806b1eadf "buildingSMART IFC4.3.x-sample-models (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/IFC4.3.x-sample-models/ef22440ec501551ccc9689309560bbc7e26aa9d7/models/basic-geometric-shape/brep-model/brep-model.ifc" \
    ifc/ifc43/basic-geometric-shape/brep-model.ifc \
    b247026e85ef163acd5a165177d8293e606e7b8f83348a9897a3a04967fe4fc1 "buildingSMART IFC4.3.x-sample-models (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/IFC4.3.x-sample-models/ef22440ec501551ccc9689309560bbc7e26aa9d7/models/basic-geometric-shape/csg-primitive/csg-primitive.ifc" \
    ifc/ifc43/basic-geometric-shape/csg-primitive.ifc \
    5f366df723fba68cb5d0ecac4efb549912f4107e292c7323a1cd100b3e8032eb "buildingSMART IFC4.3.x-sample-models (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/IFC4.3.x-sample-models/ef22440ec501551ccc9689309560bbc7e26aa9d7/models/basic-geometric-shape/extruded-solid/extruded-solid.ifc" \
    ifc/ifc43/basic-geometric-shape/extruded-solid.ifc \
    c488f0367106a17289468409531b1b687f72c25cea8e1b46924f59972ad7a8e9 "buildingSMART IFC4.3.x-sample-models (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/IFC4.3.x-sample-models/ef22440ec501551ccc9689309560bbc7e26aa9d7/models/basic-geometric-shape/surface-model/surface-model.ifc" \
    ifc/ifc43/basic-geometric-shape/surface-model.ifc \
    2b53f45bca8b2f98b624afbe74da16590076a902a64d54574fce11bfbda3f942 "buildingSMART IFC4.3.x-sample-models (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/IFC4.3.x-sample-models/ef22440ec501551ccc9689309560bbc7e26aa9d7/models/basic-geometric-shape/triangulated-item/triangulated-item.ifc" \
    ifc/ifc43/basic-geometric-shape/triangulated-item.ifc \
    935acf08d1f1c93035c9f2454928f8ff7ba89378f60e6560fcc07c82f7eb8fd1 "buildingSMART IFC4.3.x-sample-models (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/IFC4.3.x-sample-models/ef22440ec501551ccc9689309560bbc7e26aa9d7/models/building-elements/beam-extruded-solid/beam-extruded-solid.ifc" \
    ifc/ifc43/building-elements/beam-extruded-solid.ifc \
    eaf1dc6bec8371537a7be13fab74c9fd548d4b4e98bf8a800471965c7d7b6b3a "buildingSMART IFC4.3.x-sample-models (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/IFC4.3.x-sample-models/ef22440ec501551ccc9689309560bbc7e26aa9d7/models/building-elements/beam-parametric-cross-section/beam-parametric-cross-section.ifc" \
    ifc/ifc43/building-elements/beam-parametric-cross-section.ifc \
    94ca2a46961854a97905af421b1a5432c2c9d58229dadb07add1e4f354145046 "buildingSMART IFC4.3.x-sample-models (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/IFC4.3.x-sample-models/ef22440ec501551ccc9689309560bbc7e26aa9d7/models/building-elements/beam-revolved-solid/beam-revolved-solid.ifc" \
    ifc/ifc43/building-elements/beam-revolved-solid.ifc \
    1c9a7a8486b404bf3faa674b67a799828dd3ec0d3ca76021764c40b7c5acb50a "buildingSMART IFC4.3.x-sample-models (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/IFC4.3.x-sample-models/ef22440ec501551ccc9689309560bbc7e26aa9d7/models/building-elements/beam-varying-cardinal-points/beam-varying-cardinal-points.ifc" \
    ifc/ifc43/building-elements/beam-varying-cardinal-points.ifc \
    825ba41b32b61993eb54b6945779fdb777feae17620802016699913de3a1dafd "buildingSMART IFC4.3.x-sample-models (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/IFC4.3.x-sample-models/ef22440ec501551ccc9689309560bbc7e26aa9d7/models/building-elements/beam-varying-extrusion-paths/beam-varying-extrusion-paths.ifc" \
    ifc/ifc43/building-elements/beam-varying-extrusion-paths.ifc \
    d7287a53acbe9f38b322556b69e85f8e58bf253a73881a578d1133ee597972a7 "buildingSMART IFC4.3.x-sample-models (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/IFC4.3.x-sample-models/ef22440ec501551ccc9689309560bbc7e26aa9d7/models/building-elements/beam-varying-profiles/beam-varying-profiles.ifc" \
    ifc/ifc43/building-elements/beam-varying-profiles.ifc \
    67f91eb9a17c6adab398115ff3a9ec06e3035a1dc3226b315e147c133b2e0a58 "buildingSMART IFC4.3.x-sample-models (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/IFC4.3.x-sample-models/ef22440ec501551ccc9689309560bbc7e26aa9d7/models/building-elements/column-extruded-solid/column-extruded-solid.ifc" \
    ifc/ifc43/building-elements/column-extruded-solid.ifc \
    e5ada0a5883b0fced6128c25eac53b0b4b1cd23ad2dc3a821d24442558a98ae0 "buildingSMART IFC4.3.x-sample-models (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/IFC4.3.x-sample-models/ef22440ec501551ccc9689309560bbc7e26aa9d7/models/building-elements/reinforcing-assembly/reinforcing-assembly.ifc" \
    ifc/ifc43/building-elements/reinforcing-assembly.ifc \
    4ae0a9b011cd7b56d811afb49ff6aa469466c577ddb52f7ca2693183fa1c645a "buildingSMART IFC4.3.x-sample-models (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/IFC4.3.x-sample-models/ef22440ec501551ccc9689309560bbc7e26aa9d7/models/building-elements/reinforcing-stirrup/reinforcing-stirrup.ifc" \
    ifc/ifc43/building-elements/reinforcing-stirrup.ifc \
    b8905c4bf6dcc18dbb8c0893c3f32327d2e8cff3059f1ee674264afa17267763 "buildingSMART IFC4.3.x-sample-models (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/IFC4.3.x-sample-models/ef22440ec501551ccc9689309560bbc7e26aa9d7/models/building-elements/slab-extrusion/slab-extruded-solid.ifc" \
    ifc/ifc43/building-elements/slab-extruded-solid.ifc \
    0e4b80b98d8f1576d0c997af24b9486eb638de9f8428959fb6067a3bb74a4aca "buildingSMART IFC4.3.x-sample-models (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/IFC4.3.x-sample-models/ef22440ec501551ccc9689309560bbc7e26aa9d7/models/building-elements/slab-openings/slab-openings.ifc" \
    ifc/ifc43/building-elements/slab-openings.ifc \
    958350475139ad5ee7bfdf1181c259c9f188c977302233c10dbe5644b5843a90 "buildingSMART IFC4.3.x-sample-models (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/IFC4.3.x-sample-models/ef22440ec501551ccc9689309560bbc7e26aa9d7/models/building-elements/wall-extrusion/wall-extruded-solid.ifc" \
    ifc/ifc43/building-elements/wall-extruded-solid.ifc \
    7e1c9a3147300d3d9e787b657766cadb2d578930eccf8956313f2f4ee976ff99 "buildingSMART IFC4.3.x-sample-models (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/IFC4.3.x-sample-models/ef22440ec501551ccc9689309560bbc7e26aa9d7/models/construction-scheduling/construction-scheduling-task/construction-scheduling-task.ifc" \
    ifc/ifc43/construction-scheduling/construction-scheduling-task.ifc \
    abbb30934dd36ea1370d7464300cd5a33c85ebd438ab10ed2fd258823514fb7d "buildingSMART IFC4.3.x-sample-models (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/IFC4.3.x-sample-models/ef22440ec501551ccc9689309560bbc7e26aa9d7/models/mapped-geometric-shape/mapped-shape-with-multiple-items/mapped-shape-with-multiple-items.ifc" \
    ifc/ifc43/mapped-geometric-shape/mapped-shape-with-multiple-items.ifc \
    83fc89342d05dc9682f661f4ae6f0053b0832c830ba71da34c70191f04ca6ad7 "buildingSMART IFC4.3.x-sample-models (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/IFC4.3.x-sample-models/ef22440ec501551ccc9689309560bbc7e26aa9d7/models/mapped-geometric-shape/mapped-shape-with-transformation/mapped-shape-with-transformation.ifc" \
    ifc/ifc43/mapped-geometric-shape/mapped-shape-with-transformation.ifc \
    92b95e00e087b6a5ff5d3662a20861874f47932aa5d816ac1e558730b98099f9 "buildingSMART IFC4.3.x-sample-models (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/IFC4.3.x-sample-models/ef22440ec501551ccc9689309560bbc7e26aa9d7/models/mapped-geometric-shape/mapped-shape-without-transformation/mapped-shape-without-transformation.ifc" \
    ifc/ifc43/mapped-geometric-shape/mapped-shape-without-transformation.ifc \
    2f5b3eb441d06d11e9465d6f5be865765c6086de2c0230a8359f99486c259bec "buildingSMART IFC4.3.x-sample-models (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/IFC4.3.x-sample-models/ef22440ec501551ccc9689309560bbc7e26aa9d7/models/placements-and-gis-referencing/geographic-referencing-gk/geographic-referencing-gk.ifc" \
    ifc/ifc43/placements-and-gis-referencing/geographic-referencing-gk.ifc \
    de8088eac8bdcebbe43944c8eba59cc35a184ebabb1266c788b7a4f04945a130 "buildingSMART IFC4.3.x-sample-models (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/IFC4.3.x-sample-models/ef22440ec501551ccc9689309560bbc7e26aa9d7/models/placements-and-gis-referencing/geographic-referencing-rigid-operation/geographic-referencing-rigid-operation.ifc" \
    ifc/ifc43/placements-and-gis-referencing/geographic-referencing-rigid-operation.ifc \
    d295762265d1c7cca1b7bfd65a798cba6838a718d3c923aa69913a9c8f36add4 "buildingSMART IFC4.3.x-sample-models (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/IFC4.3.x-sample-models/ef22440ec501551ccc9689309560bbc7e26aa9d7/models/placements-and-gis-referencing/geographic-referencing-utm/geographic-referencing-utm.ifc" \
    ifc/ifc43/placements-and-gis-referencing/geographic-referencing-utm.ifc \
    81789138aecf9f185bea65a239816e7a303f54408051c377ad6096e480f67762 "buildingSMART IFC4.3.x-sample-models (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/IFC4.3.x-sample-models/ef22440ec501551ccc9689309560bbc7e26aa9d7/models/placements-and-gis-referencing/grid-placement/grid-placement.ifc" \
    ifc/ifc43/placements-and-gis-referencing/grid-placement.ifc \
    b74d0d65041f5bd4d990129e31b546eddf6f722a70fd5a66ed83763a2bbecd02 "buildingSMART IFC4.3.x-sample-models (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/IFC4.3.x-sample-models/ef22440ec501551ccc9689309560bbc7e26aa9d7/models/structural-analysis-model/structural-curve-member/structural-curve-member.ifc" \
    ifc/ifc43/structural-analysis-model/structural-curve-member.ifc \
    9ccd7cc699cddf11979bbba51ba8c239f32077b64e61aabd6edc7de36636e32a "buildingSMART IFC4.3.x-sample-models (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/IFC4.3.x-sample-models/ef22440ec501551ccc9689309560bbc7e26aa9d7/models/tessellated-shape-with-style/tessellation-with-blob-texture/tessellation-with-blob-texture.ifc" \
    ifc/ifc43/tessellated-shape-with-style/tessellation-with-blob-texture.ifc \
    a9984558b52629d8507f01a23aba1d62a84cc7257f8159ac4ae4ac06f71b20fd "buildingSMART IFC4.3.x-sample-models (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/IFC4.3.x-sample-models/ef22440ec501551ccc9689309560bbc7e26aa9d7/models/tessellated-shape-with-style/tessellation-with-image-texture/tessellation-with-image-texture.ifc" \
    ifc/ifc43/tessellated-shape-with-style/tessellation-with-image-texture.ifc \
    eeec637c7c870c289460c5eec18995ec2c2d41af0c26382f32e453a58773c0a6 "buildingSMART IFC4.3.x-sample-models (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/IFC4.3.x-sample-models/ef22440ec501551ccc9689309560bbc7e26aa9d7/models/tessellated-shape-with-style/tessellation-with-individual-colors/tessellation-with-individual-colors.ifc" \
    ifc/ifc43/tessellated-shape-with-style/tessellation-with-individual-colors.ifc \
    49a003e142dce4b047f6c420a804aad68e30c7e4f5af3f7e2db2f9f32560e8a5 "buildingSMART IFC4.3.x-sample-models (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/IFC4.3.x-sample-models/ef22440ec501551ccc9689309560bbc7e26aa9d7/models/tessellated-shape-with-style/tessellation-with-pixel-texture/tessellation-with-pixel-texture.ifc" \
    ifc/ifc43/tessellated-shape-with-style/tessellation-with-pixel-texture.ifc \
    522788afc44ba467730e2b8b521d4baa334df299598a6595ffd430c16a0f7e5e "buildingSMART IFC4.3.x-sample-models (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/IFC4.3.x-sample-models/ef22440ec501551ccc9689309560bbc7e26aa9d7/models/tessellated-shape/beam-curved-i-shape-tessellated/beam-curved-i-shape-tessellated.ifc" \
    ifc/ifc43/tessellated-shape/beam-curved-i-shape-tessellated.ifc \
    aaf49fa0c33cb7d19d53b9d5f6e8a3f2d7c79356238eff3b973a10f2cfff0fd7 "buildingSMART IFC4.3.x-sample-models (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/IFC4.3.x-sample-models/ef22440ec501551ccc9689309560bbc7e26aa9d7/models/tessellated-shape/beam-straight-i-shape-tessellated/beam-straight-i-shape-tessellated.ifc" \
    ifc/ifc43/tessellated-shape/beam-straight-i-shape-tessellated.ifc \
    14cfed0af4bf0aa84901552311738c545079b3a827a1e94f45cb13f0da3fcc27 "buildingSMART IFC4.3.x-sample-models (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/IFC4.3.x-sample-models/ef22440ec501551ccc9689309560bbc7e26aa9d7/models/tessellated-shape/column-straight-rectangle-tessellation/column-straight-rectangle-tessellation.ifc" \
    ifc/ifc43/tessellated-shape/column-straight-rectangle-tessellation.ifc \
    c65f0a5e7339ff654b39e63349ed4cdff22b15a7ff25c2fef4e896385dd1f310 "buildingSMART IFC4.3.x-sample-models (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/IFC4.3.x-sample-models/ef22440ec501551ccc9689309560bbc7e26aa9d7/models/tessellated-shape/polygonal-face-tessellation/polygonal-face-tessellation.ifc" \
    ifc/ifc43/tessellated-shape/polygonal-face-tessellation.ifc \
    b6ef01ef5c35a1da1955ab2c41191c5baf9f78e334f9d70e38d675fc49dad0cb "buildingSMART IFC4.3.x-sample-models (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/IFC4.3.x-sample-models/ef22440ec501551ccc9689309560bbc7e26aa9d7/models/tessellated-shape/slab-tessellated-unique-vertices/slab-tessellated-unique-vertices.ifc" \
    ifc/ifc43/tessellated-shape/slab-tessellated-unique-vertices.ifc \
    b1611154ea511526e5de5a58ea90e6cfb5abb2d8a6294d8c5f63cd6e297e912b "buildingSMART IFC4.3.x-sample-models (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/IFC4.3.x-sample-models/ef22440ec501551ccc9689309560bbc7e26aa9d7/models/trimmed-curve-parameters/curve-parameters-in-degrees/curve-parameters-in-degrees.ifc" \
    ifc/ifc43/trimmed-curve-parameters/curve-parameters-in-degrees.ifc \
    97f46c4580c1ca39d50b110a8579dac8b5bde189dad524753d8209d4443494aa "buildingSMART IFC4.3.x-sample-models (buildingSMART sample files; dev use)"
  get "https://raw.githubusercontent.com/buildingSMART/IFC4.3.x-sample-models/ef22440ec501551ccc9689309560bbc7e26aa9d7/models/trimmed-curve-parameters/curve-parameters-in-radians/curve-parameters-in-radians.ifc" \
    ifc/ifc43/trimmed-curve-parameters/curve-parameters-in-radians.ifc \
    358a598b685f33e384ee2d6c5d0c90b18f62db0d67ab999563355189f7f851b2 "buildingSMART IFC4.3.x-sample-models (buildingSMART sample files; dev use)"
  # web-ifc's public test models (real buildings: FZK-Haus, Duplex, Office, Schependomlaan,
  # a dental clinic, an advanced B-rep model, a zipped tower) and its small geometry fixtures.
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/ifcfiles/public/AC20-FZK-Haus.ifc" \
    ifc/webifc/AC20-FZK-Haus.ifc \
    70cc8ff245fc0894201d96496c031005a5cbd7a96b22d8a1b87c5a883fb77994 "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/ifcfiles/public/duplex.ifc" \
    ifc/webifc/duplex.ifc \
    b347a2c8aa8fff6db896a4417a9c50c22ac0ccd7c5cfc22b99b8d29336c606ed "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/ifcfiles/public/Office_A_20110811.ifc" \
    ifc/webifc/Office_A_20110811.ifc \
    3ccc7df480d7ae9d1af148d13634fe1af8f1a75949fdf3cb1f87c60464df4700 "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/ifcfiles/public/IfcOpenHouse_IFC4.ifc" \
    ifc/webifc/IfcOpenHouse_IFC4.ifc \
    71c9466bf02306be58d45b77d738541a0e7410767960b5eed1cf85737dfa34c9 "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/ifcfiles/public/dental_clinic.ifc" \
    ifc/webifc/dental_clinic.ifc \
    2ac970ce065ecac4e0c9e5f453a257169e90d0067f419b7e33533a64ef837880 "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/ifcfiles/public/schependomlaan.ifc" \
    ifc/webifc/schependomlaan.ifc \
    2c3565ca1904f2aa61adab92024cf3755b2c5b21a498144d3094d7cb58cebec7 "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/ifcfiles/public/advanced_model.ifc" \
    ifc/webifc/advanced_model.ifc \
    3c822eed72ab588697f3f4d4c2b074d1bb888802f877218c769845180f2b7ad2 "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/ifcfiles/public/ISSUE_053_20181220Holter_Tower_10.ifczip" \
    ifc/webifc/ISSUE_053_20181220Holter_Tower_10.ifczip \
    369b482dee0a277354e64dec7ea15b957bbb21a0f4cbf05478050f62fc0dde12 "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/ifcfiles/public/ISSUE_171_IfcSurfaceCurveSweptAreaSolid.ifc" \
    ifc/webifc/ISSUE_171_IfcSurfaceCurveSweptAreaSolid.ifc \
    349e073189b4524dabef7a13b2a022ec6a170f0b4adad220145b6d317f722305 "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/ifcfiles/public/ISSUE_044_test_IFCCOMPOSITEPROFILEDEF.ifc" \
    ifc/webifc/ISSUE_044_test_IFCCOMPOSITEPROFILEDEF.ifc \
    fe243d8450063f0dd13e57748537eb441bb454bc6dbbbc1fec92287974838c6e "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/ifcfiles/public/KIT-Simple-Road-Test-Web-IFC4x3_RC2.ifc" \
    ifc/webifc/KIT-Simple-Road-Test-Web-IFC4x3_RC2.ifc \
    0d454882ab1a7d2a4965eabd872c4b6ef31a9cd3e336087bfe3bf164815c7182 "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/ifcfiles/public/Sample_entities.ifc" \
    ifc/webifc/Sample_entities.ifc \
    a117ed9edc29f130145b21c7b63ef10124e356a9529afe07ddff2dcd71a34649 "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/ifcfiles/public/example.ifc" \
    ifc/webifc/example.ifc \
    2003ef0db3eb65f8b8115ed24d91f812aea44b93d1544b2647bf157c8852d45a "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/ifcfiles/public/tested_sample_project.ifc" \
    ifc/webifc/tested_sample_project.ifc \
    1e6a7c10fb2ce79f28d9bef7e4cd4e2529ad6fc625c0807281eac18f03dbf710 "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/boolean-robustness/noisy-face-opening.ifc" \
    ifc/webifc/fixtures/boolean-robustness/noisy-face-opening.ifc \
    e2048a4d4e3b64b52dbfc250fc480ca18bd1efeeebe14e1d6d13d927d3903deb "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/boolean-robustness/wall-openings-1170.ifc" \
    ifc/webifc/fixtures/boolean-robustness/wall-openings-1170.ifc \
    eb276d43421af8d2a50ec466e43d121cbd4a31fcc3f468c4e146a89f83264a1e "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/boolean-robustness/wall-openings-2374.ifc" \
    ifc/webifc/fixtures/boolean-robustness/wall-openings-2374.ifc \
    431904070ec283720c22826e9f84e1764b962065f98483eaee80db8f34f2105e "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/boolean-robustness/wall-openings-2675.ifc" \
    ifc/webifc/fixtures/boolean-robustness/wall-openings-2675.ifc \
    8159a226f7a4640dd53f3e137fdecc3f5592372f4d5009b95c91d3c614a98c48 "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/boolean-robustness/wall-openings-2825.ifc" \
    ifc/webifc/fixtures/boolean-robustness/wall-openings-2825.ifc \
    9e2b35794983e5852e654f548d955819d9b8dc800a0c75b98075b26a16787376 "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/boolean-robustness/wall-openings-2892.ifc" \
    ifc/webifc/fixtures/boolean-robustness/wall-openings-2892.ifc \
    017c2fbabbd0b5338fed6d8c8b5eba2e6d5e491db1619f1c4d7a433c8e2452c5 "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/boolean-robustness/wall-openings-3154.ifc" \
    ifc/webifc/fixtures/boolean-robustness/wall-openings-3154.ifc \
    71c2076ab0409e2025ef0c76b0c88aa99f29fdc647092db9ad158c7f47bb7efb "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/boolean-robustness/wall-openings-3396.ifc" \
    ifc/webifc/fixtures/boolean-robustness/wall-openings-3396.ifc \
    affc3a9637f134e08cf6684c848e074ac805e690d4d4ee91d7eeaa43f135d3e7 "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/boolean-robustness/wall-openings-3926.ifc" \
    ifc/webifc/fixtures/boolean-robustness/wall-openings-3926.ifc \
    872defc959b738ebc1c71b44cb9a211dab6d4d05538a0bccb9ca36a2e8eb879e "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/boolean-robustness/wall-openings-655.ifc" \
    ifc/webifc/fixtures/boolean-robustness/wall-openings-655.ifc \
    ca26ef543fa258f3169708b024e5f0af1aa4855a44ef81c7e32a6092dc5c04a5 "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/boolean-robustness/wall-openings-895.ifc" \
    ifc/webifc/fixtures/boolean-robustness/wall-openings-895.ifc \
    8a85af4a16ec398c1e420a6dd5617c7da554bf14c65d5ccdba17bada9bf895ee "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/circular-seams-and-sphere/ifc2x3-circle.ifc" \
    ifc/webifc/fixtures/circular-seams-and-sphere/ifc2x3-circle.ifc \
    a7648720423b6951e876710561c2a1f83c5b0891ba51c1d73cf04bb54912307c "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/circular-seams-and-sphere/ifc2x3-ellipse.ifc" \
    ifc/webifc/fixtures/circular-seams-and-sphere/ifc2x3-ellipse.ifc \
    fbfdb0638d2a93044dccbbbbf06ffd04ea5e1f9e18f359c76a6af0507f329998 "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/circular-seams-and-sphere/ifc2x3-hollow-circle.ifc" \
    ifc/webifc/fixtures/circular-seams-and-sphere/ifc2x3-hollow-circle.ifc \
    1d6bf72909334a7ca3280ee1a5e9d1a2aa40c0229cd0303d9c68d96ab166410f "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/circular-seams-and-sphere/ifc4-circle.ifc" \
    ifc/webifc/fixtures/circular-seams-and-sphere/ifc4-circle.ifc \
    115a7760a679becea22dc23a6e67685fa0eb5f90b29a14d93e288c64eaac702a "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/circular-seams-and-sphere/ifc4-cylinder.ifc" \
    ifc/webifc/fixtures/circular-seams-and-sphere/ifc4-cylinder.ifc \
    af35a078acc7f28732357f47088b0d62fb5db5832fb1ef5f903655ea9b4bf5dc "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/circular-seams-and-sphere/ifc4-ellipse.ifc" \
    ifc/webifc/fixtures/circular-seams-and-sphere/ifc4-ellipse.ifc \
    80a83299df2367067e3c5cde1df765839823b23b3da35c2e9ed8228c4143f502 "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/circular-seams-and-sphere/ifc4-hollow-circle.ifc" \
    ifc/webifc/fixtures/circular-seams-and-sphere/ifc4-hollow-circle.ifc \
    df455f965a1e6bb8a645845536473d2011b069731bef3b61f0d07d4ef31b1bbe "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/circular-seams-and-sphere/ifc4-sphere.ifc" \
    ifc/webifc/fixtures/circular-seams-and-sphere/ifc4-sphere.ifc \
    fac60b1fd6aa6d943bfd4fd1250f39cb882fc387daf9034a5631b6762184a7da "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/circular-seams-and-sphere/ifc4x3_add2-circle.ifc" \
    ifc/webifc/fixtures/circular-seams-and-sphere/ifc4x3_add2-circle.ifc \
    e3c58fe9bb11e74c465c1cec18fbab1d282737d4ebbe606314c99eda933f65c0 "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/circular-seams-and-sphere/ifc4x3_add2-cylinder.ifc" \
    ifc/webifc/fixtures/circular-seams-and-sphere/ifc4x3_add2-cylinder.ifc \
    42f27e62ac127ae1fd2163bfd66c022c258a80773732ebd9d77ef8ce104f74c5 "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/circular-seams-and-sphere/ifc4x3_add2-ellipse.ifc" \
    ifc/webifc/fixtures/circular-seams-and-sphere/ifc4x3_add2-ellipse.ifc \
    be3656c5ca8ec8604c42a277054b46a2ad44d763f290cd0272618f7e8c285444 "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/circular-seams-and-sphere/ifc4x3_add2-hollow-circle.ifc" \
    ifc/webifc/fixtures/circular-seams-and-sphere/ifc4x3_add2-hollow-circle.ifc \
    8b2648aa03ca7b7ad6b3996f0be5e7665fb0162a309d12dd9fff307973980516 "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/circular-seams-and-sphere/ifc4x3_add2-sphere.ifc" \
    ifc/webifc/fixtures/circular-seams-and-sphere/ifc4x3_add2-sphere.ifc \
    af840ca0bb91256945372f57644be11d783b3a854bb3e7c7c6863552fb74a3fa "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/composite-curve-params/arc-only.ifc" \
    ifc/webifc/fixtures/composite-curve-params/arc-only.ifc \
    d4eb17060146b94f2cdeda6596c735c0d678799ab24343583a922d965b6396be "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/composite-curve-params/end-rounding.ifc" \
    ifc/webifc/fixtures/composite-curve-params/end-rounding.ifc \
    f529b7d50744391417710e520b2e30caf09185603d8a74fa97b645c318a63df8 "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/composite-curve-params/full-range.ifc" \
    ifc/webifc/fixtures/composite-curve-params/full-range.ifc \
    af86fb3d645bbcedb0c1f92bd8c12a9286d874222ee92a48643b5182710f9c79 "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/composite-curve-params/indexed-polycurve-arc-full.ifc" \
    ifc/webifc/fixtures/composite-curve-params/indexed-polycurve-arc-full.ifc \
    86832a7b860600ffd5bbce5633893e8ba674f218f233b25311a681253fb2bb66 "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/composite-curve-params/indexed-polycurve-arc-half.ifc" \
    ifc/webifc/fixtures/composite-curve-params/indexed-polycurve-arc-half.ifc \
    5d7f0834477277e2f5c0bd212a2f9d78757155d400750a4c847609adc36f28ec "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/composite-curve-params/indexed-polycurve-end-rounding.ifc" \
    ifc/webifc/fixtures/composite-curve-params/indexed-polycurve-end-rounding.ifc \
    d2f94b68cd1c97c946015ea2b70f7e33459214aeeaa5fe4a01e1110ea91f161a "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/composite-curve-params/partial.ifc" \
    ifc/webifc/fixtures/composite-curve-params/partial.ifc \
    cc131db9f1054850c28f6765ae4d81c2cadab2eea2623724e147dac5e29757cd "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/composite-curve-params/polyline-end-rounding.ifc" \
    ifc/webifc/fixtures/composite-curve-params/polyline-end-rounding.ifc \
    9c40f78ce9faca9f56f8db8c2bcf4bb5dbdcd03cdf867568d3da3764c764f361 "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/composite-curve-params/polyline-partial.ifc" \
    ifc/webifc/fixtures/composite-curve-params/polyline-partial.ifc \
    c493a2bacce4c3de582218257cc0dadd14fb0076751151b9809e6621d2dd5ad6 "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/derived-profile/ifc4-derived-hole.ifc" \
    ifc/webifc/fixtures/derived-profile/ifc4-derived-hole.ifc \
    47e194698f77f66b46b3f95d78dbd03a69f675689391e5f69f988078a29ec577 "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/hollow-rectangle/ifc4-rectangle-hollow.ifc" \
    ifc/webifc/fixtures/hollow-rectangle/ifc4-rectangle-hollow.ifc \
    55b3eb25637158ad79b7c7abec764bfb9e074b8b96ea669a622809356fd2e30f "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/intersection/ifc4-boolean-intersection.ifc" \
    ifc/webifc/fixtures/intersection/ifc4-boolean-intersection.ifc \
    1dd1c40d8d306d3ffd69573cd2c6430f8296e211d2feb62ec5714cb772a372a0 "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/malformed-input/cyclic-placement.ifc" \
    ifc/webifc/fixtures/malformed-input/cyclic-placement.ifc \
    79a01f120341827da6ecce66a665e6224dc1062bc72e2b7f2a60cc9a5d31a78f "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/malformed-input/invalid-index.ifc" \
    ifc/webifc/fixtures/malformed-input/invalid-index.ifc \
    4f1afe9eef1b15e8da1b65ff6e7521b29aa68a97a405e84a4e92f1dbaa3642d5 "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/malformed-input/missing-placement.ifc" \
    ifc/webifc/fixtures/malformed-input/missing-placement.ifc \
    00bd87d9eff6ff63bcff655fbfed7907196c3cebda4eca0287d8dbba576a712d "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/malformed-input/unterminated-comment.ifc" \
    ifc/webifc/fixtures/malformed-input/unterminated-comment.ifc \
    afc4ad2b40ef88baf57a8b4e43d9430dff33f4f8b9a29a87c4fac55590153903 "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/malformed-input/unterminated-string.ifc" \
    ifc/webifc/fixtures/malformed-input/unterminated-string.ifc \
    60efc6c9d0564d6d2352ab664fb3d2c15d8eb41766bfad4364ba9192e771e0a1 "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/primitives-and-voids/ifc2x3-block.ifc" \
    ifc/webifc/fixtures/primitives-and-voids/ifc2x3-block.ifc \
    331bd105e9cb4a71e340371f870ecac1f308b9b24b5d58a6348facfe470dc152 "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/primitives-and-voids/ifc2x3-brep-void.ifc" \
    ifc/webifc/fixtures/primitives-and-voids/ifc2x3-brep-void.ifc \
    4f4c500729ff514971878f1169ca6c8fcfa23e7f62ff8f257d03f57ffc97add7 "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/primitives-and-voids/ifc2x3-cone.ifc" \
    ifc/webifc/fixtures/primitives-and-voids/ifc2x3-cone.ifc \
    368d4a00f72de7b45846fd5b3773f1753492c217792e32824b614d900ffab06c "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/primitives-and-voids/ifc2x3-pyramid.ifc" \
    ifc/webifc/fixtures/primitives-and-voids/ifc2x3-pyramid.ifc \
    75fca11a36ea5c4a822196299fe6ec20ae4a2e7072e6984b76cdd80e7d179ca7 "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/primitives-and-voids/ifc4-block.ifc" \
    ifc/webifc/fixtures/primitives-and-voids/ifc4-block.ifc \
    5ab115c31262a590d42dde7501772483964920f6aeb9dbf066d767808217c64c "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/primitives-and-voids/ifc4-brep-void.ifc" \
    ifc/webifc/fixtures/primitives-and-voids/ifc4-brep-void.ifc \
    952fbb5686cbd2e2d1d86517505e4df48fe69f15c3a8be62fe017cc89729b885 "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/primitives-and-voids/ifc4-circle.ifc" \
    ifc/webifc/fixtures/primitives-and-voids/ifc4-circle.ifc \
    115a7760a679becea22dc23a6e67685fa0eb5f90b29a14d93e288c64eaac702a "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/primitives-and-voids/ifc4-cone.ifc" \
    ifc/webifc/fixtures/primitives-and-voids/ifc4-cone.ifc \
    ae3b404e9df8f97140cc4a7a8ca28a7c99c1460ed1c175f27da9f5eb70d8d795 "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/primitives-and-voids/ifc4-pyramid.ifc" \
    ifc/webifc/fixtures/primitives-and-voids/ifc4-pyramid.ifc \
    77e13f7b7401da44cae4e4f39a75fddd2f7957169287ed38e58d052437ef9959 "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/primitives-and-voids/ifc4x3_add2-block.ifc" \
    ifc/webifc/fixtures/primitives-and-voids/ifc4x3_add2-block.ifc \
    76db667c897c09409176b1f229bf4e50532f784931013104dc8c14f3597926e8 "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/primitives-and-voids/ifc4x3_add2-brep-void.ifc" \
    ifc/webifc/fixtures/primitives-and-voids/ifc4x3_add2-brep-void.ifc \
    0b3fb413e35caaa5e436832bf18ba49407b787a815d6972f3ffce79d9c64af81 "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/primitives-and-voids/ifc4x3_add2-cone.ifc" \
    ifc/webifc/fixtures/primitives-and-voids/ifc4x3_add2-cone.ifc \
    bba666433d58a1e6a328e741170dd68d1178e5bf49e3888fce1a54a2e2ef3337 "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/primitives-and-voids/ifc4x3_add2-pyramid.ifc" \
    ifc/webifc/fixtures/primitives-and-voids/ifc4x3_add2-pyramid.ifc \
    2db894bc281d0eaf85b0fc49ce90ecf1b2921306ad862749a6f685129d1a8804 "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/revolved-solids/ifc4-revolve-1.57.ifc" \
    ifc/webifc/fixtures/revolved-solids/ifc4-revolve-1.57.ifc \
    e7c5adb9daa19be5591d99cc0df2784b00b5c9084d8e106ffac244f490760d79 "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/revolved-solids/ifc4-revolve-6.28.ifc" \
    ifc/webifc/fixtures/revolved-solids/ifc4-revolve-6.28.ifc \
    524240b08eaf088e2c98b67bf13b372c7d0c0c17b28f9bca7db230cce519347a "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/swept-disk/ifc4-swept-disk-hollow.ifc" \
    ifc/webifc/fixtures/swept-disk/ifc4-swept-disk-hollow.ifc \
    b14c10bdcac925db364e8768dedc8fc048b6dcc58d5641d9074f9e419f2ad47b "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/t-profile/ifc4-t-profile.ifc" \
    ifc/webifc/fixtures/t-profile/ifc4-t-profile.ifc \
    5fcb1dedb3a14e9f49e28cc5f61b0723f0dff16a4f29790d49355358c8bfddc6 "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/tapered-extrusion/ifc4-tapered-hole.ifc" \
    ifc/webifc/fixtures/tapered-extrusion/ifc4-tapered-hole.ifc \
    cedd64871b94b059ebbe1ce83948743edfb7075c866a03e377bcafd3aaf3bbed "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
  get "https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/functional/fixtures/tapered-extrusion/ifc4-tapered.ifc" \
    ifc/webifc/fixtures/tapered-extrusion/ifc4-tapered.ifc \
    ef05c6acf4d690ed8eae544cb0e0c74b895e815da26a230ae5f1c62275d1808d "web-ifc test models (repository MPL-2.0; models from their public sources; dev use)"
}

src_x3d() {          # X3D and VRML 97 samples (Web3D Consortium examples; x3dom MIT)
  # The X3D examples archive (Web3D licence, ../license.html there): each scene in its XML,
  # classic (.x3dv) and VRML 97 (.wrl) encodings; Inline parts beside their assemblies.
  local vs=https://www.web3d.org/x3d/content/examples/Vrml2Sourcebook
  local cad=https://www.web3d.org/x3d/content/examples/Basic/CAD
  local lic="Web3D Consortium X3D examples licence"
  get "$cad/CadTeapot.wrl" \
    x3d/CadTeapot.wrl 758a4637e7df5e5ab0eb2b57b2d7a45637519979b43fe71e0ebab20911a9ac38 "$lic"
  get "$cad/CadTeapot.x3d" \
    x3d/CadTeapot.x3d f90b298cd032b05cae499dd72a4ab436538c774f3a637e0365fd2ed7038b313b "$lic"
  get "$cad/CadTeapot.x3dv" \
    x3d/CadTeapot.x3dv b4ca00af3dc04ede4ebe41462e1a927c01cb30a672371fe46bbaad8cbc612b29 "$lic"
  get "$cad/CatiaHubAssemblyInline.wrl" \
    x3d/CatiaHubAssemblyInline.wrl bd0ebd20912eee7496cfc15c3508c89c0849c6cf022d985e2befce5d5937a7f6 "$lic"
  get "$cad/CatiaHubAssemblyInline.x3d" \
    x3d/CatiaHubAssemblyInline.x3d 100600f625ab83d3ce909cf846e8d4ad3c563a7a5f47a7ad45f65dbecf0eaa40 "$lic"
  get "$cad/CatiaHubAssemblyInline.x3dv" \
    x3d/CatiaHubAssemblyInline.x3dv 8b13536a51cad809fdf685d0cc257af0e06d4d3b0ad5a6e22ceaa864a3d94590 "$lic"
  get "$cad/CatiaHubBushing.wrl" \
    x3d/CatiaHubBushing.wrl 45562dbd204a0a57bb8b91d0673f25ccad2a7510f306685b1e9b37368fce3a5f "$lic"
  get "$cad/CatiaHubBushing.x3d" \
    x3d/CatiaHubBushing.x3d aae363e19b6b7f7638af2fda051d3bc0c677454678ea1485b61e8550b87d0452 "$lic"
  get "$cad/CatiaHubBushing.x3dv" \
    x3d/CatiaHubBushing.x3dv 2db15675d2a2a2709a5733959ae76a39a9a17c098c27715650c2837d40c2457a "$lic"
  get "$cad/CatiaHubCap.wrl" \
    x3d/CatiaHubCap.wrl 6bf815e3d20d707297d5eb2bf90babd6c804b736fede895ee141ce72c8288d50 "$lic"
  get "$cad/CatiaHubCap.x3d" \
    x3d/CatiaHubCap.x3d c90eb9bbd2bcd5d6840a305264a11ab8d62797505f8e856636d3dc5e7072160f "$lic"
  get "$cad/CatiaHubCap.x3dv" \
    x3d/CatiaHubCap.x3dv 2b0b730aa786dd0b9035a6bd1b7cd0e6340084a80165331d4c020da5973a4311 "$lic"
  get "$cad/CatiaHubDiscWithHoles.wrl" \
    x3d/CatiaHubDiscWithHoles.wrl beab58f28b879400b76b64af8d40ec6994134487dc44675daa485e6c4f80cba5 "$lic"
  get "$cad/CatiaHubDiscWithHoles.x3d" \
    x3d/CatiaHubDiscWithHoles.x3d aca8a76424a0b2327959870b65f7f27eb5a42f36f387e2c95fb2eb35a29a7f6b "$lic"
  get "$cad/CatiaHubDiscWithHoles.x3dv" \
    x3d/CatiaHubDiscWithHoles.x3dv ff94c35f006477536c70ac4bdb376f17c54d20e54f8cf613e02001e24a1b6511 "$lic"
  get "$cad/CatiaHubGasket.wrl" \
    x3d/CatiaHubGasket.wrl db10afef142cc76a9bce0547d0f5f286d89441304f0de7301dee651ad9dc1628 "$lic"
  get "$cad/CatiaHubGasket.x3d" \
    x3d/CatiaHubGasket.x3d d0701dcd295a08e0eef13d6333cd730cef08e402b705c7deffb436669b6977f2 "$lic"
  get "$cad/CatiaHubGasket.x3dv" \
    x3d/CatiaHubGasket.x3dv 3872bd3a92ea97e45bc69de97e7b90d0e4ec5d5367568aeaf0124b6e0f26a5c6 "$lic"
  get "$vs/Chapter12Inline/Figure12_1SimpleArch.wrl" \
    x3d/Figure12_1SimpleArch.wrl 0dc7651f0fa29a58f19516cac80e374930c20605ed7d3d8664a808c442a0bbc7 "$lic"
  get "$vs/Chapter12Inline/Figure12_1SimpleArch.x3d" \
    x3d/Figure12_1SimpleArch.x3d fec680b6342b7d9b4b2d0dce33c2e4254ad0b561df0fdc9883d57989e1edb2d7 "$lic"
  get "$vs/Chapter12Inline/Figure12_2InlinedArches.wrl" \
    x3d/Figure12_2InlinedArches.wrl 3a11ea55dab6b6124825b758cea7f73b53ddad607e013a199d1cbf75c326c48d "$lic"
  get "$vs/Chapter12Inline/Figure12_2InlinedArches.x3d" \
    x3d/Figure12_2InlinedArches.x3d 0adc80de4be23eeb70a48dbf5a2a0d174a3ed97cf623a49a13290d386122f1e5 "$lic"
  get "$vs/Chapter12Inline/Figure12_3TempleOfInlinedArches.wrl" \
    x3d/Figure12_3TempleOfInlinedArches.wrl 95808be4b6cdeccfb1ab326ced41280ba7acdb181a206a87949d2a5374665ea3 "$lic"
  get "$vs/Chapter12Inline/Figure12_3TempleOfInlinedArches.x3d" \
    x3d/Figure12_3TempleOfInlinedArches.x3d ab87d135fe93cf88adaeb47632338712ee388b55e346768f6f761514a8a58c36 "$lic"
  get "$vs/Chapter13PointsLinesFaces/Figure13_13IndexedFaceSetLightningBolt.wrl" \
    x3d/Figure13_13IndexedFaceSetLightningBolt.wrl 121033e40585e9999a024267f1d9363903d9831038158485e218d76810abae07 "$lic"
  get "$vs/Chapter13PointsLinesFaces/Figure13_13IndexedFaceSetLightningBolt.x3d" \
    x3d/Figure13_13IndexedFaceSetLightningBolt.x3d 4bc848bbcd0a0f32e8c57f7e62daa34e06636375f6735d17e731918320d2483c "$lic"
  get "$vs/Chapter13PointsLinesFaces/Figure13_15VaultedCeilingPiece.wrl" \
    x3d/Figure13_15VaultedCeilingPiece.wrl 2d6f20d2ceb45af9f770d0fee61d7118771b823bf028191ddca06a4d549652da "$lic"
  get "$vs/Chapter13PointsLinesFaces/Figure13_15VaultedCeilingPiece.x3d" \
    x3d/Figure13_15VaultedCeilingPiece.x3d 86be1ffe839ce1264941383734df1fac39b0aa8c147309c46868b08590f992f6 "$lic"
  get "$vs/Chapter13PointsLinesFaces/Figure13_16VaultedCeiling.wrl" \
    x3d/Figure13_16VaultedCeiling.wrl 883a65a778052daf9c3874032e2ce9e0d4fb33d83ba643bda915686145bd544e "$lic"
  get "$vs/Chapter13PointsLinesFaces/Figure13_16VaultedCeiling.x3d" \
    x3d/Figure13_16VaultedCeiling.x3d fbd17d39e868b6c2a24c4598610e7984ecc1a0920c369622f57b168fdfdc156c "$lic"
  get "$vs/Chapter14ElevationGrid/Figure14_2ElevationGridMountain.wrl" \
    x3d/Figure14_2ElevationGridMountain.wrl 21bb812bc1adb5579facb18108cb4b65d649f704ae1e60557ab1226d808b06ee "$lic"
  get "$vs/Chapter14ElevationGrid/Figure14_2ElevationGridMountain.x3d" \
    x3d/Figure14_2ElevationGridMountain.x3d d1cdeffb8a4e0d6b6ad785873268db2b7091fbbfeb86f64c7f43fdb3e8f96893 "$lic"
  get "$vs/Chapter15Extrusion/Figure15_12Torus.wrl" \
    x3d/Figure15_12Torus.wrl 7807045a17c7ceb57675798306cb846c7cea9d8bb7d06f5910fc5346bfdaef53 "$lic"
  get "$vs/Chapter15Extrusion/Figure15_12Torus.x3d" \
    x3d/Figure15_12Torus.x3d 06b2f69be473437778b49aaf865130b0bc4fc0c4699df3172610463134ff6448 "$lic"
  get "$vs/Chapter15Extrusion/Figure15_14ExtrudedVase.wrl" \
    x3d/Figure15_14ExtrudedVase.wrl 53d0a5520b75a10eadfb3e0f8754bb6f11296a1f69a7b5d290d36af49c4178fd "$lic"
  get "$vs/Chapter15Extrusion/Figure15_14ExtrudedVase.x3d" \
    x3d/Figure15_14ExtrudedVase.x3d 31ebe08149879f54e4c51fd302f39f4c12d4d1a87b162da1edb5ec3826e9a68a "$lic"
  get "$vs/Chapter15Extrusion/Figure15_15TwistedBar.wrl" \
    x3d/Figure15_15TwistedBar.wrl cf28d8fb48e23b75c14f8c88c7a8f779830319b2c8837dd46974987c9b5187e9 "$lic"
  get "$vs/Chapter15Extrusion/Figure15_15TwistedBar.x3d" \
    x3d/Figure15_15TwistedBar.x3d 7d8852430a16e51d1073a906a932a3858b3a5318809a98066b90d8cb8668862c "$lic"
  get "$vs/Chapter16Color/Figure16_5LogFireColoredFaces.wrl" \
    x3d/Figure16_5LogFireColoredFaces.wrl 8baeaee5518b83cf5ee6c6c229c8e624ea7db28f16e1564568fc9e3ec4cd0807 "$lic"
  get "$vs/Chapter16Color/Figure16_5LogFireColoredFaces.x3d" \
    x3d/Figure16_5LogFireColoredFaces.x3d cc9621134dec14a98b7e6862e2573a6551939c2d344c656d5bc0c4280f151fd0 "$lic"
  get "$vs/Chapter16Color/Figure16_7VertexColoredElevationGridMountain.wrl" \
    x3d/Figure16_7VertexColoredElevationGridMountain.wrl f90b81d503e53ce49de86a6bf726dcd407fbbeb0c5af77cdc6dd52a3ee44ff35 "$lic"
  get "$vs/Chapter16Color/Figure16_7VertexColoredElevationGridMountain.x3d" \
    x3d/Figure16_7VertexColoredElevationGridMountain.x3d 1e8dc534b86df2b6e2ff58fb2746e20d38d1763d965025ab9d43cfd42a2501f0 "$lic"
  get "$vs/Chapter25LevelOfDetail/Figure25_01TorchHighDetail.wrl" \
    x3d/Figure25_01TorchHighDetail.wrl c9ed4d4f7e908e8c52aca13d890ee16a1723c8768e74166db6c89b564ad9476c "$lic"
  get "$vs/Chapter25LevelOfDetail/Figure25_01TorchHighDetail.x3d" \
    x3d/Figure25_01TorchHighDetail.x3d bcff3b3dc13c728f7e132e742798c5563544279a6b84e4853c4e6afcbbd2c5e2 "$lic"
  get "$vs/Chapter25LevelOfDetail/Figure25_05ThreeTorchesSingleLOD.wrl" \
    x3d/Figure25_05ThreeTorchesSingleLOD.wrl 4031c997bc1346a1e4ff8c1564477578e66c39d007f88644a2c0a5de647a2494 "$lic"
  get "$vs/Chapter25LevelOfDetail/Figure25_05ThreeTorchesSingleLOD.x3d" \
    x3d/Figure25_05ThreeTorchesSingleLOD.x3d 8e25607c4a5ed9cea5ef5675ce19fa4cbd51d675c6c6cc28325d006ba8c876b1 "$lic"
  get "$cad/IndexedQuadSetExample.wrl" \
    x3d/IndexedQuadSetExample.wrl 6f441f56d93d444fa85f133f10e823a3ebfbebe37112aaa973704296ffc2685e "$lic"
  get "$cad/IndexedQuadSetExample.x3d" \
    x3d/IndexedQuadSetExample.x3d 5f3d701c45f9b76a972b728b5cd0ba4c1ce88445d1e080cfdb2a77b2ceb113c2 "$lic"
  get "$cad/IndexedQuadSetExample.x3dv" \
    x3d/IndexedQuadSetExample.x3dv 50ce86f279aec97fc2c68c99d7ca0b7964364dd235f67ac7f40dac1b0d4fe5e4 "$lic"
  get "$cad/QuadSetColor.wrl" \
    x3d/QuadSetColor.wrl 5f8c187883901eee9a9c3723dcc720d63cfaaefa52516ef36fffd1908d843a37 "$lic"
  get "$cad/QuadSetColor.x3d" \
    x3d/QuadSetColor.x3d 586656664ec394a90ef6126b44ce7b2e517f5fd4691c59b293b3e86e326592c5 "$lic"
  get "$cad/QuadSetColor.x3dv" \
    x3d/QuadSetColor.x3dv 0a26749f51fc44f4f809f0493d99af47cc592c336b84c5254f73bc341323559b "$lic"
  get "https://raw.githubusercontent.com/x3dom/x3dom/a1e12d27c555801f5bc18523919da79d0162816a/build/jsdoc/jsdoc-template/base/tutorials/lighting/lights/teapot.x3d" \
    x3d/x3dom-teapot.x3d 3927b218303c1f4541f67a8f6c6f0d2f907b9c3bdbd2efb61549db10a861eafb MIT
  # Gzipped copies, made here from the files above.
  gzip -9nc "$OUT/x3d/CadTeapot.x3d" > "$OUT/x3d/CadTeapot.x3dz"
  gzip -9nc "$OUT/x3d/CadTeapot.x3dv" > "$OUT/x3d/CadTeapot.x3dvz"
  gzip -9nc "$OUT/x3d/CadTeapot.wrl" > "$OUT/x3d/CadTeapot.wrz"
}

src_rhino() {        # Rhino .3dm samples: openNURBS example files (Rhino 1-8), rhino3dm tests
  # openNURBS (mcneel/opennurbs, its MIT-style licence) and rhino3dm (MIT), pinned commits.
  local on="https://raw.githubusercontent.com/mcneel/opennurbs/eb92af3ba1806b0a34a99aba0d3bda83e3d46083/example_files"
  local r3="https://raw.githubusercontent.com/mcneel/rhino3dm/bc7b030d79636834e8e1ace3fb17b6c5ad4e2687/tests/models"
  local lon="openNURBS licence (mcneel/opennurbs)" lr3="MIT (mcneel/rhino3dm)"
  local f
  while read -r f sha; do
    get "$on/$f" "rhino/opennurbs/$(basename "$f" | sed 's/^my_/v6_my_/')" "$sha" "$lon"
  done <<'EOF'
V1/v1_RhinoLogo.3dm 2fc71592168dc19589c1e270ebf523b301d99daeb3a6955ed73d950152dac080
V1/v1_Soccer.3dm 5a194819b3ee640bb075a63ed86ee503a072a9fccd97a52984174e8e561b31e8
V2/v2_my_mesh.3dm f4198471b35a223866403766cf5e8e8510a1556a585a783c17535943f7e28cba
V2/v2_my_brep.3dm 46024ddcf75f73ed2014a50f9ebca377e0b3fdd3c85edc4edbda4edb091c160e
V2/v2_RhinoLogo.3dm e8d4e020884b139e8bf793af14e07296c8cb8bd1edb9f160aca66716826ca610
V3/v3_my_mesh.3dm 13cf687d75ff5b8671830ee22d2013f288fc882657ce56c13e3fe89ebec3114f
V3/v3_Soccer.3dm e31b64212f92875144e2a25d3bd41df2a354e9fadb4dac8f1865539b09a24c2d
V3/v3_RhinoLogo.3dm ee8e47130d072516b18dcafbb5796a854efab563a3a74c2c929446cbf51f851b
V4/v4_Gear.3dm 595cb7511020c44e75f93c18f714c71c5b1d36995eab805f5724b4da619b7630
V4/v4_DinerMug.3dm 35093ef8a8624870268301c3efcce5230e778874b6e24cf88490dca62acddeca
V4/v4_SaltAndPepper.3dm 84bd37b9a97cc5eb0a2ceb9c1de4b96412b8416cd741d3ea95545e5b0fe0208d
V4/v4_MechPartA.3dm a1b0ef69925b5d9223a7d797033055bb766842768a96f7713e1ecaec2763bb31
V4/v4_example_file.3dm da90b35d40b7df1b9150b6f754f0448a6208f27fc7eca1b0547a8d0f165ad10d
V5/v5_my_mesh.3dm 3813836826f8b6ad85087c5d734ed482042612306ec4c31a2598bc21c03f86e7
V5/v5_my_brep.3dm 59a632b9a04c5278053bd4f28d3580bfdc9de160aa9138f45ffe0d2f5b0bcc4c
V5/v5_my_mesh_with_material.3dm ba79a572fb8b67ed352b713a483c18c0eea57e31edd0d237dfcc259d39620cc5
V5/v5_teapot.3dm feb34db3478dc5075a15e228366d26f7c8f63dd5147e8714b445b5169ef92874
V5/v5_extrusions.3dm 63091e38458464761e2ee067e88167847c08f581b08a0b9bf4628d9007f44340
V5/v5_rhino_logo.3dm afa04756691e5438b2f1c194c60da7512d6d03cdca11bf2096be2e35d7d18494
V5/v5_example_file.3dm 217b576fd39788eb3d64d2e980b9d0289be7cd1afd1d97cdb99eb477549c813b
V5/v5_ring.3dm d48456ab79ac79339029ea9296571bdb1db6c14a854ed4cbe6bf7b69dd3a6cc3
V6/my_mesh.3dm fdf39bb1b67a31ad0df166af6a063ce270990fcefce6c6c1e61100c8b4f625fc
V6/my_mesh_with_material.3dm d9080add559f8b54eee07ff12257bd88e1f45dfe01000668ad7516652a0900cb
V6/v6_rhino_logo.3dm 34c8d808e8ae3887545d55928240e180572b7dadda8005648b79a72967229758
V6/v6_rhino_subd_logo.3dm 8f5eef1a8eefdbb6a83545d600f2f4d3868d1409726a336611f0ec7a1e6cd1d5
V7/v7_my_mesh.3dm b6eb64a1dc48f5f51faa8042666f4d3b9867946a2698fdec282f588504ccf21a
V7/v7_my_brep.3dm e7a3d01a0b699f665f11141f6bb0f1f5eb43e0b275c639ee4522831de0bf2d1a
V7/v7_rhino_logo_nurbs.3dm 854730269b0fcaeec41ca29122936c05adcc2b94bad678c4c72e7480cf865fcc
V7/v7_rhino_logo_subd.3dm 0b5e18d167603ae3ac93339a4e5ce9c2017e48e31cb8f003b645200fda76d441
V7/v7_my_surfaces.3dm 7a9bbc2c63cbf4380dd639e0b7e1ad2cfe996dc816833ed9f386ba3ed8c39b1a
V8/v8_caps_and_joins.3dm 5f4f8d98be42f790ae4eb296ea478a9c861be723b16e86fc98a2aef5ce521ba1
V8/v8_selective_clipping.3dm d6a6724d0e69bcae54ac4b9213740167a16a1c1f91cb10444b27f39060739cdf
EOF
  while read -r f sha; do
    get "$r3/$f" "rhino/rhino3dm/$f" "$sha" "$lr3"
  done <<'EOF'
mesh.3dm 59e78629c5c19a5e04a195d746cd6b3981c504fcdcd76423ba507ec9e58e69b7
blocks.3dm 1e428317489c7c22ee68fb93e119079c718a0ba44efa7c89efb10cf0d8491cb8
subd.3dm 8ba98659cd54a5eed98bc26c75bbe26dbae1351324b7130c9d22d8f4458e8839
meshWithTexture.3dm 6d0f789c626990784171758d29e6e616ba7a7e36e0018f9f45bc22bd011a9cb1
file3dm_stuff.3dm e78ca005c86130953a5b4c0c44d068ae1d00665f4c0f6028edd3911a01d4ff88
EOF
}

src_amf() {          # AMF samples: assimp test models (BSD-3), Slic3r test inputs (AGPL-3.0)
  get "https://raw.githubusercontent.com/assimp/assimp/f20e46b3fc1fdf9bc2675fd549d180dda06861f1/test/models/AMF/test1.amf" \
    amf/assimp-test1.amf 1e906ae702d7d034e23f4fc08b6d0ffc5d4f4c138906cb7130816fa0fb7d875e "BSD-3-Clause (assimp)"
  get "https://raw.githubusercontent.com/assimp/assimp/f20e46b3fc1fdf9bc2675fd549d180dda06861f1/test/models/AMF/test2.amf" \
    amf/assimp-test2.amf eb7ae079aaa050e7237b081c39fc3fd1436593875f7663362559b53390a5d385 "BSD-3-Clause (assimp)"
  get "https://raw.githubusercontent.com/assimp/assimp/f20e46b3fc1fdf9bc2675fd549d180dda06861f1/test/models/AMF/test3.amf" \
    amf/assimp-test3.amf a436958c7843168fb43920f6a4bdef29ea4032a1499255f82ad98f2f47848723 "BSD-3-Clause (assimp)"
  get "https://raw.githubusercontent.com/assimp/assimp/f20e46b3fc1fdf9bc2675fd549d180dda06861f1/test/models/AMF/test4.amf" \
    amf/assimp-test4.amf cbed72e0be0301cb7dc588c29e666417bc9ce842426567e468c9e35ba332830f "BSD-3-Clause (assimp)"
  get "https://raw.githubusercontent.com/assimp/assimp/f20e46b3fc1fdf9bc2675fd549d180dda06861f1/test/models/AMF/test5.amf" \
    amf/assimp-test5.amf 7b4e91400233ae453bdd71aa9cb6cbcaccd0df06bc277e3fc1170fe74f21b6d4 "BSD-3-Clause (assimp)"
  get "https://raw.githubusercontent.com/assimp/assimp/f20e46b3fc1fdf9bc2675fd549d180dda06861f1/test/models/AMF/test5a.amf" \
    amf/assimp-test5a.amf 5477a28281ae3c28e1767965ddd2fae45dc9f326b56c8ae52527dd6966441d82 "BSD-3-Clause (assimp)"
  get "https://raw.githubusercontent.com/assimp/assimp/f20e46b3fc1fdf9bc2675fd549d180dda06861f1/test/models/AMF/test6.amf" \
    amf/assimp-test6.amf d39c05732c88124e428074562557a333808c785119c8c9ad8a248a0aaacfee20 "BSD-3-Clause (assimp)"
  get "https://raw.githubusercontent.com/assimp/assimp/f20e46b3fc1fdf9bc2675fd549d180dda06861f1/test/models/AMF/test7.amf" \
    amf/assimp-test7.amf 5b84e7eb1c8b0405ef68366fd2f11d191f7ae5df74ba6e1502a6137a3bafd5dd "BSD-3-Clause (assimp)"
  get "https://raw.githubusercontent.com/assimp/assimp/f20e46b3fc1fdf9bc2675fd549d180dda06861f1/test/models/AMF/test8.amf" \
    amf/assimp-test8.amf dee00cabc972ab6452d3bdc9e2f921a9de60241a621d794ab8bdc2f83ae682ef "BSD-3-Clause (assimp)"
  get "https://raw.githubusercontent.com/assimp/assimp/f20e46b3fc1fdf9bc2675fd549d180dda06861f1/test/models/AMF/test9.amf" \
    amf/assimp-test9.amf f1b1ce1419e8679ea2458b73523f92386c4841b3784175376b5fedbb3d7bc530 "BSD-3-Clause (assimp)"
  get "https://raw.githubusercontent.com/assimp/assimp/f20e46b3fc1fdf9bc2675fd549d180dda06861f1/test/models/AMF/test_with_mat.amf" \
    amf/assimp-test_with_mat.amf 23e32ed4ad893e6f3ff136719877003e3335825cf898abf8c6673873aba34e8a "BSD-3-Clause (assimp)"
  get "https://raw.githubusercontent.com/slic3r/Slic3r/026c1380e0ad12176dd2fc8cdf8f6f181deeb071/src/test/inputs/test_amf/20mmbox.amf" \
    amf/slic3r-20mmbox.amf ff3be11c5dc4f31e683afeab1008400c95170ea99008e37c6dfab4be30e157c3 "AGPL-3.0 (Slic3r)"
  get "https://raw.githubusercontent.com/slic3r/Slic3r/026c1380e0ad12176dd2fc8cdf8f6f181deeb071/src/test/inputs/test_amf/20mmbox_deflated-in_directories.amf" \
    amf/slic3r-20mmbox_deflated-in_directories.amf 196d84c39520ae5acd30dc5baf76c1071ad52fbf7ffd8be1276feb184d0842a4 "AGPL-3.0 (Slic3r)"
  get "https://raw.githubusercontent.com/slic3r/Slic3r/026c1380e0ad12176dd2fc8cdf8f6f181deeb071/src/test/inputs/test_amf/20mmbox_deflated-mult_files.amf" \
    amf/slic3r-20mmbox_deflated-mult_files.amf ad18a915188b8e232ac8c6661e550eaa2d3d906793842725c48b4c32be4626ea "AGPL-3.0 (Slic3r)"
  get "https://raw.githubusercontent.com/slic3r/Slic3r/026c1380e0ad12176dd2fc8cdf8f6f181deeb071/src/test/inputs/test_amf/20mmbox_deflated.amf" \
    amf/slic3r-20mmbox_deflated.amf f707308b505efe4572b80febc707bdc0b8dac03a014d4a85e5f73b9f8bf8944f "AGPL-3.0 (Slic3r)"
  get "https://raw.githubusercontent.com/slic3r/Slic3r/026c1380e0ad12176dd2fc8cdf8f6f181deeb071/src/test/inputs/test_amf/5061-malicious.amf" \
    amf/slic3r-5061-malicious.amf 9676d3b237de3b1d32f14e453c75f9533e32ef831d989a0c5fb2ade17ff6f364 "AGPL-3.0 (Slic3r)"
  get "https://raw.githubusercontent.com/slic3r/Slic3r/026c1380e0ad12176dd2fc8cdf8f6f181deeb071/xs/t/models/amf/FaceColors.amf.xml" \
    amf/slic3r-FaceColors.amf 917c7ef84c1794812dee8856dfc7268bbf6c6ad9016d249d23998fe97578f9e0 "AGPL-3.0 (Slic3r)"
  get "https://raw.githubusercontent.com/slic3r/Slic3r/026c1380e0ad12176dd2fc8cdf8f6f181deeb071/src/test/inputs/test_amf/read-amf.amf" \
    amf/slic3r-read-amf.amf 7df74f7862f6cb6c42eeefcf8ba8dbd88b4b6d68589db9de6cc697500247960d "AGPL-3.0 (Slic3r)"
}

src_ptx() {          # Leica PTX samples: PDAL test data (BSD-3)
  get "https://raw.githubusercontent.com/PDAL/PDAL/29ed1c77b0afdabef20ecf640a81644fa247ca84/test/data/ptx/1.2-with-color.ptx" \
    ptx/1.2-with-color.ptx 0004b6280e08241721fecc26bd62b66d52c16e6048e1a2f4603953c92d604161 "BSD-3-Clause (PDAL)"
  get "https://raw.githubusercontent.com/PDAL/PDAL/29ed1c77b0afdabef20ecf640a81644fa247ca84/test/data/ptx/complex-transform.ptx" \
    ptx/complex-transform.ptx d26c4867b8715f4ef7bf974c7e693f0ffb0b59a363a00994e0292bb8565bafab "BSD-3-Clause (PDAL)"
  get "https://raw.githubusercontent.com/PDAL/PDAL/29ed1c77b0afdabef20ecf640a81644fa247ca84/test/data/ptx/multiple-and-transform.ptx" \
    ptx/multiple-and-transform.ptx b91cf50f80ff9b392eaf1881d16dd094a3b72d0827a911384d3cb05699ad1bca "BSD-3-Clause (PDAL)"
  get "https://raw.githubusercontent.com/PDAL/PDAL/29ed1c77b0afdabef20ecf640a81644fa247ca84/test/data/ptx/no-color.ptx" \
    ptx/no-color.ptx 937c9e9609c27db9d11c5feb37134bc8826403a794032c9a02adf4888e5f3d02 "BSD-3-Clause (PDAL)"
}

src_oracle() {       # ifcopenshell and rhino3dm facts, generated files (skipped without them)
  # IFC elements (counts, bounds, volumes, areas) through IfcOpenShell, a file per process.
  if have_python ifcopenshell; then
    "$PYTHON" -I "$ROOT/crates/formats/specular-mesh/gen/ifc_oracle.py" "$OUT"
  fi
  # X3D through a small independent reader (standard library only).
  "$PYTHON" -I "$ROOT/crates/formats/specular-mesh/gen/x3d_oracle.py" "$OUT"
  # AMF through a small independent reader and assimp (assimp-py).
  if have_python assimp_py; then
    "$PYTHON" -I "$ROOT/crates/formats/specular-mesh/gen/amf_oracle.py" "$OUT"
  fi
  if have_python rhino3dm; then
    "$PYTHON" -I "$ROOT/crates/formats/specular-mesh/gen/rhino_oracle.py" "$OUT"
  fi
}

main() {
  if (( $# == 0 )); then
    set -- src_ifc src_x3d src_rhino src_amf src_ptx src_oracle
    rm -f "$OUT/.provenance.tsv"
  fi
  for s in "$@"; do
    declare -F "$s" > /dev/null || die "unknown section $s"
    echo "$s"
    "$s"
  done
  rm -rf "$SCRATCH"
}

main "$@"
