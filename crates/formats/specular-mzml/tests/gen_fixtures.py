"""Writes the small mzML / mzXML fixtures in tests/data (deterministic).

Run with the oracle venv (numpy, pynumpress):
  target/pcb-spectra/venv/bin/python -I crates/formats/specular-mzml/tests/gen_fixtures.py
"""
import base64
import os
import struct
import zlib

import numpy as np
import pynumpress

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "data")

CV = {
    "f32": ("MS:1000521", "32-bit float"),
    "f64": ("MS:1000523", "64-bit float"),
    "i32": ("MS:1000519", "32-bit integer"),
    "i64": ("MS:1000522", "64-bit integer"),
    "none": ("MS:1000576", "no compression"),
    "zlib": ("MS:1000574", "zlib compression"),
    "lin": ("MS:1002312", "MS-Numpress linear prediction compression"),
    "pic": ("MS:1002313", "MS-Numpress positive integer compression"),
    "slof": ("MS:1002314", "MS-Numpress short logged float compression"),
    "lin+z": ("MS:1002746", "MS-Numpress linear prediction compression followed by zlib compression"),
    "pic+z": ("MS:1002747", "MS-Numpress positive integer compression followed by zlib compression"),
    "slof+z": ("MS:1002748", "MS-Numpress short logged float compression followed by zlib compression"),
    "zstd": ("MS:1003780", "zstd compression"),
    "mz": ("MS:1000514", "m/z array"),
    "int": ("MS:1000515", "intensity array"),
    "time": ("MS:1000595", "time array"),
}


def cv(key, extra=""):
    acc, name = CV[key]
    return f'<cvParam cvRef="MS" accession="{acc}" name="{name}" value=""{extra}/>'


def encode(values, number, comp):
    a = np.asarray(values, dtype=np.float64)
    if comp.startswith("lin"):
        raw = bytes(pynumpress.encode_linear(a, pynumpress.optimal_linear_fixed_point(a)))
    elif comp.startswith("pic"):
        raw = bytes(pynumpress.encode_pic(a))
    elif comp.startswith("slof"):
        raw = bytes(pynumpress.encode_slof(a, pynumpress.optimal_slof_fixed_point(a)))
    else:
        dt = {"f32": "<f4", "f64": "<f8", "i32": "<i4", "i64": "<i8"}[number]
        raw = a.astype(dt).tobytes()
    if comp in ("zlib", "zstd") or comp.endswith("+z"):
        raw = zlib.compress(raw, 6)
    return base64.b64encode(raw).decode()


def array(kind, values, number="f64", comp="none", unit=""):
    text = encode(values, number, comp)
    params = [cv(number), cv(comp), cv(kind, unit)]
    return (f'<binaryDataArray encodedLength="{len(text)}">' + "".join(params)
            + f"<binary>{text}</binary></binaryDataArray>")


def spectrum(index, sid, mz, inten, level=1, centroid=False, rt=None, rt_unit="minute",
             tic=True, mzenc=("f64", "none"), intenc=("f32", "none"), precursor=None,
             group=None):
    n = len(mz)
    p = []
    if group:
        p.append(f'<referenceableParamGroupRef ref="{group}"/>')
    p.append(f'<cvParam cvRef="MS" accession="MS:1000511" name="ms level" value="{level}"/>')
    p.append('<cvParam cvRef="MS" accession="MS:1000127" name="centroid spectrum" value=""/>' if centroid
             else '<cvParam cvRef="MS" accession="MS:1000128" name="profile spectrum" value=""/>')
    if tic:
        p.append(f'<cvParam cvRef="MS" accession="MS:1000285" name="total ion current" value="{float(np.sum(inten))!r}"/>')
    k = int(np.argmax(inten)) if n else 0
    if n:
        p.append(f'<cvParam cvRef="MS" accession="MS:1000504" name="base peak m/z" value="{float(mz[k])!r}"/>')
    scan = ""
    if rt is not None:
        ua = "UO:0000031" if rt_unit == "minute" else "UO:0000010"
        scan = ('<scanList count="1"><cvParam cvRef="MS" accession="MS:1000795" name="no combination" value=""/>'
                f'<scan><cvParam cvRef="MS" accession="MS:1000016" name="scan start time" value="{rt!r}" '
                f'unitCvRef="UO" unitAccession="{ua}" unitName="{rt_unit}"/></scan></scanList>')
    prec = ""
    if precursor:
        pmz, z = precursor
        prec = ('<precursorList count="1"><precursor><isolationWindow>'
                f'<cvParam cvRef="MS" accession="MS:1000827" name="isolation window target m/z" value="{pmz - 0.01!r}"/>'
                '</isolationWindow><selectedIonList count="1"><selectedIon>'
                f'<cvParam cvRef="MS" accession="MS:1000744" name="selected ion m/z" value="{pmz!r}"/>'
                f'<cvParam cvRef="MS" accession="MS:1000041" name="charge state" value="{z}"/>'
                '</selectedIon></selectedIonList><activation>'
                '<cvParam cvRef="MS" accession="MS:1000133" name="collision-induced dissociation" value=""/>'
                '</activation></precursor></precursorList>')
    arrays = (f'<binaryDataArrayList count="2">{array("mz", mz, *mzenc)}{array("int", inten, *intenc)}'
              "</binaryDataArrayList>")
    return (f'<spectrum index="{index}" id="{sid}" defaultArrayLength="{n}">' + "".join(p) + scan + prec
            + arrays + "</spectrum>\n")


