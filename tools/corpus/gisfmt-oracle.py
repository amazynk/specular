#!/usr/bin/env python3
"""Oracle for the GIS formats of fmt500d items 18 and 19 (tools/corpus/fetch-gisfmt.sh):
writes more samples beside the fetched ones (ER Mapper, SAGA and BSB through rasterio,
VRTs over GeoTIFFs it writes, IGC flights and NMEA logs) and records under `expected/`
what rasterio 1.5.2 (GDAL), pyogrio 0.13.0 (GDAL) and pynmea2 read from every file:

    $CARGO_TARGET_DIR/venv/bin/python -I tools/corpus/gisfmt-oracle.py $CARGO_TARGET_DIR/corpus/gisfmt

Rasters: `expected/<file>.json` (size, bands, data types, no-data, transform in GDAL's
order, CRS as EPSG code and WKT, GCPs, colour table) and `<file>.b<band>.f64` (each band's
values as little-endian float64, row by row, no-data as stored). Vectors:
`expected/<file>.json` with every layer's features (geometry as nested coordinate lists from
the WKB, and attributes). Corpus files are only read by rasterio and pyogrio, never run.
"""
import json
import math
import os
import random
import struct
import sys

import numpy as np
import pynmea2
import pyogrio
import pyogrio.raw
import rasterio
import rasterio.transform
import rasterio.warp
from rasterio.control import GroundControlPoint
from rasterio.transform import Affine

out = sys.argv[1]
exp = os.path.join(out, "expected")
os.makedirs(exp, exist_ok=True)


def rel(p):
    return os.path.relpath(p, out)


def put_json(path, obj):
    dest = os.path.join(exp, rel(path) + ".json")
    os.makedirs(os.path.dirname(dest), exist_ok=True)
    with open(dest, "w") as f:
        json.dump(obj, f, indent=1, sort_keys=True, default=str)


def crs_of(c):
    if c is None:
        return None
    try:
        epsg = c.to_epsg()
    except Exception:  # noqa: BLE001 - CRSs without a code
        epsg = None
    return {"epsg": epsg, "wkt": c.to_wkt()}


def raster(path, open_path=None):
    with rasterio.open(open_path or path) as ds:
        info = {
            "driver": ds.driver,
            "width": ds.width,
            "height": ds.height,
            "count": ds.count,
            "dtypes": list(ds.dtypes),
            # NaN is not JSON: written as the text "nan".
            "nodata": [None if v is None else ("nan" if math.isnan(v) else float(v))
                       for v in ds.nodatavals],
            "transform": list(ds.transform.to_gdal()),
            "has_transform": not ds.transform.is_identity,
            "crs": crs_of(ds.crs),
            "gcps": [],
            "colormap": None,
            "scales": list(ds.scales),
            "offsets": list(ds.offsets),
            "descriptions": list(ds.descriptions),
            "units": list(ds.units),
        }
        gcps, gcrs = ds.gcps
        info["gcps"] = [[g.col, g.row, g.x, g.y] for g in gcps]
        info["gcp_crs"] = crs_of(gcrs)
        # Longitude and latitude at a grid of pixel positions, as GDAL places the raster
        # (its transform, else an affine fit of its GCPs).
        src_crs, tr = ds.crs, ds.transform
        if tr.is_identity and gcps:
            tr, src_crs = rasterio.transform.from_gcps(gcps), gcrs
        src_crs = src_crs or gcrs
        if not tr.is_identity and src_crs is not None:
            px = [(ds.width * i / 4.0, ds.height * j / 4.0) for i in range(5) for j in range(5)]
            xs, ys = zip(*[tr * p for p in px])
            lon, lat = rasterio.warp.transform(src_crs, "EPSG:4326", xs, ys)
            info["lonlat"] = [[p[0], p[1], a, b] for p, a, b in zip(px, lon, lat)]
        try:
            cm = ds.colormap(1)
            info["colormap"] = [list(cm[i]) for i in range(max(cm) + 1)]
        except ValueError:
            pass
        for b in range(1, ds.count + 1):
            a = ds.read(b).astype("<f8")
            dest = os.path.join(exp, rel(path) + ".b%d.f64" % b)
            os.makedirs(os.path.dirname(dest), exist_ok=True)
            with open(dest, "wb") as f:
                f.write(a.tobytes())
    put_json(path, info)
    return info


