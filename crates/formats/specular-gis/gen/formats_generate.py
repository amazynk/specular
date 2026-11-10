"""Generated samples for the GIS formats corpus, written by GDAL (through rasterio) and
pyosmium into OUT/gen/:

  gen/nitf/   NITF 2.1 images: uncompressed (bands interleaved by block and by pixel, 8 and
              16 bits, one and several blocks), JPEG (IC=C3) and JPEG 2000 (IC=C8) blocks,
              IGEOLO in geographic and UTM corners, and an RPC00B model
  gen/cog/    Cloud-Optimized GeoTIFFs (tiled, internal overviews, Deflate and JPEG) for the
              HTTP range-request tests
  gen/pdf/    a GeoPDF in the ISO 32000 encoding (Measure/GEO); GDAL no longer writes the
              OGC best practice (LGIDict), which its own test files cover
  osm/        the Monaco extract as OSM XML (its PBF twin is downloaded)

Usage: python -I formats_generate.py CORPUS_DIR
"""

import os
import sys

import numpy as np
import osmium
import rasterio
import rasterio.shutil
from rasterio.rpc import RPC
from rasterio.transform import from_origin


def pattern(h, w, bands, dtype):
    y, x = np.mgrid[0:h, 0:w]
    out = []
    for b in range(bands):
        v = (x * (3 + b) + y * (5 + 2 * b) + (x // 7) * (y // 5) * (b + 1)) % 251
        if dtype == "uint16":
            v = v * 257 + (x % 13) * 3
        out.append(v.astype(dtype))
    return np.stack(out)


def write(path, data, crs=None, transform=None, **opts):
    """Writes `data` (bands, rows, columns). Drivers that only copy (COG, PDF, compressed
    NITF) get a temporary GeoTIFF first."""
    bands, h, w = data.shape
    driver = opts.pop("driver", "NITF")
    rpcs = opts.pop("rpcs", None)
    copy_only = driver in ("COG", "PDF") or "IC" in opts or rpcs is not None
    profile = {
        "driver": "GTiff" if copy_only else driver,
        "width": w,
        "height": h,
        "count": bands,
        "dtype": data.dtype.name,
    }
    if crs:
        profile["crs"] = crs
    if transform is not None:
        profile["transform"] = transform
    target = path + ".tmp.tif" if copy_only else path
    if not copy_only:
        profile.update(opts)
    with rasterio.open(target, "w", **profile) as dst:
        dst.write(data)
        if rpcs is not None:
            dst.rpcs = rpcs
    if copy_only:
        rasterio.shutil.copy(target, path, driver=driver, **opts)
        os.remove(target)


def nitf(out):
    d = os.path.join(out, "gen", "nitf")
    os.makedirs(d, exist_ok=True)
    geo = from_origin(7.0, 47.0, 0.001, 0.001)
    utm = from_origin(500000.0, 5200000.0, 10.0, 10.0)
    write(f"{d}/rgb_geo.ntf", pattern(200, 300, 3, "uint8"), "EPSG:4326", geo, ICORDS="G")
    write(f"{d}/rgb_pixel_interleaved.ntf", pattern(130, 170, 3, "uint8"), "EPSG:4326", geo, ICORDS="G", IMODE="P")
    write(f"{d}/grey16_utm.ntf", pattern(150, 140, 1, "uint16"), "EPSG:32632", utm, ICORDS="N")
    write(f"{d}/blocked.ntf", pattern(600, 520, 1, "uint8"), "EPSG:32632", utm, ICORDS="N", BLOCKXSIZE=256, BLOCKYSIZE=256)
    write(f"{d}/rgb_jpeg.ntf", pattern(256, 384, 3, "uint8"), "EPSG:4326", geo, ICORDS="G", IC="C3", QUALITY=90)
    write(f"{d}/grey_jpeg_blocked.ntf", pattern(512, 512, 1, "uint8"), "EPSG:4326", geo, ICORDS="G", IC="C3", BLOCKXSIZE=256, BLOCKYSIZE=256)
    try:
        write(f"{d}/rgb_j2k.ntf", pattern(240, 320, 3, "uint8"), "EPSG:4326", geo, ICORDS="G", IC="C8", JPEG2000_DRIVER="JP2OpenJPEG")
    except Exception as e:  # noqa: BLE001 - no JPEG 2000 driver in this GDAL
        print(f"  no JPEG 2000 NITF: {e}")
    rpcs = RPC(
        height_off=200.0,
        height_scale=500.0,
        lat_off=46.95,
        lat_scale=0.05,
        line_num_coeff=[0.0, -1.0, 0.0, 0.01] + [0.0] * 16,
        line_den_coeff=[1.0] + [0.0] * 19,
        line_off=100.0,
        line_scale=100.0,
        long_off=7.45,
        long_scale=0.05,
        samp_num_coeff=[0.0, 0.0, 1.0, 0.0] + [0.0] * 16,
        samp_den_coeff=[1.0] + [0.0] * 19,
        samp_off=150.0,
        samp_scale=150.0,
    )
    write(f"{d}/rpc.ntf", pattern(200, 300, 1, "uint8"), rpcs=rpcs)
    print(f"  {len(os.listdir(d))} NITF files")


def cog(out):
    d = os.path.join(out, "gen", "cog")
    os.makedirs(d, exist_ok=True)
    t = from_origin(500000.0, 5200000.0, 2.0, 2.0)
    write(f"{d}/utm_deflate.tif", pattern(1500, 1800, 1, "uint16"), "EPSG:32632", t, driver="COG", COMPRESS="DEFLATE", BLOCKSIZE=256)
    write(f"{d}/rgb_jpeg.tif", pattern(1024, 1280, 3, "uint8"), "EPSG:4326", from_origin(7.0, 47.0, 1e-4, 1e-4), driver="COG", COMPRESS="JPEG", BLOCKSIZE=512)
    print(f"  {len(os.listdir(d))} COGs")


def pdf(out):
    d = os.path.join(out, "gen", "pdf")
    os.makedirs(d, exist_ok=True)
    t = from_origin(500000.0, 5200000.0, 10.0, 10.0)
    # GDAL no longer writes the OGC best practice (LGIDict); its test files cover that one.
    for enc in ("ISO32000",):
        try:
            write(f"{d}/geo_{enc.lower()}.pdf", pattern(200, 260, 3, "uint8"), "EPSG:32632", t, driver="PDF", GEO_ENCODING=enc)
        except Exception as e:  # noqa: BLE001
            print(f"  no {enc} GeoPDF: {e}")
    print(f"  {len(os.listdir(d))} GeoPDFs")


def osm_xml(out):
    src = os.path.join(out, "osm", "monaco-250101.osm.pbf")
    dst = os.path.join(out, "osm", "monaco-250101.osm")
    if not os.path.exists(src):
        return
    if os.path.exists(dst):
        os.remove(dst)
    w = osmium.SimpleWriter(dst)

    class Copy(osmium.SimpleHandler):
        def node(self, n):
            w.add_node(n)

        def way(self, x):
            w.add_way(x)

        def relation(self, r):
            w.add_relation(r)

    Copy().apply_file(src)
    w.close()
    print(f"  {dst}: {os.path.getsize(dst) // 1024} KB")


def main():
    out = sys.argv[1]
    # No .aux.xml sidecars: the files must carry everything themselves.
    with rasterio.Env(GDAL_PAM_ENABLED="NO"):
        nitf(out)
        cog(out)
        pdf(out)
    osm_xml(out)


if __name__ == "__main__":
    main()
