"""Independent readings of Testo .bmt files (testo 865-883), for tests/bmt_is2.rs.

Usage: python3 -I gen/bmt_oracle.py DIR OUT.tsv [STRIDE]

Standard library only; written apart from the Rust reader, from Testo's published layout
(github.com/testo/read_bmt): the <ToFo> header, the XML schema (parsed with ElementTree),
one separator byte, then each item's bytes in schema order. Rows:
  size    file  width height
  minmax  file  min max     the camera's own scale minimum and maximum (°C), when it scaled
                            automatically (they are then the image's extremes)
  spot    file  x y hot|cold the hot and cold spots the camera marked
  px      file  index count celsius   every STRIDE-th pixel (count * 1102 / 65535 - 101)
"""

import struct
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

DIR, OUT = Path(sys.argv[1]), Path(sys.argv[2])
STRIDE = int(sys.argv[3]) if len(sys.argv) > 3 else 7


def read(path):
    d = path.read_bytes()
    i = d.find(b"<ToFo")
    j = d.find(b"</ToFo>", i) + len(b"</ToFo>")
    tofo = ET.fromstring(d[i:j])
    xml_size = int(tofo.find("xml").get("size"))
    order = "<" if tofo.find("data").get("endianness") == "little" else ">"
    schema = ET.fromstring(d[j:j + xml_size])
    p = j + xml_size + 1
    items = {}

    def walk(node, path):
        nonlocal p
        for c in node:
            if c.tag == "group":
                walk(c, path + [c.get("name")])
            else:
                n = int(c.get("size"))
                items["/".join(path + [c.get("name")])] = (c.get("type"), d[p:p + n])
                p += n

    walk(schema, [])
    return items, order


rows = []
for path in sorted(DIR.iterdir()):
    if path.suffix.lower() != ".bmt" or path.read_bytes()[:2] == b"BM":
        continue
    items, o = read(path)
    mat = items["Images/Ir"][1]
    _, rows_, cols, _, _, _ = struct.unpack(o + "6I", mat[:24])
    counts = struct.unpack(f"{o}{rows_ * cols}H", mat[24:24 + 2 * rows_ * cols])
    name = path.name
    rows.append(f"{name}\tsize\t{cols}\t{rows_}")
    mode = items.get("Scaling/Mode")
    if mode and mode[1][4:4 + struct.unpack(o + "I", mode[1][:4])[0]] == b"Auto":
        k = lambda key: struct.unpack(o + "f", items[key][1])[0] - 273.15
        rows.append(f"{name}\tminmax\t{k('Scaling/TempMin'):.6f}\t{k('Scaling/TempMax'):.6f}")
    for spot in ("HotSpot", "ColdSpot"):
        key = f"MeasPoints/{spot}/Position"
        if key in items:
            x, y = struct.unpack(o + "2i", items[key][1])
            rows.append(f"{name}\tspot\t{x}\t{y}\t{'hot' if spot == 'HotSpot' else 'cold'}")
    for i in range(0, len(counts), STRIDE):
        c = counts[i]
        rows.append(f"{name}\tpx\t{i}\t{c}\t{c * 1102 / 65535 - 101:.6f}")

OUT.write_text("# file\trow\tvalues\n" + "\n".join(rows) + "\n")
print(f"{OUT}: {len(rows)} rows")
