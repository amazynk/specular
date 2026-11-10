"""dicomwsi-oracle.py [--tiles] [--raw DIR] <series folder> <out.tsv>: what wsidicom reads
dicomwsi-oracle.py --dicom <out.tsv> <file>...: JPEG XL frames and real-world values

The oracle of specular-wsi's DICOM WSI tests (tests/dicomwsi.rs reads the fixtures' full.tsv and
sparse.tsv; tests/dicomwsi_corpus.rs the fetched series', written by fetch-dicomwsi.sh). Needs wsidicom and numpy (the
oracle venv; run with python -I). Reads the series at its first optical path (Optical Path
Sequence order) and lowest focal plane, the plane specular-wsi shows by default.

Lines (tab-separated):
  slide   width height levels mpp_x mpp_y path z
  level   index width height tile_w tile_h fnv        fnv: FNV-1a 64 of the level's RGB bytes
                                                       ("-" when the level is over 4096 x 4096)
  tile    level tx ty fnv                              (--tiles: every tile, cropped to the image)
  assoc   label|macro width height fnv [hex]           (--tiles: the RGB bytes in hex when at
                                                       most 4096 pixels, for lossy images)
  region  level x y width height file                  (--raw: up to 1024 x 1024 RGB at each
                                                       level's centre, written to DIR/file)
"""
import os
import sys

import numpy as np
import wsidicom

FNV_OFFSET = 0xCBF29CE484222325
FNV_PRIME = 0x100000001B3
MASK = (1 << 64) - 1


def fnv(a):
    h = FNV_OFFSET
    for b in np.ascontiguousarray(a, np.uint8).tobytes():
        h = ((h ^ b) * FNV_PRIME) & MASK
    return f"{h:016x}"


def rgb(img):
    a = np.asarray(img)
    if a.ndim == 2:
        a = np.repeat(a[..., None], 3, axis=2)
    return a[..., :3]


def first_path(files, paths):
    """The base level's first optical path in its Optical Path Sequence (wsidicom's own order
    of paths is not stable between runs); of base instances holding one path each, the one with
    the lowest Instance Number."""
    import pydicom

    best = None
    for name in files:
        try:
            ds = pydicom.dcmread(name, stop_before_pixels=True)
        except Exception:
            continue
        if ds.get("SOPClassUID") != "1.2.840.10008.5.1.4.1.1.77.1.6" or ds.ImageType[2] != "VOLUME":
            continue
        key = (-int(ds.TotalPixelMatrixColumns) * int(ds.TotalPixelMatrixRows), int(ds.get("InstanceNumber", 0)))
        if best is None or key < best[0]:
            best = (key, [str(p.OpticalPathIdentifier) for p in ds.OpticalPathSequence])
    for p in best[1] if best else []:
        if p in paths:
            return p
    return sorted(paths)[0]


def cross_check(files, lines):
    """highdicom's total pixel matrix of each single-plane VOLUME instance it can index, against
    wsidicom's level of that size (reported on stderr)."""
    import highdicom

    levels = {(int(l[2]), int(l[3])): l[6] for l in lines if l[0] == "level"}
    for name in files:
        try:
            im = highdicom.imread(name)
            if im.ImageType[2] != "VOLUME":
                continue
            a = rgb(im.get_total_pixel_matrix(dtype=np.uint8))
        except Exception as e:  # highdicom indexes TILED_FULL single-plane instances only
            print(f"highdicom: {os.path.basename(name)}: skipped ({type(e).__name__})", file=sys.stderr)
            continue
        want = levels.get((a.shape[1], a.shape[0]))
        verdict = "agrees" if want == fnv(a) else "DIFFERS"
        print(f"highdicom: {os.path.basename(name)}: {verdict} with wsidicom", file=sys.stderr)


