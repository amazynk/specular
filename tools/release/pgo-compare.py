#!/usr/bin/env python3
"""Compare specular-jpeg `bench-jpeg` example binaries (e.g. plain release vs PGO vs PGO+BOLT).

    tools/release/pgo-compare.py --rounds 5 plain=PATH pgo=PATH [bolt=PATH] -- FILE...

Runs every binary on every file, alternating the binaries within each round so that machine
load drifts hit all of them alike. Each run is `BENCH_RUNS=$BENCH_RUNS BENCH_NO_EXTERNAL=1 bench
FILE` (best-of-N inside the run); the table reports the median over rounds of those numbers, and
the ratio to the first binary (< 1.00 = faster). The 1-minute load average is printed per round.
"""

import argparse
import os
import re
import statistics
import subprocess
import sys

ROW = re.compile(r"^\s+(Full|Half|Quarter|Eighth)\s+([\d.]+)\s+([\d.]+)")
INDEX = re.compile(r"^\s+build_index \(auto\)\s+([\d.]+) ms")
REGION = re.compile(r"^\s+region 512² Full (Auto|Single): with index\s+([\d.]+) ms")


def parse(out):
    m = {}
    for line in out.splitlines():
        if r := ROW.match(line):
            m[f"{r[1].lower()} auto"] = float(r[2])
            m[f"{r[1].lower()} 1t"] = float(r[3])
        elif r := INDEX.match(line):
            m["build_index"] = float(r[1])
        elif r := REGION.match(line):
            m[f"region {r[1].lower()}"] = float(r[2])
    return m


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--rounds", type=int, default=5)
    ap.add_argument("--bench-runs", default="5")
    ap.add_argument("bins", nargs="+", help="label=path ..., then -- and the files")
    argv = sys.argv[1:]
    if "--" not in argv:
        ap.error("missing -- before the files")
    i = argv.index("--")
    a, files = ap.parse_args(argv[:i]), argv[i + 1 :]
    if not files:
        ap.error("no files given")
    bins = [b.split("=", 1) for b in a.bins]
    env = dict(os.environ, BENCH_RUNS=a.bench_runs, BENCH_NO_EXTERNAL="1")
    data = {}  # (file, metric, label) -> [values]
    for rnd in range(a.rounds):
        print(f"round {rnd + 1}/{a.rounds}, load {os.getloadavg()[0]:.1f}", file=sys.stderr, flush=True)
        for f in files:
            order = bins if rnd % 2 == 0 else bins[::-1]
            for label, path in order:
                out = subprocess.run([path, f], env=env, capture_output=True, text=True, check=True).stdout
                for k, v in parse(out).items():
                    data.setdefault((f, k, label), []).append(v)

    labels = [b[0] for b in bins]
    print(f"| file | metric | {' | '.join(labels)} | " + " | ".join(f"{l}/{labels[0]}" for l in labels[1:]) + " |")
    print("|---|---|" + "---:|" * (len(labels) * 2 - 1))
    ratios = {l: [] for l in labels[1:]}
    for f in files:
        metrics = sorted({k for (ff, k, _) in data if ff == f}, key=lambda k: list(data).index((f, k, labels[0])))
        for k in metrics:
            med = {l: statistics.median(data[(f, k, l)]) for l in labels if (f, k, l) in data}
            if len(med) != len(labels):
                continue
            r = [med[l] / med[labels[0]] for l in labels[1:]]
            for l, x in zip(labels[1:], r):
                ratios[l].append(x)
            print(
                f"| {os.path.basename(f)} | {k} | "
                + " | ".join(f"{med[l]:.1f}" for l in labels)
                + " | "
                + " | ".join(f"{x:.3f}" for x in r)
                + " |"
            )
    for l, rs in ratios.items():
        if rs:
            g = statistics.geometric_mean(rs)
            print(f"\n{l} vs {labels[0]}: geometric mean ratio {g:.3f} over {len(rs)} metrics "
                  f"(min {min(rs):.3f}, max {max(rs):.3f})")


if __name__ == "__main__":
    main()
