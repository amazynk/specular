#!/usr/bin/env python3
"""Generates the JPEG-LS test vectors in ../tests/vectors with CharLS as the oracle.

CharLS (through the pyjpegls wheel, which bundles its shared library) is only a test oracle:
it encodes small synthetic images and decodes them again for the expected output. Cases CharLS
cannot encode (restart intervals, mapping tables, DNL heights, oversize dimensions) are built by
splicing its output, which the comments below justify from T.87. The custom-MAXVAL case takes
its expected output from libjpeg (the pylibjpeg-libjpeg wheel) instead; see `case`.

Usage: python make_vectors.py OUT_DIR [RAW_DIR]
  OUT_DIR  receives NAME.jls and manifest.txt (name, width, height, components, FNV-1a 64 hash
           of the expected planes as little-endian u16 in component order).
  RAW_DIR  optionally receives NAME.raw with those planes, for debugging.
"""

import ctypes
import glob
import os
import struct
import sys

import numpy as np
import jpeg_ls  # noqa: F401  (locates the bundled CharLS library)

LIB = ctypes.CDLL(
    glob.glob(os.path.join(os.path.dirname(os.path.dirname(jpeg_ls.__file__)), "_CharLS*.so"))[0]
)


class FrameInfo(ctypes.Structure):
    _fields_ = [
        ("width", ctypes.c_uint32),
        ("height", ctypes.c_uint32),
        ("bits_per_sample", ctypes.c_int32),
        ("component_count", ctypes.c_int32),
    ]


class PcParameters(ctypes.Structure):
    _fields_ = [
        ("maximum_sample_value", ctypes.c_int32),
        ("threshold1", ctypes.c_int32),
        ("threshold2", ctypes.c_int32),
        ("threshold3", ctypes.c_int32),
        ("reset_value", ctypes.c_int32),
    ]


for name in [
    "charls_jpegls_encoder_create",
    "charls_jpegls_decoder_create",
]:
    getattr(LIB, name).restype = ctypes.c_void_p
VP = ctypes.c_void_p


def check(err, what):
    if err != 0:
        LIB.charls_get_error_message.restype = ctypes.c_char_p
        raise RuntimeError(f"{what}: {LIB.charls_get_error_message(err).decode()}")


def encode(planes, bits, near=0, ilv=0, preset=None, transform=0):
    """Encodes planar `planes` (list of 2-D uint16 arrays) with CharLS."""
    h, w = planes[0].shape
    n = len(planes)
    dt = np.uint8 if bits <= 8 else np.uint16
    if ilv == 0:
        src = np.stack(planes).astype(dt)
    else:
        src = np.stack(planes, axis=-1).astype(dt)
    src = np.ascontiguousarray(src).tobytes()
    enc = VP(LIB.charls_jpegls_encoder_create())
    fi = FrameInfo(w, h, bits, n)
    check(LIB.charls_jpegls_encoder_set_frame_info(enc, ctypes.byref(fi)), "frame info")
    check(LIB.charls_jpegls_encoder_set_near_lossless(enc, near), "near")
    check(LIB.charls_jpegls_encoder_set_interleave_mode(enc, ilv), "interleave")
    if preset:
        pc = PcParameters(*preset)
        check(
            LIB.charls_jpegls_encoder_set_preset_coding_parameters(enc, ctypes.byref(pc)),
            "preset",
        )
    if transform:
        check(LIB.charls_jpegls_encoder_set_color_transformation(enc, transform), "transform")
    size = ctypes.c_size_t()
    check(LIB.charls_jpegls_encoder_get_estimated_destination_size(enc, ctypes.byref(size)), "size")
    dst = ctypes.create_string_buffer(size.value + 1024)
    check(
        LIB.charls_jpegls_encoder_set_destination_buffer(enc, dst, ctypes.c_size_t(len(dst))),
        "dest",
    )
    check(
        LIB.charls_jpegls_encoder_encode_from_buffer(
            enc, src, ctypes.c_size_t(len(src)), ctypes.c_uint32(0)
        ),
        "encode",
    )
    written = ctypes.c_size_t()
    check(LIB.charls_jpegls_encoder_get_bytes_written(enc, ctypes.byref(written)), "written")
    LIB.charls_jpegls_encoder_destroy(enc)
    return dst.raw[: written.value]


def decode(data, w, h, n, bits, ilv):
    """Decodes with CharLS; returns planar uint16 planes."""
    dec = VP(LIB.charls_jpegls_decoder_create())
    check(LIB.charls_jpegls_decoder_set_source_buffer(dec, data, ctypes.c_size_t(len(data))), "src")
    check(LIB.charls_jpegls_decoder_read_header(dec), "header")
    size = ctypes.c_size_t()
    check(
        LIB.charls_jpegls_decoder_get_destination_size(dec, ctypes.c_uint32(0), ctypes.byref(size)),
        "dsize",
    )
    dst = ctypes.create_string_buffer(size.value)
    check(
        LIB.charls_jpegls_decoder_decode_to_buffer(
            dec, dst, ctypes.c_size_t(size.value), ctypes.c_uint32(0)
        ),
        "decode",
    )
    LIB.charls_jpegls_decoder_destroy(dec)
    dt = np.uint8 if bits <= 8 else np.uint16
    a = np.frombuffer(dst.raw, dtype=dt).astype(np.uint16)
    if ilv == 0:
        a = a.reshape(n, h, w)
        return [a[i] for i in range(n)]
    a = a.reshape(h, w, n)
    return [a[:, :, i] for i in range(n)]


