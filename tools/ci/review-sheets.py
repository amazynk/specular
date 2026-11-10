#!/usr/bin/env python3
"""Visual review sheets: what our decoders actually show, per format and variant.

    tools/ci/review-sheets.sh [--long PX] [--per-format N] [--jobs N] [--only GROUP,...]
                              [--root DIR]... [--out DIR] [--no-probe-cache]

The wrapper makes the venv ($CARGO_TARGET_DIR/venv-review: numpy, pillow, rawpy, pydicom,
imagecodecs, tifffile, pillow-heif) and builds the `review_decode` example of specular-convert,
then runs this script with that venv's python. Run it like any cargo command (CARGO_TARGET_DIR
and friends set). Over the fixtures a rerun takes well under a minute (about 700 files, three
jobs); fetched corpora add their files' decode time. `--only raster,video` limits the groups.

What it does:
1. Collects candidate files from the test fixtures (crates/**/tests, crates/**/fixtures,
   app/**/tests/data) and the fetched corpora (tools/corpus/* of this checkout and of the
   main checkout, target/*/corpus), plus any --root. tools/corpus/real (private photos) is
   never entered.
2. Probes them through the registry (`review_decode probe`: format, coding, bit depth, size,
   orientation, alpha, ICC, pages) and keeps one file per format and variant (coding, bits,
   alpha, orientation, ICC, animation), at most --per-format variants per format.
3. Decodes each through the viewer's path (`review_decode decode`: the registry's session,
   ICC to sRGB, HDR tone-mapped, orientation applied, long side at most --long).
4. Decodes the same file with a reference decoder where one exists (the oracle runners':
   Pillow, LibRaw through rawpy, libheif, libjxl/OpenJPEG/CharLS/jxrlib through imagecodecs,
   pydicom, ffmpeg for movies and float images, ImageMagick, pdftoppm, inkscape), brought to
   sRGB the same way (embedded ICC applied with LittleCMS, EXIF orientation, alpha kept).
5. Writes, per format group, PNG sheets into OUT (default target/review):
   `<group>-oracle-N.png` rows of ours | reference | difference x8, labelled with file,
   format, variant, size and max/mean difference (0-255 over RGB on grey, and alpha);
   `<group>-grid-N.png` a labelled grid of the files without a reference (or whose
   reference failed); plus `index.html` (every row, full-size images under img/) and
   `summary.tsv`.

Reference notes: HDR/float references are clipped, not tone-mapped (expect a difference in
highlights); RAW references are LibRaw's defaults (camera white balance, auto brightness), so
a tone difference is expected and only casts, orientation, crops and garbage count; DICOM
references use the file's first VOI window (else min/max). Differences are a prompt to look,
not a verdict: the oracle tests decide pass/fail.
"""

import argparse
import concurrent.futures as cf
import hashlib
import html
import io
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

import numpy as np
from PIL import Image, ImageCms, ImageDraw, ImageFont, ImageOps

ROOT = Path(__file__).resolve().parents[2]

# Never data: sources, sidecars, expected-output dumps, audio.
SKIP_EXT = set(
    """rs py sh md txt tsv csv json lock md5 framemd5 frames info expect log readme toml yml yaml
    yuv rgba f32 f64 npy npz pcm s16 s32 u8 wav flac mp2 mp3 ac3 eac3 aac opus mka m4a ogg_audio
    zattrs zarray zgroup zmetadata lut c h cpp patch diff html xmp sha256 sig gitignore""".split()
)
MAX_BYTES = 300 << 20
PER_EXT_PROBE = 80  # candidates probed per extension (smallest first, then spread)

RAW_EXT = set(
    """cr2 cr3 crw nef nrw arw srf sr2 raf orf rw2 rwl pef dng dcr kdc k25 mrw x3f 3fr fff iiq
    erf mef mos srw gpr raw cap eip ari bay""".split()
)
VIDEO_EXT = set(
    """mp4 m4v mov mkv webm ivf ogv avi 3gp 3g2 mxf ts mts m2ts m2t m2v m1v mpv mpg mpeg vob h263
    y4m divx wmv asf flv dv""".split()
)

