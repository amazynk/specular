"""Writes the synthetic fixtures of tests/dicomrt.rs into tests/data/dicomrt/.

Usage: dicomrt_make.py OUT_DIR   (normally tests/data/dicomrt)

Needs pydicom, numpy and highdicom (test tools only, never dependencies of Specular), e.g. in a
venv under target/:

    python3 -m venv target/dicomrt/venv
    target/dicomrt/venv/bin/pip install pydicom==3.0.2 numpy highdicom==0.28.2 \
        dicompyler-core==0.5.6
    target/dicomrt/venv/bin/python -I gen/dicomrt_make.py tests/data/dicomrt

UIDs are derived from the file names, so a rerun writes the same files.

study/   a 32 × 32 × 10 CT (4 mm pixels, 5 mm slices), an RT Structure Set on it (body, a round
         PTV, an organ at risk partly beyond the dose grid, a ring with a hole), an RT Plan
         (56 Gy prescription), an RT Dose on its own grid (4.5 mm pixels, 3 mm frames: some
         structure planes fall on a frame, some between, two beyond), the same dose with an
         absolute Grid Frame Offset Vector, and structured reports on the CT: Basic Text,
         Enhanced (SCOORD on an image, TCOORD on a waveform), Comprehensive (by-reference
         items, a TABLE-free tree of every value type) and two TID 1500 measurement reports
         from highdicom (planar SCOORD and SCOORD3D regions).
ecg/     a 12-lead ECG (10 s at 250 Hz, 16-bit signed, µV), a General ECG (3 leads, 8-bit
         signed, implicit VR), a haemodynamic waveform (2 channels in mmHg, 16-bit unsigned
         with baseline and correction factors, plus an ECG group) and a µ-law audio channel.
oct/     an Ophthalmic Tomography volume (8 B-scans) located on an Ophthalmic Photography
         image by per-frame Ophthalmic Frame Location items.
"""

import math
import os
import sys

import numpy as np
import pydicom
from pydicom.dataset import Dataset, FileMetaDataset
from pydicom.sequence import Sequence
from pydicom.uid import generate_uid

EXPLICIT_LE = "1.2.840.10008.1.2.1"
IMPLICIT_LE = "1.2.840.10008.1.2"
CT = "1.2.840.10008.5.1.4.1.1.2"
RTSTRUCT = "1.2.840.10008.5.1.4.1.1.481.3"
RTDOSE = "1.2.840.10008.5.1.4.1.1.481.2"
RTPLAN = "1.2.840.10008.5.1.4.1.1.481.5"
BASIC_SR = "1.2.840.10008.5.1.4.1.1.88.11"
ENHANCED_SR = "1.2.840.10008.5.1.4.1.1.88.22"
COMPREHENSIVE_SR = "1.2.840.10008.5.1.4.1.1.88.33"
TWELVE_LEAD = "1.2.840.10008.5.1.4.1.1.9.1.1"
GENERAL_ECG = "1.2.840.10008.5.1.4.1.1.9.1.2"
HEMODYNAMIC = "1.2.840.10008.5.1.4.1.1.9.2.1"
BASIC_VOICE = "1.2.840.10008.5.1.4.1.1.9.4.1"
OPT = "1.2.840.10008.5.1.4.1.1.77.1.5.4"
OP8 = "1.2.840.10008.5.1.4.1.1.77.1.5.1"

PREFIX = "2.25."


def uid(name):
    return generate_uid(prefix=None, entropy_srcs=["specular-dicomrt", name])


STUDY = uid("study")
FOR = uid("frame-of-reference")
CT_SERIES = uid("ct-series")


def common(ds, cls, name, modality, series, number=1):
    ds.SpecificCharacterSet = "ISO_IR 192"
    ds.SOPClassUID = cls
    ds.SOPInstanceUID = uid(name)
    ds.StudyInstanceUID = STUDY
    ds.SeriesInstanceUID = series
    ds.PatientName = "Synthetic^Phantom"
    ds.PatientID = "DICOMRT"
    ds.PatientBirthDate = ""
    ds.PatientSex = "O"
    ds.StudyDate = "20260101"
    ds.StudyTime = "120000"
    ds.StudyID = "1"
    ds.AccessionNumber = ""
    ds.ReferringPhysicianName = ""
    ds.Modality = modality
    ds.SeriesNumber = 1
    ds.InstanceNumber = number
    ds.Manufacturer = "Specular test fixtures"
    ds.StudyDescription = "Specular dicomrt fixtures"


def save(out, rel, ds, ts=EXPLICIT_LE):
    meta = FileMetaDataset()
    meta.MediaStorageSOPClassUID = ds.SOPClassUID
    meta.MediaStorageSOPInstanceUID = ds.SOPInstanceUID
    meta.TransferSyntaxUID = ts
    meta.ImplementationClassUID = uid("implementation")
    ds.file_meta = meta
    path = os.path.join(out, rel)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    ds.save_as(path, enforce_file_format=True, implicit_vr=ts == IMPLICIT_LE, little_endian=True)


