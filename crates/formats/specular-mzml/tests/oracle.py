"""pyteomics' reading of every .mzML / .mzXML under a folder, for tests/oracle.rs.

  python -I oracle.py CORPUS_DIR EXPECT_DIR

For each file it writes EXPECT_DIR/<relative path>.expect: the kind, the counts, then per
spectrum (S) and chromatogram (C) its id, MS level, retention time in minutes, and each
array's length and SHA-256 over its values as little-endian float64. A file pyteomics cannot
read gets a line "E <reason>". Needs pyteomics, psims, pynumpress and numpy.
"""
import hashlib
import os
import subprocess
import sys
import warnings

import numpy as np
from pyteomics import mzml, mzxml

warnings.simplefilter("ignore")


def digest(a):
    if a is None:
        return "-1\t-"
    v = np.asarray(a, dtype="<f8")
    return f"{len(v)}\t{hashlib.sha256(v.tobytes()).hexdigest()[:16]}"


def rt_minutes(s):
    try:
        scan = s["scanList"]["scan"][0]
        v = scan["scan start time"]
    except (KeyError, IndexError, TypeError):
        # mzML 0.99: spectrumDescription/scan with "scan time".
        try:
            scan = s["spectrumDescription"]["scan"][0]
            v = scan.get("scan time", scan.get("scan start time"))
        except (KeyError, IndexError, TypeError, AttributeError):
            return None
        if v is None:
            return None
    unit = getattr(v, "unit_info", None) or ""
    f = float(v)
    if unit in ("second", "s"):
        return f / 60.0
    if unit == "hour":
        return f * 60.0
    return f


def fmt(v):
    return "-" if v is None else repr(float(v))


def mzml_lines(path):
    out = []
    with mzml.MzML(path, decode_binary=True, use_index=False) as r:
        sp = list(r.iterfind("spectrum"))
        r.reset()
        ch = list(r.iterfind("chromatogram"))
    out.append("kind mzml")
    out.append(f"spectra {len(sp)}")
    out.append(f"chromatograms {len(ch)}")
    for i, s in enumerate(sp):
        lvl = s.get("ms level", s.get("msLevel"))
        x = s.get("m/z array")
        if x is None:
            x = s.get("wavelength array")
        out.append("\t".join(["S", str(i), s.get("id", ""), "-" if lvl is None else str(int(lvl)),
                              fmt(rt_minutes(s)), digest(x), digest(s.get("intensity array"))]))
    for i, c in enumerate(ch):
        out.append("\t".join(["C", str(i), c.get("id", ""), "-", "-", digest(c.get("time array")),
                              digest(c.get("intensity array"))]))
    return out


def mzxml_lines(path):
    out = []
    with mzxml.MzXML(path, decode_binary=True, use_index=False) as r:
        sp = list(r.iterfind("scan"))
    out.append("kind mzxml")
    out.append(f"spectra {len(sp)}")
    out.append("chromatograms 0")
    for i, s in enumerate(sp):
        lvl = s.get("msLevel")
        rt = s.get("retentionTime")
        out.append("\t".join(["S", str(i), str(s.get("num", "")), "-" if lvl is None else str(int(lvl)),
                              fmt(rt), digest(s.get("m/z array")), digest(s.get("intensity array"))]))
    return out


def one(path, out):
    low = path.lower()
    try:
        lines = mzxml_lines(path) if low.endswith(".mzxml") else mzml_lines(path)
    except Exception as e:  # noqa: BLE001 - any failure is recorded
        lines = ["E " + str(e).replace("\n", " ")[:200]]
    with open(out, "w", encoding="utf-8") as fh:
        fh.write("\n".join(lines) + "\n")


def main():
    if sys.argv[1] == "--many":
        # A worker: input/output pairs, one per line.
        for line in open(sys.argv[2], encoding="utf-8"):
            path, out = line.rstrip("\n").split("\t")
            one(path, out)
        return
    src, dst = sys.argv[1], sys.argv[2]
    todo = []
    for root, _, files in os.walk(src):
        for f in sorted(files):
            low = f.lower()
            if not (low.endswith(".mzml") or low.endswith(".mzxml")):
                continue
            path = os.path.join(root, f)
            out = os.path.join(dst, os.path.relpath(path, src) + ".expect")
            os.makedirs(os.path.dirname(out), exist_ok=True)
            if os.path.exists(out):
                os.remove(out)
            todo.append((path, out))
    n = len(todo)
    # Workers handle the files in order; pynumpress aborts the process on corrupt data, so
    # the first file left without an expectation after a crash is marked failed and the rest
    # go to a new worker.
    lst = os.path.join(dst, ".todo")
    while todo:
        with open(lst, "w", encoding="utf-8") as fh:
            fh.write("".join(f"{p}\t{o}\n" for p, o in todo))
        r = subprocess.run([sys.executable, "-I", os.path.abspath(__file__), "--many", lst],
                           capture_output=True, text=True)
        rest = [t for t in todo if not os.path.exists(t[1])]
        if rest and r.returncode != 0:
            with open(rest[0][1], "w", encoding="utf-8") as fh:
                fh.write(f"E oracle process failed ({r.returncode}): {r.stderr.strip()[-150:]}\n")
            rest = rest[1:]
        elif rest:
            break
        todo = rest
    os.remove(lst)
    print(f"{n} files")


if __name__ == "__main__":
    main()
