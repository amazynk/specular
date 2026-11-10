#!/usr/bin/env python3
"""Make the WavPack / Monkey's Audio / Musepack sample corpus with the reference encoders
built by fetch-lossless.sh, and each sample's reference decode (raw little-endian PCM, or
float bits) beside it.

Usage: python3 -I tools/corpus/lossless-samples.py OUT
  OUT/bin holds wavpack, wvunpack, mac, mpcenc, mpcdec; samples go to OUT/samples.

Each sample X gets X.ref (the reference decoder's raw output) and a line in
OUT/samples/manifest.tsv: name, codec, channels, rate, bytes-per-sample, float(0/1),
correction file or '-'.
"""

import math
import os
import random
import struct
import subprocess
import sys
import wave

OUT = sys.argv[1]
BIN = os.path.join(OUT, "bin")
S = os.path.join(OUT, "samples")
os.makedirs(S, exist_ok=True)
SECONDS = float(os.environ.get("LOSSLESS_SECONDS", "3"))


def run(*args, ok=(0,)):
    r = subprocess.run(args, capture_output=True)
    if r.returncode not in ok:
        sys.stderr.write(f"failed: {' '.join(args)}\n{r.stderr.decode(errors='replace')[-400:]}\n")
        raise SystemExit(1)


def signal(n, ch, rate, seed):
    """Music-like test signal: chirps, a chord, noise bursts and silence, per channel."""
    rnd = random.Random(seed)
    out = []
    for i in range(n):
        t = i / rate
        frame = []
        for c in range(ch):
            f0 = 110 * (c + 1)
            v = 0.3 * math.sin(2 * math.pi * (f0 + 400 * t) * t)
            v += 0.2 * math.sin(2 * math.pi * f0 * 1.5 * t + c)
            if int(t * 4) % 3 == 1:
                v += 0.15 * (rnd.random() * 2 - 1)
            if 0.40 < (t % 1.0) < 0.45:
                v = 0.0  # silence: zero runs
            frame.append(max(-1.0, min(1.0, v)))
        out.append(frame)
    return out


def write_wav(path, frames, ch, rate, bits, flt=False):
    """WAVE (extensible for >2 channels or float or >16 bits)."""
    data = bytearray()
    for fr in frames:
        for v in fr:
            if flt:
                data += struct.pack("<f", v * 0.9)
            elif bits == 8:
                data += struct.pack("<B", int(round(v * 127)) + 128)
            else:
                full = 1 << (bits - 1)
                x = int(round(v * (full - 1)))
                if bits == 16:
                    data += struct.pack("<h", x)
                elif bits == 24:
                    data += struct.pack("<i", x)[:3]
                elif bits == 32:
                    data += struct.pack("<i", x)
    block = ch * bits // 8
    fmt_tag = 3 if flt else 1
    if ch > 2 or bits > 16 or flt:
        mask = {1: 4, 2: 3, 4: 0x33, 6: 0x3F, 8: 0x63F}.get(ch, 0)
        guid = struct.pack("<IHH8s", fmt_tag, 0, 0x10, b"\x80\x00\x00\xaa\x00\x38\x9b\x71")
        fmt = struct.pack("<HHIIHHHHI", 0xFFFE, ch, rate, rate * block, block, bits, 22, bits, mask) + guid
    else:
        fmt = struct.pack("<HHIIHH", fmt_tag, ch, rate, rate * block, block, bits)
    body = b"WAVE" + b"fmt " + struct.pack("<I", len(fmt)) + fmt + b"data" + struct.pack("<I", len(data)) + bytes(data)
    if len(data) & 1:
        body += b"\0"
    with open(path, "wb") as f:
        f.write(b"RIFF" + struct.pack("<I", len(body)) + body)


def wav_data(path):
    """The raw data chunk of a WAVE file (any layout)."""
    with open(path, "rb") as f:
        d = f.read()
    at = 12
    while at + 8 <= len(d):
        cid, size = d[at:at + 4], struct.unpack("<I", d[at + 4:at + 8])[0]
        if cid == b"data":
            return d[at + 8:at + 8 + size]
        at += 8 + size + (size & 1)
    raise SystemExit(f"{path}: no data chunk")


manifest = []


def source(name, ch, rate, bits, flt=False, seconds=SECONDS):
    p = os.path.join(S, f"src-{name}.wav")
    if not os.path.exists(p):
        write_wav(p, signal(int(rate * seconds), ch, rate, hash(name) & 0xFFFF), ch, rate, bits, flt)
    return p


