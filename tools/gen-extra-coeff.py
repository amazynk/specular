#!/usr/bin/env python3
"""Generate crates/formats/specular-raw/src/extra_coeff.rs: colour data for bodies the dcraw
table of tools/gen-adobe-coeff.py lacks, from two published sources.

* LibRaw's colour table (src/tables/colordata.cpp of LibRaw 0.22.1, pinned by SHA-256): the
  XYZ -> camera matrices (x10000) and levels for an explicit list of bodies (BODIES), and the
  four-colour (4 x 3) matrices of CMYG / RGBE sensors (FOUR). Numbers only: no LibRaw code is
  used. For the Leica bodies the Panasonic body of the same camera is named (LibRaw itself
  files them under it).
* ACES input transforms for ARRIRAW (aces-dev v1.3.1,
  transforms/ctl/idt/vendorSupplied/arri/alexa/v3/EI800/IDT.ARRI.Alexa-v3-raw-EI800-CCT*.ctl,
  written by ARRI): per correlated colour temperature, the matrix from white-balanced ALEXA
  (ALEV III) sensor RGB to ACES AP0 primaries.

Usage (run Python isolated; the inputs are untrusted downloads):

    curl -o DIR/colordata.cpp \\
      https://raw.githubusercontent.com/LibRaw/LibRaw/0.22.1/src/tables/colordata.cpp
    for t in 2000 2100 2200 2400 2600 2900 3200 3500 3900 4300 4700 5100 5600 6500 7500 9000 11000; do
      curl -o DIR/aces/v3-EI800-$t.ctl https://raw.githubusercontent.com/ampas/aces-dev/v1.3.1/\\
    transforms/ctl/idt/vendorSupplied/arri/alexa/v3/EI800/IDT.ARRI.Alexa-v3-raw-EI800-CCT$t.ctl
    done
    python3 -I tools/gen-extra-coeff.py DIR/colordata.cpp DIR/aces \\
        > crates/formats/specular-raw/src/extra_coeff.rs
"""

import hashlib
import os
import re
import sys

COLORDATA_SHA256 = "621d6a7e1a1904095d27087cf10ac7e935c6a984ec336816b79c273a8eaca768"

MAKERS = {"PhaseOne": "Phase One", "Panasonic": "Panasonic", "Canon": "Canon", "Sony": "Sony",
          "Apple": "Apple", "Fujifilm": "Fujifilm"}

# (our prefix, LibRaw maker, LibRaw model, note). Three-colour bodies.
BODIES = [
    ("Phase One IQ140", "PhaseOne", "IQ140", ""),
    ("Phase One IQ150", "PhaseOne", "IQ150", ""),
    ("Phase One IQ160", "PhaseOne", "IQ160", ""),
    ("Phase One IQ180", "PhaseOne", "IQ180", ""),
    ("Phase One IQ250", "PhaseOne", "IQ250", ""),
    ("Phase One IQ260", "PhaseOne", "IQ260", ""),
    ("Phase One IQ280", "PhaseOne", "IQ280", ""),
    ("Phase One IQ3 50MP", "PhaseOne", "IQ3 50MP", ""),
    ("Phase One IQ3 60MP", "PhaseOne", "IQ3 60MP", ""),
    ("Phase One IQ3 80MP", "PhaseOne", "IQ3 80MP", ""),
    ("Phase One IQ3 100MP", "PhaseOne", "IQ3 100MP", ""),
    ("Phase One IQ4 150MP", "PhaseOne", "IQ4 150MP", ""),
    ("Leica D-Lux 7", "Panasonic", "DC-LX100M2", "Panasonic DC-LX100M2"),
    ("Leica V-Lux 5", "Panasonic", "DC-FZ1000M2", "Panasonic DC-FZ1000M2"),
    # Dave Coffin's measurement, used by dcraw and LibRaw for every RADC camera (Kodak DC40,
    # DC50): the Kodak decoder looks it up under this name.
    ("Apple QuickTake", "Apple", "QuickTake", ""),
    # SuperCCD SR (S photosites); LibRaw files the F700 and F710 under "F7".
    ("Fujifilm F700", "Fujifilm", "F7", ""),
    ("Fujifilm F710", "Fujifilm", "F7", ""),
]

# Four-colour sensors: (our prefix, LibRaw maker, LibRaw model, filter colours in matrix row
# order as named by dcraw's colour description). Their levels come from the dcraw table.
FOUR = [
    ("Canon PowerShot Pro70", "Canon", "PowerShot Pro70", "GMCY"),
    ("Canon PowerShot Pro90", "Canon", "PowerShot Pro90", "GMCY"),
    ("Canon PowerShot G1", "Canon", "PowerShot G1", "GMCY"),
    ("Sony DSC-F828", "Sony", "DSC-F828", "RGBE"),
]

ACES_CCT = [2000, 2100, 2200, 2400, 2600, 2900, 3200, 3500, 3900, 4300, 4700, 5100, 5600, 6500,
            7500, 9000, 11000]


def strip_comments(src: str) -> str:
    src = re.sub(r"/\*.*?\*/", "", src, flags=re.S)
    return re.sub(r"//[^\n]*", "", src)