# Format groups: keywords matched against our format name (lower case), then extensions.
GROUPS = [
    # Our camera formats are named "Camera RAW", "DNG", "GoPro GPR", ...; ".raw" is also a
    # volume's detached data, so that extension alone does not make a camera file.
    ("raw", "Camera RAW", ["camera raw", "dng", "gopro", "gpr", "sigma x3f"],
     RAW_EXT - {"raw"}),
    ("modern", "Modern codecs (JPEG XL, AVIF, HEIC, WebP, JPEG 2000, JPEG XS, JPEG XR, BPG, "
               "FLIF, JPEG-LS)",
     ["jpeg xl", "avif", "heic", "heif", "webp", "jpeg 2000", "jpeg xs", "jpeg xr", "bpg",
      "flif", "jpeg-ls", "htj2k", "jpx"],
     set("jxl avif heic heif heics hif webp jp2 j2k j2c jph jpx jhc jxs jxsi jxr wdp hdp bpg "
         "flif jls".split())),
    ("video", "Video first frames", ["mpeg", "matroska", "webm", "quicktime", "avi", "ivf",
                                     "ogg", "mxf", "3gpp", "video", "h.26", "vp8", "vp9",
                                     "theora", "prores", "dv"], VIDEO_EXT),
    ("hdr", "HDR / float", ["openexr", "radiance", "pfm", "float"], set("exr hdr pfm pic".split())),
    ("medical", "Medical", ["dicom", "nifti", "nrrd", "minc", "mgh", "analyze", "metaimage",
                            "gifti", "tract"],
     set("dcm nii mgh mgz nrrd nhdr mnc mha mhd gii trk tck img hdr_analyze".split())),
    ("science", "Science / microscopy / astro / spectra",
     ["fits", "xisf", "ser", "microscop", "vicar", "pds", "planetary", "isis", "flow", "zeiss",
      "nikon", "x-tek", "volume graphics", "leica", "olympus vsi", "imaris",
      "mrc", "em ", "electron", "digital micrograph", "spm", "afm", "nanoscope", "gwyddion",
      "hdf", "netcdf", "grib", "tdms", "segy", "seg-y", "jcamp", "nmr", "mzml", "imzml",
      "spectr", "crystal", "cif", "diffraction", "ebsd", "asdf", "root", "casa", "fcs", "rti",
      "surface", "x3p", "thermal", "flir", "array", "ics", "lsm", "czi", "slide", "wsi"],
     set("fits fit fts xisf ser nd2 lif lof czi lsm ics ids mrc mrcs dm3 dm4 emi ser sxm spm "
         "ibw gwy hdf h5 he5 nc grib grib1 grib2 grb tdms sgy segy jdx dx spc mzml mzxml imzml "
         "fid cif cbf ang ctf asdf root fcs rti ptm x3p sur plu datx vk4 al3d mdt jpk irb irg "
         "edf mccd sfrm txm txrm xrm am vol ccp4 map svs ndpi vms mrxs scn bif isyntax "
         "stk".split())),
    ("documents", "Documents / vector", ["pdf", "svg", "epub", "djvu", "xps", "postscript",
                                         "opendocument", "visio", "coreldraw", "wpg", "cgm",
                                         "sketch", "clip studio", "font", "truetype", "opentype",
                                         "woff", "metafile", "wmf", "emf", "drawing"],
     set("pdf svg svgz epub djvu djv xps oxps ps eps odg odp ods odt vsd vsdx cdr wpg cgm "
         "sketch clip ttf otf ttc woff woff2 sfnt otc wmf emf".split())),
    ("cad", "CAD / boards", ["dxf", "dwg", "hpgl", "gerber", "excellon", "kicad", "gds",
                             "oasis", "step", "iges", "ifc", "pcb", "eagle"],
     set("dxf dwg plt hpgl gbr ger drl gbrjob kicad_pcb kicad_sch brd sch gds oas step stp iges "
         "igs ifc".split())),
    ("3d", "3D / point clouds (rendered views)",
     ["stl", "ply", "obj", "gltf", "glb", "fbx", "3mf", "x3d", "vtk", "las", "laz", "e57",
      "point", "mesh", "splat", "pcd", "xyz", "molecul", "pdb", "mol", "usd", "collada"],
     set("stl ply obj gltf glb fbx 3mf x3d vtk vtp vtu vts vtr vti las laz e57 pcd xyz pts "
         "splat pdb mol sdf mol2 xyz dae usdz".split())),
    ("maps", "Maps / GIS", ["geotiff", "shapefile", "geojson", "geopackage", "kml", "gpx",
                            "esri", "dem", "terrain", "raster map", "mbtiles", "pmtiles", "osm"],
     set("shp geojson gpkg kml kmz gpx asc dem bil mbtiles pmtiles osm pbf tif_geo".split())),
    ("raster", "TIFF / film / legacy raster", [], set()),  # everything else that decodes
]
GROUP_TITLE = {g[0]: g[1] for g in GROUPS}


def group_of(fmt, ext, coding):
    f = fmt.lower()
    for gid, _, keys, exts in GROUPS:
        if any(re.search(r"(^|\W)" + re.escape(k), f) for k in keys):
            return gid
    for gid, _, keys, exts in GROUPS:
        if ext in exts:
            return gid
    if "geo" in coding.lower():
        return "maps"
    return "raster"


# ---------------------------------------------------------------------------------- candidates

def main_checkout():
    try:
        common = subprocess.run(["git", "-C", str(ROOT), "rev-parse", "--git-common-dir"],
                                capture_output=True, text=True, check=True).stdout.strip()
        p = Path(common)
        if not p.is_absolute():
            p = ROOT / p
        return p.resolve().parent
    except (subprocess.CalledProcessError, OSError):
        return ROOT


def walk(root, out):
    root = Path(root)
    if not root.exists():
        return
    for dirpath, dirnames, filenames in os.walk(root):
        d = Path(dirpath)
        # The user's private photos: never read.
        dirnames[:] = [n for n in dirnames
                       if not (n == "real" and d.name == "corpus" and d.parent.name == "tools")
                       and n not in ("src", "fuzz", ".git", "node_modules", "target")]
        for n in filenames:
            p = d / n
            if "/tools/corpus/real/" in str(p.resolve()) + "/":
                continue
            ext = n.rsplit(".", 1)[-1].lower() if "." in n else ""
            if ext in SKIP_EXT:
                continue
            try:
                st = p.stat()
            except OSError:
                continue
            if 16 <= st.st_size <= MAX_BYTES:
                out.append((p, ext, st.st_size))