def code(value, scheme, meaning):
    c = Dataset()
    c.CodeValue = value
    c.CodingSchemeDesignator = scheme
    c.CodeMeaning = meaning
    return c


# ------------------------------------------------------------------------------------------ CT

CT_N = 32
CT_PX = 4.0
CT_SLICES = 10
CT_DZ = 5.0
CT_X0 = -62.0


def ct(out):
    uids = []
    yy, xx = np.mgrid[0:CT_N, 0:CT_N]
    x = CT_X0 + xx * CT_PX
    y = CT_X0 + yy * CT_PX
    for k in range(CT_SLICES):
        z = k * CT_DZ
        name = "ct-%d" % k
        ds = Dataset()
        common(ds, CT, name, "CT", CT_SERIES, k + 1)
        ds.SeriesDescription = "Phantom CT"
        ds.FrameOfReferenceUID = FOR
        ds.PositionReferenceIndicator = ""
        ds.ImagePositionPatient = [CT_X0, CT_X0, z]
        ds.ImageOrientationPatient = [1, 0, 0, 0, 1, 0]
        ds.PixelSpacing = [CT_PX, CT_PX]
        ds.SliceThickness = CT_DZ
        ds.Rows = CT_N
        ds.Columns = CT_N
        ds.SamplesPerPixel = 1
        ds.PhotometricInterpretation = "MONOCHROME2"
        ds.BitsAllocated = 16
        ds.BitsStored = 16
        ds.HighBit = 15
        ds.PixelRepresentation = 1
        ds.RescaleIntercept = 0
        ds.RescaleSlope = 1
        ds.RescaleType = "HU"
        ds.WindowCenter = 40
        ds.WindowWidth = 400
        body = (x / 55.0) ** 2 + (y / 45.0) ** 2 <= 1.0
        img = np.where(body, 20, -1000).astype(np.int16)
        tumour = (x ** 2 + y ** 2 + (z - 22.0) ** 2) <= 15.0 ** 2
        img[tumour] = 60
        ds.PixelData = img.tobytes()
        save(out, "study/ct_%02d.dcm" % k, ds)
        uids.append(ds.SOPInstanceUID)
    return uids


# ------------------------------------------------------------------------------------ RTSTRUCT


def circle(cx, cy, r, z, n=24, phase=0.13):
    pts = []
    for i in range(n):
        a = 2 * math.pi * i / n + phase
        pts += [round(cx + r * math.cos(a), 3), round(cy + r * math.sin(a), 3), z]
    return pts


def rect(x0, y0, x1, y1, z):
    return [x0, y0, z, x1, y0, z, x1, y1, z, x0, y1, z]


def rtstruct(out, ct_uids):
    ds = Dataset()
    common(ds, RTSTRUCT, "rtstruct", "RTSTRUCT", uid("rtstruct-series"))
    ds.StructureSetLabel = "Phantom"
    ds.StructureSetDate = "20260101"
    ds.StructureSetTime = "120000"
    ref_for = Dataset()
    ref_for.FrameOfReferenceUID = FOR
    study = Dataset()
    study.ReferencedSOPClassUID = "1.2.840.10008.3.1.2.3.1"
    study.ReferencedSOPInstanceUID = STUDY
    series = Dataset()
    series.SeriesInstanceUID = CT_SERIES
    series.ContourImageSequence = Sequence(
        [Dataset() for _ in ct_uids])
    for item, u in zip(series.ContourImageSequence, ct_uids):
        item.ReferencedSOPClassUID = CT
        item.ReferencedSOPInstanceUID = u
    study.RTReferencedSeriesSequence = Sequence([series])
    ref_for.RTReferencedStudySequence = Sequence([study])
    ds.ReferencedFrameOfReferenceSequence = Sequence([ref_for])

    rois = []
    # (number, name, colour, type, [(z, [contour points...]), ...])
    body = [(k * CT_DZ, [circle(0.3, 0.2, 44.7, k * CT_DZ, 32)]) for k in range(CT_SLICES)]
    ptv = [(z, [circle(0.4, -0.3, 15.1 - abs(z - 22.0) * 0.3, z)])
           for z in (10.0, 15.0, 20.0, 25.0, 30.0, 35.0)]
    oar = [(z, [rect(18.3, -12.2, 33.7, 7.6, z)]) for z in (0.0, 5.0, 10.0, 40.0, 45.0)]
    ring = [(z, [circle(-20.2, 18.1, 14.3, z), circle(-20.2, 18.1, 6.2, z)])
            for z in (15.0, 20.0, 25.0)]
    rois = [(1, "BODY", [0, 255, 0], "EXTERNAL", body),
            (2, "PTV", [255, 0, 0], "PTV", ptv),
            (3, "Spinal cord", [0, 128, 255], "ORGAN", oar),
            (4, "Ring", [255, 200, 0], "ORGAN", ring)]
    ssroi, contours, obs = [], [], []
    for number, name, colour, kind, planes in rois:
        s = Dataset()
        s.ROINumber = number
        s.ReferencedFrameOfReferenceUID = FOR
        s.ROIName = name
        s.ROIGenerationAlgorithm = "MANUAL"
        ssroi.append(s)
        c = Dataset()
        c.ROIDisplayColor = colour
        c.ReferencedROINumber = number
        seq = []
        for z, polys in planes:
            for p in polys:
                item = Dataset()
                item.ContourGeometricType = "CLOSED_PLANAR"
                item.NumberOfContourPoints = len(p) // 3
                item.ContourData = [float(v) for v in p]
                seq.append(item)
        c.ContourSequence = Sequence(seq)
        contours.append(c)
        o = Dataset()
        o.ObservationNumber = number
        o.ReferencedROINumber = number
        o.RTROIInterpretedType = kind
        o.ROIInterpreter = ""
        obs.append(o)
    ds.StructureSetROISequence = Sequence(ssroi)
    ds.ROIContourSequence = Sequence(contours)
    ds.RTROIObservationsSequence = Sequence(obs)
    save(out, "study/rtstruct.dcm", ds)


