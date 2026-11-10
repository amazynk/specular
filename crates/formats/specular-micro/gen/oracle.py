#!/usr/bin/env python3
"""Write the expected JSON for the LIF / LOF / ND2 files in a directory, from the reference
readers: liffile (cross-checked against readlif where it can read the file) and nd2.

    python oracle.py <dir> [outdir]

Each file `name.ext` gets `outdir/name.ext.json` (outdir defaults to `<dir>/expected`) with
the structure the Rust test compares (images, dimensions, channels, colours, calibration,
loops) and an FNV-1a hash per plane: every sample as a little-endian u32 (integers) or its
f32 bits (floats), rows top-down.
"""

from __future__ import annotations

import itertools
import json
import sys
import warnings
from pathlib import Path

import numpy as np

FNV_OFFSET, FNV_PRIME, MASK = 0xCBF29CE484222325, 0x100000001B3, (1 << 64) - 1


def plane_hash(a: np.ndarray) -> str:
    a = np.ascontiguousarray(a)
    if a.dtype.kind == "f":
        raw = a.astype("<f4").tobytes()
    else:
        raw = a.astype("<u4").tobytes()
    h = FNV_OFFSET
    for b in raw:
        h = ((h ^ b) * FNV_PRIME) & MASK
    return f"{h:016x}"


def lif_expected(path: Path) -> dict:
    import liffile

    out = {"kind": "lif", "images": []}
    rl_agree = rl_total = 0
    try:
        from readlif.reader import LifFile as RL

        rl = RL(str(path)) if path.suffix.lower() == ".lif" else None
    except Exception:  # noqa: BLE001
        rl = None
    with liffile.LifFile(path, squeeze=False) as lf:
        out["lof"] = lf.type.name == "LOF"
        out["version"] = int(lf.version or 0)
        for n, img in enumerate(lf.images):
            if type(img).__name__ != "LifImage":
                continue
            dims = [
                {
                    "code": d.label,
                    "count": d.number_elements,
                    "origin": d.origin,
                    "length": d.length,
                    "unit": d.unit,
                }
                for d in sorted(img._dimensions, key=lambda d: d.bytes_inc)
            ]
            chans = [
                {"lut": c.lut_name, "bits": c.resolution, "float": c.data_type == 1}
                for c in img._channels
            ]
            names = list(img._channel_names)
            if img.sizes.get("C", 0) > 1 and img.sizes.get("S", 0) > 1:
                # liffile numbers RGB entries' channels Ch0…; name them by ChannelTag (1 red,
                # 2 green, 3 blue) in file order, as for a single RGB entry.
                names = [
                    ("Red", "Green", "Blue")[c.channel_tag - 1] if 0 < c.channel_tag < 4 else f"Ch{i}"
                    for i, c in enumerate(img._channels)
                ]
            for c, nm in zip(chans, names, strict=False):
                c["name"] = nm
            entry = {"path": img.path, "name": img.name, "dims": dims, "channels": chans, "planes": {}}
            x = img.asxarray()
            ch_axis = next((a for a in ("C", "S") if a in x.dims), None)
            extra = [d for d in dims if d["code"] not in ("X", "Y")]
            for c in range(len(chans)):
                for idx in itertools.product(*[range(d["count"]) for d in extra]):
                    sel = {d["code"]: i for d, i in zip(extra, idx, strict=True)}
                    if "C" in x.dims and "S" in x.dims:
                        # RGB channels (liffile: C entries of S samples each): channel c, in
                        # file (BytesInc) order, is sample c % S of entry c // S; liffile
                        # turns BGR samples around to RGB.
                        ns = x.sizes["S"]
                        s = c % ns
                        if getattr(img, "_is_bgr", False):
                            s = ns - 1 - s
                        sel["C"], sel["S"] = c // ns, s
                    elif ch_axis:
                        sel[ch_axis] = c
                    plane = x.isel(sel).transpose("Y", "X").values if "Y" in x.dims else x.isel(sel).values[None, :]
                    entry["planes"][f"{c}:{','.join(map(str, idx))}"] = plane_hash(plane)
                    # readlif addresses (z, t, c, m) only.
                    if rl is not None and {d["code"] for d in extra} <= {"Z", "T", "M"}:
                        try:
                            ri = rl.get_image(n)
                            fr = np.asarray(
                                ri.get_frame(z=sel.get("Z", 0), t=sel.get("T", 0), c=c, m=sel.get("M", 0))
                            )
                            rl_total += 1
                            rl_agree += int(plane_hash(fr) == entry["planes"][f"{c}:{','.join(map(str, idx))}"])
                        except Exception:  # noqa: BLE001
                            pass
            out["images"].append(entry)
    out["readlif"] = {"agree": rl_agree, "total": rl_total}
    return out


def nd2_expected(path: Path) -> dict:
    import nd2

    with nd2.ND2File(path) as f:
        a = f.attributes
        loops = [
            {
                "kind": {"TimeLoop": "time", "NETimeLoop": "time", "XYPosLoop": "position", "ZStackLoop": "z"}.get(
                    lp.type, "custom"
                ),
                "count": lp.count,
            }
            for lp in f.experiment
        ]
        for lp, src in zip(loops, f.experiment, strict=True):
            if src.type == "XYPosLoop":
                lp["labels"] = [p.name or "" for p in src.parameters.points]
            if src.type == "ZStackLoop":
                lp["step"] = src.parameters.stepUm
        chans = []
        for c in f.metadata.channels:
            col = c.channel.color
            chans.append(
                {
                    "name": c.channel.name,
                    "colour": [col.r, col.g, col.b],
                    "components": c.volume.componentCount,
                    "emission": c.channel.emissionLambdaNm,
                }
            )
        vx = f.voxel_size()
        out = {
            "kind": "nd2",
            "version": list(f.version),
            "width": a.widthPx,
            "height": a.heightPx,
            "components": a.componentCount,
            "bits": a.bitsPerComponentSignificant,
            "frames": len(f.loop_indices) if f.loop_indices else a.sequenceCount,
            "loops": loops,
            "channels": chans,
            "calibration": vx.x,
            "text": dict(f.text_info),
            "planes": {},
        }
        n = out["frames"]
        for i in range(n):
            fr = np.asarray(f._rdr.read_frame(i)).reshape(a.heightPx, a.widthPx, -1)
            for k in range(fr.shape[2]):
                out["planes"][f"{i}:{k}"] = plane_hash(fr[:, :, k])
    return out


def main() -> None:
    src = Path(sys.argv[1])
    dst = Path(sys.argv[2]) if len(sys.argv) > 2 else src / "expected"
    dst.mkdir(parents=True, exist_ok=True)
    warnings.simplefilter("ignore")
    for p in sorted(src.iterdir()):
        ext = p.suffix.lower()
        try:
            if ext in (".lif", ".lof"):
                exp = lif_expected(p)
            elif ext == ".nd2":
                exp = nd2_expected(p)
            else:
                continue
        except Exception as e:  # noqa: BLE001
            print(f"{p.name}: oracle failed: {e}")
            continue
        (dst / (p.name + ".json")).write_text(json.dumps(exp, indent=1, ensure_ascii=False) + "\n")
        extra = f" readlif {exp['readlif']['agree']}/{exp['readlif']['total']}" if "readlif" in exp else ""
        print(f"{p.name}: {len(exp.get('planes', {})) or sum(len(i['planes']) for i in exp.get('images', []))} planes{extra}")


if __name__ == "__main__":
    main()
