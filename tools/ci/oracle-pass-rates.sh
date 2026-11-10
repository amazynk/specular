#!/usr/bin/env bash
# Per-format oracle pass rates over whole corpora, tracked over time. Runs each decoder's
# oracle test (an `#[ignore]` test that compares our output with a reference tool over its
# corpus), counts files that pass, fail or are skipped, and writes a dated TSV. A drop against
# the committed summary, or against the previous run, is printed and fails with --check.
#
#   tools/ci/oracle-pass-rates.sh [--check] [--update] [--only ID[,ID...]] [--list] [--history]
#
#   --check    exit 1 when a format's pass rate or passed count fell below the committed
#              summary (tools/ci/oracle-pass-rates.tsv)
#   --update   rewrite the committed summary's rows for the runners that ran (review the diff)
#   --only     run only these runner IDs (see --list)
#   --list     print the runners and exit
#   --history  print every dated run's pass rates side by side and exit
#
# Output (never committed): $OUT (default <repo>/target/oracle-rates)/YYYY-MM-DD.tsv, one row
# per runner and format (date, runner, format, files, passed, failed, skipped, pass_rate,
# status), and logs/<runner>.log. A second run on the same day replaces that day's rows for
# the runners it ran.
#
# Counting. A test opts in to per-file counts by printing one line per file:
#     ORACLE<TAB>format<TAB>file<TAB>pass|fail|skip
# (most do; see --list). A runner that
# prints none counts as one unit under its own ID: pass or fail by its test result, "no
# oracle" when its output says the corpus or oracle is missing (it skipped).
#
# Oracles and corpora are what each test documents (fetch scripts under tools/corpus and
# tools/quality; the Python venv from tools/quality/setup-venv.sh). Environment: the usual
# cargo variables (CARGO_TARGET_DIR, RUSTC_WRAPPER, ...); VENV (default
# $CARGO_TARGET_DIR/venv/bin/python, passed as SPECULAR_RAWPY); SPECULAR_QUALITY_CORPUS
# (default $CARGO_TARGET_DIR/corpus) for the ICC and RAW-camera sets, and
# SPECULAR_ORACLE4_CORPUS (default <that>/oracle4) for round 4's runners;
# SPECULAR_FIDELITY5_CORPUS (default $CARGO_TARGET_DIR/corpus/fidelity5) for fidelity round 5;
# SPECULAR_FIDELITY7_CORPUS (default $CARGO_TARGET_DIR/corpus/fidelity7) for round 7 (video
# decoders and JPEG XL CMYK, tools/corpus/fetch-fidelity7.sh).
# Correctness only: no timings are recorded. Oracles stay out of the shipped build (dev-dependencies and Python).
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
cd "$ROOT"
SUMMARY=$ROOT/tools/ci/oracle-pass-rates.tsv
TARGET=${CARGO_TARGET_DIR:-$ROOT/target}
OUT=${OUT:-$ROOT/target/oracle-rates}
VENV=${VENV:-$TARGET/venv/bin/python}
QCORPUS=${SPECULAR_QUALITY_CORPUS:-$TARGET/corpus}
O4=${SPECULAR_ORACLE4_CORPUS:-$QCORPUS/oracle4}
DATE=$(date +%F)

