#!/usr/bin/env python3
"""Finds the MPEG audio Layer III Huffman code tables (ISO/IEC 11172-3 Annex B, Tables
B.7) by probing a reference decoder, without copying any implementation's tables.

Every probe is a synthetic MPEG-1 Layer III frame (48 kHz, 320 kbit/s, mono, long blocks,
no scalefactors) whose first granule holds one Huffman pair (or one count1 quadruple)
followed by bits that decode to nothing, then two silent frames. ffmpeg decodes the
probe file to float; the decoded spectral lines are read back by projecting each group's
samples onto the responses of single lines (measured the same way with count1 table B,
whose 4-bit codes are the complement of the quadruple and need no table). A depth-first
walk over the code tree finds every code word: the word that starts a bit string is as long
as the shortest prefix after which flipping the next bit no longer changes the symbol.

Output: crates/formats/specular-mpa/src/huffman_data.rs (code words by symbol). The decoder
itself is checked against ffmpeg on real streams by the crate's and specular-video's tests.

  python3 -I tools/corpus/mp3-huffman-probe.py WORKDIR [OUT.rs]

Needs ffmpeg and numpy. Takes a few minutes.
"""
import os
import subprocess
import sys

import numpy as np

FRAME = 960  # bytes: 144 * 320000 / 48000
GROUP = 3 * 1152  # samples a probe and its two silent frames take
GAIN = 154  # global gain: a line of value 1 is 2^-14

# Tables: number -> (size of x and y, linbits). Tables 16-23 and 24-31 share a tree each.
TABLES = {1: (2, 0), 2: (3, 0), 3: (3, 0), 5: (4, 0), 6: (4, 0), 7: (6, 0), 8: (6, 0),
          9: (6, 0), 10: (8, 0), 11: (8, 0), 12: (8, 0), 13: (16, 0), 15: (16, 0),
          16: (16, 1), 24: (16, 4)}


class Bits:
    def __init__(self):
        self.s = []

    def put(self, v, n):
        for i in range(n - 1, -1, -1):
            self.s.append((v >> i) & 1)

    def bytes(self, total):
        s = self.s + [0] * (-len(self.s) % 8)
        b = bytes(int("".join(map(str, s[i:i + 8])), 2) for i in range(0, len(s), 8))
        return b + bytes(total - len(b))


def frame(huff, table=0, big_values=0, count1b=True):
    """One frame: granule 0 holds the bit string `huff`, granule 1 nothing."""
    b = Bits()
    b.put(0xFFFB, 16)  # MPEG-1 Layer III, no CRC
    b.put(14, 4)  # 320 kbit/s
    b.put(1, 2)  # 48 kHz
    b.put(0, 2)  # no padding, private
    b.put(3, 2)  # mono
    b.put(0, 6)
    b.put(0, 9)  # main_data_begin
    b.put(0, 5)  # private bits
    b.put(0, 4)  # scfsi
    for gr in range(2):
        n = len(huff) if gr == 0 else 0
        b.put(n, 12)  # part2_3_length
        b.put(big_values if gr == 0 else 0, 9)
        b.put(GAIN if gr == 0 else 0, 8)
        b.put(0, 4)  # scalefac_compress: no scalefactors
        b.put(0, 1)  # no window switching
        for _ in range(3):
            b.put(table if gr == 0 else 0, 5)
        b.put(15, 4)  # region0_count
        b.put(7, 3)  # region1_count
        b.put(0, 1)  # preflag
        b.put(0, 1)  # scalefac_scale
        b.put(1 if count1b else 0, 1)
    assert len(b.s) == 32 + 136
    for bit in huff:
        b.s.append(bit)
    return b.bytes(FRAME)


SILENT = frame([])


def decode(frames, work):
    """ffmpeg's float decode of the probe groups, one row of GROUP samples per probe."""
    path = os.path.join(work, "probe.mp3")
    with open(path, "wb") as f:
        for fr in frames:
            f.write(fr + SILENT + SILENT)
    out = subprocess.run(
        ["ffmpeg", "-nostdin", "-loglevel", "error", "-f", "mp3", "-i", path,
         "-f", "f32le", "-"], check=True, capture_output=True).stdout
    pcm = np.frombuffer(out, dtype="<f4").astype(np.float64)
    pcm = np.concatenate([pcm, np.zeros(len(frames) * GROUP + GROUP - len(pcm))])
    return pcm[:len(frames) * GROUP].reshape(len(frames), GROUP)


FILL = [1] * 96  # decodes to nothing in count1 region B (and to (0,0,0,0) in region A)


