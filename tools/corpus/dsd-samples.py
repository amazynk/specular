#!/usr/bin/env python3
"""Make DSD (1-bit) WavPack samples and their references, for specular-wavpack's `dsd` test.

Usage: python3 -I tools/corpus/dsd-samples.py OUT
       python3 -I tools/corpus/dsd-samples.py OUT fixtures
  OUT/bin holds wavpack and wvunpack (built by fetch-lossless.sh); samples go to
  OUT/samples/dsd. Needs numpy. `fixtures` instead remakes the small committed test files:
  crates/formats/specular-wavpack/tests/data/dsd-{fast,high,stored}.wv with their .raw, and
  crates/specular-video/tests/data/snd-dsd.wv with snd-dsd.pcm.

Sources are DSF files made here by a second-order sigma-delta modulator (tones, a sweep,
noise), plus incompressible random bits (stored blocks), identical channels (false stereo)
and FATE's dsd.wv when fetched. Each X.wv gets:
  X.raw  wvunpack -r: the DSD bytes, channels interleaved by byte, oldest bit on top;
  X.pcm  the PCM of specular-wavpack's decimation filter, computed here with numpy from
         X.raw (float32 little-endian, interleaved): the same Kaiser-windowed sinc,
         applied to the ±1 stream with 0x55 bytes before the start.
and a line in manifest.tsv: the .wv (relative), X, channels, DSD rate, mode.
"""

import math
import os
import struct
import subprocess
import sys

import numpy as np

OUT = sys.argv[1]
BIN = os.path.join(OUT, "bin")
S = os.path.join(OUT, "samples", "dsd")
os.makedirs(S, exist_ok=True)


def run(*args):
    r = subprocess.run(args, capture_output=True)
    if r.returncode != 0:
        sys.stderr.write(f"failed: {' '.join(args)}\n{r.stderr.decode(errors='replace')[-400:]}\n")
        raise SystemExit(1)


def sdm(x):
    """Second-order sigma-delta modulation of x (|x| < 0.5) to bits (0/1)."""
    out = np.empty(len(x), dtype=np.uint8)
    i1 = i2 = 0.0
    y = 0.0
    for n, v in enumerate(x.tolist()):
        i1 += v - y
        i2 += i1 - y
        b = 1 if i2 >= 0 else 0
        y = 1.0 if b else -1.0
        out[n] = b
    return out


def signal(kind, n, rate, ch, rng):
    t = np.arange(n) / rate
    chans = []
    for c in range(ch):
        if kind == "tone":
            x = 0.4 * np.sin(2 * math.pi * (440 * (c + 1)) * t)
        elif kind == "sweep":
            f = 20 + (20000 - 20) * t / t[-1]
            x = 0.35 * np.sin(2 * math.pi * np.cumsum(f) / rate + c)
        else:
            x = 0.3 * np.convolve(rng.standard_normal(n), np.ones(64) / 8, "same").clip(-1.4, 1.4) / 1.4
        chans.append(x)
    return chans


def write_dsf(path, chans_bits, rate):
    """A DSF file (bits LSB first, 4096-byte blocks a channel)."""
    ch = len(chans_bits)
    n = len(chans_bits[0])
    blocks = []
    for b in chans_bits:
        by = np.packbits(b, bitorder="little")
        pad = (-len(by)) % 4096
        blocks.append(np.concatenate([by, np.zeros(pad, np.uint8)]))
    nblk = len(blocks[0]) // 4096
    data = bytearray()
    for k in range(nblk):
        for c in range(ch):
            data += blocks[c][k * 4096:(k + 1) * 4096].tobytes()
    ctype = {1: 1, 2: 2, 3: 3, 4: 4, 5: 6, 6: 7}[ch]
    fmt = b"fmt " + struct.pack("<QIIIIIIQII", 52, 1, 0, ctype, ch, rate, 1, n, 4096, 0)
    body = fmt + b"data" + struct.pack("<Q", 12 + len(data)) + bytes(data)
    head = b"DSD " + struct.pack("<QQQ", 28, 28 + len(body), 0)
    open(path, "wb").write(head + body)