def dicom_oracle(out, names):
    """--dicom: specular-dicom's JPEG XL and RWVM fixtures. JPEG XL frames decoded by libjxl
    (imagecodecs); real-world values by highdicom's RealWorldValueMapping (the first item whose
    range covers the stored value), stored values by pydicom."""
    import imagecodecs
    import pydicom
    from pydicom.dataset import Dataset
    from highdicom.pm.content import RealWorldValueMapping

    lines = []
    for name in names:
        ds = pydicom.dcmread(name)
        base = os.path.basename(name)
        if ds.file_meta.TransferSyntaxUID.startswith("1.2.840.10008.1.2.4.11"):
            frame = next(pydicom.encaps.generate_frames(ds.PixelData, number_of_frames=1))
            a = np.asarray(imagecodecs.jpegxl_decode(frame))
            data = a.astype("<u2" if a.dtype.itemsize > 1 else np.uint8).tobytes()
            ch = 1 if a.ndim == 2 else a.shape[2]
            lines.append(["jxl", base, a.shape[1], a.shape[0], ch, a.dtype.itemsize * 8, data.hex()])
        if "RealWorldValueMappingSequence" in ds:
            maps = []
            for item in ds.RealWorldValueMappingSequence:
                # highdicom builds these for writing; wrap the read item in the class.
                m = RealWorldValueMapping.__new__(RealWorldValueMapping)
                Dataset.__init__(m, item)
                maps.append(m)
            px = ds.pixel_array
            for y in range(px.shape[0]):
                for x in range(px.shape[1]):
                    v = int(px[y, x])
                    for m in maps:
                        lo, hi = m.value_range
                        if lo <= v <= hi:
                            r = float(m.apply(np.array([v], dtype=np.int64))[0])
                            lines.append(["rwvm", base, x, y, v, repr(r)])
                            break
                    else:
                        lines.append(["rwvm", base, x, y, v, "-"])
    with open(out, "w") as f:
        for line in lines:
            f.write("\t".join(str(x) for x in line) + "\n")


def main():
    args = sys.argv[1:]
    if args and args[0] == "--dicom":
        dicom_oracle(args[1], args[2:])
        return
    tiles = "--tiles" in args
    if tiles:
        args.remove("--tiles")
    raw = None
    if "--raw" in args:
        i = args.index("--raw")
        raw = args[i + 1]
        del args[i : i + 2]
        os.makedirs(raw, exist_ok=True)
    folder, out = args
    files = sorted(os.path.join(folder, f) for f in os.listdir(folder) if os.path.isfile(os.path.join(folder, f)))
    wsi = wsidicom.WsiDicom.open(files)
    base = wsi.levels[0]
    path = first_path(files, base.optical_paths)
    z = min(base.focal_planes)
    lines = []
    mpp = wsi.mpp
    lines.append(["slide", wsi.size.width, wsi.size.height, len(wsi.levels), f"{mpp.width:.6f}", f"{mpp.height:.6f}", path, z])
    # wsidicom reads by pyramid level (log2 of the downsample), not by index: a pyramid of 1, 4
    # and 16 has levels 0, 2 and 4.
    for i, level in enumerate(wsi.levels):
        w, h = level.size.width, level.size.height
        tw, th = level.tile_size.width, level.tile_size.height
        lz = min(level.focal_planes, key=lambda f: abs(f - z))
        lp = path if path in level.optical_paths else level.optical_paths[0]
        if w * h <= 4096 * 4096:
            full = rgb(wsi.read_region((0, 0), level.level, (w, h), z=lz, path=lp, as_array=True))
            lines.append(["level", i, w, h, tw, th, fnv(full)])
        else:
            full = None
            lines.append(["level", i, w, h, tw, th, "-"])
        if tiles and full is not None:
            for ty in range(-(-h // th)):
                for tx in range(-(-w // tw)):
                    t = full[ty * th : (ty + 1) * th, tx * tw : (tx + 1) * tw]
                    lines.append(["tile", i, tx, ty, fnv(t)])
        if raw is not None:
            rw, rh = min(w, 1024), min(h, 1024)
            x, y = (w - rw) // 2, (h - rh) // 2
            reg = rgb(wsi.read_region((x, y), level.level, (rw, rh), z=lz, path=lp, as_array=True))
            name = f"L{i}.rgb"
            with open(os.path.join(raw, name), "wb") as f:
                f.write(np.ascontiguousarray(reg, np.uint8).tobytes())
            lines.append(["region", i, x, y, rw, rh, name])
    for kind, imgs, read in [("label", wsi.labels, wsi.read_label), ("macro", wsi.overviews, wsi.read_overview)]:
        if imgs:
            a = rgb(read(as_array=True))
            row = ["assoc", kind, a.shape[1], a.shape[0], fnv(a)]
            if tiles and a.shape[0] * a.shape[1] <= 4096:
                row.append(np.ascontiguousarray(a, np.uint8).tobytes().hex())
            lines.append(row)
    if tiles:
        cross_check(files, lines)
    with open(out, "w") as f:
        for line in lines:
            f.write("\t".join(str(x) for x in line) + "\n")


if __name__ == "__main__":
    main()
