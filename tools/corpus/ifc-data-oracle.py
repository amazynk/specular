"""Oracle for Specular's IFC model information (specular_mesh::ifc::data): what IfcOpenShell
reads for each object with a GlobalId.

Usage: python -I tools/corpus/ifc-data-oracle.py MODEL.ifc OUT.json

Per object: class, Name, predefined type (ifcopenshell.util.element.get_predefined_type),
type object's GlobalId, container's GlobalId, material names and categories, classification
codes, and the property and quantity sets (get_psets, the type's merged under the
occurrence's) as {set: {property: text}} for single values and quantities. Numbers are
written as repr() of the file's value (no unit conversion).
"""

import json
import sys

import ifcopenshell
import ifcopenshell.util.element as ue


def text(v):
    if isinstance(v, bool):
        return "TRUE" if v else "FALSE"
    if isinstance(v, float):
        return repr(v)
    return str(v)


def main():
    f = ifcopenshell.open(sys.argv[1])
    out = {}
    for o in f.by_type("IfcObjectDefinition"):
        g = getattr(o, "GlobalId", None)
        if not g:
            continue
        t = ue.get_type(o)
        c = ue.get_container(o)
        psets = {}
        for name, props in ue.get_psets(o).items():
            psets[name] = {
                k: text(v)
                for k, v in props.items()
                if k != "id" and isinstance(v, (str, int, float, bool))
            }
        out[g] = {
            "class": o.is_a().upper(),
            "name": o.Name or "",
            "predefined": ue.get_predefined_type(o) or "",
            "type": t.GlobalId if t is not None and t != o else "",
            "container": c.GlobalId if c is not None else "",
            "psets": psets,
        }
    with open(sys.argv[2], "w") as fh:
        json.dump(out, fh)
    print(sys.argv[1], len(out))


main()
