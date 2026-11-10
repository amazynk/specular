"""pynetdicom C-FIND SCP (Study Root): the oracle for specular-dimse's C-FIND SCU.

Usage: findscp.py OUT_DIR
Prints the port it listens on (127.0.0.1, ephemeral), then serves until stdin closes.
Holds four studies and three series in memory; matches PatientName (prefix wildcard,
any case), ModalitiesInStudy, StudyDate (ranges) and StudyInstanceUID (series level),
and answers with the keys asked for. Each request's identifier is written to
OUT_DIR/find-<n>.json (keyword: value) and whether it was cancelled to cancel-<n>.json.
"""
import fnmatch
import json
import os
import sys
import time

from pydicom.dataset import Dataset
from pynetdicom import AE, evt
from pynetdicom.sop_class import StudyRootQueryRetrieveInformationModelFind, Verification

out = sys.argv[1]

STUDIES = [
    ("1.2.1", "FAKE^ALPHA", "20240105", ["CT"]),
    ("1.2.2", "FAKE^BRAVO", "20240210", ["MR"]),
    ("1.2.3", "TEST^ECHO", "20240301", ["CT", "SR"]),
    ("1.2.4", "Müller^Jörg", "20240302", ["US"]),
]
SERIES = [
    ("1.2.1", "1.2.1.1", 2, "CT"),
    ("1.2.1", "1.2.1.2", 1, "CT"),
    ("1.2.2", "1.2.2.1", 1, "MR"),
]

ae = AE(ae_title="ORACLE")
ae.add_supported_context(StudyRootQueryRetrieveInformationModelFind)
ae.add_supported_context(Verification)
count = [0]


def in_range(key, v):
    if "-" in key:
        a, b = key.split("-", 1)
        return (not a or v >= a) and (not b or v <= b)
    return v == key


def on_find(event):
    q = event.identifier
    n = count[0]
    count[0] += 1
    asked = {}
    for e in q:
        asked[e.keyword] = str(e.value) if e.value is not None else ""
    with open(os.path.join(out, "find-%d.json" % n), "w") as f:
        json.dump(asked, f)
    level = q.QueryRetrieveLevel
    rows = []
    if level == "STUDY":
        for uid, name, date, mods in STUDIES:
            pn = str(q.get("PatientName", "") or "")
            if pn and not fnmatch.fnmatch(name.lower(), pn.lower()):
                continue
            m = str(q.get("ModalitiesInStudy", "") or "")
            if m and m not in mods:
                continue
            d = str(q.get("StudyDate", "") or "")
            if d and not in_range(d, date):
                continue
            r = Dataset()
            r.QueryRetrieveLevel = "STUDY"
            r.SpecificCharacterSet = "ISO_IR 192"
            r.StudyInstanceUID = uid
            r.PatientName = name
            r.PatientID = "ID" + uid
            r.StudyDate = date
            r.StudyTime = "101500"
            r.StudyDescription = "Study " + uid
            r.ModalitiesInStudy = mods
            r.NumberOfStudyRelatedSeries = 2
            r.NumberOfStudyRelatedInstances = 40
            rows.append(r)
    elif level == "SERIES":
        st = str(q.get("StudyInstanceUID", "") or "")
        for s, uid, num, mod in SERIES:
            if st and st != s:
                continue
            r = Dataset()
            r.QueryRetrieveLevel = "SERIES"
            r.StudyInstanceUID = s
            r.SeriesInstanceUID = uid
            r.SeriesNumber = num
            r.Modality = mod
            r.SeriesDescription = "Series %d" % num
            r.NumberOfSeriesRelatedInstances = 20
            rows.append(r)
    cancelled = False
    for r in rows:
        # Slow enough that a C-CANCEL arrives between matches.
        time.sleep(0.05)
        if event.is_cancelled:
            cancelled = True
            break
        yield (0xFF00, r)
    with open(os.path.join(out, "cancel-%d.json" % n), "w") as f:
        json.dump(cancelled, f)
    if cancelled:
        yield (0xFE00, None)


server = ae.start_server(
    ("127.0.0.1", 0), block=False, evt_handlers=[(evt.EVT_C_FIND, on_find)]
)
print(server.server_address[1], flush=True)
sys.stdin.read()
server.shutdown()