# -------------------------------------------------------------------------------- RTPLAN, DOSE

PRESCRIPTION = 56.0


def rtplan(out):
    ds = Dataset()
    common(ds, RTPLAN, "rtplan", "RTPLAN", uid("rtplan-series"))
    ds.RTPlanLabel = "Phantom plan"
    ds.RTPlanGeometry = "PATIENT"
    ref = Dataset()
    ref.ReferencedSOPClassUID = RTSTRUCT
    ref.ReferencedSOPInstanceUID = uid("rtstruct")
    ds.ReferencedStructureSetSequence = Sequence([ref])
    dr = Dataset()
    dr.DoseReferenceNumber = 1
    dr.DoseReferenceStructureType = "SITE"
    dr.DoseReferenceType = "TARGET"
    dr.TargetPrescriptionDose = PRESCRIPTION
    dr2 = Dataset()
    dr2.DoseReferenceNumber = 2
    dr2.DoseReferenceStructureType = "SITE"
    dr2.DoseReferenceType = "ORGAN_AT_RISK"
    dr2.OrganAtRiskMaximumDose = 45.0
    ds.DoseReferenceSequence = Sequence([dr, dr2])
    fg = Dataset()
    fg.FractionGroupNumber = 1
    fg.NumberOfFractionsPlanned = 28
    fg.NumberOfBeams = 0
    fg.NumberOfBrachyApplicationSetups = 0
    ds.FractionGroupSequence = Sequence([fg])
    save(out, "study/rtplan.dcm", ds)


DOSE_N = 26
DOSE_PX = 4.5
DOSE_X0 = -56.25
DOSE_Z0 = 1.0
DOSE_DZ = 3.0
DOSE_FRAMES = 15
DOSE_SCALING = 2.5e-5


def dose_values():
    zz, yy, xx = np.mgrid[0:DOSE_FRAMES, 0:DOSE_N, 0:DOSE_N]
    x = DOSE_X0 + xx * DOSE_PX
    y = DOSE_X0 + yy * DOSE_PX
    z = DOSE_Z0 + zz * DOSE_DZ
    r2 = x ** 2 + y ** 2 + ((z - 22.0) * 1.3) ** 2
    gy = 60.0 * np.exp(-r2 / (2 * 22.0 ** 2)) + 2.0 * np.exp(-((x - 25) ** 2) / 400.0)
    return np.round(gy / DOSE_SCALING).astype(np.uint32)


def rtdose(out, name, absolute):
    ds = Dataset()
    common(ds, RTDOSE, name, "RTDOSE", uid("rtdose-series"))
    ds.SeriesDescription = "Plan dose" + (" (absolute offsets)" if absolute else "")
    ds.FrameOfReferenceUID = FOR
    ds.ImagePositionPatient = [DOSE_X0, DOSE_X0, DOSE_Z0]
    ds.ImageOrientationPatient = [1, 0, 0, 0, 1, 0]
    ds.PixelSpacing = [DOSE_PX, DOSE_PX]
    ds.SliceThickness = ""
    ds.Rows = DOSE_N
    ds.Columns = DOSE_N
    ds.NumberOfFrames = DOSE_FRAMES
    ds.FrameIncrementPointer = 0x3004000C
    offsets = [DOSE_Z0 + k * DOSE_DZ for k in range(DOSE_FRAMES)] if absolute else \
        [k * DOSE_DZ for k in range(DOSE_FRAMES)]
    ds.GridFrameOffsetVector = offsets
    ds.SamplesPerPixel = 1
    ds.PhotometricInterpretation = "MONOCHROME2"
    ds.BitsAllocated = 32
    ds.BitsStored = 32
    ds.HighBit = 31
    ds.PixelRepresentation = 0
    ds.DoseUnits = "GY"
    ds.DoseType = "PHYSICAL"
    ds.DoseSummationType = "PLAN"
    ds.DoseGridScaling = DOSE_SCALING
    plan = Dataset()
    plan.ReferencedSOPClassUID = RTPLAN
    plan.ReferencedSOPInstanceUID = uid("rtplan")
    ds.ReferencedRTPlanSequence = Sequence([plan])
    ds.PixelData = dose_values().tobytes()
    save(out, "study/%s.dcm" % name, ds)


