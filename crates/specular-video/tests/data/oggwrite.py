"""Own minimal Ogg writer (RFC 3533) for fixtures FFmpeg cannot mux:

  ogm  OUT FOURCC FPS W H DIR   OGM video: the 0x01 stream header (type "video", subtype FOURCC,
                                100 ns per frame), a 0x03 comment packet, then each packet of
                                DIR (FFmpeg image2 copies, one frame each) behind a data byte
                                (keyframe flag 0x08 on the first) and one length byte.
  ogm-audio OUT TAG RATE CH BITS FILE
                                OGM sound: the 0x01 stream header (type "audio", the WAVE format
                                TAG in hex), a 0x03 comment packet, then FILE's packets behind a
                                data byte: an MP3 file split at its frames, each with two length
                                bytes (1152 samples); anything else as raw PCM in 4096-byte
                                packets without length bytes (timed by their size).
  dirac OUT DIR                 Dirac/VC-2: the first packet's leading data unit (the sequence
                                header) alone, then each packet of DIR whole.

Packets go one per page; granule positions count frames (sound: samples; Dirac's are only
ordered).

    python3 -I oggwrite.py ogm out.ogm XVID 25 64 48 frames.d
"""

import os
import struct
import sys


def crc32_ogg(data):
    crc = 0
    for b in data:
        crc ^= b << 24
        for _ in range(8):
            crc = ((crc << 1) ^ 0x04C11DB7) & 0xFFFFFFFF if crc & 0x80000000 else (crc << 1) & 0xFFFFFFFF
    return crc


def page(serial, seq, granule, flags, packet):
    lacing = []
    n = len(packet)
    while n >= 255:
        lacing.append(255)
        n -= 255
    lacing.append(n)
    assert len(lacing) <= 255
    head = b"OggS" + bytes([0, flags]) + struct.pack("<qII", granule, serial, seq) + b"\0\0\0\0"
    head += bytes([len(lacing)]) + bytes(lacing)
    p = bytearray(head + packet)
    struct.pack_into("<I", p, 22, crc32_ogg(p))
    return bytes(p)


def write(out, packets, granules):
    serial = 0x5EC7
    data = b""
    for i, (pk, g) in enumerate(zip(packets, granules)):
        flags = 2 if i == 0 else (4 if i == len(packets) - 1 else 0)
        data += page(serial, i, g, flags, pk)
    open(out, "wb").write(data)


def frames(folder):
    return [open(os.path.join(folder, n), "rb").read() for n in sorted(os.listdir(folder))]


mode = sys.argv[1]
if mode == "ogm":
    out, fourcc, fps, w, h, folder = sys.argv[2:8]
    unit = 10_000_000 // int(fps)
    head = b"\x01video\0\0\0" + fourcc.encode()
    head += struct.pack("<iqqiihhii", 0x38, unit, 1, 1, 1 << 20, 24, 0, int(w), int(h))
    comment = b"\x03" + b"vorbis" + struct.pack("<I", 4) + b"test" + struct.pack("<I", 0) + b"\x01"
    data = []
    for i, f in enumerate(frames(folder)):
        # Data byte: length-of-duration bytes (one, in bit 6), keyframe flag on the first.
        flag = 0x40 | (0x08 if i == 0 else 0)
        data.append(bytes([flag, 1]) + f)
    write(out, [head, comment] + data, [0, 0] + list(range(1, len(data) + 1)))
elif mode == "ogm-audio":
    out, tag, rate, ch, bits, path = sys.argv[2:8]
    rate, ch, bits = int(rate), int(ch), int(bits)
    raw = open(path, "rb").read()
    head = b"\x01audio\0\0\0" + tag.encode().ljust(4, b"\0")
    head += struct.pack("<iqqiihh", 0x38, 10_000_000, rate, 1, 1 << 16, bits, 0)
    head += struct.pack("<hhi", ch, ch * max(bits, 8) // 8, rate * ch * max(bits, 8) // 8)
    head += b"\0" * 22  # 75 bytes, as DirectShow OGM writers make it (FFmpeg reads no less)
    comment = b"\x03" + b"vorbis" + struct.pack("<I", 4) + b"test" + struct.pack("<I", 0) + b"\x01"
    data, granules, at = [], [], 0
    if tag == "55":
        # MPEG-1 Layer III frames: 144 * bitrate / rate bytes plus the padding bit.
        rates = [0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320]
        srs = [44100, 48000, 32000]
        i = 0
        while i + 4 <= len(raw):
            h = raw[i:i + 4]
            assert h[0] == 0xFF and h[1] & 0xFE == 0xFA, i
            n = 144000 * rates[h[2] >> 4] // srs[(h[2] >> 2) & 3] + ((h[2] >> 1) & 1)
            data.append(bytes([0x80, 0x80, 0x04]) + raw[i:i + n])
            at += 1152
            granules.append(at)
            i += n
    else:
        step = ch * bits // 8
        for i in range(0, len(raw), 4096):
            data.append(b"\0" + raw[i:i + 4096])
            at += len(raw[i:i + 4096]) // step
            granules.append(at)
    write(out, [head, comment] + data, [0, 0] + granules)
elif mode == "dirac":
    out, folder = sys.argv[2:4]
    fs = frames(folder)
    first = fs[0]
    assert first[:4] == b"BBCD"
    nxt = struct.unpack(">I", first[5:9])[0]
    seq = first[:nxt]
    write(out, [seq] + fs, [0] + [(i + 1) << 22 for i in range(len(fs))])
else:
    sys.exit("mode: ogm, ogm-audio or dirac")
