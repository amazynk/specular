#!/usr/bin/env python3
"""List or extract members of a remote ZIP archive with HTTP range requests, so that a few
small files can be taken from a multi-gigabyte archive (the JPEG XS conformance set of
ISO/IEC 21122-4) without downloading all of it.

    jpegxs-zip-range.py list URL [SUBSTRING]
    jpegxs-zip-range.py get URL MEMBER OUTFILE [SHA256]
    jpegxs-zip-range.py many URL < LIST   (lines "SHA256 MEMBER OUTFILE")

Standard library only. Stored and deflated members are supported, ZIP64 included.
"""

import hashlib
import os
import struct
import sys
import urllib.request
import zlib


def fetch(url, start, end):
    req = urllib.request.Request(url, headers={"Range": f"bytes={start}-{end}"})
    with urllib.request.urlopen(req, timeout=120) as r:
        if r.status != 206:
            raise SystemExit(f"server ignored the range request ({r.status})")
        return r.read()


def size_of(url):
    req = urllib.request.Request(url, method="HEAD")
    with urllib.request.urlopen(req, timeout=60) as r:
        return int(r.headers["Content-Length"])


def central_directory(url):
    total = size_of(url)
    tail = fetch(url, max(0, total - 65558), total - 1)
    i = tail.rfind(b"PK\x05\x06")
    if i < 0:
        raise SystemExit("no end of central directory record")
    (_, _, _, _, count, cd_size, cd_off, _) = struct.unpack_from("<IHHHHIIH", tail, i)
    if cd_off == 0xFFFFFFFF or count == 0xFFFF:
        j = tail.rfind(b"PK\x06\x07")
        (_, _, z64_off, _) = struct.unpack_from("<IIQI", tail, j)
        rec = fetch(url, z64_off, z64_off + 55)
        (_, _, _, _, _, _, count, _, cd_size, cd_off) = struct.unpack_from("<IQHHIIQQQQ", rec, 0)
    cd = fetch(url, cd_off, cd_off + cd_size - 1)
    out = []
    p = 0
    for _ in range(count):
        (sig, _, _, _, method, _, _, crc, csize, usize, nlen, xlen, clen, _, _, _, off) = struct.unpack_from(
            "<IHHHHHHIIIHHHHHII", cd, p)
        if sig != 0x02014B50:
            raise SystemExit("bad central directory entry")
        name = cd[p + 46:p + 46 + nlen].decode("utf-8", "replace")
        extra = cd[p + 46 + nlen:p + 46 + nlen + xlen]
        q = 0
        while q + 4 <= len(extra):
            (tag, ln) = struct.unpack_from("<HH", extra, q)
            if tag == 1:
                vals = list(struct.unpack_from("<" + "Q" * (ln // 8), extra, q + 4))
                if usize == 0xFFFFFFFF:
                    usize = vals.pop(0)
                if csize == 0xFFFFFFFF:
                    csize = vals.pop(0)
                if off == 0xFFFFFFFF:
                    off = vals.pop(0)
            q += 4 + ln
        out.append((name, method, csize, usize, off, crc))
        p += 46 + nlen + xlen + clen
    return out


def get(url, entry):
    (name, method, csize, usize, off, crc) = entry
    head = fetch(url, off, off + 29)
    (nlen, xlen) = struct.unpack_from("<HH", head, 26)
    start = off + 30 + nlen + xlen
    data = fetch(url, start, start + csize - 1) if csize else b""
    if method == 8:
        data = zlib.decompress(data, -15)
    elif method != 0:
        raise SystemExit(f"{name}: compression method {method} not supported")
    if zlib.crc32(data) & 0xFFFFFFFF != crc:
        raise SystemExit(f"{name}: CRC mismatch")
    return data


def main():
    if len(sys.argv) >= 3 and sys.argv[1] == "list":
        sub = sys.argv[3] if len(sys.argv) > 3 else ""
        for (name, _, csize, usize, _, _) in central_directory(sys.argv[2]):
            if sub in name:
                print(f"{usize}\t{csize}\t{name}")
    elif len(sys.argv) == 3 and sys.argv[1] == "many":
        # stdin: "SHA256 MEMBER OUTFILE" per line; existing files with the right hash stay.
        url = sys.argv[2]
        entries = None
        for line in sys.stdin:
            parts = line.split()
            if len(parts) != 3:
                continue
            want, member, outfile = parts
            try:
                with open(outfile, "rb") as f:
                    if hashlib.sha256(f.read()).hexdigest() == want:
                        continue
            except OSError:
                pass
            if entries is None:
                entries = {e[0]: e for e in central_directory(url)}
            if member not in entries:
                raise SystemExit(f"{member}: not in the archive")
            data = get(url, entries[member])
            digest = hashlib.sha256(data).hexdigest()
            if digest != want:
                raise SystemExit(f"{member}: sha256 {digest}, expected {want}")
            with open(outfile + ".part", "wb") as f:
                f.write(data)
            os.replace(outfile + ".part", outfile)
            print(f"{member}: {len(data)} bytes")
    elif len(sys.argv) >= 5 and sys.argv[1] == "get":
        url, member, outfile = sys.argv[2:5]
        want = sys.argv[5] if len(sys.argv) > 5 else None
        entry = next((e for e in central_directory(url) if e[0] == member), None)
        if entry is None:
            raise SystemExit(f"{member}: not in the archive")
        data = get(url, entry)
        digest = hashlib.sha256(data).hexdigest()
        if want and digest != want:
            raise SystemExit(f"{member}: sha256 {digest}, expected {want}")
        with open(outfile, "wb") as f:
            f.write(data)
        print(f"{digest}  {outfile}")
    else:
        raise SystemExit(__doc__)


if __name__ == "__main__":
    main()
