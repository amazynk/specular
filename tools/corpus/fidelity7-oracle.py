#!/usr/bin/env python3
"""Fidelity round 7: video decoders through the app's path against two references.

    fidelity7-oracle.py gen OUT            write the option matrices (ffmpeg encoders) to
                                           OUT/<codec>/gen/
    fidelity7-oracle.py remux SRC DST      a conformance elementary stream into a container
                                           the app opens (ffmpeg -c copy)
    fidelity7-oracle.py check ROOT MANIFEST LIMIT
                                           compare crates/specular-video/tests/fidelity7.rs's
                                           dumps; one ORACLE line per file

Checks per movie (the manifest row and its <n>.planes / <n>.rgb / <n>.frames):

  planes   every frame bit for bit against libavcodec's decode in its native pix_fmt
           (-autorotate 0, passthrough timing, no deinterlacing), and against a second
           decoder where one exists: dav1d and libaom (AV1), libvpx (VP8, VP9); the libvpx
           vectors' and the JVT/JCT-VC suites' own checksums are a third.
  shown    the frame as the viewer shows it (yuv::to_rgb, the CPU twin of the GPU shader)
           against a reference conversion of libavcodec's planes with the matrix, range
           and chroma siting ffprobe reads (bilinear, the app's filter: within 1 code),
           and against zscale's conversion (looser: another filter). A mismatch is tried
           against the other matrices, ranges and sitings and named by the one that fits.
  facts    size after cropping, orientation from the display matrix, pixel aspect, the
           transfer (PQ, HLG, SDR) and frame count against ffprobe's.

Prints `ORACLE<TAB>codec/origin<TAB>file<TAB>pass|fail|skip`, a `#` detail line per
finding, and `#<TAB>unexpected<TAB>N` (failures not in KNOWN).
"""

import hashlib
import json
import os
import subprocess
import sys

import numpy as np

FF = ["ffmpeg", "-hide_banner", "-loglevel", "error", "-nostdin", "-y"]

# --------------------------------------------------------------------------------------
# gen


def run(args):
    subprocess.run(args, check=True)


def src(size, frames, rate=25, pix=None):
    """testsrc2 (saturated bars, gradients and motion) as the encoder's input."""
    a = ["-f", "lavfi", "-i", f"testsrc2=size={size}:rate={rate}", "-frames:v", str(frames)]
    return a + (["-pix_fmt", pix] if pix else [])


def tags(prim, trc, mat, rng=None):
    a = ["-color_primaries", prim, "-color_trc", trc, "-colorspace", mat]
    return a + (["-color_range", rng] if rng else [])


T709 = tags("bt709", "bt709", "bt709", "tv")
T601 = tags("smpte170m", "smpte170m", "smpte170m", "tv")
TPQ = tags("bt2020", "smpte2084", "bt2020nc", "tv")
THLG = tags("bt2020", "arib-std-b67", "bt2020nc", "tv")

X264 = ["-c:v", "libx264", "-preset", "veryfast", "-x264-params", "log-level=error"]
X265 = ["-c:v", "libx265", "-preset", "ultrafast", "-x265-params", "log-level=error"]
AOM = ["-c:v", "libaom-av1", "-cpu-used", "8", "-crf", "30", "-row-mt", "1"]
VP9 = ["-c:v", "libvpx-vp9", "-deadline", "realtime", "-cpu-used", "8", "-b:v", "300k"]
VP8 = ["-c:v", "libvpx", "-deadline", "realtime", "-cpu-used", "8", "-b:v", "300k"]

S = "176x144"


