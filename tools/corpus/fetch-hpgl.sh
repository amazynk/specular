#!/usr/bin/env bash
# Fetch the public HP-GL / HP-GL/2 test corpus into tools/corpus/hpgl/ (gitignored: nothing
# downloaded here is committed). The specular-cad HP-GL tests read it in place from
# SPECULAR_HPGL_CORPUS, else from tools/corpus/hpgl, and skip when it is absent.
#
# Usage: tools/corpus/fetch-hpgl.sh [SECTION...]
#   SECTION is a function name below (e.g. src_hp2xx). Default: all sections.
#   OUT defaults to $SPECULAR_HPGL_CORPUS, else tools/corpus/hpgl.
#
# What it covers (about 1.3 MB, 68 files): 7475A/7550A-era HP-GL from the hp2xx test suite
# (labels and character sets, LO/DI/DV/SI/SL, CP, LT/UL line types, IP/SC/IW/RO, arcs and
# circles, fills FT/RA/RR/EA/EW/WG/PM/FP/EP, 7475A ESC. device control, multi-pen, pages),
# HP-GL/2 inside PCL5 (ESC E, ESC%1B / ESC%0B, ESC%1A, UEL, PJL; from the hp2xx and GhostPCL
# examples), real CAD plot files with 8-bit polyline-encoded PE data, PC/PW/NP pen tables and
# PM/FP polygons (ezdxf), and plotter output of Inkscape, vpype and hand-written plotter tests.
# derived/ is made here from ezdxf's PLOTFILE.plt: the same plot wrapped in PJL only (UEL +
# ENTER LANGUAGE=HPGL2, as HP DesignJet drivers write it), wrapped in PCL5 (ESC E ... ESC%1B
# ... ESC%0A ESC E), and with every PE re-encoded in 7-bit (base-32, "7" flag) mode; all three
# must plot exactly like the original. Sources and licences are logged in .provenance.tsv;
# every file is checked against the SHA-256 list at the end of this script.
#
# Downloads are untrusted data: this script only copies files, never runs anything from them.
# Scratch space is target/corpus/hpgl-fetch (deleted afterwards). Stops if free space on
# the target filesystem drops below MIN_FREE_GB (default 6).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_HPGL_CORPUS:-$ROOT/tools/corpus/hpgl}
SCRATCH="$ROOT/target/corpus/hpgl-fetch"
MIN_FREE_GB=${MIN_FREE_GB:-6}
MAX_MB=${MAX_MB:-100}
export GIT_LFS_SKIP_SMUDGE=1 GIT_TERMINAL_PROMPT=0
mkdir -p "$OUT" "$SCRATCH"

die() { echo "fetch-hpgl: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

# provenance RELPATH URL LICENCE
provenance() { printf '%s\t%s\t%s\n' "$1" "$2" "$3" >> "$OUT/.provenance.tsv"; }

# git_files URL COMMIT DEST LICENCE PATTERN...
# Sparse, blobless, depth-1 fetch of PATTERN... (git sparse-checkout --no-cone patterns) at
# COMMIT; copies every matched file to $OUT/DEST/<repo path>. Files named *.out (Inkscape's
# expected-output files) get .hpgl appended so the corpus is recognisable by extension.
git_files() {
  local url=$1 commit=$2 dest=$3 lic=$4; shift 4
  local work="$SCRATCH/$dest"
  local web=${url%.git}
  check_space
  rm -rf "$work"
  git init -q "$work"
  git -C "$work" remote add origin "$url"
  git -C "$work" config core.sparseCheckout true
  git -C "$work" sparse-checkout set --no-cone "$@"
  git -C "$work" fetch -q --depth 1 --filter=blob:none origin "$commit"
  git -C "$work" -c advice.detachedHead=false checkout -q FETCH_HEAD
  local n=0 blob
  case $web in
    *gitlab.com*) blob="$web/-/blob/$commit" ;;
    *) blob="$web/blob/$commit" ;;
  esac
  while IFS= read -r -d '' f; do
    local rel=${f#"$work"/} name
    name=$rel
    [[ $name == *.out ]] && name="$name.hpgl"
    mkdir -p "$OUT/$dest/$(dirname "$name")"
    cp "$f" "$OUT/$dest/$name"
    provenance "$dest/$name" "$blob/$rel" "$lic"
    n=$((n + 1))
  done < <(find "$work" -path "$work/.git" -prune -o -type f -print0)
  rm -rf "$work"
  echo "  $web: $n files -> $dest"
}

