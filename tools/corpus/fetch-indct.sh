#!/usr/bin/env bash
# Fetch the industrial CT / NDT corpus (Zeiss Xradia TXRM/TXM, Amira/Avizo AmiraMesh, Bruker
# SkyScan NRecon reconstructions, Nikon XT H xtekct/xtekVolume headers, Volume Graphics .vgi
# headers) into tools/corpus/indct/ (gitignored: nothing here is committed). Tests and oracles
# read it from SPECULAR_INDCT_CORPUS (a worktree has no corpus of its own), else
# tools/corpus/indct, and skip when it is absent.
#
# Usage: tools/corpus/fetch-indct.sh [SECTION...]
#   SECTION is a function name below (e.g. src_amira). Default: every section.
#   OUT defaults to $SPECULAR_INDCT_CORPUS, else tools/corpus/indct.
#
# About 345 MB downloaded (the two SkyScan zips are unpacked and then removed):
#   txrm/     Zeiss Xradia Versa 510 drift-reference projection series (OLE compound files):
#             ds-uct-007 asphalt concrete (UFF micro-CT facility, Zenodo 3958325, CC-BY-4.0)
#             and the Battery Imaging Library pin-type Li-ion scout (Southampton MuVIS, 2023,
#             Zenodo 17014436, CC-BY-4.0)
#   txm/      a Zeiss Xradia nano-CT reconstruction .txm of a polymer microlattice (Zenodo
#             15389357, CC-BY-4.0). 256 MB: no smaller public .txm reconstruction was found
#             (the HDTomo, ds-uct and uct-c sets ship 2 GB ones); skip with a section list.
#   amira/    AmiraMesh/Avizo lattices: a real micro-CT volume (250x250x328 byte, raw), its
#             HxByteRLE label field and two ASCII colormaps from Zenodo 5931381 (Borophagus
#             coprolite, CC-BY-4.0, saved by Avizo without an .am suffix); OME's samples
#             (downloads.openmicroscopy.org/images/AmiraMesh, CC-BY-4.0): an Avizo
#             big-endian HxZip volume and Ilya Belevich's AmiraMesh 2.1 / 3.0 files with the
#             TIFF they were made from; ahds' test lattices (emdb-empiar/ahds, Apache-2.0):
#             HxByteRLE 284³ labels, float scalar and float[3] vector fields
#   skyscan/  Bruker SkyScan NRecon output: SlicerMorph's sample reconstruction (complete
#             folder: __rec.log, 488 PNG slices, _spr.bmp preview; SlicerMorph/SampleData,
#             which carries no licence file: public tutorial data of the BSD-2-Clause
#             SlicerMorph project) and the SDU SOLID 001 NRecon slices (324 TIFFs without a
#             log; Zenodo 12205501, CC-BY-4.0)
#   nikon/    Nikon XT H / CT Pro headers only: .xtekct, .xtekVolume, .ctprofile.xml,
#             .ctinfo.xml, .ang and a CT Pro .vgi for the stoat scan (figshare 19529398,
#             CC-BY-4.0); xtekct files from Zenodo 4938735 (CC0) and 3352268 (CC-BY-4.0);
#             a .xtekVolume from Zenodo 4306256 (CC0). Every public .vol/.raw found is 4-24 GB,
#             so none is fetched.
#   vgi/      Volume Graphics .vgi headers (CT Pro and VGStudio): Manchester meteorite
#             HUT 18026 with its .xtekVolume (figshare files 36443751/36443754, CC-BY-4.0) and the
#             Zasphinctus papua cybertype (Zenodo 20324596, CC-BY-4.0); their .raw/.vol
#             volumes are 5-15 GB and are not fetched.
#
# Not fetched: DICONDE (ASTM E2339): no public sample files were found (Zenodo, figshare,
# GitHub, web); Digital Rocks Portal and MorphoSource need a login; the Deep Blue (UMMZ)
# xtek/vgi sets refuse scripted access.
#
# Every download is pinned by size and sha256 (entries whose hash is "-" are pinned by size
# only until their hash is recorded: the script prints it). Downloads are untrusted data:
# this script only copies and unpacks files, never runs anything from them. Stops if free
# space on the target filesystem drops below MIN_FREE_GB (default 8).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_INDCT_CORPUS:-$ROOT/tools/corpus/indct}
MIN_FREE_GB=${MIN_FREE_GB:-8}
mkdir -p "$OUT"

