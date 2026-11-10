#!/usr/bin/env python3
"""MPEG-1 D-pictures (ISO/IEC 11172-2: DC coefficients only, no encoder writes them):
dpict.m1v, three D-pictures of 64x48 testsrc2 frames (8x8 block means as DC levels), and
dpict.framemd5, libmpeg2's decode of it (the oracle: libavcodec refuses D-pictures).

Run from this directory: python3 gen_dpictures.py, then
python3 -I ../../../../../tools/corpus/mpeg1-libmpeg2-oracle.py dpict.m1v > dpict.framemd5
"""
import subprocess

W, H, N = 64, 48, 3
raw = subprocess.run(
    ["ffmpeg", "-nostdin", "-loglevel", "error", "-f", "lavfi", "-i",
     f"testsrc2=size={W}x{H}:rate=2", "-frames:v", str(N), "-pix_fmt", "yuv420p",
     "-f", "rawvideo", "-"],
    check=True, capture_output=True).stdout

# dct_dc_size_luma / dct_dc_size_chrominance codes (Tables B.5a, B.5b), by size.
DC_LUMA = ["100", "00", "01", "101", "110", "1110", "11110", "111110", "1111110"]
DC_CHROMA = ["00", "01", "10", "110", "1110", "11110", "111110", "1111110", "11111110"]


class Bits:
    def __init__(self):
        self.s = ""

    def put(self, v, n):
        self.s += format(v, f"0{n}b") if n else ""

    def code(self, c):
        self.s += c

    def align(self):
        self.s += "0" * (-len(self.s) % 8)

    def start(self, code):
        self.align()
        self.s += "000000000000000000000001" + format(code, "08b")

    def bytes(self):
        self.align()
        return bytes(int(self.s[i:i + 8], 2) for i in range(0, len(self.s), 8))


b = Bits()
b.start(0xB3)
b.put(W, 12)
b.put(H, 12)
b.put(1, 4)  # square pixels
b.put(3, 4)  # 25 Hz
b.put(0x3FFFF, 18)
b.put(1, 1)
b.put(20, 10)
b.put(0, 3)  # constrained_parameters_flag, no quantiser matrices
b.start(0xB8)
b.put(0, 25)
b.put(1, 1)  # closed_gop
b.put(0, 1)
size = W * H * 3 // 2
for f in range(N):
    frame = raw[f * size:(f + 1) * size]
    planes = [frame[:W * H], frame[W * H:W * H * 5 // 4], frame[W * H * 5 // 4:]]
    b.start(0x00)
    b.put(f, 10)
    b.put(4, 3)  # picture_coding_type: D
    b.put(0xFFFF, 16)
    b.put(0, 1)  # extra_bit_picture
    for row in range(H // 16):
        b.start(row + 1)
        b.put(8, 5)  # quantizer_scale (unused by DC)
        b.put(0, 1)
        pred = [128, 128, 128]
        for col in range(W // 16):
            b.code("1")  # macroblock_address_increment 1
            b.code("1")  # macroblock_type: intra
            for k in range(6):
                c, w = (0, W) if k < 4 else (k - 3, W // 2)
                x0 = col * 16 + (k & 1) * 8 if k < 4 else col * 8
                y0 = row * 16 + (k >> 1) * 8 if k < 4 else row * 8
                p = planes[c]
                s = sum(p[(y0 + y) * w + x0 + x] for y in range(8) for x in range(8))
                level = min(255, (s + 32) // 64)
                diff = level - pred[c]
                pred[c] = level
                n = abs(diff).bit_length()
                b.code((DC_LUMA if c == 0 else DC_CHROMA)[n])
                if n:
                    b.put(diff if diff > 0 else diff + (1 << n) - 1, n)
            b.code("1")  # end_of_macroblock
b.start(0xB7)
open("dpict.m1v", "wb").write(b.bytes())
