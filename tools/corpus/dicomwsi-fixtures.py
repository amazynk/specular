"""dicomwsi-fixtures.py <repo>: the tiny DICOM fixtures of DICOM whole-slide support

Writes, under <repo>/crates/formats:
  specular-wsi/tests/data/dicomwsi/full/    a TILED_FULL series: 3 levels (160x96, 80x48, 40x24;
                                            32x32 tiles, 2 focal planes at the base), LABEL,
                                            OVERVIEW and THUMBNAIL instances
  specular-wsi/tests/data/dicomwsi/sparse/  a TILED_SPARSE series (2 levels, 2 optical paths, two
                                            base tiles not stored) and a LABEL in another series
                                            of the same slide
  specular-dicom/tests/data/jxl-*.dcm       JPEG XL transfer syntaxes (Supplement 232)
  specular-dicom/tests/data/rwvm.dcm        Real World Value Mapping (linear and LUT items)

Needs numpy, pydicom and imagecodecs (the oracle venv; run with python -I). Then run
dicomwsi-oracle.py --tiles on the two series folders for full.tsv and sparse.tsv beside them, and
dicomwsi-oracle.py --dicom on the jxl/rwvm files for specular-dicom's jxl_rwvm_oracle.tsv.

Pixel values (base-level coordinates X, Y; a level of downsample d samples X = x*d, Y = y*d):
  rgb(X, Y, c) = (X*3 + Y*5 + c*70 + (X//16 + Y//16) % 2 * 40 + z*9 + path*100) % 256
"""
import os
import sys

import imagecodecs
import numpy as np
import pydicom
from pydicom.dataset import Dataset, FileMetaDataset
from pydicom.encaps import encapsulate
from pydicom.sequence import Sequence
from pydicom.uid import (
    ExplicitVRLittleEndian,
    JPEG2000Lossless,
    JPEGBaseline8Bit,
    JPEGLSLossless,
    RLELossless,
    generate_uid,
)

WSI = "1.2.840.10008.5.1.4.1.1.77.1.6"
JXL_LOSSLESS = "1.2.840.10008.1.2.4.110"
JXL = "1.2.840.10008.1.2.4.112"
ROOT = "1.2.826.0.1.3680043.10.1418."  # fixtures only
PX_MM = 0.0005  # 0.5 um per pixel at the base


def uid(n):
    return generate_uid(prefix=ROOT, entropy_srcs=[n])


