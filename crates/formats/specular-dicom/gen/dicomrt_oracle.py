"""Writes the oracle tables of tests/dicomrt.rs from pydicom and dicompyler-core.

Usage: dicomrt_oracle.py DATA_DIR
       dicomrt_oracle.py --corpus CORPUS_DIR > CORPUS_DIR/oracle.tsv

DATA_DIR is tests/data (the fixtures of dicomrt_make.py are under DATA_DIR/dicomrt). With
--corpus (tools/corpus/fetch-dicomrt.sh), every .dcm file under CORPUS_DIR goes into one table
on stdout for tests/dicomrt_corpus.rs, each row prefixed by its kind: S (SR content items) and
W (waveform channels) with the file's path relative to CORPUS_DIR, and D (a DVH, as in
dvh_oracle.tsv) for every ROI of every structure set over every dose of the same frame of
reference.

Run with the venv of dicomrt_make.py:

    target/dicomrt/venv/bin/python -I gen/dicomrt_oracle.py tests/data

sr_oracle.tsv        every content item of every SR fixture, depth first:
                     file, path (1-based, root empty), relationship, value type, concept
                     (value|scheme|meaning), value in a canonical text form (see `value`).
waveform_oracle.tsv  every channel of every waveform fixture: file, group, channel, label,
                     frequency, samples, then min, max, mean and every 37th sample of the
                     channel in its units, from pydicom's multiplex_array (raw × sensitivity ×
                     correction + baseline). pydicom returns µ-law and A-law bytes unexpanded,
                     so those channels are expanded here per ITU-T G.711 first.
dvh_oracle.tsv       the cumulative DVH of every ROI of the RT Structure Set over each RT Dose
                     fixture from dicompyler-core's get_dvh (no interpolation): volume (cm³),
                     max, min, mean, D95, D100 (Gy), V20Gy (cm³), and the cumulative volume at
                     every 50 cGy.
"""

import os
import sys

import numpy as np
import pydicom


def clean(s):
    return str(s).replace("\t", " ").replace("\r", " ").replace("\n", " ").strip()


def code(seq):
    if not seq:
        return ""
    c = seq[0]
    value = c.get("CodeValue") or c.get("LongCodeValue") or c.get("URNCodeValue") or ""
    return "%s|%s|%s" % (clean(value), clean(c.get("CodingSchemeDesignator", "")),
                         clean(c.get("CodeMeaning", "")))


def raw_text(it, keyword):
    """The element's value as written (DS/IS as their original strings)."""
    if keyword not in it:
        return ""
    v = it[keyword].value
    if v is None:
        return ""
    if isinstance(v, (list, pydicom.multival.MultiValue)):
        return "\\".join(clean(getattr(x, "original_string", x)) for x in v)
    return clean(getattr(v, "original_string", v))


def floats(it, keyword):
    if keyword not in it:
        return []
    v = it[keyword].value
    if isinstance(v, (bytes, bytearray)):
        return []
    if not isinstance(v, (list, pydicom.multival.MultiValue)):
        v = [v]
    return [float(x) for x in v]


def fmt(x):
    return "%.4f" % x


def sop_ref(it):
    seq = it.get("ReferencedSOPSequence")
    if not seq:
        return "||"
    r = seq[0]
    frames = r.get("ReferencedFrameNumber")
    if frames is None:
        frames = []
    elif not isinstance(frames, (list, pydicom.multival.MultiValue)):
        frames = [frames]
    return "%s|%s|%s" % (clean(r.get("ReferencedSOPClassUID", "")),
                         clean(r.get("ReferencedSOPInstanceUID", "")),
                         ",".join(str(int(f)) for f in frames))