def image(w, h, bits, seed, flat=0.3):
    """Smooth gradients, noise and flat patches (so both regular and run mode are used)."""
    rng = np.random.default_rng(seed)
    y, x = np.mgrid[0:h, 0:w]
    top = (1 << bits) - 1
    a = (x * 3 + y * 5) / (w * 3 + h * 5) * top
    a += np.sin(x / 3.0 + seed) * top / 8
    a += rng.normal(0, max(1.0, top / 64), (h, w))
    mask = rng.random((h // 4 + 1, w // 4 + 1)) < flat
    mask = np.kron(mask, np.ones((4, 4), dtype=bool))[:h, :w]
    a[mask] = top // 3
    a[0:2, :] = top  # a saturated band
    return np.clip(np.rint(a), 0, top).astype(np.uint16)


# --- Splicing helpers (marker segments of T.87 Annex C) ---------------------------------------


def segments(data):
    """Splits a single-scan stream into (header bytes before SOS, SOS segment, scan data, tail)."""
    sos = data.index(b"\xff\xda")
    ls = struct.unpack(">H", data[sos + 2 : sos + 4])[0]
    start = sos + 2 + ls
    end = data.rindex(b"\xff\xd9")
    return data[:sos], data[sos:start], data[start:end], data[end:]


def set_size(head, w, h):
    i = head.index(b"\xff\xf7")
    return head[: i + 5] + struct.pack(">HH", h, w) + head[i + 9 :]


def lse(body):
    return b"\xff\xf8" + struct.pack(">H", len(body) + 2) + body


def fnv(planes):
    h = 0xCBF29CE484222325
    for p in planes:
        for b in np.ascontiguousarray(p.astype("<u2")).tobytes():
            h = ((h ^ b) * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return h


def main():
    out = sys.argv[1]
    raw = sys.argv[2] if len(sys.argv) > 2 else None
    os.makedirs(out, exist_ok=True)
    if raw:
        os.makedirs(raw, exist_ok=True)
    manifest = []

    def emit(name, data, planes, w, h):
        with open(os.path.join(out, name + ".jls"), "wb") as f:
            f.write(data)
        manifest.append(f"{name} {w} {h} {len(planes)} {fnv(planes):016x}")
        if raw:
            with open(os.path.join(raw, name + ".raw"), "wb") as f:
                for p in planes:
                    f.write(np.ascontiguousarray(p.astype("<u2")).tobytes())

    def case(name, w, h, bits, n=1, near=0, ilv=0, preset=None, transform=0, seed=1):
        planes = [image(w, h, bits, seed + i) for i in range(n)]
        if preset and preset[0]:
            # Samples above a custom MAXVAL are invalid input.
            planes = [np.minimum(p, preset[0]) for p in planes]
        data = encode(planes, bits, near, ilv, preset, transform)
        if preset and preset[0] and preset[0] != (1 << bits) - 1:
            # CharLS 2.4.2 codes a custom MAXVAL with RANGE = 2^P instead of MAXVAL + 1
            # (T.87 C.2.4.1.1), so its stream only round-trips through CharLS. libjpeg follows
            # the standard; its reading of the stream is the expected output. Read that way the
            # stream needs more bits than CharLS wrote, so filler bytes are appended to the scan
            # data, keeping the end of the image defined instead of each decoder's padding.
            from libjpeg import decode as libjpeg_decode

            head, sos, scan, tail = segments(data)
            data = head + sos + scan + b"\x5a" * 1024 + tail
            got = [libjpeg_decode(data).astype(np.uint16)]
            emit(name, data, got, w, h)
            return planes, data
        got = decode(data, w, h, n, bits, ilv)
        if near == 0:
            for a, b in zip(planes, got):
                assert (a == b).all(), name
        emit(name, data, got, w, h)
        return planes, data

    # Grey, lossless and near-lossless, across precisions.
    for bits in [2, 3, 5, 8, 10, 12, 16]:
        case(f"grey{bits}", 41, 29, bits, seed=bits)
    case("grey8_near1", 41, 29, 8, near=1)
    case("grey8_near3", 64, 48, 8, near=3, seed=3)
    case("grey12_near2", 47, 33, 12, near=2, seed=4)
    case("grey16_near7", 64, 64, 16, near=7, seed=5)
    case("grey16_big", 96, 80, 16, seed=6)  # RESET reached in many contexts
    # Custom coding parameters (LSE type 1), like conformance test T8NDE.
    case("grey8_preset", 41, 29, 8, near=3, preset=(255, 9, 9, 9, 31), seed=7)
    case("grey12_maxval", 41, 20, 12, preset=(3000, 0, 0, 0, 0), seed=8)
    case("grey16_preset", 41, 29, 16, preset=(0, 30, 100, 400, 200), seed=9)
    # Colour in all interleave modes.
    for ilv in [0, 1, 2]:
        case(f"rgb8_ilv{ilv}", 37, 23, 8, n=3, ilv=ilv, seed=10 + ilv)
        case(f"rgb16_ilv{ilv}", 23, 19, 16, n=3, ilv=ilv, seed=20 + ilv)
        case(f"rgb8_ilv{ilv}_near2", 37, 23, 8, n=3, ilv=ilv, near=2, seed=30 + ilv)
    case("rgba8_ilv2", 31, 17, 8, n=4, ilv=2, seed=40)
    case("rgba12_ilv1", 31, 17, 12, n=4, ilv=1, seed=41)
    # HP colour transforms (APP8 "mrfx").
    for t in [1, 2, 3]:
        case(f"rgb8_hp{t}", 37, 23, 8, n=3, ilv=2, transform=t, seed=50 + t)
        case(f"rgb8_hp{t}_line", 37, 23, 8, n=3, ilv=1, transform=t, seed=55 + t)
    for t in [1, 2, 3]:
        try:
            case(f"rgb16_hp{t}", 23, 19, 16, n=3, ilv=2, transform=t, seed=60 + t)
        except RuntimeError as e:
            print(f"rgb16_hp{t}: CharLS refuses ({e})")

    # Restart intervals: T.87 D.4 resets the coder at each interval exactly as at the start of a
    # scan (contexts, RUNindex, and the previous line as for the first line), so a stream with
    # restart interval R lines is the concatenation of independently encoded R-line strips.
    def restart_case(name, w, h, bits, n, ilv, near, interval, seed, ri_bytes=2):
        planes = [image(w, h, bits, seed + i) for i in range(n)]
        parts = []
        expect = [np.zeros((h, w), np.uint16) for _ in range(n)]
        head = None
        for k, y in enumerate(range(0, h, interval)):
            strip = [p[y : y + interval] for p in planes]
            sh = strip[0].shape[0]
            data = encode(strip, bits, near, ilv)
            got = decode(data, w, sh, n, bits, ilv)
            for e, g in zip(expect, got):
                e[y : y + sh] = g
            hd, sos, scan, _ = segments(data)
            if head is None:
                head = set_size(hd, w, h) + b"\xff\xdd" + struct.pack(">H", 2 + ri_bytes)
                head += interval.to_bytes(ri_bytes, "big") + sos
            if k:
                parts.append(bytes([0xFF, 0xD0 + (k - 1) % 8]))
            parts.append(scan)
        emit(name, head + b"".join(parts) + b"\xff\xd9", expect, w, h)

    restart_case("grey8_restart", 41, 29, 8, 1, 0, 0, 4, 70)
    restart_case("grey16_restart_near", 33, 30, 16, 1, 0, 2, 7, 71, ri_bytes=3)
    restart_case("rgb8_ilv2_restart", 21, 17, 8, 3, 2, 0, 5, 72)
    restart_case("rgb8_ilv1_restart", 21, 17, 8, 3, 1, 0, 3, 75, ri_bytes=4)

    # Mapping table (LSE type 2, 3-byte entries) referenced from the scan: the coded samples are
    # palette indices, so the expected planes are the indices and the table is checked apart.
    idx = image(29, 21, 6, 80)
    data = encode([idx], 6)
    hd, sos, scan, tail = segments(data)
    table = bytes(v for i in range(64) for v in ((i * 4) & 255, 255 - i * 4, (i * 37) & 255))
    sos = sos[:5] + b"\x01\x05" + sos[7:]  # Tm1 = 5
    emit(
        "pal6_table",
        hd + lse(b"\x02\x05\x03" + table[: 32 * 3]) + lse(b"\x03\x05\x03" + table[32 * 3 :])
        + sos + scan + tail,
        [idx],
        29,
        21,
    )

    # Height from DNL (Y = 0 in the frame header, DNL after the first scan).
    g = image(25, 19, 8, 90)
    hd, sos, scan, tail = segments(encode([g], 8))
    emit("grey8_dnl", set_size(hd, 25, 0) + sos + scan + b"\xff\xdc\x00\x04\x00\x13" + tail, [g], 25, 19)
    # Oversize dimensions (LSE type 4 with 3-byte fields, frame header size 0).
    emit(
        "grey8_lse4",
        set_size(hd, 0, 0) + lse(b"\x04\x03" + (19).to_bytes(3, "big") + (25).to_bytes(3, "big"))
        + sos + scan + tail,
        [g],
        25,
        19,
    )

    with open(os.path.join(out, "manifest.txt"), "w") as f:
        f.write("# name width height components fnv1a64(planes as u16le) - make_vectors.py\n")
        f.write("\n".join(manifest) + "\n")
    print(f"{len(manifest)} vectors, {sum(os.path.getsize(os.path.join(out, m.split()[0] + '.jls')) for m in manifest)} bytes")


if __name__ == "__main__":
    main()