# ------------------------------------------------------------------------------------------ SR


def item(rel, vt, concept, **kw):
    it = Dataset()
    if rel:
        it.RelationshipType = rel
    it.ValueType = vt
    if concept is not None:
        it.ConceptNameCodeSequence = Sequence([code(*concept)])
    for k, v in kw.items():
        setattr(it, k, v)
    return it


def num(rel, concept, value, unit):
    mv = Dataset()
    mv.NumericValue = value
    mv.MeasurementUnitsCodeSequence = Sequence([code(*unit)])
    return item(rel, "NUM", concept, MeasuredValueSequence=Sequence([mv]))


def image_ref(rel, sop_uid, frames=None, cls=CT):
    r = Dataset()
    r.ReferencedSOPClassUID = cls
    r.ReferencedSOPInstanceUID = sop_uid
    if frames:
        r.ReferencedFrameNumber = frames
    vt = "IMAGE" if cls not in (TWELVE_LEAD,) else "WAVEFORM"
    return item(rel, vt, None, ReferencedSOPSequence=Sequence([r]))


def sr_document(out, rel, cls, name, title, content, template=None):
    ds = Dataset()
    common(ds, cls, name, "SR", uid(name + "-series"))
    ds.SeriesDescription = title[2]
    ds.ContentDate = "20260102"
    ds.ContentTime = "093000"
    ds.ValueType = "CONTAINER"
    ds.ConceptNameCodeSequence = Sequence([code(*title)])
    ds.ContinuityOfContent = "SEPARATE"
    if template:
        t = Dataset()
        t.MappingResource = "DCMR"
        t.TemplateIdentifier = template
        ds.ContentTemplateSequence = Sequence([t])
    ds.CompletionFlag = "COMPLETE"
    ds.VerificationFlag = "UNVERIFIED"
    ds.ContentSequence = Sequence(content)
    save(out, rel, ds)


def srs(out, ct_uids):
    sr_document(out, "study/sr_basic.dcm", BASIC_SR, "sr-basic",
                ("11528-7", "LN", "Radiology Report"), [
                    item("HAS CONCEPT MOD", "CODE", ("121049", "DCM", "Language of Content Item and Descendants"),
                         ConceptCodeSequence=Sequence([code("en-US", "RFC5646", "English (United States)")])),
                    item("HAS OBS CONTEXT", "PNAME", ("121008", "DCM", "Person Observer Name"),
                         PersonName="Curie^Marie^^Dr"),
                    item("HAS OBS CONTEXT", "DATE", ("111060", "DCM", "Study Date"), Date="20260101"),
                    item("HAS OBS CONTEXT", "TIME", ("111061", "DCM", "Study Time"), Time="120000"),
                    item("HAS OBS CONTEXT", "DATETIME", ("111526", "DCM", "DateTime Started"),
                         DateTime="20260101120000"),
                    item("HAS OBS CONTEXT", "UIDREF", ("110180", "DCM", "Study Instance UID"), UID=STUDY),
                    item("CONTAINS", "CONTAINER", ("121070", "DCM", "Findings"), ContinuityOfContent="SEPARATE",
                         ContentSequence=Sequence([
                             item("CONTAINS", "TEXT", ("121071", "DCM", "Finding"),
                                  TextValue="Round lesion in the mid abdomen, 30 mm. Größe stabil."),
                         ])),
                    item("CONTAINS", "CONTAINER", ("121076", "DCM", "Conclusions"), ContinuityOfContent="SEPARATE",
                         ContentSequence=Sequence([
                             item("CONTAINS", "CODE", ("121077", "DCM", "Conclusion"),
                                  ConceptCodeSequence=Sequence([code("108369006", "SCT", "Neoplasm")])),
                         ])),
                ])

    scoord = item("CONTAINS", "SCOORD", ("111030", "DCM", "Image Region"), GraphicType="POLYLINE",
                  GraphicData=[10.5, 12.0, 20.25, 12.0, 20.25, 19.5, 10.5, 19.5, 10.5, 12.0],
                  ContentSequence=Sequence([image_ref("SELECTED FROM", ct_uids[4])]))
    point = item("CONTAINS", "SCOORD", ("111010", "DCM", "Center"), GraphicType="POINT",
                 GraphicData=[16.0, 15.5],
                 ContentSequence=Sequence([image_ref("SELECTED FROM", ct_uids[5])]))
    sr_document(out, "study/sr_enhanced.dcm", ENHANCED_SR, "sr-enhanced",
                ("18748-4", "LN", "Diagnostic Imaging Report"), [
                    item("CONTAINS", "CONTAINER", ("121070", "DCM", "Findings"), ContinuityOfContent="SEPARATE",
                         ContentSequence=Sequence([
                             num("CONTAINS", ("410668003", "SCT", "Length"), "30.5", ("mm", "UCUM", "millimeter")),
                             num("CONTAINS", ("42798000", "SCT", "Area"), "706.86", ("mm2", "UCUM", "square millimeter")),
                             num("CONTAINS", ("112031", "DCM", "Attenuation Coefficient"), "60",
                                 ("[hnsf'U]", "UCUM", "Hounsfield unit")),
                             num("CONTAINS", ("C0205160", "UMLS", "Ratio"), "0.75", ("1", "UCUM", "no units")),
                             scoord, point,
                             item("CONTAINS", "TCOORD", ("122142", "DCM", "Acquisition Time"),
                                  TemporalRangeType="SEGMENT", ReferencedSamplePositions=[100, 350],
                                  ContentSequence=Sequence([image_ref("SELECTED FROM", uid("ecg-12lead"),
                                                                      cls=TWELVE_LEAD)])),
                             image_ref("CONTAINS", ct_uids[4]),
                         ])),
                ])

    # Comprehensive: by-reference items and a measurement with a qualifier but no value.
    finding = item("CONTAINS", "CODE", ("121071", "DCM", "Finding"),
                   ConceptCodeSequence=Sequence([code("4147007", "SCT", "Mass")]),
                   ContentSequence=Sequence([
                       num("HAS PROPERTIES", ("410668003", "SCT", "Length"), "30.5", ("mm", "UCUM", "millimeter")),
                   ]))
    failed = item("CONTAINS", "NUM", ("103339001", "SCT", "Long axis"),
                  MeasuredValueSequence=Sequence([]),
                  NumericValueQualifierCodeSequence=Sequence([code("114006", "DCM", "Measurement failure")]))
    by_ref = Dataset()
    by_ref.RelationshipType = "INFERRED FROM"
    by_ref.ReferencedContentItemIdentifier = [1, 1, 1]
    conclusion = item("CONTAINS", "CODE", ("121077", "DCM", "Conclusion"),
                      ConceptCodeSequence=Sequence([code("108369006", "SCT", "Neoplasm")]),
                      ContentSequence=Sequence([by_ref]))
    sr_document(out, "study/sr_comprehensive.dcm", COMPREHENSIVE_SR, "sr-comprehensive",
                ("18748-4", "LN", "Diagnostic Imaging Report"), [
                    item("CONTAINS", "CONTAINER", ("121070", "DCM", "Findings"), ContinuityOfContent="SEPARATE",
                         ContentSequence=Sequence([finding, failed, scoord])),
                    item("CONTAINS", "CONTAINER", ("121076", "DCM", "Conclusions"), ContinuityOfContent="SEPARATE",
                         ContentSequence=Sequence([conclusion])),
                ])
    tid1500(out)