def wkb(b):
    """Coordinates of a WKB geometry as nested lists (2-D), with its type name."""
    pos = [0]

    def u8():
        v = b[pos[0]]
        pos[0] += 1
        return v

    def geom():
        le = u8() == 1
        e = "<" if le else ">"
        (t,) = struct.unpack(e + "I", b[pos[0]:pos[0] + 4])
        pos[0] += 4
        base = t % 1000
        dims = 2
        if t & 0x80000000 or (1000 <= t < 2000) or (2000 <= t < 3000):
            dims = 3
        if 3000 <= t < 4000:
            dims = 4
        if t & 0x80000000:
            base = t & 0xFF

        def pt():
            v = struct.unpack(e + "d" * dims, b[pos[0]:pos[0] + 8 * dims])
            pos[0] += 8 * dims
            return list(v[:2])

        def n():
            (k,) = struct.unpack(e + "I", b[pos[0]:pos[0] + 4])
            pos[0] += 4
            return k

        if base == 1:
            return ["Point", pt()]
        if base == 2:
            return ["LineString", [pt() for _ in range(n())]]
        if base == 3:
            return ["Polygon", [[pt() for _ in range(n())] for _ in range(n())]]
        if base in (4, 5, 6, 7):
            parts = [geom() for _ in range(n())]
            name = {4: "MultiPoint", 5: "MultiLineString", 6: "MultiPolygon", 7: "GeometryCollection"}[base]
            return [name, [p[1] for p in parts] if base != 7 else parts]
        raise ValueError("WKB type %d" % t)

    return geom()


def jsonable(v):
    if v is None:
        return None
    if isinstance(v, (np.integer,)):
        return int(v)
    if isinstance(v, (np.floating, float)):
        f = float(v)
        return None if math.isnan(f) else f
    if isinstance(v, np.ndarray):
        return [jsonable(x) for x in v.tolist()]
    if isinstance(v, (list, tuple)):
        return [jsonable(x) for x in v]
    if isinstance(v, bytes):
        return v.hex()
    return v if isinstance(v, (int, str, bool)) else str(v)


def vector(path):
    layers = []
    for name, gtype in pyogrio.list_layers(path):
        meta, fids, geoms, fields = pyogrio.raw.read(path, layer=name)
        count = len(geoms) if geoms is not None else (len(fields[0]) if len(fields) else 0)
        feats = []
        for i in range(count):
            g = geoms[i] if geoms is not None else None
            feats.append(
                {
                    "fid": int(fids[i]) if fids is not None else i,
                    "geometry": wkb(bytes(g)) if g is not None else None,
                    "fields": {
                        f: jsonable(fields[k][i]) for k, f in enumerate(meta["fields"])
                    },
                }
            )
        layers.append({"name": name, "type": gtype, "features": feats, "crs": meta.get("crs")})
    put_json(path, {"layers": layers})


def write(path, arr, profile, **kw):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    p = dict(profile)
    p.update(kw)
    with rasterio.open(path, "w", **p) as ds:
        if arr.ndim == 2:
            ds.write(arr, 1)
        else:
            ds.write(arr)


