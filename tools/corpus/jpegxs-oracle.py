#!/usr/bin/env python3
"""Synthetic JPEG XS corpus and reference decodings from the ISO/IEC 21122-5 reference
software (jxs_encoder / jxs_decoder, built by fetch-jpegxs.sh).

    jpegxs-oracle.py synth BIN_DIR OUT_DIR [--small] [--4k] [--keep-ref]
    jpegxs-oracle.py decode BIN_DIR DIR [--keep-ref]
    jpegxs-oracle.py sequences BIN_DIR DIR [--keep-ref]

`synth` writes synthetic test images and encodes each with one configuration of the matrix
below into OUT_DIR/NAME.jxs. `--small` makes 64×32 images (the committed fixtures); `--4k`
adds a 4096×2160 4:2:2 10-bit frame for timing. `decode` takes every .jxc/.jxs codestream or
JPEG XS file in DIR (the conformance codestreams). `sequences` takes each sequence
PREFIX_frame_NNNNNNNN.jxc in DIR in order (the reference decoder's sequence mode, which keeps
the frame buffer of temporal prediction).

Each writes DIR/manifest.tsv: file, outcome ("ok", or "refused" when the reference decoder
declines it), sample count and SHA-256 of the reference decoder's samples (16-bit
little-endian planes, its .yuv16p output), and a note (the encoder configuration). With
`--keep-ref` the samples stay beside the codestream as NAME.ref, for finding where a
mismatch is.

Standard library only; work files go to a temporary directory inside the output directory.
"""

import hashlib
import os
import re
import shutil
import struct
import subprocess
import sys

# name, sampling ("1", "444", "422", "420", "4444", "bayer:RGGB"), depth, configuration,
# expected outcome.
MATRIX = [
    ("main422-10", "422", 10, "p=Main422.10;rate=3", "ok"),
    ("main422-10-6bpp", "422", 10, "p=Main422.10;rate=6", "ok"),
    ("main422-8", "422", 8, "p=Main422.10;rate=2", "ok"),
    ("light422-10", "422", 10, "p=Light422.10;rate=2", "ok"),
    ("light-subline422-10", "422", 10, "p=Light-Subline422.10;rate=3", "ok"),
    ("main420-12", "420", 12, "p=Main420.12;rate=3", "ok"),
    ("high420-12", "420", 12, "p=High420.12;rate=4", "ok"),
    ("main444-12-rct", "444", 12, "p=Main444.12;rate=4", "ok"),
    ("main444-8-rct", "444", 8, "p=Main444.12;rate=3", "ok"),
    ("high444-12-none", "444", 12, "p=High444.12;rate=5;mct=none", "ok"),
    ("light444-12", "444", 12, "p=Light444.12;rate=4", "ok"),
    ("main4444-12", "4444", 12, "p=Main4444.12;rate=6", "ok"),
    ("high4444-12", "4444", 12, "p=High4444.12;rate=8", "ok"),
    ("chigh444-12", "444", 12, "p=CHigh444.12;rate=4", "ok"),
    ("mls12", "444", 12, "p=MLS.12", "ok"),
    ("mls12-422", "422", 10, "p=MLS.12", "ok"),
    # The reference encoder cannot write MLS.16 (its rate control counts 4-bit raw bit-plane
    # counts where packing writes 5); 16-bit lossless goes through MLS.12 with Bw = 16.
    ("mls12-bw16", "444", 16, "p=MLS.12;bw=16", "ok"),
    ("u-grey16-fq0", "1", 16, "p=unrestricted;bw=16;fq=0;rate=100", "ok"),
    ("lightbayer-rggb", "bayer:RGGB", 12, "p=LightBayer;cfa=RGGB;rate=3", "ok"),
    ("mainbayer-grbg", "bayer:GRBG", 12, "p=MainBayer;cfa=GRBG;rate=3", "ok"),
    ("highbayer-bggr", "bayer:BGGR", 14, "p=HighBayer;cfa=BGGR;rate=4", "ok"),
    ("highbayer-gbrg", "bayer:GBRG", 16, "p=HighBayer;cfa=GBGR;rate=5", "ok"),
    ("bayer-tetrix-e", "bayer:RGGB", 12, "p=HighBayer;cfa=RGGB;rate=4;mct=tetrix,0,1,2", "ok"),
    ("grey-8", "1", 8, "p=unrestricted;rate=2", "ok"),
    ("grey-12-nly0", "1", 12, "p=unrestricted;rate=3;nlx=3;nly=0", "ok"),
    ("u444-nlx1", "444", 10, "p=unrestricted;rate=4;nlx=1;nly=1", "ok"),
    ("u444-nlx2-nly2", "444", 10, "p=unrestricted;rate=4;nlx=2;nly=2", "ok"),
    ("u422-nly2", "422", 10, "p=unrestricted;rate=3;nlx=4;nly=2", "ok"),
    ("u420-nly2", "420", 10, "p=unrestricted;rate=3;nlx=5;nly=2", "ok"),
    ("u444-uniform", "444", 10, "p=unrestricted;rate=3;qpih=1", "ok"),
    ("u444-signs", "444", 10, "p=unrestricted;rate=3;fs=1", "ok"),
    ("u444-rm1", "444", 10, "p=unrestricted;rate=2;rm=1", "ok"),
    ("u444-rm0", "444", 10, "p=unrestricted;rate=2;rm=0", "ok"),
    ("u444-raw", "444", 10, "p=unrestricted;rate=2;rl=1", "ok"),
    ("u444-long", "444", 10, "p=unrestricted;rate=3;lh=1", "ok"),
    ("u444-slh8", "444", 10, "p=unrestricted;rate=3;slh=8", "ok"),
    ("u444-visual", "444", 10, "p=unrestricted;rate=3;gains=visual", "ok"),
    ("u444-fq6", "444", 10, "p=unrestricted;rate=3;fq=6", "ok"),
    ("u444-bw18", "444", 10, "p=unrestricted;rate=3;bw=18", "ok"),
    ("u4444-sd1", "4444", 10, "p=unrestricted;rate=4;sd=1", "ok"),
    ("u444-quadratic", "444", 12, "p=unrestricted;rate=4;nlt=quadratic,0,64", "ok"),
    ("u444-quadratic-neg", "444", 12, "p=unrestricted;rate=4;nlt=quadratic,1,32700", "ok"),
    ("u444-extended", "444", 12, "p=unrestricted;rate=4;nlt=extended,512,3072", "ok"),
    ("tdc444-12", "444", 12, "p=TDC444.12;rate=4", "ok"),
    # The encoder leaves Bw unresolved (255) for TDC MLS unless it is given.
    ("tdcmls444-12", "444", 12, "p=TDCMLS444.12;bw=12", "ok"),
]

