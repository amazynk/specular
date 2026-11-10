# Run: python3 -I opus_fec_oracle.py snd-opus-fec.opus 10,20,29,30,40,44 (system libopus).
"""libopus (black box, ctypes) on the FEC fixture with pages dropped: per lost packet the SNR
of libopus's FEC decode against its intact decode, and whether the next packet has LBRR."""
import ctypes, math, sys

lib = ctypes.CDLL("libopus.so.0")
lib.opus_decoder_create.restype = ctypes.c_void_p
lib.opus_decoder_create.argtypes = [ctypes.c_int, ctypes.c_int, ctypes.POINTER(ctypes.c_int)]
lib.opus_decode_float.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_int, ctypes.POINTER(ctypes.c_float), ctypes.c_int, ctypes.c_int]
d = open(sys.argv[1], "rb").read()
lost = [int(x) for x in sys.argv[2].split(",")]
pk = []
i = 0
while i < len(d):
    n = d[i + 26]
    lac = d[i + 27:i + 27 + n]
    body = d[i + 27 + n:i + 27 + n + sum(lac)]
    o = 0
    cur = b""
    for l in lac:
        cur += body[o:o + l]
        o += l
        if l < 255:
            pk.append(cur)
            cur = b""
    i += 27 + n + sum(lac)
pk = pk[2:]


def dec(fec):
    err = ctypes.c_int()
    st = lib.opus_decoder_create(48000, 1, ctypes.byref(err))
    out = []
    buf = (ctypes.c_float * 5760)()
    for k, p in enumerate(pk):
        if fec is not None and k in lost:
            nxt = pk[k + 1] if k + 1 < len(pk) and (k + 1) not in lost else None
            if nxt is not None and fec:
                n = lib.opus_decode_float(st, nxt, len(nxt), buf, 960, 1)
            else:
                n = lib.opus_decode_float(st, None, 0, buf, 960, 0)
        else:
            n = lib.opus_decode_float(st, p, len(p), buf, 5760, 0)
        out.extend(buf[:n])
    return out


def snr(a, b, r):
    s = sum(a[i] ** 2 for i in r)
    e = sum((a[i] - b[i]) ** 2 for i in r)
    return 10 * math.log10(s / max(e, 1e-20))


want = dec(None)
fec = dec(True)
conc = dec(False)
has = getattr(lib, "opus_packet_has_lbrr", None)
for k in lost:
    r = range(k * 960, k * 960 + 960)
    lb = has(pk[k + 1], len(pk[k + 1])) if has and k + 1 < len(pk) else "?"
    print(f"lost {k}: libopus FEC {snr(want, fec, r):.1f} dB, concealed {snr(want, conc, r):.1f} dB, next has LBRR {lb}")