def generate():
    gen = os.path.join(out, "gen")
    rng = np.random.default_rng(8211)
    # ER Mapper: 3 bands of 16-bit on UTM, and float with no-data on lon/lat.
    a = rng.integers(0, 4000, size=(3, 37, 53), dtype=np.uint16)
    write(os.path.join(gen, "ers/utm3.ers"), a, {"driver": "ERS", "width": 53, "height": 37,
          "count": 3, "dtype": "uint16", "crs": "EPSG:32633",
          "transform": Affine(30.0, 0, 500000.0, 0, -30.0, 4650000.0)})
    f = rng.normal(100.0, 20.0, size=(29, 41)).astype(np.float32)
    f[3:6, 7:9] = -9999.0
    write(os.path.join(gen, "ers/float.ers"), f, {"driver": "ERS", "width": 41, "height": 29,
          "count": 1, "dtype": "float32", "crs": "EPSG:4326", "nodata": -9999.0,
          "transform": Affine(0.01, 0, 10.0, 0, -0.01, 47.5)})
    i32 = rng.integers(-100000, 100000, size=(11, 13), dtype=np.int32)
    write(os.path.join(gen, "ers/int32.ers"), i32, {"driver": "ERS", "width": 13, "height": 11,
          "count": 1, "dtype": "int32", "crs": "EPSG:26711",
          "transform": Affine(2.0, 0, 440000.0, 0, -2.0, 3750000.0)})
    # SAGA: 16-bit with no-data, double, byte.
    s = rng.integers(-500, 3000, size=(23, 31), dtype=np.int16)
    s[0, :4] = -32767
    write(os.path.join(gen, "saga/int16.sdat"), s, {"driver": "SAGA", "width": 31, "height": 23,
          "count": 1, "dtype": "int16", "nodata": -32767, "crs": "EPSG:32632",
          "transform": Affine(25.0, 0, 600000.0, 0, -25.0, 5300000.0)})
    d = rng.normal(0, 1, size=(17, 19)).astype(np.float64)
    write(os.path.join(gen, "saga/double.sdat"), d, {"driver": "SAGA", "width": 19, "height": 17,
          "count": 1, "dtype": "float64", "transform": Affine(0.5, 0, -3.0, 0, -0.5, 9.0)})
    by = rng.integers(0, 255, size=(9, 14), dtype=np.uint8)
    write(os.path.join(gen, "saga/byte.sdat"), by, {"driver": "SAGA", "width": 14, "height": 9,
          "count": 1, "dtype": "uint8", "transform": Affine(1.0, 0, 0.0, 0, -1.0, 9.0)})
    # GeoTIFFs and VRTs over them.
    vdir = os.path.join(gen, "vrt")
    rgb = rng.integers(0, 255, size=(3, 40, 60), dtype=np.uint8)
    write(os.path.join(vdir, "rgb.tif"), rgb, {"driver": "GTiff", "width": 60, "height": 40,
          "count": 3, "dtype": "uint8", "crs": "EPSG:4326",
          "transform": Affine(0.1, 0, 5.0, 0, -0.1, 45.0)})
    t16 = rng.integers(-1000, 1000, size=(40, 60), dtype=np.int16)
    t16[5, 5:10] = -999
    write(os.path.join(vdir, "dem.tif"), t16, {"driver": "GTiff", "width": 60, "height": 40,
          "count": 1, "dtype": "int16", "crs": "EPSG:4326",
          "transform": Affine(0.1, 0, 5.0, 0, -0.1, 45.0)})
    t2 = rng.integers(0, 1000, size=(40, 60), dtype=np.int16)
    write(os.path.join(vdir, "dem_east.tif"), t2, {"driver": "GTiff", "width": 60, "height": 40,
          "count": 1, "dtype": "int16", "crs": "EPSG:4326",
          "transform": Affine(0.1, 0, 11.0, 0, -0.1, 45.0)})
    gt = "5.0, 0.1, 0, 45.0, 0, -0.1"
    srs = "<SRS>EPSG:4326</SRS>"

    def band(n, dtype, sources, extra=""):
        return '  <VRTRasterBand dataType="%s" band="%d">%s\n%s  </VRTRasterBand>\n' % (dtype, n, extra, sources)

    def simple(f, b, src, dst, kind="SimpleSource", extra=""):
        return ("    <%s>\n      <SourceFilename relativeToVRT=\"1\">%s</SourceFilename>\n"
                "      <SourceBand>%d</SourceBand>\n      <SrcRect xOff=\"%g\" yOff=\"%g\" xSize=\"%g\" ySize=\"%g\"/>\n"
                "      <DstRect xOff=\"%g\" yOff=\"%g\" xSize=\"%g\" ySize=\"%g\"/>\n%s    </%s>\n"
                % (kind, f, b, *src, *dst, extra, kind))

    def vrt(name, w, h, bands, geo=True):
        with open(os.path.join(vdir, name), "w") as o:
            o.write('<VRTDataset rasterXSize="%d" rasterYSize="%d">\n' % (w, h))
            if geo:
                o.write("  %s\n  <GeoTransform>%s</GeoTransform>\n" % (srs, gt))
            o.write("".join(bands))
            o.write("</VRTDataset>\n")

    full = (0, 0, 60, 40)
    vrt("rgb.vrt", 60, 40, [band(b, "Byte", simple("rgb.tif", b, full, full),
        "\n    <ColorInterp>%s</ColorInterp>\n" % c) for b, c in ((1, "Red"), (2, "Green"), (3, "Blue"))])
    vrt("mosaic.vrt", 120, 40, [band(1, "Int16", simple("dem.tif", 1, full, full)
        + simple("dem_east.tif", 1, full, (60, 0, 60, 40)), "\n    <NoDataValue>-999</NoDataValue>\n")])
    vrt("scaled.vrt", 60, 40, [band(1, "Float32", simple("dem.tif", 1, full, full, "ComplexSource",
        "      <NODATA>-999</NODATA>\n      <ScaleOffset>10</ScaleOffset>\n      <ScaleRatio>0.5</ScaleRatio>\n"),
        "\n    <NoDataValue>-1e30</NoDataValue>\n")])
    vrt("half.vrt", 30, 20, [band(1, "Int16", simple("dem.tif", 1, full, (0, 0, 30, 20)))], geo=False)
    vrt("window.vrt", 25, 15, [band(1, "Int16", simple("dem.tif", 1, (10, 8, 20, 12), (3, 2, 20, 12)))],
        geo=False)
    vrt("swap.vrt", 60, 40, [band(1, "Byte", simple("rgb.tif", 3, full, full)),
                             band(2, "Byte", simple("rgb.tif", 1, full, full))], geo=False)
    # BSB through GDAL's writer, when this build has it.
    try:
        pal = rng.integers(0, 255, size=(20, 3))
        idx = np.zeros((90, 130), dtype=np.uint8)
        for y in range(90):
            x = 0
            while x < 130:
                n = int(rng.integers(1, 60))
                idx[y, x:x + n] = int(rng.integers(1, 20))
                x += n
        path = os.path.join(gen, "bsb/runs.kap")
        gcps = [GroundControlPoint(row=r, col=c, x=-70.0 + c * 0.001, y=42.0 - r * 0.0007)
                for r, c in ((0, 0), (0, 129), (89, 0), (89, 129))]
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with rasterio.open(path, "w", driver="BSB", width=130, height=90, count=1, dtype="uint8",
                           gcps=gcps, crs="EPSG:4326") as ds:
            ds.write(idx, 1)
            ds.write_colormap(1, {i: tuple(int(v) for v in pal[i]) + (255,) for i in range(20)})
    except Exception as e:  # noqa: BLE001 - BSB writing is optional in GDAL
        print("bsb writer:", e)
    # IGC flights (no pip oracle has wheels: the generator's own values are the truth).
    igc_dir = os.path.join(gen, "igc")
    os.makedirs(igc_dir, exist_ok=True)
    for k, (lat0, lon0, start) in enumerate(((46.5, 8.0, 41000), (-33.9, 151.2, 86000))):
        lines = ["AXCS0A%d test logger" % k, "HFDTE150624", "HFPLTPILOTINCHARGE:Test Pilot %d" % k,
                 "HFGTYGLIDERTYPE:Discus", "HFGIDGLIDERID:D-%04d" % k]
        fixes = []
        lat, lon, alt = lat0, lon0, 1200
        t = start
        for i in range(300):
            lat += rng.normal(0, 0.0004)
            lon += rng.normal(0.0003, 0.0004)
            alt += int(rng.integers(-5, 6))
            t += 4
            secs = t % 86400
            hh, mm, ss = secs // 3600, secs // 60 % 60, secs % 60

            def dm(v, w):
                a = abs(v)
                d = int(a)
                m = round((a - d) * 60000)
                if m == 60000:
                    d, m = d + 1, 0
                return ("%0" + str(w) + "d%05d") % (d, m)

            ll = dm(lat, 2) + ("N" if lat >= 0 else "S")
            lo = dm(lon, 3) + ("E" if lon >= 0 else "W")
            lines.append("B%02d%02d%02d%s%sA%05d%05d" % (hh, mm, ss, ll, lo, alt - 10, alt))
            q = lambda s, w: (int(s[:w]) + int(s[w:w + 5]) / 60000.0) * (-1 if s[-1] in "SW" else 1)
            fixes.append([q(lo, 3), q(ll, 2), t - start, alt])
        lines.append("GXXXXXXXX")
        p = os.path.join(igc_dir, "flight%d.igc" % k)
        with open(p, "w", newline="\r\n") as o:
            o.write("\n".join(lines) + "\n")
        put_json(p, {"fixes": fixes, "pilot": "Test Pilot %d" % k, "start_secs": start + 4})
    # NMEA logs, read back by pynmea2.
    nm = os.path.join(gen, "nmea")
    os.makedirs(nm, exist_ok=True)

    def sentence(body):
        c = 0
        for ch in body.encode():
            c ^= ch
        return "$%s*%02X" % (body, c)

    def dm(v, w):
        a = abs(v)
        d = int(a)
        return ("%0" + str(w) + "d%07.4f") % (d, (a - d) * 60)

    lines = []
    lat, lon = 51.4778, -0.0015
    t = 86400 - 120
    for i in range(200):
        lat += 0.0001
        lon += 0.00015
        t += 1
        secs = t % 86400
        day = 14 if t < 86400 else 15
        hms = "%02d%02d%02d.00" % (secs // 3600, secs // 60 % 60, secs % 60)
        la, lo = dm(lat, 2), dm(lon, 3)
        ns, ew = ("N" if lat >= 0 else "S"), ("E" if lon >= 0 else "W")
        lines.append(sentence("GPRMC,%s,A,%s,%s,%s,%s,5.5,45.0,%02d0324,,," % (hms, la, ns, lo, ew, day)))
        lines.append(sentence("GPGGA,%s,%s,%s,%s,%s,1,09,0.9,%.1f,M,47.0,M,," % (hms, la, ns, lo, ew, 30 + i * 0.1)))
        if i == 50:
            lines.append("$GPGGA,garbage*00")
    lines.append(sentence("GPWPL,%s,N,%s,W,HOME" % (dm(51.4778, 2), dm(0.0015, 3))))
    p = os.path.join(nm, "walk.nmea")
    with open(p, "w", newline="\r\n") as o:
        o.write("\n".join(lines) + "\n")
    fixes = []
    for line in lines:
        try:
            m = pynmea2.parse(line, check=True)
        except Exception:  # noqa: BLE001 - the damaged line
            continue
        if isinstance(m, pynmea2.types.talker.RMC):
            fixes.append([m.longitude, m.latitude, m.datetime.isoformat()])
    put_json(p, {"fixes": fixes, "waypoints": 1})


def main():
    generate()
    done = 0
    for d, _, files in sorted(os.walk(out)):
        if os.path.relpath(d, out).split(os.sep)[0] == "expected":
            continue
        for f in sorted(files):
            p = os.path.join(d, f)
            low = f.lower()
            try:
                if low.endswith(".sgrd"):
                    raster(p, p[:-5] + ".sdat")
                elif low.endswith((".kap", ".ers", ".sg-grd-z", ".vrt")) or low == "hdr.adf":
                    raster(os.path.dirname(p) if low == "hdr.adf" else p)
                elif low.endswith((".000", ".topojson")):
                    vector(p)
                else:
                    continue
                done += 1
            except Exception as e:  # noqa: BLE001 - recorded for the test to see
                put_json(os.path.dirname(p) if low == "hdr.adf" else p, {"error": str(e)})
                print("%s: %s" % (rel(p), e))
    print("expected values for %d files in %s" % (done, exp))


main()
