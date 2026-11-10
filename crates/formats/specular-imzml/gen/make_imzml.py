#!/usr/bin/env python3
"""Synthetic imzML 1.1 fixtures for specular-imzml (tests/data). Standard library only, and
written from the imzML 1.1.0 specification, not with any imzML library (pyimzML is the
oracle, gen/oracle.py, so it must not also be the writer).

    python3 -I make_imzml.py OUT_DIR

Each data set is a .imzML and an .ibd under 200 KB together: a few ions whose images are
blobs, rings and gradients over a noisy baseline, so ion images differ from the TIC.

  continuous_f32        continuous, 32-bit float m/z and intensities, profile, positive
  continuous_f64_i64    continuous, 64-bit float m/z, 64-bit integer intensities, negative,
                        pixels from (5, 3), UUID without braces, instrument via a group
  processed_f64_f32     processed, 64-bit float m/z, 32-bit float intensities, centroid,
                        holes, spectra out of raster order
  processed_f32_i32     processed, 32-bit float m/z, 32-bit integer intensities (IMS:1000141),
                        some empty spectra, mixed polarity, no pixel size
  processed_3d          processed, two z planes, 64-bit float both
"""

import math
import os
import struct
import sys
import uuid as uuidlib

FMT = {
    "f32": ("<f", 4, "MS:1000521", "32-bit float"),
    "f64": ("<d", 8, "MS:1000523", "64-bit float"),
    "i32": ("<i", 4, "IMS:1000141", "32-bit integer"),
    "i64": ("<q", 8, "MS:1000522", "64-bit integer"),
}

IONS = [  # m/z, pattern
    (184.0733, "blob"),
    (369.3516, "ring"),
    (520.3398, "gradient"),
    (760.5851, "corner"),
]


class Rng:
    """A small LCG, so the fixtures never change."""

    def __init__(self, seed):
        self.s = seed & 0xFFFFFFFF

    def next(self):
        self.s = (1664525 * self.s + 1013904223) & 0xFFFFFFFF
        return self.s / 2**32


def pattern(kind, x, y, w, h):
    u, v = (x + 0.5) / w, (y + 0.5) / h
    if kind == "blob":
        return math.exp(-((u - 0.35) ** 2 + (v - 0.4) ** 2) / 0.03)
    if kind == "ring":
        r = math.hypot(u - 0.6, v - 0.55)
        return math.exp(-((r - 0.25) ** 2) / 0.004)
    if kind == "gradient":
        return u * 0.8 + v * 0.2
    return 1.0 if (u > 0.7 and v > 0.7) else 0.05


def esc(s):
    return (s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")
            .replace('"', "&quot;"))


def cv(acc, name, value=None, extra=""):
    ref = acc.split(":")[0]
    v = f' value="{esc(str(value))}"' if value is not None else ""
    return f'<cvParam cvRef="{ref}" accession="{acc}" name="{esc(name)}"{v}{extra}/>'