# id|package|cargo test target and filter (before --)|corpus it needs ("-": none known; the
# test then decides, and a run that says it skipped counts as "no oracle")[|test flag: default
# --ignored (oracle tests are #[ignore]d); --include-ignored for an oracle that is a plain test]
RUNNERS=(
  "icc|specular-color|--test icc_conformance|$QCORPUS/icc"
  "raw-cameras|specular-raw|--test raw_cameras|${SPECULAR_RAW_CAMERAS:-$QCORPUS/raw-cameras}"
  "raw-broad|specular-raw|--test raw_cameras|${SPECULAR_RAW_BROAD:-$QCORPUS/raw-cameras-broad}"
  "raw-more|specular-raw|--test raw_more|${SPECULAR_RAW_MORE_CORPUS:-tools/corpus/raw-more}"
  "raw-rest|specular-raw|--test rawrest|${SPECULAR_RAWREST_CORPUS:-tools/corpus/rawrest}"
  'dng-profile|specular-raw|--test dng_profile|tools/corpus/large/raw'
  "tiff-variants|specular-tiff|--test oracle|${SPECULAR_TIFF_VARIANTS_CORPUS:-$QCORPUS/tiff-variants}"
  "hevc|specular-hevc|--test conformance conformance|${SPECULAR_HEVC_CORPUS:-tools/corpus/conformance/hevc}|--include-ignored --exact"
  'jp2-jpx|specular-jp2|--test jpx oracle_|-|--include-ignored'
  'jpeg-lossless-arith|specular-jpeg|--lib arith::lossless::tests::lossless_arith_matrix|-|--include-ignored'
  "gainmap|specular-decode|--test gainmap corpus_|${SPECULAR_CONFORMANCE_CORPUS:-tools/corpus/conformance}|--include-ignored"
  'dicom-display|specular-render|--lib renderer::medical::tests::gsdf_display|-|--include-ignored'
  "film|specular-film|--test oracle corpus_against_oracles|${SPECULAR_RASTER_CORPUS_FILM:-tools/corpus/raster/film}"
  "legacy2|specular-legacy2|--test oracle corpus_matches_imagemagick|${SPECULAR_RASTER_CORPUS_LEGACY2:-tools/corpus/raster/legacy2}"
  "flif|specular-flif|--test oracle reference_decoder_matches|${SPECULAR_FLIF_CORPUS:-tools/corpus/editors/flif/flif}|--include-ignored"
  "jbig|specular-jbig|--test oracle|${SPECULAR_RASTER_CORPUS_JBIG:-tools/corpus/raster/jbig/files}"
  "jbig2|specular-jbig2|--test oracle|${SPECULAR_RASTER_CORPUS_JBIG2:-tools/corpus/raster/jbig2}"
  "wsq|specular-wsq|--test oracle|${SPECULAR_RASTER_CORPUS_WSQ:-tools/corpus/raster/wsq}"
  "wsi|specular-wsi|--test microscopy corpus_matches|${SPECULAR_MICROSCOPY_CORPUS:-tools/corpus/microscopy}|--include-ignored"
  "em|specular-em|--test corpus corpus_matches|${SPECULAR_EM_CORPUS:-tools/corpus/em}|--include-ignored"
  "spm|specular-spm|--test corpus corpus_matches|${SPECULAR_EM_CORPUS:-tools/corpus/em}|--include-ignored"
  "spm-afm|specular-spm|--test corpus_afm corpus_matches|${SPECULAR_MATSCI_CORPUS:-tools/corpus/matsci}/afm|--include-ignored"
  "asdf|specular-asdf|--test oracle corpus_matches|${SPECULAR_ASDF_DIR:-tools/corpus/asdf}|--include-ignored"
  "jcamp|specular-jcamp|--test oracle|${SPECULAR_JCAMP_EXPECT:-tools/corpus/pcb-spectra/jcamp-expect}"
  "nmr|specular-nmr|--test oracle|${SPECULAR_NMR_CORPUS:-tools/corpus/pcb-spectra/nmr}"
  "mzml|specular-mzml|--test oracle|${SPECULAR_MZML_CORPUS:-tools/corpus/pcb-spectra/mzml}"
  "gds|specular-gds|--test oracle|${SPECULAR_GDS_CORPUS:-tools/corpus/gdsii/files}"
  "pcb|specular-pcb|--test oracle corpus_matches_gerbonara|${SPECULAR_PCB_CORPUS:-tools/corpus/pcb-spectra/gerber}"
  "kicad|specular-kicad|--test oracle|${SPECULAR_KICAD_CORPUS:-tools/corpus/pcb-spectra/kicad}"
  "cad|specular-cad|--test corpus|${SPECULAR_DXF_CORPUS:-tools/corpus/dxf}"
  "hpgl|specular-cad|--test hpgl|${SPECULAR_HPGL_CORPUS:-tools/corpus/hpgl}"
  "points|specular-points|--test corpus|${SPECULAR_POINTS_CORPUS:-tools/corpus/points}|--include-ignored"
  "mesh-x3d|specular-mesh|--lib x3d::tests|${SPECULAR_MODELS2_CORPUS:-tools/corpus/models2}|--include-ignored"
  "mesh-ifc|specular-mesh|--lib ifc::tests::corpus_against|${SPECULAR_MODELS2_CORPUS:-tools/corpus/models2}|--include-ignored"
  'terrain|specular-geodesy|--test terrain|-|--include-ignored'
  'archive|specular-archive|--lib codec::oracle|-|--include-ignored --skip speed'
  'fft|specular-fft|--test numpy|-|--include-ignored'
  'video-encode|specular-video|--test video_encode|-|--include-ignored --skip bench'
  'h264enc|specular-h264enc|--test oracle|-|--include-ignored'
  "h264-extended|specular-h264|--test conformance conformance|${SPECULAR_H264X_CORPUS:-$TARGET/corpus/h264x}/jvt|--include-ignored --exact"
  "dv|specular-dv|--test oracle|${SPECULAR_DV_CORPUS:-$TARGET/corpus/dv}"
  "vc1|specular-vc1|--test oracle|${SPECULAR_VC1_CORPUS:-$TARGET/corpus/vc1/fate}"
  "vp3|specular-theora|--test vp3|${SPECULAR_VPX3_CORPUS:-$TARGET/corpus/vpx3}/vp3"
  "sorenson|specular-mpeg4|--test sorenson|${SPECULAR_VPX3_CORPUS:-$TARGET/corpus/vpx3}/flv1"
  "vp9-vectors|specular-vp9|--test vectors|${SPECULAR_VP9_VECTORS:-$TARGET/corpus/vpx3/vp9}|--include-ignored"
  "video-round4|specular-video|--test round4 corpus_matches_ffmpeg|${SPECULAR_ROUND4_CORPUS:-$TARGET/corpus/round4}|--include-ignored"
  # Oracles, round 4 (tools/corpus/fetch-oracle4.sh builds $O4).
  "radiance|specular-radiance|--test oracle|$O4/radiance"
  "icns|specular-icns|--test oracle|$O4/icns"
  "xisf|specular-xisf|--test oracle|$O4/xisf"
  "tdms|specular-tdms|--test oracle|$O4/tdms"
  "fcs|specular-fcs|--test oracle|$O4/fcs"
  "cfb|specular-cfb|--test oracle|$O4/cfb"
  "planetary|specular-planetary|--test oracle|$O4/planetary"
  "ser|specular-ser|--test oracle|$O4/ser"
  "aec|specular-aec|--test oracle|$O4/aec"
  "cdr|specular-cdr|--test oracle|$O4/cdr"
  "clip|specular-clip|--test oracle|$O4/clip"
  "spectra|specular-spectra|--lib chart::tests::contours_match_scikit_image|$O4/spectra"
  "iff|specular-iff|--test cycling_oracle|$O4/iff"
  "legacy2-real|specular-legacy2|--test oracle oracle4_matches_imagemagick|$O4/legacy2"
  "film-luma|specular-film|--test oracle oracle4_luma_against_oracles|$O4/film"
  'video-sound3|specular-video|--test sound3|-|--include-ignored --skip decode_speed'
  'mpa|specular-mpa|--test fixtures layer3_matches_libavcodec|-|--include-ignored'
  "mp3-compliance|specular-mpa|--test corpus|${SPECULAR_MP3_CORPUS:-$TARGET/corpus/mp3}"
  "fidelity|specular-convert|--test fidelity|${SPECULAR_FIDELITY_CORPUS:-$TARGET/corpus/fidelity4}"
  "fidelity5|specular-convert|--test fidelity|${SPECULAR_FIDELITY5_CORPUS:-$TARGET/corpus/fidelity5}"
  "fidelity5-psd-compose|specular-psd|--test compose_oracle|${SPECULAR_FIDELITY5_CORPUS:-$TARGET/corpus/fidelity5}"
  "fidelity7|specular-video|--test fidelity7|${SPECULAR_FIDELITY7_CORPUS:-$TARGET/corpus/fidelity7}"
  "fidelity7-jxl|specular-decode|--test jxl_cmyk|${SPECULAR_FIDELITY7_CORPUS:-$TARGET/corpus/fidelity7}"
)

