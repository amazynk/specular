#!/usr/bin/env python3
"""Benchmark gate: compare two specular-bench decode JSON files.

    tools/release/bench-compare.py BASELINE.json CURRENT.json [--threshold 0.10]
        [--decoders specular-] [--min-ms 2.0] [--markdown OUT.md]

For each (file, decoder) pair present in both runs whose decoder name starts with one of the
--decoders prefixes (default: specular's own rows; competitor rows are context, not our
regressions), the current median is compared with the baseline median. The gate fails (exit 1)
if any pair is more than --threshold slower.

Noise control:
  * medians below --min-ms are ignored (timer and scheduler noise dominate);
  * if the baseline and current runs were taken on different CPUs (system.cpu), the comparison is
    reported but never fails, because the numbers are not comparable;
  * a pair only fails if the *fastest* current sample is also slower than the threshold allows
    relative to the baseline median, so a single noisy median does not fail the gate.
"""

import argparse
import json
import sys


def load(path):
    with open(path, encoding="utf-8") as f:
        d = json.load(f)
    rows = {}
    for r in d.get("results", []):
        if r.get("error") or r.get("median_ms") is None:
            continue
        rows[(r["file"], r["decoder"])] = r
    return d, rows


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("baseline")
    ap.add_argument("current")
    ap.add_argument("--threshold", type=float, default=0.10)
    ap.add_argument("--decoders", default="specular-", help="comma-separated decoder name prefixes to gate")
    ap.add_argument("--min-ms", type=float, default=2.0)
    ap.add_argument("--markdown", help="also write the table to this file (e.g. $GITHUB_STEP_SUMMARY)")
    a = ap.parse_args()

    bd, base = load(a.baseline)
    cd, cur = load(a.current)
    prefixes = tuple(p for p in a.decoders.split(",") if p)
    bcpu = bd.get("system", {}).get("cpu")
    ccpu = cd.get("system", {}).get("cpu")
    comparable = bcpu == ccpu
    lines = [
        f"### Decode benchmark vs baseline (gate: +{a.threshold:.0%})",
        "",
        f"Baseline: {bd.get('system', {}).get('date', '?')} on {bcpu}; current: {cd.get('system', {}).get('date', '?')} on {ccpu}.",
        "",
    ]
    if not comparable:
        lines += ["**Different CPUs: report only, the gate cannot fail.**", ""]
    lines += ["| File | Decoder | Baseline ms | Current ms | Change | |", "|---|---|---:|---:|---:|---|"]

    failures = []
    for key in sorted(set(base) & set(cur)):
        f, dec = key
        if not dec.startswith(prefixes):
            continue
        b, c = base[key]["median_ms"], cur[key]["median_ms"]
        if b < a.min_ms and c < a.min_ms:
            continue
        change = c / b - 1.0
        best = min(cur[key].get("samples_ms") or [c])
        status = ""
        if change > a.threshold and best / b - 1.0 > a.threshold:
            status = "REGRESSION" if comparable else "slower (not comparable)"
            if comparable:
                failures.append((f, dec, change))
        elif change < -a.threshold:
            status = "faster"
        lines.append(f"| {f} | {dec} | {b:.1f} | {c:.1f} | {change:+.1%} | {status} |")

    missing = sorted(k for k in base if k not in cur and k[1].startswith(prefixes))
    if missing:
        lines += ["", "Rows in the baseline but not in this run: " + ", ".join(f"{f} ({d})" for f, d in missing)]
    lines += ["", f"{len(failures)} regression(s) over {a.threshold:.0%}."]
    text = "\n".join(lines) + "\n"
    print(text)
    if a.markdown:
        with open(a.markdown, "a", encoding="utf-8") as out:
            out.write(text)
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
