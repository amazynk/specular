#!/usr/bin/env bash
# Fetch the WordPerfect Graphics (.wpg) dev corpus into tools/corpus/design2/wpg/ (gitignored:
# nothing downloaded here is committed). The specular-wpg corpus test reads it from
# SPECULAR_WPG_CORPUS, else tools/corpus/design2/wpg, and skips when it is absent.
#
# Usage: tools/corpus/fetch-design2-wpg.sh   (PIN=1 prints the SHA-256 lines to paste below)
#
# Sources (well under 1 MB): WPG 1 and 2 clip art from the telparia.com file-format sample
# collection (old shareware and clip-art discs, redistributed as format samples) and
# LibreOffice's libwpg import test file (MPL-2.0). Every file is pinned by SHA-256. BADGE.wpg
# is an OLE file misnamed .wpg (kept as a refusal case). Synthetic WPG 1/2 files are added with
# SPECULAR_WPG_WRITE_FIXTURES=tools/corpus/design2/wpg cargo test -p specular-wpg write_fixtures.
# Downloads are untrusted data: this script only copies them, never runs anything from them.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_WPG_CORPUS:-$ROOT/tools/corpus/design2/wpg}
mkdir -p "$OUT"

T=https://telparia.com/fileFormatSamples/image/wpg
LO=https://raw.githubusercontent.com/LibreOffice/core/master/writerperfect/qa/unit/data/draw/libwpg/pass

fetch() {
  local name=$1 url=$2
  if [[ ! -s "$OUT/$name" ]]; then
    echo "fetch-design2-wpg: $name <- $url"
    curl -fsSL --retry 3 -m 120 -o "$OUT/$name.part" "$url"
    mv -- "$OUT/$name.part" "$OUT/$name"
  fi
}

for n in BADGE.wpg BEETHOV.WPG DARVADER.WPG HAPPY.WPG HOLMES.WPG adze_5tb.wpg basebal4.wpg \
  chest.wpg cup.wpg diamond.wpg eagle1.wpg flag_b24.wpg holly.wpg input.wpg july4d.wpg \
  lion.wpg snowman.wpg starsbur.wpg test.wpg; do
  fetch "$n" "$T/$n"
done
fetch lo-WPG1.wpg "$LO/WPG1.wpg"

if [[ "${PIN:-}" == 1 ]]; then
  (cd "$OUT" && sha256sum -- *.wpg *.WPG | grep -v ' gen-' | sort -u)
  exit 0
fi

(cd "$OUT" && sha256sum --quiet -c -) <<'EOF'
035c594498e66c5fa6f149aee1427ef14697eaa3124db8f6876b78d1fa55d910  starsbur.wpg
066b660d07be4d735a5d981c7bd55644200f518447929207d166827afd34ee9b  basebal4.wpg
0a71b70d68d1ba40ed935d6b2636ca14346e548db8b7f4fc07d321c1dca47882  diamond.wpg
10dbb1c7b6c81e87c69a971bc39ec5a7773e128812d2fc71d7b620d51d885dc3  lo-WPG1.wpg
16fba34673f8f6f8669a34974b9a54864a34c1e05f10bb023deb87c053f51bb8  DARVADER.WPG
22395f1107fd7f6d5dc478755d3c986a8042729a7ee474c745d55a387e49876c  HOLMES.WPG
2fd3f994d073395d3d749f745234dde5d7151290451ebb745835c20da3685661  flag_b24.wpg
3114a382e8c345ffe072b773cfa5cc9ed626ced69e6fe12076d3f2066cbdcac8  july4d.wpg
3879c5ad86cb8605afc8f0f2476998ce95322a11b3111a24dc7b75c8bbdfa858  eagle1.wpg
448056cbb49e9710a79f095958e1a1f791d7d5f24fe53f3234c44276cdcc4f86  BEETHOV.WPG
4f4de826bad760d98802ef44dc3b5252dd9230f212924011f9f1ef07a56bcc6e  test.wpg
5ff1d2500308fe9f04a748d54a25575283f505aa0ee8470f6d34670793f5366e  snowman.wpg
8ce2f79b05e7ce6778f17cb6b25cd497cddcf87403bbe511bc3c1f2ebc939c28  adze_5tb.wpg
91148ce63257421e6f1e6f183d787b8f2031a14036dc6b2933263d2c358984c6  input.wpg
9e312542e76d2c3b3ec3fd13129814fe6de6fb3e120f2d3a8aff9f0bcb574067  lion.wpg
9ed4524c56e7f175c424cbe33bdad27a183d2cbb71af0fcba251145b68035b07  holly.wpg
a0fb89b4e08deef6692df07c256332ae6ea556887a525a354fd0920cff5ebc81  HAPPY.WPG
a42df99188a802d0d74334d1367a296bdf5807709b2a03f22bca5fd5602b1bb6  cup.wpg
d05b9031df56f598e52818c8656f25a4c4af51c1bed541f12d224c27a857069f  BADGE.wpg
d27076e980babcd125ff0416e1caed44feae5fefb2ba6b64cb871c4a3db132f1  chest.wpg
EOF
echo "fetch-design2-wpg: $(find "$OUT" -iname '*.wpg' | wc -l) files, $(du -sh "$OUT" | cut -f1)"
