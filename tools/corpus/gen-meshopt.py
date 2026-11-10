#!/usr/bin/env python3
"""Meshopt-compressed glTF test data for specular-mesh (gitignored, never committed).

Usage: PYTHON=<venv>/bin/python tools/corpus/gen-meshopt.py [OUT]
  OUT defaults to $SPECULAR_MESHOPT_CORPUS, else tools/corpus/meshopt.
  Needs numpy and meshoptimizer (pip install meshoptimizer numpy; a venv under target/).

Writes:
  streams/NNN.{json,bin,out}  one compressed stream each: mode, filter, count, stride in the
                              JSON, the stream, and the oracle's decoded bytes (meshoptimizer's
                              own decoder and filters; the colour filter, which this build
                              lacks, from a numpy transcription of the specification)
  models/*.glb                compressed models with an uncompressed twin (`*.plain.glb`) built
                              from the oracle's decoded views; `pirate.glb` is meshoptimizer's
                              demo model (MIT), fetched at a pinned commit
The Rust tests decode every stream and compare byte for byte, and read each model and its twin.
"""
import ctypes
import json
import os
import struct
import sys
import urllib.request

import numpy as np
import meshoptimizer as mo

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
OUT = sys.argv[1] if len(sys.argv) > 1 else os.environ.get(
    "SPECULAR_MESHOPT_CORPUS", os.path.join(ROOT, "tools/corpus/meshopt"))
PIRATE = ("https://raw.githubusercontent.com/zeux/meshoptimizer/"
          "458ebf4794b61db7dd76e8469eaf8369d6603971/demo/pirate.glb")

lib = ctypes.CDLL(os.path.join(os.path.dirname(mo.__file__),
                               [f for f in os.listdir(os.path.dirname(mo.__file__))
                                if f.startswith("_meshoptimizer")][0]))
sz = ctypes.c_size_t
lib.meshopt_encodeVertexBufferBound.restype = sz
lib.meshopt_encodeVertexBufferBound.argtypes = [sz, sz]
lib.meshopt_encodeVertexBufferLevel.restype = sz
lib.meshopt_encodeVertexBufferLevel.argtypes = [ctypes.c_void_p, sz, ctypes.c_void_p, sz, sz,
                                                ctypes.c_int, ctypes.c_int]
for f in ("Oct", "Quat"):
    getattr(lib, "meshopt_encodeFilter" + f).argtypes = [ctypes.c_void_p, sz, sz, ctypes.c_int,
                                                          ctypes.c_void_p]
lib.meshopt_encodeFilterExp.argtypes = [ctypes.c_void_p, sz, sz, ctypes.c_int, ctypes.c_void_p,
                                        ctypes.c_int]


def idx_bytes(a, size):
    """Decoded indices at `size` bytes: the binding decodes into a u32 array whatever the
    size, so 16-bit indices are packed in its first half."""
    return np.ascontiguousarray(a).view(np.uint8).tobytes()[:len(a) * size]


def vcache(ib, verts):
    ib = np.ascontiguousarray(ib, np.uint32)
    out = np.zeros_like(ib)
    mo.optimize_vertex_cache(out, ib, len(ib), verts)
    return out


def encode_vertex(data, count, stride, level, version):
    data = np.ascontiguousarray(data).view(np.uint8)
    cap = lib.meshopt_encodeVertexBufferBound(count, stride)
    buf = ctypes.create_string_buffer(cap)
    n = lib.meshopt_encodeVertexBufferLevel(buf, cap, data.ctypes.data, count, stride, level,
                                            version)
    assert n > 0
    return buf.raw[:n]


def encode_filter(kind, floats, count, stride, bits, mode=0):
    out = np.zeros(count * stride, np.uint8)
    f = np.ascontiguousarray(floats, np.float32)
    fn = getattr(lib, "meshopt_encodeFilter" + kind)
    if kind == "Exp":
        fn(out.ctypes.data, count, stride, bits, f.ctypes.data, mode)
    else:
        fn(out.ctypes.data, count, stride, bits, f.ctypes.data)
    return out


def color_reference(raw, stride):
    """KHR_meshopt_compression Appendix B, filter 4, as written."""
    n = 8 if stride == 4 else 16
    dt = np.uint8 if n == 8 else np.uint16
    v = raw.view(dt).reshape(-1, 4).astype(np.int64)
    out = np.zeros_like(v)
    umax = (1 << n) - 1
    for i, (y, co, cg, a) in enumerate(v):
        msb = int(a).bit_length() - 1
        sc = max((1 << (msb + 1)) - 1, 1)
        co = co - (1 << n) if co >= 1 << (n - 1) else co
        cg = cg - (1 << n) if cg >= 1 << (n - 1) else cg
        r, g, b = y + co - cg, y + cg, y - co - cg
        al = int(a) & (sc >> 1)
        al = (al << 1) | (al & 1)
        ss = np.float32(umax) / np.float32(sc)
        for k, x in enumerate((r, g, b, al)):
            o = np.float32(x) * ss
            out[i, k] = min(max(int(np.sign(o) * np.floor(abs(o) + 0.5)), 0), umax)
    return out.astype(dt).tobytes()


