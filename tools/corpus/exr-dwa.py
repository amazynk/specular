# OpenEXR (Python bindings, 3.x) generator for the specular-exr DWA and deep conformance
# corpus (see exr-dwa.sh, which runs this in its own venv).
#
# Usage: python exr-dwa.py CONFORMANCE_DIR [--bench BENCH_DIR]
#
# Writes into CONFORMANCE_DIR/exr-dwa/:
# * dwa/<name>.exr: openexr-images files (part 0) and synthetic images re-encoded as DWAA and
#   DWAB at a few dwaCompressionLevel values (scanline and tiled, odd sizes, layers, HALF /
#   FLOAT / UINT mixes, R/G/B, Y/RY/BY, plain names, A, pLinear, subsampled chroma), each with
#   <name>.oracle: the pixels OpenEXR itself decodes from that file.
# * deep/<dir>_<file>.p<part>.flat: a flattened oracle for each part of the deep files in
#   openexr-images/v2, and deep/syn_*.exr synthetic deep files (scanline NONE / RLE / ZIPS,
#   tiled, offset data window, UINT, no A) with deep/syn_*.p<part>.flat. Flattening: samples
#   sorted by Z (stable), composited front to back with "over" using A, in f32 (HALF channels
#   rounded to half at the end); without A the front-most sample. Z, ZBack and UINT channels
#   take the front-most sample. Pixels without samples are zero.
#
# .oracle / .flat format (little-endian):
#   b"XEXR" | u32 width | u32 height | u32 nchan | per channel: u8 name length, name bytes,
#   u8 type (0 UINT, 1 HALF, 2 FLOAT), u8 x sampling, u8 y sampling | then per channel, its
#   sampled pixels (ceil-free: OpenEXR's sampled size) as f32 (UINT as f32 of the value).
#
# With --bench, also writes the 4k kloofendal file (tools/corpus/large/exr) as DWAA, DWAB,
# PIZ and ZIP into BENCH_DIR for speed measurements.
import os
import struct
import sys

import numpy as np
import OpenEXR

DWA = [("dwaa", OpenEXR.DWAA_COMPRESSION), ("dwab", OpenEXR.DWAB_COMPRESSION)]
TYPES = {OpenEXR.UINT: 0, OpenEXR.HALF: 1, OpenEXR.FLOAT: 2}


def type_code(px):
    if px.dtype == np.uint32:
        return 0
    if px.dtype == np.float16:
        return 1
    return 2


def write_oracle(path, width, height, chans):
    """chans: list of (name, type code, xs, ys, 2-D array)."""
    with open(path, "wb") as f:
        f.write(b"XEXR" + struct.pack("<III", width, height, len(chans)))
        for name, ty, xs, ys, _ in chans:
            n = name.encode()
            f.write(struct.pack("<B", len(n)) + n + struct.pack("<BBB", ty, xs, ys))
        for _, _, _, _, px in chans:
            f.write(np.ascontiguousarray(px, dtype="<f4").tobytes())


def oracle_of(path, out):
    f = OpenEXR.File(path, separate_channels=True)
    h = f.header()
    (x0, y0), (x1, y1) = h["dataWindow"]
    chans = []
    for name in sorted(f.channels()):
        c = f.channels()[name]
        chans.append((name, type_code(c.pixels), c.xSampling, c.ySampling, c.pixels))
    write_oracle(out, int(x1 - x0 + 1), int(y1 - y0 + 1), chans)


def write(path, header, chans):
    with OpenEXR.File(header, chans) as f:
        f.write(path)


def encode_all(out_dir, stem, chans, levels, tiles=None, extra=None):
    for tag, comp in DWA:
        for lvl in levels:
            h = {
                "compression": comp,
                "dwaCompressionLevel": float(lvl),
                "type": OpenEXR.tiledimage if tiles else OpenEXR.scanlineimage,
            }
            if tiles:
                h["tiles"] = tiles
            if extra:
                h.update(extra)
            name = f"{stem}.{tag}.l{int(lvl)}"
            p = os.path.join(out_dir, name + ".exr")
            write(p, h, chans)
            oracle_of(p, os.path.join(out_dir, name + ".oracle"))
            print("wrote", name)


