#!/usr/bin/env python3
"""AMR / AMR-WB fixtures and oracle for specular-amr.

Usage: tools/corpus/amr-oracle.py OUT_DIR [--ref NB_DECODER WB_DECODER] [--ffmpeg]

Encodes a synthetic speech-like test signal (voiced vowels with a gliding pitch, a pause
of low noise for DTX, a fricative burst) in every mode with the system's
libopencore-amrnb encoder (AMR) and libvo-amrwbenc (AMR-WB), writes `.amr`/`.awb` storage
files, a mode-switching file with DTX, and a copy with lost, damaged and empty frames.
The reference decode is the 3GPP fixed-point decoder of TS 26.073 / TS 26.173 built
from the specification by tools/corpus/fetch-amr.sh (--ref; storage-format input), else
libopencore-amrnb / libopencore-amrwb. opencore-amr ignores the frame quality bit, so
only the 3GPP decoders are a valid oracle for damaged frames. The decode is recorded as
an FNV-1a 64 hash of the 16-bit output in OUT_DIR/oracle.txt;
`cargo test -p specular-amr --test fixtures` checks our decoder against it.

With --ffmpeg, also reports SNR and largest difference of FFmpeg's own floating-point
AMR decoders against the fixed-point reference (for the record; not a test).

Only the system libraries are loaded (ctypes); nothing is downloaded.
"""

import ctypes
import math
import os
import subprocess
import sys
import tempfile

NB = ctypes.CDLL("libopencore-amrnb.so.0")
WB = ctypes.CDLL("libopencore-amrwb.so.0")
WBE = ctypes.CDLL("libvo-amrwbenc.so.0")
for f in (NB.Encoder_Interface_init, NB.Decoder_Interface_init, WB.D_IF_init, WBE.E_IF_init):
    f.restype = ctypes.c_void_p
NB.Encoder_Interface_init.argtypes = [ctypes.c_int]
WBE.E_IF_init.argtypes = []
NB.Encoder_Interface_Encode.argtypes = [ctypes.c_void_p, ctypes.c_int, ctypes.POINTER(ctypes.c_short),
                                        ctypes.POINTER(ctypes.c_ubyte), ctypes.c_int]
WBE.E_IF_encode.argtypes = [ctypes.c_void_p, ctypes.c_int, ctypes.POINTER(ctypes.c_short),
                            ctypes.POINTER(ctypes.c_ubyte), ctypes.c_int]
NB.Decoder_Interface_Decode.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_ubyte),
                                        ctypes.POINTER(ctypes.c_short), ctypes.c_int]
WB.D_IF_decode.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_ubyte), ctypes.POINTER(ctypes.c_short),
                           ctypes.c_int]
for f in (NB.Encoder_Interface_exit, NB.Decoder_Interface_exit, WB.D_IF_exit, WBE.E_IF_exit):
    f.argtypes = [ctypes.c_void_p]

NB_SIZE = [12, 13, 15, 17, 19, 20, 26, 31, 5, 0, 0, 0, 0, 0, 0, 0]
WB_SIZE = [17, 23, 32, 36, 40, 46, 50, 58, 60, 5, 0, 0, 0, 0, 0, 0]
NB_MODES = ["475", "515", "59", "67", "74", "795", "102", "122"]
WB_MODES = ["660", "885", "1265", "1425", "1585", "1825", "1985", "2305", "2385"]


class Lcg:
    def __init__(self, seed):
        self.s = seed

    def next(self):
        self.s = (self.s * 1103515245 + 12345) & 0x7FFFFFFF
        return self.s / 0x7FFFFFFF * 2 - 1


def signal(rate, seconds=1.2):
    """Voiced vowels (glottal pulses through two formant resonators) with a pitch glide,
    then low noise (a speech pause), then a fricative burst."""
    rnd = Lcg(1234)
    n = int(rate * seconds)
    out = []
    phase = 0.0
    y1 = y2 = z1 = z2 = 0.0

    def reson(f, bw):
        r = math.exp(-math.pi * bw / rate)
        return 2 * r * math.cos(2 * math.pi * f / rate), -r * r

    for i in range(n):
        t = i / rate
        if t < 0.6:
            f0 = 110 + 90 * t / 0.6
            phase += f0 / rate
            x = 1.0 if phase >= 1 else 0.0
            phase -= int(phase)
            f1, f2 = (700, 1200) if t < 0.3 else (300, 2300)
            a1, b1 = reson(f1, 90)
            a2, b2 = reson(f2, 120)
            y = x + a1 * y1 + b1 * y2
            y2, y1 = y1, y
            z = y + a2 * z1 + b2 * z2
            z2, z1 = z1, z
            v = 900 * z
        elif t < 0.95:
            v = 40 * rnd.next()
        else:
            v = 2500 * rnd.next() * math.sin(math.pi * (t - 0.95) / 0.25)
        out.append(max(-32768, min(32767, int(v))))
    return out