def matrix():
    """(path, ffmpeg args after the input) for every generated file."""
    m = []
    h = "h264/gen/"
    m += [
        (h + "420-709.mp4", src(S, 8) + X264 + ["-pix_fmt", "yuv420p"] + T709),
        (h + "420-601.mp4", src(S, 8) + X264 + ["-pix_fmt", "yuv420p"] + T601),
        (h + "420-709-full.mp4", src(S, 8) + X264 + ["-pix_fmt", "yuv420p"]
         + tags("bt709", "bt709", "bt709", "pc")),
        (h + "420-untagged-sd.mp4", src(S, 6) + X264 + ["-pix_fmt", "yuv420p"]),
        (h + "420-untagged-hd.mkv", src("1280x720", 2) + X264 + ["-pix_fmt", "yuv420p"]),
        (h + "420-240m.mp4", src(S, 6) + X264 + ["-pix_fmt", "yuv420p"]
         + tags("smpte240m", "smpte240m", "smpte240m", "tv")),
        (h + "420-fcc.mp4", src(S, 6) + X264 + ["-pix_fmt", "yuv420p"]
         + tags("bt470m", "gamma22", "fcc", "tv")),
        (h + "420-470bg.mp4", src(S, 6) + X264 + ["-pix_fmt", "yuv420p"]
         + tags("bt470bg", "gamma28", "bt470bg", "tv")),
        (h + "420-10-pq.mp4", src(S, 8) + X264 + ["-pix_fmt", "yuv420p10le"] + TPQ),
        (h + "420-10-hlg.mp4", src(S, 8) + X264 + ["-pix_fmt", "yuv420p10le"] + THLG),
        (h + "422-8.mp4", src(S, 8) + X264 + ["-pix_fmt", "yuv422p"] + T709),
        (h + "422-10.mp4", src(S, 8) + X264 + ["-pix_fmt", "yuv422p10le"] + T709),
        (h + "444-8.mp4", src(S, 8) + X264 + ["-pix_fmt", "yuv444p"] + T709),
        (h + "444-10.mp4", src(S, 8) + X264 + ["-pix_fmt", "yuv444p10le"] + T709),
        (h + "444-lossless.mp4", src(S, 4) + X264 + ["-pix_fmt", "yuv444p", "-qp", "0"] + T709),
        (h + "gray-8.mp4", src(S, 6) + X264 + ["-pix_fmt", "gray"] + T709),
        (h + "gray-10.mp4", src(S, 6) + X264 + ["-pix_fmt", "gray10le"] + T709),
        (h + "rgb.mp4", src(S, 6) + ["-c:v", "libx264rgb", "-preset", "veryfast",
                                     "-pix_fmt", "bgr0"]),
        (h + "odd-99x67.mp4", src("99x67", 8) + X264 + ["-pix_fmt", "yuv420p"] + T709),
        (h + "sar-16-11.mp4", src(S, 6) + X264 + ["-pix_fmt", "yuv420p", "-vf", "setsar=16/11"]
         + T709),
        # The VUI's chroma_sample_loc_type (x264 writes it from its own option only).
        (h + "chroma-topleft.mp4", src(S, 6) + X264[:-1] + ["log-level=error:chromaloc=2",
                                                            "-pix_fmt", "yuv420p"] + T709),
        (h + "chroma-center.mp4", src(S, 6) + X264[:-1] + ["log-level=error:chromaloc=1",
                                                           "-pix_fmt", "yuv420p"] + T709),
        # Bottom-left and bottom (4, 5; round 8: they were shown centred).
        (h + "chroma-bottomleft.mp4", src(S, 6) + X264[:-1] + ["log-level=error:chromaloc=4",
                                                               "-pix_fmt", "yuv420p"] + T709),
        (h + "chroma-bottom.mp4", src(S, 6) + X264[:-1] + ["log-level=error:chromaloc=5",
                                                           "-pix_fmt", "yuv420p"] + T709),
        (h + "mbaff.mp4", src("352x288", 6) + X264 + ["-pix_fmt", "yuv420p", "-flags",
                                                      "+ildct+ilme", "-top", "1"] + T601),
        (h + "baseline.mkv", src(S, 8) + ["-c:v", "libx264", "-profile:v", "baseline",
                                          "-pix_fmt", "yuv420p"] + T709),
    ]
    h = "hevc/gen/"
    m += [
        (h + "420-8.mp4", src(S, 8) + X265 + ["-pix_fmt", "yuv420p"] + T709),
        (h + "420-601.mp4", src(S, 6) + X265 + ["-pix_fmt", "yuv420p"] + T601),
        (h + "420-full.mp4", src(S, 6) + X265 + ["-pix_fmt", "yuv420p"]
         + tags("bt709", "bt709", "bt709", "pc")),
        (h + "420-10-pq.mp4", src(S, 8) + X265 + ["-pix_fmt", "yuv420p10le"] + TPQ),
        (h + "420-10-hlg.mp4", src(S, 8) + X265 + ["-pix_fmt", "yuv420p10le"] + THLG),
        (h + "420-12.mp4", src(S, 6) + X265 + ["-pix_fmt", "yuv420p12le"] + T709),
        (h + "422-10.mp4", src(S, 6) + X265 + ["-pix_fmt", "yuv422p10le"] + T709),
        (h + "444-8.mp4", src(S, 6) + X265 + ["-pix_fmt", "yuv444p"] + T709),
        (h + "444-10.mp4", src(S, 6) + X265 + ["-pix_fmt", "yuv444p10le"] + T709),
        (h + "444-12.mp4", src(S, 6) + X265 + ["-pix_fmt", "yuv444p12le"] + T709),
        (h + "gray-8.mp4", src(S, 6) + X265 + ["-pix_fmt", "gray"] + T709),
        (h + "gray-10.mp4", src(S, 6) + X265 + ["-pix_fmt", "gray10le"] + T709),
        (h + "gbr-8.mp4", src(S, 6) + X265 + ["-pix_fmt", "gbrp"]
         + tags("bt709", "iec61966-2-1", "rgb", "pc")),
        (h + "gbr-10.mp4", src(S, 6) + X265 + ["-pix_fmt", "gbrp10le"]
         + tags("bt709", "iec61966-2-1", "rgb", "pc")),
        (h + "odd-99x67.mp4", src("99x67", 6) + X265 + ["-pix_fmt", "yuv420p"] + T709),
        (h + "sar-4-3.mkv", src(S, 6) + X265 + ["-pix_fmt", "yuv420p", "-vf", "setsar=4/3"]
         + T709),
        (h + "chroma-topleft.mp4", src(S, 6) + X265[:-1] + ["log-level=error:chromaloc=2",
                                                            "-pix_fmt", "yuv420p"] + TPQ),
        (h + "chroma-bottomleft.mp4", src(S, 6) + X265[:-1] + ["log-level=error:chromaloc=4",
                                                               "-pix_fmt", "yuv420p"] + T709),
        (h + "untagged-hd.mp4", src("1280x720", 2) + X265 + ["-pix_fmt", "yuv420p"]),
    ]
    h = "av1/gen/"
    m += [
        (h + "420-8.mkv", src(S, 8) + AOM + ["-pix_fmt", "yuv420p"] + T709),
        (h + "420-601-full.mp4", src(S, 6) + AOM + ["-pix_fmt", "yuv420p"]
         + tags("smpte170m", "smpte170m", "smpte170m", "pc")),
        (h + "420-10-pq.mp4", src(S, 8) + AOM + ["-pix_fmt", "yuv420p10le"] + TPQ),
        (h + "420-10-hlg.ivf", src(S, 6) + AOM + ["-pix_fmt", "yuv420p10le"] + THLG),
        (h + "420-12.mkv", src(S, 4) + AOM + ["-pix_fmt", "yuv420p12le"] + T709),
        (h + "422-10.mkv", src(S, 4) + AOM + ["-pix_fmt", "yuv422p10le"] + T709),
        (h + "444-8.mp4", src(S, 6) + AOM + ["-pix_fmt", "yuv444p"] + T709),
        (h + "444-10.mkv", src(S, 4) + AOM + ["-pix_fmt", "yuv444p10le"] + T709),
        (h + "gray-8.mkv", src(S, 6) + AOM + ["-pix_fmt", "gray"] + T709),
        (h + "gbr-8.mkv", src(S, 6) + AOM + ["-pix_fmt", "gbrp"]
         + tags("bt709", "iec61966-2-1", "rgb", "pc")),
        (h + "odd-99x67.ivf", src("99x67", 6) + AOM + ["-pix_fmt", "yuv420p"] + T709),
        # libaom writes no siting of its own: the sequence header's chroma_sample_position
        # is set to colocated afterwards (round 8; it was 0, unknown, before).
        (h + "colocated.mkv", src(S, 6) + AOM + ["-pix_fmt", "yuv420p",
                                                 "-chroma_sample_location", "topleft",
                                                 "-bsf:v",
                                                 "av1_metadata=chroma_sample_position=colocated"]
         + T709),
        (h + "sar-2-1.mp4", src(S, 6) + AOM + ["-pix_fmt", "yuv420p", "-vf", "setsar=2/1"]
         + T709),
    ]
    h = "vp9/gen/"
    m += [
        (h + "p0-709.webm", src(S, 8) + VP9 + ["-pix_fmt", "yuv420p"] + T709),
        (h + "p0-601-full.webm", src(S, 6) + VP9 + ["-pix_fmt", "yuv420p"]
         + tags("smpte170m", "smpte170m", "smpte170m", "pc")),
        (h + "p0-2020.ivf", src(S, 6) + VP9 + ["-pix_fmt", "yuv420p"]
         + tags("bt2020", "bt709", "bt2020nc", "tv")),
        (h + "p1-444.webm", src(S, 6) + VP9 + ["-pix_fmt", "yuv444p"] + T709),
        (h + "p1-422.webm", src(S, 6) + VP9 + ["-pix_fmt", "yuv422p"] + T709),
        (h + "p1-440.webm", src(S, 6) + VP9 + ["-pix_fmt", "yuv440p"] + T709),
        (h + "p1-gbr.webm", src(S, 6) + VP9 + ["-pix_fmt", "gbrp"]
         + tags("bt709", "iec61966-2-1", "rgb", "pc")),
        (h + "p2-10-pq.webm", src(S, 8) + VP9 + ["-pix_fmt", "yuv420p10le"] + TPQ),
        (h + "p2-12.webm", src(S, 4) + VP9 + ["-pix_fmt", "yuv420p12le"] + T709),
        (h + "p3-444-10.webm", src(S, 6) + VP9 + ["-pix_fmt", "yuv444p10le"] + T709),
        (h + "p3-422-12.webm", src(S, 4) + VP9 + ["-pix_fmt", "yuv422p12le"] + T709),
        (h + "odd-99x67.webm", src("99x67", 6) + VP9 + ["-pix_fmt", "yuv420p"] + T709),
        (h + "aspect-16-9.webm", src(S, 6) + VP9 + ["-pix_fmt", "yuv420p", "-aspect", "16:9"]
         + T709),
    ]
    h = "vp8/gen/"
    m += [
        (h + "420.webm", src(S, 8) + VP8 + ["-pix_fmt", "yuv420p"]),
        (h + "odd-99x67.ivf", src("99x67", 6) + VP8 + ["-pix_fmt", "yuv420p"]),
        (h + "aspect-4-3.webm", src(S, 6) + VP8 + ["-pix_fmt", "yuv420p", "-aspect", "4:3"]),
    ]
    h = "mpeg2/gen/"
    M2 = ["-c:v", "mpeg2video", "-q:v", "4", "-g", "6", "-bf", "2"]
    m += [
        (h + "420-601.mpg", src("352x288", 8) + M2 + ["-pix_fmt", "yuv420p"] + T601),
        (h + "420-709.ts", src("352x288", 8) + M2 + ["-pix_fmt", "yuv420p"] + T709),
        (h + "422.mkv", src("352x288", 6) + M2 + ["-pix_fmt", "yuv422p"] + T709),
        (h + "interlaced.mpg", src("352x288", 6) + M2 + ["-pix_fmt", "yuv420p", "-flags",
                                                         "+ildct+ilme", "-top", "1"] + T601),
        (h + "odd-99x67.m2v", src("99x67", 6) + M2 + ["-pix_fmt", "yuv420p"]),
        (h + "dar-16-9.mpg", src("352x288", 6) + M2 + ["-pix_fmt", "yuv420p", "-aspect",
                                                       "16:9"] + T601),
        (h + "mpeg1.mpg", src("352x288", 6) + ["-c:v", "mpeg1video", "-q:v", "4",
                                               "-pix_fmt", "yuv420p"]),
    ]
    h = "mpeg4/gen/"
    M4 = ["-c:v", "mpeg4", "-q:v", "4", "-g", "6", "-bf", "2"]
    m += [
        (h + "simple.avi", src(S, 8) + M4 + ["-pix_fmt", "yuv420p"]),
        (h + "qpel-gmc.mp4", src(S, 8) + M4 + ["-pix_fmt", "yuv420p", "-flags", "+qpel+mv4",
                                               "-mpeg_quant", "1"]),
        (h + "interlaced.avi", src("352x288", 6) + M4 + ["-pix_fmt", "yuv420p", "-flags",
                                                         "+ildct+ilme"]),
        (h + "odd-99x67.mp4", src("99x67", 6) + M4 + ["-pix_fmt", "yuv420p"]),
        (h + "sar-12-11.avi", src(S, 6) + M4 + ["-pix_fmt", "yuv420p", "-vf", "setsar=12/11"]),
    ]
    h = "prores/gen/"
    PR = ["-c:v", "prores_ks"]
    m += [
        (h + "proxy.mov", src(S, 4) + PR + ["-profile:v", "0", "-pix_fmt", "yuv422p10le"] + T709),
        (h + "lt.mov", src(S, 4) + PR + ["-profile:v", "1", "-pix_fmt", "yuv422p10le"] + T709),
        (h + "standard.mov", src(S, 4) + PR + ["-profile:v", "2", "-pix_fmt", "yuv422p10le"]
         + T601),
        (h + "hq-pq.mov", src(S, 4) + PR + ["-profile:v", "3", "-pix_fmt", "yuv422p10le"] + TPQ),
        (h + "4444.mov", src(S, 4) + PR + ["-profile:v", "4", "-pix_fmt", "yuv444p10le"] + T709),
        (h + "4444-alpha.mov", src(S, 4) + PR + ["-profile:v", "4", "-pix_fmt", "yuva444p10le"]
         + T709),
        (h + "4444xq.mov", src(S, 4) + PR + ["-profile:v", "5", "-pix_fmt", "yuv444p10le"]
         + T709),
        (h + "interlaced.mov", src("352x288", 4) + PR + ["-profile:v", "2", "-flags", "+ildct",
                                                         "-pix_fmt", "yuv422p10le"] + T709),
        (h + "odd-99x67.mov", src("99x67", 4) + PR + ["-profile:v", "2",
                                                      "-pix_fmt", "yuv422p10le"] + T709),
    ]
    h = "mjpeg/gen/"
    MJ = ["-c:v", "mjpeg", "-q:v", "3"]
    m += [
        (h + "420.avi", src(S, 4) + MJ + ["-pix_fmt", "yuvj420p"]),
        (h + "422.mov", src(S, 4) + MJ + ["-pix_fmt", "yuvj422p"]),
        (h + "444.mkv", src(S, 4) + MJ + ["-pix_fmt", "yuvj444p"]),
        (h + "420-limited.avi", src(S, 4) + MJ + ["-pix_fmt", "yuv420p", "-strict", "-1",
                                                  "-color_range", "tv"]),
        (h + "odd-99x67.avi", src("99x67", 4) + MJ + ["-pix_fmt", "yuvj420p"]),
    ]
    h = "dv/gen/"
    m += [
        (h + "ntsc-411.dv", src("720x480", 2, rate="30000/1001", pix="yuv411p")
         + ["-c:v", "dvvideo"]),
        (h + "pal-420.dv", src("720x576", 2, pix="yuv420p") + ["-c:v", "dvvideo"]),
        (h + "dvcpro50-422.dv", src("720x576", 2, pix="yuv422p") + ["-c:v", "dvvideo"]),
        (h + "pal-wide.avi", src("720x576", 2, pix="yuv420p") + ["-c:v", "dvvideo",
                                                                 "-aspect", "16:9"]),
    ]
    h = "theora/gen/"
    TH = ["-c:v", "libtheora", "-q:v", "7"]
    m += [
        (h + "420.ogv", src(S, 8) + TH + ["-pix_fmt", "yuv420p"]),
        (h + "422.ogv", src(S, 6) + TH + ["-pix_fmt", "yuv422p"]),
        (h + "444.ogv", src(S, 6) + TH + ["-pix_fmt", "yuv444p"]),
        (h + "odd-99x67.ogv", src("99x67", 6) + TH + ["-pix_fmt", "yuv420p"]),
    ]
    return m


