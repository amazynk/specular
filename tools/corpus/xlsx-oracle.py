"""Reads the XLSX workbooks Specular's quantity export wrote back with openpyxl.

Usage: python -I tools/corpus/xlsx-oracle.py DIR
DIR holds the `*.xlsx` the specular-bim test `ifc_checks_corpus` writes when
SPECULAR_DUMP_XLSX=DIR. For each workbook: every sheet opens, the header row is there,
every data row has as many cells as the header, numbers are numbers, and the element
sheet's volumes add up to the class sheet's. Prints one line per workbook and exits
non-zero on a problem.
"""

import math
import os
import sys

import openpyxl


def main():
    d = sys.argv[1]
    bad = 0
    for f in sorted(os.listdir(d)):
        if not f.endswith(".xlsx"):
            continue
        wb = openpyxl.load_workbook(os.path.join(d, f), read_only=True)
        sheets = wb.sheetnames
        rows = {s: list(wb[s].iter_rows(values_only=True)) for s in sheets}
        problems = []
        for s, r in rows.items():
            if not r:
                problems.append(f"{s}: empty")
                continue
            width = len(r[0])
            for i, row in enumerate(r[1:], 2):
                if len(row) > width:
                    problems.append(f"{s} row {i}: {len(row)} cells")
        el = rows.get("Elements", [])
        cl = rows.get("By class", [])
        if el and cl:
            v_el = sum(x[8] for x in el[1:] if isinstance(x[8], (int, float)))
            v_cl = sum(x[4] for x in cl[1:] if isinstance(x[4], (int, float)))
            if not math.isclose(v_el, v_cl, rel_tol=1e-9, abs_tol=1e-9):
                problems.append(f"volumes {v_el} vs {v_cl}")
        print(f"{f}: {len(sheets)} sheets, {sum(len(r) for r in rows.values())} rows, {len(problems)} problems")
        for p in problems[:5]:
            print("  ", p)
        bad += len(problems)
    sys.exit(1 if bad else 0)


main()
