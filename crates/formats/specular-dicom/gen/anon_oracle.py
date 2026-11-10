#!/usr/bin/env python3
"""Oracle for Specular's patient disc folders (tests/anon.rs): pydicom reads the DICOMDIR as a
FileSet, loads every referenced instance and checks that the record and the file agree, and
that the de-identified files say so. Prints "instances N" on success.

Usage: python -I gen/anon_oracle.py <folder with DICOMDIR>
"""

import sys
from pathlib import Path

from pydicom import dcmread
from pydicom.fileset import FileSet


def main(root: Path) -> int:
    ds = dcmread(root / "DICOMDIR")
    fs = FileSet(ds)
    n = 0
    errors = []
    for inst in fs:
        f = inst.load()
        if f.SOPInstanceUID != inst.SOPInstanceUID:
            errors.append(f"{inst.path}: SOP Instance UID differs from its record")
        if f.file_meta.MediaStorageSOPInstanceUID != f.SOPInstanceUID:
            errors.append(f"{inst.path}: File Meta SOP Instance UID differs")
        if f.get("PatientIdentityRemoved") != "YES":
            errors.append(f"{inst.path}: Patient Identity Removed is not YES")
        # Patient, study and series records hold what the instance holds.
        series = inst.node.parent
        study = series.parent
        patient = study.parent
        if f.get("PatientID") != patient._record.get("PatientID"):
            errors.append(f"{inst.path}: patient record differs")
        if f.StudyInstanceUID != study._record.StudyInstanceUID:
            errors.append(f"{inst.path}: study record differs")
        if f.SeriesInstanceUID != series._record.SeriesInstanceUID:
            errors.append(f"{inst.path}: series record differs")
        n += 1
    for e in errors[:20]:
        print(e)
    if errors:
        return 1
    print(f"instances {n}")
    return 0


if __name__ == "__main__":
    sys.exit(main(Path(sys.argv[1])))
