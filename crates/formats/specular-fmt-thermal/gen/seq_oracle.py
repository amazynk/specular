#!/usr/bin/env python3
"""Independent readings of FLIR sequences (.seq, .csq), the oracle for specular-fmt-thermal
(tests/seq_oracle.rs). Run from a venv with numpy, Pillow and pyjpegls, and flirpy for the
second reading of SEQ frames; nothing here is linked into Specular.

    python3 -I seq_oracle.py DIR OUT.tsv [WORK_DIR]

SEQ_ORACLE_STRIDE (default 1) keeps every n-th pixel; SEQ_ORACLE_FRAMES (default all)
the first n frames.

* Frames are cut at every "FFF\\0" that starts a plausible FFF header (version 100..199 in
  either byte order, a directory inside the file), each written to WORK_DIR as a .fff.
* exiftool reads each frame: `-b -RawThermalImage` (a TIFF of raw counts, read by Pillow; a
  PNG, byte-swapped as FLIR fills it; or a JPEG-LS stream, decoded by CharLS through
  pyjpegls) and the Planck and object tags (`-n`). Counts become degrees Celsius here in
  numpy by the published method (Minkina and Dudzik; the window half-way along the path and
  non-reflecting; the two-band atmospheric transmission with water vapour from humidity),
  written for this oracle.
* flirpy (MIT), when importable, reads each SEQ frame again (`flirpy.io.fff.Fff`): its
  counts and its own raw-to-temperature conversion, as a second column.

Rows (tab separated):
  seq    name frames width height
  frame  name k seconds                  (capture time, Unix seconds with milliseconds)
  cond   name k E OD Tr Ta Tw IRT RH     (degrees Celsius)
  px     name k i raw celsius flirpy     (flirpy '-' when not read)
  error  name words
"""
import glob
import io
import json
import os
import struct
import subprocess
import sys

import numpy as np
from PIL import Image

DIR, OUT = sys.argv[1], sys.argv[2]
WORK = sys.argv[3] if len(sys.argv) > 3 else os.path.join(os.environ.get("TMPDIR", "."), "seq-oracle")
STRIDE = int(os.environ.get("SEQ_ORACLE_STRIDE", "1"))
MAX_FRAMES = int(os.environ.get("SEQ_ORACLE_FRAMES", "0")) or None
TAGS = ["Emissivity", "ObjectDistance", "ReflectedApparentTemperature", "AtmosphericTemperature",
        "IRWindowTemperature", "IRWindowTransmission", "RelativeHumidity", "PlanckR1", "PlanckR2",
        "PlanckB", "PlanckF", "PlanckO", "AtmosphericTransAlpha1", "AtmosphericTransAlpha2",
        "AtmosphericTransBeta1", "AtmosphericTransBeta2", "AtmosphericTransX",
        "RawThermalImageWidth", "RawThermalImageHeight", "DateTimeOriginal"]


def plausible(d, at):
    if d[at:at + 4] != b"FFF\0" or at + 64 > len(d):
        return False
    for e in "<>":
        ver, dirp, n = struct.unpack_from(e + "III", d, at + 0x14)
        if 100 <= ver < 200:
            return 0 < n < 1000 and at + dirp + 32 * n <= len(d)
    return False


def frames(d):
    starts, at = [], 0
    while True:
        at = d.find(b"FFF\0", at)
        if at < 0:
            break
        if plausible(d, at):
            starts.append(at)
        at += 4
    # A header found inside a frame's records would cut it short: keep only starts at or after
    # the end of the previous frame's directory entries' data.
    out = []
    for i, s in enumerate(starts):
        e = starts[i + 1] if i + 1 < len(starts) else len(d)
        out.append(d[s:e])
    return out


def exif(path):
    out = subprocess.run(["exiftool", "-j", "-n"] + ["-" + t for t in TAGS] + [path],
                         capture_output=True, check=True).stdout
    return json.loads(out)[0]


def seconds(t):
    """Unix seconds of exiftool's -n DateTimeOriginal ("2017:05:19 12:45:33.583-07:00")."""
    import datetime as dt
    s = str(t.get("DateTimeOriginal", ""))
    if not s:
        return None
    base, frac, tz = s[:19], "0", "+00:00"
    rest = s[19:]
    if rest.startswith("."):
        j = 1
        while j < len(rest) and rest[j].isdigit():
            j += 1
        frac, rest = rest[1:j], rest[j:]
    if rest:
        tz = rest
    when = dt.datetime.strptime(base, "%Y:%m:%d %H:%M:%S")
    sign = -1 if tz[0] == "-" else 1
    hh, mm = int(tz[1:3]), int(tz[4:6])
    off = sign * (hh * 3600 + mm * 60)
    epoch = (when - dt.datetime(1970, 1, 1)).total_seconds() - off
    return epoch + float("0." + frac)


def counts(path, t):
    blob = subprocess.run(["exiftool", "-b", "-RawThermalImage", path],
                          capture_output=True, check=True).stdout
    w, h = int(t["RawThermalImageWidth"]), int(t["RawThermalImageHeight"])
    if blob.startswith(b"\xff\xd8\xff\xf7"):
        import jpeg_ls
        return np.asarray(jpeg_ls.decode(np.frombuffer(blob, dtype=np.uint8)), dtype=np.uint16).reshape(h, w)
    if blob.startswith(b"\x89PNG"):
        return np.array(Image.open(io.BytesIO(blob)), dtype=np.uint16).byteswap()
    if blob[:4] in (b"II*\0", b"MM\0*"):
        im = Image.open(io.BytesIO(blob))
        return np.array(im, dtype=np.uint16).reshape(im.size[1], im.size[0])
    return np.frombuffer(blob[:2 * w * h], dtype=">u2").reshape(h, w)


