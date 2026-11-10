#!/usr/bin/env bash
# Fetch the "cheap volumes" corpus (CCP4/MRC maps, FreeSurfer MGH/MGZ, MINC1/MINC2, NeXus,
# an Eiger master with its data file, Imaris, BAG and an SRTM tile) into tools/corpus/cheapvol/
# (gitignored: nothing here is committed). Tests and oracles read it from
# SPECULAR_CHEAPVOL_CORPUS (a worktree has no corpus of its own), else tools/corpus/cheapvol,
# and skip when it is absent.
#
# Usage: tools/corpus/fetch-cheapvol.sh [SECTION...]
#   SECTION is a function name below (e.g. src_srtm). Default: every section.
#   OUT defaults to $SPECULAR_CHEAPVOL_CORPUS, else tools/corpus/cheapvol.
#
# About 190 MB downloaded (plus 16 MB extracted from the ICBM152 zip, which is then removed):
#   mrc/      EMD-5778 (TRPV1, EMDB, CC0) as .map.gz, the way EMDB serves maps
#   mgh/      the MNE "sample" subject's FreeSurfer T1.mgz (256³, BSD-3-Clause) and nibabel's
#             test.mgz (MIT)
#   minc/     nibabel's MINC1 and MINC2 test volumes (MIT) and the ICBM 2009c T1 template as
#             MINC2 (McConnell Brain Imaging Centre; free use with the copyright notice).
#             BrainWeb serves its volumes only through a web form, so the ICBM template, from
#             the same lab and in the same MINC2 flavour, stands in for it.
#   nexus/    NeXus example data (nexusformat/exampledata): Diamond I16 with an external link
#             to a Pilatus file in a subfolder, and a P45 scan
#   eiger/    Diamond I04 Eiger 2X 16M thaumatin set Therm_6_1 (Zenodo 3385862, CC-BY-4.0):
#             master, NeXus file and the bitshuffle-LZ4 data file behind its external links
#   imaris/   OME's Convallaria 3-channel confocal .ims (downloads.openmicroscopy.org,
#             davemason; see its COPYING)
#   bag/      the BAG reference library's sample.bag (OpenNavigationSurface/BAG)
#   srtm/     N47E011 (1″, 3601²; NASA SRTM, public domain) from the Mapzen/AWS terrain tiles,
#             unpacked to .hgt
#
# Every download is pinned by size and sha256 (entries whose hash is "-" are pinned by size
# only until their hash is recorded: the script prints it). Downloads are untrusted data:
# this script only copies and unpacks files, never runs anything from them. Stops if free
# space on the target filesystem drops below MIN_FREE_GB (default 8).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_CHEAPVOL_CORPUS:-$ROOT/tools/corpus/cheapvol}
MIN_FREE_GB=${MIN_FREE_GB:-8}
mkdir -p "$OUT"

die() { echo "fetch-cheapvol: $*" >&2; exit 1; }

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

# ------------------------------------------------------------------------------- sections

NIBABEL=https://raw.githubusercontent.com/nipy/nibabel/5.2.1/nibabel/tests/data
NEXUS=https://raw.githubusercontent.com/nexusformat/exampledata/eae516807ef7e27d1c45aab3af3a64a679154677
EIGER=https://zenodo.org/api/records/3385862/files

src_mrc() {
  pinned https://ftp.ebi.ac.uk/pub/databases/emdb/structures/EMD-5778/map/emd_5778.map.gz \
    mrc/emd_5778.map.gz 48375630 \
    877cbe37b86561c3dfb255aa2308fefcdd8f51f91928b17c2ef5c8dd3afaaef7 CC0-1.0
}

src_mgh() {
  pinned https://raw.githubusercontent.com/mne-tools/mne-testing-data/0.150/subjects/sample/mri/T1.mgz \
    mgh/sample_T1.mgz 196193 \
    2f7facef5d0aeb5d1e7bd15cbf04412b72b12f2c553c8f725f502f3e35a796cd BSD-3-Clause
  pinned "$NIBABEL/test.mgz" mgh/nibabel_test.mgz 680 \
    ee0d09022fa30dca0c16959a769893141122fe8c04b19708e2264576d2b7a6e9 MIT
}