# ------------------------------------------------------------------------------- sections

src_hp2xx() {        # hp2xx hp-tests: HP-GL/1 manual examples, fills, labels, PCL-wrapped HP-GL/2
  git_files https://github.com/dgtlrift/hp2xx 5e1f411c5e6cee87be5d0ce39272d3fd272b77ac hp2xx \
    'GPL-2.0-or-later (hp2xx; examples partly from HP plotter manuals)' \
    '/hp-tests/*.hp' '/hp-tests/*.hpg' '/hp-tests/*.hpgl' '/hp-tests/*.plt'
}
src_ezdxf() {        # ezdxf: real HP-GL/2 CAD plot files (8-bit PE, PC/PW pens, PM/FP polygons)
  git_files https://github.com/mozman/ezdxf 7c64bce52c9d60b9db2dad4d2793c1168c8d3f64 ezdxf \
    'MIT (ezdxf)' '/examples_hpgl2/*.plt' '/integration_tests/data/PLOTFILE.plt'
}
src_ghostpcl() {     # GhostPCL examples with embedded HP-GL/2 (ESC E, ESC%1B, UEL, raster mixed in)
  git_files https://github.com/ArtifexSoftware/ghostpdl 3346f1992e9ea5988d2dd15c4ef4d6b0db97ec8d \
    ghostpcl 'AGPL-3.0-or-later (GhostPDL)' \
    '/pcl/examples/gl-chars.pcl' '/pcl/examples/grid.pcl' '/pcl/examples/origins.pcl' \
    '/pcl/examples/owl.pcl'
}
src_inkscape() {     # Inkscape extensions: HP-GL input test file and HP-GL output references
  git_files https://gitlab.com/inkscape/extensions.git 59fbb7bb7e2abd824339bda3e203a0bfcb06e5c8 \
    inkscape 'GPL-2.0-or-later (Inkscape extensions)' \
    '/tests/data/io/test.hpgl' '/tests/data/refs/hpgl_output__*.out' \
    '/tests/data/refs/plotter__--serialPort____test__.out'
}
src_dwm() {          # Drawing With Machines (CMU): vpype / hand-written 7475A, 7550A, DraftMaster plots
  git_files https://github.com/golanlevin/DrawingWithMachines \
    141ff0ba6bf7753a33bacf1823cceb1fe8771e1a dwm \
    'no licence declared (golanlevin/DrawingWithMachines); dev corpus only' \
    '/machines/hp7475a/vpype/lissajous.hpgl' '/machines/hp7550a/hpgl/*.hpgl' \
    '/machines/hp_draftmaster/hpgl/*.hpgl' '/machines/hp_draftmaster/workflow/*.hpgl'
}
src_plottertools() { # WesleyAC plotter-tools: canonicalize test input (PA/PD with implicit moves)
  git_files https://github.com/WesleyAC/plotter-tools 4a285e167421d2a917561413cda4e8724e860f5c \
    plotter-tools 'no licence declared (WesleyAC/plotter-tools); dev corpus only' \
    '/hpgl/testdata/canonicalize.hpgl'
}

