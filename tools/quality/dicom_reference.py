#!/usr/bin/env python3
"""DICOM grey display pipeline oracle (pydicom) and GSDF oracle (colour-science).

Writes crates/formats/specular-dicom/tests/data/display_oracle.tsv, which the
`display_conformance` test reads: each case lists the data elements of a one-row image
(the test builds the Part 10 file with specular-dicom's writer), its stored values and the
displayed fraction pydicom's pipeline gives for each (modality LUT / rescale, then the
window or VOI LUT, then the Presentation LUT, Presentation LUT Shape or MONOCHROME1
inversion). It also writes gsdf_oracle.tsv: PS3.14 luminance per JND index from
colour-science.

    target/quality/venv/bin/python tools/quality/dicom_reference.py [--check]

--check regenerates in memory and compares with the committed files.
"""

import math
import sys
from pathlib import Path

import numpy as np
from pydicom.dataset import Dataset
from pydicom.pixels.processing import (
    apply_modality_lut,
    apply_presentation_lut,
    apply_voi,
    apply_windowing,
)

ROOT = Path(__file__).resolve().parents[2]
DATA = ROOT / "crates/formats/specular-dicom/tests/data"


def samples(lo, hi, crit, n=160):
    """About n values spread over lo..=hi, plus every value within 3 of each critical point."""
    s = set(np.linspace(lo, hi, n).round().astype(np.int64).tolist())
    for c in crit:
        for d in range(-3, 4):
            v = int(math.floor(c)) + d
            if lo <= v <= hi:
                s.add(v)
    return sorted(s)


def lut_data(n, bits, kind):
    top = (1 << bits) - 1
    i = np.arange(n, dtype=np.float64) / (n - 1)
    if kind == "sqrt":
        v = np.sqrt(i)
    elif kind == "gamma":
        v = i**2.2
    elif kind == "s":
        v = 0.5 - 0.5 * np.cos(np.pi * i)
    else:
        raise ValueError(kind)
    return np.floor(v * top + 0.5).astype(np.int64).tolist()


class Case:
    def __init__(self, name, bits_stored, signed):
        self.name = name
        self.bits = bits_stored
        self.signed = signed
        self.elems = []  # (tag, vr, values) or ("seq", tag, [elems])
        self.ds = Dataset()
        self.ds.BitsAllocated = 16
        self.ds.BitsStored = bits_stored
        self.ds.HighBit = bits_stored - 1
        self.ds.PixelRepresentation = 1 if signed else 0
        self.ds.SamplesPerPixel = 1
        self.photometric = "MONOCHROME2"
        self.elems.append((0x00280100, "US", [16]))
        self.elems.append((0x00280101, "US", [bits_stored]))
        self.elems.append((0x00280102, "US", [bits_stored - 1]))
        self.elems.append((0x00280103, "US", [1 if signed else 0]))

    def put(self, tag, vr, values, kw):
        self.elems.append((tag, vr, values))
        setattr(self.ds, kw, values if len(values) > 1 else values[0])

    def lut_item(self, seq_tag, seq_kw, n, first, bits, data, desc_vr="US"):
        item = Dataset()
        item.LUTDescriptor = [n, first, bits]
        item["LUTDescriptor"].VR = "SS" if first < 0 else "US"
        item.LUTData = data
        item["LUTData"].VR = "US"
        setattr(self.ds, seq_kw, [item])
        # The file gets the descriptor as US (the usual encoding; a negative first value is
        # its two's complement word, read by context).
        desc = [n & 0xFFFF, first & 0xFFFF, bits]
        self.elems.append(("seq", seq_tag, [(0x00283002, desc_vr, desc), (0x00283006, "US", data)]))


def ds_has(ds, kw):
    return kw in ds


def expected(case, stored):
    ds = case.ds
    ds.PhotometricInterpretation = case.photometric
    arr = np.asarray(stored, dtype=np.int32 if case.signed else np.uint16)
    mod = apply_modality_lut(arr, ds)
    if ds_has(ds, "VOILUTSequence") and not ds_has(ds, "WindowCenter"):
        item = ds.VOILUTSequence[0]
        bits = item.LUTDescriptor[2]
        # pydicom 3.0.2 indexes an 8-bit LUT in uint8, wrapping inputs 256 or more above
        # the first mapped value; clamping to the LUT's input range first is equivalent.
        n, first = item.LUTDescriptor[0] or 65536, item.LUTDescriptor[1]
        iv = np.clip(np.asarray(mod).astype(np.int64), first, first + n - 1)
        voi = apply_voi(iv, ds).astype(np.float64)
        d = voi / ((1 << bits) - 1)
        y = d * 65535.0
    elif ds_has(ds, "WindowCenter"):
        # Windowing into a fixed 0..65535 range: unsigned 16-bit, no modality transform.
        w = Dataset()
        w.BitsStored = 16
        w.PixelRepresentation = 0
        w.PhotometricInterpretation = "MONOCHROME2"
        w.WindowCenter = ds.WindowCenter
        w.WindowWidth = ds.WindowWidth
        if ds_has(ds, "VOILUTFunction"):
            w.VOILUTFunction = ds.VOILUTFunction
        y = apply_windowing(np.asarray(mod, dtype=np.float64), w)
        d = y / 65535.0
    else:
        raise ValueError(case.name)
    if ds_has(ds, "PresentationLUTSequence"):
        item = ds.PresentationLUTSequence[0]
        bits = item.LUTDescriptor[2]
        p = apply_presentation_lut(y, ds).astype(np.float64)
        d = p / ((1 << bits) - 1)
    elif ds_has(ds, "PresentationLUTShape"):
        p = apply_presentation_lut(y, ds)
        d = np.asarray(p, dtype=np.float64) / 65535.0
    if case.photometric == "MONOCHROME1":
        # PS3.3 C.7.6.3.1.2: minimum sample value is white.
        d = 1.0 - d
    return d


