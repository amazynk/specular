# The point cloud corpus oracle (tools/corpus/fetch-points.sh): reads every LAS/LAZ/COPC file
# with laspy (lazrs backend), every E57 with pye57 and every PCD with open3d (when it is
# installed), and writes under CORPUS/oracle/:
#
#   expected.tsv      RELPATH  POINTS  MINX MINY MINZ  MAXX MAXY MAXZ  RGBMAX
#   RELPATH.tsv       up to 2000 evenly spaced points:  X Y Z  R G B  INTENSITY  CLASS
#
# POINTS counts the points a viewer shows (E57's invalid ones and PCD's NaN ones dropped).
# Colours are as stored (16-bit for LAS, as the file's limits say for E57, 8-bit for PCD);
# RGBMAX is the largest stored component (LAS colour depth). "-" marks what the oracle does
# not know. Files the oracle cannot read are left out (the corpus test still reads them).
#
# Usage: PYTHON -I oracle.py CORPUS   (laspy[lazrs], pye57, optionally open3d)
import os
import sys

import numpy as np

SAMPLES = 2000


def even(n):
    if n <= SAMPLES:
        return np.arange(n)
    return (np.arange(SAMPLES, dtype=np.int64) * n) // SAMPLES


def rows(xyz, rgb, inten, cls, idx):
    out = []
    for i in idx:
        x, y, z = xyz[i]
        c = " ".join(str(int(v)) for v in rgb[i]) if rgb is not None else "- - -"
        it = str(int(inten[i])) if inten is not None else "-"
        cl = str(int(cls[i])) if cls is not None else "-"
        out.append(f"{float(x)!r} {float(y)!r} {float(z)!r}\t{c}\t{it}\t{cl}")
    return out


def las(path):
    import laspy

    f = laspy.read(path, laz_backend=laspy.LazBackend.Lazrs)
    n = len(f.points)
    xyz = np.stack([np.asarray(f.x), np.asarray(f.y), np.asarray(f.z)], axis=1)
    dims = set(f.point_format.dimension_names)
    rgb = None
    rgbmax = "-"
    if {"red", "green", "blue"} <= dims:
        rgb = np.stack([np.asarray(f.red), np.asarray(f.green), np.asarray(f.blue)], axis=1)
        rgbmax = str(int(rgb.max())) if n else "0"
    cls = np.asarray(f.classification)
    return n, xyz, rgb, np.asarray(f.intensity), cls, rgbmax


def e57(path):
    import pye57

    e = pye57.E57(path)
    xyzs, rgbs = [], []
    has_rgb = True
    for i in range(e.scan_count):
        d = e.read_scan(i, ignore_missing_fields=True, colors=True, intensity=True, transform=True)
        xyz = np.stack([d["cartesianX"], d["cartesianY"], d["cartesianZ"]], axis=1)
        xyzs.append(xyz)
        if "colorRed" in d and len(d["colorRed"]) == len(xyz):
            rgbs.append(np.stack([d["colorRed"], d["colorGreen"], d["colorBlue"]], axis=1))
        else:
            has_rgb = False
    xyz = np.concatenate(xyzs) if xyzs else np.zeros((0, 3))
    rgb = np.concatenate(rgbs) if has_rgb and rgbs else None
    return len(xyz), xyz, rgb, None, None, "-"


def pcd(path):
    import open3d as o3d

    p = o3d.io.read_point_cloud(path, remove_nan_points=True, remove_infinite_points=True)
    xyz = np.asarray(p.points)
    rgb = None
    if p.has_colors():
        rgb = np.rint(np.asarray(p.colors) * 255.0).astype(np.int64)
    return len(xyz), xyz, rgb, None, None, "-"


def readers():
    try:
        import open3d  # noqa: F401

        return {"las": las, "laz": las, "e57": e57, "pcd": pcd}
    except ImportError:
        return {"las": las, "laz": las, "e57": e57}


def one(root, rel):
    """Reads one file, writes its samples and prints its expected.tsv line."""
    ext = rel.rsplit(".", 1)[-1].lower()
    n, xyz, rgb, inten, cls, rgbmax = readers()[ext](os.path.join(root, rel))
    if n:
        lo, hi = xyz.min(axis=0), xyz.max(axis=0)
    else:
        lo = hi = np.zeros(3)
    sample = os.path.join(root, "oracle", rel + ".tsv")
    os.makedirs(os.path.dirname(sample), exist_ok=True)
    with open(sample, "w") as f:
        f.write("\n".join(rows(xyz, rgb, inten, cls, even(n))) + "\n")
    print(
        f"{rel}\t{n}\t{' '.join(repr(float(v)) for v in lo)}\t"
        f"{' '.join(repr(float(v)) for v in hi)}\t{rgbmax}"
    )


def capped():
    # Broken files on purpose can make a reader allocate without bound.
    import resource

    resource.setrlimit(resource.RLIMIT_AS, (LIMIT_BYTES, LIMIT_BYTES))


LIMIT_BYTES = 6 << 30
TIMEOUT_S = 300


def main():
    import subprocess

    root = sys.argv[1]
    out = os.path.join(root, "oracle")
    os.makedirs(out, exist_ok=True)
    exts = readers()
    lines = []
    # Each file in its own process: the corpus has broken files on purpose, and a reader that
    # panics, hangs or runs out of memory on one must not stop the rest.
    for dirpath, dirs, files in os.walk(root):
        dirs[:] = sorted(d for d in dirs if d != "oracle")
        for name in sorted(files):
            if name.rsplit(".", 1)[-1].lower() not in exts:
                continue
            rel = os.path.relpath(os.path.join(dirpath, name), root)
            try:
                r = subprocess.run(
                    [sys.executable, "-I", os.path.abspath(__file__), "--one", root, rel],
                    capture_output=True,
                    text=True,
                    timeout=TIMEOUT_S,
                    preexec_fn=capped,
                )
            except subprocess.TimeoutExpired:
                print(f"  oracle skips {rel}: timed out", flush=True)
                continue
            line = r.stdout.strip().splitlines()[-1:] if r.returncode == 0 else []
            if not line:
                err = (r.stderr.strip().splitlines() or ["?"])[-1]
                print(f"  oracle skips {rel}: {err}", flush=True)
                continue
            lines.append(line[0])
    with open(os.path.join(out, "expected.tsv"), "w") as f:
        f.write("\n".join(lines) + "\n")
    print(f"  oracle: {len(lines)} files")


if len(sys.argv) == 4 and sys.argv[1] == "--one":
    one(sys.argv[2], sys.argv[3])
else:
    main()