def candidates(extra_roots):
    files = []
    roots = []
    # This checkout first: its fixtures win over the main checkout's copies.
    for base in [ROOT] + [b for b in [main_checkout()] if b != ROOT]:
        for pat in ("crates/*/tests", "crates/*/fixtures", "crates/formats/*/tests",
                    "crates/formats/*/fixtures", "app/*/tests/data"):
            roots += sorted(base.glob(pat))
        corpus = base / "tools" / "corpus"
        if corpus.is_dir():
            roots += sorted(p for p in corpus.iterdir() if p.is_dir() and p.name != "real")
        if base == main_checkout():
            roots += sorted((base / "target").glob("*/corpus"))
    roots += [Path(r) for r in extra_roots]
    seen = set()
    for r in roots:
        rr = r.resolve()
        if rr in seen or "tools/corpus/real" in str(rr):
            continue
        seen.add(rr)
        walk(r, files)
    # Same content found twice (worktree and main checkout): keep one.
    uniq = {}
    for p, ext, size in files:
        uniq.setdefault((p.name, size), (p, ext, size))
    by_ext = {}
    for p, ext, size in uniq.values():
        by_ext.setdefault(ext, []).append((size, str(p)))
    picked = []
    for ext, lst in by_ext.items():
        lst.sort()
        if len(lst) > PER_EXT_PROBE:
            step = len(lst) / PER_EXT_PROBE
            lst = [lst[int(i * step)] for i in range(PER_EXT_PROBE)]
        picked += [p for _, p in lst]
    return sorted(picked)


# ------------------------------------------------------------------------------------- ours

FIELDS = ["path", "status", "format", "coding", "bits", "width", "height", "dw", "dh",
          "orientation", "alpha", "icc", "pages", "animated", "ms"]


def parse_line(line):
    parts = line.rstrip("\n").split("\t")
    parts += [""] * (len(FIELDS) - len(parts))
    return dict(zip(FIELDS, parts))


def run_probe(exe, paths, timeout=120):
    try:
        r = subprocess.run([exe, "probe", *paths], capture_output=True, text=True,
                           timeout=timeout, errors="replace")
        rows = [parse_line(l) for l in r.stdout.splitlines() if "\t" in l]
        if len(rows) == len(paths):
            return rows
    except subprocess.TimeoutExpired:
        pass
    if len(paths) == 1:
        return [dict(path=paths[0], status="err:probe crashed or timed out")]
    out = []
    for p in paths:
        out += run_probe(exe, [p], timeout=60)
    return out


def probe_all(exe, paths, jobs, cache_file, use_cache):
    cache = {}
    if use_cache and cache_file.exists():
        for l in cache_file.read_text().splitlines():
            row = json.loads(l)
            cache[row["key"]] = row
    def key(p):
        st = os.stat(p)
        return f"{p}|{st.st_size}|{int(st.st_mtime)}"
    todo = [p for p in paths if key(p) not in cache]
    batches = [todo[i:i + 40] for i in range(0, len(todo), 40)]
    with cf.ThreadPoolExecutor(jobs) as ex:
        for rows in ex.map(lambda b: run_probe(exe, b), batches):
            for row in rows:
                row["key"] = key(row["path"])
                cache[row["key"]] = row
    cache_file.parent.mkdir(parents=True, exist_ok=True)
    with open(cache_file, "w") as f:
        for row in cache.values():
            f.write(json.dumps(row) + "\n")
    return [cache[key(p)] for p in paths]


def variant_key(r):
    # Sizes and counts inside the coding text do not make a variant; codec names stay.
    keep = {"2000", "263", "264", "265", "601", "709", "2020", "2100"}
    coding = re.sub(r"\d{3,}", lambda m: m[0] if m[0] in keep else "N", r.get("coding", ""))
    bits = []
    if r.get("alpha") == "1":
        bits.append("alpha")
    if r.get("orientation") not in ("", "0", "1"):
        bits.append(f"orient{r['orientation']}")
    if r.get("icc") == "1":
        bits.append("icc")
    if r.get("animated") == "1":
        bits.append("anim")
    if r.get("pages") not in ("", "0", "1"):
        bits.append("pages")
    return " ".join(x for x in [coding, f"{r.get('bits')}-bit"] + bits if x)


def select(rows, per_format):
    buckets = {}
    for r in rows:
        if r["status"] != "ok":
            continue
        try:
            dw, dh = int(r["dw"]), int(r["dh"])
        except ValueError:
            continue
        if dw * dh == 0:
            continue
        ext = r["path"].rsplit(".", 1)[-1].lower()
        k = (r["format"], variant_key(r))
        # Prefer files of a viewable size (>= 64 px), then smaller files; a known reference
        # breaks ties.
        small = min(dw, dh) < 64
        score = (small, ext not in REF_EXTS, os.path.getsize(r["path"]))
        if k not in buckets or score < buckets[k][0]:
            buckets[k] = (score, r)
    by_fmt = {}
    for (fmt, _), (score, r) in sorted(buckets.items(), key=lambda kv: kv[1][0]):
        by_fmt.setdefault(fmt, []).append(r)
    out = []
    for fmt, lst in sorted(by_fmt.items()):
        out += lst[:per_format]
    return out


def decode_ours(exe, path, out_png, long, timeout=180):
    try:
        r = subprocess.run([exe, "decode", path, str(out_png), str(long)], capture_output=True,
                           text=True, timeout=timeout, errors="replace")
        rows = [parse_line(l) for l in r.stdout.splitlines() if "\t" in l]
        if rows:
            return rows[-1]
        tail = (r.stderr.strip().splitlines() or ["no output"])[-1][:200]
        return dict(path=path, status=f"err:crashed ({r.returncode}): {tail}")
    except subprocess.TimeoutExpired:
        return dict(path=path, status=f"err:timeout {timeout}s")


# -------------------------------------------------------------------------------- reference