def tid1500(out):
    """TID 1500 measurement reports written by highdicom on the CT."""
    import highdicom as hd
    from highdicom.sr import (CodedConcept, Comprehensive3DSR, FindingSite, ImageRegion,
                              ImageRegion3D, Measurement, MeasurementReport,
                              ObservationContext, ObserverContext, PersonObserverIdentifyingAttributes,
                              PlanarROIMeasurementsAndQualitativeEvaluations, GraphicTypeValues,
                              GraphicTypeValues3D, SourceImageForRegion, TrackingIdentifier)
    from pydicom.sr.codedict import codes

    images = [pydicom.dcmread(os.path.join(out, "study/ct_%02d.dcm" % k)) for k in range(CT_SLICES)]
    observer = ObserverContext(
        observer_type=codes.DCM.Person,
        observer_identifying_attributes=PersonObserverIdentifyingAttributes(name="Doe^Jane"))
    context = ObservationContext(observer_person_context=observer)

    def measurement(name, value, unit):
        return Measurement(name=name, value=value, unit=unit)

    groups = []
    region = ImageRegion(
        graphic_type=GraphicTypeValues.POLYLINE,
        graphic_data=np.array([[10.5, 12.0], [20.25, 12.0], [20.25, 19.5], [10.5, 19.5], [10.5, 12.0]]),
        source_image=SourceImageForRegion.from_source_image(images[4]))
    groups.append(PlanarROIMeasurementsAndQualitativeEvaluations(
        tracking_identifier=TrackingIdentifier(uid=uid("track-1"), identifier="Lesion 1"),
        referenced_region=region,
        finding_type=CodedConcept("108369006", "SCT", "Neoplasm"),
        finding_sites=[FindingSite(CodedConcept("818981001", "SCT", "Abdomen"))],
        measurements=[
            measurement(CodedConcept("410668003", "SCT", "Length"), 30.5, codes.UCUM.Millimeter),
            measurement(CodedConcept("42798000", "SCT", "Area"), 706.86, CodedConcept("mm2", "UCUM", "square millimeter")),
        ]))
    report = MeasurementReport(
        observation_context=context,
        procedure_reported=CodedConcept("77477000", "SCT", "Computerized tomography"),
        imaging_measurements=groups)
    sr = Comprehensive3DSR(
        evidence=[images[4]], content=report[0], series_instance_uid=uid("sr-tid1500-series"),
        series_number=10, sop_instance_uid=uid("sr-tid1500"), instance_number=1,
        manufacturer="Specular test fixtures", institution_name="", is_complete=True)
    path = os.path.join(out, "study/sr_tid1500.dcm")
    sr.save_as(path, enforce_file_format=True)

    region3d = ImageRegion3D(
        graphic_type=GraphicTypeValues3D.POLYGON,
        graphic_data=np.array([[-20.0, -20.0, 20.0], [20.0, -20.0, 20.0], [20.0, 20.0, 20.0],
                               [-20.0, 20.0, 20.0], [-20.0, -20.0, 20.0]]),
        frame_of_reference_uid=FOR)
    group3d = PlanarROIMeasurementsAndQualitativeEvaluations(
        tracking_identifier=TrackingIdentifier(uid=uid("track-2"), identifier="Lesion 2"),
        referenced_region=region3d,
        finding_type=CodedConcept("108369006", "SCT", "Neoplasm"),
        measurements=[measurement(CodedConcept("42798000", "SCT", "Area"), 1600.0,
                                  CodedConcept("mm2", "UCUM", "square millimeter"))])
    report3d = MeasurementReport(
        observation_context=context,
        procedure_reported=CodedConcept("77477000", "SCT", "Computerized tomography"),
        imaging_measurements=[group3d])
    sr3d = Comprehensive3DSR(
        evidence=[images[4]], content=report3d[0], series_instance_uid=uid("sr-tid1500-3d-series"),
        series_number=11, sop_instance_uid=uid("sr-tid1500-3d"), instance_number=1,
        manufacturer="Specular test fixtures", institution_name="", is_complete=True)
    sr3d.save_as(os.path.join(out, "study/sr_tid1500_3d.dcm"), enforce_file_format=True)


