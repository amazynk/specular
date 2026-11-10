#!/usr/bin/env python3
"""Make closed caption test movies and ffmpeg's reading of them (the oracle for
specular-subs's `cc_oracle` test). Nothing here is committed; ffmpeg (with libx264) needed.

Usage: python3 -I tools/corpus/cc-oracle.py [OUT]
  OUT defaults to $SPECULAR_CC_CORPUS, else $CARGO_TARGET_DIR/corpus/cc.
  Then: SPECULAR_CC_CORPUS=<OUT> cargo test -p specular-subs --test cc_oracle -- --ignored

The captions (CC1: pop-on, roll-up, paint-on, special and extended characters) are made
here from the CEA-608 tables, put into the video as A/53 cc_data (H.264 SEI, MPEG-2 user
data) and, through a Scenarist file, into a MOV c608 track. ffmpeg's own caption decoder
(cc_dec) writes what it shows to <movie>.srt beside each movie.
"""

import os
import subprocess
import sys

FPS_NUM, FPS_DEN = 30000, 1001
FRAMES = 900


def parity(b):
    b &= 0x7F
    return b | 0x80 if bin(b).count("1") % 2 == 0 else b


def ctrl(hi, lo):
    return [(parity(hi), parity(lo))] * 2


def text(s):
    out = []
    data = s.encode("latin-1")
    for i in range(0, len(data), 2):
        a = data[i]
        b = data[i + 1] if i + 1 < len(data) else 0
        out.append((parity(a), parity(b)))
    return out


PAC_HI = [0, 0x11, 0x11, 0x12, 0x12, 0x15, 0x15, 0x16, 0x16, 0x17, 0x17, 0x10, 0x13, 0x13, 0x14, 0x14]