PIL_EXT = set("png apng jpg jpeg jpe jfif bmp dib gif tga icb vda vst ppm pgm pbm pnm ico cur sgi "
              "rgb rgba_sgi bw qoi dds pcx xpm xbm im webp psd tif tiff icns msp blp dcx".split())
HEIF_EXT = set("heic heif heics hif avif".split())
CODEC_EXT = set("jxl jp2 j2k j2c jph jpx jhc jls jxr wdp hdp".split())
MAGICK_EXT = set("dpx cin xwd miff pict pct wpg sun ras viff iff ilbm lbm pcd pix wbmp otb "
                 "mng jng sgi palm".split())
FLOAT_EXT = set("exr hdr pfm pic".split())
DOC_EXT = set("pdf svg svgz".split())
REF_EXTS = PIL_EXT | HEIF_EXT | CODEC_EXT | MAGICK_EXT | FLOAT_EXT | DOC_EXT | RAW_EXT | \
    VIDEO_EXT | {"dcm"}

SRGB = ImageCms.createProfile("sRGB")


def to_rgba8(a, bits=None):
    """numpy H x W [x C] of any dtype -> RGBA uint8 (straight alpha)."""
    a = np.asarray(a)
    if a.ndim == 2:
        a = a[..., None]
    if a.dtype.kind == "f":
        x = np.nan_to_num(a.astype(np.float32))
        c = x[..., :3] if x.shape[2] >= 3 else x[..., :1]
        c = np.clip(c, 0, 1)
        c = np.where(c <= 0.0031308, c * 12.92, 1.055 * np.power(c, 1 / 2.4) - 0.055)
        x = np.concatenate([c, np.clip(x[..., c.shape[2]:], 0, 1)], axis=2)
        a = np.round(x * 255).astype(np.uint8)
    elif a.dtype != np.uint8 or (bits and bits < 8 and a.max() < (1 << bits)):
        top = (1 << bits) - 1 if bits and bits <= 16 and a.max() <= (1 << bits) - 1 else \
            np.iinfo(a.dtype).max
        a = np.round(a.astype(np.float64) * 255 / max(top, 1)).clip(0, 255).astype(np.uint8)
    ch = a.shape[2]
    if ch == 1:
        a = np.repeat(a, 3, 2)
    elif ch == 2:
        a = np.concatenate([np.repeat(a[..., :1], 3, 2), a[..., 1:2]], 2)
    elif ch > 4:
        a = a[..., :4]
    if a.shape[2] == 3:
        a = np.concatenate([a, np.full(a.shape[:2] + (1,), 255, np.uint8)], 2)
    return np.ascontiguousarray(a)


def apply_icc(img, icc):
    if not icc:
        return img
    try:
        src = ImageCms.ImageCmsProfile(io.BytesIO(icc))
        mode = img.mode
        if mode in ("RGBA", "LA"):
            alpha = img.getchannel("A")
            base = img.convert("RGB" if mode == "RGBA" else "L")
        else:
            alpha, base = None, img
        if base.mode not in ("RGB", "L", "CMYK"):
            base = base.convert("RGB")
        out = ImageCms.profileToProfile(base, src, SRGB, outputMode="RGB",
                                        renderingIntent=ImageCms.Intent.PERCEPTUAL)
        if alpha is not None:
            out.putalpha(alpha)
        return out
    except Exception:  # noqa: BLE001 - a broken profile: show the samples as they are
        return img


def ref_pil(path, bits):
    img = Image.open(path)
    img.seek(0)
    icc = img.info.get("icc_profile")
    img = ImageOps.exif_transpose(img)
    if img.mode in ("I;16", "I;16B", "I;16L", "I"):
        a = np.array(img)
        return to_rgba8(a.astype(np.uint16 if a.max() <= 65535 else np.uint32), bits)
    if img.mode == "F":
        return to_rgba8(np.array(img))
    if img.mode == "P" or img.mode == "PA":
        img = img.convert("RGBA")
    if img.mode == "CMYK" and not icc:
        img = img.convert("RGB")
    img = apply_icc(img, icc)
    return to_rgba8(np.array(img.convert("RGBA")))


def ref_heif(path, bits):
    import pillow_heif
    pillow_heif.register_heif_opener()
    pillow_heif.register_avif_opener() if hasattr(pillow_heif, "register_avif_opener") else None
    return ref_pil(path, bits)


def ref_codec(path, bits, ext):
    import imagecodecs
    data = Path(path).read_bytes()
    if ext == "jxl":
        a = imagecodecs.jpegxl_decode(data)
    elif ext in ("jls",):
        a = imagecodecs.jpegls_decode(data)
    elif ext in ("jxr", "wdp", "hdp"):
        a = imagecodecs.jpegxr_decode(data)
    else:
        a = imagecodecs.jpeg2k_decode(data)
    return to_rgba8(a, bits)


def ref_rawpy(path, bits):
    import rawpy
    with rawpy.imread(path) as r:
        a = r.postprocess(use_camera_wb=True, output_bps=8)
    return to_rgba8(a)


