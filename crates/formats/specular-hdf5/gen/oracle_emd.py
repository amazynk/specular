"""Expected Velox / FEI EMD images for tests/corpus_emd.rs, from RosettaSciIO.

Run with an isolated interpreter from the matsci lane venv:
    python -I gen/oracle_emd.py <corpus>/rsciio/emd
Writes <dir>/oracle_emd.json: per file, per image signal, the title, data shape (frames, rows,
columns), dtype, axes and, for the first, middle and last frame, the sum and a few pixels
(magnitudes for complex data). Spectrum images are not decoded: their spectrum stream (which
needs `sparse`) is skipped, the images beside it are kept with all their frames.
"""

import json
import pathlib
import sys
import types

import numpy as np
from rsciio import emd
from rsciio.emd import _emd_velox


class _Finder:
    @staticmethod
    def find_spec(name):
        return object()


# Images only: pretend `sparse` is present and leave the spectrum stream unread.
_emd_velox.importlib = types.SimpleNamespace(util=_Finder)
_emd_velox.FeiEMDReader._read_spectrum_stream = lambda self: None


def describe(sig):
    data = np.asarray(sig["data"])
    if np.iscomplexobj(data):
        data = np.abs(data)
    h, w = data.shape[-2:]
    planes = data.reshape(-1, h, w).astype(np.float64)
    return {
        "title": str(sig["metadata"].get("General", {}).get("title", "")),
        "shape": [planes.shape[0], h, w],
        "dtype": str(np.asarray(sig["data"]).dtype),
        "axes": [
            {
                "name": str(a.get("name", "")),
                "size": int(a["size"]),
                "offset": float(a.get("offset", 0.0)),
                "scale": float(a.get("scale", 1.0)),
                "units": str(a.get("units", "")),
            }
            for a in sig["axes"]
        ],
        "planes": [
            {
                "index": k,
                "sum": float(planes[k].sum()),
                "pixels": [
                    [x, y, float(planes[k][y, x])]
                    for (x, y) in [(0, 0), (w - 1, h - 1), (w // 3, h // 2), (w - 1, 0)]
                ],
            }
            for k in sorted({0, len(planes) // 2, len(planes) - 1})
        ],
    }


def main():
    root = pathlib.Path(sys.argv[1])
    result = {}
    for p in sorted(root.rglob("*.emd")):
        key = str(p.relative_to(root))
        try:
            sigs = emd.file_reader(str(p), select_type="images", load_SI_image_stack=True)
            result[key] = [describe(s) for s in sigs if len(np.shape(s["data"])) >= 2]
        except Exception as e:  # noqa: BLE001
            result[key] = {"error": repr(e)}
    (root / "oracle_emd.json").write_text(json.dumps(result, indent=1))
    print(f"{len(result)} files")


if __name__ == "__main__":
    main()
