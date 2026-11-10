"""The EPSG table of specular-gis: geographic and projected CRSs with their datum shifts to
WGS 84, from the EPSG Geodetic Parameter Dataset as PROJ ships it (proj.db).

Usage: python3 -I epsg.py [PROJ_DB] > ../src/epsg.bin

Only data is taken: names, ellipsoids, prime meridians, projection methods with their
parameters in degrees and metres, linear units, and Helmert transformations to WGS 84
(EPSG:4326) with their areas of use. Deprecated entries, time-dependent Helmerts and
geocentric-domain operations are left out. The file is zlib-compressed; layout (little
endian), after the magic b"SPEPSG2\\0":

  u32 epsg_version_len, bytes   the dataset version ("v12.029")
  u16 n_ellipsoids              then per ellipsoid: f64 a, f64 inverse flattening (0: sphere)
  u32 n_geographic              then per CRS, sorted by code:
      u32 code, str name, str datum, str ellipsoid name, u16 ellipsoid, f64 prime meridian
      (degrees east of Greenwich), u8 n_shifts, then per shift:
      f32 west, south, east, north (degrees), f32 accuracy (m, -1 unknown),
      u8 kind (0 translation, 1 position vector, 2 Molodensky-Badekas (position vector)),
      f64 tx, ty, tz (m), rx, ry, rz (arc-seconds, position vector), ds (ppm),
      and for kind 2: f64 px, py, pz (m)
  u32 n_projected               then per CRS, sorted by code:
      u32 code, str name, u32 geographic code, u16 method (EPSG code; 0: not supported
      here, then str method name), u8 n_params, f64 params..., f64 unit metres, str unit

  str: u16 length + UTF-8 bytes.

Parameters per method (degrees, metres, scale):
  9807 / 9808 Transverse Mercator (South Orientated): lat0 lon0 k0 fe fn
  9801 / 9802 / 1051 Lambert conic: lat0 lon0 lat1 lat2 k0 fe fn
  9822 Albers: lat0 lon0 lat1 lat2 fe fn
  9804 Mercator A: lon0 k0 fe fn;  9805 Mercator B: lon0 lat1 fe fn
  1024 Pseudo-Mercator: lon0 fe fn
  9810 Polar stereographic A: lat0 lon0 k0 fe fn;  9829 B: lat_ts lon0 fe fn
  9820 Lambert azimuthal equal-area: lat0 lon0 fe fn
  1028 / 1029 Equidistant cylindrical: lat_ts lon0 fe fn
  9809 Oblique stereographic: lat0 lon0 k0 fe fn
  9812 / 9815 Hotine oblique Mercator A / B: latc lonc alpha gamma kc fe fn
  9806 Cassini-Soldner: lat0 lon0 fe fn
"""

import math
import sqlite3
import struct
import sys
import zlib

DB = sys.argv[1] if len(sys.argv) > 1 else "/usr/share/proj/proj.db"
c = sqlite3.connect(f"file:{DB}?mode=ro", uri=True)

UNITS = {
    (r[0], str(r[1])): (r[2], r[3], r[4])
    for r in c.execute("select auth_name, code, name, type, conv_factor from unit_of_measure")
}


def dms(v):
    """EPSG's sexagesimal DMS (DDD.MMSSsss) to degrees."""
    s = -1 if v < 0 else 1
    v = abs(v)
    d = math.floor(v)
    rest = round((v - d) * 100, 10)
    m = math.floor(rest)
    sec = (rest - m) * 100
    return s * (d + m / 60 + sec / 3600)


def angle_deg(v, uom):
    if v is None:
        return None
    if uom == ("EPSG", "9110"):
        return dms(v)
    f = UNITS[uom][2]
    return math.degrees(v * f)


def length_m(v, uom):
    return None if v is None else v * UNITS[uom][2]


def to_metres_or_degrees(v, uom):
    """A parameter value in degrees (angles), metres (lengths) or unity (scales)."""
    if v is None:
        return None
    name, kind, f = UNITS[uom]
    if kind == "angle":
        return angle_deg(v, uom)
    if kind == "length":
        return v * f
    if kind == "scale":
        return v * f
    raise ValueError(f"unit {uom}")


