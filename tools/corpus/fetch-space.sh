#!/usr/bin/env bash
# Fetch the planetary image corpus (VICAR, PDS3, PDS4, fpack) into tools/corpus/space/
# (gitignored: nothing here is committed), then write the oracle's expectations. The
# specular-decode test `space_corpus` reads it in place from SPECULAR_SPACE_CORPUS (a
# worktree has no corpus of its own), else from tools/corpus/space, and skips when absent.
#
# Usage: tools/corpus/fetch-space.sh [SECTION...]
#   SECTION is a function name below (e.g. src_gdal). Default: all sections, then the
#   oracle (src_oracle) when PYTHON has rasterio, astropy and numpy.
#   OUT defaults to $SPECULAR_SPACE_CORPUS, else tools/corpus/space.
#   PYTHON (default python3) is for the oracle only; a venv from wheels is enough:
#     python3 -m venv target/space-venv
#     target/space-venv/bin/pip install --no-cache-dir --only-binary=:all: rasterio astropy numpy
#   then PYTHON=target/space-venv/bin/python.
#
# What it covers (about 90 MB, each file pinned by sha256):
#   gdal      GDAL's autotest data at tag v3.11.0 (MIT/X11): PDS3 attached and detached labels
#             (line prefixes, record and byte pointers, FITS and ZIP data, HiRISE JPEG 2000),
#             PDS4 Cartography labels, VICAR with binary prefixes and BASIC/BASIC2 compression
#   nasa      public-domain NASA PDS archive products: Cassini ISS (VICAR with a detached
#             PDS3 label, binary header and prefixes), Cassini VIMS (a QUBE with suffixes),
#             MGS MOLA and LRO LOLA topography (map-projected, LOLA with PDS3 and PDS4
#             labels), MER Pancam (attached PDS3 with embedded VICAR), Mars 2020 Navcam
#             (VICAR with attached ODL and PDS4 labels, mono, RGB and a thumbnail), Galileo
#             SSI (VICAR from a VAX with an SFDU-labelled PDS3 label)
#   oracle    space-oracle.py: synthetic VICAR / PDS3 / PDS4 files over every sample type,
#             byte order (VAX too), band order, prefix and constant; fpack files written by
#             astropy; then expected/: GDAL's (rasterio) values per band for every image,
#             astropy's for the fpack files
#
# Downloads are untrusted data: this script only copies files, never runs anything from
# them (the oracle reads them from a script outside the corpus). Stops if free space drops
# below MIN_FREE_GB (default 6), or the corpus would pass 300 MB.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_SPACE_CORPUS:-$ROOT/tools/corpus/space}
MIN_FREE_GB=${MIN_FREE_GB:-6}
MAX_MB=300
PYTHON=${PYTHON:-python3}
mkdir -p "$OUT"

die() { echo "fetch-space: $*" >&2; exit 1; }

check_space() {
  local avail_kb used_mb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
  used_mb=$(du -sm "$OUT" | cut -f1)
  (( used_mb < MAX_MB )) || die "the corpus is ${used_mb} MB (>= ${MAX_MB} MB); stopping"
}

# provenance RELPATH URL LICENCE
provenance() { printf '%s\t%s\t%s\n' "$1" "$2" "$3" >> "$OUT/.provenance.tsv"; }

# fetch URL DEST SHA256 LICENCE: one file, kept only when its hash matches (a file already
# there with the right hash is not fetched again)
fetch() {
  local url=$1 dest=$2 sum=$3 lic=$4 tmp
  if [[ -f $OUT/$dest ]] && sha256sum "$OUT/$dest" | grep -q "^$sum "; then
    return
  fi
  check_space
  mkdir -p "$(dirname "$OUT/$dest")"
  tmp="$OUT/$dest.part"
  curl -fsSL --retry 3 --max-time 900 -o "$tmp" "$url" || die "$dest: download failed"
  if ! sha256sum "$tmp" | grep -q "^$sum "; then
    rm -f "$tmp"
    die "$dest: sha256 mismatch"
  fi
  mv "$tmp" "$OUT/$dest"
  provenance "$dest" "$url" "$lic"
  echo "  $dest"
}

# ------------------------------------------------------------------------------- sections