def ref_ffmpeg(path, bits, float_img=False, alpha=False):
    if not float_img:
        probe = subprocess.run(["ffprobe", "-v", "error", "-select_streams", "v:0",
                                "-show_entries", "stream=codec_name,chroma_location,color_transfer",
                                "-of", "json", path], capture_output=True, text=True, timeout=60)
        info = (json.loads(probe.stdout or "{}").get("streams") or [{}])[0]
        loc = info.get("chroma_location", "unspecified")
        if loc in ("unspecified", "unknown", ""):
            # Unsignalled: MPEG-2, H.264 and HEVC co-site chroma ("left"), and ours takes VP9,
            # ProRes and JPEG XS as co-sited too; MPEG-1, MPEG-4 Part 2, VP8, Theora: centre.
            loc = "left" if info.get("codec_name") in (
                "mpeg2video", "h264", "hevc", "vp9", "prores", "jpegxs") else "center"
        # ffmpeg applies the display rotation (autorotate) itself. Bilinear chroma upsampling
        # with the stream's siting, as ours (swscale's default is nearest-like and ignores it).
        r = subprocess.run(["ffmpeg", "-v", "error", "-i", path, "-frames:v", "1", "-vf",
                            f"scale=in_chroma_loc={loc}:flags=bilinear+full_chroma_int+accurate_rnd",
                            "-f", "image2pipe", "-vcodec", "png", "-pix_fmt", "rgba", "-"],
                           capture_output=True, timeout=120)
        if r.returncode or not r.stdout:
            raise RuntimeError("ffmpeg: " + r.stderr.decode(errors="replace").strip()[-160:])
        a = to_rgba8(np.array(Image.open(io.BytesIO(r.stdout)).convert("RGBA")))
        return a, info.get("color_transfer") in ("smpte2084", "arib-std-b67")
    probe = subprocess.run(["ffprobe", "-v", "error", "-select_streams", "v:0", "-show_entries",
                            "stream=width,height", "-of", "json", path],
                           capture_output=True, text=True, timeout=60)
    info = json.loads(probe.stdout or "{}").get("streams", [{}])[0]
    w, h = int(info["width"]), int(info["height"])
    n = 4 if alpha else 3
    r = subprocess.run(["ffmpeg", "-v", "error", "-i", path, "-frames:v", "1", "-f", "rawvideo",
                        "-pix_fmt", "gbrapf32le" if alpha else "gbrpf32le", "-"],
                       capture_output=True, timeout=120)
    if r.returncode or len(r.stdout) < w * h * 4 * n:
        raise RuntimeError("ffmpeg: " + r.stderr.decode(errors="replace").strip()[-160:])
    pl = np.frombuffer(r.stdout[:w * h * 4 * n], np.float32).reshape(n, h, w).copy()
    if alpha:
        # OpenEXR colour is premultiplied; the sheets compare straight alpha.
        a = pl[3]
        pl[:3] = np.where(a > 0, pl[:3] / np.maximum(a, 1e-6), 0)
    return to_rgba8(np.stack([pl[2], pl[0], pl[1]] + list(pl[3:]), 2)), False  # linear, clipped


def ref_magick(path, bits):
    r = subprocess.run(["magick", path + "[0]", "-auto-orient", "-colorspace", "sRGB", "-depth",
                        "8", "png32:-"], capture_output=True, timeout=120)
    if r.returncode or not r.stdout:
        raise RuntimeError("magick: " + r.stderr.decode(errors="replace").strip()[-160:])
    return to_rgba8(np.array(Image.open(io.BytesIO(r.stdout)).convert("RGBA")))


def ref_pdf(path, bits, ext, size):
    with tempfile.TemporaryDirectory(dir=os.environ.get("TMPDIR")) as td:
        out = Path(td) / "page.png"
        if ext == "pdf":
            subprocess.run(["pdftoppm", "-png", "-r", "72", "-f", "1", "-l", "1", "-singlefile",
                            "-scale-to", str(size), path, str(out)[:-4]], check=True,
                           capture_output=True, timeout=120)
        else:
            subprocess.run(["inkscape", path, "--export-type=png", f"--export-filename={out}",
                            f"--export-width={size}"], check=True, capture_output=True,
                           timeout=120, env={**os.environ, "HOME": td})
        return to_rgba8(np.array(Image.open(out).convert("RGBA")))


def ref_dicom(path, bits):
    import pydicom
    from pydicom.pixels import apply_modality_lut, apply_voi_lut
    ds = pydicom.dcmread(path)
    a = ds.pixel_array
    if int(getattr(ds, "NumberOfFrames", 1) or 1) > 1:
        a = a[0]
    photo = str(getattr(ds, "PhotometricInterpretation", ""))
    if a.ndim == 3 and a.shape[-1] == 3:
        if photo.startswith("YBR"):
            from pydicom.pixels import convert_color_space
            a = convert_color_space(a, photo, "RGB")
        return to_rgba8(a, bits)
    x = apply_modality_lut(a, ds)
    if "WindowCenter" in ds or "VOILUTSequence" in ds:
        x = apply_voi_lut(x, ds, index=0)
    x = x.astype(np.float64)
    lo, hi = float(x.min()), float(x.max())
    x = (x - lo) / max(hi - lo, 1e-9)
    if photo == "MONOCHROME1":
        x = 1 - x
    return to_rgba8(np.round(x * 255).astype(np.uint8))


