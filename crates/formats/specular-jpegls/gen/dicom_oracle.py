#!/usr/bin/env python3
"""Compares specular-dicom's JPEG-LS frames with pydicom + CharLS (pyjpegls), bit for bit.

Usage: python -I dicom_oracle.py DUMP_DIR CORPUS_DIR

DUMP_DIR holds the output of `dicom_dump DUMP_DIR FILE...` (specular-dicom example) for the
JPEG-LS files of CORPUS_DIR: `NAME.f<k>.raw`, frame k's stored values as little-endian i32
(grey) or u16 (RGB), for the first and the last frame. CharLS is only a test oracle here.
"""

import glob
import os
import sys
import warnings

import numpy as np
import pydicom

JPEG_LS = {"1.2.840.10008.1.2.4.80", "1.2.840.10008.1.2.4.81"}


def main(dump, corpus):
    warnings.simplefilter("ignore")
    ok = bad = 0
    for path in sorted(glob.glob(os.path.join(corpus, "**", "*.dcm"), recursive=True)):
        try:
            ds = pydicom.dcmread(path)
            if str(ds.file_meta.TransferSyntaxUID) not in JPEG_LS:
                continue
        except Exception:
            continue
        name = os.path.basename(path)
        rel = os.path.relpath(path, corpus)
        try:
            ref = ds.pixel_array
        except Exception as e:
            print(f"SKIP {rel}: oracle fails ({e})")
            continue
        frames = int(ds.get("NumberOfFrames", 1) or 1)
        if frames == 1:
            ref = ref[None]
        rgb = int(ds.SamplesPerPixel) == 3
        status = open(os.path.join(dump, name + ".txt")).read().split("\n", 1)[0]
        if status != "ok":
            print(f"FAIL {rel}: {status}")
            bad += 1
            continue
        mismatch = []
        for k in sorted({0, frames - 1}):
            raw = os.path.join(dump, f"{name}.f{k}.raw")
            got = np.fromfile(raw, dtype="<u2" if rgb else "<i4").reshape(ref[k].shape)
            diff = int((got.astype(np.int64) != ref[k].astype(np.int64)).sum())
            if diff:
                mismatch.append(f"frame {k}: {diff} samples differ")
        if mismatch:
            print(f"FAIL {rel}: {'; '.join(mismatch)}")
            bad += 1
        else:
            print(f"ok   {rel} ({ds.Columns}x{ds.Rows}, {frames} frame(s), {ds.BitsStored}-bit)")
            ok += 1
    print(f"{ok} bit-exact, {bad} differ")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1], sys.argv[2]))