src_minc() {
  pinned "$NIBABEL/minc2_4d.mnc" minc/minc2_4d.mnc 27738 \
    a942420945d66e98a72fd0bc2e4a5bfd2b5cef6c5f9c9ad286def13a8a35e315 MIT
  pinned "$NIBABEL/minc1_4d.mnc" minc/minc1_4d.mnc 11804 \
    4fc90aa5a51274a99ffec492e1bdddea5c84743123d4345126415d79ccc1da55 MIT
  pinned "$NIBABEL/minc2_1_scale.mnc" minc/minc2_1_scale.mnc 18268 \
    f8910c3cb9a6fb2e4bb0a9bfff1219861109119bc002525c7620c93b1565e49e MIT
  local t1=$OUT/minc/mni_icbm152_t1_tal_nlin_sym_09c.mnc
  if [[ ! -f $t1 ]]; then
    pinned http://www.bic.mni.mcgill.ca/~vfonov/icbm/2009/mni_icbm152_nlin_sym_09c_minc2.zip \
      minc/mni_icbm152_nlin_sym_09c_minc2.zip 59220391 \
      7ca0b9234b4b6e50559af559fd76fe1bb441e7d443c989618af43f449ddf560b "MNI (copyright notice)"
    unzip -o -j -q "$OUT/minc/mni_icbm152_nlin_sym_09c_minc2.zip" \
      mni_icbm152_t1_tal_nlin_sym_09c.mnc -d "$OUT/minc"
    rm -f "$OUT/minc/mni_icbm152_nlin_sym_09c_minc2.zip"
  fi
}

src_nexus() {
  # The I16 file links to the Pilatus frames in its subfolder (a followed external link).
  pinned "$NEXUS/DLS/i16/hdf5/538039.nxs" nexus/i16/538039.nxs 900856 \
    307cb3ed134e50e4348f2f0b4ad8db44737830ea2a4a9d082c4e0760429cbf10 "NeXus example data"
  pinned "$NEXUS/DLS/i16/hdf5/538039-pilatus100k-files/538039.hdf" \
    nexus/i16/538039-pilatus100k-files/538039.hdf 23185368 \
    e756590e0c25c01df8d47c4f12b47a2d0b61726dda5b5f04fe7e752849bc4b71 "NeXus example data"
  pinned "$NEXUS/DLS/p45/hdf5/p45-316.nxs" nexus/p45-316.nxs 4957004 \
    5e1931296c571dfa00e22c35a9b66c03ed0cb9f4f838e510c4a1c38964300b3e "NeXus example data"
}

src_eiger() {
  pinned "$EIGER/Therm_6_1_master.h5/content" eiger/Therm_6_1_master.h5 49528 \
    fc5d9d12ccaff0801fa8cdc20793b3bdee5e5aa4973be8c61240eae56158c104 CC-BY-4.0
  pinned "$EIGER/Therm_6_1_000001.h5/content" eiger/Therm_6_1_000001.h5 18656993 \
    e4fea221e51412591cd2df5b098d8480d65b94f0982b176d3f28bc78df59d59e CC-BY-4.0
  pinned "$EIGER/Therm_6_1.nxs/content" eiger/Therm_6_1.nxs 55664 - CC-BY-4.0
}

src_imaris() {
  pinned https://downloads.openmicroscopy.org/images/Imaris-IMS/davemason/Convallaria_3C_1T_confocal.ims \
    imaris/Convallaria_3C_1T_confocal.ims 7390551 - "OME sample (see COPYING)"
  pinned https://downloads.openmicroscopy.org/images/Imaris-IMS/davemason/COPYING \
    imaris/COPYING 18652 - "OME sample licence"
}

src_bag() {
  pinned https://raw.githubusercontent.com/OpenNavigationSurface/BAG/3d4c1c03c88313c3c196eee0e94159247a71564a/examples/sample-data/sample.bag \
    bag/sample.bag 648985 - "BAG reference library sample"
}

src_srtm() {
  local hgt=$OUT/srtm/N47E011.hgt
  if [[ ! -f $hgt ]]; then
    pinned https://s3.amazonaws.com/elevation-tiles-prod/skadi/N47/N47E011.hgt.gz \
      srtm/N47E011.hgt.gz 14063455 - "public domain (NASA SRTM)"
    gzip -dc "$OUT/srtm/N47E011.hgt.gz" > "$hgt.part"
    [[ $(stat -c %s "$hgt.part") == $((2 * 3601 * 3601)) ]] || die "N47E011.hgt: wrong size"
    mv "$hgt.part" "$hgt"
    rm -f "$OUT/srtm/N47E011.hgt.gz"
  fi
}

SECTIONS=(src_mrc src_mgh src_minc src_nexus src_eiger src_imaris src_bag src_srtm)
if (( $# )); then SECTIONS=("$@"); fi
for s in "${SECTIONS[@]}"; do
  declare -F "$s" >/dev/null || die "no section $s"
  echo "$s"
  "$s"
done
du -sh "$OUT"