def wavpack(name, src, ch, rate, nbytes, flt, opts, wvc=False):
    out = os.path.join(S, f"{name}.wv")
    run(os.path.join(BIN, "wavpack"), "-q", "-y", *opts, src, "-o", out)
    # Lossy decode (no correction file in sight) and, if any, the corrected decode.
    ref = out + ".ref"
    tmp = os.path.join(S, "tmp.wav")
    if wvc:
        hidden = out[:-3] + ".hidden-wvc"
        os.replace(out[:-3] + ".wvc", hidden)
        # Blocks failing their check are muted (zeros) by wvunpack, which then exits 1.
        run(os.path.join(BIN, "wvunpack"), "-q", "-y", out, "-o", tmp, ok=(0, 1))
        open(ref, "wb").write(wav_data(tmp))
        os.replace(hidden, out[:-3] + ".wvc")
        run(os.path.join(BIN, "wvunpack"), "-q", "-y", out, "-o", tmp)
        open(out + ".wvc.ref", "wb").write(wav_data(tmp))
    else:
        run(os.path.join(BIN, "wvunpack"), "-q", "-y", out, "-o", tmp)
        open(ref, "wb").write(wav_data(tmp))
    os.remove(tmp)
    manifest.append((os.path.basename(out), "wavpack", ch, rate, nbytes, int(flt),
                     os.path.basename(out[:-3] + ".wvc") if wvc else "-"))


def make_wavpack():
    s16 = source("s16", 2, 44100, 16)
    m16 = source("m16", 1, 22050, 16)
    s24 = source("s24", 2, 96000, 24, seconds=1.5)
    s32 = source("s32", 2, 48000, 32, seconds=1.5)
    s8 = source("s8", 1, 8000, 8)
    f32 = source("f32", 2, 48000, 32, flt=True, seconds=1.5)
    c51 = source("c51", 6, 48000, 16, seconds=1.5)
    c4 = source("c4", 4, 44100, 24, seconds=1.0)
    for name, src, ch, rate, nb, flt in [
        ("wv-s16", s16, 2, 44100, 2, False),
        ("wv-m16", m16, 1, 22050, 2, False),
        ("wv-s24", s24, 2, 96000, 3, False),
        ("wv-s32", s32, 2, 48000, 4, False),
        ("wv-s8", s8, 1, 8000, 1, False),
        ("wv-f32", f32, 2, 48000, 4, True),
        ("wv-c51", c51, 6, 48000, 2, False),
        ("wv-c4", c4, 4, 44100, 3, False),
    ]:
        wavpack(name, src, ch, rate, nb, flt, [])
        wavpack(name + "-hh-x6", src, ch, rate, nb, flt, ["-hh", "-x6"])
        wavpack(name + "-f", src, ch, rate, nb, flt, ["-f"])
        wavpack(name + "-b3", src, ch, rate, nb, flt, ["-b3"])
        wavpack(name + "-b4c", src, ch, rate, nb, flt, ["-b4", "-c"], wvc=True)
    wavpack("wv-s16-b2-s05", s16, 2, 44100, 2, False, ["-b2.5", "-s0.5"])
    wavpack("wv-s16-b3-shape-c", s16, 2, 44100, 2, False, ["-b3", "-s-0.5", "-c"], wvc=True)
    wavpack("wv-s16-b3-joint", s16, 2, 44100, 2, False, ["-b3", "-j1", "-c", "-x4"], wvc=True)
    wavpack("wv-s16-b3-nojoint", s16, 2, 44100, 2, False, ["-b3", "-j0", "-c"], wvc=True)
    wavpack("wv-m16-b2-c-hh", m16, 1, 22050, 2, False, ["-b2", "-c", "-hh"], wvc=True)
    wavpack("wv-f32-b4-c-x3", f32, 2, 48000, 4, True, ["-b4", "-c", "-x3"], wvc=True)
    wavpack("wv-s32-b5-c", s32, 2, 48000, 4, False, ["-b5", "-c"], wvc=True)
    wavpack("wv-s16-blk", s16, 2, 44100, 2, False, ["--blocksize=2000", "-x"])


def ape(name, src, ch, rate, nb, flt, level):
    out = os.path.join(S, f"{name}.ape")
    run(os.path.join(BIN, "mac"), src, out, f"-c{level}")
    tmp = os.path.join(S, "tmp.wav")
    run(os.path.join(BIN, "mac"), out, tmp, "-d")
    open(out + ".ref", "wb").write(wav_data(tmp))
    os.remove(tmp)
    manifest.append((os.path.basename(out), "ape", ch, rate, nb, int(flt), "-"))


