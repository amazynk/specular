#!/usr/bin/env python3
"""Independent readings of thermal stills, as the oracle for specular-fmt-thermal
(tests/oracle.rs). Run from a venv with numpy, Pillow and infiray_irg; nothing here is
linked into Specular.

    python3 -I oracle.py DIR OUT.tsv [IRB_CLASSPATH BUILD_DIR]

THERMAL_ORACLE_STRIDE (default 1) keeps every n-th pixel only.

* FLIR (.jpg, .fff, .img): exiftool's `-b -RawThermalImage` (a PNG, which FLIR fills with
  little-endian bytes and so is byte-swapped after decoding; a TIFF, read by Pillow; or
  the bare counts exiftool leaves unwrapped, which are big-endian) and its Planck and
  object tags (`-n`), converted to degrees Celsius here in numpy by the published method
  (Minkina and Dudzik; window half-way along the path, non-reflecting).
* InfiRay .irg: `infiray_irg.load` (MIT, jaseg).
* InfraTec .irb: the `irb` Java reader (Apache-2.0, Jonathan Schilling) through
  gen/IrbDump.java (compiled into BUILD_DIR), when IRB_CLASSPATH (the reader's compiled
  classes and gson) is given.

Rows (tab separated):
  file  name kind width height
  cond  name E OD Tr Ta Tw IRT RH        (FLIR: the parameters used; degrees Celsius)
  px    name i raw celsius               (every pixel, or every n-th; raw is '-' where the
                                          oracle has no counts)
  error name words                       (the oracle could not read the file)
"""

import glob
import io
import json
import os
import subprocess
import sys
import tempfile

import numpy as np
from PIL import Image

DIR, OUT = sys.argv[1], sys.argv[2]
CLASSPATH = sys.argv[3] if len(sys.argv) > 4 else None
BUILD = sys.argv[4] if len(sys.argv) > 4 else None
STRIDE = int(os.environ.get("THERMAL_ORACLE_STRIDE", "1"))
HERE = os.path.dirname(os.path.abspath(__file__))

TAGS = ["Emissivity", "ObjectDistance", "ReflectedApparentTemperature", "AtmosphericTemperature",
        "IRWindowTemperature", "IRWindowTransmission", "RelativeHumidity", "PlanckR1", "PlanckR2",
        "PlanckB", "PlanckF", "PlanckO", "AtmosphericTransAlpha1", "AtmosphericTransAlpha2",
        "AtmosphericTransBeta1", "AtmosphericTransBeta2", "AtmosphericTransX",
        "RawThermalImageWidth", "RawThermalImageHeight", "RawThermalImageType"]


def exif(path):
    out = subprocess.run(["exiftool", "-j", "-n"] + ["-" + t for t in TAGS] + [path],
                         capture_output=True, check=True).stdout
    return json.loads(out)[0]


def flir_counts(path, tags):
    blob = subprocess.run(["exiftool", "-b", "-RawThermalImage", path],
                          capture_output=True, check=True).stdout
    w, h = int(tags["RawThermalImageWidth"]), int(tags["RawThermalImageHeight"])
    if blob.startswith(b"\x89PNG"):
        a = np.array(Image.open(io.BytesIO(blob)), dtype=np.uint16)
        return a.byteswap()
    if blob[:4] in (b"II*\0", b"MM\0*"):
        im = Image.open(io.BytesIO(blob))
        return np.array(im, dtype=np.uint16).reshape(im.size[1], im.size[0])
    return np.frombuffer(blob[:2 * w * h], dtype=">u2").reshape(h, w)


