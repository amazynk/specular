"""Generates the DICOM video corpus of tools/corpus/fetch-dcmobj.sh.

Usage: dcmobj_make.py OUT_DIR

Streams come from ffmpeg (libx264, libx265, mpeg2video, mpeg4) on its synthetic sources and
are wrapped with pydicom as Video Endoscopic Image Storage objects (YBR_PARTIAL_420, Frame Time,
an empty Basic Offset Table). Also writes an encapsulated PDF and an encapsulated CDA. Each
file name says what it covers; `manifest.tsv` lists the transfer syntax and frame count.
"""

import os
import subprocess
import sys

import pydicom
from pydicom.dataset import Dataset, FileMetaDataset
from pydicom.uid import generate_uid

VIDEO_ENDOSCOPIC = "1.2.840.10008.5.1.4.1.1.77.1.1.1"
ENCAPSULATED_PDF = "1.2.840.10008.5.1.4.1.1.104.1"
ENCAPSULATED_CDA = "1.2.840.10008.5.1.4.1.1.104.2"
EXPLICIT_LE = "1.2.840.10008.1.2.1"
MPEG2_MAIN = "1.2.840.10008.1.2.4.100"
H264_HIGH_41 = "1.2.840.10008.1.2.4.102"
H264_HIGH_41_FRAGMENTABLE = "1.2.840.10008.1.2.4.102.1"
H264_BD = "1.2.840.10008.1.2.4.103"
HEVC_MAIN = "1.2.840.10008.1.2.4.107"
HEVC_MAIN10 = "1.2.840.10008.1.2.4.108"


def ffmpeg(out, src, *codec):
    cmd = ["ffmpeg", "-nostdin", "-hide_banner", "-loglevel", "error", "-y",
           "-f", "lavfi", "-i", src, *codec, out]
    subprocess.run(cmd, check=True)
    with open(out, "rb") as f:
        data = f.read()
    os.remove(out)
    return data


def item(b):
    return b"\xfe\xff\x00\xe0" + len(b).to_bytes(4, "little") + b


def encapsulate(stream, fragment):
    """An empty Basic Offset Table, then the stream in fragments of `fragment` bytes (even; 0
    for one fragment). Only the last fragment is padded."""
    if len(stream) % 2:
        stream += b"\0"
    parts = [stream] if fragment == 0 else [stream[i:i + fragment] for i in range(0, len(stream), fragment)]
    return item(b"") + b"".join(item(p) for p in parts)


def common(ds, cls):
    ds.SOPClassUID = cls
    ds.SOPInstanceUID = generate_uid()
    ds.StudyInstanceUID = generate_uid()
    ds.SeriesInstanceUID = generate_uid()
    ds.PatientName = "Synthetic^Video"
    ds.PatientID = "DCMOBJ"
    ds.StudyDescription = "Specular dcmobj corpus"
    ds.SeriesNumber = 1
    ds.InstanceNumber = 1


def save(path, ds, ts):
    meta = FileMetaDataset()
    meta.MediaStorageSOPClassUID = ds.SOPClassUID
    meta.MediaStorageSOPInstanceUID = ds.SOPInstanceUID
    meta.TransferSyntaxUID = ts
    ds.file_meta = meta
    ds.save_as(path, enforce_file_format=True)


def video(path, ts, stream, w, h, frames, frame_ms, bits=8, fragment=0, desc=""):
    ds = Dataset()
    common(ds, VIDEO_ENDOSCOPIC)
    ds.Modality = "ES"
    ds.SeriesDescription = desc
    ds.SamplesPerPixel = 3
    ds.PhotometricInterpretation = "YBR_PARTIAL_420"
    ds.PlanarConfiguration = 0
    ds.Rows = h
    ds.Columns = w
    ds.BitsAllocated = 8 if bits == 8 else 16
    ds.BitsStored = bits
    ds.HighBit = bits - 1
    ds.PixelRepresentation = 0
    ds.NumberOfFrames = frames
    ds.FrameTime = frame_ms
    ds.FrameIncrementPointer = 0x00181063
    ds.CineRate = round(1000 / frame_ms)
    ds.LossyImageCompression = "01"
    ds.add_new(0x7FE00010, "OB", encapsulate(stream, fragment))
    ds[0x7FE00010].is_undefined_length = True
    save(path, ds, ts)