version = c.execute("select value from metadata where key='EPSG.VERSION'").fetchone()[0]

# Ellipsoids.
ells = {}
ell_list = []


def ellipsoid(auth, code):
    key = (auth, str(code))
    if key in ells:
        return ells[key]
    a, uom_a, uom_c, rf, b, name = c.execute(
        "select semi_major_axis, uom_auth_name, uom_code, inv_flattening, semi_minor_axis, name "
        "from ellipsoid where auth_name=? and code=?",
        key,
    ).fetchone()
    a = length_m(a, (uom_a, str(uom_c)))
    if rf is None or rf == 0:
        if b is not None:
            b = length_m(b, (uom_a, str(uom_c)))
            rf = 0.0 if abs(a - b) < 1e-9 else a / (a - b)
        else:
            rf = 0.0
    ells[key] = (len(ell_list), name)
    ell_list.append((a, rf))
    return ells[key]


def pm_deg(auth, code):
    lon, ua, uc = c.execute(
        "select longitude, uom_auth_name, uom_code from prime_meridian where auth_name=? and code=?",
        (auth, str(code)),
    ).fetchone()
    return angle_deg(lon, (ua, str(uc)))


# Helmert transformations to WGS 84, by source geographic CRS.
shifts = {}
q = """
select h.source_crs_code, h.method_code, h.accuracy, h.tx, h.ty, h.tz,
       h.translation_uom_auth_name, h.translation_uom_code,
       h.rx, h.ry, h.rz, h.rotation_uom_auth_name, h.rotation_uom_code,
       h.scale_difference, h.scale_difference_uom_auth_name, h.scale_difference_uom_code,
       h.rate_tx, h.rate_rx, h.rate_scale_difference, h.epoch,
       h.px, h.py, h.pz, h.pivot_uom_auth_name, h.pivot_uom_code,
       e.west_lon, e.south_lat, e.east_lon, e.north_lat, h.code
from helmert_transformation_table h
join usage u on u.object_table_name='helmert_transformation' and u.object_auth_name=h.auth_name
     and u.object_code=h.code
join extent e on e.auth_name=u.extent_auth_name and e.code=u.extent_code
where h.auth_name='EPSG' and h.deprecated=0 and h.source_crs_auth_name='EPSG'
  and h.target_crs_auth_name='EPSG' and h.target_crs_code='4326'
  and h.method_code in ('9603','9606','9607','9636')
"""
for r in c.execute(q):
    (src, method, acc, tx, ty, tz, tua, tuc, rx, ry, rz, rua, ruc, ds, sua, suc,
     rate_tx, rate_rx, rate_ds, epoch, px, py, pz, pua, puc, w, s, e, n, code) = r
    method = str(method)
    if rate_tx not in (None, 0) or rate_rx not in (None, 0) or rate_ds not in (None, 0):
        continue
    tl = (tua, str(tuc))
    t = [length_m(v, tl) for v in (tx, ty, tz)]
    if method == "9603" or rx is None:
        rot = [0.0, 0.0, 0.0]
        sc = 0.0
        kind = 0
        if method == "9636":
            continue
    else:
        ru = (rua, str(ruc))
        rad = UNITS[ru][2]
        rot = [v * rad / UNITS[("EPSG", "9104")][2] for v in (rx, ry, rz)]
        sc = ds * UNITS[(sua, str(suc))][2] / 1e-6
        if method in ("9607", "9636"):  # coordinate frame: the opposite rotation sign
            rot = [-v for v in rot]
        kind = 2 if method == "9636" else 1
    pivot = None
    if method == "9636":
        pl = (pua, str(puc))
        pivot = [length_m(v, pl) for v in (px, py, pz)]
    shifts.setdefault(str(src), []).append(
        (w, s, e, n, -1.0 if acc is None else acc, kind, t + rot + [sc], pivot, int(code))
    )