src_derived() {      # derived/: ezdxf PLOTFILE.plt wrapped in PJL / PCL5 and re-encoded as 7-bit PE
  local src="$OUT/ezdxf/integration_tests/data/PLOTFILE.plt"
  [[ -f $src ]] || src_ezdxf
  check_space
  mkdir -p "$OUT/derived"
  python3 -I - "$src" "$OUT/derived" <<'PY'
import os
import sys

# Pure byte transforms of an HP-GL/2 plot; nothing in the input is executed.
src, out = sys.argv[1], sys.argv[2]
body = open(src, "rb").read()
UEL = b"\x1b%-12345X"


def write(name, data):
    with open(os.path.join(out, name), "wb") as f:
        f.write(data)


# PJL only: DesignJet-style job, HP-GL/2 entered via PJL, no PCL escapes at all.
write("PLOTFILE-pjl.plt",
      UEL + b"@PJL JOB NAME=\"PLOTFILE\"\r\n@PJL ENTER LANGUAGE=HPGL2\r\n" + body
      + UEL + b"@PJL EOJ\r\n" + UEL)

# PCL5: reset, landscape, picture frame, enter HP-GL/2 with ESC%1B, back with ESC%0A, reset.
write("PLOTFILE-pcl5.pcl",
      UEL + b"@PJL ENTER LANGUAGE=PCL5\r\n\x1bE\x1b&l1O\x1b*c0T\x1b%1B" + body
      + b"\x1b%0A\x1bE" + UEL)


# 7-bit PE: every PE command gets the "7" flag and its numbers re-encoded in base 32
# (digits 63..94, terminal digits 95..126) instead of base 64 (63..126 / 191..254).
def pe7(args):
    res = bytearray(b"7")
    digits = []
    for c in args:
        if c <= 32:
            continue
        if c in b":<>=7" and not digits:
            if c != ord("7"):
                res.append(c)
            continue
        if 63 <= c <= 126:
            digits.append(c - 63)
        elif 191 <= c <= 254:
            digits.append(c - 191)
            v = 0
            for d in reversed(digits):
                v = v * 64 + d
            digits.clear()
            enc = bytearray()
            while v >= 32:
                v, r = divmod(v, 32)
                enc.append(63 + r)
            enc.append(95 + v)
            res += enc
        else:
            raise SystemExit(f"unexpected PE byte {c}")
    if digits:
        raise SystemExit("unterminated PE number")
    return bytes(res)


res = bytearray()
i, n = 0, len(body)
while i < n:
    c = body[i]
    if c <= 32 or c == ord(";"):
        res.append(c)
        i += 1
        continue
    if not (chr(c).isalpha() and i + 1 < n and chr(body[i + 1]).isalpha()):
        raise SystemExit(f"unexpected byte {c} at {i}")
    name = body[i:i + 2]
    j = i + 2
    if name in (b"LB", b"DT"):
        raise SystemExit("labels not handled")
    if name == b"PE":
        k = body.index(b";", j)
        res += b"PE" + pe7(body[j:k]) + b";"
        i = k + 1
        continue
    while j < n and not chr(body[j]).isalpha() and body[j] != ord(";"):
        j += 1
    res += body[i:j]
    i = j
write("PLOTFILE-pe7.plt", bytes(res))
PY
  local f
  for f in PLOTFILE-pjl.plt PLOTFILE-pcl5.pcl PLOTFILE-pe7.plt; do
    provenance "derived/$f" \
      'generated by tools/corpus/fetch-hpgl.sh from ezdxf/integration_tests/data/PLOTFILE.plt' \
      'MIT (ezdxf), derived'
  done
  echo "  derived: 3 files -> derived"
}

verify() {
  local bad=0 sum path
  while read -r sum path; do
    [[ -n $sum ]] || continue
    if [[ ! -f $OUT/$path ]]; then continue; fi
    if [[ $(sha256sum "$OUT/$path" | cut -d' ' -f1) != "$sum" ]]; then
      echo "fetch-hpgl: sha256 mismatch: $path" >&2
      bad=1
    fi
  done < <(sed -n '/^# SHA256 BEGIN$/,/^# SHA256 END$/p' "$0" | sed -e '1d' -e '$d' -e 's/^# //')
  (( bad == 0 )) || die "sha256 mismatches (see above)"
  echo "  sha256: ok"
}