def celsius(raw, t):
    f = lambda k: float(t[k])
    E, OD, Tr, Ta, Tw, IRT, RH = (f("Emissivity"), f("ObjectDistance"), f("ReflectedApparentTemperature"),
                                  f("AtmosphericTemperature"), f("IRWindowTemperature"),
                                  f("IRWindowTransmission"), f("RelativeHumidity"))
    R1, R2, B, F, O = f("PlanckR1"), f("PlanckR2"), f("PlanckB"), f("PlanckF"), f("PlanckO")
    a1, a2, b1, b2, X = (f("AtmosphericTransAlpha1"), f("AtmosphericTransAlpha2"),
                         f("AtmosphericTransBeta1"), f("AtmosphericTransBeta2"), f("AtmosphericTransX"))
    if RH > 2:
        RH /= 100
    vapour = RH * np.exp(1.5587 + 0.06939 * Ta - 0.00027816 * Ta ** 2 + 0.00000068455 * Ta ** 3)
    half = np.sqrt(OD / 2)
    tau = (X * np.exp(-half * (a1 + b1 * np.sqrt(vapour)))
           + (1 - X) * np.exp(-half * (a2 + b2 * np.sqrt(vapour))))
    radiance = lambda c: R1 / (R2 * (np.exp(B / (c + 273.15)) - F)) - O
    # Air (tau1) between object and window, the window (IRT, emitting 1 - IRT), air (tau2)
    # between window and camera; both halves alike here. Each term of the measured signal
    # is taken away, the object's share divided out.
    tau1 = tau2 = tau
    obj = (raw.astype(np.float64) / (E * tau1 * IRT * tau2)
           - (1 - tau1) / (E * tau1) * radiance(Ta)
           - (1 - tau2) / (E * tau1 * IRT * tau2) * radiance(Ta)
           - (1 - IRT) / (E * tau1 * IRT) * radiance(Tw)
           - (1 - E) / E * radiance(Tr))
    with np.errstate(all="ignore"):
        c = B / np.log(R1 / (R2 * (obj + O)) + F) - 273.15
    return c, (E, OD, Tr, Ta, Tw, IRT, RH)


def flirpy_frame(data):
    try:
        from flirpy.io.fff import Fff
    except Exception:  # noqa: BLE001 - flirpy is optional
        return None
    try:
        f = Fff(data)
        raw = np.asarray(f.get_image(), dtype=np.uint16)
        temp = np.asarray(f.get_radiometric_image(), dtype=np.float64)
        return raw, temp
    except Exception:  # noqa: BLE001 - recorded as not read
        return None


def main():
    os.makedirs(WORK, exist_ok=True)
    files = sorted(p for p in glob.glob(os.path.join(DIR, "**", "*"), recursive=True)
                   if os.path.isfile(p) and p.lower().endswith((".seq", ".csq")))
    rows = []
    for p in files:
        name = os.path.relpath(p, DIR)
        with open(p, "rb") as fh:
            d = fh.read()
        try:
            fr = frames(d)[:MAX_FRAMES]
            out = []
            for k, b in enumerate(fr):
                path = os.path.join(WORK, "frame.fff")
                with open(path, "wb") as fh:
                    fh.write(b)
                t = exif(path)
                if "RawThermalImageWidth" not in t and k == len(fr) - 1 and k > 0:
                    break  # the last frame is cut short: exiftool finds no image in it
                raw = counts(path, t)
                c, cond = celsius(raw, t)
                second = flirpy_frame(b) if name.lower().endswith(".seq") else None
                out.append((k, seconds(t), cond, raw, c, second))
        except Exception as e:  # noqa: BLE001 - recorded, then compared as a refusal
            rows.append(["error", name, " ".join(str(e).split())[:200]])
            continue
        h, w = out[0][3].shape
        rows.append(["seq", name, str(len(out)), str(w), str(h)])
        for k, secs, cond, raw, c, second in out:
            rows.append(["frame", name, str(k), "-" if secs is None else "%.3f" % secs])
            rows.append(["cond", name, str(k)] + [repr(float(x)) for x in cond])
            fr_raw, fc = raw.reshape(-1), c.reshape(-1)
            fp = second[1].reshape(-1) if second is not None and second[1].size == fc.size else None
            if second is not None and not np.array_equal(second[0].reshape(-1), fr_raw):
                rows.append(["error", name, "flirpy counts differ from exiftool's in frame %d" % k])
            for i in range(0, fc.size, STRIDE):
                v = "%.6f" % fc[i] if np.isfinite(fc[i]) else "nan"
                q = "%.6f" % fp[i] if fp is not None and np.isfinite(fp[i]) else "-"
                rows.append(["px", name, str(k), str(i), str(int(fr_raw[i])), v, q])
    with open(OUT, "w") as fh:
        fh.write("# written by gen/seq_oracle.py\n")
        for r in rows:
            fh.write("\t".join(r) + "\n")


main()