# ----------------------------------------------------------------------------------- waveforms

LEADS = [("I", "2:1"), ("II", "2:2"), ("III", "2:61"), ("aVR", "2:62"), ("aVL", "2:63"),
         ("aVF", "2:64"), ("V1", "2:3"), ("V2", "2:4"), ("V3", "2:5"), ("V4", "2:6"),
         ("V5", "2:7"), ("V6", "2:8")]


def beat(t, rr):
    """A synthetic P-QRS-T complex (mV) at times t (s) for RR interval rr."""
    ph = np.mod(t, rr)
    g = lambda c, w, a: a * np.exp(-((ph - c) ** 2) / (2 * w * w))
    return (g(0.10, 0.025, 0.12) + g(0.235, 0.008, -0.10) + g(0.25, 0.010, 1.1)
            + g(0.265, 0.008, -0.25) + g(0.45, 0.04, 0.3))


def channel_def(label, source, units, sensitivity, baseline=0.0, correction=1.0, bits=16, skew="0"):
    ch = Dataset()
    ch.ChannelLabel = label
    ch.ChannelStatus = "OK"
    ch.ChannelSourceSequence = Sequence([code(*source)])
    ch.ChannelSensitivity = sensitivity
    ch.ChannelSensitivityUnitsSequence = Sequence([code(*units)])
    ch.ChannelSensitivityCorrectionFactor = correction
    ch.ChannelBaseline = baseline
    ch.ChannelTimeSkew = skew
    ch.WaveformBitsStored = bits
    ch.FilterLowFrequency = "0.05"
    ch.FilterHighFrequency = "150"
    return ch


def wave_group(label, freq, samples, defs, data, bits, interp, vr):
    g = Dataset()
    g.MultiplexGroupTimeOffset = "0"
    g.WaveformOriginality = "ORIGINAL"
    g.NumberOfWaveformChannels = len(defs)
    g.NumberOfWaveformSamples = samples
    g.SamplingFrequency = freq
    g.MultiplexGroupLabel = label
    g.ChannelDefinitionSequence = Sequence(defs)
    g.WaveformBitsAllocated = bits
    g.WaveformSampleInterpretation = interp
    g.add_new(0x54001010, vr, data)
    return g


def waveform_doc(out, rel, cls, name, groups, ts=EXPLICIT_LE, annotations=None):
    ds = Dataset()
    common(ds, cls, name, "ECG" if cls in (TWELVE_LEAD, GENERAL_ECG) else
           ("HD" if cls == HEMODYNAMIC else "AU"), uid(name + "-series"))
    ds.ContentDate = "20260101"
    ds.ContentTime = "120000"
    ds.AcquisitionDateTime = "20260101120000"
    ds.WaveformSequence = Sequence(groups)
    if annotations:
        ds.WaveformAnnotationSequence = Sequence(annotations)
    save(out, rel, ds, ts)