die() { echo "fetch-indct: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

provenance() { printf '%s\t%s\t%s\n' "$1" "$2" "$3" >> "$OUT/.provenance.tsv"; }

# pinned URL DEST SIZE SHA256 LICENCE: downloads once and checks the size and hash (a
# mismatch deletes the download and stops).
pinned() {
  local url=$1 dest=$OUT/$2 size=$3 sha=$4 lic=$5
  if [[ -f $dest ]] && [[ $(stat -c %s "$dest") == "$size" ]] \
    && [[ $sha == - || $(sha256sum "$dest" | cut -d' ' -f1) == "$sha" ]]; then
    return
  fi
  check_space
  mkdir -p "$(dirname "$dest")"
  curl -fsSL --retry 3 --max-filesize $((size + 1)) -o "$dest.part" "$url"
  local got_size got
  got_size=$(stat -c %s "$dest.part")
  if [[ $got_size != "$size" ]]; then
    rm -f "$dest.part"
    die "$2: $got_size bytes, expected $size"
  fi
  got=$(sha256sum "$dest.part" | cut -d' ' -f1)
  if [[ $sha == - ]]; then
    echo "  $2: sha256 $got (record it in this script)"
  elif [[ $got != "$sha" ]]; then
    rm -f "$dest.part"
    die "$2: sha256 $got, expected $sha"
  fi
  mv "$dest.part" "$dest"
  provenance "$2" "$url" "$lic"
}

# unpack ZIP DIR MEMBER...: extracts the named members (globs as unzip reads them) into DIR,
# then removes the zip. A stamp file marks a complete extraction.
unpack() {
  local zip=$OUT/$1 dir=$OUT/$2
  shift 2
  mkdir -p "$dir"
  unzip -o -q "$zip" "$@" -d "$dir"
  touch "$dir/.unpacked"
  rm -f "$zip"
}

# ------------------------------------------------------------------------------- sections

ZEN=https://zenodo.org/api/records
FIG=https://ndownloader.figshare.com/files
OME=https://downloads.openmicroscopy.org/images/AmiraMesh
AHDS=https://raw.githubusercontent.com/emdb-empiar/ahds/86ccb902799a5829a1ba5ea24334a7d0a82a7ec7/ahds/data

src_txrm() {
  pinned "$ZEN/3958325/files/ds-uct-007_asphalt_concrete_07um_1600p_Drift.txrm/content" \
    txrm/ds-uct-007_asphalt_concrete_07um_1600p_Drift.txrm 25464832 3f519100de89db223a50d83ce2f1a659b33067f6b7885d9b57c67f1f388ce5a2 CC-BY-4.0
  pinned "$ZEN/3958325/files/_info_ds-uct-007.txt/content" \
    txrm/_info_ds-uct-007.txt 1209 755ad67f5fe91dae370f705c1d0a01e88b7fd1bbefee8c2557efbcbbc3d2ccbd CC-BY-4.0
  pinned "$ZEN/17014436/files/20231012_VERSA_3666_AV_battery_scout_bin2_Drift.txrm/content" \
    txrm/20231012_VERSA_3666_AV_battery_scout_bin2_Drift.txrm 25550848 2ff26ad16e7014c8f8d4c1c90cf2ee3a2c17102ffdc88043d42d7c47cef4834b CC-BY-4.0
}

src_txm() {
  pinned "$ZEN/15389357/files/Without%20defect_0um_reconst.txm/content" \
    txm/Without_defect_0um_reconst.txm 256466944 e11e9a17e0c8344d956b7e4d3590c3f13805a23699d4bfb5b5c3718bbf3ddc7a CC-BY-4.0
}

src_amira() {
  local z=$ZEN/5931381/files
  pinned "$z/8-bit-bin-2.filtered/content" amira/borophagus/8-bit-bin-2.filtered 20500298 d715bce8a12216a7f5de9a3071347f3e637fd806ee78664889faa11f7b7c732d CC-BY-4.0
  pinned "$z/8-bit-bin-2.labels/content" amira/borophagus/8-bit-bin-2.labels 323288 a6b904067e97034511cee0b36ce27d70ff8eec09e894fc0f4258fbe500ead14a CC-BY-4.0
  pinned "$z/grayScale.am/content" amira/borophagus/grayScale.am 23243 f1590040a1103a6867cff37a128517b28c003c701aec46511e6096cf90a0eafa CC-BY-4.0
  pinned "$z/labels.am/content" amira/borophagus/labels.am 1123 0dae131407ecb1f052745c800558f2a6479449a2f8eed91cb1e30a269b890c2c CC-BY-4.0
  pinned "$OME/ignacio/test.am" amira/ome/ignacio_test.am 18130 fe71430f4432353ddc49e12693b7cc877049393d40bbeb22245a2b61f6931323 CC-BY-4.0
  pinned "$OME/ignacio/readme.txt" amira/ome/ignacio_readme.txt 415 da003ee4ac53d431747f47ee7aa6314055d9e7d2d2b179b18ce862afcf7f743e CC-BY-4.0
  pinned "$OME/imagesc-50585/BF_CheckAmira.am" amira/ome/BF_CheckAmira.am 14579 c139b8db9b6e7fa81af999f791cffc23d549b6de8f1a352b39b1907e05bc544e CC-BY-4.0
  pinned "$OME/imagesc-50585/BF_CheckMIB.am" amira/ome/BF_CheckMIB.am 11187 ccea0b0fb0814f215339d6f4a71669fad80d180b5ea815aabda6c8190557330c CC-BY-4.0
  pinned "$OME/imagesc-50585/BF_Check.tif" amira/ome/BF_Check.tif 10284 076c5b0d530c1130ed1b65b14ae33c9007afa83397535aed0468b764bb4d96ac CC-BY-4.0
  pinned "$OME/imagesc-50585/readme.txt" amira/ome/imagesc-50585_readme.txt 301 04b69de31dc09880b2164d277e88ecb469b5ae626faab13ec10045fd2f2ca191 CC-BY-4.0
  pinned "$AHDS/test9.am" amira/ahds/test9.am 361288 eb3c38c890156726c5a4ec2be4955d1aaa305169e85f03020992a9d262ddd00d Apache-2.0
  pinned "$AHDS/testscalar.am" amira/ahds/testscalar.am 1006 63df5c119f561c29f5a62628b517cea25d637751b23eee195c5c8b92bc5b5f8c Apache-2.0
  pinned "$AHDS/testvector3c.am" amira/ahds/testvector3c.am 2603 6190a1cb8009b0fe1a7700a5de5847c80b524d82faa80bfcf5e419d35518b353 Apache-2.0
}

src_skyscan() {
  if [[ ! -f $OUT/skyscan/slicermorph/.unpacked ]]; then
    pinned https://raw.githubusercontent.com/SlicerMorph/SampleData/9e5edcd20a0227c6becf175d04d900e0515ff97e/sample_Skyscan_mCT_reconstruction.zip \
      skyscan/sample_Skyscan_mCT_reconstruction.zip 7589937 745bcd68406d7ce8cffdebcc4d4764a8610c1fdda5e732180dea42c5047c5bf0 \
      "SlicerMorph sample data (no licence file)"
    unpack skyscan/sample_Skyscan_mCT_reconstruction.zip skyscan/slicermorph 'png_recon/*'
  fi
  if [[ ! -f $OUT/skyscan/solid_001/.unpacked ]]; then
    pinned "$ZEN/12205501/files/001_Rec_NRecon.zip/content" skyscan/001_Rec_NRecon.zip \
      2080825 2a89067bc5478b1e8ae8ec332deaf995e14fe09a1e4ab75a6d2627f6f2451efe CC-BY-4.0
    unpack skyscan/001_Rec_NRecon.zip skyscan/solid_001 '001_Rec_NRecon/*'
  fi
}

src_nikon() {
  pinned "$FIG/34714003" nikon/stoat/EmmaClear-Unstained_stoat.xtekct 2437 f6b9d05222b696df086355ab44081088f5a1af6cd3d9c7a828872b7e8ddddafb CC-BY-4.0
  pinned "$FIG/34714000" nikon/stoat/EmmaClear-Unstained_stoat.ctprofile.xml 6170 0135ca5132eda731b33e343c385475fb9e6259b8b1f4f34503f5fcbf75d4029c CC-BY-4.0
  pinned "$FIG/34713997" nikon/stoat/EmmaClear-Unstained_stoat.ctinfo.xml 338 a9f6c65def846dc725b5d90f158fd58bf0bb120444718dfaf4a3e0ebe5e42c08 CC-BY-4.0
  pinned "$FIG/34713994" nikon/stoat/EmmaClear-Unstained_stoat.ang 49181 ddad6ebfaf58b811c64ecb405fde902a05f8873390f5b63d42bdaefbda35363b CC-BY-4.0
  pinned "$FIG/34714204" nikon/stoat/EmmaClear-Unstained_stoat_01.xtekct 2443 89441c11733652ec3f5889555cb5932a173284d37130b627f9e43f2cf04a22cc CC-BY-4.0
  pinned "$FIG/34714207" nikon/stoat/EmmaClear-Unstained_stoat_01.xtekVolume 435834 adf01cd33898b272dce9e0e11a94444cfa5519f79fc9fb416609c972db6e0e14 CC-BY-4.0
  pinned "$FIG/34714201" nikon/stoat/EmmaClear-Unstained_stoat_01.vgi 1957 44553d56f5b8683cb07704b2db393c86e346123dbf0d4435211bfdffb20d8cbe CC-BY-4.0
  pinned "$ZEN/4938735/files/Hedrick_Bone4.xtekct/content" nikon/Hedrick_Bone4.xtekct 1194 393b828b6c5bfa8b16d73d27536cc4c1f283b4e91e90f6137fbebd1a252afe71 CC0-1.0
  pinned "$ZEN/4938735/files/Hedrick_Bone1_01.xtekct/content" nikon/Hedrick_Bone1_01.xtekct \
    2080 70a09e95e988c3e456f89560f7a60b2b25340933cb2c5911ada405cf3decbbbb CC0-1.0
  pinned "$ZEN/4306256/files/Leptodontichthys_Complete_jaw.xtekVolume/content" \
    nikon/Leptodontichthys_Complete_jaw.xtekVolume 341561 842eea16fc64e42cff523b8271c494b0bdfbc1e6988ba64196a72d04465c1a84 CC0-1.0
  if [[ ! -f $OUT/nikon/willow/.unpacked ]]; then
    pinned "$ZEN/3352268/files/CT_Scan_Metadata.zip/content" nikon/CT_Scan_Metadata.zip 16275 d38328ff45c7f1b7796d9e71c72d00e3330c98c7a7394e58fa10fe87c679847c \
      CC-BY-4.0
    unpack nikon/CT_Scan_Metadata.zip nikon/willow 'CT_Scan_META_Data/*'
  fi
}

src_vgi() {
  pinned "$FIG/36443751" vgi/JM_000026CT.vgi 1930 4c946f8eb8873b1d30d0c41e460874171d261134dc5a3947d6bd2b9b13c67e3f CC-BY-4.0
  pinned "$FIG/36443754" vgi/JM_000026CT.xtekVolume 444026 6c325bdee8b1a461437519208f9214b78c5b885e294828f0796bc242a503d352 CC-BY-4.0
  pinned "$ZEN/20324596/files/Zasphinctus%20papua_CASENT0881742.vgi/content" \
    vgi/Zasphinctus_papua_CASENT0881742.vgi 1199 40c8434267f7ca3fda551376b5c5ed91d05f53449d0318e7ac8e930417ddc8f3 CC-BY-4.0
}

SECTIONS=(src_txrm src_txm src_amira src_skyscan src_nikon src_vgi)
if (( $# )); then SECTIONS=("$@"); fi
for s in "${SECTIONS[@]}"; do
  declare -F "$s" >/dev/null || die "no section $s"
  echo "$s"
  "$s"
done
du -sh "$OUT"
