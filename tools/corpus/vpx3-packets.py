#!/usr/bin/env python3
"""Write the video packets of an AVI, Ogg or FLV file as NAME.pkts for the crate tests.

Usage: vpx3-packets.py IN OUT

OUT holds each packet of the first video stream as a 32-bit little-endian length and the
bytes (Ogg: the Theora stream's packets, headers included; FLV: the video tag bodies without
their one-byte codec header). Only the container framing is read; the input is untrusted data
and nothing in it is run.
"""
import struct
import sys


def avi_packets(d):
    out = []
    vids = None
    stream = 0

    def walk(p, end):
        nonlocal vids, stream
        while p + 8 <= end:
            tag, size = d[p:p + 4], struct.unpack_from("<I", d, p + 4)[0]
            body = p + 8
            if tag in (b"RIFF", b"LIST"):
                walk(body + 4, min(body + size, end))
            elif tag == b"strh":
                if d[body:body + 4] == b"vids" and vids is None:
                    vids = stream
                stream += 1
            elif vids is not None and tag[2:] in (b"dc", b"db") and tag[:2] == b"%02d" % vids:
                out.append(d[body:body + size])
            p = body + size + (size & 1)

    walk(0, len(d))
    return out


def ogg_packets(d):
    streams = {}
    order = []
    p = 0
    while p + 27 <= len(d) and d[p:p + 4] == b"OggS":
        serial = struct.unpack_from("<I", d, p + 14)[0]
        nseg = d[p + 26]
        lace = d[p + 27:p + 27 + nseg]
        q = p + 27 + nseg
        st = streams.setdefault(serial, [[], b""])
        if serial not in order:
            order.append(serial)
        for n in lace:
            st[1] += d[q:q + n]
            q += n
            if n < 255:
                st[0].append(st[1])
                st[1] = b""
        p = q
    for s in order:
        pk = streams[s][0]
        if pk and pk[0][:7] == b"\x80theora":
            return pk
    return []


def flv_packets(d):
    out = []
    p = struct.unpack_from(">I", d, 5)[0] + 4
    while p + 11 <= len(d):
        kind = d[p] & 0x1F
        size = int.from_bytes(d[p + 1:p + 4], "big")
        body = d[p + 11:p + 11 + size]
        if kind == 9 and body:
            out.append(body[1:])
        p += 11 + size + 4
    return out


def main():
    src, dst = sys.argv[1], sys.argv[2]
    d = open(src, "rb").read()
    if d[:4] == b"RIFF":
        pk = avi_packets(d)
    elif d[:4] == b"OggS":
        pk = ogg_packets(d)
    elif d[:3] == b"FLV":
        pk = flv_packets(d)
    else:
        sys.exit(f"{src}: not AVI, Ogg or FLV")
    with open(dst, "wb") as f:
        for x in pk:
            f.write(struct.pack("<I", len(x)))
            f.write(x)
    print(f"{dst}: {len(pk)} packets")


main()
