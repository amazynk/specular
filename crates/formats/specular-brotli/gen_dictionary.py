#!/usr/bin/env python3
"""Regenerates src/dictionary.bin, the Brotli static dictionary (RFC 7932 Appendix A).

    python3 -I crates/formats/specular-brotli/gen_dictionary.py [rfc7932.txt]

Without an argument the RFC text is fetched from the RFC Editor. The dictionary is the hex dump
of Appendix A; it is checked against the length and CRC-32 the RFC gives and against a pinned
SHA-256 before anything is written.
"""
import hashlib
import os
import re
import sys
import urllib.request
import zlib

URL = "https://www.rfc-editor.org/rfc/rfc7932.txt"
SIZE = 122784
CRC32 = 0x5136CB04  # RFC 7932 Appendix A
SHA256 = "20e42eb1b511c21806d4d227d07e5dd06877d8ce7b3a817f378f313653f35c70"

if len(sys.argv) > 1:
    with open(sys.argv[1], "rb") as f:
        text = f.read().decode("ascii")
else:
    with urllib.request.urlopen(URL, timeout=60) as r:
        text = r.read().decode("ascii")

start = text.index("Appendix A.  Static Dictionary Data")
start = text.index("0x5136cb04.", start)
end = text.index("The number of words for each length", start)
hexdump = "".join(
    line.strip()
    for line in text[start:end].splitlines()
    if re.fullmatch(r"\s{6}[0-9a-f]+", line)
)
data = bytes.fromhex(hexdump)

if len(data) != SIZE:
    sys.exit(f"dictionary is {len(data)} bytes, want {SIZE}")
if zlib.crc32(data) != CRC32:
    sys.exit(f"dictionary CRC-32 {zlib.crc32(data):#010x}, want {CRC32:#010x}")
digest = hashlib.sha256(data).hexdigest()
if digest != SHA256:
    sys.exit(f"dictionary SHA-256 {digest}, want {SHA256}")

out = os.path.join(os.path.dirname(os.path.abspath(__file__)), "src", "dictionary.bin")
with open(out, "wb") as f:
    f.write(data)
print(f"wrote {out} ({len(data)} bytes, sha256 {digest})")