def encode(codec, samples, modes, dtx):
    frame = 160 if codec == "nb" else 320
    st = NB.Encoder_Interface_init(int(dtx)) if codec == "nb" else WBE.E_IF_init()
    buf = (ctypes.c_ubyte * 64)()
    frames = []
    for k in range(len(samples) // frame):
        pcm = (ctypes.c_short * frame)(*samples[k * frame:(k + 1) * frame])
        mode = modes[k % len(modes)]
        if codec == "nb":
            n = NB.Encoder_Interface_Encode(st, mode, pcm, buf, 0)
        else:
            n = WBE.E_IF_encode(st, mode, pcm, buf, int(dtx))
        frames.append(bytes(buf[:n]))
    (NB.Encoder_Interface_exit if codec == "nb" else WBE.E_IF_exit)(st)
    return frames


def decode(codec, frames):
    frame = 160 if codec == "nb" else 320
    st = NB.Decoder_Interface_init() if codec == "nb" else WB.D_IF_init()
    out = []
    pcm = (ctypes.c_short * frame)()
    for f in frames:
        b = (ctypes.c_ubyte * 64)(*f)
        if codec == "nb":
            NB.Decoder_Interface_Decode(st, b, pcm, 0)
        else:
            WB.D_IF_decode(st, b, pcm, 0)
        out.extend(pcm)
    (NB.Decoder_Interface_exit if codec == "nb" else WB.D_IF_exit)(st)
    return out


def decode_ref(binary, path):
    with tempfile.TemporaryDirectory(dir=os.path.dirname(path)) as td:
        raw = os.path.join(td, "o.raw")
        args = [binary, path, raw] if binary.endswith("nb/decoder") else [binary, "-mime", path, raw]
        subprocess.run(args, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        data = open(raw, "rb").read()
    return [int.from_bytes(data[i:i + 2], "little", signed=True) for i in range(0, len(data), 2)]


def fnv(samples):
    h = 0xCBF29CE484222325
    for s in samples:
        for b in (s & 0xFF, (s >> 8) & 0xFF):
            h = ((h ^ b) * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return h


def damage(codec, frames):
    """Every 7th frame lost (NO_DATA), every 11th marked damaged (Q = 0), and a run of
    four lost frames in the voiced part."""
    out = []
    for k, f in enumerate(frames):
        if k % 7 == 3 or 12 <= k < 16:
            out.append(bytes([0x7C]))  # FT 15, Q 1: no data
        elif k % 11 == 5:
            out.append(bytes([f[0] & ~0x04]) + f[1:])
        else:
            out.append(f)
    return out


def main():
    out_dir = sys.argv[1]
    use_ffmpeg = "--ffmpeg" in sys.argv
    ref = None
    if "--ref" in sys.argv:
        k = sys.argv.index("--ref")
        ref = {"nb": os.path.abspath(sys.argv[k + 1]), "wb": os.path.abspath(sys.argv[k + 2])}
    os.makedirs(out_dir, exist_ok=True)
    cases = []
    for codec, rate, modes, names, magic, ext in (
        ("nb", 8000, range(8), NB_MODES, b"#!AMR\n", "amr"),
        ("wb", 16000, range(9), WB_MODES, b"#!AMR-WB\n", "awb"),
    ):
        sig = signal(rate)
        for m, name in zip(modes, names):
            cases.append((f"{codec}-{name}.{ext}", codec, magic, encode(codec, sig, [m], False)))
        switching = encode(codec, sig, list(modes)[::-1], True)
        cases.append((f"{codec}-switch-dtx.{ext}", codec, magic, switching))
        top = len(names) - 1
        cases.append((f"{codec}-lost.{ext}", codec, magic, damage(codec, encode(codec, sig, [top], True))))
    who = "3GPP TS 26.073/26.173 reference decoder" if ref else "libopencore-amr"
    lines = [f"# name frames fnv1a64-of-reference-decode ({who})"]
    for name, codec, magic, frames in cases:
        with open(os.path.join(out_dir, name), "wb") as fh:
            fh.write(magic + b"".join(frames))
        path = os.path.join(out_dir, name)
        out = decode_ref(ref[codec], path) if ref else decode(codec, frames)
        if ref:
            oc = decode(codec, frames)
            same = "same" if oc == out else "differs (opencore-amr ignores the quality bit)"
            print(f"opencore-amr vs 3GPP reference {name:22} {same}")
        lines.append(f"{name} {len(frames)} {fnv(out):016x}")
        with open(path + ".ref", "wb") as fh:
            fh.write(b"".join(int(s).to_bytes(2, "little", signed=True) for s in out))
        if use_ffmpeg:
            with tempfile.TemporaryDirectory(dir=out_dir) as td:
                raw = os.path.join(td, "o.raw")
                subprocess.run(["ffmpeg", "-nostdin", "-loglevel", "error", "-y", "-i",
                                os.path.join(out_dir, name), "-f", "s16le", raw], check=True)
                data = open(raw, "rb").read()
            ff = [int.from_bytes(data[i:i + 2], "little", signed=True) for i in range(0, len(data), 2)]
            n = min(len(ff), len(out))
            sig_e = sum(r * r for r in out[:n]) or 1
            err = sum((ff[i] - out[i]) ** 2 for i in range(n))
            mx = max((abs(ff[i] - out[i]) for i in range(n)), default=0)
            snr = float("inf") if err == 0 else 10 * math.log10(sig_e / err)
            print(f"ffmpeg vs reference {name:22} snr {snr:6.1f} dB  max {mx}")
    with open(os.path.join(out_dir, "oracle.txt"), "w") as fh:
        fh.write("\n".join(lines) + "\n")
    print("\n".join(lines))


if __name__ == "__main__":
    main()