def make_ape():
    srcs = [
        ("s16", source("s16", 2, 44100, 16), 2, 44100, 2, False),
        ("m16", source("m16", 1, 22050, 16), 1, 22050, 2, False),
        ("s24", source("s24", 2, 96000, 24, seconds=1.5), 2, 96000, 3, False),
        ("s32", source("s32", 2, 48000, 32, seconds=1.5), 2, 48000, 4, False),
        ("s8", source("s8", 1, 8000, 8), 1, 8000, 1, False),
        ("f32", source("f32", 2, 48000, 32, flt=True, seconds=1.5), 2, 48000, 4, True),
        ("c51", source("c51", 6, 48000, 16, seconds=1.5), 6, 48000, 2, False),
        ("c4", source("c4", 4, 44100, 24, seconds=1.0), 4, 44100, 3, False),
        ("st8", source("st8", 2, 11025, 8), 2, 11025, 1, False),
    ]
    for tag, src, ch, rate, nb, flt in srcs:
        for level in (1000, 2000, 3000, 4000, 5000):
            ape(f"ape-{tag}-{level // 1000}", src, ch, rate, nb, flt, level)


def mpc(name, src, ch, rate, opts):
    out = os.path.join(S, f"{name}.mpc")
    run(os.path.join(BIN, "mpcenc"), "--silent", "--overwrite", *opts, src, out)
    tmp = os.path.join(S, "tmp.wav")
    run(os.path.join(BIN, "mpcdec"), out, tmp)
    open(out + ".ref", "wb").write(wav_data(tmp))
    os.remove(tmp)
    manifest.append((os.path.basename(out), "mpc", ch, rate, 2, 0, "-"))


def make_mpc():
    s16 = source("s16", 2, 44100, 16)
    m44 = source("m44", 1, 44100, 16, seconds=2)
    s48 = source("s48", 2, 48000, 16, seconds=2)
    s32k = source("s32k", 2, 32000, 16, seconds=2)
    for q in ("--telephone", "--radio", "--standard", "--xtreme", "--insane", "--braindead"):
        mpc(f"mpc-s16{q[1:]}", s16, 2, 44100, [q])
    mpc("mpc-m44-standard", m44, 1, 44100, ["--standard"])
    mpc("mpc-s48-standard", s48, 2, 48000, ["--standard"])
    mpc("mpc-s32k-q3", s32k, 2, 32000, ["--quality", "3"])
    mpc("mpc-s16-q9-ms0", s16, 2, 44100, ["--quality", "9", "--ms", "0"])


def make_fate():
    """The FATE samples fetched by fetch-lossless.sh, each decoded by its reference decoder."""
    fate = os.path.join(OUT, "fate")
    if not os.path.isdir(fate):
        return
    tmp = os.path.join(S, "tmp.wav")
    for name in sorted(os.listdir(fate)):
        src = os.path.join(fate, name)
        ext = name.rsplit(".", 1)[-1]
        if ext == "wv":
            # Partial (cut) files end in an error the decoder reports with exit 1.
            r = subprocess.run([os.path.join(BIN, "wvunpack"), "-q", "-y", src, "-o", tmp], capture_output=True)
            codec = "wavpack"
        elif ext == "ape":
            r = subprocess.run([os.path.join(BIN, "mac"), src, tmp, "-d"], capture_output=True)
            codec = "ape"
        elif ext == "mpc":
            r = subprocess.run([os.path.join(BIN, "mpcdec"), src, tmp], capture_output=True)
            codec = "mpc"
        else:
            continue
        ref = os.path.join(S, f"fate-{name}.ref")
        data = b""
        if os.path.exists(tmp):
            try:
                data = wav_data(tmp)
            except SystemExit:
                data = b""  # the reference stopped before any sound
            os.remove(tmp)
        open(ref, "wb").write(data)
        manifest.append((f"../fate/{name}", codec, "?", "?", "?", "?", "-", f"fate-{name}.ref", r.returncode))


if __name__ == "__main__":
    which = sys.argv[2:] or ["wavpack", "ape", "mpc", "fate"]
    if "wavpack" in which:
        make_wavpack()
    if "ape" in which:
        make_ape()
    if "mpc" in which:
        make_mpc()
    if "fate" in which:
        make_fate()
    path = os.path.join(S, "manifest.tsv")
    old = []
    if os.path.exists(path):
        names = {m[0] for m in manifest}
        old = [l for l in open(path).read().splitlines() if l.split("\t")[0] not in names]
    with open(path, "w") as f:
        for l in old:
            f.write(l + "\n")
        for m in manifest:
            f.write("\t".join(map(str, m)) + "\n")
    print(f"lossless-samples: {len(manifest)} samples in {S}")
