#!/usr/bin/env python3
"""Writes a small hand-made AP242 STEP assembly for the PMI-per-instance test: one annotated
part (a 100 x 40 x 20 mm block) used twice, the second turned 90 degrees about z and moved.

The block carries semantic PMI on its own faces (so on every instance):
  - a location dimension 100 +-0.2 between its x = 0 and x = 100 faces
  - a flatness tolerance 0.05 on its top face

  python3 tools/corpus/pmi-assembly.py crates/specular-decode-host/tests/data/pmi-twice.step

Written entity by entity from ISO 10303-242's semantic PMI structures; no other tool's
output is used.
"""
import sys


class W:
    def __init__(self):
        self.lines = []

    def add(self, text):
        self.lines.append(text)
        return '#%d' % len(self.lines)


def main(out):
    w = W()
    a = w.add
    mm = a("( LENGTH_UNIT() NAMED_UNIT(*) SI_UNIT(.MILLI.,.METRE.) )")
    rad = a("( NAMED_UNIT(*) PLANE_ANGLE_UNIT() SI_UNIT($,.RADIAN.) )")
    sr = a("( NAMED_UNIT(*) SI_UNIT($,.STERADIAN.) SOLID_ANGLE_UNIT() )")
    unc = a("UNCERTAINTY_MEASURE_WITH_UNIT(LENGTH_MEASURE(1.E-07),%s,'distance_accuracy_value','confusion accuracy')" % mm)
    ctx = a("( GEOMETRIC_REPRESENTATION_CONTEXT(3) GLOBAL_UNCERTAINTY_ASSIGNED_CONTEXT((%s)) "
            "GLOBAL_UNIT_ASSIGNED_CONTEXT((%s,%s,%s)) REPRESENTATION_CONTEXT('3D','3D context') )" % (unc, mm, rad, sr))
    app = a("APPLICATION_CONTEXT('managed model based 3d engineering')")
    a("APPLICATION_PROTOCOL_DEFINITION('international standard','ap242_managed_model_based_3d_engineering',2014,%s)" % app)
    pctx = a("PRODUCT_CONTEXT('',%s,'mechanical')" % app)
    pdctx = a("PRODUCT_DEFINITION_CONTEXT('part definition',%s,'design')" % app)

    def point(p):
        return a("CARTESIAN_POINT('',(%s))" % ','.join('%.6f' % v for v in p))

    def direction(d):
        return a("DIRECTION('',(%s))" % ','.join('%.6f' % v for v in d))

    def axis(o, z, x):
        return a("AXIS2_PLACEMENT_3D('',%s,%s,%s)" % (point(o), direction(z), direction(x)))

    def product(name):
        p = a("PRODUCT('%s','%s','',(%s))" % (name, name, pctx))
        a("PRODUCT_RELATED_PRODUCT_CATEGORY('part',$,(%s))" % p)
        f = a("PRODUCT_DEFINITION_FORMATION('','',%s)" % p)
        d = a("PRODUCT_DEFINITION('design','',%s,%s)" % (f, pdctx))
        s = a("PRODUCT_DEFINITION_SHAPE('','',%s)" % d)
        return d, s

    # The block's B-rep: vertices, edges, six planar faces (outward normals).
    L, D, H = 100.0, 40.0, 20.0
    vs = [(x, y, z) for z in (0.0, H) for y in (0.0, D) for x in (0.0, L)]
    vp = [a("VERTEX_POINT('',%s)" % point(v)) for v in vs]
    # Faces as vertex loops, counter-clockwise seen from outside, with their normals.
    faces = {
        'x0': ([0, 4, 6, 2], (-1, 0, 0)),
        'x1': ([1, 3, 7, 5], (1, 0, 0)),
        'y0': ([0, 1, 5, 4], (0, -1, 0)),
        'y1': ([2, 6, 7, 3], (0, 1, 0)),
        'z0': ([0, 2, 3, 1], (0, 0, -1)),
        'z1': ([4, 5, 7, 6], (0, 0, 1)),
    }
    edges = {}

    def edge(i, j):
        k = (min(i, j), max(i, j))
        if k not in edges:
            p, q = vs[k[0]], vs[k[1]]
            d = [q[n] - p[n] for n in range(3)]
            n = sum(c * c for c in d) ** 0.5
            vec = a("VECTOR('',%s,%.6f)" % (direction([c / n for c in d]), n))
            line = a("LINE('',%s,%s)" % (point(p), vec))
            edges[k] = a("EDGE_CURVE('',%s,%s,%s,.T.)" % (vp[k[0]], vp[k[1]], line))
        return a("ORIENTED_EDGE('',*,*,%s,%s)" % (edges[k], '.T.' if i < j else '.F.'))

    face_ids = {}
    for name, (loop, nrm) in faces.items():
        oes = [edge(loop[k], loop[(k + 1) % 4]) for k in range(4)]
        el = a("EDGE_LOOP('',(%s))" % ','.join(oes))
        bound = a("FACE_OUTER_BOUND('',%s,.T.)" % el)
        p0, p1 = vs[loop[0]], vs[loop[1]]
        xd = [p1[n] - p0[n] for n in range(3)]
        xn = sum(c * c for c in xd) ** 0.5
        plane = a("PLANE('',%s)" % axis(p0, nrm, [c / xn for c in xd]))
        face_ids[name] = a("ADVANCED_FACE('%s',(%s),%s,.T.)" % (name, bound, plane))
    shell = a("CLOSED_SHELL('',(%s))" % ','.join(face_ids.values()))
    solid = a("MANIFOLD_SOLID_BREP('Block',%s)" % shell)
    part_origin = axis((0, 0, 0), (0, 0, 1), (1, 0, 0))
    part_rep = a("ADVANCED_BREP_SHAPE_REPRESENTATION('',(%s,%s),%s)" % (part_origin, solid, ctx))
    part_pd, part_pds = product('Block')
    a("SHAPE_DEFINITION_REPRESENTATION(%s,%s)" % (part_pds, part_rep))

    # The assembly: the block twice.
    asm_pd, asm_pds = product('Two blocks')
    places = [((0, 0, 0), (1, 0, 0)), ((150, 50, 0), (0, 1, 0))]
    asm_axes = [axis(o, (0, 0, 1), x) for o, x in places]
    asm_rep = a("SHAPE_REPRESENTATION('',(%s,%s),%s)" % (axis((0, 0, 0), (0, 0, 1), (1, 0, 0)), ','.join(asm_axes), ctx))
    a("SHAPE_DEFINITION_REPRESENTATION(%s,%s)" % (asm_pds, asm_rep))
    for k, ax in enumerate(asm_axes):
        nauo = a("NEXT_ASSEMBLY_USAGE_OCCURRENCE('%d','Block %d','',%s,%s,$)" % (k + 1, k + 1, asm_pd, part_pd))
        pds = a("PRODUCT_DEFINITION_SHAPE('Placement','',%s)" % nauo)
        idt = a("ITEM_DEFINED_TRANSFORMATION('','',%s,%s)" % (part_origin, ax))
        rr = a("( REPRESENTATION_RELATIONSHIP('','',%s,%s) REPRESENTATION_RELATIONSHIP_WITH_TRANSFORMATION(%s) "
               "SHAPE_REPRESENTATION_RELATIONSHIP() )" % (part_rep, asm_rep, idt))
        a("CONTEXT_DEPENDENT_SHAPE_REPRESENTATION(%s,%s)" % (rr, pds))

    # Semantic PMI on the block's own faces.
    def aspect(face, name):
        sa = a("SHAPE_ASPECT('%s','',%s,.T.)" % (name, part_pds))
        a("GEOMETRIC_ITEM_SPECIFIC_USAGE('','',%s,%s,%s)" % (sa, part_rep, face_ids[face]))
        return sa

    left, right, top = aspect('x0', 'left end'), aspect('x1', 'right end'), aspect('z1', 'top')
    dim = a("DIMENSIONAL_LOCATION('linear distance','',%s,%s)" % (left, right))
    nominal = a("( LENGTH_MEASURE_WITH_UNIT() MEASURE_REPRESENTATION_ITEM() "
                "MEASURE_WITH_UNIT(LENGTH_MEASURE(100.),%s) REPRESENTATION_ITEM('nominal value') )" % mm)
    sdr = a("SHAPE_DIMENSION_REPRESENTATION('',(%s),%s)" % (nominal, ctx))
    a("DIMENSIONAL_CHARACTERISTIC_REPRESENTATION(%s,%s)" % (dim, sdr))
    lo = a("LENGTH_MEASURE_WITH_UNIT(LENGTH_MEASURE(-0.2),%s)" % mm)
    hi = a("LENGTH_MEASURE_WITH_UNIT(LENGTH_MEASURE(0.2),%s)" % mm)
    tv = a("TOLERANCE_VALUE(%s,%s)" % (lo, hi))
    a("PLUS_MINUS_TOLERANCE(%s,%s)" % (tv, dim))
    flat = a("LENGTH_MEASURE_WITH_UNIT(LENGTH_MEASURE(0.05),%s)" % mm)
    a("FLATNESS_TOLERANCE('flatness','',%s,%s)" % (flat, top))

    with open(out, 'w', newline='\n') as f:
        f.write("ISO-10303-21;\nHEADER;\n"
                "FILE_DESCRIPTION(('PMI on a part used twice'),'2;1');\n"
                "FILE_NAME('pmi-twice.step','2026-11-01T00:00:00',('Specular'),('Specular'),'pmi-assembly.py','','');\n"
                "FILE_SCHEMA(('AP242_MANAGED_MODEL_BASED_3D_ENGINEERING_MIM_LF { 1 0 10303 442 1 1 4 }'));\n"
                "ENDSEC;\nDATA;\n")
        for i, l in enumerate(w.lines):
            f.write('#%d = %s;\n' % (i + 1, l))
        f.write("ENDSEC;\nEND-ISO-10303-21;\n")


if __name__ == '__main__':
    main(sys.argv[1])
