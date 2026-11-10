#!/usr/bin/env bash
# Fetch a small GRIB corpus into tools/corpus/grib/ (gitignored: nothing here is committed).
# crates/formats/specular-grib/tests/oracle.rs (corpus_against_eccodes) reads every file there
# from SPECULAR_GRIB_CORPUS (a worktree has no corpus of its own), else tools/corpus/grib, and
# compares it with ecCodes; it skips when the directory is absent.
#
# Usage: tools/corpus/fetch-grib.sh [SECTION...]
#   SECTION is a function name below (e.g. src_gfs). Default: every section.
#   OUT defaults to $SPECULAR_GRIB_CORPUS, else tools/corpus/grib.
#
# About 25 MB in all:
#   ecc_*      ecCodes test data (ECMWF, Apache-2.0): GRIB1 and GRIB2, regular and reduced
#              lat/lon and Gaussian grids, JPEG 2000 and CCSDS packing, complex packing with
#              spatial differencing, bitmaps, constant fields, ensembles, a Mercator grid (not
#              supported: it must fail cleanly)
#   gfs_*      NCEP GFS 1° forecast fields, 2024-01-01 00 UTC +6 h (NOAA, public domain):
#              byte ranges of the pgrb2 file, read from its .idx
#   hrrr_*     NOAA HRRR CONUS (Lambert conformal, 3 km) and Alaska (polar stereographic)
#              analyses, 2024-01-01 00 UTC (public domain), byte ranges
#   ecmwf_*    ECMWF IFS open data 0.25°, 2024-03-01 00 UTC (CC-BY-4.0): CCSDS-packed surface
#              and pressure-level fields and ensemble members, byte ranges from the .index
#   icon_*     DWD ICON-EU regular lat/lon 2 m temperature of today's 00 UTC run
#              (opendata.dwd.de, CC-BY-4.0, kept for 24 hours only, so unpinned)
#
# ERA5 GRIB needs a Copernicus account, so none is fetched; ECMWF's open IFS data stands in
# (same encoder, CCSDS packing).
#
# Every pinned download is checked by size and sha256 (a hash of "-" pins the size only and
# prints the hash). Downloads are untrusted data: this script only copies and decompresses
# files, never runs anything from them. Stops if free space on the target filesystem drops
# below MIN_FREE_GB (default 8).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_GRIB_CORPUS:-$ROOT/tools/corpus/grib}
MIN_FREE_GB=${MIN_FREE_GB:-8}
mkdir -p "$OUT"
WORK=$OUT/.work
mkdir -p "$WORK"

