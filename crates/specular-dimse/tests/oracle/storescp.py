"""pynetdicom C-ECHO/C-STORE SCP: the oracle for specular-dimse's SCU (tests/oracle.rs).

Usage: storescp.py OUT_DIR SYNTAXES MAX_PDU
  SYNTAXES: "all" (every transfer syntax pynetdicom knows) or a comma list of UIDs.
Prints the port it listens on (127.0.0.1, ephemeral), then serves until stdin closes.
For each instance stored it writes OUT_DIR/<uid>.raw (the data set bytes as received)
and OUT_DIR/<uid>.json (what pydicom read: transfer syntax, rows, columns, pixel data
length, the first code value of (0008,2218), the calling AE title).
"""
import json
import os
import sys

from pynetdicom import AE, ALL_TRANSFER_SYNTAXES, AllStoragePresentationContexts, evt
from pynetdicom.sop_class import Verification

out, syntaxes, max_pdu = sys.argv[1], sys.argv[2], int(sys.argv[3])
ts = ALL_TRANSFER_SYNTAXES if syntaxes == "all" else syntaxes.split(",")

ae = AE(ae_title="ORACLE")
ae.maximum_pdu_size = max_pdu
for cx in AllStoragePresentationContexts:
    ae.add_supported_context(cx.abstract_syntax, ts)
ae.add_supported_context(Verification)


def on_store(event):
    raw = event.request.DataSet.getvalue()
    ds = event.dataset
    uid = str(ds.SOPInstanceUID)
    with open(os.path.join(out, uid + ".raw"), "wb") as f:
        f.write(raw)
    seq = ds.get("AnatomicRegionSequence")
    info = {
        "ts": str(event.context.transfer_syntax),
        "rows": int(ds.Rows),
        "columns": int(ds.Columns),
        "pixel_len": len(ds.PixelData) if "PixelData" in ds else 0,
        "code": str(seq[0].CodeValue) if seq else "",
        "calling": event.assoc.requestor.ae_title,
        "patient": str(ds.get("PatientName", "")),
    }
    with open(os.path.join(out, uid + ".json"), "w") as f:
        json.dump(info, f)
    return 0x0000


server = ae.start_server(
    ("127.0.0.1", 0), block=False, evt_handlers=[(evt.EVT_C_STORE, on_store)]
)
print(server.server_address[1], flush=True)
sys.stdin.read()
server.shutdown()