CHECK=0 UPDATE=0 ONLY=""
while [[ $# -gt 0 ]]; do
  case $1 in
    --check) CHECK=1 ;;
    --update) UPDATE=1 ;;
    --only) shift; ONLY=",${1:-},"; ;;
    --list) printf '%s\n' "${RUNNERS[@]}" | tr '|' '\t'; exit 0 ;;
    --history)
      python3 -I - "$OUT" <<'EOF'
import csv, glob, os, sys
runs = sorted(glob.glob(os.path.join(sys.argv[1], "????-??-??.tsv")))
table, dates = {}, []
for f in runs:
    d = os.path.basename(f)[:-4]
    dates.append(d)
    for r in csv.DictReader(open(f), delimiter="\t"):
        n = int(r["passed"]) + int(r["failed"])
        table.setdefault((r["runner"], r["format"]), {})[d] = f'{r["passed"]}/{n}' if n else r["status"]
print("\t".join(["runner", "format"] + dates))
for k in sorted(table):
    print("\t".join(list(k) + [table[k].get(d, "") for d in dates]))
EOF
      exit 0 ;;
    -h|--help) sed -n '2,34p' "$0"; exit 0 ;;
    *) echo "oracle-pass-rates: unknown option $1" >&2; exit 2 ;;
  esac
  shift
