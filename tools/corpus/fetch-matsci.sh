#!/usr/bin/env bash
# Fetch the materials-science test corpus (FEI/Thermo TIA .ser/.emi, Velox EMD, Bruker BCF
# hypermaps, EBSD .ang/.ctf, AFM files: Bruker Nanoscope, Nanonis .sxm, JPK, NT-MDT; areal
# topography: X3P, Zygo .dat/.datx, Keyence .vk4, Sensofar .plu, Alicona .al3d, Digital Surf
# .sur) into tools/corpus/matsci/ (gitignored: nothing here is committed), and write the
# oracles' expected planes next to them. The corpus tests of specular-em, specular-ebsd,
# specular-spm and specular-surf read it from SPECULAR_MATSCI_CORPUS (a worktree has no corpus
# of its own), else tools/corpus/matsci, and skip when it is absent.
#
# Usage: tools/corpus/fetch-matsci.sh [SECTION...]
#   SECTION is a function name below (e.g. src_tia). Default: every src_ section, then the
#   oracle_ sections.
#   OUT defaults to $SPECULAR_MATSCI_CORPUS, else tools/corpus/matsci.
#   PYTHON (default python3) needs numpy, h5py, rosettasciio, orix, surfalize and nanonispy
#   for the oracles. A venv under target/ is fine:
#     python3 -m venv target/matsci-venv && target/matsci-venv/bin/pip install --no-cache-dir \
#       numpy h5py rosettasciio orix surfalize nanonispy
#   then PYTHON=target/matsci-venv/bin/python.
#
# Every download is pinned by sha256. Downloads are untrusted data: this script only copies
# files, never runs anything from them (the oracles read them with python -I from scripts
# outside the corpus). Stops if free space on the target filesystem drops below MIN_FREE_GB
# (default 8). Total under 1 GB.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_MATSCI_CORPUS:-$ROOT/tools/corpus/matsci}
MIN_FREE_GB=${MIN_FREE_GB:-8}
PYTHON=${PYTHON:-python3}
mkdir -p "$OUT"