rng = np.random.default_rng(20261025)
streams = []


def add(mode, flt, count, stride, enc, dec):
    streams.append((mode, flt, count, stride, enc, dec))


def smooth(count, stride):
    """Byte data with structure: slowly varying u16/f32 lanes, noise and constant bytes."""
    t = np.arange(count, dtype=np.float32)
    lanes = []
    for k in range(stride // 4):
        kind = k % 4
        if kind == 0:
            lanes.append((np.sin(t * 0.01 * (k + 1)) * 100).astype(np.float32).view(np.uint8))
        elif kind == 1:
            lanes.append((t * 3 + k).astype(np.uint32).view(np.uint8))
        elif kind == 2:
            lanes.append(rng.integers(0, 256, count * 4, dtype=np.uint8))
        else:
            lanes.append(np.full(count * 4, 7, np.uint8))
    return np.stack([l.reshape(count, 4) for l in lanes], 1).reshape(-1)


# Attributes, version 0 and 1, every level, odd counts and block boundaries.
for version in (0, 1):
    for level in ((2,) if version == 0 else (0, 1, 2, 3)):
        for count, stride in ((1, 4), (15, 4), (16, 8), (17, 12), (255, 16), (256, 4),
                              (1000, 32), (3000, 64), (700, 256), (5000, 20)):
            raw = smooth(count, stride)
            enc = encode_vertex(raw, count, stride, level, version)
            dec = mo.decode_vertex_buffer(count, stride, enc).view(np.uint8).tobytes()
            assert dec == raw.tobytes()
            add("ATTRIBUTES", "NONE", count, stride, enc, dec)

# Filters: input from meshoptimizer's encoders, oracle from its decoders.
for version in (0, 1):
    for count in (37, 1024):
        v = rng.normal(size=(count, 3)).astype(np.float32)
        v /= np.linalg.norm(v, axis=1, keepdims=True)
        n4 = np.concatenate([v, np.ones((count, 1), np.float32)], 1)
        for stride, bits in ((4, 8), (4, 5), (8, 16), (8, 12)):
            raw = encode_filter("Oct", n4, count, stride, bits)
            enc = encode_vertex(raw, count, stride, 2, version)
            d = mo.decode_vertex_buffer(count, stride, enc).view(np.uint8)
            d = mo.decode_filter_oct(d.copy(), count, stride).view(np.uint8).tobytes()
            add("ATTRIBUTES", "OCTAHEDRAL", count, stride, enc, d)
        q = rng.normal(size=(count, 4)).astype(np.float32)
        q /= np.linalg.norm(q, axis=1, keepdims=True)
        for bits in (16, 12, 8):
            raw = encode_filter("Quat", q, count, 8, bits)
            enc = encode_vertex(raw, count, 8, 2, version)
            d = mo.decode_vertex_buffer(count, 8, enc).view(np.uint8)
            d = mo.decode_filter_quat(d.copy(), count, 8).view(np.uint8).tobytes()
            add("ATTRIBUTES", "QUATERNION", count, 8, enc, d)
        f = (rng.normal(size=(count, 3)) * 1000).astype(np.float32)
        for bits, mode in ((15, 0), (20, 1), (24, 2), (10, 3)):
            raw = encode_filter("Exp", f, count, 12, bits, mode)
            enc = encode_vertex(raw, count, 12, 2, version)
            d = mo.decode_vertex_buffer(count, 12, enc).view(np.uint8)
            d = mo.decode_filter_exp(d.copy(), count, 12).view(np.uint8).tobytes()
            add("ATTRIBUTES", "EXPONENTIAL", count, 12, enc, d)
        for stride, k in ((4, 8), (4, 6), (8, 16), (8, 10)):
            n = 8 if stride == 4 else 16
            dt = np.uint8 if n == 8 else np.uint16
            m = (1 << (k - 1)) - 1
            y = rng.integers(0, 1 << (k - 1), count)
            co = rng.integers(-m // 2, m // 2 + 1, count)
            cg = rng.integers(-m // 2, m // 2 + 1, count)
            a = rng.integers(0, 1 << (k - 1), count) | (1 << (k - 1))
            raw = np.stack([y, co & ((1 << n) - 1), cg & ((1 << n) - 1), a], 1).astype(dt)
            raw = raw.view(np.uint8).reshape(-1)
            enc = encode_vertex(raw, count, stride, 2, 1)
            d = mo.decode_vertex_buffer(count, stride, enc).view(np.uint8)
            add("ATTRIBUTES", "COLOR", count, stride, enc, color_reference(d.copy(), stride))

# Triangles and index sequences, 16- and 32-bit.
for tris, verts in ((1, 3), (64, 40), (5000, 2600), (20000, 12000)):
    idx = rng.integers(0, verts, (tris, 3)).astype(np.uint32)
    grid = int(np.sqrt(verts))
    quads = [(y * grid + x, y * grid + x + 1, (y + 1) * grid + x)
             for y in range(grid - 1) for x in range(grid - 1)][:tris]
    for ib in (idx.reshape(-1), np.array(quads, np.uint32).reshape(-1)):
        if len(ib) == 0:
            continue
        ib = vcache(ib, verts) if len(ib) > 3 else ib
        ib = np.ascontiguousarray(ib, np.uint32)
        enc = mo.encode_index_buffer(ib, len(ib), verts)
        for size in (2, 4):
            dec = idx_bytes(mo.decode_index_buffer(len(ib), size, enc), size)
            add("TRIANGLES", "NONE", len(ib), size, enc, dec)
    seq = np.cumsum(rng.integers(-3, 9, tris * 3)).astype(np.int64)
    seq = np.ascontiguousarray(seq - seq.min(), np.uint32)
    enc = mo.encode_index_sequence(seq, len(seq), int(seq.max()) + 1)
    for size in (2, 4):
        dec = idx_bytes(mo.decode_index_sequence(len(seq), size, enc), size)
        add("INDICES", "NONE", len(seq), size, enc, dec)

os.makedirs(os.path.join(OUT, "streams"), exist_ok=True)
for i, (mode, flt, count, stride, enc, dec) in enumerate(streams):
    base = os.path.join(OUT, "streams", f"{i:03}")
    with open(base + ".json", "w") as f:
        json.dump({"mode": mode, "filter": flt, "count": count, "byteStride": stride}, f)
    open(base + ".bin", "wb").write(enc)
    open(base + ".out", "wb").write(dec)


# ----- Models ---------------------------------------------------------------------------------

def glb(doc, bin_):
    j = json.dumps(doc, separators=(",", ":")).encode()
    j += b" " * (-len(j) % 4)
    bin_ += b"\0" * (-len(bin_) % 4)
    body = struct.pack("<II", len(j), 0x4E4F534A) + j + struct.pack("<II", len(bin_), 0x004E4942) + bin_
    return struct.pack("<III", 0x46546C67, 2, 12 + len(body)) + body


def read_glb(data):
    jl = struct.unpack_from("<I", data, 12)[0]
    doc = json.loads(data[20:20 + jl])
    bl = struct.unpack_from("<I", data, 20 + jl)[0]
    return doc, data[28 + jl:28 + jl + bl]


def plain_twin(doc, bin_):
    """The same model with every compressed view replaced by the oracle's decoded bytes."""
    doc = json.loads(json.dumps(doc))
    out = bytearray()
    for v in doc.get("bufferViews", []):
        e = v.get("extensions", {})
        ext = e.get("KHR_meshopt_compression") or e.get("EXT_meshopt_compression")
        if ext is None:
            assert v.get("buffer", 0) == 0
            data = bin_[v.get("byteOffset", 0):v.get("byteOffset", 0) + v["byteLength"]]
        else:
            src = bin_[ext.get("byteOffset", 0):ext.get("byteOffset", 0) + ext["byteLength"]]
            n, s = ext["count"], ext["byteStride"]
            mode, flt = ext["mode"], ext.get("filter", "NONE")
            if mode == "ATTRIBUTES":
                d = mo.decode_vertex_buffer(n, s, src).view(np.uint8).copy()
                if flt == "OCTAHEDRAL":
                    d = mo.decode_filter_oct(d, n, s).view(np.uint8)
                elif flt == "QUATERNION":
                    d = mo.decode_filter_quat(d, n, s).view(np.uint8)
                elif flt == "EXPONENTIAL":
                    d = mo.decode_filter_exp(d, n, s).view(np.uint8)
                elif flt == "COLOR":
                    d = np.frombuffer(color_reference(d, s), np.uint8)
                data = d.tobytes()
            elif mode == "TRIANGLES":
                data = idx_bytes(mo.decode_index_buffer(n, s, src), s)
            else:
                data = idx_bytes(mo.decode_index_sequence(n, s, src), s)
            v.pop("extensions")
            if mode == "ATTRIBUTES":
                v["byteStride"] = s
        out += b"\0" * (-len(out) % 4)
        v["buffer"] = 0
        v["byteOffset"] = len(out)
        v["byteLength"] = len(data)
        out += data
    doc["buffers"] = [{"byteLength": len(out)}]
    for k in ("extensionsUsed", "extensionsRequired"):
        if k in doc:
            doc[k] = [x for x in doc[k] if "meshopt" not in x]
    return glb(doc, bytes(out))


def own_model(version, ext_name):
    """A torus: positions as f32 (exponential filter), normals octahedral, triangle codec."""
    R, r, nu, nv = 1.0, 0.35, 64, 32
    pos, nrm, tris = [], [], []
    for i in range(nu):
        for j in range(nv):
            u, w = 2 * np.pi * i / nu, 2 * np.pi * j / nv
            c = np.array([np.cos(u), np.sin(u), 0.0])
            pos.append((R + r * np.cos(w)) * c + [0, 0, r * np.sin(w)])
            nrm.append(np.cos(w) * c + [0, 0, np.sin(w)])
            a, b = i * nv + j, ((i + 1) % nu) * nv + j
            c2, d = i * nv + (j + 1) % nv, ((i + 1) % nu) * nv + (j + 1) % nv
            tris += [a, b, d, a, d, c2]
    count = len(pos)
    ib = vcache(np.array(tris, np.uint32), count)
    p_raw = encode_filter("Exp", np.array(pos, np.float32), count, 12, 18, 0)
    n4 = np.concatenate([np.array(nrm, np.float32), np.ones((count, 1), np.float32)], 1)
    n_raw = encode_filter("Oct", n4, count, 4, 8)
    p_enc = encode_vertex(p_raw, count, 12, 2, version)
    n_enc = encode_vertex(n_raw, count, 4, 2, version)
    i_enc = mo.encode_index_buffer(ib, len(ib), count)
    bin_ = bytearray()
    views = []
    for enc, n, s, mode, flt in ((p_enc, count, 12, "ATTRIBUTES", "EXPONENTIAL"),
                                 (n_enc, count, 4, "ATTRIBUTES", "OCTAHEDRAL"),
                                 (i_enc, len(ib), 4, "TRIANGLES", None)):
        bin_ += b"\0" * (-len(bin_) % 4)
        ext = {"buffer": 0, "byteOffset": len(bin_), "byteLength": len(enc), "byteStride": s,
               "mode": mode, "count": n}
        if flt:
            ext["filter"] = flt
        view = {"buffer": 1, "byteLength": n * s, "extensions": {ext_name: ext}}
        if mode == "ATTRIBUTES":
            view["byteStride"] = s
        views.append(view)
        bin_ += enc
    pmin = np.array(pos).min(0).tolist()
    pmax = np.array(pos).max(0).tolist()
    doc = {
        "asset": {"version": "2.0", "generator": "specular gen-meshopt.py"},
        "extensionsUsed": [ext_name], "extensionsRequired": [ext_name],
        "buffers": [{"byteLength": len(bin_)},
                    {"byteLength": 8 << 20, "extensions": {ext_name: {"fallback": True}}}],
        "bufferViews": views,
        "accessors": [
            {"bufferView": 0, "componentType": 5126, "count": count, "type": "VEC3",
             "min": pmin, "max": pmax},
            {"bufferView": 1, "componentType": 5120, "normalized": True, "count": count,
             "type": "VEC3"},
            {"bufferView": 2, "componentType": 5125, "count": len(ib), "type": "SCALAR"}],
        "meshes": [{"primitives": [{"attributes": {"POSITION": 0, "NORMAL": 1},
                                    "indices": 2}]}],
        "nodes": [{"mesh": 0}], "scenes": [{"nodes": [0]}], "scene": 0,
    }
    return glb(doc, bytes(bin_))


os.makedirs(os.path.join(OUT, "models"), exist_ok=True)
models = {
    "torus-ext-v0.glb": own_model(0, "EXT_meshopt_compression"),
    "torus-khr-v1.glb": own_model(1, "KHR_meshopt_compression"),
}
try:
    with urllib.request.urlopen(PIRATE, timeout=60) as r:
        models["pirate.glb"] = r.read()
except Exception as e:  # The rest of the corpus is still useful offline.
    print(f"gen-meshopt: pirate.glb not fetched: {e}", file=sys.stderr)
for name, data in models.items():
    open(os.path.join(OUT, "models", name), "wb").write(data)
    doc, bin_ = read_glb(data)
    open(os.path.join(OUT, "models", name.replace(".glb", ".plain.glb")), "wb").write(
        plain_twin(doc, bin_))
print(f"gen-meshopt: {len(streams)} streams, {len(models)} models in {OUT}")