def value(it):
    """The canonical text of a content item's value (tests/dicomrt.rs builds the same)."""
    vt = clean(it.get("ValueType", ""))
    if "ReferencedContentItemIdentifier" in it and not vt:
        return "ref|" + ".".join(str(int(x)) for x in it.ReferencedContentItemIdentifier)
    if vt == "CONTAINER":
        tid = ""
        if it.get("ContentTemplateSequence"):
            tid = clean(it.ContentTemplateSequence[0].get("TemplateIdentifier", ""))
        return "%s|%s" % (clean(it.get("ContinuityOfContent", "")), tid)
    if vt == "TEXT":
        return clean(it.get("TextValue", ""))
    if vt in ("DATETIME", "DATE", "TIME"):
        return raw_text(it, {"DATETIME": "DateTime", "DATE": "Date", "TIME": "Time"}[vt])
    if vt == "PNAME":
        return clean(it.get("PersonName", ""))
    if vt == "UIDREF":
        return clean(it.get("UID", ""))
    if vt == "CODE":
        return code(it.get("ConceptCodeSequence"))
    if vt == "NUM":
        mv = it.get("MeasuredValueSequence")
        q = code(it.get("NumericValueQualifierCodeSequence"))
        if mv:
            m = mv[0]
            if not q:
                q = code(m.get("NumericValueQualifierCodeSequence"))
            return "%s|%s|%s" % (raw_text(m, "NumericValue"),
                                 code(m.get("MeasurementUnitsCodeSequence")), q)
        return "||%s" % q
    if vt in ("IMAGE", "COMPOSITE", "WAVEFORM"):
        return sop_ref(it)
    if vt == "SCOORD":
        return "%s|%s" % (clean(it.get("GraphicType", "")),
                          ",".join(fmt(x) for x in floats(it, "GraphicData")))
    if vt == "SCOORD3D":
        return "%s|%s|%s" % (clean(it.get("GraphicType", "")),
                             clean(it.get("ReferencedFrameOfReferenceUID", "")),
                             ",".join(fmt(x) for x in floats(it, "GraphicData")))
    if vt == "TCOORD":
        if "ReferencedSamplePositions" in it:
            vals = [str(int(x)) for x in floats(it, "ReferencedSamplePositions")]
        elif "ReferencedTimeOffsets" in it:
            # Shortest round-trip text, integers without a fraction (sr::format_number).
            vals = [str(int(x)) if x == int(x) else repr(float(x))
                    for x in floats(it, "ReferencedTimeOffsets")]
        else:
            vals = [clean(x) for x in (it.get("ReferencedDateTime") or [])]
        return "%s|%s" % (clean(it.get("TemporalRangeType", "")), ",".join(vals))
    return ""


def sr_rows(name, ds):
    rows = []

    def go(it, path, root):
        rel = "" if root else clean(it.get("RelationshipType", ""))
        vt = clean(it.get("ValueType", ""))
        rows.append([name, ".".join(map(str, path)), rel, vt,
                     code(it.get("ConceptNameCodeSequence")), value(it)])
        for i, child in enumerate(it.get("ContentSequence") or []):
            go(child, path + [i + 1], False)

    go(ds, [], True)
    return rows


def g711_mu(b):
    u = (~b.astype(np.int32)) & 0xFF
    sign = u & 0x80
    exp = (u >> 4) & 7
    mant = u & 0x0F
    mag = (((mant << 3) + 0x84) << exp) - 0x84
    return np.where(sign != 0, -mag, mag).astype(np.float64)


def g711_a(b):
    a = b.astype(np.int32) ^ 0x55
    sign = a & 0x80
    exp = (a >> 4) & 7
    mant = a & 0x0F
    mag = np.where(exp == 0, (mant << 4) + 8, ((mant << 4) + 0x108) << np.maximum(exp - 1, 0))
    return np.where(sign != 0, mag, -mag).astype(np.float64)


def waveform_rows(name, ds):
    from pydicom.waveforms import multiplex_array
    rows = []
    for g, group in enumerate(ds.WaveformSequence):
        interp = clean(group.WaveformSampleInterpretation)
        companded = interp in ("MB", "AB")
        arr = multiplex_array(ds, g, as_raw=companded)
        for c, ch in enumerate(group.ChannelDefinitionSequence):
            x = np.asarray(arr[:, c], dtype=np.float64)
            if companded:
                x = (g711_mu if interp == "MB" else g711_a)(arr[:, c].astype(np.uint8))
                sens = float(ch.get("ChannelSensitivity", 1) or 1)
                corr = float(ch.get("ChannelSensitivityCorrectionFactor", 1) or 1)
                base = float(ch.get("ChannelBaseline", 0) or 0)
                x = x * sens * corr + base
            rows.append([name, str(g), str(c), clean(ch.get("ChannelLabel", "")),
                         "%g" % float(group.SamplingFrequency), str(len(x)),
                         "%.6g" % x.min(), "%.6g" % x.max(), "%.6g" % x.mean(),
                         ",".join("%.6g" % v for v in x[::37])])
    return rows


def dicompyler():
    """dicompyler-core 0.5.6 imports two pydicom 2 names that pydicom 3 moved; alias them."""
    import types
    import pydicom.dicomio
    import pydicom.pixels.utils
    pydicom.dicomio.read_file = pydicom.dcmread
    util = types.ModuleType("pydicom.pixel_data_handlers.util")
    util.pixel_dtype = pydicom.pixels.utils.pixel_dtype
    handlers = types.ModuleType("pydicom.pixel_data_handlers")
    handlers.util = util
    sys.modules.setdefault("pydicom.pixel_data_handlers", handlers)
    sys.modules.setdefault("pydicom.pixel_data_handlers.util", util)
    from dicompylercore import dicomparser, dvhcalc
    return dicomparser, dvhcalc