def chromatogram(index, cid, times, inten, kind, unit="second", tenc=("f64", "zlib"), ienc=("f32", "zlib")):
    ua = "UO:0000010" if unit == "second" else "UO:0000031"
    acc, name = kind
    u = f' unitCvRef="UO" unitAccession="{ua}" unitName="{unit}"'
    return (f'<chromatogram index="{index}" id="{cid}" defaultArrayLength="{len(times)}">'
            f'<cvParam cvRef="MS" accession="{acc}" name="{name}" value=""/>'
            f'<binaryDataArrayList count="2">{array("time", times, *tenc, unit=u)}{array("int", inten, *ienc)}'
            "</binaryDataArrayList></chromatogram>\n")


HEAD = """<mzML xmlns="http://psi.hupo.org/ms/mzml" version="1.1.0" id="fixture">
<cvList count="2"><cv id="MS" fullName="PSI-MS" version="4.1.0" URI="https://psidev.info/ms.obo"/><cv id="UO" fullName="Unit Ontology" URI="http://ontologies.berkeleybop.org/uo.obo"/></cvList>
<fileDescription><fileContent><cvParam cvRef="MS" accession="MS:1000579" name="MS1 spectrum" value=""/><cvParam cvRef="MS" accession="MS:1000580" name="MSn spectrum" value=""/></fileContent>
<sourceFileList count="1"><sourceFile id="RAW1" name="sample01.raw" location="file:///data"/></sourceFileList></fileDescription>
<referenceableParamGroupList count="1"><referenceableParamGroup id="Pos"><cvParam cvRef="MS" accession="MS:1000130" name="positive scan" value=""/></referenceableParamGroup></referenceableParamGroupList>
<softwareList count="1"><software id="gen" version="1.0"><cvParam cvRef="MS" accession="MS:1000799" name="custom unreleased software tool" value="gen_fixtures"/></software></softwareList>
<instrumentConfigurationList count="1"><instrumentConfiguration id="IC1"><cvParam cvRef="MS" accession="MS:1001911" name="Q Exactive" value=""/><cvParam cvRef="MS" accession="MS:1000529" name="instrument serial number" value="SN42"/>
<componentList count="3"><source order="1"><cvParam cvRef="MS" accession="MS:1000073" name="electrospray ionization" value=""/></source><analyzer order="2"><cvParam cvRef="MS" accession="MS:1000484" name="orbitrap" value=""/></analyzer><detector order="3"><cvParam cvRef="MS" accession="MS:1000624" name="inductive detector" value=""/></detector></componentList></instrumentConfiguration></instrumentConfigurationList>
<dataProcessingList count="1"><dataProcessing id="dp"><processingMethod order="1" softwareRef="gen"><cvParam cvRef="MS" accession="MS:1000544" name="Conversion to mzML" value=""/></processingMethod></dataProcessing></dataProcessingList>
<run id="run01" defaultInstrumentConfigurationRef="IC1" startTimeStamp="2026-01-02T03:04:05Z">
"""


def body(spectra, chroms):
    s = HEAD
    if spectra:
        s += f'<spectrumList count="{len(spectra)}" defaultDataProcessingRef="dp">\n' + "".join(spectra) + "</spectrumList>\n"
    if chroms:
        s += f'<chromatogramList count="{len(chroms)}" defaultDataProcessingRef="dp">\n' + "".join(chroms) + "</chromatogramList>\n"
    return s + "</run>\n</mzML>"


