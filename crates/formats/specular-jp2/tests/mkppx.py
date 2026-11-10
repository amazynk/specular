"""Rewrite a codestream coded with SOP and EPH markers to carry its packet headers in PPT
(tile-part headers) or PPM (main header) marker segments. OpenJPEG cannot write either.

Each packet header is the bytes after an SOP segment up to and including its EPH marker (FF92);
the bit-stuffing rules keep FF92 out of header bits and FF91 out of packet bodies.
Usage: mkppx.py ppt|ppm in.j2k out.j2k"""
import struct, sys

mode, src, dst = sys.argv[1:4]
d = open(src, "rb").read()
first_sot = d.index(b"\xff\x90")
main = d[:first_sot]
parts = []  # (tile-part header markers without SOT/SOD, isot, tpsot, tnsot, headers, body)
pos = first_sot
while d[pos:pos + 2] == b"\xff\x90":
    _, isot, psot, tpsot, tnsot = struct.unpack(">HHIBB", d[pos + 2:pos + 12])
    end = pos + psot if psot else len(d) - 2
    sod = d.index(b"\xff\x93", pos + 12)
    tph = d[pos + 12:sod]
    data = d[sod + 2:end]
    headers, body = bytearray(), bytearray()
    i = 0
    while i < len(data):
        assert data[i:i + 2] == b"\xff\x91", "expected SOP"
        body += data[i:i + 6]
        e = data.index(b"\xff\x92", i + 6) + 2
        headers += data[i + 6:e]
        nxt = data.find(b"\xff\x91", e)
        nxt = len(data) if nxt < 0 else nxt
        body += data[e:nxt]
        i = nxt
    parts.append((tph, isot, tpsot, tnsot, bytes(headers), bytes(body)))
    pos = end


def segments(marker, payload, z0=0, chunk=60000):
    """Marker segments carrying `payload`; returns (bytes, next Z index)."""
    out = bytearray()
    z = z0
    for s in range(0, max(len(payload), 1), chunk):
        piece = payload[s:s + chunk]
        out += marker + struct.pack(">HB", 3 + len(piece), z) + piece
        z += 1
    return bytes(out), z


out = bytearray()
if mode == "ppm":
    stream = b"".join(struct.pack(">I", len(p[4])) + p[4] for p in parts)
    out += main + segments(b"\xff\x60", stream)[0]
else:
    out += main
zppt = {}  # Zppt runs across all tile-parts of a tile
for tph, isot, tpsot, tnsot, headers, body in parts:
    if mode == "ppt":
        seg, zppt[isot] = segments(b"\xff\x61", headers, zppt.get(isot, 0))
        tph = tph + seg
    psot = 12 + len(tph) + 2 + len(body)
    out += b"\xff\x90" + struct.pack(">HHIBB", 10, isot, psot, tpsot, tnsot) + tph + b"\xff\x93" + body
out += b"\xff\xd9"
open(dst, "wb").write(out)
