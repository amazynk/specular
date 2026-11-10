"""Generates the character set tables of src/charset.rs (PS3.5 §6.1, Specific Character Set).

Sources, downloaded by this script into SRC (default: <repository>/target/charset-src, which is
gitignored; nothing downloaded is committed) and checked against the SHA-256 sums below:

- the Unicode consortium's ISO 8859 mapping files (www.unicode.org/Public/MAPPINGS/ISO8859),
  for the single-byte sets (ISO_IR 100, 101, 109, 110, 144, 127, 126, 138, 148, 203, 166);
- the WHATWG Encoding Standard's index files (encoding.spec.whatwg.org), for JIS X 0208 and
  JIS X 0212 (ISO 2022 IR 87 / 159), KS X 1001 (ISO 2022 IR 149, from index-euc-kr), and
  GBK / GB18030 / GB2312 (from index-gb18030 and index-gb18030-ranges).

Run from the crate folder with any Python 3:

    python3 -I gen/charset.py [SRC]

Writes src/charset_table.rs (the single-byte sets and the GB18030 four-byte ranges) and the
double-byte tables in src/charset_data/ (little-endian u16 code points, 0 for unmapped):
jis0208.bin, jis0212.bin and ksx1001.bin (94 x 94 cells, row-major from 0x21 0x21 /
0xA1 0xA1), gb18030.bin (126 x 190 cells, lead 0x81..0xFE, trail 0x40..0x7E then 0x80..0xFE).
"""
import hashlib
import os
import sys
import urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
CRATE = os.path.dirname(HERE)
REPO = os.path.dirname(os.path.dirname(os.path.dirname(CRATE)))

UNICODE = "https://www.unicode.org/Public/MAPPINGS/ISO8859/"
WHATWG = "https://encoding.spec.whatwg.org/"

# (DICOM defined term, mapping file) in the order of `SingleByte` in charset.rs.
SINGLE = [
    ("ISO_IR 100", "8859-1.TXT"),
    ("ISO_IR 101", "8859-2.TXT"),
    ("ISO_IR 109", "8859-3.TXT"),
    ("ISO_IR 110", "8859-4.TXT"),
    ("ISO_IR 144", "8859-5.TXT"),
    ("ISO_IR 127", "8859-6.TXT"),
    ("ISO_IR 126", "8859-7.TXT"),
    ("ISO_IR 138", "8859-8.TXT"),
    ("ISO_IR 148", "8859-9.TXT"),
    ("ISO_IR 203", "8859-15.TXT"),
    ("ISO_IR 166", "8859-11.TXT"),
]

# Sums of the files the committed tables were generated from; a changed upstream file stops
# the script (review the change, then update the sum).
SHA256 = {
    '8859-1.TXT': '2a06a462bf74ae65a77b700f235ac05156ac20f4aaefe39f09cc20d50628bb5e',
    '8859-2.TXT': '020946e114335b17e7d29fe8a8449786cffed714cf409b11d0b3a375197188d1',
    '8859-3.TXT': '23b2d7815946cb1232c71a1543033afc396c750ffa19a3771662cf23e6194552',
    '8859-4.TXT': 'e717699903bef2144234279a598ce39b44502942c9cc23a2ce2069a65f46b00d',
    '8859-5.TXT': '9a2efea39673f95c5daa277c46ad09a41e95f27c225270424a604d033db9bd8f',
    '8859-6.TXT': '7012241bd7143a3bf98861e497cd3cc092edebc3e063fc7d576420f22d22f11e',
    '8859-7.TXT': '94f4a1c89ba6b2e1c1f8b30676cb5d15f232dbac36ae4ad20018a65e0f3fb9a3',
    '8859-8.TXT': 'cbe3205c5c6f92589d6a8673d56c51d89659ae290185457e6e2f81fee7a1ad2d',
    '8859-9.TXT': '4397082eb5cbbc299264b3b73405cc0946ec5c79328eb9de75f8a5da1ab8e712',
    '8859-15.TXT': 'bc72d7bd5be1d15bdad8e684f9ae9f573c361bb73812c3084ed1b7622a05e58f',
    '8859-11.TXT': 'a6c8891b5e43bbcf55ac356b0a31be8c8322a09fba183b7b7507e9252587bc13',
    'index-jis0208.txt': '341dcde7e8b984e9c7bbf5ed75c8da7c6087d47083a1a2b3ed558bfd5bef9468',
    'index-jis0212.txt': '6ac698a473822b282ff4f4d4f17873b970612ef9bd232e03941854c09b67c7ee',
    'index-euc-kr.txt': '89af20dd867c84cefb710b1790229786cfef2bf11916361a210d81b90381e267',
    'index-gb18030.txt': '746b3c55f1a8ec4b90b451f384437a17fd37cd51cd668456a28071a758d10784',
    'index-gb18030-ranges.txt': '874c6b6f6f74cf7d427ad228d5b41ddd9354fffd92a2259bf429f86e6baa7a1e',
}