die() { echo "fetch-matsci: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

provenance() { printf '%s\t%s\t%s\n' "$1" "$2" "$3" >> "$OUT/.provenance.tsv"; }

# pinned URL DEST SHA256 LICENCE: downloads once and checks the hash (a mismatch deletes the
# download and stops).
pinned() {
  local url=$1 dest=$OUT/$2 sha=$3 lic=$4
  if [[ -f $dest ]] && [[ $(sha256sum "$dest" | cut -d' ' -f1) == "$sha" ]]; then
    return
  fi
  check_space
  mkdir -p "$(dirname "$dest")"
  curl -fsSL --retry 3 -o "$dest.part" "$url"
  local got
  got=$(sha256sum "$dest.part" | cut -d' ' -f1)
  if [[ $got != "$sha" ]]; then
    rm -f "$dest.part"
    die "$2: sha256 $got, expected $sha"
  fi
  mv "$dest.part" "$dest"
  provenance "$2" "$url" "$lic"
}

# list BASE_URL DEST_DIR LICENCE < "SHA256 RELPATH" lines (RELPATH may hold spaces and
# parentheses).
list() {
  local base=$1 dir=$2 lic=$3 sha rel n=0
  while read -r sha rel; do
    [[ -z $sha || $sha == \#* ]] && continue
    local enc=${rel// /%20}
    enc=${enc//(/%28}
    enc=${enc//)/%29}
    pinned "$base/$enc" "$dir/$rel" "$sha" "$lic"
    n=$((n + 1))
  done
  echo "  $n files -> $dir"
}

need_python() {
  "$PYTHON" -I -c "import $1" 2>/dev/null || die "$PYTHON lacks '$1' (see the header for a venv)"
}

# Runs one of the crates' oracle scripts over the corpus: oracle CRATE_DIR ARGS...
oracle() {
  local script=$ROOT/crates/$1/gen/oracle.py
  shift
  "$PYTHON" -I "$script" "$OUT" "$@"
}

# ------------------------------------------------------------------------------- sections

RSCIIO=https://github.com/hyperspy/rosettasciio/raw/v0.15.0/rsciio/tests/data

# >>> ebsd
# EBSD maps: MTEX sample data (square and hexagonal .ang from EDAX OIM and ASTAR, .ctf from
# Oxford Channel 5 / AZtec, cubic, hexagonal, orthorhombic, trigonal, tetragonal and monoclinic
# phases) and the orix test data (two-phase duplex steel .ang). About 60 MB.
MTEX=https://github.com/mtex-toolbox/mtex/raw/4ff4852f76bdf5223b3dadeedad9d226cbeca6ce/data/EBSD
ORIXDATA=https://github.com/pyxem/orix-data/raw/fd9d2fa9b771b60b63956a0001425be7c8a5f141

src_ebsd() {
  list "$MTEX" ebsd/mtex "MTEX sample data (GPL-2.0 toolbox repository)" <<'LIST'
71b2a089c44b922e354111ebe373d7898c59ea45f2ce3b6f018462cd4482f9b5 ACOM.ang
764b250d445614fa0009548dfebaf5abe655f5c3c99d48babbd732b4f80d7851 DC06_2uniax.ang
7d38ab292054eb95cd928eb4d9e3d44a3dd2d848ef40c08ca28d9d5347dcab1a ferrite.ang
6e29ecff1323e7cbe0b710181b81bd08965699cf5575dc87c7eda6b47f0043bf olivineopticalmap.ang
c307be17e0fc90a52cb5fecb6d78509222adbf61b59bcd31bfbfd9ae5f0859e2 data.ctf
3d72e24fd82bab014f4e67ce63fce20465f6e62d72f5e17d283774f174510248 eclogite.ctf
f367a28f8b3ad6df24067e22c884dd31f7ff62cb895379a1f18ac7fb031354a3 Forsterite.ctf
aa30286d770de9af19abfe042cc9a3075f2ab4217ef654863750aaa02dea43ba testdata_hex.ctf
8ee3f9dc549e3360546cb697db2b11f2ae6b5a8c55704dc9103d7304fc1a84af testdata_sqr.ctf
cdb0e40625331790803c9c67f6ab56a1908c1997700c13e4bf21f773427171ca twins.ctf
LIST
  pinned "$ORIXDATA/sdss_ferrite_austenite/sdss_ferrite_austenite.ang" \
    ebsd/orix/sdss_ferrite_austenite.ang \
    ac2fc2fc2b8c9c598703c96f9888677bccc9cd88cddd5e78735653fd5504e371 "orix-data repository (pyxem)"
  # Not ti_orientations/Ti_orientations.ctf: an MTEX column export without the Channel Text
  # File header, which neither orix nor Oxford software reads.
}

# orix reads every map: Euler angles, phases, the sample-Z crystal direction in the standard
# sector and its own TSL colour per point, into expected/ebsd/.
oracle_ebsd() {
  need_python orix
  oracle formats/specular-ebsd "$OUT/ebsd" "$OUT/expected/ebsd"
}
# <<< ebsd

# >>> afm
# AFM: Bruker Nanoscope (current and v5.31 headers), Nanonis .sxm, JPK image and QI image,
# NT-MDT. Repositories pinned to commits; licences are the repositories' own.
TOPOSTATS=https://github.com/AFM-SPM/TopoStats/raw/41cfcc5697f55e2ee1d1de1dffd801d5d0fbbc1e/tests/resources
AFMREADER=https://github.com/AFM-SPM/AFMReader/raw/caeecf32ac5900f99c0dfee4b49d27ba4ca0fd77/tests/resources
G80SXM=https://github.com/New-Horizons-SPM/G80SXM/raw/521f85a4edaaf26e7a130278349b0fcb872d2be1/ExampleData
STMANALYSIS=https://github.com/gholds/STMAnalysis/raw/c01887338efdb3f302faa7372540eeb925d4ce25/test_files

src_afm() {
  list "$TOPOSTATS" afm/topostats "TopoStats repository licence" <<'EOF'
d662c5a4f18c8dc73572f33c3c999259ca1e1e5cc52d0e96d5d8af49ba1b63d5 minicircle.spm
628da14982bed8d5fca9742c3a4b93650f0d9254b1f5dca476350a655b60f1b6 plasmids.spm
55a6dbaa290d9ea97571786caa624dc504653ba8788c4d1d162e0839811bf133 old_bruker.002
f6c4d5f7e6abfa64c2b793b5c450c3387a518f693c3afbed31570996f209b591 old_bruker.004
7b1945c97936f005446b2d75be49643a364ecaf8798a2d2b97b092d40f21de2e file.jpk
EOF
  list "$AFMREADER" afm/afmreader "AFMReader repository licence" <<'EOF'
af04539519c8a8a7d498148af7f61288d8f4434a5d8a750a2f47bff999c5d682 sample_0.jpk-qi-image
EOF
  list "$G80SXM" afm/g80sxm "G80SXM repository licence" <<'EOF'
3029e0f87b64588c9a7cf9a8baa96dc94055dbc63866ff51a5eae06c72c202a6 example_TOPO.sxm
704cdca99c59d5dcc79e90473fbb33c49bf657af4ed44c49454b65f1f76403fe example_nc-AFM.sxm
EOF
  list "$STMANALYSIS" afm/stmanalysis "STMAnalysis repository licence" <<'EOF'
c624877b63a278fd409772fcfeb5ba28a01df16b315f23f238783539deaf5672 06-08-2018_blg001.sxm
978355ce138be4bcfaaa59836dac47d15090a1f8d39e8f881876eb45e8e44d4b 4-20-18_BLG_on_HBN007.sxm
EOF
  # NT-MDT sample from the Callisto viewer's site, inside a zip (unpacked by Python's zipfile;
  # nothing in it is run).
  local mdt=$OUT/afm/callisto/Rect_2_2.mdt
  local mdt_sha=0e0b16bfaa1e8e9556e74ff176d08c5fec1af8f9d7d8d15385c2a7b59d01f34e
  if [[ ! -f $mdt ]] || [[ $(sha256sum "$mdt" | cut -d' ' -f1) != "$mdt_sha" ]]; then
    pinned http://callistosoft.narod.ru/Resources/Mdt.zip afm/callisto/Mdt.zip \
      a54d709cf366a0fac252e88d7d8812817ca5372bcc6f549a06c6963990dc9ddd "Callisto sample, licence unstated"
    "$PYTHON" -I -c 'import sys, zipfile; open(sys.argv[2], "wb").write(zipfile.ZipFile(sys.argv[1]).read("Rect_2_2.mdt"))' \
      "$OUT/afm/callisto/Mdt.zip" "$mdt"
    rm -f "$OUT/afm/callisto/Mdt.zip"
    [[ $(sha256sum "$mdt" | cut -d' ' -f1) == "$mdt_sha" ]] || die "Rect_2_2.mdt: sha256 mismatch"
  fi
  echo "  1 file -> afm/callisto"
}

# Expected planes from pySPM, nanonispy, tifffile + AFMReader and an own NT-MDT reader. pySPM,
# AFMReader and tifffile come from AFM_PYLIB, a pip --target directory, e.g.
#   pip install --no-cache-dir --no-deps --target target/afm-pylib pySPM==0.3.4 \
#     AFMReader==0.0.7 tifffile loguru ruamel.yaml igor2
oracle_afm() {
  need_python numpy
  need_python nanonispy
  [[ -n ${AFM_PYLIB:-} ]] || die "set AFM_PYLIB to the directory holding pySPM, AFMReader and tifffile"
  AFM_PYLIB=$AFM_PYLIB "$PYTHON" -I "$ROOT/crates/formats/specular-spm/gen/oracle_afm.py" "$OUT/afm"
}
# <<< afm

# >>> surface
# Areal topography (specular-surf): Digital Surf .sur/.pro from RosettaSciIO's tests; X3P
# from x3ptools; surfalize's test files (AL3D, MetroPro, PLu, VK4, X3P, SUR); Zygo .datx
# from profilometry_nx2.
X3PTOOLS=https://github.com/heike/x3ptools/raw/3a76128d872d0a16c14b74a14396c095a0100224/inst
SURFALIZE=https://github.com/fredericjs/surfalize/raw/b2cabb4fbd4f5ed3373c178351bcafc0ac1f5739/tests/test_files
PROFILOMETRY=https://github.com/htmdec/profilometry_nx2/raw/14609f985763830382eb7a3ad470da3605acf5fe/data_examples

src_surface() {
  list "$RSCIIO/digitalsurf" surface/digitalsurf "GPL-3.0 (RosettaSciIO test data)" <<'LIST'
fdd9936a4b5e205b819b1d82813bb21045b702b4610e8ef8d1d0932d63344f6d test_profile.pro
802f3d915bf9feb7c264ef3f1242df35033da7227e5a7a5924fd37f8f49f4778 test_RGB.sur
15e8b345cc5d67e7399831c881c63362fd92bc075fad8d763f3ff0d26dfe29a2 test_RGBSURFACE.sur
654ea1d855e88fe97373e5554b25494047aded285ba6558a1a328e13ef8729dc test_spectral_map_compressed.sur
f9c863e3fd61be89c3b68cef6fa2434ffedc7e486efe2263c2241109fa58c3f7 test_spectral_map.sur
ea1602de193b73046beb5e700fcac727fb088bf459edeec3494b0362a41bdcb1 test_spectra.pro
b00ea7aab16047eb205a3ed09ea36fbbdee4511945af8cedab000ed03ed75f6d test_spectrum_compressed.pro
14a6971914cd2fbb0def79b70d72bc4bc105b6e18060d3a400d37a11758013e9 test_spectrum.pro
6ed59a9a235c0b6dc7e15f155d0e738c5841cfc0fe78f1861b7e145f9dcaadf4 test_surface.sur
LIST
  list "$X3PTOOLS" surface/x3p "GPL-3 (x3ptools package data)" <<'LIST'
55380f9db67d02edee5dda09993aa0521de9af84dfa2efa0ad9cf0f36e3a9687 csafe-logo.x3p
32513428fcdc0e9b26ae85c938cae1ea8d68cacebcf6a141f780d9964dc654c0 pyramid.x3p
983d7168a879826fc4e24f98a657093934b622b8ad2e607df8ca8d4f8f95397a sample-land.x3p
a8969c117ff1e553938142ef0926cf2a6cdba67a359ed80ae7349ad8a3d81223 testing.x3p
LIST
  list "$SURFALIZE" surface/surfalize "GPL-3.0 (surfalize test files)" <<'LIST'
b60a9c5fb1d5dd0b58158eadb81ebdf5844ef065a54ac4160a89a26e54193ba5 test_1.al3d
d7865bde99e4ce021ccd804ea15ae61dd0134cbba1b7ab4f72c6a90f9b6a0efb test_1.dat
d54b40acdb1d350ca0d5f645ae1368766e8ece579e7460fad69b45ed7e92391b test_1.plu
0647649a44f268c1a4f6a141498a732184cf099508c8b4cdb93c1d00af754abb test_1.vk4
5248da79ebe7623268367c2a95f9d9693a65fd8569914827cfdc4477b1bec70b test_1.x3p
46d2de0250e3b857849a22c0bc8358e0581a5d34f51e4d29089b008f52f8b258 test_2_non_unicode.vk4
d110587f584caa3ed55de9de705b362641de62d1c3be440169bac41c937913a6 test_compressed.sur
8b6ecd2e1c9c9b8e7be91ab5ca3c85b643474a8699a247a55e65ce97bb8c9c58 test_gwy_export.sur
ae897d23af9b63c7039f5f3a1a77ee190e4de77a100a921e39fc68e7d1b48a1f test_int16.x3p
2b958102e399954955ac5ca4882fa7272db80e65c5473b5074c9e023f702b415 test_text.x3p
b0bae27801844f8a2c0eba8835fa457f078ba9fe8a4a6d4d5b2141c981b8c70b test_uncompressed.sur
LIST
  local lic="profilometry_nx2 example data (see the repository)"
  pinned "$PROFILOMETRY/Cu_h32.datx" surface/datx/Cu_h32.datx \
    333c162b1a4537820b21e86d89d501ab85f5d8fc6c2079d6fba16ca9f3c6fd52 "$lic"
  pinned "$PROFILOMETRY/FScuts/Middle.datx" surface/datx/Middle.datx \
    8139b4e50ae3759317d2eeb588800bd1f124d88551068f1c4365493c74ba826c "$lic"
  pinned "$PROFILOMETRY/incrementalpasses/1_Pass_E.datx" surface/datx/1_Pass_E.datx \
    5f6707d417e8c48969334f1593c197a4dcaf207f840e48eeba6d9df7adb82984 "$lic"
  echo "  3 files -> surface/datx"
}

oracle_surface() {
  need_python numpy
  need_python h5py
  need_python rsciio
  need_python surfalize
  oracle formats/specular-surf
}
# <<< surface

# >>> tia
# FEI/Thermo TIA .ser/.emi (images, spectrum images, line profiles, point spectra).
src_tia() {
  list "$RSCIIO" rsciio "GPL-3.0 (RosettaSciIO test data)" <<'LIST'
0b0a18b5b6eeb166e0b363f5f7737003b6308f5c523b2d063c3f38bd66235f89 tia/new/128x128-TEM_search.emi
8b79f81363d41f759ecda84fb91d50463e2a828f532d5b8f9378960e57fc3ae7 tia/new/128x128-TEM_search_1.ser
beff2c0d6e4a5f2d59004925697f9aac3ffbeee82943639fd15ef4f632a82885 tia/new/128x128_TEM_acquire-sum1.emi
44994f43b7ee2363f177bd48afd8da3423f34e8425b3b22366a3f69a9c1649dc tia/new/128x128_TEM_acquire-sum1_1.ser
4e450eca753cc05af99333251c8b95637d926e81b38c4a89d79f936fa317c1dc tia/new/128x128x5-diffraction_preview.emi
a605aae2e51ba2507e4c5e3af409677e4a178b1139016077c45d42646c224f27 tia/new/128x128x5-diffraction_preview_1.ser
b4d3b5368f6b7c1a3a22ae7aa0fc1b9018ce4b7fe356c2342422634ece7e8b41 tia/new/16x16-diffraction_imagel_5x5x256x256_EDS.emi
e47597787fd224a90cd9dcdc5eb1d9fdedb9534e6acccc01d3397e6c03b40744 tia/new/16x16-diffraction_imagel_5x5x256x256_EDS_1.ser
9db7b1a7bdecfa22cd5d6020d88a3961bc6ba9e91ea28f9f6fa6a5e8b0d7797a tia/new/16x16-diffraction_imagel_5x5x256x256_EDS_2.ser
b4d3b5368f6b7c1a3a22ae7aa0fc1b9018ce4b7fe356c2342422634ece7e8b41 tia/new/16x16-diffraction_imagel_5x5x256x256_EDS_copy.emi
e47597787fd224a90cd9dcdc5eb1d9fdedb9534e6acccc01d3397e6c03b40744 tia/new/16x16-diffraction_imagel_5x5x256x256_EDS_copy_1.ser
f8e662cfcdd8a5673ac77895f02796d6088bc680d46bcdfb63f02cfb24c21523 tia/new/16x16-line_profile_horizontal_5x128x128_EDS.emi
e4db49dd2934effb37657f81840dc97c24ddd466feacc8b11b36f3e8003f1058 tia/new/16x16-line_profile_horizontal_5x128x128_EDS_1.ser
44313deaa13c7d8ea6c65f190e978ab310a7e68c420022b5224de3f2ce78bcd7 tia/new/16x16-line_profile_horizontal_5x128x128_EDS_2.ser
7471eda1585ce446533fb184c41dcdd2451548203b961af2d0f15cc1b9337364 tia/new/16x16-spectrum_image_5x5x4000-not_square.emi
6d90a6a3ac1fc015b5d16bac564f7849874d32d928e1fc21a5140845c0d5f2f5 tia/new/16x16-spectrum_image_5x5x4000-not_square_1.ser
5714bda3b3c244bc80acb7a68a2cd0d392111dfb3c91d806f8715535149b52e9 tia/old/03_Scanning Preview.emi
2c95ae5a775d652ee3ac323204fe98756a7ce4da0901906f94bf1a84f9926c26 tia/old/03_Scanning Preview_1.ser
4f703787880808827b4de10b8cc765b7cf4dbf9b610823f7d99f92342911bccc tia/old/16x16-2_point-spectra-2x1024.emi
24fbe2c94435b8bac350c1bc1aed12ff3e415307cc40fdcc940f1e74d184f9e4 tia/old/16x16-2_point-spectra-2x1024_1.ser
2cebbe74d7867fd2b36c3afd884c5517163647212f3bec007ac4970d8f64f943 tia/old/16x16-line_profile_diagonal_10x1024.emi
f9758f56c7ccab50e295f982cf9a7fd8f6ef13513b5512997ac6194f6cb38a62 tia/old/16x16-line_profile_diagonal_10x1024_1.ser
686e62d0ff9f3f04b01f78ace5920db5f272be291a6f0fd4c92532d27daae402 tia/old/16x16-line_profile_horizontal_10x1024.emi
e2f3f7a56f0b87f1b960aa9b7379d85781d56a1e219a48397542702cd93114f6 tia/old/16x16-line_profile_horizontal_10x1024_1.ser
7d390aee54d559f1996c7252784a0504216581bcca0eb3ad3939612cd0283b3e tia/old/16x16-point_spectrum-1x1024.emi
118d531cfea1ac99e49a9cbdf17e8b6c254a6555a4772a58c9e7dca70d8954f8 tia/old/16x16-point_spectrum-1x1024_1.ser
b320b9100670a1edf5414d9020c19fca4a3a6ef31882e79434b86c946d982b00 tia/old/16x16-spectrum_image-5x5x1024.emi
0a2b96111e87b67d56afce6d600207a610547908762ebeb7a2112cc9dac8caa6 tia/old/16x16-spectrum_image-5x5x1024_1.ser
c7eee2a855c18f7c1e180f1a9185bd27d3a642eccfe882e4fd48db99a1b3bbab tia/old/16x16_STEM_BF_DF_acquire.emi
a945e229b6bdaeca56e9f1a75148867aa8c35898ed484dff29a30ffa907f16cf tia/old/16x16_STEM_BF_DF_acquire_1.ser
e90c1a85d897d088ff0d7833c419d0a83f16f8e786ffac1599c861af1e3ced92 tia/old/16x16_STEM_BF_DF_acquire_2.ser
65d9463254b17e7f91c104a1375c015eb985513ec0969975d9c7f1444e858be9 tia/old/16x16_STEM_BF_DF_search.emi
fad438ecc6bc366b8ec8bf448c8ca29cc43e4c74e11fe74f109d25b10908d6ac tia/old/16x16_STEM_BF_DF_search_1.ser
eac31c0b3530571f912ad2f6b15bbf1dc69ca20f98e0fb66fd79f36f88a76fce tia/old/16x16_STEM_BF_DF_search_2.ser
b2a5c97bb7b63ee7714accc78577e3cdc268b273e89ee71f3814d0fa5ad0be87 tia/old/16x16x5_STEM_BF_DF_preview.emi
9ad3a66eb17c6732378be889c557345140f7b3af28e02484c627782843c6978f tia/old/16x16x5_STEM_BF_DF_preview_1.ser
41581a663f6d74d7f65526b67f31a256aba864969b433d68460d1b69f4d10a10 tia/old/16x16x5_STEM_BF_DF_preview_2.ser
d820ec205f047ed1db5e3a785e8f532e99ade7b70f65657aa184c6cce59f98eb tia/old/64x64_TEM_images_acquire.emi
cfb0cb9c6ba5924a8a03db2d35c4864c5d3bb9a28ebe7c8a92710d0d8bcb9a98 tia/old/64x64_TEM_images_acquire_1.ser
843be6975fc74fc1fe33a4965b67e048dd33dee4613f3b36248499abc6edbb36 tia/old/64x64_diffraction_acquire.emi
d5b0b0b0a5800706494e2edd7a4f2e151b2d0383df31f882a5220243c65a22af tia/old/64x64_diffraction_acquire_1.ser
d2096088cda3a47abe6d18404aabdc0a2ae896b3940991aa6cf31c98959a4d49 tia/old/64x64x5_TEM_preview.emi
08f1dc9d52bfb6da3d7d9ae4db16dbd5783332186bacbd4769e9f97a14f31d70 tia/old/64x64x5_TEM_preview_1.ser
8c19dc06797cc0c408e67b7204c6aecfb3e7ac766ecbe69c1dec08171ef35b7a tia/old/X - Au NP EELS_2.ser
04e7d9968776975a483e0be0c0d634cb8da8fca2550cc986046d8f6f2b736449 tia/old/more_ser_then_emi_metadata.emi
9708b9e85cb1b63eaacc785396728cd7e20fca5ae104df4987fa9f10422e90bf tia/old/more_ser_then_emi_metadata_1.ser
307cde0bbea6f3b54a750ad6d6b7db661479cfda4f4d555a4796790df1bf670e tia/old/more_ser_then_emi_metadata_2.ser
976d655dda302d9722febab9a97bd57a37cc83e74f793bc4d3c7af863b8b7df6 tia/old/no_AcquireDate.emi
0c7ec51b8ba4058490313a1e5d78c0ca0ced63182158aeb4e0894184fcdf4a67 tia/old/no_AcquireDate_1.ser
LIST
}
# Bruker Esprit BCF hypermaps (12-bit, instructively packed 8/16-bit, over-16-bit, v2, EBSD).
src_bcf() {
  list "$RSCIIO" rsciio "GPL-3.0 (RosettaSciIO test data)" <<'LIST'
086da87287d2cb8938020e80c165337dd72d63706260c4b8fc2364c3219ae741 bruker/16x16_12bit_packed_8bit.bcf
2ed9597332ee09e760ccc192f84750c18fa6c260b72e4916ac1939691efa259e bruker/30x30_instructively_packed_16bit_compressed.bcf
984e0c4eef0d10704fabd865fd9e1286005aade95e57018e5914bfe54726248c bruker/Hitachi_TM3030Plus.bcf
5c783dc35f2804dacfcff3953665e802067cb119d9e7f20bd24ef9e465aaaa97 bruker/Nope.bcf
b25b73478cdd3793d85920d4b6927d45b868bea08597df3541bab4b1d1b6b63e bruker/P45_the_default_job.bcf
ffbc663fde128439bfe636120aebd3028f73fa97ae15e478836e9e081f17dce7 bruker/bcf-edx-ebsd.bcf
b3200c747c96f50dd3d58cd5cd7effedc1bf2c0304f69332335d84db78adc1ba bruker/bcf_v2_50x50px.bcf
69d51e2d6bf9dfb9857cf129d6e53d80e093c8f7e5d22a7cd429eb0a272583d0 bruker/over16bit.bcf
529ae0de6de470d48c9cceabfdc505237669b09ef52b5038045fabcfde75fa92 bruker/test_TEM.bcf
LIST
}
# Thermo Fisher Velox EMD (zips of v11 files) and FEI EMD examples.
src_emd() {
  list "$RSCIIO" rsciio "GPL-3.0 (RosettaSciIO test data)" <<'LIST'
101512cb1e0d66b63ac62ef4390a8bd654714a41e416c94230229e14e0d9ea62 emd/fei_emd_files.zip
eec20bd422428dc498334143e2a721aa793e79f399bfe16c21cb8b0313ff0c07 emd/fei_example_complex_fft.emd
c06422c623e7a7b18ed8864c99d34590488b98fef85423ac33e1ea10bef66b2f emd/fei_example_dpc_titles.emd
397d5076b0133db608abd985985fad275bf6594823393f72a069020e47c21a1e emd/fei_example_tem_stack.emd
11a15991d706c962ba304cca1227941dde8a946261ece8263a53e99c85d548f7 emd/velox_EELS_EDS.zip
b88ba4093d053da04a7d9ada51055c5936da6c5ca4bb0cd924b6d0fe67f702cd emd/velox_emd_v11_elementSelection.zip
125f0f6b1517e6bb2a1c44f2157b874fe244bb7716f37a9efbda853f16e395c1 emd/velox_emd_version11.zip
LIST
  # The zips hold the Velox files; unpack them beside (only .emd members, no paths kept).
  local z
  for z in fei_emd_files velox_EELS_EDS velox_emd_v11_elementSelection velox_emd_version11; do
    mkdir -p "$OUT/rsciio/emd/$z"
    unzip -q -o -j "$OUT/rsciio/emd/$z.zip" '*.emd' -d "$OUT/rsciio/emd/$z"
  done
}
oracle_em_matsci() {
  need_python rsciio
  need_python numpy
  "$PYTHON" -I "$ROOT/crates/formats/specular-em/gen/oracle_matsci.py" "$OUT/rsciio"
}
# <<< tia

# @@SECTIONS@@

main() {
  local s
  if (( $# == 0 )); then
    # shellcheck disable=SC2046
    set -- $(declare -F | awk '{print $3}' | grep '^src_') \
      $(declare -F | awk '{print $3}' | grep '^oracle_')
  fi
  for s in "$@"; do
    declare -F "$s" > /dev/null || die "unknown section $s"
    echo "$s"
    "$s"
  done
}

main "$@"
