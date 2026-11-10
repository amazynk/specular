"""The fmt500e oracles, written beside the corpus tools/corpus/fetch-fmt500e.sh fetched:

- str/: ffmpeg's decode of the PlayStation movie (yuvj420p planes, s16le sound);
- trackers/: libopenmpt's render (through ffmpeg, 48 kHz stereo) shrunk to loudness and
  zero-crossing envelopes in 20 ms windows (`<module>.env`: frames, then "rms crossings").

Run: python3 -I tools/corpus/fmt500e-oracle.py <folder>. Only ffmpeg is run; nothing is
linked or read from its sources.
"""

import math
import os
import struct
import subprocess
import sys

FF = ["ffmpeg", "-hide_banner", "-loglevel", "error", "-y"]


def str_movie(root):
    d = os.path.join(root, "str")
    src = os.path.join(d, "descent-partial.str")
    if not os.path.exists(src):
        return
    # The file is cut short: ffmpeg reports the end as an error after its last whole frame.
    subprocess.run(FF + ["-i", src, "-map", "0:v", "-f", "rawvideo", "-pix_fmt", "yuvj420p",
                         os.path.join(d, "descent.yuv"), "-map", "0:a", "-f", "s16le",
                         "-acodec", "pcm_s16le", os.path.join(d, "descent.s16")], check=False)