def ecgs(out):
    freq, seconds = 250, 10
    n = freq * seconds
    t = np.arange(n) / freq
    rr = 0.8
    base = beat(t, rr)
    # Einthoven / Goldberger relations for limb leads; precordial from the same complex.
    lead_i = 0.6 * base
    lead_ii = 1.0 * base + 0.03 * np.sin(2 * np.pi * 0.3 * t)
    lead_iii = lead_ii - lead_i
    avr = -(lead_i + lead_ii) / 2
    avl = lead_i - lead_ii / 2
    avf = lead_ii - lead_i / 2
    pre = [-0.6, -0.2, 0.3, 0.9, 1.1, 0.8]
    sig = [lead_i, lead_ii, lead_iii, avr, avl, avf] + [p * base for p in pre]
    # 2.5 µV per unit: µV = raw × 2.5.
    raw = np.stack([np.round(s * 1000 / 2.5) for s in sig], axis=1).astype(np.int16)
    defs = [channel_def("Lead " + name, (src, "MDC", "Lead " + name), ("uV", "UCUM", "microvolt"),
                        "2.5") for name, src in LEADS]
    ann = []
    for meaning, cv, value, unit in (("Ventricular Heart Rate", "8867-4", "75", ("/min", "UCUM", "per minute")),
                                     ("QT Interval", "8634-8", "380", ("ms", "UCUM", "millisecond"))):
        a = Dataset()
        a.ConceptNameCodeSequence = Sequence([code(cv, "LN", meaning)])
        a.NumericValue = value
        a.MeasurementUnitsCodeSequence = Sequence([code(*unit)])
        a.ReferencedWaveformChannels = [1, 0]
        ann.append(a)
    a = Dataset()
    a.UnformattedTextValue = "Sinus rhythm. Synthetic test tracing."
    ann.append(a)
    waveform_doc(out, "ecg/ecg_12lead.dcm", TWELVE_LEAD, "ecg-12lead",
                 [wave_group("RHYTHM", freq, n, defs, raw.tobytes(), 16, "SS", "OW")],
                 annotations=ann)

    # General ECG: 3 leads, 8-bit signed at 20 µV, 2 s at 200 Hz, implicit VR.
    freq8, n8 = 200, 400
    t8 = np.arange(n8) / freq8
    b8 = beat(t8, 0.75)
    sig8 = np.stack([np.clip(np.round(k * b8 * 1000 / 20), -128, 127) for k in (0.6, 1.0, 0.4)],
                    axis=1).astype(np.int8)
    defs8 = [channel_def("Lead " + nm, (src, "MDC", "Lead " + nm), ("uV", "UCUM", "microvolt"), "20",
                         bits=8) for nm, src in LEADS[:3]]
    waveform_doc(out, "ecg/ecg_general_8bit.dcm", GENERAL_ECG, "ecg-general",
                 [wave_group("RHYTHM", freq8, n8, defs8, sig8.tobytes(), 8, "SB", "OB")], ts=IMPLICIT_LE)

    # Haemodynamic: arterial and pulmonary pressure (mmHg) in 16-bit unsigned with a baseline and
    # correction factors, 4 s at 125 Hz, plus a 1-lead ECG group at 250 Hz.
    fh, nh = 125, 500
    th = np.arange(nh) / fh
    ph = np.mod(th, 0.8) / 0.8
    art = 80 + 40 * np.exp(-((ph - 0.25) ** 2) / 0.01) + 5 * np.sin(2 * np.pi * ph)
    pa = 10 + 15 * np.exp(-((ph - 0.27) ** 2) / 0.012)
    raw_art = np.round((art + 50.0) / (0.05 * 1.02)).astype(np.uint16)
    raw_pa = np.round((pa + 20.0) / (0.02 * 0.98)).astype(np.uint16)
    hd = np.stack([raw_art, raw_pa], axis=1).astype(np.uint16)
    defs_h = [channel_def("ART", ("2:4106", "MDC", "Arterial pressure"), ("mm[Hg]", "UCUM", "mmHg"),
                          "0.05", baseline="-50", correction="1.02"),
              channel_def("PA", ("2:4196", "MDC", "Pulmonary arterial pressure"), ("mm[Hg]", "UCUM", "mmHg"),
                          "0.02", baseline="-20", correction="0.98")]
    ne = 1000
    te = np.arange(ne) / 250
    raw_e = np.round(beat(te, 0.8) * 1000 / 5).astype(np.int16)
    defs_e = [channel_def("Lead II", ("2:2", "MDC", "Lead II"), ("uV", "UCUM", "microvolt"), "5")]
    waveform_doc(out, "ecg/hemodynamic.dcm", HEMODYNAMIC, "hemodynamic",
                 [wave_group("PRESSURE", fh, nh, defs_h, hd.tobytes(), 16, "US", "OW"),
                  wave_group("ECG", 250, ne, defs_e, raw_e.tobytes(), 16, "SS", "OW")])

    # µ-law audio: a short tone (8 kHz, 0.1 s).
    fa, na = 8000, 800
    tone = 0.5 * np.sin(2 * np.pi * 440 * np.arange(na) / fa)
    mu = mulaw_encode(tone)
    defs_a = [channel_def("Voice", ("1.0", "99SPEC", "Audio"), ("1", "UCUM", "no units"), "1", bits=8)]
    waveform_doc(out, "ecg/audio_mulaw.dcm", BASIC_VOICE, "audio",
                 [wave_group("VOICE", fa, na, defs_a, mu.tobytes(), 8, "MB", "OB")])