def gen(out):
    os.makedirs(out, exist_ok=True)
    open(os.path.join(out, ".fidelity7"), "w").close()
    for rel, args in matrix():
        dst = os.path.join(out, rel)
        if os.path.exists(dst) and os.path.getsize(dst) > 0:
            continue
        os.makedirs(os.path.dirname(dst), exist_ok=True)
        tmp = dst + ".part" + os.path.splitext(dst)[1]
        try:
            run(FF + args + ["-an", tmp])
            os.replace(tmp, dst)
        except subprocess.CalledProcessError:
            print(f"gen: {rel} failed", file=sys.stderr)
            if os.path.exists(tmp):
                os.remove(tmp)
    # Rotation lives in the container: remux with a display matrix.
    for rel, base, deg in [
        ("h264/gen/rot90.mp4", "h264/gen/420-709.mp4", 90),
        ("h264/gen/rot270.mov", "h264/gen/420-709.mp4", 270),
        ("hevc/gen/rot180.mp4", "hevc/gen/420-8.mp4", 180),
        ("av1/gen/rot90.mp4", "av1/gen/420-10-pq.mp4", 90),
    ]:
        dst = os.path.join(out, rel)
        b = os.path.join(out, base)
        if not os.path.exists(dst) and os.path.exists(b):
            run(FF + ["-display_rotation", str(deg), "-i", b, "-c", "copy", dst])