def reference(path, ext, bits, size, fmt="", alpha=False, variant=""):
    """(RGBA uint8 array, tool name, note) or (None, None, why)."""
    b = int(bits) if str(bits).isdigit() else None
    if any(k in fmt.lower() for k in ("radiometric", "thermal", "flir")):
        # We show the temperatures; the JPEG in the file is the camera's own rendering.
        return None, None, "no reference (thermal data, not the embedded picture)"
    try:
        if ext in RAW_EXT and "raw" in fmt.lower() and "thermal" not in fmt.lower():
            return ref_rawpy(path, b), "rawpy (LibRaw)", "tone differs by design"
        if ext in HEIF_EXT:
            return ref_heif(path, b), "libheif", ""
        if ext in CODEC_EXT and re.search(r"BGR(555|565|101010)", variant):
            # imagecodecs (jxrlib) unpacks these packed pixel formats into garbled rows.
            return None, None, "no reference (imagecodecs garbles packed JPEG XR)"
        if ext in CODEC_EXT:
            return ref_codec(path, b, ext), "imagecodecs", ""
        if ext in FLOAT_EXT and (ext != "hdr" or "radiance" in fmt.lower()):
            return ref_ffmpeg(path, b, float_img=True, alpha=alpha)[0], "ffmpeg", \
                "clipped, not tone-mapped"
        if ext in VIDEO_EXT:
            a, hdr = ref_ffmpeg(path, b)
            return a, "ffmpeg", "first frame" + (", HDR: ours tone-mapped, ref not" if hdr else "")
        if ext == "dcm" and fmt == "DICOM":  # not WSI (tiled frames), SR, RT, ...
            return ref_dicom(path, b), "pydicom", "first VOI window"
        if ext in DOC_EXT:
            return ref_pdf(path, b, ext, size), "pdftoppm" if ext == "pdf" else "inkscape", \
                "page 1"
        if ext in PIL_EXT:
            try:
                return ref_pil(path, b), "Pillow", ""
            except Exception:  # noqa: BLE001 - try the next tool
                return ref_magick(path, b), "ImageMagick", ""
        if ext in MAGICK_EXT:
            return ref_magick(path, b), "ImageMagick", ""
    except Exception as e:  # noqa: BLE001 - recorded on the sheet
        msg = re.sub(r"\s+", " ", str(e))[-120:]
        return None, None, f"reference failed: {type(e).__name__}: {msg}"
    return None, None, "no reference decoder"


# ----------------------------------------------------------------------------------- compare

def srgb_to_lin(a):
    a = a / 255.0
    return np.where(a <= 0.04045, a / 12.92, ((a + 0.055) / 1.055) ** 2.4)


def lin_to_srgb8(a):
    a = np.clip(a, 0, 1)
    a = np.where(a <= 0.0031308, a * 12.92, 1.055 * np.power(a, 1 / 2.4) - 0.055)
    return np.round(a * 255).astype(np.uint8)


def resize_linear(a, w, h):
    """RGBA8 resized in linear light with Lanczos (as our converter does)."""
    lin = srgb_to_lin(a[..., :3].astype(np.float64))
    al = a[..., 3].astype(np.float64) / 255.0
    chans = []
    for c in range(3):
        im = Image.fromarray((lin[..., c] * al).astype(np.float32), "F")
        chans.append(np.array(im.resize((w, h), Image.LANCZOS)))
    alr = np.array(Image.fromarray(al.astype(np.float32), "F").resize((w, h), Image.LANCZOS))
    alr = np.clip(alr, 0, 1)
    rgb = np.stack(chans, 2) / np.maximum(alr, 1e-6)[..., None]
    out = np.concatenate([lin_to_srgb8(rgb), np.round(alr * 255).astype(np.uint8)[..., None]], 2)
    return out


def on_grey(a):
    al = a[..., 3:4].astype(np.float64) / 255.0
    return a[..., :3].astype(np.float64) * al + 128.0 * (1 - al)


def compare(ours, ref):
    """(diff image RGBA8 x8, max, mean, note)."""
    oh, ow = ours.shape[:2]
    rh, rw = ref.shape[:2]
    note = ""
    if (rh, rw) != (oh, ow):
        if abs(rw / rh - ow / oh) > 0.015 * (ow / oh) + 1.5 / min(oh, ow):
            if abs(rh / rw - ow / oh) < 0.015 * (ow / oh):
                note = f"ROTATED? ref {rw}x{rh}"
            else:
                note = f"SIZE MISMATCH ref {rw}x{rh}"
            return None, None, None, note
        ref = resize_linear(ref, ow, oh)
        note = f"ref {rw}x{rh} resized"
    d = np.abs(on_grey(ours) - on_grey(ref))
    da = np.abs(ours[..., 3].astype(np.float64) - ref[..., 3].astype(np.float64))
    dd = np.maximum(d.max(axis=2), da)
    img = np.clip(np.concatenate([d, da[..., None]], 2).max(axis=2) * 8, 0, 255).astype(np.uint8)
    img = np.repeat(img[..., None], 3, 2)
    img = np.concatenate([img, np.full(img.shape[:2] + (1,), 255, np.uint8)], 2)
    return img, float(dd.max()), float(dd.mean()), note


# ------------------------------------------------------------------------------------ sheets

TILE = 300
FONT = None


def font(size=13):
    for f in ("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
              "/usr/share/fonts/TTF/DejaVuSans.ttf",
              "/usr/share/fonts/dejavu/DejaVuSans.ttf"):
        if os.path.exists(f):
            return ImageFont.truetype(f, size)
    return ImageFont.load_default()