def libraw_table(path: str) -> dict:
    raw = open(path, "rb").read()
    digest = hashlib.sha256(raw).hexdigest()
    if digest != COLORDATA_SHA256:
        raise SystemExit(f"{path}: sha256 {digest} is not the reviewed LibRaw 0.22.1 table")
    src = strip_comments(raw.decode("latin-1"))
    entry = re.compile(
        r'\{\s*LIBRAW_CAMERAMAKER_(\w+)\s*,\s*"([^"]+)"\s*,\s*(-?\w+)\s*,\s*(-?\w+)\s*,'
        r"\s*\{([^}]*)\}\s*\}"
    )
    out = {}
    for m in entry.finditer(src):
        maker, model, black, maximum, nums = m.groups()
        vals = [int(v) for v in nums.replace("\n", " ").split(",") if v.strip()]
        out.setdefault((maker, model), (int(black, 0), int(maximum, 0), vals))
    return out, digest


def aces(dirpath: str) -> list:
    rows = []
    line = re.compile(
        r"(r|g|b)Out\s*=\s*r_lin\s*\*\s*([-0-9.e+]+)\s*\+\s*g_lin\s*\*\s*([-0-9.e+]+)\s*\+"
        r"\s*b_lin\s*\*\s*([-0-9.e+]+)\s*;"
    )
    for t in ACES_CCT:
        p = os.path.join(dirpath, f"v3-EI800-{t}.ctl")
        raw = open(p, "rb").read()
        txt = raw.decode("latin-1")
        if f"CCT of adopted white set to {t}K" not in txt:
            raise SystemExit(f"{p}: not the {t} K transform")
        m = {k: [float(a), float(b), float(c)] for k, a, b, c in line.findall(txt)}
        if set(m) != {"r", "g", "b"}:
            raise SystemExit(f"{p}: matrix not found")
        rows.append((t, m["r"] + m["g"] + m["b"], hashlib.sha256(raw).hexdigest()))
    return rows


def level(v: int) -> int:
    # LibRaw writes Panasonic's +15 black offset as -15; dcraw's table (and specular) as 15.
    return abs(v)


def main() -> int:
    if len(sys.argv) != 3:
        raise SystemExit(__doc__)
    table, digest = libraw_table(sys.argv[1])
    idt = aces(sys.argv[2])
    out = []
    out.append("//! GENERATED by tools/gen-extra-coeff.py -- do not edit by hand.")
    out.append("//!")
    out.append("//! Colour data for bodies the dcraw table (adobe_coeff.rs) lacks:")
    out.append(f"//! - LibRaw 0.22.1 src/tables/colordata.cpp (sha256 {digest}):")
    out.append("//!   XYZ -> camera matrices x10000 and levels (numbers only), for the bodies of")
    out.append("//!   TABLE and the four-colour sensors of FOUR;")
    out.append("//! - ACES input transforms for ARRIRAW by ARRI (aces-dev v1.3.1, alexa/v3/EI800): the")
    out.append("//!   white-balanced ALEXA (ALEV III) sensor RGB -> ACES AP0 matrix per colour temperature.")
    out.append("")
    out.append("use crate::adobe_coeff::Entry;")
    out.append("")
    out.append("/// Three-colour bodies; `sibling_of` names the body whose entry it is (same camera).")
    out.append("#[rustfmt::skip]")
    out.append("pub(crate) static TABLE: &[Entry] = &[")
    for prefix, maker, model, sib in BODIES:
        black, maximum, vals = table[(maker, model)]
        if len(vals) < 9 or any(vals[9:]):
            raise SystemExit(f"{maker} {model}: not a 3-colour matrix")
        m = ", ".join(str(v) for v in vals[:9])
        out.append(
            f'    Entry {{ prefix: "{prefix}", black: {level(black)}, maximum: {maximum}, '
            f'matrix: Some([{m}]), sibling_of: "{sib}" }},'
        )
    out.append("];")
    out.append("")
    out.append("/// A four-colour sensor: XYZ -> camera (one row per filter colour, x10000).")
    out.append("pub(crate) struct Four {")
    out.append("    pub prefix: &'static str,")
    out.append("    /// The filter colour of each matrix row, as dcraw names them (G M C Y, R G B E).")
    out.append("    pub colors: &'static str,")
    out.append("    pub matrix: [i16; 12],")
    out.append("}")
    out.append("")
    out.append("#[rustfmt::skip]")
    out.append("pub(crate) static FOUR: &[Four] = &[")
    for prefix, maker, model, colors in FOUR:
        _, _, vals = table[(maker, model)]
        if len(vals) != 12:
            raise SystemExit(f"{maker} {model}: not a 4-colour matrix")
        m = ", ".join(str(v) for v in vals)
        out.append(f'    Four {{ prefix: "{prefix}", colors: "{colors}", matrix: [{m}] }},')
    out.append("];")
    out.append("")
    out.append("/// ARRI ALEXA (ALEV III): colour temperature (K) and the white-balanced sensor RGB ->")
    out.append("/// ACES AP0 matrix (rows sum to 1), from ARRI's ACES input transforms (file SHA-256).")
    out.append("#[rustfmt::skip]")
    out.append("pub(crate) static ALEXA_TO_AP0: &[(u32, [f64; 9])] = &[")
    for t, m, sha in idt:
        vals = ", ".join(repr(v) for v in m)
        out.append(f"    ({t}, [{vals}]), // {sha[:16]}")
    out.append("];")
    print("\n".join(out))
    return 0


if __name__ == "__main__":
    sys.exit(main())
