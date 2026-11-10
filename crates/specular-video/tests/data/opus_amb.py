"""Opus ambisonics fixtures for gen_fixtures_sound_gaps.sh.

  python3 -I opus_amb.py render IN.f32 CHANNELS OUT.s32 [MATRIX]
      libopus's decode (f32, CHANNELS interleaved) as the player plays it: ambisonics to
      stereo by two cardioids facing left and right, ½(W ± Y), plus a non-diegetic pair;
      MATRIX "family3" first mixes the channels through the matrix below. CHANNELS 255x
      (e.g. 2553): no layout, the first two channels.
  python3 -I opus_amb.py family3 IN.ogg OUT.ogg
      Rewrites a 4-channel family 2 Ogg Opus file (4 mono streams, mapping 0 1 2 3) as
      family 3 with the matrix below.
"""

import struct
import sys

# Output channel c from decoded channel j: MATRIX[j][c] (column j of the demixing matrix),
# Q15. A rotation of the horizontal components with some leakage.
MATRIX = [
    [32767, 0, 1000, 0],
    [0, 23170, 0, -23170],
    [-1000, 0, 30000, 0],
    [0, 23170, 0, 23170],
]


# ffmpeg's libopus wrapper puts 3 to 8 channels in its own order whatever the family; where
# each of the stream's channels lands.
FFMPEG_ORDER = {
    3: [0, 2, 1],
    5: [0, 2, 1, 3, 4],
    6: [0, 2, 1, 4, 5, 3],
    7: [0, 2, 1, 5, 6, 4, 3],
    8: [0, 2, 1, 6, 7, 4, 5, 3],
}


def render(src, channels, dst, family3):
    raw = open(src, "rb").read()
    n = len(raw) // 4
    v = struct.unpack(f"<{n}f", raw[: n * 4])
    ch = channels - 2550 if channels >= 2550 else channels
    pos = FFMPEG_ORDER.get(ch)
    if pos:
        v = [v[i - i % ch + pos[i % ch]] for i in range(n - n % ch)]
        n = len(v)
    out = []
    if channels >= 2550:
        ch = channels - 2550
        for i in range(0, n - ch + 1, ch):
            out += [v[i], v[i + 1]]
    else:
        ch = channels
        comps = max(k * k for k in range(1, 16) if k * k <= ch)
        for i in range(0, n - ch + 1, ch):
            f = list(v[i : i + ch])
            if family3:
                f = [sum(MATRIX[j][c] / 32768.0 * f[j] for j in range(4)) for c in range(4)]
            w, y = f[0], f[1] if comps >= 4 else 0.0
            l, r = 0.5 * (w + y), 0.5 * (w - y)
            if ch - comps == 2:
                l, r = l + f[comps], r + f[comps + 1]
            out += [l, r]
    s = [max(-(2**31), min(2**31 - 1, round(x * 2**31))) for x in out]
    open(dst, "wb").write(struct.pack(f"<{len(s)}i", *s))


def crc32_ogg(data):
    crc = 0
    for b in data:
        crc ^= b << 24
        for _ in range(8):
            crc = ((crc << 1) ^ 0x04C11DB7) & 0xFFFFFFFF if crc & 0x80000000 else (crc << 1) & 0xFFFFFFFF
    return crc


def family3(src, dst):
    d = open(src, "rb").read()
    assert d[:4] == b"OggS"
    nseg = d[26]
    seg = d[27 : 27 + nseg]
    body = d[27 + nseg : 27 + nseg + sum(seg)]
    rest = d[27 + nseg + sum(seg) :]
    assert body[:8] == b"OpusHead" and body[18] == 2 and body[9] == 4
    head = bytearray(body[:19]) + bytes([4, 0])
    head[18] = 3
    for col in MATRIX:
        for x in col:
            head += struct.pack("<h", x)
    page = bytearray(d[:26]) + bytes([1, len(head)]) + head
    page[22:26] = b"\0\0\0\0"
    page[22:26] = struct.pack("<I", crc32_ogg(page))
    open(dst, "wb").write(bytes(page) + rest)


if sys.argv[1] == "render":
    render(sys.argv[2], int(sys.argv[3]), sys.argv[4], len(sys.argv) > 5)
else:
    family3(sys.argv[2], sys.argv[3])