def flir_celsius(raw, t):
    f = lambda k: float(t[k])
    E, OD, RTemp, ATemp, IRWTemp, IRT, RH = (f("Emissivity"), f("ObjectDistance"),
                                             f("ReflectedApparentTemperature"),
                                             f("AtmosphericTemperature"), f("IRWindowTemperature"),
                                             f("IRWindowTransmission"), f("RelativeHumidity"))
    R1, R2, B, F, O = f("PlanckR1"), f("PlanckR2"), f("PlanckB"), f("PlanckF"), f("PlanckO")
    a1, a2, b1, b2, X = (f("AtmosphericTransAlpha1"), f("AtmosphericTransAlpha2"),
                         f("AtmosphericTransBeta1"), f("AtmosphericTransBeta2"), f("AtmosphericTransX"))
    if RH > 2:
        RH /= 100
    h2o = RH * np.exp(1.5587 + 0.06939 * ATemp - 0.00027816 * ATemp ** 2 + 0.00000068455 * ATemp ** 3)
    half = np.sqrt(OD / 2)
    tau = X * np.exp(-half * (a1 + b1 * np.sqrt(h2o))) + (1 - X) * np.exp(-half * (a2 + b2 * np.sqrt(h2o)))
    planck = lambda c: R1 / (R2 * (np.exp(B / (c + 273.15)) - F)) - O
    raw_refl, raw_atm, raw_wind = planck(RTemp), planck(ATemp), planck(IRWTemp)
    emiss_wind = 1 - IRT
    obj = (raw.astype(np.float64) / E / tau / IRT / tau
           - (1 - tau) / E / tau * raw_atm
           - (1 - tau) / E / tau / IRT / tau * raw_atm
           - emiss_wind / E / tau / IRT * raw_wind
           - (1 - E) / E * raw_refl)
    with np.errstate(all="ignore"):
        c = B / np.log(R1 / (R2 * (obj + O)) + F) - 273.15
    return c, (E, OD, RTemp, ATemp, IRWTemp, IRT, RH)


def irg(path):
    import infiray_irg
    with open(path, "rb") as fh:
        _coarse, fine, _vis = infiray_irg.load(fh.read())
    return np.asarray(fine, dtype=np.float64)


def irb(path):
    with tempfile.TemporaryDirectory(dir=os.environ.get("TMPDIR")) as d:
        out = os.path.join(d, "t.txt")
        subprocess.run(["java", "-cp", CLASSPATH + os.pathsep + BUILD, "IrbDump", os.path.abspath(path), out],
                       capture_output=True, check=True, cwd=d)
        with open(out) as fh:
            w, h = map(int, fh.readline().split())
            v = np.array([float(x) for x in fh.read().split()], dtype=np.float64)
    return v.reshape(h, w)


def kind(path):
    with open(path, "rb") as fh:
        head = fh.read(4)
    if head.startswith(b"\xffIRB"):
        return "irb"
    if head[:2] in (b"\xca\xac", b"\xba\xab", b"\x04\xa0"):
        return "irg"
    if head in (b"FFF\0", b"AFF\0") or head.startswith(b"\xff\xd8"):
        return "flir"
    return None


def main():
    files = sorted(p for p in glob.glob(os.path.join(DIR, "**", "*"), recursive=True) if os.path.isfile(p))
    if CLASSPATH:
        subprocess.run(["javac", "-d", BUILD, "-cp", CLASSPATH,
                        os.path.join(HERE, "IrbDump.java")], check=True)
    rows = []
    for p in files:
        name = os.path.relpath(p, DIR)
        k = kind(p)
        cond = None
        try:
            if k == "flir":
                t = exif(p)
                if "RawThermalImageType" not in t:
                    continue
                raw = flir_counts(p, t)
                c, cond = flir_celsius(raw, t)
            elif k == "irg":
                c = irg(p)
                raw = None
            elif k == "irb" and CLASSPATH:
                c = irb(p)
                raw = None
            else:
                continue
        except Exception as e:  # noqa: BLE001 - recorded, then compared as a refusal
            rows.append(["error", name, " ".join(str(e).split())[:200]])
            continue
        h, w = c.shape
        rows.append(["file", name, k, str(w), str(h)])
        if cond is not None:
            rows.append(["cond", name] + [repr(float(x)) for x in cond])
        flat_c = c.reshape(-1)
        flat_r = raw.reshape(-1) if raw is not None else None
        for i in range(0, w * h, STRIDE):
            r = str(int(flat_r[i])) if flat_r is not None else "-"
            rows.append(["px", name, str(i), r, "%.6f" % flat_c[i] if np.isfinite(flat_c[i]) else "nan"])
    with open(OUT, "w") as fh:
        fh.write("# written by gen/oracle.py\n")
        for r in rows:
            fh.write("\t".join(r) + "\n")


main()
