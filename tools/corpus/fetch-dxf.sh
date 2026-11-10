#!/usr/bin/env bash
# Fetch the public DXF test corpus into tools/corpus/dxf/ (gitignored: nothing downloaded here
# is committed). The specular-cad ignored tests (corpus_oracle, corpus_timing) read it in place
# from SPECULAR_DXF_CORPUS, else from tools/corpus/dxf, and skip when it is absent.
#
# Usage: tools/corpus/fetch-dxf.sh [SECTION...]
#   SECTION is a function name below (e.g. src_ezdxf). Default: all sections.
#   OUT defaults to $SPECULAR_DXF_CORPUS, else tools/corpus/dxf.
#
# What it covers (about 55 MB): R12 to 2018 ASCII files, binary DXF (R12, R13, R14, 2000),
# layouts and viewports (frozen layers, clipped), hatches (solid, ANSI/ISO patterns, gradients,
# islands), dimensions (also nested in blocks), MTEXT inline codes and columns, TEXT
# alignments/mirroring/oblique, nested and arrayed INSERTs with attributes, splines, ellipses,
# polyface meshes, WIPEOUT, LEADER/MLEADER, ACAD_TABLE, non-ASCII code pages, real floor plans,
# and one large drawing (no public architectural DXF of 5 MB or more was found, so
# large/floorplan-tiled.dxf is made here from the real floorplan.dxf: its ENTITIES copied 12x12
# with offsets, about 30 MB). Sources and licences are logged in .provenance.tsv; every file is
# checked against the SHA-256 list at the end of this script.
#
# Downloads are untrusted data: this script only copies files, never runs anything from them.
# Scratch space is target/agent-corpus/dxf-fetch (deleted afterwards). Stops if free space on
# the target filesystem drops below MIN_FREE_GB (default 6).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_DXF_CORPUS:-$ROOT/tools/corpus/dxf}
SCRATCH="$ROOT/target/agent-corpus/dxf-fetch"
MIN_FREE_GB=${MIN_FREE_GB:-6}
export GIT_LFS_SKIP_SMUDGE=1 GIT_TERMINAL_PROMPT=0
mkdir -p "$OUT" "$SCRATCH"