done

mkdir -p "$OUT/logs"
rows=$(mktemp -p "$OUT" rows.XXXXXX)
trap 'rm -f "$rows"' EXIT
for e in "${RUNNERS[@]}"; do
  IFS='|' read -r id pkg args needs flag <<< "$e"
  flag=${flag:---ignored}
  [[ -z $ONLY || $ONLY == *",$id,"* ]] || continue
  log=$OUT/logs/$id.log
  if [[ $needs != - ]] && [[ -z $(ls -A "$needs" 2>/dev/null) ]]; then
    echo "== $id: no corpus at $needs" >&2
    printf '%s\t%s\t%s\t0\t0\t0\t0\t\tno-corpus\n' "$DATE" "$id" "$id" >> "$rows"
    continue
  fi
  echo "== $id ($pkg $args)" >&2
  # shellcheck disable=SC2086 # args and flag are word lists on purpose
  if SPECULAR_RAWPY=${SPECULAR_RAWPY:-$VENV} SPECULAR_QUALITY_CORPUS=$QCORPUS \
     SPECULAR_ORACLE4_CORPUS=$O4 \
     SPECULAR_RAW_CAMERAS=$([[ $id == raw-broad ]] && echo "$needs" || echo "${SPECULAR_RAW_CAMERAS:-$QCORPUS/raw-cameras}") \
     SPECULAR_FIDELITY_CORPUS=$([[ $id == fidelity* ]] && echo "$needs" || echo "${SPECULAR_FIDELITY_CORPUS:-$TARGET/corpus/fidelity4}") \
     SPECULAR_PCB_EXPECT=${SPECULAR_PCB_EXPECT:-${SPECULAR_PCB_CORPUS:-tools/corpus/pcb-spectra/gerber}-expect} \
     SPECULAR_H264_CORPUS=${SPECULAR_H264X_CORPUS:-$TARGET/corpus/h264x}/jvt \
     SPECULAR_DV_CORPUS=${SPECULAR_DV_CORPUS:-$TARGET/corpus/dv} \
     SPECULAR_VC1_CORPUS=${SPECULAR_VC1_CORPUS:-$TARGET/corpus/vc1/fate} \
     SPECULAR_VPX3_CORPUS=${SPECULAR_VPX3_CORPUS:-$TARGET/corpus/vpx3} \
     SPECULAR_VP9_VECTORS=${SPECULAR_VP9_VECTORS:-$TARGET/corpus/vpx3/vp9} \
     SPECULAR_ROUND4_CORPUS=${SPECULAR_ROUND4_CORPUS:-$TARGET/corpus/round4} \
     cargo test --offline -q -p "$pkg" $args -- $flag --nocapture --test-threads 1 \
     > "$log" 2>&1; then status=ok; else status=failed; fi
  python3 -I - "$id" "$status" "$log" "$DATE" >> "$rows" <<'EOF'
import re, sys
rid, status, log, date = sys.argv[1:5]
text = open(log, errors="replace").read()
per = {}
# Not anchored: libtest prints "test name ... " in front of a test's first output line.
for m in re.finditer(r"ORACLE\t([^\t\n]+)\t([^\t\n]*)\t(pass|fail|skip)[ \t]*$", text, re.M):
    per.setdefault(m.group(1), {"pass": 0, "fail": 0, "skip": 0})[m.group(3)] += 1
if not per:
    # One unit under the runner's own ID (a per-file runner that printed no line had no files).
    missing = re.search(r"skipp|unset|not found|no corpus|missing|no adapter|no GPU", text, re.I)
    ran = re.search(r"test result: (ok|FAILED)\. [1-9]", text)
    if status == "ok" and (missing or not ran):
        per[rid] = {"pass": 0, "fail": 0, "skip": 1}
        status = "no-oracle"
    else:
        per[rid] = {"pass": int(status == "ok"), "fail": int(status != "ok"), "skip": 0}