def remux(srcf, dst):
    """A conformance elementary stream into MP4 or Matroska (25 fps; FFmpeg only copies).
    Without an edit list: FFmpeg's guessed times would make it hide the leading pictures."""
    os.makedirs(os.path.dirname(dst), exist_ok=True)
    ext = os.path.splitext(srcf)[1].lower()
    fmt = {".264": "h264", ".jsv": "h264", ".26l": "h264", ".avc": "h264", ".jvt": "h264",
           ".bit": "hevc", ".bin": "hevc", ".vc1": "vc1"}.get(ext)
    pre = ["-f", fmt, "-framerate", "25"] if fmt in ("h264", "hevc") else (
        ["-f", fmt] if fmt else [])
    post = ["-use_editlist", "0"] if dst.endswith(".mp4") else []
    gen = [] if dst.endswith(".mp4") else ["-fflags", "+genpts"]
    run(FF + gen + pre + ["-i", srcf, "-map", "0:v:0", "-c", "copy", *post, dst])


STREAM_EXT = (".264", ".h264", ".jsv", ".jvt", ".26l", ".avc", ".bit", ".bin")


def conf(zpath, out, name):
    """One JVT / JCT-VC conformance zip (untrusted data: members are read by base name,
    nothing is executed) into OUT/NAME.mp4 (Matroska cannot hold field pictures without times) plus the reference decoder's result beside it:
    NAME.mp4.md5 (JM's YUV, one MD5 per frame) or NAME.mp4.whole (HM's MD5 of the whole
    YUV; the harness then decodes every frame)."""
    import zipfile

    os.makedirs(out, exist_ok=True)
    dst = os.path.join(out, name + ".mp4")
    with zipfile.ZipFile(zpath) as z:
        members = [m for m in z.infolist() if not m.is_dir()]
        streams = [m for m in members if m.filename.lower().endswith(STREAM_EXT)]
        if len(streams) != 1:
            sys.exit(f"{zpath}: expected one bitstream, got {[m.filename for m in members]}")
        ext = os.path.splitext(streams[0].filename)[1].lower()
        raw = os.path.join(out, name + ".src" + (".bit" if ext == ".bin" else ext))
        with z.open(streams[0]) as s, open(raw, "wb") as d:
            d.write(s.read())
        try:
            if not os.path.exists(dst):
                remux(raw, dst)
        except subprocess.CalledProcessError:
            if os.path.exists(dst):
                os.remove(dst)
            raise
        finally:
            os.remove(raw)
        st = probe(dst)
        w, h, pix = int(st["width"]), int(st["height"]), st["pix_fmt"]
        yuvs = [m for m in members if m.filename.lower().endswith(".yuv")]
        md5s = [m for m in members if m.filename.lower().endswith((".md5", "md5sum.txt"))]
        if yuvs:
            fb = frame_bytes(w, h, pix)
            if PIX[pix][3] == 1:
                # JM writes 4:0:0 as 4:2:0 with neutral chroma.
                fb = fb * 3 // 2
            pick = [m for m in yuvs if m.file_size % fb == 0]
            if not pick:
                sys.exit(f"{zpath}: no YUV of {w}x{h} {pix} frames")
            with z.open(pick[-1]) as f, open(dst + ".md5", "w") as o:
                while True:
                    b = f.read(fb)
                    if len(b) < fb:
                        break
                    o.write(hashlib.md5(b).hexdigest() + "\n")
        else:
            for m in md5s:
                for ln in z.read(m).decode("latin-1").splitlines():
                    p = ln.replace("*", " ").split()
                    if p and len(p[0]) == 32 and (len(p) == 1 or p[1].lower().endswith(".yuv")):
                        with open(dst + ".whole", "w") as o:
                            o.write(p[0] + "\n")


# --------------------------------------------------------------------------------------
# check

# Failures understood and accepted, by path: the reason is printed with the row.
KNOWN = {
    "mpeg4/fate/mpeg4_sstp_dpcm.mkv": "MPEG-4 Studio Profile, refused by name (not built)",
    "vc1/fate/sb-adv-q60.avi": "WMV9 Complex profile, refused by name (not built)",
    "mpeg2/fate/sony-ct3.m2v": "pixel aspect from the sequence display extension as "
    "ISO/IEC 13818-2 6.3.3 says (4:3; libavcodec ignores the extension unless it makes "
    "4:3 or 16:9: 8:9); every picture bit exact since round 8",
    "mpeg2/fate/sony-ct3-ps.mpg": "pixel aspect as sony-ct3.m2v (the same stream as a "
    "programme stream)",
    "vp9/gen/p1-440.webm": "4:4:0 chroma shown with its rows doubled (no 4:4:0 layout in the "
    "player), not interpolated",
    "vp9/libvpx/vp91-2-04-yuv440.webm": "4:4:0 chroma rows doubled (see p1-440)",
}

# (largest difference, share of samples that may differ) for codecs whose inverse DCT is not
# bit exact by their standard.
IDCT_TOLERANCE = {"mjpeg": (1, 1.0), "dvvideo": (2, 0.20)}


def probe(path):
    p = subprocess.run(
        ["ffprobe", "-v", "error", "-select_streams", "v:0", "-show_streams", "-show_entries",
         "stream_side_data", "-of", "json", path], capture_output=True, text=True)
    try:
        st = json.loads(p.stdout)["streams"][0]
    except (ValueError, KeyError, IndexError):
        return None
    # The decoder's view of the first frame where the stream level does not say (ProRes
    # and VP8 carry their colour in the frames).
    f = subprocess.run(
        ["ffprobe", "-v", "error", "-select_streams", "v:0", "-read_intervals", "%+#1",
         "-show_entries", "frame=color_range,color_space,color_primaries,color_transfer,"
         "chroma_location", "-of", "json", path], capture_output=True, text=True)
    try:
        fr = json.loads(f.stdout)["frames"][0]
    except (ValueError, KeyError, IndexError):
        fr = {}
    for k, v in fr.items():
        if st.get(k) in (None, "unknown", "unspecified") and v not in ("unknown", "unspecified"):
            st[k] = v
    return st


