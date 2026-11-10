"""PROJ (through pyproj) as the oracle for the EPSG table, the projections and the datum
shifts of specular-gis.

Usage: python -I proj_oracle.py OUT.jsonl

For every EPSG geographic and projected CRS pyproj knows, five points inside its area of
use (the centre and four points halfway to the corners), each as one line:
  {"code": C, "lon": ..., "lat": ..., "x": ..., "y": ...,       on the CRS's own datum
   "wlon": ..., "wlat": ..., "wx": ..., "wy": ...}              from WGS 84 (datum shift)
`x`, `y` is the projection of (lon, lat) on the CRS's own geographic CRS; `wx`, `wy` the
CRS's coordinates of WGS 84's (wlon, wlat) = (lon, lat), as PROJ transforms them without
network grids (it picks the operation per point, as specular-gis does); `op` names the
operation PROJ used ("Ballpark..." when it had none) and `grid` whether it needs a grid.
Values PROJ cannot compute are null. The test `tests/proj_oracle.rs` reads the file from
SPECULAR_GIS_PROJ_ORACLE.
"""

import json
import math
import sys

import pyproj
from pyproj import CRS, Transformer
from pyproj.database import query_crs_info
from pyproj.enums import PJType

pyproj.network.set_network_enabled(False)
out = open(sys.argv[1], "w")
codes = sorted(
    {int(i.code) for i in query_crs_info(auth_name="EPSG", pj_types=[PJType.PROJECTED_CRS])}
    | {int(i.code) for i in query_crs_info(auth_name="EPSG", pj_types=[PJType.GEOGRAPHIC_2D_CRS])}
)
wgs = CRS.from_epsg(4326)
n = 0
for code in codes:
    try:
        crs = CRS.from_epsg(code)
    except Exception:
        continue
    if crs.is_deprecated:
        continue
    area = crs.area_of_use
    if area is None:
        continue
    w, s, e, nn = area.west, area.south, area.east, area.north
    if e < w:
        e += 360
    cx, cy = (w + e) / 2, (s + nn) / 2
    pts = [(cx, cy)] + [
        ((cx + px) / 2, (cy + py) / 2) for px, py in [(w, s), (w, nn), (e, s), (e, nn)]
    ]
    pts = [(((x + 180) % 360) - 180, max(-89.9, min(89.9, y))) for x, y in pts]
    base = crs.geodetic_crs if crs.is_projected else crs
    # The own-datum check needs a base in degrees from Greenwich (the CRS's own projection
    # parameters are relative to its prime meridian); others get the WGS 84 check only.
    plain = (
        base.prime_meridian.longitude == 0
        and all(a.unit_name == "degree" for a in base.axis_info[:2])
    )
    if crs.is_geographic and not all(a.unit_name == "degree" for a in crs.axis_info[:2]):
        continue
    try:
        own = Transformer.from_crs(base, crs, always_xy=True) if plain else None
        from_wgs = Transformer.from_crs(wgs, crs, always_xy=True)
    except Exception:
        continue
    for lon, lat in pts:
        rec = {"code": code, "lon": lon, "lat": lat}
        try:
            x, y = own.transform(lon, lat)
            rec["x"], rec["y"] = (x, y) if math.isfinite(x) and math.isfinite(y) else (None, None)
        except Exception:
            rec["x"] = rec["y"] = None
        try:
            x, y = from_wgs.transform(lon, lat)
            rec["wx"], rec["wy"] = (x, y) if math.isfinite(x) and math.isfinite(y) else (None, None)
            try:
                op = from_wgs.get_last_used_operation()
                rec["op"] = op.description
                rec["acc"] = op.accuracy
                rec["grid"] = any(True for _ in op.grids) if hasattr(op, "grids") else False
            except Exception:
                rec["op"] = None
        except Exception:
            rec["wx"] = rec["wy"] = None
        out.write(json.dumps(rec) + "\n")
        n += 1
print(f"{n} points for {len(codes)} codes", file=sys.stderr)