def rgb(w, h, d, z=0, path=0):
    y, x = np.mgrid[0:h, 0:w].astype(np.int64)
    X, Y = x * d, y * d
    out = np.zeros((h, w, 3), np.uint8)
    for c in range(3):
        out[..., c] = (X * 3 + Y * 5 + c * 70 + (X // 16 + Y // 16) % 2 * 40 + z * 9 + path * 100) % 256
    return out


def code(value, scheme, meaning):
    c = Dataset()
    c.CodeValue = value
    c.CodingSchemeDesignator = scheme
    c.CodeMeaning = meaning
    return c


def optical_path(ident):
    p = Dataset()
    p.OpticalPathIdentifier = ident
    p.IlluminationTypeCodeSequence = [code("111744", "DCM", "Brightfield illumination")]
    p.IlluminationColorCodeSequence = [code("414298005", "SCT", "Full Spectrum")]
    p.ObjectiveLensPower = 20
    return p


def base(series, number, kind, study, frame_of_ref, container):
    ds = Dataset()
    ds.SOPClassUID = WSI
    ds.SOPInstanceUID = uid(f"{series}-{number}-{kind}")
    ds.StudyInstanceUID = study
    ds.SeriesInstanceUID = series
    ds.FrameOfReferenceUID = frame_of_ref
    ds.Modality = "SM"
    if kind == "VOLUME" and number != 1:
        ds.ImageType = ["DERIVED", "PRIMARY", "VOLUME", "RESAMPLED"]
    else:
        ds.ImageType = ["ORIGINAL", "PRIMARY", kind, "NONE"]
    ds.InstanceNumber = number
    ds.SeriesNumber = 1
    ds.PatientName = "Fixture^Slide"
    ds.PatientID = "FIXTURE"
    ds.StudyDate = "20261011"
    ds.StudyTime = "120000"
    ds.ContentDate = "20261011"
    ds.ContentTime = "120000"
    ds.AcquisitionDateTime = "20261011120000"
    ds.Manufacturer = "Specular"
    ds.ManufacturerModelName = "Fixture Scanner"
    ds.DeviceSerialNumber = "1"
    ds.SoftwareVersions = "1"
    ds.SeriesDescription = "Fixture slide"
    ds.ContainerIdentifier = container
    ds.IssuerOfTheContainerIdentifierSequence = []
    ds.ContainerTypeCodeSequence = [code("433466003", "SCT", "Microscope slide")]
    ds.SpecimenDescriptionSequence = []
    ds.AcquisitionContextSequence = []
    ds.VolumetricProperties = "VOLUME"
    ds.BurnedInAnnotation = "YES" if kind == "LABEL" else "NO"
    ds.SpecimenLabelInImage = "YES" if kind == "LABEL" else "NO"
    ds.FocusMethod = "AUTO"
    ds.ExtendedDepthOfField = "NO"
    ds.ImageOrientationSlide = [0, -1, 0, -1, 0, 0]
    origin = Dataset()
    origin.XOffsetInSlideCoordinateSystem = 20
    origin.YOffsetInSlideCoordinateSystem = 40
    ds.TotalPixelMatrixOriginSequence = [origin]
    ds.LossyImageCompression = "00"
    ds.PresentationLUTShape = "IDENTITY"
    ds.RescaleIntercept = 0
    ds.RescaleSlope = 1
    return ds


def frames_of(img, tw, th):
    """Tiles of `img` in row-major order, edge tiles padded with black to the full size."""
    h, w = img.shape[:2]
    out = []
    for ty in range(0, h, th):
        for tx in range(0, w, tw):
            t = np.zeros((th, tw, img.shape[2]), img.dtype)
            part = img[ty:ty + th, tx:tx + tw]
            t[: part.shape[0], : part.shape[1]] = part
            out.append(t)
    return out


def encode(frames, ts):
    if ts == ExplicitVRLittleEndian:
        return b"".join(f.tobytes() for f in frames), False
    if ts == JPEG2000Lossless:
        enc = [imagecodecs.jpeg2k_encode(f, codecformat="J2K", reversible=True, mct=False) for f in frames]
    elif ts == JPEGLSLossless:
        enc = [imagecodecs.jpegls_encode(f, level=0) for f in frames]
    elif ts == JPEGBaseline8Bit:
        enc = [imagecodecs.jpeg8_encode(f, level=92, subsampling=(1, 1)) for f in frames]
    elif ts in (JXL_LOSSLESS, JXL):
        enc = [imagecodecs.jpegxl_encode(f, lossless=ts == JXL_LOSSLESS, distance=None if ts == JXL_LOSSLESS else 1.0, effort=3) for f in frames]
    elif ts == RLELossless:
        from pydicom.pixels.encoders.base import RLELosslessEncoder
        enc = []
        for f in frames:
            enc.append(RLELosslessEncoder.encode(
                f, encoding_plugin="pydicom", rows=f.shape[0], columns=f.shape[1],
                samples_per_pixel=f.shape[2] if f.ndim == 3 else 1, bits_allocated=8, bits_stored=8,
                pixel_representation=0, photometric_interpretation="RGB" if f.ndim == 3 else "MONOCHROME2",
                number_of_frames=1, planar_configuration=0))
    else:
        raise ValueError(ts)
    return encapsulate(enc), True


def set_pixels(ds, frames, ts, mono=False):
    t = frames[0]
    ds.Rows, ds.Columns = t.shape[:2]
    ds.SamplesPerPixel = 1 if mono else 3
    ds.PhotometricInterpretation = "MONOCHROME2" if mono else "RGB"
    if ts == JPEGBaseline8Bit and not mono:
        ds.PhotometricInterpretation = "YBR_FULL"
    if ts == JPEG2000Lossless and not mono:
        ds.PhotometricInterpretation = "RGB"
    if not mono:
        ds.PlanarConfiguration = 0
    ds.BitsAllocated = 8
    ds.BitsStored = 8
    ds.HighBit = 7
    ds.PixelRepresentation = 0
    ds.NumberOfFrames = len(frames)
    data, encapsulated = encode(frames, ts)
    ds.PixelData = data
    ds["PixelData"].VR = "OB"
    # pydicom does not know the JPEG XL syntaxes: mark encapsulated data undefined-length.
    ds["PixelData"].is_undefined_length = encapsulated
    ds.LossyImageCompression = "01" if ts in (JPEGBaseline8Bit, JXL) else "00"
    if ds.LossyImageCompression == "01":
        ds.LossyImageCompressionMethod = "ISO_10918_1" if ts == JPEGBaseline8Bit else "ISO_18181_1"
        ds.LossyImageCompressionRatio = 10
    return encapsulated


def shared(spacing_mm, z_mm=None, slice_mm=None, path=None):
    g = Dataset()
    pm = Dataset()
    pm.PixelSpacing = [spacing_mm, spacing_mm]
    pm.SliceThickness = 0.001
    if slice_mm is not None:
        pm.SpacingBetweenSlices = slice_mm
    g.PixelMeasuresSequence = [pm]
    if z_mm is not None:
        pos = Dataset()
        pos.ZOffsetInSlideCoordinateSystem = z_mm
        g.PlanePositionSlideSequence = [pos]
    if path is not None:
        p = Dataset()
        p.OpticalPathIdentifier = path
        g.OpticalPathIdentificationSequence = [p]
    return g


def save(ds, ts, path, encapsulated):
    meta = FileMetaDataset()
    meta.MediaStorageSOPClassUID = ds.SOPClassUID
    meta.MediaStorageSOPInstanceUID = ds.SOPInstanceUID
    meta.TransferSyntaxUID = ts
    meta.ImplementationClassUID = ROOT + "1"
    ds.file_meta = meta
    os.makedirs(os.path.dirname(path), exist_ok=True)
    ds.preamble = b"\0" * 128
    ds.save_as(path, implicit_vr=False, little_endian=True, force_encoding=True)
    size = os.path.getsize(path)
    assert size < 200 * 1024, (path, size)
    print(f"{path}: {size} bytes")


def full_series(out):
    study, series = uid("full-study"), uid("full-series")
    fref, container = uid("full-for"), "FIXTURE-FULL"
    w0, h0, tile = 160, 96, 32
    levels = [(1, ExplicitVRLittleEndian, 2), (2, RLELossless, 1), (4, JPEGLSLossless, 1)]
    for i, (d, ts, planes) in enumerate(levels):
        w, h = w0 // d, h0 // d
        ds = base(series, i + 1, "VOLUME", study, fref, container)
        ds.DimensionOrganizationType = "TILED_FULL"
        ds.TotalPixelMatrixColumns, ds.TotalPixelMatrixRows = w, h
        ds.TotalPixelMatrixFocalPlanes = planes
        ds.NumberOfOpticalPaths = 1
        ds.OpticalPathSequence = [optical_path("0")]
        ds.ImagedVolumeWidth = w0 * PX_MM
        ds.ImagedVolumeHeight = h0 * PX_MM
        ds.ImagedVolumeDepth = 0.001 * planes
        ds.SharedFunctionalGroupsSequence = [
            shared(PX_MM * d, z_mm=0.0 if planes == 1 else 0.0, slice_mm=0.002 if planes > 1 else None)]
        frames = []
        for z in range(planes):
            frames += frames_of(rgb(w, h, d, z=z), tile, tile)
        enc = set_pixels(ds, frames, ts)
        save(ds, ts, f"{out}/level{i}.dcm", enc)
    # Associated images: whole-image single frames.
    for kind, (w, h), ts in [("LABEL", (40, 32), JPEGBaseline8Bit), ("OVERVIEW", (64, 24), ExplicitVRLittleEndian),
                             ("THUMBNAIL", (20, 12), JXL_LOSSLESS)]:
        ds = base(series, 10 + len(kind), kind, study, fref, container)
        ds.DimensionOrganizationType = "TILED_FULL"
        ds.TotalPixelMatrixColumns, ds.TotalPixelMatrixRows = w, h
        ds.TotalPixelMatrixFocalPlanes = 1
        ds.NumberOfOpticalPaths = 1
        ds.OpticalPathSequence = [optical_path("0")]
        ds.ImagedVolumeWidth = w0 * PX_MM
        ds.ImagedVolumeHeight = h0 * PX_MM
        ds.ImagedVolumeDepth = 0.001
        ds.SharedFunctionalGroupsSequence = [shared(PX_MM * w0 / w)]
        img = rgb(w, h, 1, path={"LABEL": 1, "OVERVIEW": 2, "THUMBNAIL": 0}[kind])
        if kind == "THUMBNAIL":
            img = rgb(w, h, 8)
        enc = set_pixels(ds, [img], ts)
        save(ds, ts, f"{out}/{kind.lower()}.dcm", enc)


def sparse_series(out):
    study, series, other = uid("sparse-study"), uid("sparse-series"), uid("sparse-label-series")
    fref, container = uid("sparse-for"), "FIXTURE-SPARSE"
    w0, h0, tile = 128, 96, 32
    missing = {(1, 1), (3, 2)}  # (col, row) of base tiles not stored
    for i, (d, ts) in enumerate([(1, JPEG2000Lossless), (2, ExplicitVRLittleEndian)]):
        w, h = w0 // d, h0 // d
        ds = base(series, i + 1, "VOLUME", study, fref, container)
        ds.DimensionOrganizationType = "TILED_SPARSE"
        ds.TotalPixelMatrixColumns, ds.TotalPixelMatrixRows = w, h
        ds.TotalPixelMatrixFocalPlanes = 1
        ds.NumberOfOpticalPaths = 2
        ds.OpticalPathSequence = [optical_path("A"), optical_path("B")]
        ds.ImagedVolumeWidth = w0 * PX_MM
        ds.ImagedVolumeHeight = h0 * PX_MM
        ds.ImagedVolumeDepth = 0.001
        ds.SharedFunctionalGroupsSequence = [shared(PX_MM * d)]
        per, frames = [], []
        # Paths B then A, tiles bottom-up: the order must not matter.
        for p, name in [(1, "B"), (0, "A")]:
            img = rgb(w, h, d, path=p)
            for ty in reversed(range(0, h, tile)):
                for tx in range(0, w, tile):
                    if d == 1 and (tx // tile, ty // tile) in missing:
                        continue
                    t = np.zeros((tile, tile, 3), np.uint8)
                    part = img[ty:ty + tile, tx:tx + tile]
                    t[: part.shape[0], : part.shape[1]] = part
                    frames.append(t)
                    item = Dataset()
                    pos = Dataset()
                    pos.ColumnPositionInTotalImagePixelMatrix = tx + 1
                    pos.RowPositionInTotalImagePixelMatrix = ty + 1
                    pos.XOffsetInSlideCoordinateSystem = 20 - ty * PX_MM * d
                    pos.YOffsetInSlideCoordinateSystem = 40 - tx * PX_MM * d
                    pos.ZOffsetInSlideCoordinateSystem = 0.0
                    item.PlanePositionSlideSequence = [pos]
                    op = Dataset()
                    op.OpticalPathIdentifier = name
                    item.OpticalPathIdentificationSequence = [op]
                    per.append(item)
        ds.PerFrameFunctionalGroupsSequence = per
        enc = set_pixels(ds, frames, ts)
        save(ds, ts, f"{out}/level{i}.dcm", enc)
    ds = base(other, 1, "LABEL", study, fref, container)
    ds.DimensionOrganizationType = "TILED_FULL"
    ds.TotalPixelMatrixColumns, ds.TotalPixelMatrixRows = 48, 24
    ds.TotalPixelMatrixFocalPlanes = 1
    ds.NumberOfOpticalPaths = 1
    ds.OpticalPathSequence = [optical_path("A")]
    ds.ImagedVolumeWidth = w0 * PX_MM
    ds.ImagedVolumeHeight = h0 * PX_MM
    ds.ImagedVolumeDepth = 0.001
    ds.SharedFunctionalGroupsSequence = [shared(PX_MM)]
    enc = set_pixels(ds, [rgb(48, 24, 1, path=1)], ExplicitVRLittleEndian)
    save(ds, ExplicitVRLittleEndian, f"{out}/label-other-series.dcm", enc)


def plain(sop, modality, w, h):
    ds = Dataset()
    ds.SOPClassUID = sop
    ds.SOPInstanceUID = uid(f"plain-{modality}-{w}-{h}-{sop}")
    ds.StudyInstanceUID = uid("plain-study")
    ds.SeriesInstanceUID = uid(f"plain-series-{modality}")
    ds.Modality = modality
    ds.PatientName = "Fixture^Plain"
    ds.PatientID = "FIXTURE"
    ds.Rows, ds.Columns = h, w
    return ds


def grey_pixels(ds, img, bits, signed=False):
    ds.SamplesPerPixel = 1
    ds.PhotometricInterpretation = "MONOCHROME2"
    ds.BitsAllocated = 16 if bits > 8 else 8
    ds.BitsStored = bits
    ds.HighBit = bits - 1
    ds.PixelRepresentation = 1 if signed else 0


def jxl_files(out):
    secondary = "1.2.840.10008.5.1.4.1.1.7"
    w, h = 24, 16
    y, x = np.mgrid[0:h, 0:w].astype(np.int64)
    # 12-bit grey, lossless.
    grey = ((x * 157 + y * 263 + (x * y) % 13 * 31) % 4096).astype(np.uint16)
    ds = plain(secondary, "OT", w, h)
    grey_pixels(ds, grey, 12)
    ds.PixelData = encapsulate([imagecodecs.jpegxl_encode(grey, lossless=True, bitspersample=12, effort=3)])
    ds["PixelData"].VR = "OB"
    ds["PixelData"].is_undefined_length = True
    save(ds, JXL_LOSSLESS, f"{out}/jxl-lossless-grey12.dcm", True)
    # 8-bit RGB, lossless and lossy.
    img = rgb(w, h, 1)
    for ts, name in [(JXL_LOSSLESS, "jxl-lossless-rgb8"), (JXL, "jxl-lossy-rgb8")]:
        ds = plain(secondary, "OT", w, h)
        ds.SOPInstanceUID = uid(name)
        ds.SamplesPerPixel = 3
        ds.PhotometricInterpretation = "RGB"
        ds.PlanarConfiguration = 0
        ds.BitsAllocated = ds.BitsStored = 8
        ds.HighBit = 7
        ds.PixelRepresentation = 0
        enc = imagecodecs.jpegxl_encode(img, lossless=ts == JXL_LOSSLESS, distance=None if ts == JXL_LOSSLESS else 1.0, effort=3)
        ds.PixelData = encapsulate([enc])
        ds["PixelData"].VR = "OB"
        ds["PixelData"].is_undefined_length = True
        if ts == JXL:
            ds.LossyImageCompression = "01"
        save(ds, ts, f"{out}/{name}.dcm", True)


def rwvm_file(out):
    mr = "1.2.840.10008.5.1.4.1.1.4"
    w, h = 8, 4
    y, x = np.mgrid[0:h, 0:w].astype(np.int64)
    img = (x * 300 - 1000 + y * 7).astype(np.int16)  # -1000 .. 1114
    img[3] = np.arange(8)  # the LUT item's range
    ds = plain(mr, "MR", w, h)
    grey_pixels(ds, img, 16, signed=True)
    ds.PixelData = img.tobytes()
    ds["PixelData"].VR = "OW"
    # Item 1: negative stored values, linear, ADC in UCUM um2/s with a LUT label.
    lin = Dataset()
    lin.add_new(0x00409216, "SS", -1000)  # Real World Value First Value Mapped
    lin.add_new(0x00409211, "SS", -1)  # Real World Value Last Value Mapped
    lin.RealWorldValueIntercept = 2.5
    lin.RealWorldValueSlope = 0.5
    lin.LUTLabel = "ADC"
    lin.LUTExplanation = "Apparent diffusion"
    lin.MeasurementUnitsCodeSequence = [code("um2/s", "UCUM", "um2/s")]
    # Item 2: stored 0 .. 7 by a LUT, unitless ratio, label from the quantity definition.
    lut = Dataset()
    lut.add_new(0x00409216, "SS", 0)
    lut.add_new(0x00409211, "SS", 7)
    lut.RealWorldValueLUTData = [0.0, 0.25, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0]
    lut.MeasurementUnitsCodeSequence = [code("1", "UCUM", "no units")]
    q = Dataset()
    q.ValueType = "CODE"
    q.ConceptNameCodeSequence = [code("246205007", "SCT", "Quantity")]
    q.ConceptCodeSequence = [code("C0000", "99FIX", "Ratio")]
    lut.QuantityDefinitionSequence = [q]
    # Item 3: stored 8 .. 2000 in seconds (code meaning only, no UCUM).
    sec = Dataset()
    sec.add_new(0x00409216, "SS", 8)
    sec.add_new(0x00409211, "SS", 2000)
    sec.RealWorldValueIntercept = 0.0
    sec.RealWorldValueSlope = 0.001
    sec.MeasurementUnitsCodeSequence = [code("s", "99FIX", "second")]
    ds.RealWorldValueMappingSequence = [lin, lut, sec]
    save(ds, ExplicitVRLittleEndian, f"{out}/rwvm.dcm", False)


def main():
    repo = sys.argv[1]
    formats = os.path.join(repo, "crates/formats")
    full_series(f"{formats}/specular-wsi/tests/data/dicomwsi/full")
    sparse_series(f"{formats}/specular-wsi/tests/data/dicomwsi/sparse")
    jxl_files(f"{formats}/specular-dicom/tests/data")
    rwvm_file(f"{formats}/specular-dicom/tests/data")


if __name__ == "__main__":
    main()
