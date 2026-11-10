#!/usr/bin/env python3
"""Scores line recognition: TRUTH.tsv (name, text) against one or more result TSVs (name or
name.pgm/.png, text[, confidence]). Prints exact lines and 1 − character error rate per
result, and how often two results agree exactly.

Usage: python -I tools/corpus/ocr-lines-score.py TRUTH.tsv OURS.tsv [REF.tsv]
"""

import sys


def load(path):
    out = {}
    with open(path, encoding="utf-8") as f:
        for line in f:
            p = line.rstrip("\n").split("\t")
            if len(p) >= 2:
                out[p[0].rsplit(".", 1)[0] if p[0].endswith((".png", ".pgm")) else p[0]] = p[1].strip()
    return out


def lev(a, b):
    prev = list(range(len(b) + 1))
    for i, ca in enumerate(a, 1):
        cur = [i]
        for j, cb in enumerate(b, 1):
            cur.append(min(prev[j] + 1, cur[j - 1] + 1, prev[j - 1] + (ca != cb)))
        prev = cur
    return prev[-1]


def main():
    if len(sys.argv) < 3:
        sys.exit(__doc__)
    truth = load(sys.argv[1])
    results = [(p, load(p)) for p in sys.argv[2:]]
    for path, r in results:
        exact = sum(1 for k, t in truth.items() if r.get(k, "") == t)
        chars = sum(len(t) for t in truth.values())
        errs = sum(lev(r.get(k, ""), t) for k, t in truth.items())
        print(f"{path}: exact {exact}/{len(truth)}, 1-CER {1 - errs / chars:.4f}")
    if len(results) == 2:
        a, b = results[0][1], results[1][1]
        same = sum(1 for k in truth if a.get(k, "") == b.get(k, ""))
        print(f"agree exactly: {same}/{len(truth)}")


if __name__ == "__main__":
    main()
