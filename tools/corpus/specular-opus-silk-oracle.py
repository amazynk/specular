#!/usr/bin/env python3
"""SILK oracle for specular-opus (crates/formats/specular-opus/src/silk_study.rs).

Decodes packet files with the system libopus as a black box (ctypes on libopus.so.0; no
source read) at a given API rate, so that SILK frames whose internal rate equals that rate
come out without libopus's resampler. Writes, per input and rate, three decodes:

  <name>.<rate>.pcm       every packet decoded
  <name>.<rate>.plc.pcm   the packets in LOST concealed (decode with no data)
  <name>.<rate>.fec.pcm   the same, but a lost packet is rebuilt from the next packet's
                          LBRR data (decode_fec=1) when that packet exists

s16le, channels as the stream (1 or 2), no pre-skip trimming. Inputs are opus_demo-style
packet files (per packet: u32 BE length, u32 BE final range, packet), or Ogg Opus files
which are converted to that format first (<name>.bit next to the output).

Usage: specular-opus-silk-oracle.py <out dir> <input>...
"""
import ctypes
import os
import struct
import sys

lib = ctypes.CDLL("libopus.so.0")
lib.opus_decoder_create.restype = ctypes.c_void_p
lib.opus_decoder_create.argtypes = [ctypes.c_int32, ctypes.c_int, ctypes.POINTER(ctypes.c_int)]
lib.opus_decode.restype = ctypes.c_int
lib.opus_decode.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_int32,
                            ctypes.POINTER(ctypes.c_int16), ctypes.c_int, ctypes.c_int]
lib.opus_decoder_destroy.argtypes = [ctypes.c_void_p]
lib.opus_packet_get_nb_samples.argtypes = [ctypes.c_char_p, ctypes.c_int32, ctypes.c_int32]


def lost(i):
    """The loss pattern shared with silk_study.rs: every 9th packet from 5, and 23..25."""
    return (i >= 5 and (i - 5) % 9 == 0) or 23 <= i <= 25


def ogg_packets(data):
    pkts, cur, p = [], b"", 0
    while p + 27 <= len(data):
        assert data[p:p + 4] == b"OggS"
        nseg = data[p + 26]
        segs = data[p + 27:p + 27 + nseg]
        q = p + 27 + nseg
        for s in segs:
            cur += data[q:q + s]
            q += s
            if s < 255:
                pkts.append(cur)
                cur = b""
        p = q
    return pkts


def read_bit(data):
    out, p = [], 0
    while p + 8 <= len(data):
        n = struct.unpack(">I", data[p:p + 4])[0]
        out.append(data[p + 8:p + 8 + n])
        p += 8 + n
    return out


def decode(pkts, rate, channels, mode):
    err = ctypes.c_int()
    dec = lib.opus_decoder_create(rate, channels, ctypes.byref(err))
    assert err.value == 0
    buf = (ctypes.c_int16 * (5760 * 2))()
    out = bytearray()
    last = 960 * rate // 48000
    for i, p in enumerate(pkts):
        if mode != "all" and lost(i):
            nxt = pkts[i + 1] if i + 1 < len(pkts) and not lost(i + 1) else None
            if mode == "fec" and nxt is not None:
                n = lib.opus_decode(dec, nxt, len(nxt), buf, last, 1)
            else:
                n = lib.opus_decode(dec, None, 0, buf, last, 0)
        else:
            n = lib.opus_decode(dec, p, len(p), buf, 5760, 0)
            if n > 0:
                last = n
        assert n >= 0, (i, n)
        out += bytes(memoryview(buf).cast("B")[:n * channels * 2])
    lib.opus_decoder_destroy(dec)
    return bytes(out)


def main():
    outdir = sys.argv[1]
    os.makedirs(outdir, exist_ok=True)
    for path in sys.argv[2:]:
        data = open(path, "rb").read()
        name = os.path.splitext(os.path.basename(path))[0]
        if data[:4] == b"OggS":
            pk = ogg_packets(data)
            head = pk[0]
            channels = head[9]
            pkts = pk[2:]
            with open(os.path.join(outdir, name + ".bit"), "wb") as f:
                for p in pkts:
                    f.write(struct.pack(">II", len(p), 0) + p)
        else:
            pkts = read_bit(data)
            channels = 2
        for rate in (8000, 12000, 16000):
            for mode, suf in (("all", ""), ("plc", ".plc"), ("fec", ".fec")):
                pcm = decode(pkts, rate, channels, mode)
                with open(os.path.join(outdir, f"{name}.{rate // 1000}{suf}.pcm"), "wb") as f:
                    f.write(pcm)
        print(name, channels, len(pkts))


main()
