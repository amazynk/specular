#!/usr/bin/env bash
# Fetch the molecular structure corpus into tools/corpus/mol/ (gitignored: nothing here is
# committed) and write the independent gemmi/RDKit reading of it (the oracle,
# crates/formats/specular-mol/gen/oracle.py) to expected.tsv beside it.
#
# Usage: tools/corpus/fetch-mol.sh
#   OUT defaults to $SPECULAR_MOL_CORPUS, else tools/corpus/mol.
#   PYTHON (default python3) must have gemmi and rdkit for the oracle.
#   Then: SPECULAR_MOL_DIR=tools/corpus/mol SPECULAR_MOL_ORACLE=tools/corpus/mol/expected.tsv
#   cargo test -p specular-mol --test oracle corpus_matches_gemmi_and_rdkit -- --nocapture
#
# What it covers (about 70 MB; RCSB PDB, COD and PubChem data are public domain / CC0):
#   1crn.pdb          crambin, 327 atoms: a small protein with helices, a sheet, disulfides
#   1bna.pdb          the Dickerson–Drew B-DNA dodecamer
#   1hsg.pdb.gz, 1hsg.cif.gz   HIV-1 protease with the inhibitor MK1 (ligand complex)
#   4hhb.cif.gz       haemoglobin with four haems
#   1d3z.cif.gz       ubiquitin by NMR, 10 models
#   1aon.cif.gz       GroEL–GroES, 58 000 atoms
#   4v6x.cif.gz       the human 80S ribosome (mmCIF only), ~240 000 atoms
#   3j3q.cif.gz       the HIV-1 capsid, ~2.4 million atoms: the timing file
#   cod_*.cif         COD small-molecule and mineral structures (several space groups)
#   pubchem_*.sdf     PubChem 3D conformers (aspirin, caffeine, cholesterol, celecoxib)
#
# Downloads are untrusted data: this script only copies files, never runs anything from them.
# Stops if free space on the target filesystem drops below MIN_FREE_GB (default 8).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_MOL_CORPUS:-$ROOT/tools/corpus/mol}
MIN_FREE_GB=${MIN_FREE_GB:-8}
PYTHON=${PYTHON:-python3}
mkdir -p "$OUT"
: > "$OUT/.provenance.tsv"

die() { echo "fetch-mol: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

# get URL DEST LICENCE
get() {
  local url=$1 dest=$2 lic=$3 try
  check_space
  if [[ ! -s "$OUT/$dest" ]]; then
    # COD resets connections under load: retry with a pause.
    for try in 1 2 3 4 5; do
      if curl -fsSL --retry 3 -o "$OUT/$dest.part" "$url"; then
        break
      fi
      [[ $try == 5 ]] && die "download failed: $url"
      sleep $((try * 3))
    done
    mv "$OUT/$dest.part" "$OUT/$dest"
  fi
  printf '%s\t%s\t%s\n' "$dest" "$url" "$lic" >> "$OUT/.provenance.tsv"
}

RCSB=https://files.rcsb.org/download
get "$RCSB/1CRN.pdb" 1crn.pdb "CC0 (RCSB PDB)"
get "$RCSB/1BNA.pdb" 1bna.pdb "CC0 (RCSB PDB)"
get "$RCSB/1HSG.pdb.gz" 1hsg.pdb.gz "CC0 (RCSB PDB)"
for e in 1HSG 4HHB 1D3Z 1AON 4V6X 3J3Q; do
  lower=$(echo "$e" | tr 'A-Z' 'a-z')
  get "$RCSB/$e.cif.gz" "$lower.cif.gz" "CC0 (RCSB PDB)"
done

COD=https://www.crystallography.net/cod
for id in 1000007 1011176 2300084 7022868 9008564 1000118; do
  get "$COD/$id.cif" "cod_$id.cif" "CC0 (Crystallography Open Database)"
  sleep 1
done

PUG=https://pubchem.ncbi.nlm.nih.gov/rest/pug/compound/cid
for cid in 2244 2519 5997 2662; do
  get "$PUG/$cid/record/SDF?record_type=3d" "pubchem_$cid.sdf" "public domain (PubChem)"
done
# Several records in one SD file: the first is read, the count reported.
get "$PUG/2244,2519,5997/record/SDF?record_type=3d" pubchem_multi.sdf "public domain (PubChem)"

echo "fetch-mol: running the oracle"
"$PYTHON" -I "$ROOT/crates/formats/specular-mol/gen/oracle.py" "$OUT" "$OUT/expected.tsv"
du -sh "$OUT"
