"""pynetdicom C-GET/C-MOVE SCP (Study Root): the oracle for specular-dimse's retrieval SCU.

Usage: getscp.py OUT_DIR MOVE_AE MOVE_PORT [DELAY]
Generates a small study into OUT_DIR/served: a CT series of 6 slices (3 Explicit VR
Little Endian, 2 Implicit, 1 RLE Lossless) and an MR series of 3 (Explicit), 16-bit
synthetic pixels. For each file OUT_DIR/served/<uid>.px holds pydicom's pixel_array as
uint16 little-endian (an independent decode). Prints the port it listens on (127.0.0.1,
ephemeral), then serves until stdin closes. C-MOVE knows one destination, MOVE_AE at
127.0.0.1:MOVE_PORT. DELAY: seconds before each sub-operation (cancel tests). Each
request's identifier is written to OUT_DIR/req-<n>.json, with whether it was cancelled.
"""
import json
import os
import sys
import time

import numpy as np
from pydicom.dataset import Dataset, FileMetaDataset
from pydicom.uid import (
    ExplicitVRLittleEndian,
    ImplicitVRLittleEndian,
    RLELossless,
)
from pynetdicom import AE, ALL_TRANSFER_SYNTAXES, StoragePresentationContexts, build_context, evt
from pynetdicom.sop_class import (
    CTImageStorage,
    MRImageStorage,
    StudyRootQueryRetrieveInformationModelGet,
    StudyRootQueryRetrieveInformationModelMove,
    Verification,
)

out, move_ae, move_port = sys.argv[1], sys.argv[2], int(sys.argv[3])
delay = float(sys.argv[4]) if len(sys.argv) > 4 else 0.0
served = os.path.join(out, "served")
os.makedirs(served, exist_ok=True)

STUDY = "1.2.826.0.1.3680043.10.77.1"
CT_SERIES = "1.2.826.0.1.3680043.10.77.1.1"
MR_SERIES = "1.2.826.0.1.3680043.10.77.1.2"


def make(cls, series, modality, i, size, ts):
    ds = Dataset()
    ds.SOPClassUID = cls
    ds.SOPInstanceUID = "%s.%d" % (series, i + 1)
    ds.StudyInstanceUID = STUDY
    ds.SeriesInstanceUID = series
    ds.Modality = modality
    ds.PatientName = "ORACLE^RETRIEVE"
    ds.PatientID = "RET-1"
    ds.InstanceNumber = i + 1
    ds.SeriesNumber = 1 if series == CT_SERIES else 2
    ds.ImagePositionPatient = [0, 0, float(i) * 2.5]
    ds.ImageOrientationPatient = [1, 0, 0, 0, 1, 0]
    ds.PixelSpacing = [0.7, 0.7]
    ds.SliceThickness = 2.5
    ds.SamplesPerPixel = 1
    ds.PhotometricInterpretation = "MONOCHROME2"
    ds.Rows = size
    ds.Columns = size
    ds.BitsAllocated = 16
    ds.BitsStored = 16
    ds.HighBit = 15
    ds.PixelRepresentation = 0
    y, x = np.mgrid[0:size, 0:size]
    px = ((x * 37 + y * 11 + i * 997) % 65536).astype(np.uint16)
    px[size // 4 : size // 2, size // 4 : size // 2] ^= 0xA5A5
    ds.PixelData = px.tobytes()
    ds.file_meta = FileMetaDataset()
    ds.file_meta.MediaStorageSOPClassUID = cls
    ds.file_meta.MediaStorageSOPInstanceUID = ds.SOPInstanceUID
    ds.file_meta.TransferSyntaxUID = ExplicitVRLittleEndian
    if ts == RLELossless:
        ds.compress(RLELossless)
    else:
        ds.file_meta.TransferSyntaxUID = ts
    path = os.path.join(served, ds.SOPInstanceUID + ".dcm")
    ds.save_as(path, enforce_file_format=True)
    with open(os.path.join(served, ds.SOPInstanceUID + ".px"), "wb") as f:
        f.write(ds.pixel_array.astype("<u2").tobytes())
    return ds


DATA = []
for i, ts in enumerate(
    [ExplicitVRLittleEndian, ImplicitVRLittleEndian, RLELossless,
     ExplicitVRLittleEndian, ImplicitVRLittleEndian, ExplicitVRLittleEndian]
):
    DATA.append(make(CTImageStorage, CT_SERIES, "CT", i, 64, ts))
for i in range(3):
    DATA.append(make(MRImageStorage, MR_SERIES, "MR", i, 48, ExplicitVRLittleEndian))

ae = AE(ae_title="ORACLE")
ae.add_supported_context(StudyRootQueryRetrieveInformationModelGet)
ae.add_supported_context(StudyRootQueryRetrieveInformationModelMove)
ae.add_supported_context(Verification)
for cx in StoragePresentationContexts:
    ae.add_supported_context(cx.abstract_syntax, ALL_TRANSFER_SYNTAXES, scp_role=True, scu_role=True)
count = [0]


def matches(q):
    level = q.QueryRetrieveLevel
    study = str(q.get("StudyInstanceUID", ""))
    series = str(q.get("SeriesInstanceUID", ""))
    for ds in DATA:
        if ds.StudyInstanceUID != study:
            continue
        if level == "SERIES" and ds.SeriesInstanceUID != series:
            continue
        yield ds


def note(event, kind, n):
    asked = {e.keyword: str(e.value) for e in event.identifier}
    asked["kind"] = kind
    asked["calling"] = event.assoc.requestor.ae_title
    if kind == "move":
        asked["destination"] = event.move_destination.decode().strip() if isinstance(
            event.move_destination, bytes) else str(event.move_destination).strip()
    with open(os.path.join(out, "req-%d.json" % n), "w") as f:
        json.dump(asked, f)


def finish(event, n, cancelled):
    p = os.path.join(out, "req-%d.json" % n)
    with open(p) as f:
        v = json.load(f)
    v["cancelled"] = cancelled
    with open(p, "w") as f:
        json.dump(v, f)


def on_get(event):
    n = count[0]
    count[0] += 1
    note(event, "get", n)
    found = list(matches(event.identifier))
    yield len(found)
    for ds in found:
        time.sleep(delay)
        if event.is_cancelled:
            finish(event, n, True)
            yield (0xFE00, None)
            return
        yield (0xFF00, ds)
    finish(event, n, False)


def on_move(event):
    n = count[0]
    count[0] += 1
    note(event, "move", n)
    dest = event.move_destination
    dest = dest.decode() if isinstance(dest, bytes) else str(dest)
    if dest.strip() != move_ae:
        yield (None, None)
        return
    contexts = [build_context(c, [ExplicitVRLittleEndian, ImplicitVRLittleEndian])
                for c in (CTImageStorage, MRImageStorage)]
    contexts.append(build_context(CTImageStorage, [RLELossless]))
    yield ("127.0.0.1", move_port, {"contexts": contexts})
    found = list(matches(event.identifier))
    yield len(found)
    for ds in found:
        time.sleep(delay)
        if event.is_cancelled:
            finish(event, n, True)
            yield (0xFE00, None)
            return
        yield (0xFF00, ds)
    finish(event, n, False)


server = ae.start_server(
    ("127.0.0.1", 0),
    block=False,
    evt_handlers=[(evt.EVT_C_GET, on_get), (evt.EVT_C_MOVE, on_move)],
)
print(server.server_address[1], flush=True)
sys.stdin.read()
server.shutdown()
