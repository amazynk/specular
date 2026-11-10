#!/usr/bin/env python3
"""Wraps JPEG XS codestreams in the containers of ISO/IEC 21122-3 that no available tool
writes: a HEIF file whose primary item is a `jxs1` image item (`image/jxsi`), and an MP4
movie with a `jxsm` video track (one codestream per sample). Written from the box
definitions of ISO/IEC 14496-12 and 23008-12, independently of Specular's readers, so the
readers are checked against a second implementation of the layout.

    jpegxs-wrap.py heif IN.jxc OUT.jxsi [--irot N] [--imir AXIS] [--clap W H] [--split]
                   [--idat] [--nclx P,T,M,FULL]
    jpegxs-wrap.py mp4 OUT.mp4 FRAME.jxc...
    jpegxs-wrap.py corpus ROOT

`--split` stores the item in two extents, `--idat` inside the meta box (construction method
1); `--clap W H` crops a centred W×H. `corpus` writes ROOT/heif/ (every codestream of
ROOT/synth and ROOT/conformance the reference decoder reads, as .jxsi, with their manifest
rows) and ROOT/mp4/ (each TDC sequence of ROOT/sequences as one .mp4, the manifest naming
frames FILE.mp4#N).

Standard library only.
"""

import os
import struct
import sys


def box(kind, body):
    return struct.pack(">I4s", 8 + len(body), kind) + body


def full(kind, version, flags, body):
    return box(kind, struct.pack(">I", (version << 24) | flags) + body)


def header(cs):
    """Width, height, Ppih, Plev from the codestream's PIH marker segment."""
    i = 2
    while i + 4 <= len(cs):
        if cs[i] != 0xFF:
            break
        marker = cs[i + 1]
        if marker == 0x50:  # CAP
            i += 2 + struct.unpack(">H", cs[i + 2 : i + 4])[0]
            continue
        if marker == 0x12:  # PIH: Lpih Lcod Ppih Plev Wf Hf
            ppih, plev, wf, hf = struct.unpack(">HHHH", cs[i + 8 : i + 16])
            return wf, hf, ppih, plev
        i += 2 + struct.unpack(">H", cs[i + 2 : i + 4])[0]
    raise SystemExit("no PIH marker segment")


def nclx(p, t, m, full_range):
    return box(b"colr", b"nclx" + struct.pack(">HHHB", p, t, m, 0x80 if full_range else 0))


