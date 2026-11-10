#!/usr/bin/env python3
"""Unpack one ITU-T JVT conformance zip (untrusted data) for the H.264 Extended-profile tests.

Usage: python3 -I tools/corpus/h264x-refs.py ZIP OUTDIR NAME

Writes into OUTDIR (never anywhere else):
  NAME.<ext>       the bitstream (the one .264/.h264/.jsv/.jvt/.26l/.avc member)
  NAME.framemd5    one line per frame of the reference decoder's YUV in the zip:
                   "0, n, n, 1, size, md5" (the field the conformance test reads)
and prints one manifest line: name, file, width, height, pix_fmt, field_order, frames.

The picture size comes from ffprobe (oracle tooling only; it reads the SPS). Member names are
reduced to their base name; nothing from the zip is executed.
"""

import hashlib
import json
import os
import subprocess
import sys
import zipfile

STREAM_EXT = (".264", ".h264", ".jsv", ".jvt", ".26l", ".avc", ".26L")
YUV_EXT = (".yuv",)


def sps_size(path):
    """The coded picture size from the first sequence parameter set (Baseline/Extended
    streams: no chroma_format_idc), for when ffprobe can't open the stream."""
    data = open(path, "rb").read()
    i = data.find(b"\x00\x00\x01")
    while i >= 0:
        if data[i + 3] & 0x1F == 7:
            break
        i = data.find(b"\x00\x00\x01", i + 3)
    if i < 0:
        sys.exit(f"{path}: no SPS")
    raw = data[i + 4:i + 64]
    rbsp = bytearray()
    zeros = 0
    for b in raw:
        if zeros >= 2 and b == 3:
            zeros = 0
            continue
        zeros = zeros + 1 if b == 0 else 0
        rbsp.append(b)
    bits = "".join(f"{b:08b}" for b in rbsp)
    pos = 0

    def u(n):
        nonlocal pos
        v = int(bits[pos:pos + n], 2)
        pos += n
        return v

    def ue():
        nonlocal pos
        z = 0
        while bits[pos] == "0":
            z += 1
            pos += 1
        pos += 1
        return (1 << z) - 1 + (u(z) if z else 0)

    profile = u(8)
    u(16)
    ue()
    if profile in (100, 110, 122, 244, 44, 83, 86, 118, 128):
        sys.exit(f"{path}: high-profile SPS")
    ue()
    poc = ue()
    if poc == 0:
        ue()
    elif poc == 1:
        u(1)
        ue()
        ue()
        for _ in range(ue()):
            ue()
    ue()
    u(1)
    w = (ue() + 1) * 16
    h = ue() + 1
    frame_mbs_only = u(1)
    return w, h * 16 * (2 - frame_mbs_only)


def main():
    zpath, out, name = sys.argv[1:4]
    os.makedirs(out, exist_ok=True)
    with zipfile.ZipFile(zpath) as z:
        members = [m for m in z.infolist() if not m.is_dir()]
        streams = [m for m in members if m.filename.endswith(STREAM_EXT)]
        yuvs = [m for m in members if m.filename.lower().endswith(YUV_EXT)]
        if len(streams) != 1 or not yuvs:
            sys.exit(f"{zpath}: expected one bitstream and a YUV, got "
                     f"{[m.filename for m in members]}")
        s = streams[0]
        ext = os.path.splitext(os.path.basename(s.filename))[1]
        stream_path = os.path.join(out, name + ext)
        with z.open(s) as src, open(stream_path, "wb") as dst:
            dst.write(src.read())
        # The decoded reference: prefer a name with "dec" or "rec" in it.
        yuvs.sort(key=lambda m: (not any(k in m.filename.lower() for k in ("dec", "rec")),
                                 m.filename))
        info = json.loads(subprocess.run(
            ["ffprobe", "-v", "error", "-select_streams", "v:0", "-show_entries",
             "stream=width,height", "-of", "json", stream_path],
            capture_output=True, check=True, text=True).stdout)["streams"][0]
        w, h = info.get("width", 0), info.get("height", 0)
        if not w or not h:
            w, h = sps_size(stream_path)
        size = w * h * 3 // 2
        lines = []
        with z.open(yuvs[0]) as y:
            n = 0
            while True:
                f = y.read(size)
                if len(f) < size:
                    break
                lines.append(f"0, {n}, {n}, 1, {size}, {hashlib.md5(f).hexdigest()}")
                n += 1
    with open(os.path.join(out, name + ".framemd5"), "w") as f:
        f.write("#format: frame checksums (reference decoder YUV)\n")
        f.write("\n".join(lines) + "\n")
    print(f"{name}\t{name}{ext}\t{w}\t{h}\tyuv420p\tprogressive\t{len(lines)}")


if __name__ == "__main__":
    main()