def dvh_rows(rs, rd, dose_name):
    """The DVH row of every ROI with contours of structure set `rs` over dose `rd`."""
    dicomparser, dvhcalc = dicompyler()
    structures = dicomparser.DicomParser(rs).GetStructures()
    contoured = {r.ReferencedROINumber for r in pydicom.dcmread(rs, force=True).get("ROIContourSequence", [])
                 if r.get("ContourSequence")}
    rows = []
    for number in sorted(structures):
        if number not in contoured:
            continue
        d = dvhcalc.get_dvh(rs, rd, number)
        counts = d.cumulative.counts
        stride = ",".join("%.6f" % counts[i] for i in range(0, len(counts), 50))
        rows.append([dose_name, str(number), clean(structures[number]["name"]),
                     "%.6f" % d.volume, "%.4f" % d.max, "%.4f" % d.min, "%.6f" % d.mean,
                     "%.4f" % d.D95.value, "%.4f" % d.D100.value,
                     "%.6f" % d.V20Gy.value, stride])
    return rows


def corpus(root):
    files = sorted(os.path.join(d, f) for d, _, fs in os.walk(root) for f in fs
                   if f.endswith(".dcm"))
    doses, sets = [], []
    for path in files:
        rel = os.path.relpath(path, root)
        ds = pydicom.dcmread(path, stop_before_pixels=True, force=True)
        if "WaveformSequence" in ds:
            for r in waveform_rows(rel, pydicom.dcmread(path, force=True)):
                print("W\t" + "\t".join(r))
        elif ds.get("Modality") == "SR" and ds.get("ValueType") == "CONTAINER":
            for r in sr_rows(rel, ds):
                print("S\t" + "\t".join(r))
        elif ds.get("Modality") == "RTDOSE" and ds.get("DoseSummationType") != "BEAM" \
                and int(ds.get("NumberOfFrames", 1) or 1) > 1:
            doses.append((rel, path, ds.get("FrameOfReferenceUID")))
        elif ds.get("Modality") == "RTSTRUCT":
            fors = {f.FrameOfReferenceUID for f in ds.get("ReferencedFrameOfReferenceSequence", [])}
            fors |= {r.get("ReferencedFrameOfReferenceUID")
                     for r in ds.get("StructureSetROISequence", [])}
            sets.append((rel, path, fors))
    for rel, path, f in doses:
        for srel, spath, fors in sets:
            if f in fors:
                for r in dvh_rows(spath, path, rel):
                    print("D\t%s\t" % srel + "\t".join(r))


def write(path, header, rows):
    with open(path, "w", encoding="utf-8", newline="\n") as f:
        f.write("\t".join(header) + "\n")
        for r in rows:
            f.write("\t".join(r) + "\n")


SR_HEADER = ["file", "path", "relationship", "value_type", "concept", "value"]
WAVE_HEADER = ["file", "group", "channel", "label", "frequency", "samples", "min", "max", "mean",
               "every_37th"]


def main():
    if sys.argv[1] == "--corpus":
        corpus(sys.argv[2])
        return
    data = sys.argv[1]
    root = os.path.join(data, "dicomrt")
    srs, waves = [], []
    for sub in ("study", "ecg"):
        for f in sorted(os.listdir(os.path.join(root, sub))):
            ds = pydicom.dcmread(os.path.join(root, sub, f))
            name = sub + "/" + f
            if "WaveformSequence" in ds:
                waves += waveform_rows(name, ds)
            elif ds.get("Modality") == "SR":
                srs += sr_rows(name, ds)
    write(os.path.join(data, "sr_oracle.tsv"), SR_HEADER, srs)
    write(os.path.join(data, "waveform_oracle.tsv"), WAVE_HEADER, waves)
    write(os.path.join(data, "dvh_oracle.tsv"),
          ["dose", "roi", "name", "volume_cc", "max_gy", "min_gy", "mean_gy", "d95_gy",
           "d100_gy", "v20gy_cc", "cumulative_every_50cgy"],
          dvh_rows(os.path.join(root, "study", "rtstruct.dcm"),
                   os.path.join(root, "study", "rtdose.dcm"), "rtdose.dcm"))


if __name__ == "__main__":
    main()
