"""Writes the IFC entity tables in src/ifc/schema/ (one per schema) from IfcOpenShell's copy
of the buildingSMART EXPRESS schemas.

Usage: python -I gen/ifc_schema.py  (from crates/formats/specular-mesh, with ifcopenshell)

Each line is one entity: its name as the schema spells it (`IfcWall`), its supertype ("-"
for none), then its own explicit
attributes in order as name:kind, where kind is
  s string, i integer, r real or number, b boolean, l logical, e enumeration,
  x entity or select, a aggregate, y binary.
Inherited attributes come first in a file's argument list, so an entity's attributes are
its supertypes' followed by its own.
"""

import os

import ifcopenshell.ifcopenshell_wrapper as w

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "..", "src", "ifc", "schema")
SCHEMAS = [("IFC2X3", "ifc2x3.txt"), ("IFC4", "ifc4.txt"), ("IFC4X3_ADD2", "ifc4x3.txt")]


def kind(t):
    while True:
        n = type(t).__name__
        if n == "named_type":
            t = t.declared_type()
        elif n == "type_declaration":
            t = t.declared_type()
        elif n == "simple_type":
            s = t.declared_type()
            return {"string": "s", "integer": "i", "real": "r", "number": "r",
                    "boolean": "b", "logical": "l", "binary": "y"}[s]
        elif n == "enumeration_type":
            return "e"
        elif n in ("entity", "select_type"):
            return "x"
        elif n == "aggregation_type":
            return "a"
        else:
            raise ValueError(n)


def main():
    os.makedirs(OUT, exist_ok=True)
    for name, out in SCHEMAS:
        s = w.schema_by_name(name)
        lines = []
        for e in sorted(s.entities(), key=lambda e: e.name()):
            sup = e.supertype().name() if e.supertype() else "-"
            attrs = " ".join(f"{a.name()}:{kind(a.type_of_attribute())}" for a in e.attributes())
            lines.append(f"{e.name()} {sup} {attrs}".rstrip())
        with open(os.path.join(OUT, out), "w") as f:
            f.write("\n".join(lines) + "\n")
        print(out, len(lines))


main()