die() { echo "fetch-grib: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

provenance() { printf '%s\t%s\t%s\n' "$1" "$2" "$3" >> "$OUT/.provenance.tsv"; }

# verify FILE SIZE SHA256: whether FILE has that size and hash ("-": size only, hash printed).
verify() {
  local f=$1 size=$2 sha=$3
  [[ -f $f ]] || return 1
  [[ $size == - || $(stat -c %s "$f") == "$size" ]] || return 1
  if [[ $sha == - ]]; then
    echo "fetch-grib: $(basename "$f") $(stat -c %s "$f") $(sha256sum "$f" | cut -d' ' -f1)"
    return 0
  fi
  [[ $(sha256sum "$f" | cut -d' ' -f1) == "$sha" ]]
}

# pinned URL NAME SIZE SHA256 LICENCE: a whole file.
pinned() {
  local url=$1 dest=$OUT/$2 size=$3 sha=$4 lic=$5
  verify "$dest" "$size" "$sha" >/dev/null 2>&1 && return
  check_space
  curl -fsSL --retry 3 -o "$dest.part" "$url" || die "download failed: $url"
  mv "$dest.part" "$dest"
  verify "$dest" "$size" "$sha" || { rm -f "$dest"; die "$2: size or hash mismatch"; }
  provenance "$2" "$url" "$lic"
}

# ranged URL NAME SIZE SHA256 LICENCE OFFSET:LENGTH...: byte ranges of one remote file,
# concatenated (each range is one whole GRIB message).
ranged() {
  local url=$1 name=$2 dest=$OUT/$2 size=$3 sha=$4 lic=$5
  shift 5
  verify "$dest" "$size" "$sha" >/dev/null 2>&1 && return
  check_space
  : > "$dest.part"
  local r off len
  for r in "$@"; do
    off=${r%%:*}
    len=${r##*:}
    curl -fsSL --retry 3 -r "$off-$((off + len - 1))" "$url" >> "$dest.part" \
      || die "range $r of $url failed"
  done
  mv "$dest.part" "$dest"
  verify "$dest" "$size" "$sha" || { rm -f "$dest"; die "$name: size or hash mismatch"; }
  provenance "$name" "$url ($*)" "$lic"
}

# ncep_ranges IDX PATTERN...: OFFSET:LENGTH of the messages whose .idx line matches a pattern
# (fixed strings). A message's length runs to the next line's offset.
ncep_ranges() {
  local idx=$1
  shift
  local p
  for p in "$@"; do
    awk -F: -v pat="$p" '
      { off[NR] = $2; line[NR] = $0 }
      END {
        for (i = 1; i < NR; i++)
          if (index(line[i], pat)) { print off[i] ":" (off[i + 1] - off[i]); exit }
      }' "$idx"
  done
}

# ecmwf_ranges INDEX FILTER...: OFFSET:LENGTH of the messages whose .index line contains every
# FILTER (fixed strings such as '"param": "2t"').
ecmwf_ranges() {
  local idx=$1
  shift
  local line ok f
  while IFS= read -r line; do
    ok=1
    for f in "$@"; do
      [[ $line == *"$f"* ]] || { ok=0; break; }
    done
    if (( ok )); then
      local off=${line##*\"_offset\": }
      off=${off%%,*}
      local len=${line##*\"_length\": }
      len=${len%%\}*}
      echo "$off:$len"
    fi
  done < "$idx"
}

ECC=https://sites.ecmwf.int/repository/eccodes/test-data/data

src_eccodes() {
  local f name size sha
  for f in \
    regular_latlon_surface.grib1:1100:cd5f5f75c46b84a92c32fa7cd671f82375fef020d41f685cf10aabad7fea65cf \
    regular_latlon_surface.grib2:1188:8d586747e47d33a9b727136914965de0faa92d8f95ba50afbb5dcd5bdd3f1e61 \
    reduced_gaussian_model_level.grib2:10996:a68241c5d8be36534ffc0b04b67b89d7ca47fa9133cd31a5f647259e5a162f69 \
    reduced_gaussian_pressure_level.grib1:10172:fdf880a04e02cdcfb7f2ebdea5f4c9cacb0218bf3728f69dc5cdb314ec68c92e \
    regular_gaussian_surface.grib2:14532:033c2067dd8c17daac3ffe630cf407c3c138f162a53e296748662484f9011351 \
    jpeg.grib2:40526:a1cdfa8857c532a92a6343fed5d8b82770d3581ee710cbee65968b9616f00bd3 \
    ccsds.grib2:61806:0d892e90d100d2c6b060f1d674e178423654d7ba034c343fe41f05cfcbc51bd1 \
    constant_field.grib2:199:84d0573e73ab3b79fe17149c6dfe184792d067f787db3dc456c1ffb2d1a85795 \
    tigge_cf_ecmwf.grib2:64272:49495c8d505537f5ff02e74cfee15fdc90ffa92338233f3255820905823b436e \
    mercator.grib2:6339:1f9bd7f8ad81f45e1cbc93c10f105021ec82b606a7194c55fdb0f65d7b9627a6 \
    grid_complex_spatial_differencing.grib2:53684:2ae1fbe0d6518e045d4e009b3923c14ca232ede7c2dfc743ad164518ca5388f6 \
    sample.grib2:1179:35b0994939979667423ec557aa6f7e953a7cfafcdc96555c122dbb0555887b96 \
    gfs.c255.grib2:8306:b56d08e5573f0b4f238a25519f6fc5cd24f19c2bdc860d2bc50bb5f933b16d60 \
    reduced_latlon_surface.grib2:335528:6ab75cd4bfd45bfab266b8b0734b31b02c1f54e122677eac3c105cb87eca817f \
    regular_latlon_surface_constant.grib1:108:2d7f0768cadb26faf4a2fc5cf41ff26d7d93a88c25d5e7aabda6d18d7eeb947c \
    missing.grib2:1176:6455291e7c668db5ab7217dc863477e34fd50f77ac0a7c602fe9a52a7393b3f1 \
    gen_bitmap.grib:840:f31d5ea5be4960a9802c5d12e863a481f7df142ce7270ff7b52eb5df9a34f2d6 \
    lfpw.grib1:2163480:b49a15e5bf96f039e8f08bc7d3f90782f95e3b70122a20a73eda3eb07c847d1c; do
    IFS=: read -r name size sha <<< "$f"
    pinned "$ECC/$name" "ecc_$name" "$size" "$sha" "Apache-2.0 (ECMWF ecCodes test data)"
  done
}

GFS=https://noaa-gfs-bdp-pds.s3.amazonaws.com/gfs.20240101/00/atmos/gfs.t00z.pgrb2.1p00.f006

src_gfs() {
  curl -fsSL --retry 3 -o "$WORK/gfs.idx" "$GFS.idx" || die "GFS index"
  local r
  mapfile -t r < <(ncep_ranges "$WORK/gfs.idx" ":PRMSL:mean sea level:" ":TMP:2 m above ground:" \
    ":UGRD:10 m above ground:" ":TMP:850 mb:" ":TMP:500 mb:" ":HGT:500 mb:" ":APCP:surface:")
  ranged "$GFS" gfs_1p00_f006.grib2 445233 dfd23206ddbed5ff99e7a0101fb24263a9ac14b587778120424c3a7c5631e714 \
    "public domain (NOAA/NCEP)" "${r[@]}"
}

HRRR=https://noaa-hrrr-bdp-pds.s3.amazonaws.com/hrrr.20240101

src_hrrr() {
  local r
  curl -fsSL --retry 3 -o "$WORK/hrrr.idx" "$HRRR/conus/hrrr.t00z.wrfsfcf00.grib2.idx" \
    || die "HRRR index"
  mapfile -t r < <(ncep_ranges "$WORK/hrrr.idx" ":TMP:2 m above ground:" ":REFC:")
  ranged "$HRRR/conus/hrrr.t00z.wrfsfcf00.grib2" hrrr_conus_lambert.grib2 1388733 \
    3ce9234392b2fd4710fd8edce46a16be2fda39d120efc5b933371a28976dcb9e \
    "public domain (NOAA)" "${r[@]}"
  curl -fsSL --retry 3 -o "$WORK/hrrrak.idx" "$HRRR/alaska/hrrr.t00z.wrfsfcf00.ak.grib2.idx" \
    || die "HRRR Alaska index"
  mapfile -t r < <(ncep_ranges "$WORK/hrrrak.idx" ":TMP:2 m above ground:")
  ranged "$HRRR/alaska/hrrr.t00z.wrfsfcf00.ak.grib2" hrrr_alaska_polar.grib2 684260 \
    5e476242ec2e304df57e11bf2a286f8ee7aaa09eb50d50daec3ca6706381ff52 \
    "public domain (NOAA)" "${r[@]}"
}

IFS_OD=https://storage.googleapis.com/ecmwf-open-data/20240301/00z/ifs/0p25

src_ecmwf() {
  local r
  curl -fsSL --retry 3 -o "$WORK/oper.index" "$IFS_OD/oper/20240301000000-0h-oper-fc.index" \
    || die "IFS index"
  mapfile -t r < <(
    ecmwf_ranges "$WORK/oper.index" '"levtype": "sfc"' '"param": "2t"'
    ecmwf_ranges "$WORK/oper.index" '"levtype": "sfc"' '"param": "msl"'
    ecmwf_ranges "$WORK/oper.index" '"levelist": "850"' '"param": "t"'
    ecmwf_ranges "$WORK/oper.index" '"levelist": "500"' '"param": "gh"'
  )
  ranged "$IFS_OD/oper/20240301000000-0h-oper-fc.grib2" ecmwf_ifs_oper.grib2 2183242 \
    8dd0a9cf6c7ce1e4135faa942adacbc28db1f49b66f85fecb77dc7ff4c90439d \
    "CC-BY-4.0 (ECMWF open data)" "${r[@]}"
  curl -fsSL --retry 3 -o "$WORK/enfo.index" "$IFS_OD/enfo/20240301000000-24h-enfo-ef.index" \
    || die "IFS ensemble index"
  mapfile -t r < <(
    for m in 1 2 3 4; do
      ecmwf_ranges "$WORK/enfo.index" '"type": "pf"' "\"number\": \"$m\"" '"param": "2t"'
    done
    ecmwf_ranges "$WORK/enfo.index" '"type": "cf"' '"param": "2t"'
  )
  ranged "$IFS_OD/enfo/20240301000000-24h-enfo-ef.grib2" ecmwf_ifs_enfo.grib2 3320230 \
    e6977f3a3384be2f31af00513c3a2dbd3fbdd3b5084e425aaf6f4cdb2862fcd8 \
    "CC-BY-4.0 (ECMWF open data)" "${r[@]}"
}

src_icon() {
  local day name
  day=$(date -u +%Y%m%d)
  name=icon-eu_europe_regular-lat-lon_single-level_${day}00_000_T_2M.grib2
  [[ -f $OUT/icon_eu_t2m.grib2 ]] && return
  check_space
  curl -fsSL --retry 3 -o "$WORK/icon.bz2" \
    "https://opendata.dwd.de/weather/nwp/icon-eu/grib/00/t_2m/$name.bz2" \
    || { echo "fetch-grib: ICON-EU $day 00 UTC not available; skipped" >&2; return; }
  bunzip2 -c "$WORK/icon.bz2" > "$OUT/icon_eu_t2m.grib2"
  rm -f "$WORK/icon.bz2"
  provenance icon_eu_t2m.grib2 "opendata.dwd.de icon-eu $name.bz2" "CC-BY-4.0 (DWD)"
}

SECTIONS=(src_eccodes src_gfs src_hrrr src_ecmwf src_icon)
(( $# )) && SECTIONS=("$@")
for s in "${SECTIONS[@]}"; do
  declare -F "$s" >/dev/null || die "no section $s"
  echo "fetch-grib: $s"
  "$s"
done
rm -rf "$WORK"
du -sh "$OUT"