GDAL=https://raw.githubusercontent.com/OSGeo/gdal/v3.11.0/autotest/gdrivers/data
src_gdal() {         # GDAL autotest data (MIT/X11)
  local lic="MIT/X11 (GDAL autotest)" line sum rel
  while read -r sum rel; do
    [[ -z $sum || $sum == \#* ]] && continue
    fetch "$GDAL/$rel" "gdal/$rel" "$sum" "$lic"
  done <<'EOF'
14fb4a8a55558d44462fda1e14687c571228876740361cb7bb17aa864a2963e5 pds/BIBQH03N123_D101_T020S03_V03_truncated.IMG
59ed6e9dd19291bbdc230092ab7f6bc46fb0537ee4c9eec15dd58021cd9c12be pds/byte.tif
b222843a8ee5da4375f3297ae0f30eb541536d508d7d4eb74d380da111e0cc6a pds/CE_LAMO_Q_00N_036E_MER_CLR_truncated.IMG
33c72ad599b170ad583b1487517169be36a6abfaa72a0a227f5f88427ae0fdf8 pds/EN0001426030M_truncated.IMG
a7b227793359d8ffacc51027e833edc94db8ef7599b15acd598219d83092425c pds/ESP_013951_1955_RED.LBL
c46f4a74d3b70e207e0c6d9d08e7d1eb0bfd894395c697cd5c0f5fab3df60820 pds/fl73n003_alt_truncated.img
6a46db2f7206eaa68e72bf0529c6cadc84896df1c57b61dc7f7f0a8ccd0dc44f pds/fl73n003_truncated.img
cd2255122e58c467cee62c9e7a09af02a6dfa36ccfe39e581d8124a29354c90b pds/hsp00017ba0_01_ra218s_trr3_truncated.img
0da6b6b3649a45b0d8a73f0046fe7e749a17e5112f46e0f4a302efcf8f8ee359 pds/hsp00017ba0_01_ra218s_trr3_truncated.lbl
1613b5e84a20f6dee79abf36eac054a7b8255a6d3ccea498e6fd0d20bcecdc11 pds/LDEM_4.IMG
bba3c1464f2e63a9b0b1d704c496acce6b00d2d969033a7ad2189eb50bd963f3 pds/LDEM_4.LBL
72f60437b8fb19c738003706bdfb3092560156b556d0ba64717e6b38a4f7788b pds/map_000_038_truncated.fit
71e161b6d6a55d12efd376004c091679dbc6292c029716476e110814f883effe pds/map_000_038_truncated.lbl
b29ed9c57726b40677fadf8c5b0ec5d17649af059b0d61f862b202c739530446 pds/mc02_truncated.img
059ddfe57af545489684872f336d7e38880c4b06c289a2e8262cecb10104c9d0 pds/pds_3177.lbl
dd067b115d4b7748350e3f37bc6e77b616328053adf8154540d937653b1dc009 pds/pds_3355.lbl
56ee1b6a6ac725b67f4ac2b676195ee14861c70dbf4644ecabdd54acfcbafb92 pds/PDS_WITH_ZIP_IMG.LBL
1495afe1da8899fba1b52543c7bf671a593c04920a19ba2f3bf4b9da9e19eca4 pds/PDS_WITH_ZIP_IMG.ZIP
e6b423bc8e243ffe2b37d4c8213ad70f4ae19c25757175ec9f2aa1290f97f6c4 pds/small.raw
b55a841b7b95be907f6bb0d358b8d10c9dce6e485381eb9accb71e653597d9a1 pds4/byte_pds4_cart_1700.img
19763ef58cae862892ba19497667eaa3db1825c877e607724fd6ed1f76813d5d pds4/byte_pds4_cart_1700.xml
fdf2f75b8b242c2ed77a0a39ab967b8bd0ca8a0bc9d32f9cf3711f2b75a90096 pds4/byte_pds4_cart_1700_multi_sds.xml
b55a841b7b95be907f6bb0d358b8d10c9dce6e485381eb9accb71e653597d9a1 pds4/byte_pds4_cart_1b00.img
600aac241ad55aef6d7fb2168acd90e2d4e037de248ca9cd3e89226ed950512e pds4/byte_pds4_cart_1b00.xml
b55a841b7b95be907f6bb0d358b8d10c9dce6e485381eb9accb71e653597d9a1 pds4/byte_pds4_cart_1d00_1933.img
7ffe8d6564110eb09ca3b1dab49616708a61ac10856990edd6ddfab31945d7f3 pds4/byte_pds4_cart_1d00_1933.xml
b55a841b7b95be907f6bb0d358b8d10c9dce6e485381eb9accb71e653597d9a1 pds4/byte_pds4_cart_1g00_1950.img
14f72fd05fd3a08dc88cf57377865ff77058273851d40db355f1d205eb86b2af pds4/byte_pds4_cart_1g00_1950.xml
df3f619804a92fdb4057192dc43dd748ea778adc52bc498ce80524c014b81119 pds4/oblique_cylindrical.IMG
9219d47dce8d2525766e1ae68256c11cc8552e242a565e3db9e6eab185d2a1ea pds4/oblique_cylindrical.xml
f029bd493e522c1f8a4debb81d780e8bc8e21f2cca3d96d8122ce85608eb8829 pds4/template_with_sp.xml
84cfe01f9f96d0badd5ac662c4ea9882584eabc5a094e8ad0a67b1c17ba51712 vicar/test_vicar_truncated.bin
d25cd6dfee61d3828e80d3155540d5d9a319a49dc4feb22b7fcc9549f283b17b vicar/vicar_all_ones_basic2.vic
115d86e07cf713da8000de4aeb30770d22579d7dc6659c8ab5d416e0342e1603 vicar/vicar_binary_prefix.vic
decbf18ae65be14e4fb99662b94a026c19642522d7f9db3b8eba38f5c53fa04a vicar/vicar_byte_basic.vic
424757bb8668de97abf73a07bc480d8701b81b0ee1fdc1c74416d009fce8013f vicar/vicar_byte.vic
4c8fb9ad12a61db63f876b8a6ce78975b48afe1211fbdf0e245e42957d7ef953 vicar/vicar_int16_basic2.vic
EOF
  check_space
}

IMG=https://planetarydata.jpl.nasa.gov/img/data
GEO=https://pds-geosciences.wustl.edu
src_nasa() {         # NASA Planetary Data System archives (public domain)
  local lic="public domain (NASA PDS)" sum rel url
  while read -r sum rel url; do
    [[ -z $sum || $sum == \#* ]] && continue
    fetch "$url" "nasa/$rel" "$sum" "$lic"
  done <<EOF
35b8eb50df68ebda657e6ca3e15b81bfa455de54ecbfbcecbd44bf67926e5c93 cassini-iss/N1454725799_1.IMG $IMG/cassini/cassini_orbiter/coiss_2001/data/1454725799_1455008789/N1454725799_1.IMG
a6f6e3ee53707f2f3b2c0d8f0d63a11ebbbd7691c2bb56b822f281e2e901af05 cassini-iss/N1454725799_1.LBL $IMG/cassini/cassini_orbiter/coiss_2001/data/1454725799_1455008789/N1454725799_1.LBL
dae263f3be4de4e3ad73c249ee82edc95f8586f186cb8b8d06cbd4767f05366c cassini-vims/v1294638283_1.lbl $IMG/cassini/cassini_orbiter/covims_0001/data/1999010T054026_1999010T060958/v1294638283_1.lbl
d50046ee4832e415dc821affb75b8518d2aa4707363e8554ca8ebcb7c7513182 cassini-vims/v1294638283_1.qub $IMG/cassini/cassini_orbiter/covims_0001/data/1999010T054026_1999010T060958/v1294638283_1.qub
d2992bbd60cf124afa9609b3489e80eb805567265e5d0e72975b7def735ef973 galileo/2000r.img $IMG/galileo/galileo_orbiter/go_0017/g1/ganymede/c034963/2000r.img
61fc7157b37db4044ee9bab6a8c35a53caeb7f34269cd297e0be30d35f74581c galileo/2000r.lbl $IMG/galileo/galileo_orbiter/go_0017/g1/ganymede/c034963/2000r.lbl
c04632eba6449af49e3108ed7c25b3b1c450600abd3690df4fc815853a1af476 lola/ldem_4.img $GEO/lro/lro-l-lola-3-rdr-v1/lrolol_1xxx/data/lola_gdr/cylindrical/img/ldem_4.img
b5a197b4d417e3b5e0e84f8fe15f9a1d2203b8eb5d7213236c0ffbc86e940cc6 lola/ldem_4.lbl $GEO/lro/lro-l-lola-3-rdr-v1/lrolol_1xxx/data/lola_gdr/cylindrical/img/ldem_4.lbl
534384777b2eea7c98e5a19516f31e218d0b7b18a00134a51f04002a8aa2f57e lola/ldem_4.xml $GEO/lro/lro-l-lola-3-rdr-v1/lrolol_1xxx/data/lola_gdr/cylindrical/img/ldem_4.xml
a511e40d7a3ea3275945b4da2a1df377133264fab0be94b7434b1cf8907254cb lola/ldem_16.img $GEO/lro/lro-l-lola-3-rdr-v1/lrolol_1xxx/data/lola_gdr/cylindrical/img/ldem_16.img
9aef29463ccc6ed3a3fbe0df3ecd830a99c69e16b564f455507dee2697096579 lola/ldem_16.lbl $GEO/lro/lro-l-lola-3-rdr-v1/lrolol_1xxx/data/lola_gdr/cylindrical/img/ldem_16.lbl
c41ff4e7b458190a85c2340f46bdf2994fb3e3251cd150aed2681ac44880fcbb m2020/NLE_0010_0667834979_567EDR_N0030000NCAM00130_00_0LLJ03.IMG $IMG/mars2020/mars2020_navcam_ops_raw/data/sol/00010/ids/edr/ncam/NLE_0010_0667834979_567EDR_N0030000NCAM00130_00_0LLJ03.IMG
548379d729fa66ff1f2f1b373f70b920c49c3856604937de18ef57ebe779df4c m2020/NLE_0010_0667834979_567EDR_N0030000NCAM00130_00_0LLJ03.xml $IMG/mars2020/mars2020_navcam_ops_raw/data/sol/00010/ids/edr/ncam/NLE_0010_0667834979_567EDR_N0030000NCAM00130_00_0LLJ03.xml
5ab9d3605b20f20c9ed4d93750dcbc0961c323641c38c53576091f25ea9430cc m2020/NLF_0010_0667834979_567EBY_N0030000NCAM00130_00_0LLJ03.IMG $IMG/mars2020/mars2020_navcam_ops_raw/data/sol/00010/ids/edr/ncam/NLF_0010_0667834979_567EBY_N0030000NCAM00130_00_0LLJ03.IMG
8933b666f51538735514b866d0f6ac10abbe8558ed83e38ed5783bd01b5d358f m2020/NLF_0010_0667834979_567EBY_N0030000NCAM00130_00_0LLJ03.xml $IMG/mars2020/mars2020_navcam_ops_raw/data/sol/00010/ids/edr/ncam/NLF_0010_0667834979_567EBY_N0030000NCAM00130_00_0LLJ03.xml
7eebb92e223dc768a2324b49055c84aa5c8ee1c09a25e1792457f2e819291ab9 m2020/NLF_0010_0667835038_916ECM_T0030000NCAM00130_01_600J03.IMG $IMG/mars2020/mars2020_navcam_ops_raw/data/sol/00010/ids/edr/ncam/NLF_0010_0667835038_916ECM_T0030000NCAM00130_01_600J03.IMG
1bb0c59e57b848e627bd202330f131ccdab9f406691a1409fc7904f67af115af m2020/NLF_0010_0667835038_916ECM_T0030000NCAM00130_01_600J03.xml $IMG/mars2020/mars2020_navcam_ops_raw/data/sol/00010/ids/edr/ncam/NLF_0010_0667835038_916ECM_T0030000NCAM00130_01_600J03.xml
f58bd38a43f6bd8d43ba3b35078e06e459fc3b7d2e359091e38524cdc551027c mer/2p127246687edn0211p2104l2m1.img $IMG/mer/spirit/mer2po_0xxx/data/sol0010/edr/2p127246687edn0211p2104l2m1.img
cc92236b5bccd973e981d843e5db2943a1ce2d21a5093f5fba5f1dec5b63b454 mer/2p127246687eth0211p2104l2m1.img $IMG/mer/spirit/mer2po_0xxx/data/sol0010/edr/2p127246687eth0211p2104l2m1.img
25f16fb7aaf857898dcf98bc4f841341a24f8b9f7e98453ca083bc45d897ca2c mola/megt90n000cb.img $GEO/mgs/mgs-m-mola-5-megdr-l3-v1/mgsl_300x/meg004/megt90n000cb.img
5a3fe60256afa1c35fea5d551fe0ab0a9198fb2bcd2c2e161d078aa69627ebac mola/megt90n000cb.lbl $GEO/mgs/mgs-m-mola-5-megdr-l3-v1/mgsl_300x/meg004/megt90n000cb.lbl
EOF
  check_space
}

src_oracle() {       # synthetic files, fpack files and the expectations
  if ! "$PYTHON" -I -c "import rasterio, astropy, numpy" 2>/dev/null; then
    echo "  oracle: skipped ($PYTHON lacks rasterio, astropy or numpy)"
    return
  fi
  check_space
  "$PYTHON" -I "$ROOT/tools/corpus/space-oracle.py" "$OUT"
  check_space
}

main() {
  local sections=("$@")
  if (( ${#sections[@]} == 0 )); then
    sections=(src_gdal src_nasa src_oracle)
  fi
  for s in "${sections[@]}"; do
    declare -F "$s" >/dev/null || die "unknown section $s"
    echo "$s:"
    "$s"
  done
  echo "corpus in $OUT ($(du -sh "$OUT" | cut -f1))"
}

main "$@"