def fetch(src, base, name):
    path = os.path.join(src, name)
    if not os.path.exists(path):
        os.makedirs(src, exist_ok=True)
        with urllib.request.urlopen(base + name, timeout=60) as r:
            data = r.read()
        with open(path, "wb") as f:
            f.write(data)
    with open(path, "rb") as f:
        data = f.read()
    digest = hashlib.sha256(data).hexdigest()
    want = SHA256.get(name)
    if want is not None and want != digest:
        sys.exit("%s: sha256 %s, expected %s (upstream changed; review and update SHA256)"
                 % (name, digest, want))
    if want is None:
        print("    %r: %r," % (name, digest))
    return data.decode("utf-8")


def unicode_map(text):
    """byte -> code point from a Unicode consortium mapping file."""
    out = {}
    for line in text.splitlines():
        line = line.split("#", 1)[0].strip()
        if not line:
            continue
        a, b = line.split()[:2]
        out[int(a, 16)] = int(b, 16)
    return out


def whatwg_index(text):
    """pointer -> code point from a WHATWG index file."""
    out = {}
    for line in text.splitlines():
        line = line.split("#", 1)[0].strip()
        if not line:
            continue
        p, c = line.split()[:2]
        out[int(p)] = int(c, 16)
    return out


def write_bin(path, cells):
    for c in cells:
        assert 0 <= c <= 0xFFFF, hex(c)
    with open(path, "wb") as f:
        f.write(b"".join(c.to_bytes(2, "little") for c in cells))


def main(src):
    single = []
    for term, name in SINGLE:
        m = unicode_map(fetch(src, UNICODE, name))
        for b in range(0x80):
            assert m.get(b, b) == b, (name, b)
        single.append((term, [m.get(b, 0xFFFD) for b in range(0xA0, 0x100)]))

    jis0208 = whatwg_index(fetch(src, WHATWG, "index-jis0208.txt"))
    jis0212 = whatwg_index(fetch(src, WHATWG, "index-jis0212.txt"))
    euckr = whatwg_index(fetch(src, WHATWG, "index-euc-kr.txt"))
    gb = whatwg_index(fetch(src, WHATWG, "index-gb18030.txt"))
    ranges_text = fetch(src, WHATWG, "index-gb18030-ranges.txt")
    ranges = sorted(whatwg_index(ranges_text).items())

    data = os.path.join(CRATE, "src", "charset_data")
    os.makedirs(data, exist_ok=True)
    # 94 x 94 sets: row r, cell c (0-based) is pointer r * 94 + c in the JIS indexes.
    write_bin(os.path.join(data, "jis0208.bin"),
              [jis0208.get(p, 0) for p in range(94 * 94)])
    write_bin(os.path.join(data, "jis0212.bin"),
              [jis0212.get(p, 0) for p in range(94 * 94)])
    # EUC-KR / GB18030 pointers: (lead - 0x81) * 190 + (trail - 0x41); KS X 1001 is the
    # 0xA1..0xFE square of EUC-KR.
    write_bin(os.path.join(data, "ksx1001.bin"),
              [euckr.get((0xA1 + r - 0x81) * 190 + (0xA1 + c - 0x41), 0)
               for r in range(94) for c in range(94)])
    write_bin(os.path.join(data, "gb18030.bin"), [gb.get(p, 0) for p in range(126 * 190)])

    out = os.path.join(CRATE, "src", "charset_table.rs")
    with open(out, "w", encoding="utf-8") as f:
        f.write("// @generated by gen/charset.py from the Unicode consortium's ISO 8859 mapping\n")
        f.write("// files and the WHATWG Encoding Standard's index-gb18030-ranges. Do not edit.\n\n")
        f.write("/// Code points of bytes 0xA0..=0xFF of each single-byte set (0xFFFD: unassigned),\n")
        f.write("/// in the order of `SingleByte`.\n")
        f.write("#[rustfmt::skip]\n")
        f.write("pub(crate) static SINGLE_BYTE: [[u16; 96]; %d] = [\n" % len(single))
        for term, cells in single:
            f.write("    // %s\n    [\n" % term)
            for i in range(0, 96, 12):
                f.write("        " + ", ".join("0x%04X" % c for c in cells[i:i + 12]) + ",\n")
            f.write("    ],\n")
        f.write("];\n\n")
        f.write("/// GB18030 four-byte ranges: (pointer, first code point), by pointer.\n")
        f.write("#[rustfmt::skip]\n")
        f.write("pub(crate) static GB18030_RANGES: [(u32, u32); %d] = [\n" % len(ranges))
        for i in range(0, len(ranges), 6):
            f.write("    " + " ".join("(%d, 0x%04X)," % r for r in ranges[i:i + 6]) + "\n")
        f.write("];\n")


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else os.path.join(REPO, "target", "charset-src"))