def write(out, name, *, processed, mz_t, int_t, w, h, origin=(1, 1), zs=(1,), holes=(),
          polarity="positive", centroid=False, pixel_size=(20.0, 20.0), uuid_braces=True,
          instrument_group=False, shuffle=False, empty=(), mixed=False, seed=1):
    rng = Rng(seed)
    u = uuidlib.UUID(bytes=bytes((seed * 37 + i * 11) & 0xFF for i in range(16)))
    ibd = bytearray(u.bytes)
    mz_fmt, mz_size, mz_acc, mz_name = FMT[mz_t]
    in_fmt, in_size, in_acc, in_name = FMT[int_t]
    integer = int_t.startswith("i")

    def put(fmt, values):
        off = len(ibd)
        for v in values:
            ibd.extend(struct.pack(fmt, int(round(v)) if fmt in ("<i", "<q") else v))
        return off

    if not processed:
        # A coarse axis with fine steps around each ion (continuous axes need not be even).
        axis = {round(150.0 + i * 650.0 / 119, 4) for i in range(120)}
        for c, _ in IONS:
            axis |= {round(c + (i - 10) * 0.1, 4) for i in range(21)}
        axis = sorted(axis)
        shared = put(mz_fmt, axis)

    pixels = [(x, y, z) for z in zs for y in range(h) for x in range(w)
              if (x, y) not in holes]
    if shuffle:
        pixels = pixels[1::2] + pixels[0::2]
    spectra = []
    for k, (x, y, z) in enumerate(pixels):
        zf = 1.0 + 0.5 * zs.index(z)
        if processed:
            if (x, y) in empty:
                mzs = []
            else:
                mzs = []
                for m, kind in IONS:
                    a = pattern(kind, x, y, w, h) * zf
                    if a > 0.08:
                        mzs.append(m + (rng.next() - 0.5) * 0.004)
                for _ in range(3 + int(rng.next() * 6)):
                    mzs.append(150.0 + rng.next() * 650.0)
                mzs.sort()
            ints = []
            for m in mzs:
                a = 0.0
                for c, kind in IONS:
                    if abs(m - c) < 0.01:
                        a = pattern(kind, x, y, w, h) * zf
                ints.append(a * 9000.0 + rng.next() * 300.0 + 10.0)
            mz_off = put(mz_fmt, mzs)
            n = len(mzs)
        else:
            ints = []
            for m in axis:
                a = 0.0
                for c, kind in IONS:
                    a += pattern(kind, x, y, w, h) * math.exp(-((m - c) ** 2) / 0.8)
                ints.append(a * 9000.0 + rng.next() * 120.0 + 5.0)
            mz_off = shared
            n = len(axis)
        if integer:
            ints = [round(v) for v in ints]
        int_off = put(in_fmt, ints)
        pol = polarity
        if mixed:
            pol = "positive" if (x + y) % 3 else "negative"
        spectra.append((k, x + origin[0], y + origin[1], z, n, mz_off, int_off, pol))

    uuid_text = ("{%s}" % str(u).upper()) if uuid_braces else u.hex.upper()
    mode = cv("IMS:1000031", "processed") if processed else cv("IMS:1000030", "continuous")
    spectype = cv("MS:1000127", "centroid spectrum") if centroid \
        else cv("MS:1000128", "profile spectrum")
    lines = [
        '<?xml version="1.0" encoding="ISO-8859-1"?>',
        '<mzML xmlns="http://psi.hupo.org/ms/mzml" '
        'xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" version="1.1">',
        '<cvList count="3"><cv id="MS" fullName="Proteomics Standards Initiative Mass '
        'Spectrometry Ontology" version="4.1.0" URI="http://psidev.cvs.sourceforge.net/*checkout*/'
        'psidev/psi/psi-ms/mzML/controlledVocabulary/psi-ms.obo"/>'
        '<cv id="UO" fullName="Unit Ontology" version="" URI="http://obo.cvs.sourceforge.net/'
        '*checkout*/obo/obo/ontology/phenotype/unit.obo"/>'
        '<cv id="IMS" fullName="Imaging MS Ontology" version="1.1.0" URI="http://www.maldi-msi.org/'
        'download/imzml/imagingMS.obo"/></cvList>',
        "<fileDescription><fileContent>",
        cv("MS:1000579", "MS1 spectrum"),
        spectype,
        cv("IMS:1000080", "universally unique identifier", uuid_text),
        mode,
        "</fileContent></fileDescription>",
        '<referenceableParamGroupList count="4">',
        '<referenceableParamGroup id="mzArray">',
        cv("MS:1000576", "no compression"),
        cv("MS:1000514", "m/z array", extra=' unitCvRef="MS" unitAccession="MS:1000040" '
           'unitName="m/z"'),
        cv(mz_acc, mz_name),
        cv("IMS:1000101", "external data", "true"),
        "</referenceableParamGroup>",
        '<referenceableParamGroup id="intensityArray">',
        cv(in_acc, in_name),
        cv("MS:1000515", "intensity array", extra=' unitCvRef="MS" unitAccession="MS:1000131" '
           'unitName="number of detector counts"'),
        cv("MS:1000576", "no compression"),
        cv("IMS:1000101", "external data", "true"),
        "</referenceableParamGroup>",
        '<referenceableParamGroup id="scan1">',
        cv("MS:1000093", "increasing m/z scan"),
        "</referenceableParamGroup>",
        '<referenceableParamGroup id="spectrum1">',
        cv("MS:1000579", "MS1 spectrum"),
        cv("MS:1000511", "ms level", "1"),
        spectype,
        *([] if mixed else [cv("MS:1000130", "positive scan") if polarity == "positive"
                            else cv("MS:1000129", "negative scan")]),
        "</referenceableParamGroup>",
    ]
    if instrument_group:
        lines += ['<referenceableParamGroup id="instr">',
                  cv("MS:1002416", "Orbitrap Fusion"),
                  cv("MS:1000529", "instrument serial number", "FSN-0007"),
                  "</referenceableParamGroup>"]
    lines += ["</referenceableParamGroupList>",
              '<softwareList count="1"><software id="gen" version="1.0">',
              cv("MS:1000799", "custom unreleased software tool", "make_imzml.py"),
              "</software></softwareList>",
              '<scanSettingsList count="1"><scanSettings id="scansettings1">',
              cv("IMS:1000401", "top down"),
              cv("IMS:1000413", "flyback"),
              cv("IMS:1000480", "horizontal line scan"),
              cv("IMS:1000491", "linescan left right"),
              cv("IMS:1000042", "max count of pixels x", w + origin[0]),
              cv("IMS:1000043", "max count of pixels y", h + origin[1])]
    if pixel_size:
        lines += [cv("IMS:1000046", "pixel size (x)", pixel_size[0],
                     ' unitCvRef="UO" unitAccession="UO:0000017" unitName="micrometer"'),
                  cv("IMS:1000047", "pixel size y", pixel_size[1],
                     ' unitCvRef="UO" unitAccession="UO:0000017" unitName="micrometer"')]
    lines += ["</scanSettings></scanSettingsList>",
              '<instrumentConfigurationList count="1"><instrumentConfiguration id="IC1">']
    if instrument_group:
        lines.append('<referenceableParamGroupRef ref="instr"/>')
    else:
        lines.append(cv("MS:1000557", "LTQ FT Ultra"))
    lines += ['<componentList count="3">',
              '<source order="1">' + cv("MS:1000075", "matrix-assisted laser desorption "
                                         "ionization") + "</source>",
              '<analyzer order="2">' + cv("MS:1000079", "fourier transform ion cyclotron "
                                           "resonance mass spectrometer") + "</analyzer>",
              '<detector order="3">' + cv("MS:1000624", "inductive detector") + "</detector>",
              "</componentList>",
              '<softwareRef ref="gen"/>',
              "</instrumentConfiguration></instrumentConfigurationList>",
              '<dataProcessingList count="1"><dataProcessing id="export">'
              '<processingMethod order="1" softwareRef="gen">',
              cv("MS:1000544", "Conversion to mzML"),
              "</processingMethod></dataProcessing></dataProcessingList>",
              '<run defaultInstrumentConfigurationRef="IC1" id="Experiment01">',
              f'<spectrumList count="{len(spectra)}" defaultDataProcessingRef="export">']
    for (k, x, y, z, n, mz_off, int_off, pol) in spectra:
        zline = cv("IMS:1000052", "position z", z) if len(zs) > 1 else ""
        polline = ""
        if mixed:
            polline = cv("MS:1000130", "positive scan") if pol == "positive" \
                else cv("MS:1000129", "negative scan")
        lines.append(
            f'<spectrum id="Scan={k + 1}" defaultArrayLength="0" index="{k}">'
            '<referenceableParamGroupRef ref="spectrum1"/>' + polline +
            '<scanList count="1">' + cv("MS:1000795", "no combination") +
            '<scan instrumentConfigurationRef="IC1"><referenceableParamGroupRef ref="scan1"/>' +
            cv("IMS:1000050", "position x", x) + cv("IMS:1000051", "position y", y) + zline +
            "</scan></scanList>"
            '<binaryDataArrayList count="2">'
            '<binaryDataArray encodedLength="0"><referenceableParamGroupRef ref="mzArray"/>' +
            cv("IMS:1000103", "external array length", n) +
            cv("IMS:1000104", "external encoded length", n * mz_size) +
            cv("IMS:1000102", "external offset", mz_off) + "<binary/></binaryDataArray>"
            '<binaryDataArray encodedLength="0"><referenceableParamGroupRef ref="intensityArray"/>'
            + cv("IMS:1000103", "external array length", n) +
            cv("IMS:1000104", "external encoded length", n * in_size) +
            cv("IMS:1000102", "external offset", int_off) + "<binary/></binaryDataArray>"
            "</binaryDataArrayList></spectrum>")
    lines += ["</spectrumList></run></mzML>", ""]
    xml = "\n".join(lines).encode("latin-1")
    with open(os.path.join(out, name + ".imzML"), "wb") as f:
        f.write(xml)
    with open(os.path.join(out, name + ".ibd"), "wb") as f:
        f.write(ibd)
    total = len(xml) + len(ibd)
    assert total < 200_000, (name, total)
    print(f"{name}: {len(spectra)} spectra, {total} bytes")


def main():
    out = sys.argv[1]
    os.makedirs(out, exist_ok=True)
    write(out, "continuous_f32", processed=False, mz_t="f32", int_t="f32", w=9, h=7, seed=1)
    write(out, "continuous_f64_i64", processed=False, mz_t="f64", int_t="i64", w=6, h=5,
          origin=(4, 2), polarity="negative", uuid_braces=False, instrument_group=True,
          pixel_size=(10.0, 12.5), seed=2)
    write(out, "processed_f64_f32", processed=True, mz_t="f64", int_t="f32", w=12, h=9,
          holes={(0, 0), (11, 8), (5, 4), (6, 4)}, centroid=True, shuffle=True, seed=3)
    write(out, "processed_f32_i32", processed=True, mz_t="f32", int_t="i32", w=11, h=8,
          empty={(3, 3), (4, 3)}, mixed=True, pixel_size=None, centroid=True, seed=4)
    write(out, "processed_3d", processed=True, mz_t="f64", int_t="f64", w=8, h=6, zs=(1, 2),
          centroid=True, seed=5)


if __name__ == "__main__":
    main()
