#!/usr/bin/env bash
# Fetch a federated IFC scene for the federation and cross-file clash tests into
# tools/corpus/ifc-federation/ (gitignored): buildingSMART's "Simple-Scene" (architecture,
# structure, HVAC, landscaping and infrastructure models of one georeferenced site) in
# IFC2X3, IFC4 and IFC4X3, about 17 MB.
#
# The specular-bim test `federation_corpus` reads them from SPECULAR_IFC_FED_CORPUS, else
# tools/corpus/ifc-federation, and skips when they are absent.
#
# Usage: tools/corpus/fetch-ifc-federation.sh [--oracle PYTHON]
#   --oracle runs tools/corpus/ifc-federation-oracle.py with PYTHON (it needs ifcopenshell;
#   a venv under target/ is fine) and writes `oracle.json` next to the models: each
#   model's map conversion as IfcOpenShell reads it, and the clash pairs between models.
#
# Source: https://github.com/buildingSMART/Sample-Test-Files (CC BY 4.0,
# (C) buildingSMART International Ltd.), pinned commit. Every file is checked against its
# SHA-256. Downloads are untrusted data: this script only copies them.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_IFC_FED_CORPUS:-$ROOT/tools/corpus/ifc-federation}
BASE=https://raw.githubusercontent.com/buildingSMART/Sample-Test-Files/80d976a9b193a26a8e928c3e79bff67af1de68a8

die() { echo "fetch-ifc-federation: $*" >&2; exit 1; }

dir_of() {
  case "$1" in
    ifc2x3) echo "IFC%202.3.0.1%20%28IFC%202x3%20TC1%29" ;;
    ifc4) echo "IFC%204.0.2.1%20%28IFC%204%20ADD2%20TC1%29" ;;
    ifc4x3) echo "IFC%204.3.2.0%20%28IFC%204.3%20ADD2%29" ;;
    *) die "unknown schema folder $1" ;;
  esac
}

while read -r sha name; do
  [[ -z "$sha" ]] && continue
  dest="$OUT/$name"
  mkdir -p "$(dirname "$dest")"
  if [[ -f "$dest" ]] && echo "$sha  $dest" | sha256sum -c --status; then
    continue
  fi
  schema=${name%%/*}
  file=${name#*/}
  echo "fetch-ifc-federation: $name"
  curl -fsSL --retry 3 -o "$dest.part" "$BASE/$(dir_of "$schema")/Simple-Scene/$file" \
    || die "download failed: $name"
  echo "$sha  $dest.part" | sha256sum -c --status \
    || { rm -f "$dest.part"; die "SHA-256 mismatch: $name"; }
  mv "$dest.part" "$dest"
done <<'LIST'
c4db65ba847f6b369a95d6c54fa11f4750cbe6d59f021934e8923d8d578e5885 ifc2x3/Building-Architecture.ifc
f39478ce12ae2029eed8b565551412a3a4b518d10a8b501bc5e9c2a8a6d78bdb ifc2x3/Building-Hvac.ifc
5ca6782566f4a263fa742154f05ab6a30608835734fd8cbae38472f91ae8dbaf ifc2x3/Building-Landscaping.ifc
25c09f1ea35dbf51e88b8829c423b9ce13a8264e69c620e932859772937aa25b ifc2x3/Building-Structural.ifc
50e4aa79c42281b6410ea47da16750c78d24acb3913a90780d582ec3b01e50bd ifc2x3/Infra-Bridge.ifc
60b55dfa3513076df211e3232b4dee29fd1f361abc7908f7c87f47d3b5412e98 ifc2x3/Infra-Electrical.ifc
4d868f360b99dfdeef3b79057bd5d3cd41071d959d46196b36297f80750396bc ifc2x3/Infra-Landscaping.ifc
1724daf0cf0ca43797f44e2cdcd18d273c432afc6b411012cc9c1d382408d149 ifc2x3/Infra-Plumbing.ifc
8dc7c13ba9f5758a764c2322e4e585f29651b10eec69dd0be6701f191ebcf223 ifc2x3/Infra-Rail.ifc
45efd6b6d2a26daf17a3f83a331c780234faf0b73069f0410155ffa7ead222f4 ifc2x3/Infra-Road.ifc
8790a1e193e82b8e7e7f337ec2633cd40f2120590317a1443503a25b079e2e80 ifc4/Building-Architecture.ifc
2c17ecad2b0fbd3335420ee42ba48963b5a395294e86bcdcfc786ee559f9a344 ifc4/Building-Hvac.ifc
937dc9c0e013b0dbbf56dc0fa0dc435efe16a2234cd364af820d2cf1deaa3d7f ifc4/Building-Landscaping.ifc
903b5a005901397aa2b235daf6a60f46c099eae286cb01c815abf0a341707432 ifc4/Building-Structural.ifc
6012bb5e0c33d9923d08fd04e561f0c2d897674a8f3066808494addcadae1276 ifc4/Infra-Bridge.ifc
18ec714203238bc33ef767306cfcaa8209fcda186a8fe8ad2db7f204bc6b5a49 ifc4/Infra-Electrical.ifc
aefdccddd57d9c73d7395b0caaface371e548b4dad08cca4f415b3e1c291396b ifc4/Infra-Landscaping.ifc
f44c401de3bbc1b08d7717807b5add12a1c319f98b42a6c633a69fbcf61d83d1 ifc4/Infra-Plumbing.ifc
d284d418d19489a4cfecb4134ee1145d1bc3f3ee9148da3919a4e741e3e63ef0 ifc4/Infra-Rail.ifc
ebc5c85f6da454de08dc63675af242b6572682692682dff1a064a71b6bcd73e8 ifc4/Infra-Road.ifc
96e7103812b15b65eb1fd7802d36b9962833485dce5ead4f440bbf529530881d ifc4x3/Building-Architecture.ifc
22891586f153793d098811caec8a510cd404f6c2f1979fb329eeeebc3f475642 ifc4x3/Building-Hvac.ifc
93253acebf6c2d9bfeb1300c3c8d2fdf663ea24746e5cddb8392aaf50f9a0ca8 ifc4x3/Building-Landscaping.ifc
db64895488aad4eab082c8292f5632145b0fc864c0642d4fe667404882bacf27 ifc4x3/Building-Structural.ifc
97e84cb93e07bfbedbc1bdc57aef161f4998e09accd77175dd0561af2ab4ffab ifc4x3/Infra-Bridge.ifc
39eebb4612c303e5db5adbab54cdf31ab5f04e02b1887ee49953257fa903a544 ifc4x3/Infra-Electrical.ifc
378f25564c4fc969225370e322e8ec0d16ce52239e68dd5572e4681879e33fca ifc4x3/Infra-Landscaping.ifc
a6ac7a0f4c80c91f04a1a66b73899be064f9c3992b2e886c92f0e3145c0ca50f ifc4x3/Infra-Plumbing.ifc
c9073051e17796b8b59f96cf268207f4011c08eedeaa791774480c33e9defbb3 ifc4x3/Infra-Rail.ifc
a3cb29433d7176bd31ff7535a596b5d7cd4eb7f2fb0eb6eee451612063c1f167 ifc4x3/Infra-Road.ifc
LIST

if [[ "${1:-}" == "--oracle" ]]; then
  PY=${2:?--oracle needs a python}
  "$PY" -I "$ROOT/tools/corpus/ifc-federation-oracle.py" "$OUT" "$OUT/oracle.json"
fi
echo "fetch-ifc-federation: done ($OUT)"
