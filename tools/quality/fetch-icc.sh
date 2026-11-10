#!/usr/bin/env bash
# Fetch the ICC conformance material for the colour quality runner into
# tools/corpus/quality/icc (gitignored; override with SPECULAR_QUALITY_CORPUS).
# Sources:
#   - the ICC profile registry (registry.color.org): sRGB v2/v4, the probe
#     profiles (v2/v4: each intent maps to a different colour), and every
#     profile the RGB-registry, exchange-space, Rec.709, scene-referred and
#     XYZ pages link (ICC v5 / iccMAX profiles are not fetched);
#   - the Little CMS testbed profiles (MIT): v2/v4 matrix-shaper, LUT-based,
#     Lab and gray test profiles.
# The ICC distributes its profiles for free use; the Little CMS testbed is MIT.
# Nothing fetched here is committed.
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
out=${SPECULAR_QUALITY_CORPUS:-$root/tools/corpus/quality}/icc
mkdir -p "$out/dl"
cd "$out/dl"

reg=https://registry.color.org
pages=(
  /rgb-registry/srgbprofiles /rgb-registry/srgb_appearance /rgb-registry/black_scaled_2009_srgb
  /rgb-registry/displayp3 /rgb-registry/rommrgb /rgb-registry/adobergb /rgb-registry/bt2020
  /rgb-registry/bt2100 /rgb-registry/bt601 /rgb-registry/bt709 /rgb-registry/dcip3
  /rgb-registry/ecirgb /rgb-registry/erimmrgb /rgb-registry/oprgb /rgb-registry/rimmrgb
  /rgb-registry/scrgb /rgb-registry/scrgb-nl /rgb-registry/srgb /rgb-registry/sycc
  /profile-library/probe-profile /profile-library/exchange-space-profile
  /profile-library/rec709 /profile-library/scene-referred /profile-library/xyz-profiles
  /profile-library/prmg-rgb-gamut-warning
)
fetch() { # url file
  [ -s "$2" ] && return 0
  curl -sfL -o "$2" "$1" || { echo "failed: $1"; rm -f "$2"; }
}
for p in "${pages[@]}"; do
  base=$reg${p%/*}/
  links=$(curl -sfL "$reg$p" 2>/dev/null | grep -o -i -E 'href="[^"]*\.(icc|icm|zip)"' | sed -E 's/^href="|"$//g' || true)
  for href in $links; do
    case $href in
      http*) url=$href ;;
      /*) url=$reg$href ;;
      *) url=$base$href ;;
    esac
    fetch "$url" "$(basename "$href")"
  done
done

lcms=https://raw.githubusercontent.com/mm2/Little-CMS/lcms2.16/testbed
for f in test1.icc test2.icc test3.icc test4.icc test5.icc ibm-t61.icc new.icc crayons.icc; do
  fetch "$lcms/$f" "lcms-$f"
done

# Unpack: every profile ends up flat in $out.
shopt -s nullglob nocaseglob
for z in *.zip; do
  unzip -o -j -q "$z" -d ../unz.tmp || true
done
for f in *.icc *.icm ../unz.tmp/*.icc ../unz.tmp/*.icm; do
  cp -f -- "$f" "../$(basename "$f" | tr ' ' '_')"
done
rm -rf ../unz.tmp
n=$(find "$out" -maxdepth 1 -type f \( -iname '*.icc' -o -iname '*.icm' \) | wc -l)
echo "profiles in $out: $n"