# Geographic CRSs (2D; 3D ones share their 2D twin's datum).
geog = {}
q = """
select g.code, g.name, d.name, d.ellipsoid_auth_name, d.ellipsoid_code,
       d.prime_meridian_auth_name, d.prime_meridian_code
from geodetic_crs g join geodetic_datum d on d.auth_name=g.datum_auth_name and d.code=g.datum_code
where g.auth_name='EPSG' and g.deprecated=0 and g.type in ('geographic 2D','geographic 3D')
"""
for code, name, dname, ea, ec, pa, pc in c.execute(q):
    ei, ename = ellipsoid(ea, ec)
    geog[int(code)] = (name, dname, ename, ei, pm_deg(pa, pc), shifts.get(str(code), []))
# Datum ensembles (WGS 84, ETRS89, ...): geodetic_crs points at the ensemble in
# geodetic_datum, which the join above covers.

# Projected CRSs.
PARAMS = {
    # method: list of (param codes accepted in order of preference, default)
    "9807": [(("8801",), 0), (("8802",), 0), (("8805",), 1), (("8806",), 0), (("8807",), 0)],
    "9808": [(("8801",), 0), (("8802",), 0), (("8805",), 1), (("8806",), 0), (("8807",), 0)],
    "9801": [(("8801",), 0), (("8802",), 0), (("8801",), 0), (("8801",), 0), (("8805",), 1),
             (("8806",), 0), (("8807",), 0)],
    "9802": [(("8821",), 0), (("8822",), 0), (("8823",), None), (("8824",), None), ((), 1),
             (("8826",), 0), (("8827",), 0)],
    "1051": [(("8821",), 0), (("8822",), 0), (("8823",), None), (("8824",), None), (("1038",), 1),
             (("8826",), 0), (("8827",), 0)],
    "9822": [(("8821",), 0), (("8822",), 0), (("8823",), None), (("8824",), None),
             (("8826",), 0), (("8827",), 0)],
    "9804": [(("8802",), 0), (("8805",), 1), (("8806",), 0), (("8807",), 0)],
    "9805": [(("8802",), 0), (("8823",), None), (("8806",), 0), (("8807",), 0)],
    "1024": [(("8802",), 0), (("8806",), 0), (("8807",), 0)],
    "9810": [(("8801",), None), (("8802",), 0), (("8805",), 1), (("8806",), 0), (("8807",), 0)],
    "9829": [(("8832",), None), (("8833",), 0), (("8806",), 0), (("8807",), 0)],
    "9820": [(("8801",), 0), (("8802",), 0), (("8806",), 0), (("8807",), 0)],
    "1028": [(("8823",), 0), (("8802",), 0), (("8806",), 0), (("8807",), 0)],
    "1029": [(("8823",), 0), (("8802",), 0), (("8806",), 0), (("8807",), 0)],
    "9809": [(("8801",), 0), (("8802",), 0), (("8805",), 1), (("8806",), 0), (("8807",), 0)],
    "9812": [(("8811",), None), (("8812",), None), (("8813",), None), (("8814",), None),
             (("8815",), 1), (("8806",), 0), (("8807",), 0)],
    "9815": [(("8811",), None), (("8812",), None), (("8813",), None), (("8814",), None),
             (("8815",), 1), (("8816",), 0), (("8817",), 0)],
    "9806": [(("8801",), 0), (("8802",), 0), (("8806",), 0), (("8807",), 0)],
}
# Methods given as another: LCC 1SP's lat1 = lat2 = lat0 (taken from 8801 above).