def checker(w, h):
    y, x = np.mgrid[0:h, 0:w]
    c = np.where(((x // 8) + (y // 8)) % 2 == 0, 200, 150).astype(np.uint8)
    return np.stack([c, c, c, np.full_like(c, 255)], 2)


def tile(a, size=TILE):
    if a is None:
        im = Image.new("RGBA", (size, size), (60, 60, 60, 255))
        return im
    h, w = a.shape[:2]
    s = min(size / w, size / h)
    tw, th = max(1, round(w * s)), max(1, round(h * s))
    im = Image.fromarray(a, "RGBA").resize((tw, th), Image.NEAREST if s >= 1 else Image.BOX)
    bg = Image.fromarray(checker(size, size), "RGBA")
    bg.alpha_composite(im, ((size - tw) // 2, (size - th) // 2))
    return bg


def wrap(text, width):
    out = []
    for para in text.split("\n"):
        while len(para) > width:
            cut = para.rfind(" ", 0, width)
            cut = width if cut <= 0 else cut
            out.append(para[:cut])
            para = para[cut:].lstrip()
        out.append(para)
    return out


def label_of(item):
    r = item["ours"]
    lines = [
        Path(item["path"]).name,
        f"{r.get('format', '?')}  [{item['group']}]",
        f"variant: {item['variant']}",
        f"stored {r.get('width')}x{r.get('height')}  shown {r.get('dw')}x{r.get('dh')}"
        f"  out {item.get('out_size', '-')}",
    ]
    if item.get("ref_tool"):
        lines.append(f"ref: {item['ref_tool']} {item.get('ref_note', '')}".strip())
    if item.get("max") is not None:
        lines.append(f"diff max {item['max']:.0f}  mean {item['mean']:.2f}")
    if item.get("cmp_note"):
        lines.append(item["cmp_note"])
    if item.get("error"):
        lines.append(item["error"])
    return "\n".join(lines)


def draw_sheet(rows, kind, title, path):
    f, fb = font(13), font(18)
    if kind == "oracle":
        W = 3 * TILE + 4 * 8 + 330
        RH = TILE + 12
        H = 40 + RH * len(rows)
        sheet = Image.new("RGB", (W, H), (32, 32, 32))
        d = ImageDraw.Draw(sheet)
        d.text((8, 8), title + "   (ours | reference | |diff| x8)", fill=(255, 255, 255), font=fb)
        for i, it in enumerate(rows):
            y = 40 + i * RH
            for j, a in enumerate([it.get("ours_px"), it.get("ref_px"), it.get("diff_px")]):
                sheet.paste(tile(a).convert("RGB"), (8 + j * (TILE + 8), y))
            tx = 8 + 3 * (TILE + 8)
            bad = it.get("flag")
            col = (255, 140, 120) if bad else (230, 230, 230)
            d.multiline_text((tx, y), "\n".join(sum((wrap(l, 44) for l in
                             label_of(it).split("\n")), [])), fill=col, font=f, spacing=3)
    else:
        cols = 4
        CW, CH = TILE + 8, TILE + 112
        nrow = (len(rows) + cols - 1) // cols
        sheet = Image.new("RGB", (8 + cols * CW, 40 + nrow * CH), (32, 32, 32))
        d = ImageDraw.Draw(sheet)
        d.text((8, 8), title + "   (ours; no reference)", fill=(255, 255, 255), font=fb)
        for i, it in enumerate(rows):
            x, y = 8 + (i % cols) * CW, 40 + (i // cols) * CH
            sheet.paste(tile(it.get("ours_px")).convert("RGB"), (x, y))
            lab = sum((wrap(l, 40) for l in label_of(it).split("\n")), [])[:7]
            col = (255, 140, 120) if it.get("error") else (230, 230, 230)
            d.multiline_text((x, y + TILE + 3), "\n".join(lab), fill=col, font=font(12),
                             spacing=2)
    sheet.save(path, optimize=False, compress_level=3)


# -------------------------------------------------------------------------------------- main

def process(item, exe, long, imgdir):
    path, ext = item["path"], item["ext"]
    h = hashlib.sha1(path.encode()).hexdigest()[:10]
    stem = f"{Path(path).stem[:40]}-{h}"
    ours_png = imgdir / f"{stem}-ours.png"
    r = decode_ours(exe, path, ours_png, long)
    item["ours"] = {**item["probe"], **{k: v for k, v in r.items() if v}}
    if not r.get("status", "").startswith("ok") or not ours_png.exists():
        item["error"] = r.get("status", "err")[:200]
        return item
    ours = to_rgba8(np.array(Image.open(ours_png).convert("RGBA")))
    item["ours_px"] = ours
    item["ours_file"] = ours_png.name
    item["out_size"] = f"{ours.shape[1]}x{ours.shape[0]}"
    ref, tool, note = reference(path, ext, item["probe"].get("bits"), long,
                                item["probe"].get("format", ""), item["probe"].get("alpha") == "1",
                                item["variant"])
    item["ref_tool"], item["ref_note"] = tool, note
    if ref is None:
        if note and not note.startswith("no reference"):
            item["error"] = note
        return item
    item["ref_px"] = ref
    rp = imgdir / f"{stem}-ref.png"
    Image.fromarray(ref, "RGBA").save(rp, compress_level=1)
    item["ref_file"] = rp.name
    diff, mx, mean, cn = compare(ours, ref)
    if re.search(r"\b(PQ|HLG|HDR)\b", item["variant"]) or "HDR:" in (note or ""):
        cn = (cn + "; " if cn else "") + "HDR: ours tone-mapped, ref not"
    item["max"], item["mean"], item["cmp_note"] = mx, mean, cn
    if diff is not None:
        item["diff_px"] = diff
        dp = imgdir / f"{stem}-diff.png"
        Image.fromarray(diff, "RGBA").save(dp, compress_level=1)
        item["diff_file"] = dp.name
    hdr = "HDR:" in (cn or "") or "clipped" in (note or "")
    item["flag"] = not hdr and (bool(cn and "MISMATCH" in cn or "ROTATED" in (cn or "")) or
                                (mean is not None and mean > 6))
    return item


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    ap.add_argument("--exe", required=True, help="the review_decode example binary")
    ap.add_argument("--out", default=str(ROOT / "target" / "review"))
    ap.add_argument("--long", type=int, default=768, help="long side of our output (px)")
    ap.add_argument("--per-format", type=int, default=12, help="variants kept per format")
    ap.add_argument("--jobs", type=int, default=3)
    ap.add_argument("--only", default="", help="group ids: " + ",".join(g[0] for g in GROUPS))
    ap.add_argument("--root", action="append", default=[], help="another corpus folder")
    ap.add_argument("--no-probe-cache", action="store_true")
    ap.add_argument("--rows", type=int, default=6, help="oracle rows per sheet")
    a = ap.parse_args()

    out = Path(a.out)
    if out.exists():
        for p in out.glob("*.png"):
            p.unlink()
        shutil.rmtree(out / "img", ignore_errors=True)
    imgdir = out / "img"
    imgdir.mkdir(parents=True, exist_ok=True)

    paths = candidates(a.root)
    print(f"review: {len(paths)} candidate files", file=sys.stderr)
    rows = probe_all(a.exe, paths, a.jobs, out / "probe-cache.jsonl", not a.no_probe_cache)
    ok = [r for r in rows if r.get("status") == "ok"]
    print(f"review: {len(ok)} open in our registry", file=sys.stderr)
    chosen = select(ok, a.per_format)
    only = set(filter(None, a.only.split(",")))
    items = []
    for r in chosen:
        ext = r["path"].rsplit(".", 1)[-1].lower()
        g = group_of(r["format"], ext, r.get("coding", ""))
        if only and g not in only:
            continue
        items.append(dict(path=r["path"], ext=ext, group=g, variant=variant_key(r), probe=r))
    print(f"review: {len(items)} samples ({len({i['probe']['format'] for i in items})} formats)",
          file=sys.stderr)

    done = []
    with cf.ThreadPoolExecutor(a.jobs) as ex:
        futs = [ex.submit(process, it, a.exe, a.long, imgdir) for it in items]
        for n, fu in enumerate(cf.as_completed(futs), 1):
            try:
                done.append(fu.result())
            except Exception as e:  # noqa: BLE001 - keep the sweep going
                print(f"review: internal error {e!r}", file=sys.stderr)
            if n % 25 == 0:
                print(f"review: {n}/{len(items)}", file=sys.stderr)

    done.sort(key=lambda i: (i["group"], i["ours"].get("format", ""), i["variant"]))
    sheets = []
    for gid, title, *_ in GROUPS:
        g = [i for i in done if i["group"] == gid]
        if not g:
            continue
        orc = [i for i in g if i.get("ref_px") is not None]
        grid = [i for i in g if i.get("ref_px") is None]
        for k in range(0, len(orc), a.rows):
            p = out / f"{gid}-oracle-{k // a.rows + 1}.png"
            draw_sheet(orc[k:k + a.rows], "oracle", f"{title} {k // a.rows + 1}", p)
            sheets.append(p.name)
        for k in range(0, len(grid), 16):
            p = out / f"{gid}-grid-{k // 16 + 1}.png"
            draw_sheet(grid[k:k + 16], "grid", f"{title} {k // 16 + 1}", p)
            sheets.append(p.name)

    with open(out / "summary.tsv", "w") as f:
        f.write("group\tformat\tvariant\tfile\tstored\tshown\tout\tref\tmax\tmean\tnote\n")
        for i in done:
            r = i["ours"]
            f.write("\t".join(re.sub(r"\s+", " ", str(x)) for x in [
                i["group"], r.get("format", ""), i["variant"], i["path"],
                f"{r.get('width')}x{r.get('height')}", f"{r.get('dw')}x{r.get('dh')}",
                i.get("out_size", ""), i.get("ref_tool") or "",
                "" if i.get("max") is None else f"{i['max']:.0f}",
                "" if i.get("mean") is None else f"{i['mean']:.2f}",
                (i.get("cmp_note") or i.get("error") or i.get("ref_note") or "")]) + "\n")

    def img(name):
        return f'<a href="img/{name}"><img src="img/{name}" loading="lazy"></a>' if name else ""
    with open(out / "index.html", "w") as f:
        f.write("<!doctype html><meta charset=utf-8><title>Review sheets</title><style>"
                "body{font:13px sans-serif;background:#222;color:#ddd}img{max-width:256px;"
                "max-height:256px;background:repeating-conic-gradient(#999 0 25%,#666 0 50%) "
                "0 0/16px 16px}td{vertical-align:top;padding:4px}.bad{color:#f98}</style>")
        f.write("<h1>Visual review sheets</h1><p>" + " ".join(
            f'<a href="{s}">{s}</a>' for s in sheets) + "</p><table>")
        for i in done:
            cls = ' class="bad"' if i.get("flag") or i.get("error") else ""
            f.write(f"<tr{cls}><td>{img(i.get('ours_file'))}</td><td>{img(i.get('ref_file'))}"
                    f"</td><td>{img(i.get('diff_file'))}</td><td><pre>"
                    f"{html.escape(label_of(i))}</pre></td></tr>")
        f.write("</table>")
    n_ref = sum(1 for i in done if i.get("ref_px") is not None)
    n_err = sum(1 for i in done if i.get("error") and i.get("ours_px") is None)
    print(f"review: {len(done)} samples, {n_ref} with a reference, {n_err} failed to decode; "
          f"{len(sheets)} sheets in {out}", file=sys.stderr)


if __name__ == "__main__":
    main()
