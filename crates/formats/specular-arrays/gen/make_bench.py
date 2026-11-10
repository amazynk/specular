"""Large array files for timing opens and planes (not committed).

    python -I make_bench.py OUT_DIR
"""

import os
import sys

import numpy as np

OUT = sys.argv[1]
os.makedirs(OUT, exist_ok=True)
rng = np.random.default_rng(5)
img = (rng.standard_normal((4096, 4096)) * 1000).astype("<f4")
np.save(os.path.join(OUT, "big_f4.npy"), img)
np.save(os.path.join(OUT, "big_f4_F.npy"), np.asfortranarray(img))
np.savez_compressed(os.path.join(OUT, "big_z.npz"), img=img[:2048, :2048], vol=rng.integers(0, 4000, (32, 512, 512)).astype("<u2"))

import scipy.io  # noqa: E402

scipy.io.savemat(os.path.join(OUT, "big.mat"), {"img": img.astype("<f8"), "vol": rng.integers(0, 255, (512, 512, 64)).astype("u1")})
scipy.io.savemat(os.path.join(OUT, "big_z.mat"), {"img": img.astype("<f8")}, do_compression=True)

import zarr  # noqa: E402
import zarr.codecs as zc  # noqa: E402

for fmt, comp in [(2, None), (3, zc.ZstdCodec(level=3)), (3, zc.BloscCodec(cname="lz4", shuffle="shuffle"))]:
    name = f"big_v{fmt}_{type(comp).__name__ if comp else 'none'}.zarr"
    root = zarr.open_group(os.path.join(OUT, name), mode="w", zarr_format=fmt)
    kw = {} if comp is None else {"compressors": comp}
    if fmt == 2 and comp is None:
        kw = {"compressors": None}
    z = root.create_array("img", shape=img.shape, dtype=img.dtype, chunks=(512, 512), **kw)
    z[...] = img
print("written to", OUT)