def rotation(st):
    """Clockwise degrees the picture is turned to be shown (ffmpeg's autorotate theta)."""
    for sd in st.get("side_data_list", []) or []:
        if "rotation" in sd:
            return int(round(-float(sd["rotation"]))) % 360
    return 0


PIX = {
    # name: (h_div, v_div, bits, planes); planes 1 is grey, "gbr" in G, B, R order
    "yuv420p": (2, 2, 8, 3), "yuvj420p": (2, 2, 8, 3), "yuv422p": (2, 1, 8, 3),
    "yuvj422p": (2, 1, 8, 3), "yuv444p": (1, 1, 8, 3), "yuvj444p": (1, 1, 8, 3),
    "yuv440p": (1, 2, 8, 3), "yuvj440p": (1, 2, 8, 3), "yuv411p": (4, 1, 8, 3),
    "yuv410p": (4, 4, 8, 3), "gbrp": (1, 1, 8, 3), "gray": (1, 1, 8, 1),
}
for b in (9, 10, 12, 14, 16):
    for k in ("yuv420p", "yuv422p", "yuv444p", "yuv440p", "gbrp", "gray"):
        d = PIX[k]
        PIX[f"{k}{b}le"] = (d[0], d[1], b, d[3])
for b in (8, 10, 12, 16):
    s = "" if b == 8 else f"{b}le"
    for k, d in (("yuva420p", (2, 2)), ("yuva422p", (2, 1)), ("yuva444p", (1, 1))):
        PIX[k + s] = (d[0], d[1], b, 4)


def ref_frames(path, pix, limit, extra=()):
    """libavcodec's (or the named decoder's) frames in pix_fmt `pix`, raw bytes."""
    p = subprocess.run(
        # One thread, bit-exact flags and the integer IDCT: what the video decoders' oracles use
        # (libjpeg's IDCT for JPEG, the reference IDCT of MPEG-1/2/4 and DV).
        FF + ["-noautorotate", "-threads", "1", "-flags", "+bitexact", "-idct", "int", *extra,
              "-i", path, "-map", "0:v:0", "-fps_mode",
              "passthrough", "-frames:v", str(limit), "-vf", "setpts=N", "-f", "rawvideo",
              "-pix_fmt", pix, "-"],
        capture_output=True)
    if p.returncode != 0:
        return None
    return p.stdout


def frame_pts(path, limit):
    """libavcodec's first `limit` frames' presentation times, in its output order."""
    p = subprocess.run(
        ["ffprobe", "-v", "error", "-select_streams", "v:0",
         "-show_entries", "frame=pts,best_effort_timestamp", "-of", "csv=p=0", path],
        capture_output=True, text=True)
    out = []
    for ln in p.stdout.split():
        v = [x for x in ln.split(",") if x not in ("", "N/A")]
        if not v:
            return []
        out.append(int(v[0]))
    return out[:limit]