# Precinct columns: the reference encoder wants a last, narrower column.
COLUMNS = [
    ("u422-cw1", "422", 10, "p=unrestricted;rate=8;cw=1;nlx=2;nly=1", "ok"),
    ("u444-cw2", "444", 10, "p=unrestricted;rate=8;cw=2;nlx=2;nly=1", "ok"),
    ("u444-cw1-nly2", "444", 10, "p=unrestricted;rate=10;cw=1;nlx=3;nly=2", "ok"),
]

ODD = [
    ("odd-444", "444", 10, "p=unrestricted;rate=8", "ok", 75, 37),
    ("odd-grey", "1", 12, "p=unrestricted;rate=8;nlx=4;nly=1", "ok", 61, 19),
    ("odd-422", "422", 10, "p=unrestricted;rate=8", "ok", 90, 45),
]


class Rng:
    def __init__(self, seed):
        self.s = seed & 0xFFFFFFFFFFFFFFFF

    def next(self):
        self.s = (self.s * 6364136223846793005 + 1442695040888963407) & 0xFFFFFFFFFFFFFFFF
        return self.s >> 33


def plane(w, h, depth, k, seed):
    """A synthetic component: gradients, a ring pattern, hard-edged blocks and noise."""
    import math
    rng = Rng(seed * 7919 + k * 104729)
    m = (1 << depth) - 1
    out = []
    for y in range(h):
        for x in range(w):
            v = (x * (k + 1) * 3 + y * (5 - k)) / (w + h) * 0.6
            r = math.hypot(x - w * 0.6, y - h * 0.4)
            v += 0.18 * math.sin(r * (0.3 + 0.1 * k))
            if ((x // 9) + (y // 7) + k) % 5 == 0:
                v += 0.25
            v += (rng.next() % 1000) / 1000.0 * 0.08
            out.append(max(0, min(m, int((v % 1.0) * m))))
    return out


def comps_of(samp, w, h):
    if samp == "1":
        return [(w, h)]
    if samp == "444":
        return [(w, h)] * 3
    if samp == "4444":
        return [(w, h)] * 4
    if samp == "422":
        return [(w, h), (w // 2, h), (w // 2, h)]
    if samp == "420":
        return [(w, h), (w // 2, h // 2), (w // 2, h // 2)]
    if samp.startswith("bayer:"):
        return [(w, h)]
    raise ValueError(samp)


def explicit_gains(n):
    """Gains falling from the low bands to the high ones, priorities all distinct."""
    gains = [max(0, 3 - (4 * i) // n) for i in range(n)]
    prios = list(range(n - 1, -1, -1))
    return "gains=" + ",".join(map(str, gains)) + ";priorities=" + ",".join(map(str, prios))


def write_pgx(path, w, h, depth, data):
    with open(path, "wb") as f:
        f.write(f"PG ML +{depth} {w} {h}\n".encode())
        if depth <= 8:
            f.write(bytes(data))
        else:
            f.write(struct.pack(f">{len(data)}H", *data))


def run(args):
    r = subprocess.run(args, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    return r.returncode, r.stdout.decode("utf-8", "replace")


def reference(dec, src, base, keep, extra=()):
    """Decodes `src` with the reference decoder; (samples, sha256) or None. Keeps the samples
    as BASE.ref when asked."""
    tmp = base + ".tmp.yuv16p"
    code, _ = run([dec, "-q", *extra, src, tmp])
    if code != 0 or not os.path.exists(tmp):
        if os.path.exists(tmp):
            os.remove(tmp)
        return None
    return finish(tmp, base, keep)


def finish(tmp, base, keep):
    h = hashlib.sha256()
    n = 0
    with open(tmp, "rb") as f:
        while True:
            b = f.read(1 << 20)
            if not b:
                break
            h.update(b)
            n += len(b)
    if keep:
        os.replace(tmp, base + ".ref")
    else:
        os.remove(tmp)
    return (n // 2, h.hexdigest())


def write_manifest(d, lines):
    with open(os.path.join(d, "manifest.tsv"), "w") as fh:
        fh.write("# file, outcome, samples, SHA-256 of the reference decoder's samples, note\n")
        fh.write("\n".join(lines) + "\n")


def synth(bin_dir, out, small, big, keep):
    enc = os.path.join(bin_dir, "jxs_encoder")
    dec = os.path.join(bin_dir, "jxs_decoder")
    os.makedirs(out, exist_ok=True)
    work = os.path.join(out, "work")
    os.makedirs(work, exist_ok=True)
    cases = [(n, s, d, c, e, 64, 32) if small else (n, s, d, c, e, 256, 128) for (n, s, d, c, e) in MATRIX]
    cases += [(n, s, d, c, e, 88 if small else 280, 32 if small else 128) for (n, s, d, c, e) in COLUMNS]
    if not small:
        cases += ODD
    if big:
        cases.append(("4k-main422-10", "422", 10, "p=Main422.10;rate=5", "ok", 4096, 2160))
    lines = []
    for i, (name, samp, depth, cfg, expect, w, h) in enumerate(cases):
        for f in os.listdir(work):
            os.remove(os.path.join(work, f))
        comps = comps_of(samp, w, h)
        for k, (cw, ch) in enumerate(comps):
            write_pgx(os.path.join(work, f"in_{k}.pgx"), cw, ch, depth, plane(cw, ch, depth, k, i + 1))
        jxs = os.path.join(out, name + ".jxs")
        full = cfg
        if "p=unrestricted" in cfg and "bw=" not in cfg:
            full += ";bw=20"
        if "p=unrestricted" in cfg and "fq=" not in cfg:
            full += ";fq=8"
        code, log = run([enc, "-q", "-c", full, os.path.join(work, "in_0.pgx"), jxs])
        need = re.search(r"need (\d+) band values", log)
        if code != 0 and need:
            full += ";" + explicit_gains(int(need.group(1)))
            code, log = run([enc, "-q", "-c", full, os.path.join(work, "in_0.pgx"), jxs])
        for _ in range(4):
            # Small pictures cannot reach low rates: precinct headers alone are too big.
            if code == 0 or "RATE Error" not in log:
                break
            full = re.sub(r"rate=(\d+)", lambda m: f"rate={int(m.group(1)) * 2}", full)
            code, log = run([enc, "-q", "-c", full, os.path.join(work, "in_0.pgx"), jxs])
        if code != 0:
            print(f"{name}: encoder failed\n{log}", file=sys.stderr)
            continue
        r = reference(dec, jxs, os.path.join(out, name), keep)
        if r is None:
            print(f"{name}: reference decoder failed", file=sys.stderr)
            continue
        lines.append(f"{name}.jxs\t{expect}\t{r[0]}\t{r[1]}\t{full}")
        print(f"{name}: {os.path.getsize(jxs)} bytes")
    shutil.rmtree(work)
    write_manifest(out, lines)


SIGNATURE = b"\x00\x00\x00\x0cJXS \x0d\x0a\x87\x0a"


def codestream_of(data):
    """The contiguous codestream box of a JPEG XS file (ISO/IEC 21122-3), or the data."""
    if not data.startswith(SIGNATURE):
        return data
    at = 0
    while at + 8 <= len(data):
        (ln, kind) = struct.unpack_from(">I4s", data, at)
        hdr = 8
        if ln == 1:
            (ln,) = struct.unpack_from(">Q", data, at + 8)
            hdr = 16
        elif ln == 0:
            ln = len(data) - at
        if kind == b"jp2c":
            return data[at + hdr:at + ln]
        at += ln
    raise ValueError("no codestream box")


def decode_dir(bin_dir, d, keep):
    dec = os.path.join(bin_dir, "jxs_decoder")
    names = sorted(f for f in os.listdir(d) if f.endswith((".jxc", ".jxs")) and "_frame_" not in f
                   or re.match(r".+_frame_0+[0bt]?\.jxc$", f))
    lines = []
    for f in names:
        base = f.rsplit(".", 1)[0]
        src = os.path.join(d, f)
        with open(src, "rb") as fh:
            data = fh.read()
        if data.startswith(SIGNATURE):
            # The reference decoder reads bare codestreams only.
            src = os.path.join(d, base + ".tmp.jxc")
            with open(src, "wb") as fh:
                fh.write(codestream_of(data))
        r = reference(dec, src, os.path.join(d, base), keep)
        if src.endswith(".tmp.jxc"):
            os.remove(src)
        lines.append(f"{f}\tok\t{r[0]}\t{r[1]}\t" if r else f"{f}\trefused\t-\t-\t")
        print(lines[-1])
    write_manifest(d, lines)


def decode_sequences(bin_dir, d, keep):
    dec = os.path.join(bin_dir, "jxs_decoder")
    groups = {}
    for f in os.listdir(d):
        m = re.match(r"(.+)_frame_(\d{8})\.jxc$", f)
        if m:
            groups.setdefault(m.group(1), []).append(int(m.group(2)))
    lines = []
    for prefix, frames in sorted(groups.items()):
        frames.sort()
        n = len(frames)
        if frames != list(range(frames[0], frames[0] + n)):
            raise SystemExit(f"{prefix}: frames are not consecutive")
        src = os.path.join(d, prefix + "_frame_%08d.jxc")
        out = os.path.join(d, prefix + "_frame_%08d.yuv16p")
        code, log = run([dec, "-q", "-f", str(frames[0]), "-n", str(n), src, out])
        if code != 0:
            raise SystemExit(f"{prefix}: reference decoder failed\n{log}")
        for i in frames:
            base = os.path.join(d, f"{prefix}_frame_{i:08d}")
            r = finish(out % i, base, keep)
            lines.append(f"{prefix}_frame_{i:08d}.jxc\tok\t{r[0]}\t{r[1]}\tframe {i - frames[0] + 1} of {n}")
        print(f"{prefix}: {n} frames")
    write_manifest(d, lines)


def main():
    a = sys.argv[1:]
    keep = "--keep-ref" in a
    if len(a) >= 3 and a[0] == "synth":
        synth(a[1], a[2], "--small" in a, "--4k" in a, keep)
    elif len(a) >= 3 and a[0] == "decode":
        decode_dir(a[1], a[2], keep)
    elif len(a) >= 3 and a[0] == "sequences":
        decode_sequences(a[1], a[2], keep)
    else:
        raise SystemExit(__doc__)


if __name__ == "__main__":
    main()
