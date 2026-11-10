"""Helpers for tools/corpus/texture/blocks.sh (run with python3 -I).

  blocks.py random OUT BYTES SEED   write BYTES pseudo-random bytes (reproducible per SEED)
  blocks.py level0 FILE OUT         copy level 0, image 0 of an uncompressed KTX1 / KTX2 file
                                    with ETC / EAC / PVRTC data to OUT and print "MODE W H"
                                    (an etc_ref mode, or pvrtc2 / pvrtc4), or "skip"
"""
import random
import struct
import sys

KTX1 = b"\xabKTX 11\xbb\r\n\x1a\n"
KTX2 = b"\xabKTX 20\xbb\r\n\x1a\n"

GL = {
    0x8D64: "rgb", 0x9274: "rgb", 0x9275: "rgb", 0x9276: "rgba1", 0x9277: "rgba1",
    0x9278: "rgba8", 0x9279: "rgba8", 0x9270: "r11", 0x9271: "r11s", 0x9272: "rg11",
    0x9273: "rg11s", 0x8C00: "pvrtc4", 0x8C01: "pvrtc2", 0x8C02: "pvrtc4", 0x8C03: "pvrtc2",
}
VK = {
    147: "rgb", 148: "rgb", 149: "rgba1", 150: "rgba1", 151: "rgba8", 152: "rgba8",
    153: "r11", 154: "r11s", 155: "rg11", 156: "rg11s", 1000054000: "pvrtc2",
    1000054001: "pvrtc4", 1000054004: "pvrtc2", 1000054005: "pvrtc4",
}


def image_bytes(mode, w, h):
    if mode.startswith("pvrtc"):
        bw = 8 if mode == "pvrtc2" else 4
        bx = max(-(-w // bw), 2)
        by = max(-(-h // 4), 2)
        return bx * by * 8
    per = 16 if mode in ("rgba8", "rg11", "rg11s") else 8
    return -(-w // 4) * -(-h // 4) * per


def level0(path, out):
    d = open(path, "rb").read()
    if d.startswith(KTX1):
        if struct.unpack_from("<I", d, 12)[0] != 0x04030201:
            return "skip"
        fmt, w, h = struct.unpack_from("<I", d, 28)[0], *struct.unpack_from("<II", d, 36)
        kvd = struct.unpack_from("<I", d, 60)[0]
        start = 64 + kvd + 4
    elif d.startswith(KTX2):
        if struct.unpack_from("<I", d, 44)[0] != 0:
            return "skip"
        fmt = struct.unpack_from("<I", d, 12)[0]
        w, h = struct.unpack_from("<II", d, 20)
        start = struct.unpack_from("<Q", d, 80)[0]
    else:
        return "skip"
    mode = (GL if d.startswith(KTX1) else VK).get(fmt)
    if mode is None:
        return "skip"
    h = max(h, 1)
    n = image_bytes(mode, w, h)
    data = d[start:start + n]
    if len(data) < n:
        return "skip"
    open(out, "wb").write(data)
    return f"{mode} {w} {h}"


def main():
    if sys.argv[1] == "random":
        rng = random.Random(int(sys.argv[4]))
        open(sys.argv[2], "wb").write(rng.randbytes(int(sys.argv[3])))
    elif sys.argv[1] == "level0":
        print(level0(sys.argv[2], sys.argv[3]))
    else:
        sys.exit("unknown command")


main()