# The decimation filter, as specular-wavpack's dsd::FilterSpec designs it.
def spec(byte_rate):
    dec, pcm = 1, byte_rate
    while pcm % 2 == 0 and pcm // 2 >= 88200:
        pcm //= 2
        dec *= 2
    fs = byte_rate * 8.0
    stop = pcm // 2
    passb = min(20000, stop // 10 * 9)
    df = (stop - passb) / fs
    n = math.ceil((110 - 7.95) / (14.36 * df) + 1.0)
    taps = (n + 7) // 8 * 8
    return dec, pcm, passb, stop, taps


def coefficients(byte_rate):
    dec, pcm, passb, stop, n = spec(byte_rate)
    fs = byte_rate * 8.0
    fc = (passb + stop) / 2.0 / fs
    beta = 0.1102 * (110 - 8.7)
    x = np.arange(n) - (n - 1) / 2.0
    h = 2 * fc * np.sinc(2 * fc * x) * np.kaiser(n, beta)
    return h / h.sum(), dec


def reference_pcm(raw, ch, byte_rate):
    h, dec = coefficients(byte_rate)
    t = len(h) // 8
    by = np.frombuffer(raw, np.uint8).reshape(-1, ch)
    nbytes = by.shape[0]
    out = []
    for c in range(ch):
        hist = np.concatenate([np.full(t - 1, 0x55, np.uint8), by[:, c]])
        bits = np.unpackbits(hist).astype(np.float64) * 2 - 1  # MSB first: oldest bit first
        # A sample after each byte q with (q + 1) % dec == 0: Σ h[k]·bit[i − k], i its
        # newest bit.
        q = np.arange(dec - 1, nbytes, dec)
        i = 8 * (q + t - 1) + 7
        win = np.lib.stride_tricks.sliding_window_view(bits, len(h))[i - len(h) + 1]
        out.append(win @ h[::-1])
    return np.stack(out, axis=1).astype("<f4").tobytes()


manifest = []


def add(name, wv, ch, byte_rate, mode):
    raw = os.path.join(S, name + ".raw")
    run(os.path.join(BIN, "wvunpack"), "-q", "-y", "-r", wv, "-o", raw)
    data = open(raw, "rb").read()
    open(os.path.join(S, name + ".pcm"), "wb").write(reference_pcm(data, ch, byte_rate))
    manifest.append((os.path.relpath(wv, S), name, ch, byte_rate * 8, mode))


def make(name, kind, ch, mult, seconds, opts, rng):
    rate = 44100 * 64 * mult
    n = int(rate * seconds) // 8 * 8 + 8 * 37  # a length that isn't a whole DSF block
    if kind == "random":
        bits = [rng.integers(0, 2, n, dtype=np.uint8) for _ in range(ch)]
    elif kind == "same":
        b = sdm(signal("tone", n, rate, 1, rng)[0])
        bits = [b] * ch
    else:
        bits = [sdm(x) for x in signal(kind, n, rate, ch, rng)]
    src = os.path.join(S, name + ".dsf")
    write_dsf(src, bits, rate)
    for mode, o in opts:
        wv = os.path.join(S, f"{name}-{mode}.wv")
        run(os.path.join(BIN, "wavpack"), "-q", "-y", *o, src, "-o", wv)
        add(f"{name}-{mode}", wv, ch, rate // 8, mode)
    os.remove(src)


def fixtures():
    global S
    root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
    wv_dir = os.path.join(root, "crates", "formats", "specular-wavpack", "tests", "data")
    video_dir = os.path.join(root, "crates", "specular-video", "tests", "data")
    os.makedirs(wv_dir, exist_ok=True)
    rng = np.random.default_rng(11)
    rate = 44100 * 64
    n = 8 * 4410  # 12.5 ms
    tone = [sdm(x) for x in signal("tone", n, rate, 2, rng)]
    rand = [rng.integers(0, 2, n, dtype=np.uint8) for _ in range(2)]
    for name, bits, opts in [("dsd-fast", tone, []), ("dsd-high", tone, ["-h"]), ("dsd-stored", rand, [])]:
        src = os.path.join(S, "fixture.dsf")
        write_dsf(src, bits, rate)
        wv = os.path.join(wv_dir, name + ".wv")
        run(os.path.join(BIN, "wavpack"), "-q", "-y", *opts, src, "-o", wv)
        run(os.path.join(BIN, "wvunpack"), "-q", "-y", "-r", wv, "-o", os.path.join(wv_dir, name + ".raw"))
        os.remove(src)
    # The player's fixture: 0.1 s of DSD64 stereo tones, "high" mode.
    n = 8 * 35280
    src = os.path.join(S, "fixture.dsf")
    write_dsf(src, [sdm(x) for x in signal("tone", n, rate, 2, rng)], rate)
    wv = os.path.join(video_dir, "snd-dsd.wv")
    run(os.path.join(BIN, "wavpack"), "-q", "-y", "-h", src, "-o", wv)
    raw = os.path.join(S, "fixture.raw")
    run(os.path.join(BIN, "wvunpack"), "-q", "-y", "-r", wv, "-o", raw)
    open(os.path.join(video_dir, "snd-dsd.pcm"), "wb").write(reference_pcm(open(raw, "rb").read(), 2, rate // 8))
    os.remove(src)
    os.remove(raw)


if __name__ == "__main__" and sys.argv[2:] == ["fixtures"]:
    fixtures()
elif __name__ == "__main__":
    rng = np.random.default_rng(7)
    both = [("fast", []), ("high", ["-h"])]
    make("dsd64-tone-stereo", "tone", 2, 1, 0.5, both + [("fastf", ["-f"])], rng)
    make("dsd64-sweep-mono", "sweep", 1, 1, 0.5, both, rng)
    make("dsd64-noise-3ch", "noise", 3, 1, 0.3, both, rng)
    make("dsd64-random-stereo", "random", 2, 1, 0.2, both, rng)
    make("dsd64-same-stereo", "same", 2, 1, 0.3, both, rng)
    make("dsd128-tone-stereo", "tone", 2, 2, 0.3, both, rng)
    make("dsd256-sweep-stereo", "sweep", 2, 4, 0.2, both, rng)
    make("dsd256-noise-6ch", "noise", 6, 4, 0.05, both, rng)
    fate = os.path.join(OUT, "fate", "dsd.wv")
    if os.path.exists(fate):
        add("fate-dsd", fate, 2, 352800, "fate")
    with open(os.path.join(S, "manifest.tsv"), "w") as f:
        for m in manifest:
            f.write("\t".join(map(str, m)) + "\n")
    print(f"dsd-samples: {len(manifest)} samples in {S}")
