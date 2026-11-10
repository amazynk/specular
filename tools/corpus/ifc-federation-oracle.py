"""IfcOpenShell's reading of a federated IFC scene, for specular-bim's `federation_corpus` test.

Usage: python -I ifc-federation-oracle.py CORPUS_DIR OUT.json

For each schema folder (ifc2x3, ifc4, ifc4x3) of CORPUS_DIR (see fetch-ifc-federation.sh):
- `maps`: per model, sample project points (metres) and their map coordinates (metres) from
  ifcopenshell.util.geolocation.auto_xyz2enh, and the CRS name;
- `placements`: per model, where it sits in the first model's frame (offset in metres and
  rotation about z in degrees), from auto_xyz2enh then auto_enh2xyz;
- `clashes`: hard clashes between every two models (GUID pairs) at the tolerance given:
  IfcOpenShell's triangulated geometry, each model placed as above, boxes overlapping, then
  the solids' intersection (manifold3d) thicker than the tolerance, its thickness taken as
  twice its volume over its surface area (touching faces give about zero). (IfcOpenShell's own clash tree needs its CGAL backend, which crashes
  here.) Pairs of overlapping boxes where either element is not a closed solid are listed in
  `not_solid` (not judged).

Element kinds never drawn by Specular (spaces, openings, annotations, ports, structural
analysis items, alignments) are left out on both sides.
"""

import json
import math
import multiprocessing
import os
import sys

import ifcopenshell
import manifold3d
import numpy as np
import ifcopenshell.geom as G
import ifcopenshell.util.geolocation as geo
import ifcopenshell.util.unit as U

TOLERANCE = 0.01  # metres
HIDDEN = (
    "IfcOpeningElement", "IfcVoidingFeature", "IfcSpace", "IfcSpatialZone",
    "IfcExternalSpatialElement", "IfcAnnotation", "IfcGrid", "IfcVirtualElement",
    "IfcReferent", "IfcDistributionPort",
)
SAMPLES = [(0.0, 0.0, 0.0), (10.0, 20.0, 3.0), (-5.0, 7.5, 100.0), (1234.5, -987.25, -12.0)]


def drawn(e):
    if e.Representation is None:
        return False
    t = e.is_a()
    if t.startswith("IfcStructural") or t.startswith("IfcAlignment"):
        return False
    return not any(e.is_a(h) for h in HIDDEN)


def main():
    root, out = sys.argv[1], sys.argv[2]
    result = {"tolerance": TOLERANCE, "schemas": {}}
    for schema in ("ifc2x3", "ifc4", "ifc4x3"):
        d = os.path.join(root, schema)
        if not os.path.isdir(d):
            continue
        names = sorted(n for n in os.listdir(d) if n.endswith(".ifc"))
        # The reference model first.
        names.sort(key=lambda n: (n != "Building-Architecture.ifc", n))
        files = {n: ifcopenshell.open(os.path.join(d, n)) for n in names}
        ref = files[names[0]]
        ref_u = U.calculate_unit_scale(ref)
        maps, places = {}, {}
        for n, f in files.items():
            u = U.calculate_unit_scale(f)
            pts = []
            for p in SAMPLES:
                enh = geo.auto_xyz2enh(f, *(c / u for c in p))
                crs = f.by_type("IfcProjectedCRS") if f.schema != "IFC2X3" else []
                mu = 1.0
                if crs and crs[0].MapUnit is not None and crs[0].MapUnit.is_a("IfcSIUnit"):
                    mu = U.get_prefix_multiplier(crs[0].MapUnit.Prefix)
                has_map = geo.get_helmert_transformation_parameters(f) is not None
                pts.append(list(p) + ([c * mu for c in enh] if has_map else [None, None, None]))
            crs = f.by_type("IfcProjectedCRS") if f.schema != "IFC2X3" else []
            maps[n] = {"crs": crs[0].Name if crs else None, "points": pts}

            def to_ref(x, y, z):
                enh = geo.auto_xyz2enh(f, x / u, y / u, z / u)
                q = geo.auto_enh2xyz(ref, *enh)
                return [c * ref_u for c in q]

            o = to_ref(0, 0, 0)
            ex = to_ref(1, 0, 0)
            ang = math.degrees(math.atan2(ex[1] - o[1], ex[0] - o[0]))
            places[n] = {"offset": o, "angle": ang}
        geoms = {}
        for n, f in files.items():
            pl = places[n]
            a = math.radians(pl["angle"])
            rot = np.array([[math.cos(a), -math.sin(a), 0], [math.sin(a), math.cos(a), 0], [0, 0, 1]])
            off = np.array(pl["offset"])
            s = G.settings()
            s.set("use-world-coords", True)
            elems = [e for e in f.by_type("IfcProduct") if drawn(e)]
            out_el = []
            if elems:
                it = G.iterator(s, f, multiprocessing.cpu_count() // 2 or 1, include=elems)
                if it.initialize():
                    while True:
                        sh = it.get()
                        v = np.array(sh.geometry.verts, dtype=float).reshape(-1, 3)
                        t = np.array(sh.geometry.faces, dtype=np.uint32).reshape(-1, 3)
                        if len(t):
                            v = v @ rot.T + off
                            out_el.append((sh.guid, v, t, v.min(axis=0), v.max(axis=0)))
                        if not it.next():
                            break
            geoms[n] = out_el
        clashes, unknown = {}, {}
        solids = {}

        def solid(key, v, t):
            if key not in solids:
                m = manifold3d.Manifold(manifold3d.Mesh(vert_properties=v.astype(np.float32), tri_verts=t))
                solids[key] = m if m.status() == manifold3d.Error.NoError and not m.is_empty() else None
            return solids[key]

        for i, a in enumerate(names):
            for b in names[i + 1:]:
                pairs, unk = [], []
                for ga, va, ta, la, ha in geoms[a]:
                    for gb, vb, tb, lb, hb in geoms[b]:
                        if np.any(la > hb) or np.any(lb > ha):
                            continue
                        sa, sb = solid((a, ga), va, ta), solid((b, gb), vb, tb)
                        if sa is None or sb is None:
                            unk.append([ga, gb])
                            continue
                        x = sa ^ sb
                        if x.is_empty() or x.volume() <= 0:
                            continue
                        # The overlap's thickness: twice its volume over its surface area
                        # (a thin plate's thickness; touching faces give ~0).
                        if 2.0 * x.volume() / max(x.surface_area(), 1e-12) > TOLERANCE:
                            pairs.append((ga, gb))
                clashes[f"{a}|{b}"] = sorted([list(p) for p in pairs])
                unknown[f"{a}|{b}"] = unk
        result["schemas"][schema] = {
            "models": names,
            "maps": maps,
            "placements": places,
            "clashes": clashes,
            "not_solid": unknown,
        }
        print(schema, {k: len(v) for k, v in clashes.items() if v}, file=sys.stderr)
    with open(out, "w") as fh:
        json.dump(result, fh, indent=1)


if __name__ == "__main__":
    main()
