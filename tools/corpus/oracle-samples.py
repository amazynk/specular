"""Oracle samples: independent readers' output for the corpora of tools/corpus/fetch-oracle-samples.sh.

    python3 -I tools/corpus/oracle-samples.py SECTION DIR

DIR is the section's folder (files/ holds the inputs); the reference output goes to
DIR/expect/ and DIR/expect.tsv, which the crate's tests/oracle.rs reads. Run with the venv of
fetch-oracle-samples.sh (numpy, Pillow, OpenImageIO, xisf, npTDMS, flowio, olefile, pdr,
imagecodecs). Inputs are untrusted downloads: nothing in them is executed.
"""
import hashlib
import os
import struct
import sys

import numpy as np


def fnv(b):
    h = 0xCBF29CE484222325
    for x in b:
        h = ((h ^ x) * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return h


def sha(b):
    return hashlib.sha256(b).hexdigest()[:16]


def files(d):
    src = os.path.join(d, "files")
    return sorted(f for f in os.listdir(src) if not f.startswith("."))


# --- Radiance RGBE / PFM: OpenImageIO ------------------------------------------------------
def radiance(d):
    import OpenImageIO as oiio
    rows = []
    for name in files(d):
        # PFM rows are stored bottom-up; OIIO returns them in file order unless asked to flip.
        config = oiio.ImageSpec()
        config.attribute("pnm:pfmflip", 1)
        inp = oiio.ImageInput.open(os.path.join(d, "files", name), config)
        if inp is None:
            rows.append([name, "error", oiio.geterror().replace("\t", " ")])
            continue
        spec = inp.spec()
        px = inp.read_image(0, 0, 0, spec.nchannels, "float")
        inp.close()
        a = np.ascontiguousarray(px, dtype="<f4")
        a.tofile(os.path.join(d, "expect", name + ".f32"))
        rows.append([name, spec.width, spec.height, spec.nchannels])
    return rows


# --- ICNS: Pillow's per-entry readers; rust-icns's PNGs for the types Pillow lacks -----------
ICNS_PNG = {"icm#.icns": "icm#.png", "icon.icns": "icon.png", "icsb.icns": "18x18.png"}


def icns(d):
    from PIL import IcnsImagePlugin, Image
    rows = []
    for name in files(d):
        if not name.endswith(".icns"):
            continue
        path = os.path.join(d, "files", name)
        if name in ICNS_PNG:
            ref = os.path.join(d, "files", ICNS_PNG[name])
            im = Image.open(ref).convert("RGBA")
            out = f"{name}.ref.rgba"
            open(os.path.join(d, "expect", out), "wb").write(im.tobytes())
            rows.append([name, "*", "rgba", im.width, im.height, out])
            continue
        with open(path, "rb") as f:
            icf = IcnsImagePlugin.IcnsFile(f)
            for size, readers in icf.SIZES.items():
                for code, reader in readers:
                    if code not in icf.dct or code.endswith(b"mk"):
                        continue
                    cc = code.decode("latin-1")
                    start, length = icf.dct[code]
                    if reader is IcnsImagePlugin.read_png_or_jpeg2000:
                        f.seek(start)
                        rows.append([name, cc, "embedded", start, length,
                                     sha(f.read(length))])
                        continue
                    try:
                        im = reader(f, icf.dct[code], size)["RGB"]
                    except Exception as e:  # noqa: BLE001 - recorded as the oracle's verdict
                        rows.append([name, cc, "error", str(e).replace("\t", " ")])
                        continue
                    mask = {b"is32": b"s8mk", b"il32": b"l8mk", b"ih32": b"h8mk",
                            b"it32": b"t8mk"}[code]
                    if mask in icf.dct:
                        a = IcnsImagePlugin.read_mk(f, icf.dct[mask], size)["A"]
                        im = im.copy()
                        im.putalpha(a)
                    im = im.convert("RGBA")
                    out = f"{name}.{cc}.rgba"
                    open(os.path.join(d, "expect", out), "wb").write(im.tobytes())
                    rows.append([name, cc, "rgba", im.width, im.height, out])
    return rows


# --- XISF: the xisf package (sergio-dr/xisf) -------------------------------------------------
def xisf(d):
    from xisf import XISF
    rows = []
    for name in files(d):
        try:
            x = XISF(os.path.join(d, "files", name))
            metas = x.get_images_metadata()
        except Exception as e:  # noqa: BLE001
            rows.append([name, "error", str(e).replace("\t", " ")])
            continue
        for i, m in enumerate(metas):
            who = "xisf"
            try:
                a = x.read_image(i, data_format="channels_first")
            except KeyError:
                # xisf 0.9.x cannot read location="embedded" blocks (_read_embedded_data_block
                # looks up a missing "value" key, then an undefined name): read them from the
                # spec here instead.
                a, who = xisf_embedded(os.path.join(d, "files", name), i), "spec"
            except Exception as e:  # noqa: BLE001
                rows.append([name, "error", f"image {i}: {type(e).__name__} {e}"])
                continue
            nkw = sum(len(v) for v in m.get("FITSKeywords", {}).values())
            out = f"{name}.{i}.f32"
            np.ascontiguousarray(a, dtype="<f4").tofile(os.path.join(d, "expect", out))
            w, h, ch = m["geometry"][:3]
            rows.append([name, i, w, h, ch, m["sampleFormat"], m.get("colorSpace", "Gray"),
                         nkw, out, who])
    return rows


XISF_DTYPES = {"UInt8": "u1", "UInt16": "u2", "UInt32": "u4", "UInt64": "u8",
               "Float32": "f4", "Float64": "f8"}


def xisf_embedded(path, index):
    """Image `index` of an XISF file whose pixels are an embedded <Data> element (XISF 1.0
    spec 9.3.3 and 9.4: base64/hex text, optional zlib/lz4/lz4hc/zstd with byte shuffling)."""
    import base64
    import xml.etree.ElementTree as ET
    import zlib
    raw = open(path, "rb").read()
    hlen = struct.unpack_from("<I", raw, 8)[0]
    root = ET.fromstring(raw[16:16 + hlen])
    ns = "{http://www.pixinsight.com/xisf}"
    img = root.findall(f"{ns}Image")[index]
    w, h, ch = (int(v) for v in img.get("geometry").split(":"))
    data = img.find(f"{ns}Data")
    blob = (base64.b16decode(data.text.strip().upper()) if data.get("encoding") == "hex"
            else base64.b64decode(data.text))
    if data.get("compression"):
        parts = data.get("compression").split(":")
        codec, size = parts[0], int(parts[1])
        if codec.startswith("zlib"):
            blob = zlib.decompress(blob)
        elif codec.startswith("lz4"):
            import imagecodecs
            blob = imagecodecs.lz4_decode(blob, header=False, out=size)
        elif codec.startswith("zstd"):
            import imagecodecs
            blob = imagecodecs.zstd_decode(blob)
        if codec.endswith("+sh"):
            item = int(parts[2])
            n = size // item
            body = np.frombuffer(blob[:n * item], "u1").reshape(item, n).T.reshape(-1)
            blob = body.tobytes() + blob[n * item:]
    dt = np.dtype(XISF_DTYPES[img.get("sampleFormat")]).newbyteorder(
        ">" if img.get("byteOrder") == "big" else "<")
    a = np.frombuffer(blob, dt, w * h * ch)
    if img.get("pixelStorage", "Planar") == "Normal":
        return a.reshape(h, w, ch).transpose(2, 0, 1)
    return a.reshape(ch, h, w)


# --- TDMS: npTDMS, dumped by specular-tdms's own gen/make_samples.py --------------------------
def tdms(d):
    import json
    root = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
    sys.path.insert(0, os.path.join(root, "crates/formats/specular-tdms/gen"))
    sys.dont_write_bytecode = True  # no __pycache__ in the source tree
    import make_samples
    rows = []
    for name in files(d):
        try:
            e = make_samples.expected(os.path.join(d, "files", name))
        except Exception as ex:  # noqa: BLE001
            rows.append([name, "error", f"{type(ex).__name__} {ex}".replace("\t", " ")])
            continue
        with open(os.path.join(d, "expect", name + ".json"), "w") as fh:
            json.dump(e, fh, separators=(",", ":"), ensure_ascii=False)
        rows.append([name, "ok", name + ".json"])
    return rows


# --- FCS: FlowIO ----------------------------------------------------------------------------
def stats_cols(a, picks):
    """Per column: sum, min, max and the values at `picks` (comma-separated, %.9g)."""
    out = []
    for c in range(a.shape[1]):
        col = a[:, c].astype(np.float64)
        vals = ",".join("%.9g" % col[i] for i in picks)
        out.append("%.17g:%.9g:%.9g:%s" % (col.sum(), col.min(), col.max(), vals))
    return ";".join(out)


def fcs_spill(text, names):
    """The spillover matrix and the parameter indices it applies to, from $SPILLOVER, SPILL or
    $SPILL (FlowIO lower-cases keys and drops the '$')."""
    for key in ("spillover", "spill"):
        if key in text:
            f = [x.strip() for x in text[key].split(",")]
            n = int(f[0])
            labels = f[1:1 + n]
            if len(f) < 1 + n + n * n:
                return None
            m = np.array([float(x) for x in f[1 + n:1 + n + n * n]]).reshape(n, n)
            idx = []
            for lab in labels:
                if lab in names:
                    idx.append(names.index(lab))
                elif lab.isdigit():
                    idx.append(int(lab) - 1)
                else:
                    return None
            return idx, m
    return None


def fcs_open(path):
    """FlowIO's datasets of a file, and the leniency it needed: offsets that are off by one
    (FlowIO's ignore_offset_error) or HEADER/TEXT disagreements (ignore_offset_discrepancy)."""
    import flowio
    for note, kw in (("", {}), ("ignore_offset_error", {"ignore_offset_error": True}),
                     ("ignore_offset_discrepancy", {"ignore_offset_discrepancy": True})):
        try:
            return [flowio.FlowData(path, **kw)], note
        except flowio.exceptions.MultipleDataSetsError:
            return flowio.read_multiple_data_sets(path, **kw), note
        except flowio.exceptions.FCSParsingError:
            continue
    return flowio.FlowData(path), ""  # raises the strict error


def fcs(d):
    rows = []
    for name in files(d):
        path = os.path.join(d, "files", name)
        try:
            sets, note = fcs_open(path)
        except Exception as e:  # noqa: BLE001
            rows.append([name, 0, "error", f"{type(e).__name__} {e}".replace("\t", " ")])
            continue
        for k, fd in enumerate(sets):
            try:
                raw = fd.as_array(preprocess=False)
            except Exception as e:  # noqa: BLE001
                rows.append([name, k, "error", f"{type(e).__name__} {e}".replace("\t", " ")])
                continue
            try:
                scaled = fd.as_array(preprocess=True)
            except ValueError:
                scaled = None  # e.g. $TIMESTEP 'xxxxxxxxx': FlowIO refuses to scale
            names = [fd.channels[c]["pnn"] for c in sorted(fd.channels, key=int)]
            n = raw.shape[0]
            picks = sorted({0, n // 7, n // 3, n // 2, (2 * n) // 3, n - 1}) if n else []
            sp = fcs_spill(fd.text, names) if scaled is not None else None
            comp = "-"
            if sp:
                idx, m = sp
                c = scaled.copy()
                c[:, idx] = scaled[:, idx] @ np.linalg.inv(m)
                comp = stats_cols(c, picks)
            rows.append([name, k, n, fd.channel_count, " ".join(map(str, picks)),
                         stats_cols(raw, picks),
                         stats_cols(scaled, picks) if scaled is not None else "-", comp, note])
    return rows


# --- CFB: olefile -----------------------------------------------------------------------------
def cfb(d):
    import olefile
    rows = []
    for name in files(d):
        try:
            ole = olefile.OleFileIO(os.path.join(d, "files", name))
            entries = ole.listdir(streams=True, storages=True)
            for parts in entries:
                p = "/".join(parts)
                if ole.get_type(parts) == olefile.STGTY_STORAGE:
                    rows.append([name, "storage", p, 0, "-"])
                else:
                    b = ole.openstream(parts).read()
                    declared = ole.get_size(parts)
                    rows.append([name, "stream", p, len(b), "%016x" % fnv(b), declared])
            ole.close()
        except Exception as e:  # noqa: BLE001 - olefile's verdict on a damaged file
            rows = [r for r in rows if r[0] != name]
            rows.append([name, "error", f"{type(e).__name__} {e}".replace("\t", " ")])
    return rows


# --- PDS3 / PDS4: pdr; VICAR: rms-vicar ----------------------------------------------------
def planetary_write(d, rows, name, key, a):
    """Rows of (bands, lines, samples) float32 values; complex as two planes, real then
    imaginary per band."""
    a = np.ma.filled(np.ma.asarray(a).astype(np.complex128 if np.iscomplexobj(a)
                                               else np.float64), np.nan)
    if a.ndim == 2:
        a = a[None]
    kind = "real"
    if np.iscomplexobj(a):
        a, kind = np.stack([a.real, a.imag], axis=1).reshape(-1, *a.shape[1:]), "complex"
    out = f"{name}.{key}.f32"
    np.ascontiguousarray(a, dtype="<f4").tofile(os.path.join(d, "expect", out))
    rows.append([name, key, a.shape[0], a.shape[1], a.shape[2], kind, out])


def planetary(d):
    import warnings
    warnings.simplefilter("ignore")
    rows = []
    for name in files(d):
        path = os.path.join(d, "files", name)
        head = open(path, "rb").read(8)
        try:
            if head.startswith(b"LBLSIZE"):
                import vicar
                im = vicar.VicarImage(path)
                a = im.data_3d
                org = im["ORG"]
                # data_3d is in storage order: BSQ (b, l, s), BIL (l, b, s), BIP (l, s, b).
                a = {"BSQ": a, "BIL": a.transpose(1, 0, 2), "BIP": a.transpose(2, 0, 1)}[org]
                planetary_write(d, rows, name, "VICAR", a)
            elif name.lower().endswith((".lbl", ".xml")):
                import pdr
                data = pdr.read(path)
                for key in data.keys():
                    v = data[key]
                    if isinstance(v, np.ndarray) and v.ndim in (2, 3) and v.dtype.kind in "uifc":
                        try:
                            v = data.get_scaled(key)
                        except Exception:  # noqa: BLE001 - no scaling keywords
                            pass
                        planetary_write(d, rows, name, key, v)
        except Exception as e:  # noqa: BLE001
            rows.append([name, "error", f"{type(e).__name__} {e}".replace("\t", " ")[:300]])
    # GDAL's expected Checksum() of band 1, from its autotest (read as text, never run).
    import re
    gdal = os.path.join(d, "files", "gdal-vicar-autotest.py")
    if os.path.exists(gdal):
        for m in re.finditer(r'\("(vicar_\w+)", gdal\.GDT_\w+, (\d+)\)', open(gdal).read()):
            rows.append([m.group(1) + ".vic", "gdal", m.group(2)])
    return rows


# --- SER: FFmpeg's ser demuxer for the frames; the header and trailer read here from the spec --
def ser(d):
    import subprocess
    rows = []
    for name in files(d):
        path = os.path.join(d, "files", name)
        raw = open(path, "rb").read()
        # Grischa Hahn, SER format description v3: 14-byte FileID, then little-endian int32
        # LuID, ColorID, LittleEndian, ImageWidth, ImageHeight, PixelDepthPerPlane,
        # FrameCount; three 40-byte strings; int64 DateTime and DateTime_UTC; then the frames
        # and an optional trailer of one int64 UTC tick count per frame.
        _, colour, flag, w, h, depth, n = struct.unpack_from("<7i", raw, 14)
        planes = 3 if colour in (100, 101) else 1
        frame = w * h * planes * (2 if depth > 8 else 1)
        trailer = raw[178 + n * frame:]
        ts = struct.unpack_from(f"<{n}q", trailer) if len(trailer) >= 8 * n else ()
        pix = subprocess.run(["ffprobe", "-v", "error", "-select_streams", "v:0",
                              "-show_entries", "stream=pix_fmt", "-of", "csv=p=0", path],
                             capture_output=True, text=True, check=True).stdout.strip()
        out = subprocess.run(["ffmpeg", "-v", "error", "-i", path, "-f", "rawvideo",
                              "-pix_fmt", pix, "-"], capture_output=True, check=True).stdout
        dt = "<u2" if depth > 8 else "u1"
        a = np.frombuffer(out, dt).reshape(-1, h * w * planes)
        ts_text = f"{ts[0]},{ts[-1]}" if ts else "-"
        rows.append([name, "header", colour, w, h, depth, n, a.shape[0], ts_text, pix, flag])
        for i, f in enumerate(a):
            f64 = f.astype(np.uint64)
            # A position-weighted sum: catches swapped or shifted samples that the plain sum
            # does not (weights 1..65521, repeating).
            weights = (np.arange(f.size, dtype=np.uint64) % np.uint64(65521)) + np.uint64(1)
            rows.append([name, i, int(f64.sum()), int(f.min()), int(f.max()),
                         int((f64 * weights).sum())])
    return rows


# --- AEC: libaec through imagecodecs -----------------------------------------------------------
def aec_params(name):
    """(reference .dat, bits, block, rsi, flags) of a CCSDS test stream, as libaec's
    tests/sampledata.sh codes it (flags: libaec's: 8 preprocess, 16 restricted, 32 pad RSI)."""
    import re
    flags = 8 | (16 if "-restricted" in name else 0)
    m = re.match(r"test_p(\d+)n(\d+)", name)
    if m:
        rsi = 16 if m.group(1) == "256" else 32
        return f"test_p{m.group(1)}n{m.group(2)}.dat", int(m.group(2)), 16, rsi, flags
    m = re.match(r"(Lowset\d_8bit)\.n(\d+)", name)
    if m:
        return m.group(1) + ".dat", int(m.group(2)), 16, 64, flags
    m = re.match(r"sar32bit\.j(\d+)\.r(\d+)", name)
    if m:
        return "sar32bit.dat", 32, int(m.group(1)), int(m.group(2)), flags | 32
    raise ValueError(name)


def aec(d):
    import imagecodecs
    rows = []
    for name in files(d):
        if not name.endswith(".rz"):
            continue
        dat, bits, block, rsi, flags = aec_params(name)
        width = 1 if bits <= 8 else 2 if bits <= 16 else 4
        size = os.path.getsize(os.path.join(d, "files", dat))
        data = open(os.path.join(d, "files", name), "rb").read()
        try:
            out = imagecodecs.aec_decode(data, bitspersample=bits, flags=flags,
                                         blocksize=block, rsi=rsi, out=size)
        except Exception as e:  # noqa: BLE001
            rows.append([name, "error", f"{type(e).__name__} {e}".replace("\t", " ")])
            continue
        out = bytes(out)
        ref = open(os.path.join(d, "files", dat), "rb").read()
        # libaec's own test compares the decoded prefix of the reference's length.
        agrees = out[:len(ref)] == ref
        open(os.path.join(d, "expect", name + ".out"), "wb").write(out)
        rows.append([name, dat, bits, block, rsi, flags, size // width,
                     "agrees" if agrees else "differs", name + ".out"])
    return rows


# --- CorelDRAW: the first preview, found by a RIFF walk / zipfile here, decoded by Pillow -----
def cdr_first(d):
    """(kind, payload bytes) of the preview shown first: a ZIP package's lowest page preview
    (previews/pageN.png, metadata/thumbnails/pageN.bmp), else its thumbnail; a RIFF file's
    DISP chunk (a 4-byte clipboard format, then a DIB) or version 3's imhd BMP."""
    import io
    import re
    import zipfile
    if d[:4] == b"PK\x03\x04":
        z = zipfile.ZipFile(io.BytesIO(d))
        pages, thumb = [], None
        for name in z.namelist():
            m = re.fullmatch(r"(?:previews/(page(\d+)|thumbnail)\.png|"
                             r"metadata/thumbnails/(page(\d+)|thumbnail)\.bmp)", name)
            if not m:
                continue
            num = m.group(2) or m.group(4)
            if num is not None:
                pages.append((int(num), name))
            else:
                thumb = name
        pages.sort()
        name = pages[0][1] if pages else thumb
        return (name.rsplit(".", 1)[1], z.read(name)) if name else None
    if d[:4] != b"RIFF":
        return None
    end = min(len(d), struct.unpack_from("<I", d, 4)[0] + 8)
    stack = [(12, end, 0)]
    while stack:
        off, end, depth = stack.pop()
        while off + 8 <= end:
            cid = d[off:off + 4]
            n = struct.unpack_from("<I", d, off + 4)[0]
            body = d[off + 8:off + 8 + n]
            if cid == b"DISP":
                return "dib", body[4:]
            if cid == b"imhd" and body[:2] == b"BM":
                return "bmp", body
            if cid == b"LIST" and depth < 2:
                stack.append((off + 12, min(off + 8 + n, end), depth + 1))
            off += 8 + n + (n & 1)
    return None


def cdr(d):
    import io
    from PIL import BmpImagePlugin, Image
    rows = []
    for name in files(d):
        data = open(os.path.join(d, "files", name), "rb").read()
        try:
            first = cdr_first(data)
            if first is None:
                rows.append([name, "none"])
                continue
            kind, payload = first
            im = (BmpImagePlugin.DibImageFile(io.BytesIO(payload)) if kind == "dib"
                  else Image.open(io.BytesIO(payload)))
            im.load()
            rows.append([name, kind, len(payload), "%016x" % fnv(payload), im.width, im.height])
        except Exception as e:  # noqa: BLE001
            rows.append([name, "error", f"{type(e).__name__} {e}".replace("\t", " ")])
    return rows


# --- Clip Studio Paint: Python's sqlite3 and Pillow -----------------------------------------
def clip(d):
    import io
    import sqlite3
    from PIL import Image
    rows = []
    for name in files(d):
        data = open(os.path.join(d, "files", name), "rb").read()
        # CSFCHUNK, file size and first chunk offset (big-endian u64), then chunks of an
        # 8-byte tag and a big-endian u64 length.
        off = struct.unpack_from(">Q", data, 16)[0]
        db = None
        while off + 16 <= len(data):
            tag, n = data[off:off + 8], struct.unpack_from(">Q", data, off + 8)[0]
            if tag == b"CHNKSQLi":
                db = data[off + 16:off + 16 + n]
            off += 16 + n
        con = sqlite3.connect(":memory:")
        con.deserialize(db)
        w, h = con.execute("SELECT CanvasWidth, CanvasHeight FROM Canvas").fetchone()
        blob = con.execute("SELECT ImageData FROM CanvasPreview").fetchone()[0]
        im = Image.open(io.BytesIO(blob))
        im.load()
        names = [r[0] or "" for r in con.execute(
            "SELECT LayerName FROM Layer WHERE LayerType != 256 ORDER BY _PW_ID")]
        rows.append([name, w, h, len(blob), "%016x" % fnv(blob), im.width, im.height,
                     "|".join(names)])
    return rows


# --- Spectra contours: scikit-image's marching squares ---------------------------------------
def spectra_grids(d):
    """Synthetic 2-D spectra (seeded): Gaussian and Lorentzian cross peaks of both signs on
    noise, at sizes and aspect ratios like 2-D NMR, as row-major float32 files."""
    src = os.path.join(d, "files")
    os.makedirs(src, exist_ok=True)
    for k, (w, h, peaks, noise) in enumerate([(64, 48, 3, 0.002), (128, 96, 12, 0.01),
                                               (256, 128, 30, 0.02), (200, 200, 8, 0.05),
                                               (97, 61, 5, 0.0), (300, 40, 20, 0.01)]):
        rng = np.random.default_rng(1000 + k)
        yy, xx = np.mgrid[0:h, 0:w].astype(np.float64)
        z = rng.normal(0, noise, (h, w))
        for _ in range(peaks):
            cx, cy = rng.uniform(0, w), rng.uniform(0, h)
            sx, sy = rng.uniform(1, w / 8), rng.uniform(1, h / 8)
            a = rng.uniform(0.2, 1.0) * rng.choice([1, 1, 1, -1])
            if rng.random() < 0.5:
                z += a * np.exp(-((xx - cx) ** 2 / (2 * sx ** 2) + (yy - cy) ** 2 / (2 * sy ** 2)))
            else:
                z += a / (1 + ((xx - cx) / sx) ** 2) / (1 + ((yy - cy) / sy) ** 2)
        z.astype("<f4").tofile(os.path.join(src, f"grid{k}_{w}x{h}.f32"))


def spectra(d):
    from skimage import measure
    spectra_grids(d)
    rows = []
    for name in files(d):
        w, h =(int(v) for v in name.split("_")[1][:-4].split("x"))
        z = np.fromfile(os.path.join(d, "files", name), "<f4").reshape(h, w).astype(np.float64)
        peak = np.abs(z).max()
        # Levels of the oracle's own choosing: 2 % to 90 % of the peak, both signs.
        for lv in [s * peak * f for s in (1, -1) for f in (0.02, 0.05, 0.1, 0.2, 0.4, 0.9)]:
            # Ours marks corners with value >= level; find_contours draws at the level by
            # linear interpolation along cell edges, as ours does. Saddle cells: ours joins
            # the corners at or above the level (find_contours' fully_connected="high"; its
            # default "low" pairs them the other way, a few % of length on noisy grids).
            length, segments = 0.0, 0
            for c in measure.find_contours(z, lv, fully_connected="high"):
                seg = np.diff(c, axis=0)
                length += float(np.hypot(seg[:, 0], seg[:, 1]).sum())
                segments += len(seg)
            rows.append([name, w, h, "%.17g" % lv, "%.17g" % length, segments])
    return rows


# --- IFF colour cycling: FFmpeg's ILBM decoder; the cycling from the chunks' definitions -----
def iff_ranges(data):
    """CMAP, active CRNG/CCRT ranges as (regs, reverse, num, den) per 1/60 s tick, and
    whether a DRNG is present. CRNG (Deluxe Paint): rate / 16384 steps a tick, flag 1
    active, 2 backwards; CCRT (Graphicraft): one step every seconds + microseconds,
    direction 1 forwards, -1 backwards."""
    i, cmap, ranges, drng = 12, b"", [], False
    while i + 8 <= len(data):
        tag, n = data[i:i + 4], struct.unpack_from(">I", data, i + 4)[0]
        b = data[i + 8:i + 8 + n]
        if tag == b"CMAP":
            cmap = b
        elif tag == b"CRNG" and len(b) >= 8:
            _, rate, flags, lo, hi = struct.unpack(">hhhBB", b[:8])
            if flags & 1 and not flags & ~3 and rate > 0 and lo < hi < len(cmap) // 3:
                ranges.append((list(range(lo, hi + 1)), bool(flags & 2), rate, 16384))
        elif tag == b"CCRT" and len(b) >= 12:
            direction, lo, hi, secs, micro = struct.unpack(">hBBii", b[:12])
            us = secs * 1_000_000 + micro
            if direction and us > 0 and lo < hi < len(cmap) // 3:
                # One step every `us` microseconds: 1e6/60 µs a tick.
                ranges.append((list(range(lo, hi + 1)), direction < 0, 1_000_000, 60 * us))
        elif tag == b"DRNG":
            drng = True
        i += 8 + n + (n & 1)
    return cmap, ranges, drng


def iff(d):
    import subprocess
    rows = []
    for name in files(d):
        path = os.path.join(d, "files", name)
        data = open(path, "rb").read()
        w, h = struct.unpack_from(">HH", data, data.index(b"BMHD") + 8)
        rgb = subprocess.run(["ffmpeg", "-v", "error", "-i", path, "-frames:v", "1", "-f",
                              "rawvideo", "-pix_fmt", "rgb24", "-"],
                             capture_output=True, check=True).stdout
        open(os.path.join(d, "expect", name + ".rgb"), "wb").write(rgb)
        cmap, ranges, drng = iff_ranges(data)
        # An OCS-era CMAP (4-bit values in the high nibble): FFmpeg uses the bytes as they
        # are; Amiga hardware (and ours) shows 0xA0 as 0xAA.
        ocs = int(all(v & 15 == 0 for v in cmap) and any(cmap))
        rows.append([name, "image", w, h, name + ".rgb", ocs])
        if drng or not ranges:
            rows.append([name, "cycling", "drng" if drng else "none"])
            continue
        # Every tick on which a range steps is a frame: (tick, FNV-1a of the palette).
        base = [cmap[3 * k:3 * k + 3] for k in range(min(256, len(cmap) // 3))]
        events, last = [], [0] * len(ranges)
        for tick in range(1, 60 * 60 * 20 + 1):
            steps = [tick * num // den for (_, _, num, den) in ranges]
            if steps == last:
                continue
            last = steps
            pal = list(base)
            for (regs, reverse, _, _), s in zip(ranges, steps):
                n = len(regs)
                for j, reg in enumerate(regs):
                    # Forwards, the colour of register j moves up s places (wrapping).
                    src = (j + s) % n if reverse else (j - s) % n
                    pal[reg] = base[regs[src]]
            events.append(f"{tick}:{fnv(b''.join(pal)):016x}")
            if len(events) >= 2048:
                break
        rows.append([name, "cycling", fnv(cmap[:3 * len(base)]), " ".join(events)])
    return rows


# --- TIM / XWD variants: ImageMagick, in fetch-raster-legacy2.sh's layout ------------------
def xwd2(path, msb, unit, visual, bpp=2):
    """A `bpp`-bit (default 2) ZPixmap XWD (X11 XWDFileHeader, version 7) of a 4-entry colour
    map: a 37 x 23 pattern using all four values, rows padded to `unit` bits. The header and colour map are
    big-endian, as xwd writes them; `msb` sets the image's byte and bit order."""
    w, h, name = 37, 23, b"oracle-samples\0"
    pad = unit
    bpl = (w * bpp + pad - 1) // pad * pad // 8
    e = ">"
    head = struct.pack(e + "25I", 100 + len(name), 7, 2, bpp, w, h, 0, int(msb), unit, int(msb),
                       pad, bpp, bpl, visual, 0, 0, 0, 8, 4, 4, w, h, 0, 0, 0)
    cols = [(0, 0, 0), (0xFFFF, 0x8000, 0), (0x2000, 0x9000, 0xF000), (0xFFFF, 0xFFFF, 0xFFFF)]
    if visual in (0, 1):
        cols = [(v, v, v) for v in (0, 0x5555, 0xAAAA, 0xFFFF)]
    cmap = b"".join(struct.pack(e + "IHHHBB", k, r, g, b, 7, 0) for k, (r, g, b) in enumerate(cols))
    body = bytearray()
    truth = bytearray()
    for y in range(h):
        row = bytearray(bpl)
        for x in range(w):
            v = (x // 3 + y // 2 + (x * y) % 3) % 4
            truth += struct.pack(">4H", *cols[v], 0xFFFF)
            bitpos = x * bpp
            byte = bitpos // 8
            shift = 8 - bpp - bitpos % 8 if msb else bitpos % 8
            row[byte] |= v << shift
        body += row
    open(path, "wb").write(head + name + cmap + bytes(body))
    return bytes(truth)


def tim_mixed(path):
    """A PlayStation TIM in mode 4 (mixed: a frame-buffer dump with no single pixel format),
    and its frame-buffer words as 15-bit colours (bits 0-4 red, 5-9 green, 10-14 blue, each
    widened as (v << 3) | (v >> 2)) in 16-bit RGBA."""
    w16, h = 24, 10
    words = [(x * 1311 + y * 977) & 0xFFFF for y in range(h) for x in range(w16)]
    data = b"".join(struct.pack("<H", v) for v in words)
    block = struct.pack("<IHHHH", 12 + len(data), 0, 0, w16, h) + data
    open(path, "wb").write(struct.pack("<II", 0x10, 4) + block)
    c = lambda v: ((v << 3) | (v >> 2)) * 257  # noqa: E731
    return b"".join(struct.pack(">4H", c(v & 31), c(v >> 5 & 31), c(v >> 10 & 31), 0xFFFF)
                    for v in words)


def legacy2(d):
    import subprocess
    src = os.path.join(d, "files")
    # What the writers here meant, for files ImageMagick misreads: 2-bit ZPixmap ("invalid
    # colormap index", while the same writer's 4- and 8-bit files read), grey visuals deeper
    # than 1 bit ("improper image header"), mixed-mode TIM (read as 4-bit CLUT indices, 96x10).
    truth = {
        "gen-2bit-pseudo-msb8.xwd": xwd2(os.path.join(src, "gen-2bit-pseudo-msb8.xwd"), True, 8, 3),
        "gen-2bit-pseudo-msb32.xwd": xwd2(os.path.join(src, "gen-2bit-pseudo-msb32.xwd"), True,
                                          32, 3),
        "gen-2bit-pseudo-lsb8.xwd": xwd2(os.path.join(src, "gen-2bit-pseudo-lsb8.xwd"), False,
                                         8, 3),
        "gen-2bit-grey-msb8.xwd": xwd2(os.path.join(src, "gen-2bit-grey-msb8.xwd"), True, 8, 1),
        "gen-mixed.tim": tim_mixed(os.path.join(src, "gen-mixed.tim")),
    }
    # Controls: the same writer at 4 bits, which ImageMagick reads.
    xwd2(os.path.join(src, "gen-4bit-pseudo-msb8.xwd"), True, 8, 3, 4)
    xwd2(os.path.join(src, "gen-4bit-pseudo-lsb8.xwd"), False, 8, 3, 4)
    rows = []
    for name in files(d):
        path = os.path.join(src, name)
        if name in truth:
            open(os.path.join(d, "expect", name + ".pages"), "w").write("1\n")
            open(os.path.join(d, "expect", name + ".0.rgba"), "wb").write(truth[name])
            rows.append([name, 1, "writer"])
            continue
        ids = subprocess.run(["magick", "identify", "-format", "%p\n", path],
                             capture_output=True, text=True).stdout.split()
        open(os.path.join(d, "expect", name + ".pages"), "w").write(f"{len(ids)}\n")
        for p in range(len(ids)):
            subprocess.run(["magick", f"{path}[{p}]", "-depth", "16", "-endian", "MSB",
                            f"RGBA:{os.path.join(d, 'expect', f'{name}.{p}.rgba')}"],
                           capture_output=True)
        rows.append([name, len(ids), "ImageMagick"])
    return rows


# --- Luma-only DPX: ImageMagick and OpenImageIO, in fetch-raster-film.sh's layout ---------
def dpx_luma_truth(path):
    """The code values of a one-element luma DPX (descriptor 6, packing 1: filled to 32-bit
    words, method A) read from the raw words per SMPTE 268M, scaled to 16 bits; each line
    takes (image data length / height) bytes, the padding the writer actually left. None for
    anything else. ImageMagick and OpenImageIO disagree on these files (see film tests)."""
    b = open(path, "rb").read()
    e = ">" if b[:4] == b"SDPX" else "<"
    off = struct.unpack(e + "I", b[4:8])[0]
    nel, w, h = struct.unpack(e + "HII", b[770:780])
    desc, bits = b[800], b[803]
    packing = struct.unpack(e + "H", b[804:806])[0]
    if nel != 1 or desc != 6 or packing != 1 or bits not in (8, 10, 12, 16):
        return None
    stride = (len(b) - off) // h
    out = np.zeros((h, w), np.uint32)
    for y in range(h):
        row = b[off + y * stride:off + (y + 1) * stride]
        if bits == 8:
            words = np.frombuffer(row[:(len(row) // 4) * 4], e + "u4")
            v = np.stack([(words >> s) & 255 for s in ((24, 16, 8, 0) if e == ">" else
                                                       (0, 8, 16, 24))], 1).reshape(-1)
        elif bits == 16:
            v = np.frombuffer(row[:(len(row) // 2) * 2], e + "u2")
        elif bits == 10:
            # A single component's samples fill a word from its low end (bits 2-11 first),
            # as libdpx (OpenImageIO), ImageMagick and ours all read them; three-component
            # pixels put R at the top instead.
            words = np.frombuffer(row[:(len(row) // 4) * 4], e + "u4")
            v = np.stack([(words >> s) & 1023 for s in (2, 12, 22)], 1).reshape(-1)
        else:  # 12 bits, method A: each in a 16-bit unit, top-aligned
            v = np.frombuffer(row[:(len(row) // 2) * 2], e + "u2") >> 4
        if len(v) < w:
            return None  # lines not padded to whole words: no single reading
        out[y] = v[:w]
    top = (1 << bits) - 1
    return (out * 65535 + top // 2) // top
def film(d):
    import gzip
    import subprocess
    import OpenImageIO as oiio
    src = os.path.join(d, "files")
    # Luma DPX (descriptor 6) as OpenImageIO and ImageMagick write it, at every depth.
    yy, xx = np.mgrid[0:37, 0:61]
    grey = ((xx * 1063 + yy * 1777) % 4096 / 4095.0).astype(np.float32)
    for bits, fmt in ((8, "uint8"), (10, "uint10"), (12, "uint12"), (16, "uint16")):
        spec = oiio.ImageSpec(61, 37, 1, fmt)
        spec.attribute("oiio:BitsPerSample", bits)
        out = oiio.ImageOutput.create(os.path.join(src, f"gen-oiio-luma{bits}.dpx"))
        out.open(os.path.join(src, f"gen-oiio-luma{bits}.dpx"), spec)
        out.write_image(grey[:, :, None])
        out.close()
    for bits in (8, 10, 12, 16):
        subprocess.run(["magick", "-size", "61x37", "gradient:black-white", "-colorspace", "Gray",
                        "-depth", str(bits), os.path.join(src, f"gen-magick-luma{bits}.dpx")],
                       check=True)
    root = os.path.dirname(os.path.abspath(__file__))
    rows = []
    for name in files(d):
        path = os.path.join(src, name)
        e = os.path.join(d, "expect", name)
        truth = dpx_luma_truth(path)
        if truth is not None:
            h, w = truth.shape
            with gzip.open(e + ".truth.pam.gz", "wb", compresslevel=1) as f:
                f.write(f"P7\nWIDTH {w}\nHEIGHT {h}\nDEPTH 1\nMAXVAL 65535\nTUPLTYPE GRAYSCALE\n"
                        "ENDHDR\n".encode() + truth.astype(">u2").tobytes())
        # Stored values (no colour conversion), as fetch-raster-film.sh takes them.
        pam = subprocess.run(["magick", path + "[0]", "-set", "colorspace", "sRGB", "-depth",
                              "16", "PAM:-"], capture_output=True).stdout
        if pam:
            with gzip.open(e + ".magick.pam.gz", "wb", compresslevel=1) as f:
                f.write(pam)
        r = subprocess.run([sys.executable, "-I", os.path.join(root, "raster-film-oiio.py"), path,
                            e + ".oiio.pam"], capture_output=True)
        if r.returncode == 0 and os.path.exists(e + ".oiio.pam"):
            with open(e + ".oiio.pam", "rb") as f, gzip.open(e + ".oiio.pam.gz", "wb", 1) as g:
                g.write(f.read())
            os.remove(e + ".oiio.pam")
        rows.append([name, int(bool(pam)), r.returncode])
    return rows


SECTIONS = {"radiance": radiance, "icns": icns, "xisf": xisf, "tdms": tdms, "fcs": fcs,
            "cfb": cfb, "planetary": planetary, "ser": ser, "aec": aec, "cdr": cdr,
            "clip": clip, "spectra": spectra, "iff": iff, "legacy2": legacy2, "film": film}


def main():
    section, d = sys.argv[1], sys.argv[2]
    os.makedirs(os.path.join(d, "expect"), exist_ok=True)
    rows = SECTIONS[section](d)
    with open(os.path.join(d, "expect.tsv"), "w") as f:
        for r in rows:
            f.write("\t".join(str(x) for x in r) + "\n")
    print(f"oracle-samples {section}: {len(rows)} rows")


if __name__ == "__main__":
    main()
