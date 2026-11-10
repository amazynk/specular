"""Writes the DICONDE (ASTM E2339) fixtures and their oracle for tests/diconde.rs.

Usage (from the crate directory, with a Python that has pydicom and numpy):

    python -I gen/diconde_make.py tests/data

* diconde-ct.dcm: a CT slice of an inspected component, as industrial CT writes it: the CT
  Image Storage class with the NDE component, material and inspection attributes of group
  0014, a component name and ID in the patient's name and ID (as DICONDE keeps them), an
  evaluator and two indications.
* diconde-ec.dcm: an eddy current image (a DICONDE storage class) without group 0014.
* diconde_oracle.tsv: what pydicom reads back from each file (`nde` rows: file, field,
  value) and the keyword of every group 0014 tag in them (`kw` rows: tag, keyword).
"""

import os
import sys

import numpy as np
import pydicom
from pydicom.dataset import Dataset, FileMetaDataset

CT = "1.2.840.10008.5.1.4.1.1.2"
EDDY_CURRENT = "1.2.840.10008.5.1.4.1.1.601.1"
EXPLICIT_LE = "1.2.840.10008.1.2.1"
ROOT = "1.2.826.0.1.3680043.10.1."


def base(sop, n, modality, rows, cols):
    meta = FileMetaDataset()
    meta.MediaStorageSOPClassUID = sop
    meta.MediaStorageSOPInstanceUID = ROOT + f"{n}.3"
    meta.TransferSyntaxUID = EXPLICIT_LE
    ds = Dataset()
    ds.file_meta = meta
    ds.SOPClassUID = sop
    ds.SOPInstanceUID = meta.MediaStorageSOPInstanceUID
    ds.StudyInstanceUID = ROOT + f"{n}.1"
    ds.SeriesInstanceUID = ROOT + f"{n}.2"
    ds.Modality = modality
    ds.Rows, ds.Columns = rows, cols
    ds.SamplesPerPixel = 1
    ds.PhotometricInterpretation = "MONOCHROME2"
    ds.BitsAllocated, ds.BitsStored, ds.HighBit, ds.PixelRepresentation = 16, 16, 15, 0
    ramp = (np.arange(rows * cols, dtype=np.uint16) * 37).reshape(rows, cols)
    ds.PixelData = ramp.astype("<u2").tobytes()
    return ds


def ct():
    ds = base(CT, 1, "CT", 32, 32)
    ds.PatientName = "Turbine^Blade 7"  # the component's name, in DICONDE
    ds.PatientID = "TB-0007"  # the component's ID
    ds.StudyDescription = "Turbine blade CT"
    ds.SeriesDescription = "Full blade, 30 um"
    ds.KVP = "225"
    ds.PixelSpacing = ["0.03", "0.03"]
    ds.SliceThickness = "0.03"
    ds.ImageType = ["ORIGINAL", "PRIMARY", "AXIAL"]
    ds.RescaleIntercept, ds.RescaleSlope = "0", "1"
    ds.ComponentManufacturer = "Acme Aero"
    ds.ComponentShape = "OTHER"
    ds.MaterialGrade = "Inconel 718"
    ds.MaterialThickness = "4.5"
    ds.EnvironmentalConditions = "21 C, dry"
    ds.ImageQualityIndicatorType = "WIRE"
    ds.ImageQualityIndicatorMaterial = "NICKEL"
    ev = Dataset()
    ev.EvaluatorNumber = "1"
    ev.EvaluatorName = "Smith^Ann"
    ds.EvaluatorSequence = [ev]
    inds = []
    for i, label in enumerate(("Porosity", "Crack"), 1):
        ind = Dataset()
        ind.IndicationNumber = str(i)
        ind.IndicationLabel = label
        ind.IndicationType = "LINEAR" if label == "Crack" else "VOLUMETRIC"
        inds.append(ind)
    ds.IndicationSequence = inds
    return ds


def ec():
    ds = base(EDDY_CURRENT, 2, "EC", 8, 8)
    ds.StudyDescription = "Tube bundle"
    return ds


def write(ds, path):
    ds.save_as(path, enforce_file_format=True)


def main(out):
    rows = []
    for name, make in (("diconde-ct.dcm", ct), ("diconde-ec.dcm", ec)):
        path = os.path.join(out, name)
        write(make(), path)
        back = pydicom.dcmread(path)
        get = lambda k: str(back.get(k, "")).strip()  # noqa: E731
        num = lambda k: repr(float(back[k].value)) if k in back else ""  # noqa: E731
        iqi = get("ImageQualityIndicatorType")
        if iqi and get("ImageQualityIndicatorMaterial"):
            iqi = f"{iqi} ({get('ImageQualityIndicatorMaterial')})"
        fields = {
            "modality": get("Modality"),
            "component_shape": get("ComponentShape"),
            "component_manufacturer": get("ComponentManufacturer"),
            "material_grade": get("MaterialGrade"),
            "material_thickness": num("MaterialThickness"),
            "environment": get("EnvironmentalConditions"),
            "iqi": iqi,
            "indications": str(len(back.get("IndicationSequence", []))),
            "kvp": num("KVP"),
        }
        for k, v in fields.items():
            rows.append(f"nde\t{name}\t{k}\t{v}")
        for elem in back.iterall():
            if elem.tag.group == 0x0014:
                rows.append(f"kw\t{int(elem.tag):08X}\t{elem.keyword}")
    with open(os.path.join(out, "diconde_oracle.tsv"), "w") as f:
        f.write("\n".join(rows) + "\n")


if __name__ == "__main__":
    main(sys.argv[1])