def document(path, cls, mime, title, doc):
    ds = Dataset()
    common(ds, cls)
    ds.Modality = "DOC"
    ds.ConversionType = "WSD"
    ds.DocumentTitle = title
    ds.MIMETypeOfEncapsulatedDocument = mime
    ds.BurnedInAnnotation = "NO"
    if len(doc) % 2:
        doc += b"\0"
    ds.EncapsulatedDocumentLength = len(doc.rstrip(b"\0")) if mime == "application/pdf" else len(doc)
    ds.add_new(0x00420011, "OB", doc)
    save(path, ds, EXPLICIT_LE)


def pdf():
    """A two-page PDF with text and a filled shape (hand-written objects)."""
    objs = []
    pages = []
    for n in range(2):
        content = (b"0.85 0.2 0.1 rg 72 600 300 120 re f BT /F1 28 Tf 72 500 Td (Encapsulated page %d) Tj ET"
                   % (n + 1))
        objs.append(b"<< /Length %d >>\nstream\n%s\nendstream" % (len(content), content))
        pages.append(len(objs) + 4)  # page object numbers after catalog, pages, font
    # Objects: 1 catalog, 2 pages, 3 font, 4.. contents, then pages.
    body = [b"<< /Type /Catalog /Pages 2 0 R >>",
            b"<< /Type /Pages /Kids [%s] /Count 2 >>" % b" ".join(b"%d 0 R" % (6 + i) for i in range(2)),
            b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
            objs[0], objs[1]]
    for i in range(2):
        body.append(b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Contents %d 0 R "
                    b"/Resources << /Font << /F1 3 0 R >> >> >>" % (4 + i))
    out = b"%PDF-1.4\n"
    offs = []
    for i, o in enumerate(body):
        offs.append(len(out))
        out += b"%d 0 obj\n%s\nendobj\n" % (i + 1, o)
    xref = len(out)
    out += b"xref\n0 %d\n0000000000 65535 f \n" % (len(body) + 1)
    for o in offs:
        out += b"%010d 00000 n \n" % o
    out += b"trailer\n<< /Size %d /Root 1 0 R >>\nstartxref\n%d\n%%%%EOF\n" % (len(body) + 1, xref)
    return out


CDA = b"""<?xml version="1.0" encoding="UTF-8"?>
<ClinicalDocument xmlns="urn:hl7-org:v3">
  <title>Colonoscopy report</title>
  <recordTarget><patientRole><patient><name><given>Synthetic</given></name></patient></patientRole></recordTarget>
  <component><structuredBody>
    <component><section><title>Indication</title><text>Screening. Synthetic document for the Specular corpus.</text></section></component>
    <component><section><title>Findings</title><text><list><item>Caecum reached.</item><item>No polyps seen.</item></list></text></section></component>
    <component><section><title>Impression</title><paragraph>Normal examination.</paragraph></section></component>
  </structuredBody></component>
</ClinicalDocument>
"""