def cases():
    out = []

    c = Case("ct_rescale_linear", 12, True)
    c.put(0x00281052, "DS", ["-1024"], "RescaleIntercept")
    c.put(0x00281053, "DS", ["1"], "RescaleSlope")
    c.put(0x00281050, "DS", ["40"], "WindowCenter")
    c.put(0x00281051, "DS", ["400"], "WindowWidth")
    out.append((c, samples(-2048, 2047, [1024 + 40 - 0.5 - 199.5, 1024 + 40 - 0.5 + 199.5])))

    c = Case("fractional_rescale_linear_exact", 16, False)
    c.put(0x00281052, "DS", ["-12.5"], "RescaleIntercept")
    c.put(0x00281053, "DS", ["0.37"], "RescaleSlope")
    c.put(0x00281050, "DS", ["7000"], "WindowCenter")
    c.put(0x00281051, "DS", ["9001"], "WindowWidth")
    c.put(0x00281056, "CS", ["LINEAR_EXACT"], "VOILUTFunction")
    lo, hi = (7000 - 4500.5 + 12.5) / 0.37, (7000 + 4500.5 + 12.5) / 0.37
    out.append((c, samples(0, 65535, [lo, hi], 300)))

    c = Case("sigmoid", 12, False)
    c.put(0x00281052, "DS", ["0"], "RescaleIntercept")
    c.put(0x00281053, "DS", ["2"], "RescaleSlope")
    c.put(0x00281050, "DS", ["3000"], "WindowCenter")
    c.put(0x00281051, "DS", ["1500"], "WindowWidth")
    c.put(0x00281056, "CS", ["SIGMOID"], "VOILUTFunction")
    out.append((c, samples(0, 4095, [1500])))

    c = Case("linear_width_one", 12, False)
    c.put(0x00281050, "DS", ["2048"], "WindowCenter")
    c.put(0x00281051, "DS", ["1"], "WindowWidth")
    out.append((c, samples(0, 4095, [2047.5])))

    c = Case("linear_half_centre_width_two", 12, False)
    c.put(0x00281050, "DS", ["1023.5"], "WindowCenter")
    c.put(0x00281051, "DS", ["2"], "WindowWidth")
    out.append((c, samples(0, 4095, [1023])))

    c = Case("second_window_ignored", 12, False)
    c.put(0x00281050, "DS", ["600", "2000"], "WindowCenter")
    c.put(0x00281051, "DS", ["1200", "50"], "WindowWidth")
    c.ds.WindowCenter = 600
    c.ds.WindowWidth = 1200
    out.append((c, samples(0, 4095, [0, 1200])))

    c = Case("modality_lut", 12, False)
    data = lut_data(1024, 16, "sqrt")
    c.lut_item(0x00283000, "ModalityLUTSequence", 1024, 100, 16, data)
    c.put(0x00281050, "DS", ["30000"], "WindowCenter")
    c.put(0x00281051, "DS", ["40000"], "WindowWidth")
    out.append((c, samples(0, 4095, [100, 1123])))

    c = Case("modality_lut_negative_first", 16, True)
    data = lut_data(1000, 12, "gamma")
    c.lut_item(0x00283000, "ModalityLUTSequence", 1000, -500, 16, data)
    c.put(0x00281050, "DS", ["2048"], "WindowCenter")
    c.put(0x00281051, "DS", ["4096"], "WindowWidth")
    c.put(0x00281056, "CS", ["LINEAR_EXACT"], "VOILUTFunction")
    out.append((c, samples(-1200, 1200, [-500, 499])))

    c = Case("voi_lut_12bit", 12, False)
    data = lut_data(512, 12, "s")
    c.lut_item(0x00283010, "VOILUTSequence", 512, 200, 12, data)
    out.append((c, samples(0, 4095, [200, 711])))

    c = Case("voi_lut_8bit", 12, False)
    data = lut_data(256, 8, "gamma")
    c.lut_item(0x00283010, "VOILUTSequence", 256, 1000, 8, data)
    out.append((c, samples(0, 4095, [1000, 1255])))

    c = Case("voi_lut_after_negative_rescale", 12, False)
    c.put(0x00281052, "DS", ["-1024"], "RescaleIntercept")
    c.put(0x00281053, "DS", ["1"], "RescaleSlope")
    data = lut_data(2048, 16, "s")
    c.lut_item(0x00283010, "VOILUTSequence", 2048, -1024, 16, data)
    out.append((c, samples(0, 4095, [0, 2047])))

    c = Case("presentation_lut", 12, False)
    c.put(0x00281050, "DS", ["2047.5"], "WindowCenter")
    c.put(0x00281051, "DS", ["4095"], "WindowWidth")
    c.put(0x00281056, "CS", ["LINEAR_EXACT"], "VOILUTFunction")
    data = lut_data(256, 12, "gamma")
    c.lut_item(0x20500010, "PresentationLUTSequence", 256, 0, 12, data)
    st = samples(0, 4095, [0, 4095], 600)
    out.append((c, st))

    c = Case("presentation_shape_inverse", 12, False)
    c.put(0x00281050, "DS", ["2000"], "WindowCenter")
    c.put(0x00281051, "DS", ["1000"], "WindowWidth")
    c.put(0x20500020, "CS", ["INVERSE"], "PresentationLUTShape")
    out.append((c, samples(0, 4095, [1499.5, 2500.5])))

    c = Case("monochrome1", 12, False)
    c.photometric = "MONOCHROME1"
    c.put(0x00281050, "DS", ["2000"], "WindowCenter")
    c.put(0x00281051, "DS", ["1000"], "WindowWidth")
    out.append((c, samples(0, 4095, [1499.5, 2500.5])))
    return out


