"""Second oracle for the IDS test cases: IfcOpenShell's ifctester.

Usage: python -I tools/corpus/ids-oracle.py CORPUS [OUT.tsv]
CORPUS is what tools/corpus/fetch-ids.sh fetched. Writes one line per case,
"<facet>/<name>\t<expected pass|fail>\t<ifctester pass|fail|error>", to OUT.tsv
(default CORPUS/ifctester.tsv), read by the specular-bim test `ids_test_cases`, and prints
how often ifctester agrees with the expected result.
"""

import os
import sys

import ifcopenshell
from ifctester import ids


def main():
    corpus = sys.argv[1]
    out = sys.argv[2] if len(sys.argv) > 2 else os.path.join(corpus, "ifctester.tsv")
    rows = []
    for facet in sorted(os.listdir(corpus)):
        d = os.path.join(corpus, facet)
        if not os.path.isdir(d):
            continue
        for f in sorted(os.listdir(d)):
            if not f.endswith(".ids"):
                continue
            name = f[:-4]
            want = "pass" if name.startswith("pass-") else "fail"
            ifc = os.path.join(d, name + ".ifc")
            if not os.path.exists(ifc):
                continue
            try:
                spec = ids.open(os.path.join(d, f))
                model = ifcopenshell.open(ifc)
                spec.validate(model)
                got = "pass" if all(s.status for s in spec.specifications) else "fail"
            except Exception:  # invalid IDS files are expected to fail
                got = "error"
            rows.append((f"{facet}/{name}", want, got))
    with open(out, "w") as o:
        for r in rows:
            o.write("\t".join(r) + "\n")
    agree = sum(1 for _, w, g in rows if w == g or (w == "fail" and g == "error"))
    print(f"ifctester agrees on {agree} of {len(rows)}")


main()