def main():
    out = sys.argv[1]
    os.makedirs(out, exist_ok=True)
    tmp = os.path.join(out, ".tmp")
    os.makedirs(tmp, exist_ok=True)
    manifest = []

    def raw(name, ext, src, *codec):
        return ffmpeg(os.path.join(tmp, name + ext), src, *codec)

    def add(name, ts, frames, **kw):
        manifest.append((name, ts, frames))
        return os.path.join(out, name)

    src1080 = "testsrc2=size=1920x1080:rate=30,format=yuv420p"
    bt709 = ["-colorspace", "bt709", "-color_primaries", "bt709", "-color_trc", "bt709",
             "-color_range", "tv"]

    # 1080p30 High profile, B-frames (pyramid), IDR every 30 frames: the timing file.
    s = raw("h264_1080p", ".h264", src1080, "-frames:v", "150", "-c:v", "libx264", "-preset",
            "medium", "-profile:v", "high", "-level", "4.1", "-bf", "3", "-g", "30",
            "-keyint_min", "30", "-sc_threshold", "0", "-crf", "20", *bt709)
    video(add("h264_1080p_bframes.dcm", H264_HIGH_41, 150), H264_HIGH_41, s, 1920, 1080, 150,
          1000 / 30, desc="1080p30 H.264 High, B-frames")

    # 720p, fragmentable syntax, the stream in 8 KiB fragments, open GOP (no IDR after the first).
    s = raw("h264_720p", ".h264", "testsrc=size=1280x720:rate=25,format=yuv420p", "-frames:v",
            "60", "-c:v", "libx264", "-preset", "fast", "-bf", "2", "-g", "20",
            "-x264-params", "open-gop=1", "-crf", "22", *bt709)
    video(add("h264_720p_fragments.dcm", H264_HIGH_41_FRAGMENTABLE, 60),
          H264_HIGH_41_FRAGMENTABLE, s, 1280, 720, 60, 40.0, fragment=8192,
          desc="720p25 H.264, 8 KiB fragments, open GOP")

    # 576i interlaced (MBAFF), SD with no colour description (BT.601 by default).
    s = raw("h264_576i", ".h264", "testsrc2=size=720x576:rate=25,format=yuv420p", "-frames:v",
            "40", "-c:v", "libx264", "-preset", "fast", "-flags", "+ildct+ilme",
            "-x264-params", "tff=1", "-g", "25", "-crf", "20")
    video(add("h264_576i_mbaff.dcm", H264_HIGH_41, 40), H264_HIGH_41, s, 720, 576, 40, 40.0,
          desc="576i25 H.264 MBAFF")

    # An MP4 file in the pixel data (seen in the wild) instead of an elementary stream.
    s = raw("h264_mp4", ".mp4", "testsrc2=size=640x360:rate=30,format=yuv420p", "-frames:v",
            "45", "-c:v", "libx264", "-preset", "fast", "-bf", "2", "-g", "15", "-crf", "22",
            "-movflags", "+faststart", *bt709)
    video(add("h264_in_mp4.dcm", H264_BD, 45), H264_BD, s, 640, 360, 45, 1000 / 30,
          desc="H.264 in an MP4 file")

    # HEVC Main and Main 10 with B-frames, closed GOP.
    for name, ts, fmt, bits, prof in [("hevc_main_720p.dcm", HEVC_MAIN, "yuv420p", 8, "main"),
                                      ("hevc_main10_720p.dcm", HEVC_MAIN10, "yuv420p10le", 10, "main10")]:
        s = raw(name, ".hevc", "testsrc2=size=1280x720:rate=25,format=" + fmt, "-frames:v", "50",
                "-c:v", "libx265", "-preset", "fast", "-profile:v", prof,
                "-x265-params", "log-level=error:keyint=25:min-keyint=25:bframes=3:open-gop=0",
                "-crf", "24", *bt709)
        video(add(name, ts, 50), ts, s, 1280, 720, 50, 40.0, bits=bits,
              desc="720p25 HEVC " + prof)

    # Refused: MPEG-2, and MPEG-4 Part 2 labelled as H.264.
    s = raw("mpeg2", ".m2v", "testsrc=size=352x288:rate=25,format=yuv420p", "-frames:v", "10",
            "-c:v", "mpeg2video", "-q:v", "4")
    video(add("refused_mpeg2.dcm", MPEG2_MAIN, 10), MPEG2_MAIN, s, 352, 288, 10, 40.0,
          desc="MPEG-2 (refused)")
    s = raw("mpeg4", ".m4v", "testsrc=size=352x288:rate=25,format=yuv420p", "-frames:v", "10",
            "-c:v", "mpeg4", "-q:v", "4", "-f", "m4v")
    video(add("refused_mpeg4_as_h264.dcm", H264_HIGH_41, 10), H264_HIGH_41, s, 352, 288, 10,
          40.0, desc="MPEG-4 Part 2 labelled H.264 (refused)")

    s = raw("mpeg4mp4", ".mp4", "testsrc=size=352x288:rate=25,format=yuv420p", "-frames:v", "10",
            "-c:v", "mpeg4", "-q:v", "4")
    video(add("refused_mpeg4_mp4_as_h264.dcm", H264_HIGH_41, 10), H264_HIGH_41, s, 352, 288, 10,
          40.0, desc="MPEG-4 Part 2 in an MP4 file labelled H.264 (refused)")

    document(add("encapsulated_pdf.dcm", "", 0), ENCAPSULATED_PDF, "application/pdf",
             "Procedure summary", pdf())
    document(add("encapsulated_cda.dcm", "", 0), ENCAPSULATED_CDA, "text/xml",
             "Colonoscopy report", CDA)

    os.rmdir(tmp)
    with open(os.path.join(out, "manifest.tsv"), "w") as f:
        f.write("file\ttransfer_syntax\tframes\n")
        for name, ts, frames in manifest:
            f.write("%s\t%s\t%d\n" % (name, ts, frames))


if __name__ == "__main__":
    main()
