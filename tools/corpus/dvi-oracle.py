#!/usr/bin/env python3
"""DVI oracle for specular-dvi: typesets two small plain TeX files and writes, per file,
the DVI, the TFM files it uses, and dvitype's position of every character and rule
(`<page> c <code> <h> <v>` / `<page> r <h> <v> <width> <height>`).

    python3 -I tools/corpus/dvi-oracle.py $CARGO_TARGET_DIR/corpus/dvi
    SPECULAR_DVI_CORPUS=$CARGO_TARGET_DIR/corpus/dvi \
        cargo test -p specular-dvi --test pages -- --ignored --nocapture
"""
import os
import re
import shutil
import subprocess
import sys

DOCS = {
    "plain": r"""\hrule Hello {\bf World} $x^2+y_1=\sqrt{z}$ caf\'e na\"\i ve.
\vrule width 2pt height 10pt depth 3pt\par
\vfil\eject Second page with {\it italic} and {\tt typewriter}.
\bye
""",
    "rules": r"""\hsize=3in \parindent=0pt
\hrule height 1pt \kern 4pt
\hbox{\vrule width 10pt height 5pt \kern 3pt \vrule width 1pt height 20pt depth 2pt}
\moveright 1in \vbox{\hrule width 2in \kern 2pt \hbox{Boxes \raise 3pt \hbox{up}}}
\bye
""",
}


def positions(dvitype_out):
    """dvitype -output-level=4: each set/put of a character or rule at its h, v."""
    out = []
    page = -1
    h = v = 0
    for line in dvitype_out.splitlines():
        if re.match(r"^\d+: beginning of page", line):
            page += 1
            h = v = 0
            continue
        m = re.match(r"^level \d+:\(h=(-?\d+),v=(-?\d+)", line)
        if m:
            h, v = int(m.group(1)), int(m.group(2))
            continue
        m = re.match(r"^\d+: (?:setchar(\d+)|set1 (\d+)|put1 (\d+))", line)
        if m:
            code = int(next(g for g in m.groups() if g is not None))
            out.append(f"{page} c {code} {h} {v}")
        m = re.match(r"^\d+: (setrule|putrule) height (-?\d+), width (-?\d+)", line)
        if m:
            hh, ww = int(m.group(2)), int(m.group(3))
            if hh > 0 and ww > 0:
                out.append(f"{page} r {h} {v} {ww} {hh}")
        m = re.search(r" h:=[-\d+]+=(-?\d+)", line)
        if m:
            h = int(m.group(1))
        m = re.search(r" v:=[-\d+]+=(-?\d+)", line)
        if m:
            v = int(m.group(1))
    return out


def main():
    out = os.path.abspath(sys.argv[1])
    os.makedirs(out, exist_ok=True)
    env = dict(os.environ, TEXMFVAR=os.path.join(out, "var"), TEXMFCACHE=os.path.join(out, "var"))
    for name, src in DOCS.items():
        with open(os.path.join(out, name + ".tex"), "w") as f:
            f.write(src)
        subprocess.run(["tex", "-interaction=batchmode", name + ".tex"], cwd=out, env=env,
                       capture_output=True, check=True)
        dt = subprocess.run(["dvitype", "-output-level=4", name + ".dvi"], cwd=out, env=env,
                            capture_output=True, text=True, check=True).stdout
        with open(os.path.join(out, name + ".dvitype"), "w") as f:
            f.write(dt)
        with open(os.path.join(out, name + ".positions"), "w") as f:
            f.write("\n".join(positions(dt)) + "\n")
        for font in set(re.findall(r"Font \d+: (\w+)", dt)):
            p = subprocess.run(["kpsewhich", font + ".tfm"], capture_output=True, text=True,
                               env=env).stdout.strip()
            if p:
                shutil.copy(p, os.path.join(out, font + ".tfm"))
    print("wrote", out)


main()