def indexed(inner, break_offsets=False):
    pre = '<?xml version="1.0" encoding="utf-8"?>\n<indexedmzML xmlns="http://psi.hupo.org/ms/mzml">\n'
    doc = pre + inner + "\n"
    b = doc.encode()

    def offsets(tag, pat):
        out = []
        pos = 0
        while True:
            i = b.find(tag, pos)
            if i < 0:
                break
            if b[i + len(tag):i + len(tag) + 1] == b" ":
                j = b.find(b'id="', i) + 4
                k = b.find(b'"', j)
                out.append((b[j:k].decode(), i + (7 if break_offsets else 0)))
            pos = i + 1
        return out

    sp = offsets(b"<spectrum", None)
    ch = offsets(b"<chromatogram", None)
    idx = '<indexList count="2">\n<index name="spectrum">\n'
    idx += "".join(f'<offset idRef="{i}">{o}</offset>\n' for i, o in sp)
    idx += '</index>\n<index name="chromatogram">\n'
    idx += "".join(f'<offset idRef="{i}">{o}</offset>\n' for i, o in ch)
    idx += "</index>\n</indexList>\n"
    pos = len(b)
    return doc + idx + f"<indexListOffset>{pos}</indexListOffset>\n<fileChecksum>0</fileChecksum>\n</indexedmzML>\n"


def write(name, text):
    with open(os.path.join(OUT, name), "w", encoding="utf-8", newline="\n") as f:
        f.write(text)


def mzml_fixtures():
    rng = np.random.default_rng(7)
    mz1 = np.linspace(100.0, 1000.0, 400)
    i1 = (np.exp(-((mz1 - 445.12) / 0.8) ** 2) * 1e6 + rng.uniform(0, 1e3, 400)).round(2)
    mz2 = np.array([110.071, 175.119, 262.151, 333.188, 445.33])
    i2 = np.array([120.0, 5000.0, 830.5, 77.0, 15000.0])
    mz3 = np.sort(rng.uniform(200, 1500, 300))
    i3 = rng.uniform(1, 1e5, 300).round(0)
    sp = [
        spectrum(0, "controllerType=0 controllerNumber=1 scan=1", mz1, i1, rt=1.5, mzenc=("f64", "zlib"),
                 intenc=("f64", "zlib"), group="Pos"),
        spectrum(1, "controllerType=0 controllerNumber=1 scan=2", mz2, i2, level=2, centroid=True, rt=1.52,
                 precursor=(445.33, 2), mzenc=("f32", "none"), intenc=("f32", "none"), group="Pos"),
        spectrum(2, "controllerType=0 controllerNumber=1 scan=3", mz3, i3, centroid=True, rt=1.6,
                 mzenc=("f64", "lin+z"), intenc=("f64", "slof+z"), group="Pos"),
    ]
    t = np.array([90.0, 91.2, 96.0])
    ch = [
        chromatogram(0, "TIC", t, np.array([float(np.sum(i1)), float(np.sum(i2)), float(np.sum(i3))]),
                     ("MS:1000235", "total ion current chromatogram")),
        chromatogram(1, "SRM SIC Q1=445.3 Q3=175.1", np.array([0.1, 0.2, 0.3, 0.4]),
                     np.array([1.0, 8.0, 3.0, 0.5]), ("MS:1001473", "selected reaction monitoring chromatogram"),
                     unit="minute", tenc=("f32", "none"), ienc=("f64", "none")),
    ]
    write("indexed.mzML", indexed(body(sp, ch)))
    write("broken_index.mzML", indexed(body(sp, ch), break_offsets=True))

    # Not indexed; every other encoding; no TIC chromatogram (built from the spectra; one
    # spectrum states no TIC and is summed); RT in seconds.
    mz4 = np.linspace(300.0, 310.0, 50)
    i4 = (np.abs(np.sin(mz4)) * 1000).round(0)
    sp2 = [
        spectrum(0, "scan=10", mz4, i4, rt=60.0, rt_unit="second", mzenc=("f64", "lin"), intenc=("f64", "pic")),
        spectrum(1, "scan=11", mz4 + 0.5, i4[::-1].copy(), rt=66.0, rt_unit="second", tic=False,
                 mzenc=("f64", "none"), intenc=("f64", "pic+z")),
        spectrum(2, "scan=12", mz4, i4, level=2, centroid=True, rt=67.5, rt_unit="second",
                 precursor=(305.5, 3), mzenc=("f64", "none"), intenc=("i32", "none")),
        spectrum(3, "scan=13", mz4, i4 * 2, rt=72.0, rt_unit="second", mzenc=("f32", "zlib"),
                 intenc=("i64", "zlib")),
        spectrum(4, "scan=14", mz4, i4, rt=78.0, rt_unit="second", mzenc=("f64", "none"), intenc=("f64", "slof")),
        spectrum(5, "scan=15", np.array([]), np.array([]), rt=80.0, rt_unit="second"),
    ]
    plain = '<?xml version="1.0" encoding="utf-8"?>\n' + body(sp2, [])
    write("plain.mzML", plain)
    # Cut inside the last spectrum but one.
    cut = plain.rfind('<spectrum index="4"') + 300
    write("truncated.mzML", plain[:cut])
    # zstd: refused by name when the page is read.
    sp3 = [spectrum(0, "scan=1", mz2, i2, mzenc=("f64", "zstd"), intenc=("f64", "none"))]
    write("zstd.mzML", '<?xml version="1.0"?>\n' + body(sp3, []))
    # imzML: belongs to the imzML reader.
    imz = '<?xml version="1.0"?>\n' + body(sp3, []).replace(
        '<cv id="UO"', '<cv id="IMS" fullName="Imaging MS Ontology" URI="https://www.maldi-msi.org/download/imzml/imagingMS.obo"/><cv id="UO"')
    write("imaging.mzML", imz)


