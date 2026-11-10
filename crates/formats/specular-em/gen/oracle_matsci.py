"""Expected values for the TIA and Bruker BCF corpus (tests/corpus_matsci.rs), from RosettaSciIO.

Run with an isolated interpreter from the oracle venv:
    python -I gen/oracle_matsci.py <dir>
for the corpus (<corpus>/rsciio) and the fixtures (tests/data/matsci). Writes
<dir>/oracle_matsci.json: per file, per signal, the data shape, the
axes and sample values (plane sums and pixels for images; totals and whole spectra at a few
positions for spectra).
"""

import json
import pathlib
import sys

import numpy as np
from rsciio import bruker, tia


def axes(sig):
    return [
        {
            "name": str(a.get("name", "")),
            "size": int(a["size"]),
            "offset": float(a.get("offset", 0.0)),
            "scale": float(a.get("scale", 1.0)),
            "units": str(a.get("units", "")),
            "navigate": bool(a.get("navigate", False)),
        }
        for a in sig["axes"]
    ]


def describe(sig):
    data = np.asarray(sig["data"])
    ax = axes(sig)
    signal_dims = sum(1 for a in ax if not a["navigate"])
    out = {
        "shape": list(data.shape),
        "axes": ax,
        "title": str(sig["metadata"].get("General", {}).get("title", "")),
    }
    if signal_dims == 2:
        h, w = data.shape[-2:]
        planes = data.reshape(-1, h, w)
        out["kind"] = "image"
        out["planes"] = [
            {
                "index": k,
                "sum": float(np.nansum(planes[k].astype(np.float64))),
                "pixels": [
                    [x, y, float(planes[k][y, x])]
                    for (x, y) in [(0, 0), (w - 1, h - 1), (w // 3, h // 2), (w - 1, 0)]
                ],
            }
            for k in sorted({0, len(planes) // 2, len(planes) - 1})
        ]
    else:
        c = data.shape[-1]
        nav = data.shape[:-1]
        width = nav[-1] if len(nav) >= 1 else 1
        flat = data.reshape(-1, c).astype(np.float64)
        out["kind"] = "spectrum"
        out["width"] = int(width)
        out["totals"] = [float(v) for v in np.nansum(flat, axis=1)[:256]]
        n = flat.shape[0]
        out["spectra"] = [
            {"index": int(i), "x": int(i % width), "y": int(i // width), "values": [float(v) for v in flat[i]]}
            for i in sorted({0, n // 2, n - 1})
        ]
    return out


def main():
    root = pathlib.Path(sys.argv[1])
    result = {}
    for p in sorted(root.rglob("*.ser")):
        key = str(p.relative_to(root))
        try:
            result[key] = [describe(s) for s in tia.file_reader(str(p))]
        except Exception as e:  # noqa: BLE001
            result[key] = {"error": repr(e)}
    for p in sorted(root.rglob("*.bcf")):
        key = str(p.relative_to(root))
        try:
            sigs = bruker.file_reader(str(p), select_type=None, index="all")
            result[key] = [describe(s) for s in sigs]
        except Exception as e:  # noqa: BLE001
            result[key] = {"error": repr(e)}
    (root / "oracle_matsci.json").write_text(json.dumps(result, indent=1))
    print(f"{len(result)} files")


if __name__ == "__main__":
    main()