for fmt, c in sorted(per.items()):
    n = c["pass"] + c["fail"]
    rate = f'{c["pass"] / n:.4f}' if n else ""
    print("\t".join([date, rid, fmt, str(n + c["skip"]), str(c["pass"]), str(c["fail"]),
                     str(c["skip"]), rate, status]))
EOF
done

python3 -I - "$OUT" "$DATE" "$rows" "$SUMMARY" "$CHECK" "$UPDATE" <<'EOF'
import csv, glob, os, sys
out, date, rows_path, summary, check, update = sys.argv[1:7]
head = ["date", "runner", "format", "files", "passed", "failed", "skipped", "pass_rate", "status"]
new = [dict(zip(head, l.rstrip("\n").split("\t"))) for l in open(rows_path) if l.strip()]
ran = {r["runner"] for r in new}
day = os.path.join(out, f"{date}.tsv")
old_day = list(csv.DictReader(open(day), delimiter="\t")) if os.path.exists(day) else []
merged = [r for r in old_day if r["runner"] not in ran] + new
merged.sort(key=lambda r: (r["runner"], r["format"]))
with open(day, "w") as f:
    f.write("\t".join(head) + "\n")
    for r in merged:
        f.write("\t".join(r[h] for h in head) + "\n")

prev = sorted(p for p in glob.glob(os.path.join(out, "????-??-??.tsv")) if os.path.basename(p) < f"{date}.tsv")
before = {}
if prev:
    for r in csv.DictReader(open(prev[-1]), delimiter="\t"):
        before[(r["runner"], r["format"])] = r
base = {}
if os.path.exists(summary):
    for r in csv.DictReader((l for l in open(summary) if not l.startswith("#")), delimiter="\t"):
        base[(r["runner"], r["format"])] = r

def rate(r):
    return float(r["pass_rate"]) if r.get("pass_rate") else None

drops = []
print(f"{'runner':<20} {'format':<14} {'passed':>12} {'rate':>8}  vs summary / previous run")
for r in new:
    k = (r["runner"], r["format"])
    n = int(r["passed"]) + int(r["failed"])
    notes = []
    for label, ref in (("summary", base.get(k)), ("previous", before.get(k))):
        if not ref or rate(r) is None or not ref.get("pass_rate"):
            continue
        dr = rate(r) - float(ref["pass_rate"])
        dp = int(r["passed"]) - int(ref["passed"])
        if dr < -1e-9 or dp < 0:
            notes.append(f"DROP vs {label}: {ref['passed']}/{int(ref['passed']) + int(ref.get('failed') or 0)} -> {r['passed']}/{n}")
            if label == "summary":
                drops.append(k)
        elif dr > 1e-9 or dp > 0:
            notes.append(f"up vs {label} ({ref['passed']} -> {r['passed']})")
    shown = f"{r['passed']}/{n}" if n else f"({r['status']})"
    print(f"{r['runner']:<20} {r['format']:<14} {shown:>12} {r['pass_rate'] or '-':>8}  {'; '.join(notes)}")
print(f"written: {day}")

if update == "1":
    keep = {k: v for k, v in base.items() if k[0] not in ran}
    for r in new:
        if r["pass_rate"]:
            keep[(r["runner"], r["format"])] = {
                "runner": r["runner"], "format": r["format"], "files": r["files"],
                "passed": r["passed"], "failed": r["failed"], "pass_rate": r["pass_rate"],
                "date": date}
    cols = ["runner", "format", "files", "passed", "failed", "pass_rate", "date"]
    with open(summary, "w") as f:
        f.write("# Oracle pass rates, committed summary (tools/ci/oracle-pass-rates.sh --update).\n")
        f.write("# A drop below these rows fails --check. Dated per-run TSVs live under target/.\n")
        f.write("\t".join(cols) + "\n")
        for k in sorted(keep):
            f.write("\t".join(str(keep[k][c]) for c in cols) + "\n")
    print(f"updated: {summary}")
if check == "1" and drops:
    print(f"pass rate dropped for: {', '.join('/'.join(k) for k in drops)}", file=sys.stderr)
    sys.exit(1)
EOF