def envelope(pcm):
    v = struct.unpack("<%dh" % (len(pcm) // 2), pcm)
    mono = [(v[2 * i] + v[2 * i + 1]) / 2 for i in range(len(v) // 2)]
    lines = ["%d" % len(mono)]
    for k in range(0, len(mono) - 959, 960):
        w = mono[k:k + 960]
        rms = math.sqrt(sum(x * x for x in w) / len(w))
        zc = sum(1 for i in range(1, len(w)) if (w[i - 1] < 0) != (w[i] < 0))
        lines.append("%.1f %d" % (rms, zc))
    return "\n".join(lines) + "\n"


def trackers(root):
    d = os.path.join(root, "trackers")
    if not os.path.isdir(d):
        return
    for name in sorted(os.listdir(d)):
        if name.endswith(".env"):
            continue
        out = os.path.join(d, name + ".env")
        if os.path.exists(out):
            continue
        r = subprocess.run(FF + ["-f", "libopenmpt", "-i", os.path.join(d, name), "-ar", "48000",
                                 "-ac", "2", "-f", "s16le", "-acodec", "pcm_s16le", "-"],
                           capture_output=True, check=True)
        open(out, "w").write(envelope(r.stdout))
        print(name, len(r.stdout) // 4, "frames")


def ink(im):
    """A 1-bit glyph image cropped to its ink: (width, height, rows as '0'/'1' strings)."""
    im = im.convert("L")
    box = im.point(lambda v: 255 if v > 127 else 0).getbbox()
    if box is None:
        return (0, 0, "")
    im = im.crop(box)
    w, h = im.size
    px = im.load()
    return (w, h, "".join("1" if px[x, y] > 127 else "0" for y in range(h) for x in range(w)))


def fonts(root):
    """fonts/<name>.glyphs: one line a character, "code width height bits" of its ink, from
    Pillow's own PCF and BDF readers (codes 0-255). bdftopcf's PCF of the BDF sample checks
    one reader against the other. (FreeType through Pillow finds no character map in the
    Windows .fon fonts, so they have no oracle.)"""
    d = os.path.join(root, "fonts")
    if not os.path.isdir(d):
        return
    try:
        from PIL import BdfFontFile, PcfFontFile
    except ImportError:
        print("Pillow missing: no font oracle")
        return
    bdf = os.path.join(d, "courB08.bdf")
    if os.path.exists(bdf) and not os.path.exists(os.path.join(d, "courB08-bdftopcf.pcf")):
        subprocess.run(["bdftopcf", "-o", os.path.join(d, "courB08-bdftopcf.pcf"), bdf], check=False)
    for name in sorted(os.listdir(d)):
        p = os.path.join(d, name)
        out = p + ".glyphs"
        if name.endswith(".glyphs") or os.path.exists(out):
            continue
        lines = []
        if name.endswith((".pcf", ".bdf")):
            with open(p, "rb") as f:
                ff = (PcfFontFile.PcfFontFile(f) if name.endswith(".pcf") else BdfFontFile.BdfFontFile(f))
            for code, g in enumerate(ff.glyph):
                if g:
                    w, h, bits = ink(g[3])
                    lines.append("%d %d %d %s" % (code, w, h, bits))
        else:
            continue
        open(out, "w").write("\n".join(lines) + "\n")
        print(name, len(lines), "glyphs")


def notebooks(root):
    """notebooks/<name>.cells: nbformat's reading (upgraded to version 4), one JSON line a
    cell: its type, source, count and outputs (type, stream name or MIME keys, text). Needs
    nbformat (pip, in a venv: run this script with that venv's python)."""
    d = os.path.join(root, "notebooks")
    if not os.path.isdir(d):
        return
    try:
        import nbformat
    except ImportError:
        print("nbformat missing: no notebook oracle")
        return
    import json
    for name in sorted(os.listdir(d)):
        if not name.endswith(".ipynb"):
            continue
        nb = nbformat.read(os.path.join(d, name), as_version=4)
        lines = []
        for c in nb.cells:
            outs = []
            for o in c.get("outputs", []):
                t = o["output_type"]
                if t == "stream":
                    outs.append([t, o.get("name", ""), o.get("text", "")])
                elif t == "error":
                    outs.append([t, o.get("ename", ""), o.get("evalue", "")])
                else:
                    outs.append([t, ",".join(sorted(o.get("data", {}).keys())),
                                 o.get("data", {}).get("text/plain", "")])
            lines.append(json.dumps({"type": c.cell_type, "source": c.get("source", ""),
                                     "count": c.get("execution_count"), "outputs": outs}))
        open(os.path.join(d, name + ".cells"), "w").write("\n".join(lines) + "\n")
        print(name, len(lines), "cells")


def lottie(root):
    """lottie/<name>.f<k>.png: rlottie's frames 0, n/4, n/2 and 3n/4 at the canvas size,
    composited on white. Needs rlottie-python (LGPL, run only) in the venv."""
    d = os.path.join(root, "lottie")
    if not os.path.isdir(d):
        return
    try:
        import rlottie_python
        from PIL import Image
    except ImportError:
        print("rlottie-python missing: no Lottie oracle")
        return
    for name in sorted(os.listdir(d)):
        if not name.endswith(".json"):
            continue
        a = rlottie_python.LottieAnimation.from_file(os.path.join(d, name))
        n = a.lottie_animation_get_totalframe()
        w, h = a.lottie_animation_get_size()
        for k in (0, n // 4, n // 2, 3 * n // 4):
            im = a.render_pillow_frame(frame_num=k, width=w, height=h)
            bg = Image.new("RGBA", im.size, (255, 255, 255, 255))
            bg.alpha_composite(im.convert("RGBA"))
            # Raw RGB rows: what the test reads without a PNG decoder.
            open(os.path.join(d, "%s.f%d.rgb" % (name, k)), "wb").write(bg.convert("RGB").tobytes())
        print(name, n, "frames", w, h)


def models(root):
    """models/<name>.mesh: assimp's reading (triangulated, node transforms applied): the
    triangle count and the bounding box of the vertices the triangles use. Needs
    assimp-py in the venv. MagicaVoxel files (assimp has no reader): the voxel count."""
    d = os.path.join(root, "models")
    if not os.path.isdir(d):
        return
    try:
        import assimp_py as a
    except ImportError:
        print("assimp-py missing: no model oracle")
        return
    import struct as st
    for name in sorted(os.listdir(d)):
        p = os.path.join(d, name)
        if name.endswith(".mesh"):
            continue
        if name.endswith(".vox"):
            b = open(p, "rb").read()
            n, at = 0, 20
            while at + 12 <= len(b):
                cid, size, kids = b[at:at + 4], *st.unpack("<ii", b[at + 4:at + 12])
                if cid == b"XYZI":
                    n += st.unpack("<i", b[at + 12:at + 16])[0]
                at = at + 12 if cid == b"MAIN" else at + 12 + size + kids
            open(p + ".mesh", "w").write("voxels %d\n" % n)
            print(name, n, "voxels")
            continue
        try:
            sc = a.import_file(p, a.Process_Triangulate | a.Process_PreTransformVertices)
        except Exception as e:
            print(name, "assimp:", e)
            continue
        tris, lo, hi = 0, [1e30] * 3, [-1e30] * 3
        for m in sc.meshes:
            v = list(m.vertices)
            idx = list(m.indices)
            faces = len(idx) // 3
            tris += faces
            for i in idx:
                p3 = v[i] if isinstance(v[i], (tuple, list)) else v[3 * i:3 * i + 3]
                for k in range(3):
                    lo[k] = min(lo[k], p3[k])
                    hi[k] = max(hi[k], p3[k])
        open(p + ".mesh", "w").write("triangles %d\nbounds %s %s\n" % (
            tris, " ".join("%.4f" % x for x in lo), " ".join("%.4f" % x for x in hi)))
        print(name, tris, "triangles")


def swf(root):
    """swf/<test>.expected.rgb: the pictures Ruffle's tests expect, as raw RGB rows (with
    the size in a .size file), for the test to read without a PNG decoder."""
    d = os.path.join(root, "swf")
    if not os.path.isdir(d):
        return
    from PIL import Image
    for name in sorted(os.listdir(d)):
        if not name.endswith(".output.expected.png"):
            continue
        base = name[: -len(".output.expected.png")]
        im = Image.open(os.path.join(d, name)).convert("RGBA")
        bg = Image.new("RGBA", im.size, (255, 255, 255, 255))
        bg.alpha_composite(im)
        open(os.path.join(d, base + ".expected.rgb"), "wb").write(bg.convert("RGB").tobytes())
        open(os.path.join(d, base + ".expected.size"), "w").write("%d %d\n" % im.size)


def molecules(root):
    """mol/<name>.atoms: RDKit's reading of MOL2 (atoms, bonds, then "Z x y z" per atom)
    and mdtraj's of GRO (LGPL, run only; atoms, then "Z x y z" in Å)."""
    d = os.path.join(root, "mol")
    if not os.path.isdir(d):
        return
    try:
        import mdtraj
        from rdkit import Chem
    except ImportError:
        print("rdkit or mdtraj missing: no molecule oracle")
        return
    for name in sorted(os.listdir(d)):
        p = os.path.join(d, name)
        if name.endswith(".mol2"):
            m = Chem.MolFromMol2File(p, removeHs=False, sanitize=False)
            conf = m.GetConformer()
            lines = ["atoms %d" % m.GetNumAtoms(), "bonds %d" % m.GetNumBonds()]
            for a in m.GetAtoms():
                q = conf.GetAtomPosition(a.GetIdx())
                lines.append("%d %.4f %.4f %.4f" % (a.GetAtomicNum(), q.x, q.y, q.z))
        elif name.endswith(".gro"):
            t = mdtraj.load(p)
            lines = ["atoms %d" % t.n_atoms, "bonds 0"]
            for a, q in zip(t.topology.atoms, t.xyz[0]):
                z = a.element.atomic_number if a.element is not None else 0
                lines.append("%d %.4f %.4f %.4f" % (z, q[0] * 10, q[1] * 10, q[2] * 10))
        else:
            continue
        open(p + ".atoms", "w").write("\n".join(lines) + "\n")
        print(name, lines[0])


if __name__ == "__main__":
    root = sys.argv[1]
    str_movie(root)
    trackers(root)
    fonts(root)
    notebooks(root)
    lottie(root)
    models(root)
    swf(root)
    molecules(root)