def mzxml_peaks(mz, inten, precision, comp):
    dt = ">f4" if precision == 32 else ">f8"
    pairs = np.empty(len(mz) * 2, dtype=np.float64)
    pairs[0::2] = mz
    pairs[1::2] = inten
    raw = pairs.astype(dt).tobytes()
    if comp == "zlib":
        raw = zlib.compress(raw)
    return base64.b64encode(raw).decode(), len(raw)


def mzxml(precision, comp, with_index):
    mz1 = np.linspace(400.0, 420.0, 40)
    i1 = (np.cos(mz1) ** 2 * 5000).round(1)
    mz2 = np.array([150.1, 250.2, 350.3])
    i2 = np.array([10.0, 30.0, 20.0])
    ctype = f' compressionType="{comp}" compressedLen="{{clen}}"' if comp == "zlib" else ' compressionType="none" compressedLen="0"'

    def peaks(mz, inten):
        t, n = mzxml_peaks(mz, inten, precision, comp)
        return (f'<peaks precision="{precision}" byteOrder="network" pairOrder="m/z-int"'
                + ctype.replace("{clen}", str(n)) + f">{t}</peaks>")

    head = ('<?xml version="1.0" encoding="ISO-8859-1"?>\n'
            '<mzXML xmlns="http://sashimi.sourceforge.net/schema_revision/mzXML_3.2">\n'
            '<msRun scanCount="3" startTime="PT60S" endTime="PT2M">\n'
            '<parentFile fileName="file:///data/sample02.RAW" fileType="RAWData" fileSha1="0"/>\n'
            '<msInstrument msInstrumentID="1"><msManufacturer category="msManufacturer" value="Thermo Scientific"/>'
            '<msModel category="msModel" value="LTQ Orbitrap"/><msIonisation category="msIonisation" value="ESI"/>'
            '<msMassAnalyzer category="msMassAnalyzer" value="FTMS"/><msDetector category="msDetector" value="unknown"/>'
            '<software type="acquisition" name="Xcalibur" version="2.0"/></msInstrument>\n'
            '<dataProcessing centroided="0"><software type="conversion" name="gen_fixtures" version="1.0"/></dataProcessing>\n')
    scans = (f'<scan num="1" scanType="Full" centroided="0" msLevel="1" peaksCount="40" polarity="+" '
             f'retentionTime="PT60.5S" lowMz="400" highMz="420" basePeakMz="{float(mz1[np.argmax(i1)])!r}" '
             f'basePeakIntensity="{float(i1.max())!r}" totIonCurrent="{float(i1.sum())!r}">\n{peaks(mz1, i1)}\n'
             f'<scan num="2" scanType="Full" centroided="1" msLevel="2" peaksCount="3" polarity="+" '
             f'retentionTime="PT61S" totIonCurrent="60">\n<precursorMz precursorIntensity="100" precursorCharge="2" '
             f'activationMethod="CID">410.5</precursorMz>\n{peaks(mz2, i2)}\n</scan>\n</scan>\n'
             f'<scan num="3" centroided="0" msLevel="1" peaksCount="40" polarity="-" retentionTime="PT1M5S">\n'
             f'{peaks(mz1, i1[::-1].copy())}\n</scan>\n')
    doc = head + scans + "</msRun>\n"
    if with_index:
        b = doc.encode("latin-1")
        offs = []
        for num in ("1", "2", "3"):
            offs.append((num, b.find(f'<scan num="{num}"'.encode())))
        pos = len(b)
        doc += '<index name="scan">\n' + "".join(f'<offset id="{n}">{o}</offset>\n' for n, o in offs)
        doc += f"</index>\n<indexOffset>{pos}</indexOffset>\n<sha1>0</sha1>\n"
    return doc + "</mzXML>\n"


def main():
    os.makedirs(OUT, exist_ok=True)
    mzml_fixtures()
    write("scans32.mzXML", mzxml(32, "none", True))
    write("scans64z.mzXML", mzxml(64, "zlib", False))


if __name__ == "__main__":
    main()