def count1b_word(line):
    """Count1 table B bits for a +1 at `line` (0..7) of the count1 region."""
    bits = []
    for q in range(line // 4 + 1):
        if q < line // 4:
            bits += [1, 1, 1, 1]
        else:
            v = 8 >> (line % 4)
            word = 15 - v
            bits += [(word >> i) & 1 for i in (3, 2, 1, 0)] + [0]
    return bits


def bases(work):
    rows = decode([frame(count1b_word(k) + FILL) for k in range(8)], work)
    return rows  # (8, GROUP)


class Prober:
    def __init__(self, work):
        self.work = work
        self.basis = bases(work)
        norms = np.linalg.norm(self.basis, axis=1)
        assert (norms > 1e-6).all(), norms
        self.cache = {}

    def magnitudes(self, jobs):
        """jobs: list of (kind, table, bits) -> list of line magnitude tuples."""
        todo = [j for j in jobs if j not in self.cache]
        for i in range(0, len(todo), 4000):
            chunk = todo[i:i + 4000]
            frames = []
            for kind, table, bits in chunk:
                if kind == "pair":
                    frames.append(frame(list(bits) + FILL, table, 1, True))
                else:
                    frames.append(frame(list(bits) + FILL, 0, 0, False))
            rows = decode(frames, self.work)
            coef, *_ = np.linalg.lstsq(self.basis.T, rows.T, rcond=None)
            for (job, c) in zip(chunk, coef.T):
                kind = job[0]
                lines = 2 if kind == "pair" else 4
                mags = []
                for a in c[:lines]:
                    m = abs(a) ** 0.75
                    r = round(m)
                    if abs(m - r) > 0.02 * max(1, r):
                        raise SystemExit(f"unclear magnitude {m} for {job}")
                    mags.append(r)
                self.cache[job] = tuple(mags)
        return [self.cache[j] for j in jobs]


def walk(prober, kind, table, cap):
    """Every code word of a tree: {symbol: bits}."""
    found = {}
    # Each node: (prefix, l) — the word starting prefix + 1s is at least l bits long.
    nodes = [((), 0)]
    while nodes:
        jobs = []
        for prefix, l in nodes:
            word = prefix + (1,) * (l - len(prefix))
            jobs.append((kind, table, word))
            jobs.append((kind, table, word + (0,)))
        res = prober.magnitudes(jobs)
        nxt = []
        for k, (prefix, l) in enumerate(nodes):
            a = tuple(min(v, cap) for v in res[2 * k])
            b = tuple(min(v, cap) for v in res[2 * k + 1])
            if a == b and l >= max(len(prefix), 1):
                word = prefix + (1,) * (l - len(prefix))
                if a in found and found[a] != word:
                    raise SystemExit(f"table {table}: {a} found twice: {found[a]} {word}")
                found[a] = word
                # The branches off this word's run of ones.
                for j in range(len(prefix), l):
                    nxt.append((prefix + (1,) * (j - len(prefix)) + (0,), j + 1))
            else:
                if l >= 24:
                    raise SystemExit(f"table {table}: no word under {prefix}")
                nxt.append((prefix, max(l + 1, len(prefix))))
        nodes = nxt
    return found


def main():
    work = sys.argv[1]
    out = sys.argv[2] if len(sys.argv) > 2 else os.path.join(
        os.path.dirname(__file__), "../../crates/formats/specular-mpa/src/huffman_data.rs")
    os.makedirs(work, exist_ok=True)
    prober = Prober(work)
    lines = [
        "// Generated by tools/corpus/mp3-huffman-probe.py: the Layer III Huffman code words",
        "// found by probing a reference decoder with synthetic frames (no table was copied).",
        "// Each entry is (length, code) for the symbol at index x * size + y (big values) or",
        "// 8v + 4w + 2x + y (count1 table A).",
        "",
    ]
    for t, (size, linbits) in TABLES.items():
        cap = 15 if linbits else 99
        words = walk(prober, "pair", t, cap)
        if len(words) != size * size:
            raise SystemExit(f"table {t}: {len(words)} of {size * size} words")
        kraft = sum(2.0 ** -len(w) for w in words.values())
        assert abs(kraft - 1) < 1e-9, (t, kraft)
        entries = []
        for x in range(size):
            for y in range(size):
                w = words[(x, y)]
                entries.append(f"({len(w)}, {int(''.join(map(str, w)), 2):#x})")
        lines.append(f"pub(crate) const T{t}: [(u8, u32); {size * size}] = [")
        for i in range(0, len(entries), 6):
            lines.append("    " + ", ".join(entries[i:i + 6]) + ",")
        lines.append("];")
        lines.append("")
        print(f"table {t}: {len(words)} words, longest {max(map(len, words.values()))} bits",
              file=sys.stderr)
    words = walk(prober, "quad", 0, 1)
    assert len(words) == 16, len(words)
    entries = []
    for v in range(16):
        q = ((v >> 3) & 1, (v >> 2) & 1, (v >> 1) & 1, v & 1)
        w = words[q]
        entries.append(f"({len(w)}, {int(''.join(map(str, w)), 2):#x})")
    lines.append("pub(crate) const COUNT1_A: [(u8, u32); 16] = [")
    for i in range(0, 16, 6):
        lines.append("    " + ", ".join(entries[i:i + 6]) + ",")
    lines.append("];")
    with open(out, "w") as f:
        f.write("\n".join(lines) + "\n")
    print(f"wrote {out}", file=sys.stderr)


if __name__ == "__main__":
    main()