def tile_desc(x, y, mode=OpenEXR.ONE_LEVEL):
    t = OpenEXR.TileDescription()
    t.xSize, t.ySize, t.mode, t.roundingMode = x, y, mode, OpenEXR.ROUND_DOWN
    return t


def synthetic(w, h, seed):
    rng = np.random.default_rng(seed)
    y, x = np.mgrid[0:h, 0:w].astype(np.float32)
    smooth = 0.5 + 0.45 * np.sin(x / 7.0 + seed) * np.cos(y / 5.0)
    edges = ((x // 9 + y // 11) % 2).astype(np.float32) * 3.0
    noise = rng.random((h, w), dtype=np.float32) * 0.2
    v = smooth + edges * (seed % 3 == 0) + noise
    v[(x.astype(int) * 7 + y.astype(int)) % 53 == 0] *= -1.0
    return v.astype(np.float32)


def specials(w, h):
    v = synthetic(w, h, 9).astype(np.float32) * 40000.0
    flat = v.reshape(-1)
    flat[::97] = np.inf
    flat[5::131] = -np.inf
    flat[11::173] = np.nan
    flat[13::59] = 6e-6  # denormal halves
    flat[17::61] = -0.0
    return v


def source_channels(path):
    f = OpenEXR.File(path, separate_channels=True)
    (x0, y0), (x1, y1) = f.parts[0].header["dataWindow"]
    w, h = int(x1 - x0 + 1), int(y1 - y0 + 1)
    out = {}
    for n, c in f.parts[0].channels.items():
        # Reading gives subsampled channels at their sampled size; writing wants full size.
        px = np.repeat(np.repeat(c.pixels, c.ySampling, 0), c.xSampling, 1)[:h, :w]
        out[n] = OpenEXR.Channel(np.ascontiguousarray(px), c.xSampling, c.ySampling, c.pLinear)
    return out


def main():
    conf = sys.argv[1]
    bench = sys.argv[sys.argv.index("--bench") + 1] if "--bench" in sys.argv else None
    src = os.path.join(conf, "openexr-images")
    out = os.path.join(conf, "exr-dwa", "dwa")
    deep_out = os.path.join(conf, "exr-dwa", "deep")
    os.makedirs(out, exist_ok=True)
    os.makedirs(deep_out, exist_ok=True)

    H, F, U = np.float16, np.float32, np.uint32
    w, h = 37, 45
    s = [synthetic(w, h, k) for k in range(12)]

    encode_all(out, "syn_rgba", {"R": s[0].astype(H), "G": s[1].astype(H),
                                 "B": s[2].astype(H), "A": (s[3] * 0.5).astype(H)},
               [5, 45, 300])
    encode_all(out, "syn_mixed", {
        "R": s[0].astype(H), "G": s[1].astype(F), "B": s[2].astype(H),
        "A": s[3].astype(F),
        "diffuse.R": s[4].astype(H), "diffuse.G": s[5].astype(H), "diffuse.B": s[6].astype(H),
        "spec.r": s[7].astype(H), "spec.g": s[8].astype(H), "spec.blue": s[9].astype(H),
        "Z": (s[10] * 100).astype(F), "id": (np.abs(s[11]) * 1000).astype(U),
        "mask": s[11].astype(H), "half.A": s[5].astype(H),
        "partial.R": s[6].astype(H), "partial.G": s[7].astype(H),
    }, [45])
    encode_all(out, "syn_yc", {"Y": s[0].astype(H), "BY": (s[1] - 0.5).astype(H),
                               "RY": (s[2] - 0.5).astype(H)}, [45])
    encode_all(out, "syn_float", {"R": s[0], "G": s[1], "B": s[2]}, [45])
    encode_all(out, "syn_plinear", {
        "R": OpenEXR.Channel(s[0].astype(H), 1, 1, True),
        "G": OpenEXR.Channel(s[1].astype(H), 1, 1, True),
        "B": OpenEXR.Channel(s[2].astype(H), 1, 1, False),
        "Y": OpenEXR.Channel(s[3].astype(H), 1, 1, True)}, [45])
    sp = specials(61, 33)
    encode_all(out, "syn_specials", {"R": sp.astype(H), "G": (sp * -0.001).astype(H),
                                     "B": (sp * 1e-7).astype(H), "A": sp.astype(H)}, [45])
    encode_all(out, "syn_one", {"R": np.full((1, 1), 0.25, H)}, [45])
    encode_all(out, "syn_thin", {"Y": synthetic(300, 3, 4).astype(H)}, [45])
    ws, hs = 38, 46  # chroma subsampled 2x2 (even data window)
    sub = synthetic(ws, hs, 7)  # the binding takes full-size arrays for subsampled channels
    encode_all(out, "syn_sub", {
        "Y": OpenEXR.Channel(synthetic(ws, hs, 6).astype(H), 1, 1),
        "RY": OpenEXR.Channel((sub - 0.5).astype(H), 2, 2),
        "BY": OpenEXR.Channel((sub * 0.3).astype(H), 2, 2)}, [45])
    t = synthetic(150, 97, 3)
    encode_all(out, "syn_tiled", {"R": t.astype(H), "G": (t * 0.5).astype(H),
                                  "B": (t * 0.25).astype(H), "A": (t > 1).astype(H)},
               [45], tiles=tile_desc(64, 32))

    for rel, levels, tiles in [
        ("ScanLines/Desk.exr", [45, 150], None),
        ("ScanLines/Cannon.exr", [45], None),
        ("MultiView/Fog.exr", [45], None),
        ("MultiView/Adjuster.exr", [45], None),
        ("TestImages/AllHalfValues.exr", [45], None),
        ("TestImages/BrightRingsNanInf.exr", [45], None),
        ("TestImages/WideFloatRange.exr", [45], None),
        ("LuminanceChroma/Garden.exr", [45], None),
        ("Tiles/Ocean.exr", [45], tile_desc(128, 64)),
        ("Chromaticities/Rec709_YC.exr", [45], None),
    ]:
        stem = "img_" + os.path.splitext(rel.replace("/", "_"))[0]
        encode_all(out, stem, source_channels(os.path.join(src, rel)), levels, tiles)

    for d in ["Stereo", "LowResLeftView", "LeftView"]:
        for n in ["Balls", "Ground", "Leaves", "Trunks"]:
            p = os.path.join(src, "v2", d, n + ".exr")
            flatten(p, os.path.join(deep_out, f"{d}_{n}"))

    deep_synthetic(deep_out)

    if bench:
        os.makedirs(bench, exist_ok=True)
        big = os.path.join(conf, "..", "large", "exr", "kloofendal_48d_partly_cloudy_puresky_4k.exr")
        chans = source_channels(big)
        half = {k: OpenEXR.Channel(v.pixels.astype(np.float16)) for k, v in chans.items()}
        for tag, comp in DWA + [("piz", OpenEXR.PIZ_COMPRESSION), ("zip", OpenEXR.ZIP_COMPRESSION)]:
            for pt, c in [("float", chans), ("half", half)]:
                h = {"compression": comp, "type": OpenEXR.scanlineimage}
                write(os.path.join(bench, f"kloofendal_{pt}.{tag}.exr"), h, c)
                print("bench", tag, pt)


def deep_samples(w, h, seed, maxn, types):
    rng = np.random.default_rng(seed)
    chans = {n: np.empty((h, w), dtype=object) for n in types}
    for y in range(h):
        for x in range(w):
            n = int(rng.integers(0, maxn + 1))
            for name, ty in types.items():
                if name == "Z":
                    v = rng.random(n) * 10.0
                    if n > 1 and rng.random() < 0.2:
                        v[1] = v[0]  # a tie keeps file order
                elif name == "A":
                    v = rng.random(n) * 0.9 + 0.05
                elif ty == np.uint32:
                    v = rng.integers(0, 1 << 20, n)
                else:
                    v = rng.random(n) * 2.0 - 0.2
                chans[name][y, x] = v.astype(ty)
    return chans


def deep_synthetic(out):
    H, F, U = np.float16, np.float32, np.uint32
    rgbaz = {"A": H, "B": H, "G": H, "R": H, "Z": F, "id": U}
    cases = [
        ("syn_scan_none", OpenEXR.deepscanline, OpenEXR.NO_COMPRESSION, None, 13, 7),
        ("syn_scan_rle", OpenEXR.deepscanline, OpenEXR.RLE_COMPRESSION, None, 19, 11),
        ("syn_scan_zips", OpenEXR.deepscanline, OpenEXR.ZIPS_COMPRESSION, None, 23, 9),
        ("syn_tile_zips", OpenEXR.deeptile, OpenEXR.ZIPS_COMPRESSION, tile_desc(8, 4), 29, 13),
        ("syn_tile_rle", OpenEXR.deeptile, OpenEXR.RLE_COMPRESSION, tile_desc(5, 3), 17, 10),
    ]
    for k, (name, ty, comp, tiles, w, h) in enumerate(cases):
        hdr = {"type": ty, "compression": comp,
               "dataWindow": ((3, -2), (3 + w - 1, -2 + h - 1))}
        if tiles:
            hdr["tiles"] = tiles
        p = os.path.join(out, name + ".exr")
        write(p, hdr, deep_samples(w, h, 100 + k, 4, rgbaz))
        flatten(p, os.path.join(out, name))
    # Without A: the front-most sample.
    p = os.path.join(out, "syn_noalpha.exr")
    write(p, {"type": OpenEXR.deepscanline, "compression": OpenEXR.ZIPS_COMPRESSION},
          deep_samples(11, 6, 200, 3, {"R": H, "Z": F}))
    flatten(p, os.path.join(out, "syn_noalpha"))


def flatten(path, stem):
    f = OpenEXR.File(path, separate_channels=True)
    for pi, part in enumerate(f.parts):
        (x0, y0), (x1, y1) = part.header["dataWindow"]
        w, h = int(x1 - x0 + 1), int(y1 - y0 + 1)
        names = sorted(part.channels)
        cols = {n: part.channels[n].pixels.reshape(-1) for n in names}
        # UINT samples go through f32 like everything else (exact below 2^24).
        types = {n: TYPES[part.channels[n].type()] for n in names}
        npx = w * h
        # Per-pixel sample counts, then all samples concatenated in pixel order.
        first = cols[names[0]]
        counts = np.array([0 if v is None else len(v) for v in first], dtype=np.int64)
        flat = {}
        for n in names:
            parts = [v for v in cols[n] if v is not None]
            flat[n] = (np.concatenate(parts).astype(np.float32) if parts
                       else np.zeros(0, np.float32))
        pix = np.repeat(np.arange(npx), counts)
        z = flat["Z"] if "Z" in flat else np.zeros(len(pix), np.float32)
        order = np.lexsort((z, pix))  # by pixel, then Z; stable for ties
        for n in names:
            flat[n] = flat[n][order]
        starts = np.concatenate([[0], np.cumsum(counts)[:-1]])
        rank = np.arange(len(pix)) - np.repeat(starts, counts)
        pix = pix[order]
        out = {n: np.zeros(npx, np.float32) for n in names}
        has_a = "A" in flat
        maxc = int(counts.max()) if npx else 0
        if has_a:
            acc_a = np.zeros(npx, np.float32)
            front = [n for n in names if n in ("Z", "ZBack") or types[n] == 0]
            colour = [n for n in names if n != "A" and n not in front]
            for k in range(maxc):
                sel = rank == k
                p = pix[sel]
                t = np.float32(1.0) - acc_a[p]
                for n in colour:
                    out[n][p] = out[n][p] + t * flat[n][sel]
                acc_a[p] = acc_a[p] + t * flat["A"][sel]
                if k == 0:
                    for n in front:
                        out[n][p] = flat[n][sel]
            out["A"] = acc_a
        else:
            sel = rank == 0
            for n in names:
                out[n][pix[sel]] = flat[n][sel]
        chans = []
        for n in names:
            v = out[n].reshape(h, w)
            if types[n] == 1:
                v = v.astype(np.float16).astype(np.float32)
            chans.append((n, types[n], 1, 1, v))
        write_oracle(f"{stem}.p{pi}.flat", w, h, chans)
        print("flattened", os.path.basename(stem), "part", pi, "samples", len(pix))


if __name__ == "__main__":
    main()