def pac(row, indent=0):
    upper = row in (2, 4, 6, 8, 10, 13, 15)
    lo = 0x40 | (0x20 if upper else 0) | 0x10 | ((indent // 4) << 1)
    return ctrl(PAC_HI[row], lo)


def script():
    """(frame, pairs) runs of CC1 data."""
    runs = []
    # Pop-on, two rows.
    runs.append((30, ctrl(0x14, 0x20) + pac(14, 4) + text("HELLO, WORLD.") + pac(15, 0)
                 + text("SECOND LINE") + ctrl(0x14, 0x2F)))
    runs.append((120, ctrl(0x14, 0x2C)))
    # Pop-on with a special and an extended character.
    runs.append((150, ctrl(0x14, 0x20) + ctrl(0x14, 0x2E) + pac(15, 8) + ctrl(0x11, 0x37)
                 + text(" CAFE") + ctrl(0x12, 0x21) + text(" ") + ctrl(0x11, 0x37) + ctrl(0x14, 0x2F)))
    runs.append((240, ctrl(0x14, 0x2C)))
    # Roll-up, two rows.
    runs.append((270, ctrl(0x14, 0x25) + pac(15, 0) + text("ROLL ONE")))
    runs.append((330, ctrl(0x14, 0x2D) + text("ROLL TWO")))
    runs.append((390, ctrl(0x14, 0x2D) + text("ROLL THREE")))
    runs.append((480, ctrl(0x14, 0x2C)))
    # Paint-on.
    runs.append((510, ctrl(0x14, 0x29) + pac(12, 0) + text("PAINTED")))
    runs.append((600, ctrl(0x14, 0x2C)))
    # Pop-on again, italic row.
    runs.append((630, ctrl(0x14, 0x20) + ctrl(0x14, 0x60 | 0x0E) + text("THE END") + ctrl(0x14, 0x2F)))
    runs.append((780, ctrl(0x14, 0x2C)))
    return runs


def per_frame(runs):
    frames = [None] * FRAMES
    for start, pairs in runs:
        for k, p in enumerate(pairs):
            assert frames[start + k] is None, start + k
            frames[start + k] = p
    return frames


def cc_triplets(pair):
    t = []
    if pair is not None:
        t.append(bytes([0xFC, pair[0], pair[1]]))
    else:
        t.append(bytes([0xFC, 0x80, 0x80]))
    t.append(bytes([0xFD, 0x80, 0x80]))
    return t


def cc_data(pair):
    trip = cc_triplets(pair)
    return bytes([0x40 | len(trip), 0xFF]) + b"".join(trip) + b"\xff"


def escape(rbsp):
    out = bytearray()
    zeros = 0
    for b in rbsp:
        if zeros >= 2 and b <= 3:
            out.append(3)
            zeros = 0
        zeros = zeros + 1 if b == 0 else 0
        out.append(b)
    return bytes(out)


def sei_nal(pair):
    payload = b"\xb5\x00\x31GA94\x03" + cc_data(pair)
    rbsp = bytes([4, len(payload)]) + payload + b"\x80"
    return b"\x00\x00\x00\x01\x06" + escape(rbsp)


def split_nals(es):
    """Start offsets of NAL units (after their start codes) in Annex B bytes."""
    pos = []
    i = 0
    n = len(es)
    while i + 3 <= n:
        if es[i] == 0 and es[i + 1] == 0 and es[i + 2] == 1:
            pos.append(i + 3)
            i += 3
        else:
            i += 1
    return pos


def insert_sei(es, frames):
    out = bytearray()
    starts = split_nals(es)
    k = 0
    for j, s in enumerate(starts):
        end = starts[j + 1] - 3 if j + 1 < len(starts) else len(es)
        # Keep a leading zero of a 4-byte start code with the next unit.
        while end > s and es[end - 1] == 0 and j + 1 < len(starts):
            end -= 1
        nal = es[s:end]
        out += b"\x00\x00\x00\x01" + nal
        if nal and nal[0] & 0x1F == 9:
            out += sei_nal(frames[k] if k < len(frames) else None)
            k += 1
    return bytes(out), k


def insert_user_data(es, frames):
    out = bytearray()
    starts = split_nals(es)
    k = 0
    pending = False
    for j, s in enumerate(starts):
        end = starts[j + 1] - 3 if j + 1 < len(starts) else len(es)
        unit = es[s:end]
        code = unit[0] if unit else 0
        if pending and 0x01 <= code <= 0xAF:
            out += b"\x00\x00\x01\xb2GA94\x03" + cc_data(frames[k] if k < len(frames) else None)
            k += 1
            pending = False
        if code == 0x00:
            pending = True
        out += b"\x00\x00\x01" + unit
    return bytes(out), k


def scc(runs):
    lines = ["Scenarist_SCC V1.0", ""]
    for start, pairs in runs:
        # Non-drop-frame timecode: frame counts at 30 a second.
        f = start
        tc = f"{f // 108000:02d}:{f // 1800 % 60:02d}:{f // 30 % 60:02d}:{f % 30:02d}"
        words = " ".join(f"{a:02x}{b:02x}" for a, b in pairs)
        lines += [f"{tc}\t{words}", ""]
    return "\n".join(lines) + "\n"


def run(cmd):
    subprocess.run(cmd, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def oracle(movie, srt, direct=False):
    if direct:
        cmd = ["ffmpeg", "-v", "error", "-y", "-i", movie, "-map", "0:s:0", "-c:s", "srt", srt]
    else:
        cmd = ["ffmpeg", "-v", "error", "-y", "-f", "lavfi", "-i",
               f"movie={movie}[out0+subcc]", "-map", "0:1", "-c:s", "srt", srt]
    run(cmd)


def main():
    out = sys.argv[1] if len(sys.argv) > 1 else os.environ.get("SPECULAR_CC_CORPUS") or os.path.join(
        os.environ.get("CARGO_TARGET_DIR", "target"), "corpus", "cc")
    os.makedirs(out, exist_ok=True)
    p = lambda n: os.path.join(out, n)
    runs = script()
    frames = per_frame(runs)
    with open(p("captions.scc"), "w") as f:
        f.write(scc(runs))
    rate = f"{FPS_NUM}/{FPS_DEN}"
    dur = f"{FRAMES * FPS_DEN / FPS_NUM:.3f}"
    src = ["ffmpeg", "-v", "error", "-y", "-f", "lavfi", "-i", f"testsrc=size=160x120:rate={rate}:duration={dur}"]
    run(src + ["-c:v", "libx264", "-bf", "0", "-g", "30", "-x264-params", "aud=1", "-pix_fmt", "yuv420p",
               "-f", "h264", p("base.h264")])
    run(src + ["-c:v", "mpeg2video", "-bf", "0", "-g", "15", "-f", "mpeg2video", p("base.m2v")])
    es, n = insert_sei(open(p("base.h264"), "rb").read(), frames)
    assert n == FRAMES, n
    open(p("cc.h264"), "wb").write(es)
    es, n = insert_user_data(open(p("base.m2v"), "rb").read(), frames)
    assert n == FRAMES, n
    open(p("cc.m2v"), "wb").write(es)
    movies = []
    for ext in ["ts", "mkv", "mp4"]:
        name = f"cc_h264.{ext}"
        run(["ffmpeg", "-v", "error", "-y", "-r", rate, "-i", p("cc.h264"), "-c", "copy", p(name)])
        movies.append(name)
    for ext, fmt in [("ts", "mpegts"), ("mpg", "vob")]:
        name = f"cc_mpeg2.{ext}"
        run(["ffmpeg", "-v", "error", "-y", "-fflags", "+genpts", "-r", rate, "-i", p("cc.m2v"), "-c", "copy", "-f", fmt, p(name)])
        movies.append(name)
    # B-frames: ffmpeg decodes the captioned movie and its encoders write each picture's
    # A/53 captions (frame side data, presentation order) into that picture's own SEI or
    # user data, so they come in decode order in the stream.
    for codec, ext, extra in [
        ("libx264", "ts", ["-x264-params", "aud=1:b-adapt=0:bframes=3:b-pyramid=normal", "-pix_fmt", "yuv420p"]),
        ("libx264", "mkv", ["-x264-params", "aud=1:b-adapt=0:bframes=3:b-pyramid=normal", "-pix_fmt", "yuv420p"]),
        ("libx264", "mp4", ["-x264-params", "aud=1:b-adapt=0:bframes=3:b-pyramid=normal", "-pix_fmt", "yuv420p"]),
        ("mpeg2video", "ts", ["-bf", "2", "-a53cc", "1"]),
    ]:
        kind = "h264" if codec == "libx264" else "mpeg2"
        name = f"cc_{kind}_bf.{ext}"
        run(["ffmpeg", "-v", "error", "-y", "-i", p(f"cc_{kind}.ts"), "-c:v", codec, "-g", "30"]
            + extra + [p(name)])
        movies.append(name)
    for name in movies:
        oracle(p(name), p(name + ".srt"))
    # The source's pop-on captions: (first frame shown, first frame gone) at 30000/1001,
    # for checking cue order and timing without ffmpeg.
    with open(p("popon.txt"), "w") as f:
        shown = None
        for start, pairs in runs:
            for k, pair in enumerate(pairs):
                code = ((pair[0] & 0x7F) << 8) | (pair[1] & 0x7F)
                at = start + k
                if code in (0x142F, 0x142C) and (k == 0 or pairs[k - 1] != pair):
                    if shown is not None:
                        f.write(f"{shown} {at}\n")
                        shown = None
                    if code == 0x142F:
                        shown = at
    # A MOV c608 track from the Scenarist file.
    run(["ffmpeg", "-v", "error", "-y", "-i", p("base.h264"), "-i", p("captions.scc"), "-map", "0:v",
         "-map", "1:s", "-c", "copy", "-f", "mov", p("cc608.mov")])
    oracle(p("cc608.mov"), p("cc608.mov.srt"), direct=True)
    for f in ["base.h264", "base.m2v", "cc.h264", "cc.m2v"]:
        os.remove(p(f))
    print(out)


if __name__ == "__main__":
    main()