def plane_sizes(w, h, hd, vd, n):
    cw, ch = -(-w // hd), -(-h // vd)
    return [(w, h)] + [(cw, ch)] * (2 if n >= 3 else 0) + ([(w, h)] if n == 4 else [])


def split(buf, w, h, pix):
    """One frame of libavcodec rawvideo as a list of 2-D arrays (16-bit above 8 bits)."""
    hd, vd, bits, n = PIX[pix]
    bps = 2 if bits > 8 else 1
    out, o = [], 0
    for pw, ph in plane_sizes(w, h, hd, vd, n):
        k = pw * ph * bps
        a = np.frombuffer(buf[o:o + k], dtype="<u2" if bps == 2 else np.uint8)
        out.append(a.reshape(ph, pw).astype(np.int32))
        o += k
    return out, o


def frame_bytes(w, h, pix):
    hd, vd, bits, n = PIX[pix]
    bps = 2 if bits > 8 else 1
    return sum(pw * ph * bps for pw, ph in plane_sizes(w, h, hd, vd, n))


def ours(stem, frames):
    """Our frames: [(planes, rgb, facts)]."""
    pl = open(stem + ".planes", "rb").read()
    rgb = np.frombuffer(open(stem + ".rgb", "rb").read(), dtype="<u2")
    out, o, ro = [], 0, 0
    for f in frames:
        w, h = int(f[0]), int(f[1])
        if f[2] != "yuv":
            # Already RGB (ProRes 4444 with alpha): 16-bit RGBA.
            k = w * h * 8
            a = np.frombuffer(pl[o:o + k], dtype="<u2").reshape(h, w, 4).astype(np.int32)
            o += k
            out.append(([a[..., i] for i in range(4)], rgb[ro:ro + w * h * 3].reshape(h, w, 3),
                        f))
            ro += w * h * 3
            continue
        hd, vd, bits = int(f[3]), int(f[4]), int(f[5])
        bps = 2 if bits > 8 else 1
        planes = []
        for pw, ph in plane_sizes(w, h, hd, vd, 3):
            k = pw * ph * bps
            a = np.frombuffer(pl[o:o + k], dtype="<u2" if bps == 2 else np.uint8)
            planes.append(a.reshape(ph, pw).astype(np.int32))
            o += k
        r = rgb[ro:ro + w * h * 3].reshape(h, w, 3) if f[-1] != "-" else None
        ro += w * h * 3 if r is not None else 0
        out.append((planes, r, f))
    return out


KR_KB = {"601": (0.299, 0.114), "709": (0.2126, 0.0722), "2020": (0.2627, 0.0593),
         "240m": (0.212, 0.087), "fcc": (0.30, 0.11)}
OUR_MATRIX = {0: "601", 1: "709", 2: "2020"}
FF_MATRIX = {"bt709": "709", "smpte170m": "601", "bt470bg": "601", "smpte240m": "240m",
             "bt2020nc": "2020", "bt2020c": "2020", "fcc": "fcc", "gbr": "gbr",
             "ycgco": "ycgco"}


def expected(st, codec, h):
    """(matrix, full_range, cosited_x, cosited_y) the stream's tags call for."""
    mat = FF_MATRIX.get(st.get("color_space", ""), None)
    if mat is None:
        mat = "709" if h >= 720 else "601"
    pix = st.get("pix_fmt", "")
    rng = st.get("color_range")
    full = rng == "pc" or (rng in (None, "unknown") and pix.startswith("yuvj"))
    if mat == "gbr" and rng in (None, "unknown"):
        full = True
    loc = st.get("chroma_location", "unspecified")
    # Unspecified siting: centred for MPEG-1, JPEG, VP8 and Theora (their specifications),
    # and for AV1's CSP_UNKNOWN as the app (and its AVIF path, dav1d's reading) takes it;
    # MPEG-2 style (left) for the rest.
    centre_codecs = ("mpeg1video", "mjpeg", "vp8", "theora", "vp3", "av1")
    codec = st.get("codec_name", codec)
    if loc in ("left",):
        cx, cy = True, False
    elif loc in ("center",):
        cx, cy = False, False
    elif loc == "topleft":
        cx, cy = True, True
    elif loc == "top":
        cx, cy = False, True
    elif loc == "bottomleft":
        cx, cy = True, "bottom"
    elif loc == "bottom":
        cx, cy = False, "bottom"
    else:
        cx, cy = codec not in centre_codecs, False
    return mat, full, cx, cy


def upsample(c, hd, vd, w, h, cx, cy):
    """Bilinear chroma at luma positions with the siting (edge samples clamped)."""
    c = c.astype(np.float64)
    ch, cw = c.shape

    def coords(n, div, cos, m):
        x = np.arange(n, dtype=np.float64)
        if div <= 1:
            f = x
        elif cos == "bottom":
            # Chroma row j on luma row (j + 1) * div - 1.
            f = (x + 1) / div - 1
        elif cos:
            f = x / div
        else:
            f = (x + 0.5) / div - 0.5
        f = np.maximum(f, 0.0)
        i0 = np.floor(f).astype(int)
        a = f - i0
        return np.minimum(i0, m - 1), np.minimum(i0 + 1, m - 1), a

    x0, x1, ax = coords(w, hd, cx, cw)
    y0, y1, ay = coords(h, vd, cy, ch)
    top = c[y0][:, x0] + (c[y0][:, x1] - c[y0][:, x0]) * ax
    bot = c[y1][:, x0] + (c[y1][:, x1] - c[y1][:, x0]) * ax
    return top + (bot - top) * ay[:, None]


def to_rgb(planes, bits, hd, vd, mat, full, cx, cy):
    """The app's intended conversion in float64, R'G'B' 0..1."""
    yp = planes[0].astype(np.float64)
    h, w = yp.shape
    unit = float(1 << (bits - 8))
    mx = float((1 << bits) - 1)
    if len(planes) < 3:
        u = v = np.zeros_like(yp)
        ly = (yp / mx) if full else (yp - 16 * unit) / (219 * unit)
        return np.clip(np.stack([ly, ly, ly], -1), 0, 1)
    cb = upsample(planes[1], hd, vd, w, h, cx, cy)
    cr = upsample(planes[2], hd, vd, w, h, cx, cy)
    if mat == "gbr":
        s = (lambda a: a / mx) if full else (lambda a: (a - 16 * unit) / (219 * unit))
        return np.clip(np.stack([s(cr), s(yp), s(cb)], -1), 0, 1)
    if full:
        ly, u, v = yp / mx, (cb - 128 * unit) / mx, (cr - 128 * unit) / mx
    else:
        ly = (yp - 16 * unit) / (219 * unit)
        u, v = (cb - 128 * unit) / (224 * unit), (cr - 128 * unit) / (224 * unit)
    if mat == "ycgco":
        t = ly - u
        return np.clip(np.stack([t + v, ly + u, t - v], -1), 0, 1)
    kr, kb = KR_KB[mat]
    kg = 1 - kr - kb
    r = ly + 2 * (1 - kr) * v
    b = ly + 2 * (1 - kb) * u
    g = (ly - kr * r - kb * b) / kg
    return np.clip(np.stack([r, g, b], -1), 0, 1)


def zscale_rgb(path, st, limit, mat, full, loc):
    """zscale's conversion of libavcodec's frames (an independent filter) as 16-bit RGB."""
    zmat = {"709": "709", "601": "170m", "2020": "2020_ncl", "240m": "240m", "fcc": "fcc",
            "gbr": "gbr", "ycgco": "ycgco"}[mat]
    vf = (f"setpts=N,zscale=matrixin={zmat}:rangein={'full' if full else 'limited'}"
          f":chromalin={loc}:filter=bilinear:dither=none,format=gbrp16le")
    p = subprocess.run(
        FF + ["-noautorotate", "-i", path, "-map", "0:v:0", "-fps_mode", "passthrough",
              "-frames:v", str(limit), "-vf", vf, "-f", "rawvideo", "-"], capture_output=True)
    if p.returncode != 0:
        return None
    g = np.frombuffer(p.stdout, dtype="<u2")
    w, h = int(st["width"]), int(st["height"])
    n = len(g) // (w * h * 3)
    g = g[:n * w * h * 3].reshape(n, 3, h, w)
    # G, B, R planes to interleaved R, G, B.
    return np.stack([g[:, 2], g[:, 0], g[:, 1]], -1).reshape(-1)


def classify_shown(planes, bits, hd, vd, got, want_args):
    """Names the conversion `got` fits when it is not the one the tags call for."""
    mat, full, cx, cy = want_args
    best = None
    for m in ("601", "709", "2020", "240m", "gbr"):
        for f in (False, True):
            for sx in (True, False):
                for sy in (False, True, "bottom"):
                    ref = to_rgb(planes, bits, hd, vd, m, f, sx, sy)
                    e = np.abs(ref * 65535 - got).max() / 257
                    if best is None or e < best[0]:
                        best = (e, m, f, sx, sy)
    e, m, f, sx, sy = best
    why = []
    if m != mat:
        why.append(f"matrix {m} (tags: {mat})")
    if f != full:
        why.append(f"{'full' if f else 'limited'} range (tags: {'full' if full else 'limited'})")
    if (sx, sy) != (cx, cy):
        yname = "bottom" if sy == "bottom" else ("co" if sy else "mid")
        why.append(f"siting x={'co' if sx else 'mid'} y={yname}")
    return f"fits {', '.join(why) or 'the tags'} within {e:.1f}" if why else None


def second_decoders(codec):
    return {"av1": [("dav1d", ["-c:v", "libdav1d"]), ("libaom", ["-c:v", "libaom-av1"])],
            "vp8": [("libvpx", ["-c:v", "libvpx"])],
            "vp9": [("libvpx", ["-c:v", "libvpx-vp9"])]}.get(codec, [])


def md5_list(path):
    """A libvpx vector's .md5 (one digest per frame), when it sits beside the file."""
    for cand in (path + ".md5", os.path.splitext(path)[0] + ".md5"):
        if os.path.exists(cand):
            return [ln.split()[0] for ln in open(cand) if ln.strip()]
    return None


def check(root, manifest, limit):
    rows = []
    for ln in open(manifest):
        f = ln.rstrip("\n").split("\t")
        if len(f) < 2 and rows:
            # A message that ran over a line: part of the row before.
            rows[-1][-1] += " " + f[0]
        elif ln.strip():
            rows.append(f)
    # SPECULAR_FIDELITY7_MATCH=<substring>: only the rows whose path contains it.
    match = os.environ.get("SPECULAR_FIDELITY7_MATCH")
    # jxl/: the still-image cases (specular-decode's jxl_cmyk test).
    rows = [r for r in rows if (not match or match in r[0]) and not r[0].startswith("jxl/")]
    unexpected = 0
    tally = {}
    for row in rows:
        rel = row[0]
        parts = rel.split(os.sep)
        codec = parts[0]
        fmt = f"{codec.upper()}/{parts[1] if len(parts) > 2 else 'misc'}"
        path = os.path.join(root, rel)
        notes, verdict = [], "pass"
        st = probe(path)
        if row[1] != "ok":
            # A refusal: right when libavcodec cannot decode it either, or when it is a
            # variant the app refuses by name (KNOWN says which).
            ff_ok = st is not None and ref_frames(path, "yuv420p", 1) not in (None, b"")
            verdict = "fail" if ff_ok else "skip"
            notes.append(f"{row[1]}: {row[2] if len(row) > 2 else ''}")
        else:
            whole = os.path.exists(path + ".whole")
            verdict, notes = check_one(path, row, st, codec, 100000 if whole else limit)
        known = KNOWN.get(rel)
        if verdict == "fail" and known:
            notes.append(f"known: {known}")
        elif verdict == "fail":
            unexpected += 1
        print(f"ORACLE\t{fmt}\t{rel}\t{verdict}")
        for n in notes:
            print(f"#\t{rel}\t{n}")
        t = tally.setdefault(fmt, [0, 0, 0])
        t[{"pass": 0, "fail": 1, "skip": 2}[verdict]] += 1
        sys.stdout.flush()
    for fmt in sorted(tally):
        p, f, s = tally[fmt]
        print(f"#\ttotal\t{fmt}\t{p}/{f}/{s}")
    print(f"#\tunexpected\t{unexpected}")


def check_one(path, row, st, codec, limit):
    notes = []
    fail = False
    stem, count, declared = row[2], int(row[3]), int(row[4])
    iw, ih, orient = int(row[5]), int(row[6]), int(row[7])
    par = tuple(int(x) for x in row[8].split(":"))
    interlaced = row[10] == "1"
    if row[11] != "-":
        notes.append(f"decode stopped: {row[11]}")
        fail = True
    if st is None:
        return "skip", notes + ["ffprobe cannot read it"]
    frames = [ln.rstrip("\n").split("\t") for ln in open(stem + ".frames") if ln.strip()]
    mine = ours(stem, frames)
    pix = st.get("pix_fmt")
    if pix not in PIX:
        return "skip", notes + [f"pix_fmt {pix} not handled"]
    w, h = int(st["width"]), int(st["height"])
    # Facts.
    if (iw, ih) != (w, h):
        notes.append(f"size {iw}x{ih}, ffprobe {w}x{h}")
        fail = True
    want_rot = rotation(st)
    got_rot = {1: 0, 6: 90, 3: 180, 8: 270}.get(orient, -1)
    if want_rot != got_rot:
        notes.append(f"rotation {got_rot}, display matrix {want_rot}")
        fail = True
    sar = st.get("sample_aspect_ratio", "1:1")
    if sar in ("0:1", "N/A"):
        sar = "1:1"
    sn, sd = (int(x) for x in sar.split(":"))
    if sn * par[1] != sd * par[0]:
        notes.append(f"pixel aspect {par[0]}:{par[1]}, ffprobe {sar}")
        fail = True
    # Planes against libavcodec.
    ref = ref_frames(path, pix, limit + 16)
    if ref is None:
        return "skip", notes + ["libavcodec cannot decode it"]
    fb = frame_bytes(w, h, pix)
    nref = len(ref) // fb if fb else 0
    # The app shows frames in the container's presentation order; libavcodec outputs them in
    # the decoder's. They differ only when the container's times are wrong.
    perm = list(range(nref))
    unordered = False
    pts = frame_pts(path, nref)
    if len(pts) == nref and pts != sorted(pts):
        unordered = True
        # The container's times are not the pictures' order (FFmpeg's remux of a raw
        # conformance stream guesses them), so neither the app's order nor libavcodec's
        # proves anything: each of our frames is matched to libavcodec's by content, and the
        # suites' checksums are taken in libavcodec's (the decoder's) order.
        hd0, vd0, bits0, n0 = PIX[pix]
        keys = {}
        for j in range(nref):
            keys.setdefault(hashlib.md5(ref[j * fb:(j + 1) * fb]).hexdigest(), j)
        found = 0
        for k in range(min(count, nref)):
            planes = mine[k][0]
            if mine[k][2][2] != "yuv":
                continue
            hsh = hashlib.md5()
            for p in planes[:min(n0, 3)]:
                hsh.update(p.astype("<u2" if bits0 > 8 else np.uint8).tobytes())
            j = keys.get(hsh.hexdigest())
            if j is not None:
                perm[k] = j
                found += 1
        ref = b"".join(ref[i * fb:(i + 1) * fb] for i in perm)
        notes.append(f"container times out of decoding order (remuxed stream): {found} of "
                     f"{min(count, nref)} frames matched by content")
    perm = perm[:limit]
    nref = min(nref, limit)
    ref = ref[:nref * fb]
    if nref != count:
        notes.append(f"{count} frames, libavcodec {nref}")
        fail = True
    hd, vd, bits, n = PIX[pix]
    worst, wrong, shown_bad = 0, 0, []
    mat, full, cx, cy = expected(st, codec, h)
    zs = None
    order = None
    idct_off = 0
    plane_fail = False
    for k in range(min(nref, count)):
        rp, _ = split(ref[k * fb:(k + 1) * fb], w, h, pix)
        planes, rgb, f = mine[k]
        if f[2] != "yuv":
            # RGBA from ProRes 4444 with alpha: compare alpha with libavcodec's.
            if n == 4:
                a = rp[3] * (65535 // ((1 << bits) - 1))
                d = int(np.abs(planes[3] - a).max())
                if d > 64:
                    wrong += 1
                    worst = max(worst, d)
            continue
        if int(f[5]) != bits and not (bits == 8 and int(f[5]) == 8):
            notes.append(f"frame {k}: {f[5]} bits, libavcodec {bits}")
            fail = True
            break
        ours_div = (int(f[3]), int(f[4]))
        cmp_planes = rp[:3]
        if n == 1:
            # Grey: the app shows neutral chroma at the decoder's subsampling.
            cmp_planes = rp[:1]
        elif ours_div != (hd, vd):
            # The app widens chroma it has no layout for (4:4:0 rows doubled, 4:1:1 columns
            # doubled): the coded samples are every n-th one of ours.
            sx, sy = hd // ours_div[0], vd // ours_div[1]
            if sx * ours_div[0] != hd or sy * ours_div[1] != vd:
                notes.append(f"frame {k}: chroma {ours_div}, libavcodec {(hd, vd)}")
                fail = True
                break
            if k == 0:
                notes.append(f"chroma widened from {(hd, vd)} to {ours_div}")
            planes = planes[:1] + [c[::sy, ::sx] for c in planes[1:3]]
        d = 0
        for a, b in zip(planes, cmp_planes):
            if a.shape != b.shape:
                d = 1 << 16
                break
            d = max(d, int(np.abs(a - b).max()))
        if n == 1 and len(planes) >= 3:
            mid = 1 << (int(f[5]) - 1)
            d = max(d, int(np.abs(planes[1] - mid).max()), int(np.abs(planes[2] - mid).max()))
        # JPEG and DV leave the inverse DCT's rounding open (libavcodec's is integer, the
        # app's JPEG one libjpeg-turbo's, its DV one the float transform SMPTE 314M gives):
        # the video decoders' tolerances.
        tol, share = IDCT_TOLERANCE.get(st.get("codec_name"), (0, 0.0))
        if 0 < d <= tol:
            off = sum(int(np.count_nonzero(a - b)) for a, b in zip(planes, cmp_planes))
            if off <= share * sum(a.size for a in planes):
                idct_off = max(idct_off, d)
                d = 0
        if d:
            wrong += 1
            worst = max(worst, d)
            if order is None:
                # The same picture elsewhere in libavcodec's sequence: presentation order.
                for j in range(min(nref, count + 4)):
                    rj, _ = split(ref[j * fb:(j + 1) * fb], w, h, pix)
                    if j != k and all(a.shape == b.shape and not np.any(a - b)
                                      for a, b in zip(planes[:n], rj[:n])):
                        order = (k, j)
                        break
        # Shown.
        if rgb is not None and not interlaced:
            # Our own planes: the decode is checked above, this is the conversion.
            refrgb = to_rgb(planes[:3] if n >= 3 else planes[:1], bits, hd, vd, mat, full, cx,
                            cy)
            e = np.abs(refrgb * 65535 - rgb.astype(np.float64)).max() / 257
            if e > 1.0:
                why = None if shown_bad else classify_shown(
                    planes if n >= 3 else planes[:1], bits, hd, vd, rgb.astype(np.float64),
                    (mat, full, cx, cy))
                shown_bad.append((k, e, why))
            if k == 0 and zs is None:
                loc = {(True, False): "left", (False, False): "center",
                       (True, True): "topleft", (False, True): "top",
                       (True, "bottom"): "bottomleft",
                       (False, "bottom"): "bottom"}.get((cx, cy), "left")
                z = zscale_rgb(path, st, 1, mat, full, loc)
                if z is not None and len(z) >= w * h * 3:
                    z = z[:w * h * 3].reshape(h, w, 3).astype(np.float64)
                    dz = np.abs(z - rgb.astype(np.float64)) / 257
                    zs = (float(dz.mean()), float(np.percentile(dz, 99.9)))
    if idct_off:
        notes.append(f"within the inverse DCT tolerance (max {idct_off})")
    if wrong:
        notes.append(f"planes: {wrong} of {min(nref, count)} frames differ, max {worst}")
        if order:
            notes.append(f"order: our frame {order[0]} is libavcodec's {order[1]}")
        # With the container's times out of order the frame sets may differ at the ends
        # (libavcodec drops a field the reference decoder keeps): the suite decides below.
        plane_fail = True
        fail |= not unordered
    if shown_bad:
        k, e, why = shown_bad[0]
        notes.append(f"shown: {len(shown_bad)} frames off the tags' conversion (frame {k} max "
                     f"{e:.1f}/255{'; ' + why if why else ''})")
        fail = True
    if zs is not None:
        notes.append(f"zscale: mean {zs[0]:.2f}, 99.9% {zs[1]:.1f} (/255)")
        # zscale's bilinear chroma is the reference conversion's to 0.01 code; its rounding
        # of saturated colours (clipped differently) moves a few samples by up to 3.
        if zs[0] > 0.5 or zs[1] > 3.0:
            notes.append("zscale disagrees")
            fail = True
    # Transfer the app shows with.
    trc = st.get("color_transfer", "")
    want_t = {"smpte2084": "Pq", "arib-std-b67": "Hlg"}.get(trc, "Srgb")
    if mine and mine[0][2][2] == "yuv" and mine[0][2][9] != want_t:
        notes.append(f"transfer {mine[0][2][9]}, tags {trc}")
        fail = True
    # Second decoders.
    for name, args in second_decoders(codec):
        r2 = ref_frames(path, pix, limit, extra=args)
        if r2 is None:
            notes.append(f"{name}: cannot decode")
            continue
        if r2 == ref:
            notes.append(f"{name}: agrees with libavcodec")
        else:
            n2 = len(r2) // fb
            diff = sum(r2[i * fb:(i + 1) * fb] != ref[i * fb:(i + 1) * fb]
                       for i in range(min(n2, nref)))
            notes.append(f"{name}: {diff} frames differ from libavcodec ({n2} frames)")
            # Ours against it.
            ours_ok = 0
            for k in range(min(n2, count)):
                rp, _ = split(r2[k * fb:(k + 1) * fb], w, h, pix)
                if all(a.shape == b.shape and not np.any(a - b)
                       for a, b in zip(mine[k][0], rp[:3])):
                    ours_ok += 1
            notes.append(f"{name}: ours equals it on {ours_ok} of {min(n2, count)}")
    md5s = md5_list(path)

    def frame_md5(k, h=None, luma_only=False):
        planes = mine[k][0][:1] if luma_only else mine[k][0]
        wide = int(mine[k][2][5]) > 8 if mine[k][2][2] == "yuv" else True
        h = h or hashlib.md5()
        for p in planes[:3]:
            h.update(p.astype("<u2" if wide else np.uint8).tobytes())
        return h

    if md5s:
        # The suite lists frames in output order; with the container's times out of order,
        # membership (which of the suite's frames ours are).
        if unordered:
            listed = set(md5s)
            same = sum(frame_md5(k).hexdigest() in listed for k in range(min(len(md5s), count)))
        else:
            same = sum(k < len(perm) and perm[k] < len(md5s)
                       and frame_md5(k).hexdigest() == md5s[perm[k]]
                       for k in range(min(len(md5s), count)))
        notes.append(f"suite md5: {same} of {min(len(md5s), count)} frames")
        if same < min(len(md5s), count):
            fail = True
        elif plane_fail and unordered:
            notes.append("every frame is one of the reference decoder's: libavcodec's differ")
            plane_fail = False
    if plane_fail and unordered and not os.path.exists(path + ".whole"):
        fail = True
    if os.path.exists(path + ".whole"):
        want = open(path + ".whole").read().split()[0]
        h, hy = hashlib.md5(), hashlib.md5()
        inv = {d: k for k, d in enumerate(perm)}
        for d in range(count):
            k = inv.get(d, d)
            frame_md5(k, h)
            frame_md5(k, hy, luma_only=True)
        # HM writes 4:0:0 as luma only.
        same = h.hexdigest() == want or (n == 1 and hy.hexdigest() == want)
        notes.append(f"suite md5 of all {count} frames: {'equal' if same else 'differs'}")
        fail |= not same or (plane_fail and unordered and not same)
    if declared and nref < limit and declared != nref:
        notes.append(f"declared {declared} frames, libavcodec {nref}")
    return ("fail" if fail else "pass"), notes


def main():
    if len(sys.argv) >= 3 and sys.argv[1] == "gen":
        gen(sys.argv[2])
    elif len(sys.argv) == 4 and sys.argv[1] == "remux":
        remux(sys.argv[2], sys.argv[3])
    elif len(sys.argv) == 5 and sys.argv[1] == "conf":
        conf(sys.argv[2], sys.argv[3], sys.argv[4])
    elif len(sys.argv) == 5 and sys.argv[1] == "check":
        check(sys.argv[2], sys.argv[3], int(sys.argv[4]))
    else:
        print(__doc__, file=sys.stderr)
        sys.exit(2)


if __name__ == "__main__":
    main()