def heif(cs, irot=None, imir=None, clap=None, split=False, idat=False, colour=(1, 1, 1, 0)):
    w, h, ppih, plev = header(cs)
    ftyp = box(b"ftyp", b"mif1" + struct.pack(">I", 0) + b"mif1")
    hdlr = full(b"hdlr", 0, 0, struct.pack(">I", 0) + b"pict" + bytes(12) + b"\0")
    pitm = full(b"pitm", 0, 0, struct.pack(">H", 1))
    infe = full(b"infe", 2, 0, struct.pack(">HH", 1, 0) + b"jxs1" + b"JPEG XS\0")
    iinf = full(b"iinf", 0, 0, struct.pack(">H", 1) + infe)
    # Descriptive properties first, then the transformative ones in the order they apply.
    props = [
        (full(b"ispe", 0, 0, struct.pack(">II", w, h)), False),
        (nclx(*colour), False),
        (box(b"jxpl", struct.pack(">HH", ppih, plev)), True),
    ]
    if clap:
        cw, ch = clap
        props.append((box(b"clap", struct.pack(">IIIIiIiI", cw, 1, ch, 1, 0, 1, 0, 1)), True))
    if irot is not None:
        props.append((box(b"irot", bytes([irot & 3])), True))
    if imir is not None:
        props.append((box(b"imir", bytes([imir & 1])), True))
    ipco = box(b"ipco", b"".join(p for p, _ in props))
    assoc = bytes([(0x80 if ess else 0) | (k + 1) for k, (_, ess) in enumerate(props)])
    ipma = full(b"ipma", 0, 0, struct.pack(">IHB", 1, 1, len(props)) + assoc)
    iprp = box(b"iprp", ipco + ipma)
    parts = [cs[: len(cs) // 2], cs[len(cs) // 2 :]] if split else [cs]

    def iloc(offsets):
        # Version 1 (construction method), offset and length 4 bytes, no base offset.
        body = struct.pack(">BBH", 0x44, 0x00, 1)
        body += struct.pack(">HHHH", 1, 1 if idat else 0, 0, len(parts))
        for off, p in zip(offsets, parts):
            body += struct.pack(">II", off, len(p))
        return full(b"iloc", 1, 0, body)

    idat_box = box(b"idat", b"".join(parts)) if idat else b""
    # The offsets are known once the meta box's size is: lay it out twice.
    meta = full(b"meta", 0, 0, hdlr + pitm + iinf + iloc([0] * len(parts)) + iprp + idat_box)
    if idat:
        offsets, at = [], 0
    else:
        offsets, at = [], len(ftyp) + len(meta) + 8
    for p in parts:
        offsets.append(at)
        at += len(p)
    meta = full(b"meta", 0, 0, hdlr + pitm + iinf + iloc(offsets) + iprp + idat_box)
    out = ftyp + meta
    if not idat:
        out += box(b"mdat", b"".join(parts))
    return out


def mp4(frames, rate=25):
    w, h, ppih, plev = header(frames[0])
    n = len(frames)
    ftyp = box(b"ftyp", b"isom" + struct.pack(">I", 512) + b"isomiso6mp41")
    matrix = struct.pack(">9I", 0x10000, 0, 0, 0, 0x10000, 0, 0, 0, 0x40000000)
    mvhd = full(
        b"mvhd", 0, 0,
        struct.pack(">IIII", 0, 0, rate, n) + struct.pack(">IH", 0x10000, 0x100) + bytes(10)
        + matrix + bytes(24) + struct.pack(">I", 2),
    )
    tkhd = full(
        b"tkhd", 0, 3,
        struct.pack(">IIIII", 0, 0, 1, 0, n) + bytes(8) + struct.pack(">hhhH", 0, 0, 0, 0)
        + matrix + struct.pack(">II", w << 16, h << 16),
    )
    mdhd = full(b"mdhd", 0, 0, struct.pack(">IIIIHH", 0, 0, rate, n, 0x55C4, 0))
    hdlr = full(b"hdlr", 0, 0, struct.pack(">I", 0) + b"vide" + bytes(12) + b"JPEG XS\0")
    vmhd = full(b"vmhd", 0, 1, bytes(8))
    dinf = box(b"dinf", full(b"dref", 0, 0, struct.pack(">I", 1) + full(b"url ", 0, 1, b"")))
    name = b"JPEG XS"
    entry = (
        bytes(6) + struct.pack(">H", 1) + bytes(16) + struct.pack(">HH", w, h)
        + struct.pack(">IIIH", 0x480000, 0x480000, 0, 1)
        + bytes([len(name)]) + name.ljust(31, b"\0") + struct.pack(">Hh", 0x18, -1)
        + box(b"jxpl", struct.pack(">HH", ppih, plev))
        # Video information: maximum bit rate (Mbit/s), frame rate (frat: interlace 0,
        # denominator code 1 for /1.000, rate), sample characteristics, time code.
        + box(b"jpvi", struct.pack(">IIHI", 0, (1 << 24) | rate, 0, 0))
        + nclx(1, 1, 1, 0)
    )
    stsd = full(b"stsd", 0, 0, struct.pack(">I", 1) + box(b"jxsm", entry))
    stts = full(b"stts", 0, 0, struct.pack(">III", 1, n, 1))
    stsc = full(b"stsc", 0, 0, struct.pack(">IIII", 1, 1, n, 1))
    stsz = full(b"stsz", 0, 0, struct.pack(">II", 0, n) + b"".join(struct.pack(">I", len(f)) for f in frames))

    def moov(chunk):
        stco = full(b"stco", 0, 0, struct.pack(">II", 1, chunk))
        stbl = box(b"stbl", stsd + stts + stsc + stsz + stco)
        minf = box(b"minf", vmhd + dinf + stbl)
        mdia = box(b"mdia", mdhd + hdlr + minf)
        return box(b"moov", mvhd + box(b"trak", tkhd + mdia))

    first = len(ftyp) + len(moov(0)) + 8
    return ftyp + moov(first) + box(b"mdat", b"".join(frames))


def manifest(d):
    rows = []
    try:
        with open(os.path.join(d, "manifest.tsv")) as f:
            for line in f:
                if line.startswith("#") or not line.strip():
                    continue
                rows.append(line.rstrip("\n").split("\t"))
    except FileNotFoundError:
        pass
    return rows


def corpus(root):
    head = "# file, outcome, samples, SHA-256 of the reference decoder's samples, note\n"
    out = os.path.join(root, "heif")
    os.makedirs(out, exist_ok=True)
    rows = []
    for sub in ("synth", "conformance"):
        for r in manifest(os.path.join(root, sub)):
            if r[1] != "ok":
                continue
            data = open(os.path.join(root, sub, r[0]), "rb").read()
            if not data.startswith(b"\xff\x10"):
                continue  # already a JPEG XS image file
            name = f"{sub}-{r[0].replace('/', '_').rsplit('.', 1)[0]}.jxsi"
            k = len(rows)
            # Every third file in two extents, every fifth inside the meta box.
            open(os.path.join(out, name), "wb").write(heif(data, split=k % 3 == 1, idat=k % 5 == 2))
            rows.append([name] + r[1:])
    with open(os.path.join(out, "manifest.tsv"), "w") as f:
        f.write(head)
        for r in rows:
            f.write("\t".join(r) + "\n")
    out = os.path.join(root, "mp4")
    os.makedirs(out, exist_ok=True)
    seqs = {}
    for r in manifest(os.path.join(root, "sequences")):
        seqs.setdefault(r[0].split("_frame_")[0], []).append(r)
    rows = []
    for prefix, frames in sorted(seqs.items()):
        data = [open(os.path.join(root, "sequences", r[0]), "rb").read() for r in frames]
        name = f"{prefix}.mp4"
        open(os.path.join(out, name), "wb").write(mp4(data))
        for i, r in enumerate(frames):
            rows.append([f"{name}#{i}"] + r[1:])
    with open(os.path.join(out, "manifest.tsv"), "w") as f:
        f.write(head)
        for r in rows:
            f.write("\t".join(r) + "\n")
    print(f"jpegxs-wrap: {len(os.listdir(os.path.join(root, 'heif'))) - 1} HEIF files, "
          f"{len(seqs)} MP4 movies")


def main(a):
    if len(a) >= 3 and a[0] == "heif":
        opts = dict(irot=None, imir=None, clap=None, split=False, idat=False, colour=(1, 1, 1, 0))
        i = 3
        while i < len(a):
            o = a[i]
            if o == "--irot":
                opts["irot"] = int(a[i + 1]); i += 2
            elif o == "--imir":
                opts["imir"] = int(a[i + 1]); i += 2
            elif o == "--clap":
                opts["clap"] = (int(a[i + 1]), int(a[i + 2])); i += 3
            elif o == "--split":
                opts["split"] = True; i += 1
            elif o == "--idat":
                opts["idat"] = True; i += 1
            elif o == "--nclx":
                p, t, m, fr = (int(x) for x in a[i + 1].split(","))
                opts["colour"] = (p, t, m, fr); i += 2
            else:
                raise SystemExit(f"unknown option {o}")
        data = open(a[1], "rb").read()
        open(a[2], "wb").write(heif(data, **opts))
    elif len(a) >= 3 and a[0] == "mp4":
        open(a[1], "wb").write(mp4([open(p, "rb").read() for p in a[2:]]))
    elif len(a) == 2 and a[0] == "corpus":
        corpus(a[1])
    else:
        raise SystemExit(__doc__)


if __name__ == "__main__":
    main(sys.argv[1:])
