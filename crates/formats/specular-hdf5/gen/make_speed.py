"""Large-ish HDF5 / NetCDF files for timing the open and first-plane decode.

Usage: make_speed.py OUT_DIR

Writes (about 150 MB in all):
  speed_f32_deflate.h5   4096 x 4096 float32, 256 x 256 chunks, shuffle + deflate 4
  speed_u16_contig.h5    4096 x 4096 uint16, contiguous
  speed_cube.nc          NetCDF-4, 8 x 2048 x 2048 float32, zlib (needs netCDF4)
  speed_cdf2.nc          classic CDF-2, 8 x 2048 x 2048 int16 packed (needs netCDF4)
"""

import os
import sys

import h5py
import numpy as np

try:
    import netCDF4
except ImportError:  # the NetCDF files are skipped
    netCDF4 = None


def field(shape, seed):
    rng = np.random.default_rng(seed)
    y, x = np.mgrid[0 : shape[-2], 0 : shape[-1]].astype(np.float32)
    base = np.sin(x / 97.0) * np.cos(y / 61.0) * 1000.0 + x * 0.05
    noise = rng.normal(0.0, 3.0, shape[-2:]).astype(np.float32)
    return (base + noise).astype(np.float32)


def main(out):
    os.makedirs(out, exist_ok=True)
    img = field((4096, 4096), 1)
    with h5py.File(os.path.join(out, "speed_f32_deflate.h5"), "w", libver="latest") as f:
        f.create_dataset("data", data=img, chunks=(256, 256), shuffle=True,
                         compression="gzip", compression_opts=4)
    with h5py.File(os.path.join(out, "speed_u16_contig.h5"), "w") as f:
        f.create_dataset("image", data=(img - img.min()).astype(np.uint16))
    if netCDF4 is None:
        return
    small = field((2048, 2048), 2)
    with netCDF4.Dataset(os.path.join(out, "speed_cube.nc"), "w", format="NETCDF4") as ds:
        ds.createDimension("time", None)
        ds.createDimension("y", 2048)
        ds.createDimension("x", 2048)
        v = ds.createVariable("temp", "f4", ("time", "y", "x"), zlib=True,
                              chunksizes=(1, 512, 512))
        for t in range(8):
            v[t] = small + t
    with netCDF4.Dataset(os.path.join(out, "speed_cdf2.nc"), "w",
                         format="NETCDF3_64BIT_OFFSET") as ds:
        ds.createDimension("time", 8)
        ds.createDimension("y", 2048)
        ds.createDimension("x", 2048)
        v = ds.createVariable("temp", "i2", ("time", "y", "x"))
        v.scale_factor = 0.05
        v.add_offset = 0.0
        for t in range(8):
            v[t] = small + t


if __name__ == "__main__":
    main(sys.argv[1])