def mulaw_encode(x):
    """G.711 µ-law of samples in -1..1."""
    s = np.clip(np.round(x * 32124), -32124, 32124).astype(np.int32)
    out = np.zeros(len(s), dtype=np.uint8)
    for i, v in enumerate(s):
        sign = 0x80 if v < 0 else 0
        m = min(abs(int(v)) + 0x84, 0x7FFF)
        e = 7
        while e > 0 and not (m & (0x4000 >> (7 - e))):
            e -= 1
        mant = (m >> (e + 3)) & 0x0F
        out[i] = ~(sign | (e << 4) | mant) & 0xFF
    return out


# ----------------------------------------------------------------------------------------- OCT


def oct_files(out):
    series_op = uid("op-series")
    op = Dataset()
    common(op, OP8, "op", "OP", series_op)
    op.SeriesDescription = "Fundus"
    op.ImageType = ["ORIGINAL", "PRIMARY"]
    op.Rows = 64
    op.Columns = 64
    op.SamplesPerPixel = 1
    op.PhotometricInterpretation = "MONOCHROME2"
    op.BitsAllocated = 8
    op.BitsStored = 8
    op.HighBit = 7
    op.PixelRepresentation = 0
    op.Laterality = "R"
    op.ImageLaterality = "R"
    op.PixelSpacing = [0.1, 0.1]
    yy, xx = np.mgrid[0:64, 0:64]
    fundus = 120 + 80 * np.exp(-((xx - 40) ** 2 + (yy - 32) ** 2) / 60.0) \
        - 50 * (np.abs(np.sin((xx + yy) / 7.0)) < 0.08)
    op.PixelData = np.clip(fundus, 0, 255).astype(np.uint8).tobytes()
    save(out, "oct/op.dcm", op)

    ds = Dataset()
    common(ds, OPT, "oct", "OPT", uid("oct-series"))
    ds.SeriesDescription = "Macular cube"
    ds.ImageType = ["ORIGINAL", "PRIMARY"]
    ds.Rows = 32
    ds.Columns = 48
    frames = 8
    ds.NumberOfFrames = frames
    ds.SamplesPerPixel = 1
    ds.PhotometricInterpretation = "MONOCHROME2"
    ds.BitsAllocated = 8
    ds.BitsStored = 8
    ds.HighBit = 7
    ds.PixelRepresentation = 0
    ds.Laterality = "R"
    ds.ImageLaterality = "R"
    ds.FrameOfReferenceUID = uid("oct-for")
    per = []
    imgs = []
    for f in range(frames):
        row = 8.0 + f * 6.5
        fl = Dataset()
        fl.ReferencedSOPClassUID = OP8
        fl.ReferencedSOPInstanceUID = op.SOPInstanceUID
        fl.ReferenceCoordinates = [row, 8.0, row, 56.0]
        fl.OphthalmicImageOrientation = "LINEAR"
        it = Dataset()
        it.OphthalmicFrameLocationSequence = Sequence([fl])
        fc = Dataset()
        fc.StackID = "1"
        fc.InStackPositionNumber = f + 1
        fc.DimensionIndexValues = [1, f + 1]
        it.FrameContentSequence = Sequence([fc])
        per.append(it)
        z, x = np.mgrid[0:32, 0:48]
        layer = 12 + 3 * np.sin(x / 8.0 + f)
        img = 40 + 160 * (np.abs(z - layer) < 2) + 60 * (np.abs(z - layer - 8) < 1.5)
        imgs.append(np.clip(img, 0, 255).astype(np.uint8))
    ds.PerFrameFunctionalGroupsSequence = Sequence(per)
    shared = Dataset()
    pm = Dataset()
    pm.PixelSpacing = [0.0039, 0.012]
    pm.SliceThickness = 0.0
    shared.PixelMeasuresSequence = Sequence([pm])
    ds.SharedFunctionalGroupsSequence = Sequence([shared])
    ref = Dataset()
    ref.ReferencedSOPClassUID = OP8
    ref.ReferencedSOPInstanceUID = op.SOPInstanceUID
    ref.PurposeOfReferenceCodeSequence = Sequence([code("121311", "DCM", "Localizer")])
    ds.ReferencedImageSequence = Sequence([ref])
    ds.PixelData = np.stack(imgs).tobytes()
    save(out, "oct/oct.dcm", ds)


def main():
    out = sys.argv[1]
    ct_uids = ct(out)
    rtstruct(out, ct_uids)
    rtplan(out)
    rtdose(out, "rtdose", absolute=False)
    rtdose(out, "rtdose_absolute", absolute=True)
    srs(out, ct_uids)
    ecgs(out)
    oct_files(out)


if __name__ == "__main__":
    main()