check_size() {
  local mb
  mb=$(du -sm "$OUT" | cut -f1)
  (( mb <= MAX_MB )) || die "corpus is $mb MB (> $MAX_MB MB)"
}

main() {
  local sections=() s
  for a in "$@"; do
    case $a in
      -h|--help) sed -n '2,25p' "$0"; exit 0 ;;
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
  check_size
  rm -rf "$SCRATCH"
  du -sh "$OUT"
  df -h "$OUT" | tail -1
}
main "$@"
exit 0

# SHA256 BEGIN
# 007b3d97ce2c2bd84ed89a49a511082730c2ab1187e69316ebcf5ed376135c8e derived/PLOTFILE-pcl5.pcl
# 157ab747eb03605887e660df53189ef67baafb103c4e3533f2c34b04c525f223 derived/PLOTFILE-pe7.plt
# f4c0ed9c4fd5a94674fb35f846bdfd401b593777ebc5263b5d2542445a902601 derived/PLOTFILE-pjl.plt
# ad65a9f4d094ab675dff621427d4284d4ffc9371d8f8f1bf4a2e25796ece7976 dwm/machines/hp7475a/vpype/lissajous.hpgl
# 9a48e51e54993501f68c91999b747687a4d2fa0db498f4848078126ad192a096 dwm/machines/hp7550a/hpgl/test_letter_landscape.hpgl
# b9f0471b4644b7908cafc3eefc7792d5721a6e4359b8dcdf280b35618be6b85f dwm/machines/hp7550a/hpgl/test_letter_portrait.hpgl
# a6248925531c6db2713b656517e5fd53d015700af014df907904ab65b4fa5b8c dwm/machines/hp7550a/hpgl/test_tabloid_landscape.hpgl
# 85b76739b759e9a2b134bf917ce4266bc1413d5d4c1c5c3a8b4a652832205a5e dwm/machines/hp7550a/hpgl/test_tabloid_portrait.hpgl
# 5aeb2c025dc09721a6285fff331bb39598db99ce0b02e4a25881724dc2886b66 dwm/machines/hp_draftmaster/hpgl/draftmaster_arch_d_horizontal.hpgl
# 664aca1daa2d50fbae4a5020182576ff7f3aee71a98b35fbae63f5cd8a8111d7 dwm/machines/hp_draftmaster/workflow/hp7596_24x36_lissajous.hpgl
# 84be2f4417c3ff098f046c284a4bf19dbaafa3c66c577518e26ef6e8b9ba61c6 dwm/machines/hp_draftmaster/workflow/hp7596_36x24_lissajous.hpgl
# a10967101d17530bf1941c15f798d0dd7bb88b7aa021beeb44871a20ab567cf2 ezdxf/examples_hpgl2/BF_ISO.plt
# b0f88234c184dc3ca9ad66bb7aec42470c85caa03baacc4f7675180f619e3e89 ezdxf/examples_hpgl2/GB01_Stahlbau.plt
# f5d1b93df5a68fb8504b7423d31314af65ae25a3a532a7dd453b2db25f0232c8 ezdxf/examples_hpgl2/HSG_ISO.plt
# 80b9cdd91110e7c7405a80140a43318507fcc254d7d923973b1d56a874d052c4 ezdxf/integration_tests/data/PLOTFILE.plt
# bd41316c4f385a69f09bc853e70fc18d0bdfb2623f1fae195e02eb8919537719 ghostpcl/pcl/examples/gl-chars.pcl
# b5dc9e26ef15e28f615b3638d67a714660086fbe944c4dec8a4fdb0db8cd22e2 ghostpcl/pcl/examples/grid.pcl
# 8cb4f65aeb530fbcee6dcd51f402547fb4f9eaa60c2d5aefd37ff268f47add33 ghostpcl/pcl/examples/origins.pcl
# fb301a923607fbf5de24d952436b1409e9930ff9c0f6834f9b632f8445537890 ghostpcl/pcl/examples/owl.pcl
# 534bccc78b7483cdf19dc56022596ef953fada01aa0c3010fa92537dff3283a7 hp2xx/hp-tests/286x192.5_lh.hpg
# 0c804555d558af62c48813d8ec7ef01a954f1f36835ab0f340e9d2152d15e245 hp2xx/hp-tests/286x192.5_lq.hpg
# df37c1ab48e1d67c9fd615fbdfbf9a376b31cecd625a315d4ee9a24548f02806 hp2xx/hp-tests/286x192.5_qh.hpg
# c46322493e70449272f3cc3d8137cc386d513daae2334a739c00391f541a56e5 hp2xx/hp-tests/286x192.5_qh2.hpg
# 9bcc8aa04b0f16b6a0e0bd74520e752cbcbd229f64cf65fa791feb0897488955 hp2xx/hp-tests/286x192.5_qq.hpg
# e309ed9828a589c1c877c4e00c6b272da20a7b86b44e8e8313b7858a997b7d32 hp2xx/hp-tests/acad.hp
# 75fd4c5b7681044569f589a37dc6393d53595a81d381da5ee9d49175d3ad308e hp2xx/hp-tests/arcs.hp
# 867908954f51d56792249c87a361826e736a1e8f19b1c4ae05506da154b48677 hp2xx/hp-tests/ascii.hp
# 44aafc229cedb643905cb94f7d57a127222b111fdd4dd218f865f6ff2283787c hp2xx/hp-tests/at.hpg
# c6e1eefe23b22565f7b7e890f4252ddee99fcf2ee7ff0464abf9a205c12247ef hp2xx/hp-tests/bz.hp
# f86904b374f6d1799e0b032914c8f906adeaf19b0d4984c79c9c3917f56208be hp2xx/hp-tests/char7.hp
# 07b96e5e6057a3c5b9463cd3fbb446f3d77229f3b54b93e77599e70408cb6caf hp2xx/hp-tests/charsets.hp
# 465f7f4e86a18ce63977cafecd8217e153f6289d5b98c598066e5a692e85d89b hp2xx/hp-tests/charsize.hp
# 62ce2504bea772c083c6f467fc77f27485a45eaa7da64f63088ae35b38f64e9a hp2xx/hp-tests/ci.hp
# 43afa0769245b6a9ac64167c860389dd17c625606bf81b972faf3b55978c7bbd hp2xx/hp-tests/colors.hp
# bea805c5fdcaf24de547fc8a500b752730ebc14a5aac1df43041f3973b6d7a4e hp2xx/hp-tests/cp.hp
# 2bed5383bdec066cc6287964ebc6547e7e571028e0c0152f63ec3527e633306f hp2xx/hp-tests/di.hp
# 2e3dcdb5369671e733d99e4b8226ec10453ac0be934419336c533684a470389d hp2xx/hp-tests/dt.plt
# 855604f706e3775af2c34a901016885fdb9e7028ddf89d3e3c510d42e0e27d98 hp2xx/hp-tests/dv.hpg
# 902b1155258c14db0940f7b105297ce305070fc8c10e920ba2af997e20a0b29b hp2xx/hp-tests/ew.hpg
# 88c077c2e7e0a17cbe8843d736a333bbbdbd4ede1e67732a9f263ecfce59ac76 hp2xx/hp-tests/fill.plt
# 7c61a3a6ba9a4aa245aa63e375be956a99516e94afc5faf77159082741b385e8 hp2xx/hp-tests/ft.hp
# 6fc130995dd4b46429cf52ebda0c40a1068f433b178e92e31ecf49fa7cdc5538 hp2xx/hp-tests/fwedges.plt
# 32637c7cdbab3115c351cae588327ded6b56dbf492741c6b4547334a74d58b6e hp2xx/hp-tests/inter.hp
# d232cdecab5846e97852063086ec492f3ec8c293ece32a091e77ca3f4a963ff6 hp2xx/hp-tests/iw.hp
# 8ab51f5827a81ab95b8d4e5913092474b5277d1f082d9b79d5081a24e6bea0fb hp2xx/hp-tests/la.hp
# 9709f7fe2adb98115dfc28e8457d31c181bab580509fac80f147274abdcee5e1 hp2xx/hp-tests/lo-slash.hp
# 9b38cdd4f5ec3054209cc868cb2354f40cc773e47f25cd1c0a0c9a318df1436f hp2xx/hp-tests/lo.hp
# b0673e685cb6e23b9b0c4cd5e05dce7c3f9f98ed3dcac605aec049f546a26214 hp2xx/hp-tests/lt.hp
# f821ae7846b4426484901acbafe3568ac7c4b1e8341589622c86d80a1b084639 hp2xx/hp-tests/miter.hp
# 646f2fca4cc14e8e2eedbd2e9eadb3105f720ff27f820161b275d31e031f4208 hp2xx/hp-tests/mycolors.hp
# 528368e4c2d398fffdb180046857d839bdc216354edad7136e73b15b93e25e57 hp2xx/hp-tests/pages.1.hpgl
# 415b7170a722325f58f7e9eebee18ee3cf0de7a9e15ba940399159e7235d0e2a hp2xx/hp-tests/pages.2.hpgl
# 3582017da46f0c30259446cbf593f2221d1bcc3a2b6f276861cf655964a93b3f hp2xx/hp-tests/pages.hp
# ddbc6304ee75756af5ab352dd1003fa101e66ae182ebfec82aafbb2f82ee8aed hp2xx/hp-tests/pages.hpgl
# 19234c56fc2e269eb54b2458ed9e047f45fae896aa531b0133079f304529a7bb hp2xx/hp-tests/pm.hp
# aedbb04fc62ec94c6799ba87b89ba717d55fe79b52556a8bf366fa6e184b7b85 hp2xx/hp-tests/pw.hpg
# 8df2f376e8b524cd5b113c6bcd0dd088df350a199ee4780c32b731ded7e17736 hp2xx/hp-tests/rf.plt
# 0e8c07b00c95789101627c036d68170288c07b53f6c023350ae1c0f8c1af03d0 hp2xx/hp-tests/spectrum.plt
# 9824acda2f141e2babbec2c55429ee5810354973572a4bc934acb1847fa10f9f hp2xx/hp-tests/symbols.hp
# 612deb37af49d2129df0fabb79420ed1101983303f3705512830462433e4671a hp2xx/hp-tests/thick.hp
# d911973b25bf2fb6cc7953110437714efe4245dc6b07980adf406a2571f8759c hp2xx/hp-tests/ul.hp
# 1fcf319df943a4323841f6aa2b34b3df836282ebb80d0f2bab25f7c908e5d0fb hp2xx/hp-tests/win_1.hp
# e5543f047bbfab39dfd1a1b2989deb68c3f20e5352fa871fd1a254a6f5cd4ecf inkscape/tests/data/io/test.hpgl
# 69a0c87e80314e2a939aed49bed99f94bc6896d58d35cac7fb5f0deb1b777a3d inkscape/tests/data/refs/hpgl_output__--force__24__--speed__20__--orientation__90__hpgl_multipen__svg.out.hpgl
# 1f02c39ce7b1d28eb6302c602579fc4736559ed57a4155950b67eb8790efbd89 inkscape/tests/data/refs/hpgl_output__--force__24__--speed__20__--orientation__90__shapes__svg.out.hpgl
# 41ebfd3572737616bc346863a69c6e4eda3913f053f17d257cfcee4085e5e7c3 inkscape/tests/data/refs/hpgl_output__--overcut__0__0__--toolOffset__0__0__--precut__False__--autoAlign__False__hplg_repeated_path_coords__svg.out.hpgl
# e8e3721909029d9435578f2db14af2eeec333382bb3b6e602f47b82f207ec510 inkscape/tests/data/refs/plotter__--serialPort____test__.out.hpgl
# 04e32011378c615866b47ab90d17408f476c97a9357c69cafd13439fc581602b plotter-tools/hpgl/testdata/canonicalize.hpgl
# SHA256 END