die() { echo "fetch-dxf: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

# provenance RELPATH URL LICENCE
provenance() { printf '%s\t%s\t%s\n' "$1" "$2" "$3" >> "$OUT/.provenance.tsv"; }

# git_files OWNER/REPO COMMIT DEST LICENCE PATH...
# Sparse, blobless, depth-1 fetch of PATH... at COMMIT; copies every .dxf file to
# $OUT/DEST/<repo path>.
git_files() {
  local repo=$1 commit=$2 dest=$3 lic=$4; shift 4
  local work="$SCRATCH/${repo//\//_}"
  check_space
  rm -rf "$work"
  git init -q "$work"
  git -C "$work" remote add origin "https://github.com/$repo"
  git -C "$work" config core.sparseCheckout true
  git -C "$work" sparse-checkout set --no-cone "$@"
  git -C "$work" fetch -q --depth 1 --filter=blob:none origin "$commit"
  git -C "$work" -c advice.detachedHead=false checkout -q FETCH_HEAD
  local n=0
  while IFS= read -r -d '' f; do
    local rel=${f#"$work"/}
    mkdir -p "$OUT/$dest/$(dirname "$rel")"
    cp "$f" "$OUT/$dest/$rel"
    provenance "$dest/$rel" "https://github.com/$repo/blob/$commit/$rel" "$lic"
    n=$((n + 1))
  done < <(find "$work" -path "$work/.git" -prune -o -type f -iname '*.dxf' -print0)
  rm -rf "$work"
  echo "  $repo: $n files -> $dest"
}

# ------------------------------------------------------------------------------- sections

src_jscad() {        # jscad sample-files: ezdxf, dxf-parser, AutoCAD 2017 2D/3D samples
  git_files jscad/sample-files f6f6485e7286a50c4c304d697be9cc9f7e64beb6 jscad \
    'MIT (jscad/sample-files); files from ezdxf, dxf-parser and others' '/dxf/'
}
src_ezdxf() {        # ezdxf examples and integration data: binary DXF, layouts, MTEXT, hatches
  git_files mozman/ezdxf 7c64bce52c9d60b9db2dad4d2793c1168c8d3f64 ezdxf 'MIT (ezdxf)' \
    '/examples_dxf/*.dxf' '/integration_tests/data/*.dxf' \
    '/examples/edgeminer/3_us_main.dxf' '/docs/source/tutorials/src/geo/Graz_10km_3m.dxf'
}
src_gdal() {         # GDAL OGR autotest DXF: OCS, binary wipeout, leaders, code pages
  git_files OSGeo/gdal 0d57017e566dfa576f46bfb3e4cac3dabd91d3a8 gdal 'MIT (GDAL autotest)' \
    '/autotest/ogr/data/dxf/'
}
src_bjnortier() {    # bjnortier/dxf test resources: floor plans, texts in blocks, polyface
  git_files bjnortier/dxf cb571d1d7631dd0daaf923a256d75ef0917ff806 bjnortier 'MIT (bjnortier/dxf)' \
    '/test/resources/'
}
src_fess() {         # codelibs fess-testdata: one drawing saved by AutoCAD R12 LT2 to 2024
  git_files codelibs/fess-testdata 1ef08d09b169722ed55855e9aaafa3fbad6c38e0 fess \
    'Apache-2.0 (codelibs/fess-testdata)' '/files/cad/*.dxf'
}

src_large() {        # large/floorplan-tiled.dxf: bjnortier floorplan.dxf ENTITIES tiled 12x12
  local src="$OUT/bjnortier/test/resources/floorplan.dxf"
  [[ -f $src ]] || src_bjnortier
  check_space
  mkdir -p "$OUT/large"
  python3 -I - "$src" "$OUT/large/floorplan-tiled.dxf" 12 <<'PY'
import sys

# Copies the ENTITIES section N x N times, shifting every WCS x (10-18) and y (20-28) by the
# drawing's width/height (from $EXTMIN/$EXTMAX) and dropping handles (5) and the 102 groups
# (extension dictionaries, reactors) so no handle is defined or owned twice.
# The source floor plan has no OCS entities with a non-default extrusion.
src, dst, n = sys.argv[1], sys.argv[2], int(sys.argv[3])
lines = open(src, "rb").read().replace(b"\r\n", b"\n").split(b"\n")
pairs = [(lines[i].strip(), lines[i + 1]) for i in range(0, len(lines) - 1, 2)]
ext = {}
for i, (c, v) in enumerate(pairs):
    if c == b"9" and v.strip() in (b"$EXTMIN", b"$EXTMAX"):
        ext[v.strip()] = (float(pairs[i + 1][1]), float(pairs[i + 2][1]))
w = ext[b"$EXTMAX"][0] - ext[b"$EXTMIN"][0]
h = ext[b"$EXTMAX"][1] - ext[b"$EXTMIN"][1]
start = end = None
for i, (c, v) in enumerate(pairs):
    if c == b"2" and v.strip() == b"ENTITIES" and pairs[i - 1][1].strip() == b"SECTION":
        start = i + 1
    elif start is not None and end is None and c == b"0" and v.strip() == b"ENDSEC":
        end = i
body = pairs[start:end]
out = []
for i, (c, v) in enumerate(pairs[:start]):
    out.append((c, v))
for ty in range(n):
    for tx in range(n):
        dx, dy = tx * w * 1.1, ty * h * 1.1
        group = False
        for c, v in body:
            code = int(c)
            if code == 102:
                group = v.strip().startswith(b"{")
                continue
            if code == 5 or group:
                continue
            if 10 <= code <= 18 and dx:
                v = repr(float(v) + dx).encode()
            elif 20 <= code <= 28 and dy:
                v = repr(float(v) + dy).encode()
            out.append((c, v))
out.extend(pairs[end:])
with open(dst, "wb") as f:
    for c, v in out:
        f.write(c + b"\n" + v.rstrip(b"\r") + b"\n")
PY
  provenance large/floorplan-tiled.dxf \
    'generated by tools/corpus/fetch-dxf.sh from bjnortier/test/resources/floorplan.dxf' \
    'MIT (bjnortier/dxf), derived'
  echo "  large: $(du -h "$OUT/large/floorplan-tiled.dxf" | cut -f1) -> large"
}

verify() {
  local bad=0 sum path
  while read -r sum path; do
    [[ -n $sum ]] || continue
    if [[ ! -f $OUT/$path ]]; then continue; fi
    if [[ $(sha256sum "$OUT/$path" | cut -d' ' -f1) != "$sum" ]]; then
      echo "fetch-dxf: sha256 mismatch: $path" >&2
      bad=1
    fi
  done < <(sed -n '/^# SHA256 BEGIN$/,/^# SHA256 END$/p' "$0" | sed -e '1d' -e '$d' -e 's/^# //')
  (( bad == 0 )) || die "sha256 mismatches (see above)"
  echo "  sha256: ok"
}

main() {
  local sections=() s
  for a in "$@"; do
    case $a in
      -h|--help) sed -n '2,22p' "$0"; exit 0 ;;
      *) sections+=("$a") ;;
    esac
  done
  if (( ${#sections[@]} == 0 )); then
    sections+=($(declare -F | awk '{print $3}' | grep '^src_'))
  fi
  for s in "${sections[@]}"; do
    declare -F "$s" >/dev/null || die "unknown section $s"
    echo "== $s"
    "$s"
  done
  # Re-runs can repeat entries; keep the last per path.
  if [[ -f $OUT/.provenance.tsv ]]; then
    tac "$OUT/.provenance.tsv" | awk -F'\t' '!seen[$1]++' | tac > "$OUT/.provenance.tmp"
    mv "$OUT/.provenance.tmp" "$OUT/.provenance.tsv"
  fi
  verify
  rm -rf "$SCRATCH"
  du -sh "$OUT"
  df -h "$OUT" | tail -1
}
main "$@"
exit 0

# SHA256 BEGIN
# 05b79acc4b1c619813d15e8cb0a041a04d716e8f42ede42373ae6216067e1ae9 bjnortier/test/resources/1x1rectangle.dxf
# db95542b09afd1fd635be5b9b869d2ca7a82869d2e9b84d0d2e4c07991de6e6d bjnortier/test/resources/Ceco.NET-Architecture-Tm-53.dxf
# 36f300dfc3823a6529db30f8ad9e0e815de813f3cba248dbe1d863f42f6f8632 bjnortier/test/resources/accumulatortest.dxf
# b73d4a5671d7d2bdad16f89fa21413f4f89cbd42673f54af97332041fd8e7e8c bjnortier/test/resources/alu-profile.dxf
# 2e46b1fc5925d916ef25eb0df8301d2f75aa0f720c2204336c27ebd1fbe2d891 bjnortier/test/resources/arc15.dxf
# 381208b5f34b9a507d8d53a2ebc5e92edfa9687e5ee916a7a051cfc67b7236ef bjnortier/test/resources/arc16.dxf
# f2796e62aa81956d054fd27f3157a2b35d9291b43d3f054b622ed394aca89b71 bjnortier/test/resources/arc17.dxf
# e46a455a1c7638f083f23d6ef743eb376849534ebc31b461643d3b65baed3a1a bjnortier/test/resources/array-rotated.dxf
# 29856a0eb2b8412527bef8be612200662ea5bb80fadc8decec1eccf79b8ec058 bjnortier/test/resources/arrayed-holes.dxf
# 0d22b756453482340f0e1ab78203eb46de57f9e7c87e0331c3a49f25908c4b04 bjnortier/test/resources/attribute.dxf
# ba33677808f6a534d64a5898803649caca30eccb543092e3e5a102043b9d1c2e bjnortier/test/resources/blocks1.dxf
# 5d847d833dd444fbe8a85def5881a4f9055e47d06c41ca009c9e40784e92f91f bjnortier/test/resources/blocks2.dxf
# 0580d6ae3df2afe8f35e57970fa0d4556450cf4b17d519b6b7380814ae49ce78 bjnortier/test/resources/circlesellipsesarcs.dxf
# 8e24d7f34d6532b5b3ab3576e9e44069433400d6f803fdfe828c22ccc11467b2 bjnortier/test/resources/closedlwpolylinebug.dxf
# dd4dae9712b5e4e60f7e0a13a79ccdfd7c6fee8bf31ef31c8bfcb7ee85d8c44c bjnortier/test/resources/dimension-vertical.dxf
# 513d44f4b2febdf2d2e27016c400cc774f0a364620b34730eaaece5cd6f15d77 bjnortier/test/resources/dimensions.dxf
# 5478e9de0332e5b7120e173dfa4b62f305456ee922dfb9403e1e82364c207a5b bjnortier/test/resources/elliptical-arc1.dxf
# ddbf0b074ec3979f5d57a4c108553f94df6c3b0e35bd7822610ac66e1e6e76e3 bjnortier/test/resources/elliptical-arc10.dxf
# f36768b851f3c01430f0489e9f05cbb553a6442ea397269c7cd70833a71a9806 bjnortier/test/resources/elliptical-arc11.dxf
# 94fddc8094f9b91d2bdc7d485c0fce8c85d9500d0bd4d4ea158283fd6fcce9f9 bjnortier/test/resources/elliptical-arc12.dxf
# b734c07d406328ae8f6c5d16fb91db85c1d35dfa1d5e8e6596e527f0d3909fb9 bjnortier/test/resources/elliptical-arc13.dxf
# ca90cbf400cb9bf32379e482d286ad9a1324671b3a86a46dd3ec945941c07cbf bjnortier/test/resources/elliptical-arc14.dxf
# 6c2c61be3d526e25ed952c5d48b6a2d53d9447a5617ea4147c9705d89abc91e4 bjnortier/test/resources/elliptical-arc2.dxf
# d899d938a2df9addb940b8ae5a3f37b1e3b937c31fbf164260573060fdb97611 bjnortier/test/resources/elliptical-arc3.dxf
# f6d8716b7f58795fc4604f9f894ae7b4ceecf5aa813a974cf0db1b93905f0152 bjnortier/test/resources/elliptical-arc4.dxf
# add51eb72605b5821ae25e6a424e4a6204473c3db2b65ef34f6958a23988cc00 bjnortier/test/resources/elliptical-arc5.dxf
# bf802cfbdb063f4c93c4fc753bfbc9625243dff18a2d2563590bcf452dcdcd3d bjnortier/test/resources/elliptical-arc6.dxf
# 9f40190864797ed6e9151fb7b7dfff777af0082536e0bf9d93639a19182953fa bjnortier/test/resources/elliptical-arc7.dxf
# 058e18751829304dab2347879ce1a6fc6bd76377ae636cd6dddb480309912ac1 bjnortier/test/resources/elliptical-arc8.dxf
# df4e4d0edc060bfb1f6b9be5207886487f5e29cdbfb23c196dfcf000dc5967cf bjnortier/test/resources/elliptical-arc9.dxf
# eff79f24ba4c8ac1ccfa1d82d0cdd42c5e9b2b027d275f1704248accf422acbf bjnortier/test/resources/ellipticalarcs.dxf
# 378f5e9c08d74888971b0a427c7b6a64ac6ebfdf3c163714a6acdb942a76d2d2 bjnortier/test/resources/ellipticalarcs2.dxf
# 56b9bc2de32b24c7ca3e29071d8cf6c7f9c27df1c936948d48c891ac8fd92c58 bjnortier/test/resources/empty.dxf
# fa0b296dc6c060ce8d79c61dd6461c9e1523a15ae33cdc650175ee8b9270d1d8 bjnortier/test/resources/entities.dxf
# 6d761ecb1ab59664425eb1dcc6837a12611536c8e03e3f7855951a76786d159c bjnortier/test/resources/floorplan.dxf
# e67becc4b026231d656a4b506b90d533a3fcdfb14e3003762edd4a9e40b7e6d3 bjnortier/test/resources/hatches.dxf
# 756b5a3901e6a77d74793d7ef0eb4d609d299a1beedf60c1658b4375fea8e92a bjnortier/test/resources/issue21.dxf
# f59eb881b89004b35dc30618ea07fd06fb77f1a7665c956d6c70933c77a91a59 bjnortier/test/resources/issue27a.dxf
# d6c18a060529926a877b06d4a2feb82a55fb5b2f26304f3ee368746b6b0695ee bjnortier/test/resources/issue27b.dxf
# 4c575e968d6d1190cbf36fb531908f38d69ac3fb95c17e7722c209d33c1d8575 bjnortier/test/resources/issue27c.dxf
# fdc0f73c40cf51f1fcf30e161050bc3b24d8c09de17e8579d56f0b7cd459871d bjnortier/test/resources/issue28.dxf
# d42c9d2486fdaaef326f81a084e011226527a9755ebaeefa63176b51dca59ccc bjnortier/test/resources/issue29.dxf
# 34836a7e015688ec3a4b3d481c6a771ba2088502ea27f0cd966056555a382468 bjnortier/test/resources/issue39.dxf
# 24ca55104af8e7d42cd236643bf79851636756faf1e8c7f83d216eeaeca20964 bjnortier/test/resources/issue42.dxf
# 2c88e317775fc866b0e51760f921eb30aaaefa6723e196b2decf2973d0bfe8e6 bjnortier/test/resources/issue50.dxf
# 2ff5c6056297ad680ab888fb045670fcfbd6ec1a64b37f94a44dc84b311bb5f6 bjnortier/test/resources/issue52.dxf
# 1eb628bf736a37a48146ba3e81950285768ccb73d441e6161ab1c878bfc19427 bjnortier/test/resources/issue53.dxf
# 860c3085c10b754d75fd903747f78a0e52c69b94ac77f23bdb018bec41b860ff bjnortier/test/resources/layers.dxf
# 052d8246dcf762c4100a4df8a68bed1964da5d1ee087a86af9996c7b6c6e6a30 bjnortier/test/resources/lines.dxf
# 34d41a0bab6a9c210c6878ce50e33f243a428115ed07e47bf2f64990bec8bc53 bjnortier/test/resources/lwpolyline_flip_test.dxf
# cbc8fbf3016fd7b580f3a077092c63d1903728111f283e9907c774ce94e7a3e4 bjnortier/test/resources/lwpolylines.dxf
# dfe3b732026b655dc89ad2b985944ccd96d4ab7429f2adb1ccc7df36c8686364 bjnortier/test/resources/openscad_export.dxf
# 5f0397697996b75078994e0fe95bb12a87864f68f49c14018194611edb30944f bjnortier/test/resources/points.dxf
# 1a0ea5f5d50a2d485768d76b17c7e51f9480236e03a7dc41db7176109157bbe6 bjnortier/test/resources/polyfacemesh.dxf
# af6d2781d7961e531c34e27c7d0f8a4c4b885e579cdcbcd5e5a41766074851a0 bjnortier/test/resources/polylines.dxf
# 4f6456eba44b012c250311a0f0205a7bf8f3f844a443820cf7f4e45031b7ef92 bjnortier/test/resources/splineA.dxf
# 9f672526c3e6b35c60ed958650a5c35fe3ee4f8df76d1f8b25b31f2f6d293406 bjnortier/test/resources/splines.dxf
# cdae8ce1750984586723c7d0c784ad8c01d671b6c70c7802f35f0ea62f21938c bjnortier/test/resources/squareandcircle.dxf
# ecdcd72eb0274db6fb797fcf79cda0ffc6a2015fc14d160e9109c44ba927f306 bjnortier/test/resources/squircle2.dxf
# 8601dfdfa258445a9347b873403d4bda489f26340899baad5ba1caaad1d860d9 bjnortier/test/resources/supported_entities.dxf
# 43531f64f8990fb397ada188c45a5ab7b5db26b7efec19ac4e654c7a7be8eb63 bjnortier/test/resources/testHatch.dxf
# c150ba46518b553d2fec08d1680ff6a66862f6708753946b8454c82bcadf417c bjnortier/test/resources/testModelPaperSpace.dxf
# 703cc81f289a42dc5d0686543e7d022517246d08e578cc72f5a29ca53158327f bjnortier/test/resources/testOle2Frame.dxf
# 6ee5a4f44f3f5e9e2550b8ae72960efac22dd4c12decc73ebd484a8a33d283a2 bjnortier/test/resources/texts-in-block.dxf
# 6c585c0093c1a19180faa58b6977bc48ddb47899d2800bf38807cbca3018416d bjnortier/test/resources/texts.dxf
# 184d31bcd5100148c22dda74cc7cbb47c605083f75597248677b864b82e95c23 bjnortier/test/resources/threeDFaces.dxf
# 411eb5783697bd821b2b55f4dea13bc8e7f315922b9891c69fe320e29f9cdd9b ezdxf/docs/source/tutorials/src/geo/Graz_10km_3m.dxf
# ff424f11aca74246854b64e231d735b4ee4a1914e8fc81a9b59063fdcf7daf76 ezdxf/examples/edgeminer/3_us_main.dxf
# c9447d342e267052354bede75028a934da35bd2bb1a96c1c084bdf7e5f0abc2a ezdxf/examples_dxf/3dface.dxf
# 93c8197ca313b3224647c4b7c88ce857ab55e2e8c0258cc0298c40c847882ebc ezdxf/examples_dxf/Minimal_DXF_AC1006.dxf
# 7286a35c85164d0165b6e0e2bd8093c25f818cdc31b9abd34a46646924fcccc3 ezdxf/examples_dxf/Minimal_DXF_AC1009.dxf
# 6866a50766e178f19356607ee3d3fe619f9f46cfcd2d7d5969f1077eb675464d ezdxf/examples_dxf/Minimal_DXF_AC1021.dxf
# aa535057afadab4ed0fc8cadb4b624d272c90603c7e03f44df1e6a6d1a15acc0 ezdxf/examples_dxf/VP4.dxf
# 2fc8d0e9517b36dd13db95740b37bc2f534005f16abc222faab036c3407d9e8d ezdxf/examples_dxf/acad_table_simple.dxf
# 7c42eac2e0c21b386273294b7937845ddda3da58f85c30fe130db229570f8015 ezdxf/examples_dxf/acad_table_with_blk_ref.dxf
# c1e3ec3d1a46d40aebb325d23a0d1ffefe5df64ed1336609e883e0517391faf0 ezdxf/examples_dxf/caret_encoding.dxf
# e15de0694bd251da052a1ee38eda8d096deab37eed921a44542da8dbe0bff8ac ezdxf/examples_dxf/circle_radius_le_0.dxf
# 70425c817f79364d6084ea390ecff4652d6e17d2e80d7c6eadbdc64268676672 ezdxf/examples_dxf/clockwise_arcs_hatch.dxf
# 70c12f1e3fd291c4ab02df7628624956fce315263c0ebcb18734d0b6e5b9974a ezdxf/examples_dxf/colors.dxf
# 31d4d411982821a3db75854f90108e15f867ecdd619d526fb3c9bd761b92095c ezdxf/examples_dxf/columns_R2007.dxf
# 0110c42731224a851baca4fd93209a40ebcd19b18d98e75201715d3cd3baa649 ezdxf/examples_dxf/columns_R2018.dxf
# 923cc1d0d86bf6738139dbec26b66e299fbfa2c16a49ed206ad06cf719c6e6f2 ezdxf/examples_dxf/dimension_in_block.dxf
# 03b50bf9a29513147d2b66f6ec1a4377494ced38437082c979639dfa42d613ea ezdxf/examples_dxf/dimension_in_nested_blocks.dxf
# 16ff6d4b687124d6ea681529260a4273858b87d9d5dd02eaec5d55d36dff30fb ezdxf/examples_dxf/hatch_issues.dxf
# aa10576b3ae48f574c728b862575dae13bdd5acd7da0fdcff813a8d2d59b1f03 ezdxf/examples_dxf/hatch_text_box_in_fill.dxf
# aff29a10fa952a198572ed524b1c67528d5279d04f7fd34e58d778d2fd393eb3 ezdxf/examples_dxf/hatch_text_box_in_hole.dxf
# 387c0cd9a2a1f7259706b3d28f91e8c91e39f8a5be7284b0fb4472f5aaa484a9 ezdxf/examples_dxf/hatch_text_box_overlapping_fill_and_hole.dxf
# 2dc2930b4f7d5fb16698437e4f6972986e580a81b2b5d36a780b4311f85fb326 ezdxf/examples_dxf/hatches_1.dxf
# a50f02abbbafb0fedef5140e3f99691012bcffa192ecb298470fe8c5b2b9aad6 ezdxf/examples_dxf/hatches_2.dxf
# c2597f95c007852db2ba82dedd9558f9318981115dda82fccfc0267ca60e5a56 ezdxf/examples_dxf/insert_bricscad_level_1.dxf
# 6981bc669929bd1e626ffb4846f05dc37b7444160abda796d753496630adee5b ezdxf/examples_dxf/mtext_columns_R2000.dxf
# f0a04b0072986e6623eb9caf2562ef3064ef88ba1f9015724f856fd6937ab2e9 ezdxf/examples_dxf/mtext_columns_R2007.dxf
# 51ab94490167ff11a2c154d94e626ac309b36d64ee994915aac53fd89efde60f ezdxf/examples_dxf/mtext_columns_R2018.dxf
# fb09a7af8882787553149a675c6d15b45bab8ff4a49f58200460832eab3de401 ezdxf/examples_dxf/mtext_complex_inline_codes.dxf
# 14951dbef1c750fefc0a452e0bd4fb5b281dc092820d5b6da31b3855dddb4411 ezdxf/examples_dxf/multi_insert_with_attribs.dxf
# 303f8a23596ad94e084f6bdf2b60350a7ae32795c77c63f67945c83ae6f5d64c ezdxf/examples_dxf/text.dxf
# cacf3083c17c8d3836c23c6356edcb6f70db2949ed61875c2572911719a61353 ezdxf/examples_dxf/text_alignments.dxf
# c69dbb759042ebdb3e23047e61f4053cfcd48d210ca846490d3c3ffc64d56ea1 ezdxf/examples_dxf/text_fonts.dxf
# 51dc85fbbcc04c4c3c103d92b99524bde9ef7272a9f818ad321a683cd6877d10 ezdxf/examples_dxf/text_in_viewports.dxf
# 5bed117ca6271aefb54e23203cee730c8205064ae2e8fae7afe2aed0d0946602 ezdxf/examples_dxf/text_mirror_shx_font.dxf
# 8aabce617aa44b71dfc9d6ecdd2af5cfb5168ad2542e154c271b1fc4b347ad00 ezdxf/examples_dxf/text_mirror_true_type_font.dxf
# 6c183742a7c226e3196ba03b50eb8c32ab6a0dfbf12256d349ce2fc4b078b310 ezdxf/examples_dxf/text_negative_extrusion.dxf
# abbbb19a0bda300d1d56d9c845ca0dabb59eb737eb19f6dc6995e17e7f213862 ezdxf/examples_dxf/text_oblique_rotate.dxf
# 40fe35296b70b0314825637bb79e73f5b1ab24b7ff1a13e44b6d06f0e6bb5413 ezdxf/examples_dxf/text_stacked_shx_font.dxf
# 27ac1654952c6c0eeb669e786b1f43c0808df1e3ac103375e7a62362240f0bdb ezdxf/examples_dxf/text_stacked_true_type_font.dxf
# e302137e76a4178a9c81276c5c902222207940f0fbece31d9efcb9b331d16e35 ezdxf/examples_dxf/transform_hatch_pattern.dxf
# 5efdd671e3b6d8c45bd0688b1d6479d523f2290f0b390f1584348bf46e34c965 ezdxf/examples_dxf/uncommon.dxf
# d94201194185d1511416abe43c3bc679ba8c7bf99a8dd43cd96c4ce46c38fc61 ezdxf/examples_dxf/valid_lineweights.dxf
# c04a78677627e94c0056b2c01f641770439668932bd2288392ae9ec654a5e222 ezdxf/examples_dxf/viewport_frozen_layers.dxf
# b67896731e038949e6c11f21a90ea8d345009226f8ec82f24c22d11565be8275 ezdxf/examples_dxf/visibility.dxf
# 26794284e4e52db626b889fbbaf847fabb3c6ac85603096af244d7b6d95448bb ezdxf/examples_dxf/wipeout_door.dxf
# 672a9bcdf4d2f7caa5229e3b89f38dd741fb97237616b8027623833e2c9f2f21 ezdxf/examples_dxf/wipeout_random.dxf
# cc1e5ffd5c88bd4b11f74324dc397a45f337471267a6458d5b8a2a213e6f7551 ezdxf/examples_dxf/xclip.dxf
# 8a8d6f810d0f6bd1df2e5d89327e8608d6c1758ed97f0cc29de133e71402055d ezdxf/integration_tests/data/AC1003_LINE_Example.dxf
# b476d3e53fe24c1db3c701d20b2bebd774f7bd7966b12d81891505b9b29e4d21 ezdxf/integration_tests/data/ASCII_R12.dxf
# 5ed4ceca489c9541b075805a38b2990bc15946b27a8d14b5f43ff90d5ef82507 ezdxf/integration_tests/data/Leica_Disto_S910.dxf
# 0ac5a90615e6c88f0c2bbd474ffe3814c1a2f6dc2390769a671f410960328a21 ezdxf/integration_tests/data/MODEL.dxf
# 15f5dbecb643a46742e9e6662dcabb2b126f0bdd77672424db96329e13b317f5 ezdxf/integration_tests/data/R12_with_trash_beyond_EOF.dxf
# 7c42eac2e0c21b386273294b7937845ddda3da58f85c30fe130db229570f8015 ezdxf/integration_tests/data/acad_table_with_blk_ref.dxf
# 1e7a904d67bc7036bd3f78a6273cdf28124ba7e8b731604c0593288182f2aa0e ezdxf/integration_tests/data/bin_dxf_r12.dxf
# c65782683815ff0f1680d26207da1637a486f96b30bc215685e26cf7adee8464 ezdxf/integration_tests/data/bin_dxf_r13.dxf
# 177a97e5dc86ca106d0dd6365d139122fc54ff78df87670847eb7174a8bbd5eb ezdxf/integration_tests/data/bin_dxf_r14.dxf
# bdbe44e042bc6f56fbf5bd4fde0053fa1b82dcf90305f21a521a7b70d2c37320 ezdxf/integration_tests/data/bin_dxf_r2000.dxf
# 060ba0c25d29c7b84c493bb61a7664c58835741a8b5e738cd658923f35ad0779 ezdxf/integration_tests/data/cc_dxflib.dxf
# 7c43dac879d6698277d8981b538c7e1727840e6295fb7cf4db179f32a81d3e1a ezdxf/integration_tests/data/custom_blocks.dxf
# 62c3a8c91de338b0ff1646544d86360535b1c1b5e647777be890ca4b4f7527b8 ezdxf/integration_tests/data/duplicate_handles.dxf
# 41c2b8a11abea166ff3e4aa14a415c3bbbec42efa008992e3addc77c4f57d33a ezdxf/integration_tests/data/dxf_unicode.dxf
# 6dd16d1588dec3af6b768b0901853fbf396c7d1dcab7b1857f7a8ff0311534be ezdxf/integration_tests/data/empty_handles.dxf
# fc7cc924137ba09069064062d9c36e0f6527b3bedb559d26ea479fbef9f38ef2 ezdxf/integration_tests/data/groups.dxf
# 9995d3715b2aa8c8123445e23148c5fbc1bfbaff771e938312832ca79e2889a9 ezdxf/integration_tests/data/issue1106.dxf
# 6aa80b3a618a988dbd4333981428be2cf4c81475d0783fd64eef3d516f6954ba ezdxf/integration_tests/data/issue_1359_1.dxf
# d213da56027194c47bf5530e222d20d80e7a33ba752524e99b8b06807a922363 ezdxf/integration_tests/data/issue_1359_2.dxf
# 053b87539aaeac84c85ee6634dec32a4f1709f53e183bf9c2db6827ad4c3c198 ezdxf/integration_tests/data/layout_broken_links.dxf
# ee503adf650f8ef6f2945dacba2d5af78e34ed09d47495a6622f59ae96b20766 ezdxf/integration_tests/data/layout_broken_links_2.dxf
# 5bd921cfbd29a9de5e763ec0ba775a7d3356e59ba5b22f2d79b2a93ca93dac59 ezdxf/integration_tests/data/layout_missing_block_definition.dxf
# f979b175cd80dda2a1a395dd7b6a41dada187a373544f96c098be9bb1a1afb60 ezdxf/integration_tests/data/layout_missing_block_record.dxf
# 5b4925cfa745f916fc3a7d7e05bc4662b05a950c533fbe938bee6efcd7e0ead3 ezdxf/integration_tests/data/mtext_columns_R2007.dxf
# 0a42e4c9a47355a1f7107233a1415b38bea463b8b9fd4d42e8e4daab8eaca2ab ezdxf/integration_tests/data/mtext_columns_R2018.dxf
# bdcc561aebf79edf51a75e2d545760c4274d56737dad3561ba14187156f958c3 ezdxf/integration_tests/data/mtext_framed_columns.dxf
# 380ca558f6960bc58d235d239e8131cef6f0bc4127f97b34296417d89a270dec ezdxf/integration_tests/data/mtext_text_frame.dxf
# 65d1a55530a617de9e4333ef44b38617971ccd387b15512f6dc3eb16e71eac8a ezdxf/integration_tests/data/no_layouts.dxf
# 5516994924c40e73822b2844ac01e73f8f7603673a1688f22f3a293b45dd10d0 ezdxf/integration_tests/data/r12_blocks_with_no_names.dxf
# 8c9dcc06384aada2b79e898260b82ec11fb7816541adec565db64923e39e8398 ezdxf/integration_tests/data/r2000_blocks_with_no_name_and_no_block_record.dxf
# 8becaa0b2fdd05cfbed60c51f00c52e7a6e6c48b05b55fcf6017e29d09a5717c ezdxf/integration_tests/data/r2000_blocks_with_no_names.dxf
# 02a47bc1e4263a2f4f9548eba919c5a95c22f6ebaeb10a69ac5ea94a4cfab2de ezdxf/integration_tests/data/recover01.dxf
# 24c948c7d9e8993394ae4d5c2a6956bc0b07f50b68edf4bcd358a6ed49c4b795 ezdxf/integration_tests/data/recover02.dxf
# a1007452f8ada2e71fdef60f79923924045cb371f37eeab896a61f5628573d0b ezdxf/integration_tests/data/small_r13.dxf
# 92e792f8f9169e4226c9b7dbe848ba9afb96d3e388e6297c55481e1941c65fb0 ezdxf/integration_tests/data/small_r14.dxf
# c27e3fd210fc50c807b07a459bf57b0a38531123fa980439dc333a802a39181d fess/files/cad/autocad_2000.dxf
# feaa6c5384382488ed708b2448bc039ee183785779b767d0a8e8a801a64d8b09 fess/files/cad/autocad_2004.dxf
# 3076d3f560355d831858f1f98bab7d5945ce7657aefa81021fb01100caddbeb7 fess/files/cad/autocad_2007.dxf
# b488dee4ed8ecd61ea0947c0bc9fef581247c82d4214160285e524ace40d5613 fess/files/cad/autocad_2010.dxf
# 58b483d40d88cf1a8d0f7a40fe41a269c8241166e1c30bca1d1a44e9941b74aa fess/files/cad/autocad_2013.dxf
# 7bfb3581d515bcf3516d9df0cdbed1edae564b456f5f26c8d327780b878e2e2a fess/files/cad/autocad_2018.dxf
# e93eb3419824e74c6ec574c3944811a10ba4593a91554598f80b2617ed6e4da9 fess/files/cad/autocad_2021.dxf
# 303072315ab921dddb93c1e5031f20428fefc1d731896a02e7503a16b7d03836 fess/files/cad/autocad_2024.dxf
# 6c92470133e67c89226981ab1234b7445c87638a62a383277b58c42e697610c4 fess/files/cad/autocad_R12_LT2.dxf
# bf6ee31550567592fdd203ef1e4f1a76f8a1d532328e9d9c248e4a9b692a1a2f gdal/autotest/ogr/data/dxf/3d.dxf
# 0f340b83f05a236d5f21069f8fe6c52bb55e9ab803d67c4efe8a3905b050e16c gdal/autotest/ogr/data/dxf/3dface.dxf
# 713a7e6691d6647f1e1c038ffbea3cda32949aaa2e30e7dc614d76b6b691d5f3 gdal/autotest/ogr/data/dxf/BINARY_wipeout.dxf
# b777275065c02e9afde9bbadc500f663095b72ed78f074c893a2f6ecf6b89201 gdal/autotest/ogr/data/dxf/LWPOLYLINE-OCS.dxf
# 04dfba0307cf30e1f22eb4c6bdaf06780aaa8ede14f5c40c836ddcba6002788a gdal/autotest/ogr/data/dxf/additional-entities.dxf
# c0226925e3ca9b896def206af49c7364b80bbc9154b019da3937e68ee6ae701d gdal/autotest/ogr/data/dxf/assorted.dxf
# 969457991f061563b22d5e2cdc5a6b10c809b2acc103fd9d4343b8ca7d40ef11 gdal/autotest/ogr/data/dxf/attrib-nested.dxf
# fe75d29270b849df8981e4d41fdf546e5e86f49ede81eacabf2fbe74f894f2fc gdal/autotest/ogr/data/dxf/attrib.dxf
# 1e7a904d67bc7036bd3f78a6273cdf28124ba7e8b731604c0593288182f2aa0e gdal/autotest/ogr/data/dxf/bin_dxf_r12.dxf
# bdbe44e042bc6f56fbf5bd4fde0053fa1b82dcf90305f21a521a7b70d2c37320 gdal/autotest/ogr/data/dxf/bin_dxf_r2000.dxf
# 79ae8c3a7daec34d4f31ff04a266d9e4c3ed782b2ec96ad72d49771e536d3754 gdal/autotest/ogr/data/dxf/block-basepoint.dxf
# 419c155e51a7780a19a62d89324fe82b2551ad263ca6424b26b50d93fde5ff74 gdal/autotest/ogr/data/dxf/block-hidden-entities.dxf
# 7058b1b7a19ce2b6d9e728c0f71aa48302b9c1ca46da1fb0c117aab639b468ba gdal/autotest/ogr/data/dxf/block-insert-order.dxf
# d3738953840d72c565f99abd75ad4bea78d509d59cc066b3076f7a29fa5601da gdal/autotest/ogr/data/dxf/byblock-bylayer-new.dxf
# 6b0ca98257f5c4bcddcbbf9cdd0ae3f5dfe510af45d5d4388815a560310cf0a1 gdal/autotest/ogr/data/dxf/byblock-bylayer.dxf
# a843aca9c19c16a9398551b035f6817b1b4b6aae958d176569f0a59910bad6f2 gdal/autotest/ogr/data/dxf/circle.dxf
# 5863e2b676c227be5f5dc00e831657c5ea1ff2d88f9d53443e7519cff0773d6e gdal/autotest/ogr/data/dxf/closed_polyline_with_bulge.dxf
# 25ca8e9be611bf05f78816c2361fecac491d4f3f561ff8b1be1b62687c1eb1eb gdal/autotest/ogr/data/dxf/clusterfuzz-testcase-minimized-dxf_fuzzer-5400376672124928.dxf
# ba1f2186b6f28e085e5beb510b06bce945d35c3fa8b34e504448c30ea67953e2 gdal/autotest/ogr/data/dxf/clusterfuzz-testcase-minimized-shape_fuzzer-6126814756995072.dxf
# 9f9d83683e8a9293d49db1ebfb30a84287f8326a502d7eb93a4ac3acc8c062ed gdal/autotest/ogr/data/dxf/dimension-entities-only.dxf
# 541d9283a9db397a9f3d2043fde879867958cd351a608dc0fd2cc31fcf2041c8 gdal/autotest/ogr/data/dxf/dimension.dxf
# 213bc73e32b2c2eaf916524acf9e358ff6b2cc11340dee914bf7ebdb3559262d gdal/autotest/ogr/data/dxf/ellipse_z_extrusion_minus_1.dxf
# daa87be8d8f3a4b2d60013ce8dcc1249dd5b7dd016a0f3edc8bb9fddb5087db8 gdal/autotest/ogr/data/dxf/elliptical-arc-hatch-min.dxf
# e5042775846ef1378f4701e134a234673a3aabac24eb39d02c728aaa51f183e1 gdal/autotest/ogr/data/dxf/entities_only.dxf
# fc3054f976d95161e4a13724e46ebc3c3c9d1108064bcb875e51f58982adc0cc gdal/autotest/ogr/data/dxf/frozen-off.dxf
# 84a830727109d0371e1ba156105e635440514782ed5c6838cac3b593f0b5b3e6 gdal/autotest/ogr/data/dxf/hatch.dxf
# f14c1d4cad64035c4857a2214b6798300149314509e8ca8b9ad007911a4c2ba2 gdal/autotest/ogr/data/dxf/hatch_as_multipolygon.dxf
# da2229319f8963806c582b0ea4c82be3074abe23e18548b3313a4df337c4328c gdal/autotest/ogr/data/dxf/hatch_pattern_generated_by_gdal.dxf
# b929894a20ccc5b6332cd1212a8b0c88ea86c78e3c275a7381a7dbc11518a4cf gdal/autotest/ogr/data/dxf/header_extended.dxf
# c50903b56d87946ad42f6d037e94c8b47da7cf48e61cc090f44a0a8c45d7a5c2 gdal/autotest/ogr/data/dxf/insert-recursive-pair.dxf
# 0f5d481ccad6f9b39b6d6d04d3565ae9a3e26cabdc46d1ecb7b140b827244a0a gdal/autotest/ogr/data/dxf/insert-too-many-errors.dxf
# f4d393d14688605db21c9e59ea6caa4c1baaa8043e61b3f71e77e1d2830e9603 gdal/autotest/ogr/data/dxf/insert_only.dxf
# 6a4806ab9a93db656d04d9f66aea9be106c93fcc584a6d9ee6f9c1fe24dfcefb gdal/autotest/ogr/data/dxf/insert_only_col_count_zero.dxf
# 5ed87163c08b2d2dd08d2a189ab0a42faa36b5d358144980dfd24159a4273538 gdal/autotest/ogr/data/dxf/leader-mleader.dxf
# 69671125b99a2a5abf048cd7e9c880b5477cd90589cc7d3255427a4159e21701 gdal/autotest/ogr/data/dxf/linetypes.dxf
# 493a352db8ac29043aeb6c500418e298d86889b71b88f4463f2a0f4f6d3deb38 gdal/autotest/ogr/data/dxf/lwpolyline_smooth.dxf
# b394bc388aa5edab1d0def8eb7853f4610fdebd3d6cb4434182410589c57b652 gdal/autotest/ogr/data/dxf/mtext-ocs-reduced.dxf
# 59ac900011b83aa0dd332eabce31e2e8aefe3d4b2333426cfa0356250da68dfe gdal/autotest/ogr/data/dxf/ocs2wcs1.dxf
# 11f070177bc1fc5ec8efe0bfc3ce36af987ee5ea27d0f45b38c45e666f992dd9 gdal/autotest/ogr/data/dxf/ocs2wcs2.dxf
# e57e3227f6ccee02917f12404dcc9950588142e339a478946a045b7c470ed11e gdal/autotest/ogr/data/dxf/ocs2wcs3.dxf
# b41de4f6991a2b469ba205aee97743d5c13b4b8494af0ecceab56a142cb99d74 gdal/autotest/ogr/data/dxf/polyline_smooth.dxf
# fb1d8eb74d25caddb6c0b14fa6cb4d4036acde9baba520dff7c5a7f1316db683 gdal/autotest/ogr/data/dxf/solid-less-than-4-vertices.dxf
# ab65aa3823e385c52c1fde7f15e0deadc6f4c12f1cb26431111cd5ff977ec1da gdal/autotest/ogr/data/dxf/solid-vertex-ordering.dxf
# ff8355d18f5cd91c9f0e98e802c5835fee0ea38685c668f8f8a96311f2184404 gdal/autotest/ogr/data/dxf/solid.dxf
# 623d2d20d75180290edda0c5d48cc08b43bdfac5cf9d627b2c1c3239a91cd100 gdal/autotest/ogr/data/dxf/spline_closed.dxf
# b9ab3ec8070b546e9248139ad3238db95be1abfddbc2b7c0b36fd0c5dfa8f58c gdal/autotest/ogr/data/dxf/spline_qcad.dxf
# 2912d57746bddc01570755d4275073d014433042fd5e20635c9edcdfa683941f gdal/autotest/ogr/data/dxf/spline_weight.dxf
# e97526865ae10212fe74c97e135874896ea7ddfbb3f4a778e06534354d03198c gdal/autotest/ogr/data/dxf/spline_with_very_close_neg_to_zero_knot.dxf
# 16d51c8403b6e98711a5973ac2abaac9008790d14bed9ff5b69cca8a3665744a gdal/autotest/ogr/data/dxf/text-block-transform.dxf
# 8e88d8f184515581605ec0e3542d40e06d9a373603f9ab5c5885b635f8dde0a5 gdal/autotest/ogr/data/dxf/text-fancy.dxf
# 1e548a289643fbc6a12eff2e79ed8f748b6a25e2f9d8f7f1a1f73e245e2b160d gdal/autotest/ogr/data/dxf/text.dxf
# 18ced742b40621e5315e29faf82168b8f6ff56c8c36d703505e481ea03382f8d gdal/autotest/ogr/data/dxf/transparency.dxf
# 952ae219659c70ff82b2c5c6a2ceeedb8b7387ca6ed2ad8551beb580453e5838 gdal/autotest/ogr/data/dxf/utf-8.dxf
# fd00f6651b422cd08cf0946552bc8e0605176dea9c70e3baf0b3b4061cd0724a gdal/autotest/ogr/data/dxf/wipeout.dxf
# bf6ff5af075ec34f9e1621bd3e04bd18e98598dc7851ef89568c48b2316c9b16 jscad/dxf/autocad2017/2Darcs.dxf
# adaa796dfa8f0723c93207ec8820bc4caab74f40b6e868fbe764a09b0d7714d4 jscad/dxf/autocad2017/2Dcircles.dxf
# 7d4a1e979b67e43d1c5f9fdc9ebf0e298e0111efe619aa5a1ef56a8060d8b0d5 jscad/dxf/autocad2017/2Ddonuts.dxf
# 2051fa1e65af00038afcfed6bac8b162f20b8d77849d88834743e8e5ca1db7fc jscad/dxf/autocad2017/2Dellipses.dxf
# 7cbda11ff86caa8b46de61f0bab03ce3d0b6ff9bc36f5711e9cd54fda383fb4a jscad/dxf/autocad2017/2Dhatch.dxf
# 7e7a1d538c19937a205818a79767014d01f62721908e871c68927a25d29b792c jscad/dxf/autocad2017/2Dhelix.dxf
# d238b6507d88c145388d3b18f8e9ce6c84fd2c45fd01f161129b961b2dc10cae jscad/dxf/autocad2017/2Dlines.dxf
# d8200e7a5abea82b885c757745f487667bc8e8d3da0695f768f0a489c7eaca37 jscad/dxf/autocad2017/2Dmlines.dxf
# 479a46ccb49075ce527e4ce47a1af416d0fede5550c454cb7f359b3a21c3795a jscad/dxf/autocad2017/2Dpoints.dxf
# a84d9ede1e3095863f5ed15c3c5c1ef558ee4c6d78a32fe145a21c40aed990b4 jscad/dxf/autocad2017/2Dpolylines.dxf
# d215f284ec98766c41574603d0445856fd16d6a399339546dede4daf660b2e34 jscad/dxf/autocad2017/2Drectangles.dxf
# 28780b15db925c0820853c90459d52c1f9b7959af08f80013fc80b0df74cd381 jscad/dxf/autocad2017/2Dxlines.dxf
# 0a6f00da9e14e1a69f80bdb71c60cc99a652aff7b20625ba79f3a1a30c33953b jscad/dxf/autocad2017/3Dboxes.dxf
# 26b36ce678dc34da682c0c89654bed4408918cd9754734b2fdfed45fb6304a20 jscad/dxf/autocad2017/3Ddraw.dxf
# 84731ebc5bbc3b158427b67cc12e02ebbdc38adf389b1d99f1c2249285357cd0 jscad/dxf/autocad2017/3Dhelix.dxf
# 4bc4204815f85f912a2d29c85b07d5fcdc2cd3bcb65763e3bf854309e295d562 jscad/dxf/autocad2017/3Dlines.dxf
# 280e9d4c1111f9e9283416fc9011c832adda97f60c51b3be6c2a83ef4f2b3684 jscad/dxf/autocad2017/3Dmesh01.dxf
# d517e691e84e70495fc0521720ee31267eef0285416b9ef726dc9428447d6579 jscad/dxf/autocad2017/3Dshapes.dxf
# 258e7ea4f9ef722bc8152978fafa0fa3587a425eccac3e2991a830aa2ff574eb jscad/dxf/autocad2017/empty2000.dxf
# a6448d9fcc5490e8c620816691a5ded745945692e4f7a54662b15a2214685a76 jscad/dxf/autocad2017/empty2004.dxf
# 80ebb3ffec87dbf00a40047bd4af885cf7baf63c12d55df88066fafea6ed3c19 jscad/dxf/autocad2017/empty2007.dxf
# ac1fff8950d422494a5fea6d001d3c0e9b9bee555797282ae757600e553a7ad7 jscad/dxf/autocad2017/empty2010.dxf
# ae3c3162febe13fdee3b463506cf257bade715181aeaf54403321ceec680e92e jscad/dxf/autocad2017/empty2013.dxf
# 59f513f27433ae9736b63e091638acd49d013388e12a56c8493bc3f454812fcf jscad/dxf/autocad2017/emptyR12.dxf
# 9603f83ade410225efe500c8355eef8673614714c426222c36dc93b58043cc9e jscad/dxf/bourke/3d-entities.dxf
# 36f300dfc3823a6529db30f8ad9e0e815de813f3cba248dbe1d863f42f6f8632 jscad/dxf/dxf-parser/accumulatortest.dxf
# ba33677808f6a534d64a5898803649caca30eccb543092e3e5a102043b9d1c2e jscad/dxf/dxf-parser/blocks1.dxf
# 5d847d833dd444fbe8a85def5881a4f9055e47d06c41ca009c9e40784e92f91f jscad/dxf/dxf-parser/blocks2.dxf
# 0580d6ae3df2afe8f35e57970fa0d4556450cf4b17d519b6b7380814ae49ce78 jscad/dxf/dxf-parser/circlesellipsesarcs.dxf
# 8e24d7f34d6532b5b3ab3576e9e44069433400d6f803fdfe828c22ccc11467b2 jscad/dxf/dxf-parser/closedlwpolylinebug.dxf
# 513d44f4b2febdf2d2e27016c400cc774f0a364620b34730eaaece5cd6f15d77 jscad/dxf/dxf-parser/dimensions.dxf
# eff79f24ba4c8ac1ccfa1d82d0cdd42c5e9b2b027d275f1704248accf422acbf jscad/dxf/dxf-parser/ellipticalarcs.dxf
# 378f5e9c08d74888971b0a427c7b6a64ac6ebfdf3c163714a6acdb942a76d2d2 jscad/dxf/dxf-parser/ellipticalarcs2.dxf
# 56b9bc2de32b24c7ca3e29071d8cf6c7f9c27df1c936948d48c891ac8fd92c58 jscad/dxf/dxf-parser/empty.dxf
# fa0b296dc6c060ce8d79c61dd6461c9e1523a15ae33cdc650175ee8b9270d1d8 jscad/dxf/dxf-parser/entities.dxf
# 6d761ecb1ab59664425eb1dcc6837a12611536c8e03e3f7855951a76786d159c jscad/dxf/dxf-parser/floorplan.dxf
# e67becc4b026231d656a4b506b90d533a3fcdfb14e3003762edd4a9e40b7e6d3 jscad/dxf/dxf-parser/hatches.dxf
# 860c3085c10b754d75fd903747f78a0e52c69b94ac77f23bdb018bec41b860ff jscad/dxf/dxf-parser/layers.dxf
# 052d8246dcf762c4100a4df8a68bed1964da5d1ee087a86af9996c7b6c6e6a30 jscad/dxf/dxf-parser/lines.dxf
# cbc8fbf3016fd7b580f3a077092c63d1903728111f283e9907c774ce94e7a3e4 jscad/dxf/dxf-parser/lwpolylines.dxf
# dfe3b732026b655dc89ad2b985944ccd96d4ab7429f2adb1ccc7df36c8686364 jscad/dxf/dxf-parser/openscad_export.dxf
# 5f0397697996b75078994e0fe95bb12a87864f68f49c14018194611edb30944f jscad/dxf/dxf-parser/points.dxf
# af6d2781d7961e531c34e27c7d0f8a4c4b885e579cdcbcd5e5a41766074851a0 jscad/dxf/dxf-parser/polylines.dxf
# 05b79acc4b1c619813d15e8cb0a041a04d716e8f42ede42373ae6216067e1ae9 jscad/dxf/dxf-parser/rectangle.dxf
# 4f6456eba44b012c250311a0f0205a7bf8f3f844a443820cf7f4e45031b7ef92 jscad/dxf/dxf-parser/splineA.dxf
# 9f672526c3e6b35c60ed958650a5c35fe3ee4f8df76d1f8b25b31f2f6d293406 jscad/dxf/dxf-parser/splines.dxf
# cdae8ce1750984586723c7d0c784ad8c01d671b6c70c7802f35f0ea62f21938c jscad/dxf/dxf-parser/squareandcircle.dxf
# 6c585c0093c1a19180faa58b6977bc48ddb47899d2800bf38807cbca3018416d jscad/dxf/dxf-parser/texts.dxf
# a57541a8c5ee9a6a7e1ae71dc9e58d8c113e0b281e86dcae3015c8811bebe808 jscad/dxf/ezdxf/AC1027_mesh.dxf
# 7c43dac879d6698277d8981b538c7e1727840e6295fb7cf4db179f32a81d3e1a jscad/dxf/ezdxf/CustomBlocks.dxf
# ef0afbfcddd219d8ff76ef26775781469424e671213f1235132e0286def08402 jscad/dxf/ezdxf/Minimal_DXF_AC1009.dxf
# 6866a50766e178f19356607ee3d3fe619f9f46cfcd2d7d5969f1077eb675464d jscad/dxf/ezdxf/Minimal_DXF_AC1021(1).dxf
# 6866a50766e178f19356607ee3d3fe619f9f46cfcd2d7d5969f1077eb675464d jscad/dxf/ezdxf/Minimal_DXF_AC1021.dxf
# 4d164c7963595fb52dfe2080bf1537dfddb93cd892e5889406e65de39584b922 jscad/dxf/ezdxf/Spline_R2000.dxf
# f9708a3f555f039378d34941ae5abc12238742cad3734e93650aa51267c23ff2 jscad/dxf/ezdxf/ansi_pattern.dxf
# 4a65eac65e2a1dec47358b8462c55ee51dc8252e28dd590071f04cdf8196d362 jscad/dxf/ezdxf/iso_pattern.dxf
# 65d1a55530a617de9e4333ef44b38617971ccd387b15512f6dc3eb16e71eac8a jscad/dxf/ezdxf/no_layouts.dxf
# 4a26f8a83c0e43d0338b593d35c44d3952e9f4698016c2333d0718c6396e76dc jscad/dxf/ezdxf/other_pattern.dxf
# a1007452f8ada2e71fdef60f79923924045cb371f37eeab896a61f5628573d0b jscad/dxf/ezdxf/small_r13.dxf
# 92e792f8f9169e4226c9b7dbe848ba9afb96d3e388e6297c55481e1941c65fb0 jscad/dxf/ezdxf/small_r14.dxf
# bb16aa60b7abd6c47310613b15398fec55ddda15d3e27cbb659eb85260420302 jscad/dxf/jscad/circle10.dxf
# 184d31bcd5100148c22dda74cc7cbb47c605083f75597248677b864b82e95c23 jscad/dxf/jscad/cube.dxf
# 3a9cec8813d1576d4f8ac7137c28df1bdd2aa028ae73e00b80a9c93735f5e6f7 jscad/dxf/jscad/cylinder.dxf
# e38ac46eae7a376a974f61ed61e09506ef5bb9841c4268477e0a8697c7fe3f28 jscad/dxf/jscad/pyramid.dxf
# 215a2a538e0bbc483e68fe48850ca7201fe544ff105ef634092fa7c6def69162 jscad/dxf/jscad/sphere.dxf
# 80dee4e7d65ae958b5a6f46ab32d4e74e9d191b0c0daa77e974c0fd14f374f59 jscad/dxf/jscad/square10x10.dxf
# 274f7abdf35845a9849d4b6ef5744947f5f39031d8a642e75b4be701fbb92994 jscad/dxf/others/heart.dxf
# 0e3709cb42438aa23b8534304b46a382622453b5ad2756f6484e4bc392e3e73a jscad/dxf/others/pyramid-base.dxf
# 3cc8c2b182db2b21385fd7b88d5724a499b19e5f88e561c32f28896b1fbbb7c1 large/floorplan-tiled.dxf
# SHA256 END
