#!/usr/bin/env python3
"""Checks specular-mpa's Layer III Huffman tables against an independent copy.

The reference is minimp3.h (CC0 1.0, https://github.com/lieff/minimp3), whose packed
lookup tables encode the ISO/IEC 11172-3 Annex B tables. This script reads that file
(it is never committed; tools/corpus/fetch-mp3.sh fetches it) and, for every code
word in crates/formats/specular-mpa/src/huffman_data.rs, emulates minimp3's lookup and
checks it decodes to the same (x, y) / (v, w, x, y) with the same length. It also checks
each of our tables is a complete prefix code (Kraft sum 1), so equal decodes of all our
code words means the two tables are identical.

Usage: python3 -I mp3-tables-check.py <minimp3.h> <huffman_data.rs>
"""
import re
import sys
from fractions import Fraction


def c_array(src, name):
    m = re.search(r"static const \w+ " + name + r"\[[^\]]*\]\s*=\s*\{(.*?)\};", src, re.S)
    return [int(v) for v in re.findall(r"-?\d+", m.group(1))]


def ours(src):
    out = {}
    pat = r"pub\(crate\) const (\w+): \[\(u8, u32\); \d+\] = \[(.*?)\];"
    for m in re.finditer(pat, src, re.S):
        words = re.findall(r"\((\d+), 0x([0-9a-f]+)\)", m.group(2))
        out[m.group(1)] = [(int(a), int(b, 16)) for a, b in words]
    return out


def peek(bits, pos, n):
    v = 0
    for i in range(n):
        v = v * 2 + (bits[pos + i] if pos + i < len(bits) else 0)
    return v


def mm_big(tabs, base, bits):
    pos, w = 0, 5
    leaf = tabs[base + peek(bits, pos, w)]
    while leaf < 0:
        pos += w
        w = leaf & 7
        leaf = tabs[base + peek(bits, pos, w) - (leaf >> 3)]
    pos += leaf >> 8
    return (leaf & 15, (leaf >> 4) & 15), pos


def mm_count1(tab, bits):
    leaf = tab[peek(bits, 0, 4)]
    if not leaf & 8:
        leaf = tab[(leaf >> 3) + peek(bits, 4, leaf & 3)]
    q = tuple((leaf >> (7 - s)) & 1 for s in range(4))
    return q, leaf & 7


def main():
    mm = open(sys.argv[1]).read()
    tabs = c_array(mm, "tabs")
    tab32 = c_array(mm, "tab32")
    tab33 = c_array(mm, "tab33")
    tabindex = c_array(mm, "tabindex")
    linbits = c_array(mm, "g_linbits")
    t = ours(open(sys.argv[2]).read())
    sizes = {1: 2, 2: 3, 3: 3, 5: 4, 6: 4, 7: 6, 8: 6, 9: 6, 10: 8, 11: 8, 12: 8,
             13: 16, 15: 16, 16: 16, 24: 16}
    bad = 0
    checked = 0
    for n, size in sizes.items():
        words = t["T%d" % n]
        assert len(words) == size * size, n
        kraft = sum(Fraction(1, 2 ** ln) for ln, _ in words)
        if kraft != 1:
            print("table %d: Kraft sum %s" % (n, kraft))
            bad += 1
        for sym, (ln, code) in enumerate(words):
            bits = [(code >> (ln - 1 - i)) & 1 for i in range(ln)]
            got, glen = mm_big(tabs, tabindex[n], bits)
            want = (sym // size, sym % size)
            checked += 1
            if got != want or glen != ln:
                print("table %d sym %s: ours len %d code %x -> minimp3 %s len %d"
                      % (n, want, ln, code, got, glen))
                bad += 1
    # Tables 16..23 and 24..31 share one code each; only the ESC widths differ.
    for n in range(16, 32):
        if tabindex[n] != tabindex[16 if n < 24 else 24]:
            print("table %d: minimp3 uses another code" % n)
            bad += 1
    want_lin = [1, 2, 3, 4, 6, 8, 10, 13, 4, 5, 6, 7, 8, 9, 11, 13]
    if linbits[16:32] != want_lin:
        print("linbits differ: %s" % linbits[16:32])
        bad += 1
    words = t["COUNT1_A"]
    if sum(Fraction(1, 2 ** ln) for ln, _ in words) != 1:
        print("count1 A: incomplete")
        bad += 1
    for sym, (ln, code) in enumerate(words):
        bits = [(code >> (ln - 1 - i)) & 1 for i in range(ln)]
        got, glen = mm_count1(tab32, bits)
        want = tuple((sym >> (3 - s)) & 1 for s in range(4))
        checked += 1
        if got != want or glen != ln:
            print("count1 A sym %s: ours len %d code %x -> minimp3 %s len %d"
                  % (want, ln, code, got, glen))
            bad += 1
    # Count1 table B: 4-bit words, vwxy inverted.
    for sym in range(16):
        bits = [((15 - sym) >> (3 - i)) & 1 for i in range(4)]
        got, glen = mm_count1(tab33, bits)
        checked += 1
        if got != tuple((sym >> (3 - s)) & 1 for s in range(4)) or glen != 4:
            print("count1 B sym %d: minimp3 %s len %d" % (sym, got, glen))
            bad += 1
    print("checked %d code words, %d mismatches" % (checked, bad))
    sys.exit(1 if bad else 0)


main()