def fmt_elem(e, indent=""):
    tag, vr, values = e
    if vr in ("DS", "CS"):
        v = "\\".join(values)
    else:
        v = " ".join(str(x) for x in values)
    return f"{indent}elem\t{tag:08X}\t{vr}\t{v}\n"


def display_tsv():
    s = ["# Generated by tools/quality/dicom_reference.py (pydicom display pipeline).\n"]
    for c, stored in cases():
        d = expected(c, stored)
        s.append(f"case\t{c.name}\t{c.photometric}\n")
        for e in c.elems:
            if e[0] == "seq":
                s.append(f"seq\t{e[1]:08X}\n")
                for i in e[2]:
                    s.append(fmt_elem(i, "\t"))
                s.append("end\n")
            else:
                s.append(fmt_elem(e))
        s.append("stored\t" + " ".join(str(x) for x in stored) + "\n")
        s.append("expect\t" + " ".join(f"{x:.9f}".rstrip("0").rstrip(".") for x in d) + "\n")
    return "".join(s)


def gsdf_tsv():
    import colour

    s = ["# Generated by tools/quality/dicom_reference.py (colour-science eotf_DICOMGSDF).\n"]
    s.append("# L: JND index j, luminance (cd/m2)\n")
    for j in range(1, 1024):
        L = float(colour.models.eotf_DICOMGSDF(j, in_int=True))
        s.append(f"L\t{j}\t{L:.10g}\n")
    s.append("# J: luminance (cd/m2), JND index (eotf_inverse_DICOMGSDF)\n")
    for L in np.geomspace(0.05, 3993.0, 200):
        j = float(colour.models.eotf_inverse_DICOMGSDF(L)) * 1023
        s.append(f"J\t{L:.10g}\t{j:.10g}\n")
    # A display calibrated to the GSDF between its black and white (PS3.14 7.1): P-values
    # in equal JND steps from j(Lmin) to j(Lmax).
    s.append("# P: Lmin, Lmax (cd/m2), P-value 0..1, luminance (cd/m2)\n")
    for lmin, lmax in [(0.5, 350.0), (0.2, 600.0), (1.0, 170.0), (0.05, 3000.0)]:
        j0 = float(colour.models.eotf_inverse_DICOMGSDF(lmin)) * 1023
        j1 = float(colour.models.eotf_inverse_DICOMGSDF(lmax)) * 1023
        for k in range(65):
            p = k / 64
            L = float(colour.models.eotf_DICOMGSDF(j0 + p * (j1 - j0), in_int=True))
            s.append(f"P\t{lmin:g}\t{lmax:g}\t{p:.10g}\t{L:.10g}\n")
    return "".join(s)


def main():
    files = {"display_oracle.tsv": display_tsv(), "gsdf_oracle.tsv": gsdf_tsv()}
    if "--check" in sys.argv:
        bad = [n for n, t in files.items() if (DATA / n).read_text() != t]
        print("mismatch: " + ", ".join(bad) if bad else "up to date")
        sys.exit(1 if bad else 0)
    for n, t in files.items():
        (DATA / n).write_text(t)
        print(f"wrote {DATA / n} ({len(t)} bytes)")


if __name__ == "__main__":
    main()
