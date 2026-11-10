"""Oracle for DICOM video pixel data: pydicom + PyAV (FFmpeg) against Specular.

Usage: dcmobj_oracle.py VIDEO_FRAMES_BIN OUT_TSV DIR...

For every .dcm under DIR... whose transfer syntax is a video one: joins the pixel data
fragments (pydicom), decodes the whole stream with PyAV (raw H.264 / HEVC, or MP4 when the
fragments hold an MP4 file), and runs `video_frames FILE TMPDIR FRAMES...` (Specular, built
from crates/formats/specular-dicom/examples) for frames 0, 1, the middle, the last and a few
in between. Checks:
  - the frame count equals PyAV's (and Number of Frames, when that is not larger);
  - each compared frame equals PyAV's decoded YCbCr converted here to RGB with the same rules
    Specular documents (H.273 matrix of the stream, unspecified = BT.709 above 576 lines else
    BT.601; limited or full range; 4:2:0 chroma 3/4-1/4 between rows, co-sited columns), so a
    difference is a decoding difference. Reported as max |diff| and PSNR; pass = max <= 2.
  - streams PyAV cannot decode or that are not H.264 / HEVC must be refused by Specular.
Writes one row per file to OUT_TSV and prints it.
"""

import json
import os
import subprocess
import sys
import tempfile

import av
import numpy as np
import pydicom
from pydicom.encaps import generate_fragments

H264 = {"1.2.840.10008.1.2.4.102", "1.2.840.10008.1.2.4.102.1", "1.2.840.10008.1.2.4.103",
        "1.2.840.10008.1.2.4.103.1", "1.2.840.10008.1.2.4.104", "1.2.840.10008.1.2.4.104.1",
        "1.2.840.10008.1.2.4.105", "1.2.840.10008.1.2.4.105.1", "1.2.840.10008.1.2.4.106",
        "1.2.840.10008.1.2.4.106.1"}
HEVC = {"1.2.840.10008.1.2.4.107", "1.2.840.10008.1.2.4.108"}
REFUSED = {"1.2.840.10008.1.2.4.100", "1.2.840.10008.1.2.4.100.1", "1.2.840.10008.1.2.4.101",
           "1.2.840.10008.1.2.4.101.1"}

KRKB = {1: (0.2126, 0.0722), 4: (0.30, 0.11), 5: (0.299, 0.114), 6: (0.299, 0.114),
        7: (0.212, 0.087), 9: (0.2627, 0.0593), 10: (0.2627, 0.0593)}


def stream_of(ds):
    frags = [f for f in generate_fragments(ds.PixelData) if len(f)]
    # The first item is the Basic Offset Table (empty or not); generate_fragments skips it.
    return b"".join(frags)