q = """
select p.code, p.name, p.geodetic_crs_code, ct.method_code, m.name,
       ct.param1_code, ct.param1_value, ct.param1_uom_auth_name, ct.param1_uom_code,
       ct.param2_code, ct.param2_value, ct.param2_uom_auth_name, ct.param2_uom_code,
       ct.param3_code, ct.param3_value, ct.param3_uom_auth_name, ct.param3_uom_code,
       ct.param4_code, ct.param4_value, ct.param4_uom_auth_name, ct.param4_uom_code,
       ct.param5_code, ct.param5_value, ct.param5_uom_auth_name, ct.param5_uom_code,
       ct.param6_code, ct.param6_value, ct.param6_uom_auth_name, ct.param6_uom_code,
       ct.param7_code, ct.param7_value, ct.param7_uom_auth_name, ct.param7_uom_code,
       p.coordinate_system_auth_name, p.coordinate_system_code
from projected_crs p
join conversion_table ct on ct.auth_name=p.conversion_auth_name and ct.code=p.conversion_code
left join conversion_method m on m.auth_name=ct.method_auth_name and m.code=ct.method_code
where p.auth_name='EPSG' and p.deprecated=0 and p.geodetic_crs_auth_name='EPSG'
"""
proj = []
for r in c.execute(q):
    code, name, gcode, method, mname = r[:5]
    if int(gcode) not in geog:
        continue
    vals = {}
    for i in range(7):
        pc, pv, pa, pu = r[5 + 4 * i: 9 + 4 * i]
        if pc is None or pv is None:
            continue
        vals[str(pc)] = to_metres_or_degrees(pv, (pa, str(pu)))
    csa, csc = r[33], r[34]
    axes = list(c.execute(
        "select uom_auth_name, uom_code from axis where coordinate_system_auth_name=? and "
        "coordinate_system_code=? order by coordinate_system_order", (csa, str(csc))))
    if not axes or axes[0][0] is None:
        continue
    uom = (axes[0][0], str(axes[0][1]))
    uname, ukind, uf = UNITS[uom]
    if ukind != "length" or uf is None:
        continue
    spec = PARAMS.get(str(method))
    params = None
    if spec is not None:
        params = []
        for codes, default in spec:
            v = next((vals[k] for k in codes if k in vals), default)
            if v is None:
                params = None
                break
            params.append(float(v))
        # Mercator A with a latitude of origin other than 0 is not variant A.
        if str(method) == "9804" and abs(vals.get("8801", 0.0)) > 1e-12:
            params = None
    proj.append((int(code), name, int(gcode), int(method) if params is not None else 0,
                 mname or "", params or [], uf, uname))

out = bytearray(b"SPEPSG2\0")


def s(text):
    b = text.encode("utf-8")
    out.extend(struct.pack("<H", len(b)))
    out.extend(b)


v = version.encode()
out.extend(struct.pack("<I", len(v)))
out.extend(v)
out.extend(struct.pack("<H", len(ell_list)))
for a, rf in ell_list:
    out.extend(struct.pack("<dd", a, rf))
out.extend(struct.pack("<I", len(geog)))
for code in sorted(geog):
    name, dname, ename, ei, pm, sh = geog[code]
    out.extend(struct.pack("<I", code))
    s(name)
    s(dname)
    s(ename)
    out.extend(struct.pack("<Hd", ei, pm))
    sh = sorted(sh, key=lambda t: t[8])[:255]
    out.extend(struct.pack("<B", len(sh)))
    for w, so, e, n, acc, kind, p, pivot, _ in sh:
        out.extend(struct.pack("<5fB", w, so, e, n, acc, kind))
        out.extend(struct.pack("<7d", *p))
        if kind == 2:
            out.extend(struct.pack("<3d", *pivot))
out.extend(struct.pack("<I", len(proj)))
for code, name, gcode, method, mname, params, uf, uname in sorted(proj):
    out.extend(struct.pack("<I", code))
    s(name)
    out.extend(struct.pack("<IH", gcode, method))
    if method == 0:
        s(mname)
    out.extend(struct.pack("<B", len(params)))
    out.extend(struct.pack(f"<{len(params)}d", *params))
    out.extend(struct.pack("<d", uf))
    s(uname)

sys.stdout.buffer.write(zlib.compress(bytes(out), 9))
print(
    f"{len(ell_list)} ellipsoids, {len(geog)} geographic CRSs "
    f"({sum(len(g[5]) for g in geog.values())} shifts), {len(proj)} projected CRSs "
    f"({sum(1 for p in proj if p[3])} supported); {len(out)} bytes raw",
    file=sys.stderr,
)