def to_rgb(frame, bits_out):
    """PyAV frame (yuv420p / yuv420p10le) to RGB with Specular's documented rules."""
    fmt = frame.format.name
    deep = fmt.endswith("10le")
    depth = 10 if deep else 8
    planes = []
    for i, p in enumerate(frame.planes):
        w = frame.width if i == 0 else (frame.width + 1) // 2
        h = frame.height if i == 0 else (frame.height + 1) // 2
        dt = np.uint16 if deep else np.uint8
        a = np.frombuffer(bytes(p), dtype=dt).reshape(h, p.line_size // np.dtype(dt).itemsize)[:, :w]
        planes.append(a.astype(np.float32))
    y, cb, cr = planes
    s = float(1 << (depth - 8))
    full = int(frame.color_range) == 2
    if full:
        yo, ys, cs = 0.0, 1.0 / ((1 << depth) - 1), 1.0 / ((1 << depth) - 1)
    else:
        yo, ys, cs = 16.0 * s, 1.0 / (219.0 * s), 1.0 / (224.0 * s)
    co = float(1 << (depth - 1))
    m = int(frame.colorspace)
    kr, kb = KRKB.get(m, (0.2126, 0.0722) if frame.height > 576 else (0.299, 0.114))
    kg = 1 - kr - kb
    H, W = y.shape
    ch, cw = cb.shape

    def up(c):
        rows = np.arange(H)
        cy = rows // 2
        other = np.where(rows % 2 == 0, np.maximum(cy - 1, 0), np.minimum(cy + 1, ch - 1))
        v = 0.75 * c[cy] + 0.25 * c[other]
        cols = np.arange(W)
        j = np.minimum(cols // 2, cw - 1)
        j1 = np.minimum(j + 1, cw - 1)
        return np.where(cols % 2 == 0, v[:, j], 0.5 * (v[:, j] + v[:, j1]))

    L = (y - yo) * ys
    U = (up(cb) - co) * cs
    V = (up(cr) - co) * cs
    r = L + 2 * (1 - kr) * V
    g = L - 2 * kb * (1 - kb) / kg * U - 2 * kr * (1 - kr) / kg * V
    b = L + 2 * (1 - kb) * U
    mx = float((1 << bits_out) - 1)
    out = np.stack([r, g, b], axis=-1)
    out = np.floor(np.clip(out * mx + 0.5, 0, mx))
    if bits_out > 8:
        out = np.floor(out / (1 << (bits_out - 8)))
    return out.astype(np.int32)


def decode_all(stream, ts, tmpdir):
    path = os.path.join(tmpdir, "stream.bin")
    with open(path, "wb") as f:
        f.write(stream)
    if stream[4:8] == b"ftyp":
        fmt = None
    else:
        fmt = "h264" if ts in H264 else "hevc"
    frames = []
    with av.open(path, format=fmt) as c:
        decode_all.codec = c.streams.video[0].codec_context.name
        for fr in c.decode(video=0):
            frames.append(fr)
    return frames


def run(bin_, dcm, ts):
    ds = pydicom.dcmread(dcm)
    declared = int(getattr(ds, "NumberOfFrames", 0) or 0)
    name = os.path.basename(dcm)
    with tempfile.TemporaryDirectory(dir=os.environ.get("TMPDIR")) as tmp:
        if ts not in H264 and ts not in HEVC:
            out = subprocess.run([bin_, dcm], capture_output=True, text=True).stdout
            j = json.loads(out)
            ok = "error" in j
            return [name, ts, str(declared), "-", "-", "refused" if ok else "FAIL: decoded",
                    j.get("error", "")]
        try:
            frames = decode_all(stream_of(ds), ts, tmp)
            if frames and frames[0].format.name not in ("yuv420p", "yuv420p10le", "yuvj420p"):
                raise ValueError("pixel format %s" % frames[0].format.name)
            codec = getattr(decode_all, "codec", "")
            if codec not in ("h264", "hevc"):
                raise ValueError("%s stream" % codec)
        except Exception as e:  # noqa: BLE001 - not H.264 / HEVC, or PyAV refuses: so must we
            out = subprocess.run([bin_, dcm], capture_output=True, text=True).stdout
            j = json.loads(out)
            return [name, ts, str(declared), "-", "-",
                    "refused" if "error" in j else "FAIL: decoded", "%s / pyav: %s" % (j.get("error", ""), e)]
        n = len(frames)
        want_n = min(declared, n) if declared else n
        m = want_n
        pick = sorted({0, min(1, m - 1), m // 3, m // 2, (2 * m) // 3, m - 2 if m > 1 else 0, m - 1})
        out = subprocess.run([bin_, dcm, tmp] + [str(k) for k in pick], capture_output=True,
                             text=True)
        if out.returncode != 0:
            return [name, ts, str(declared), str(n), "-", "FAIL: exit %d" % out.returncode,
                    out.stderr.strip().splitlines()[-1] if out.stderr else ""]
        j = json.loads(out.stdout)
        if "error" in j:
            return [name, ts, str(declared), str(n), "-", "FAIL: refused", j["error"]]
        bits = int(j["bits"])
        worst, psnr_min = 0, float("inf")
        for k in pick:
            ours = np.frombuffer(open(os.path.join(tmp, "%05d.rgb" % k), "rb").read(),
                                 dtype=np.uint8).astype(np.int32)
            ref = to_rgb(frames[k], bits)
            ours = ours.reshape(j["height"], j["width"], 3)
            h, w = min(ref.shape[0], ours.shape[0]), min(ref.shape[1], ours.shape[1])
            d = np.abs(ours[:h, :w] - ref[:h, :w])
            worst = max(worst, int(d.max()))
            mse = float((d.astype(np.float64) ** 2).mean())
            psnr_min = min(psnr_min, 99.0 if mse == 0 else 10 * np.log10(255 ** 2 / mse))
        count_ok = j["frames"] == want_n
        verdict = "ok" if count_ok and worst <= 2 else "FAIL"
        timing = "open %.1f ms, first %.1f ms, cine %.2f ms/frame, seek %.1f ms, back %.1f ms" % (
            j["open_ms"], j["first_ms"], j["cine_ms"], j["seek_ms"], j["back_ms"])
        return [name, ts, str(declared), "%d (ours %d)" % (n, j["frames"]),
                "max %d, PSNR >= %.1f dB over %d frames" % (worst, psnr_min, len(pick)), verdict,
                "%dx%d, %d restarts; %s" % (j["width"], j["height"], j["restarts"], timing)]


def main():
    bin_, out_tsv = sys.argv[1], sys.argv[2]
    files = []
    for d in sys.argv[3:]:
        for root, _, names in os.walk(d):
            files += [os.path.join(root, n) for n in names if n.lower().endswith(".dcm")]
    rows = [["file", "transfer_syntax", "number_of_frames", "pyav_frames", "pixels", "verdict",
             "notes"]]
    for f in sorted(files):
        ds = pydicom.dcmread(f, stop_before_pixels=True)
        ts = str(ds.file_meta.TransferSyntaxUID)
        if "PixelData" not in pydicom.dcmread(f, specific_tags=["PixelData"]):
            continue
        if not (ts in H264 or ts in HEVC or ts in REFUSED):
            continue
        row = run(bin_, f, ts)
        print("\t".join(row), flush=True)
        rows.append(row)
    with open(out_tsv, "w") as o:
        for r in rows:
            o.write("\t".join(r) + "\n")
    fails = [r for r in rows[1:] if r[5].startswith("FAIL")]
    print("%d files, %d failures" % (len(rows) - 1, len(fails)))
    sys.exit(1 if fails else 0)


if __name__ == "__main__":
    main()
