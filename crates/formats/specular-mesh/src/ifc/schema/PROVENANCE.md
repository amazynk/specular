# IFC entity tables

`ifc2x3.txt`, `ifc4.txt` and `ifc4x3.txt` list each entity's name, supertype and explicit
attributes (name and kind). They are facts from the buildingSMART EXPRESS schemas
(IFC2X3 TC1, IFC4 ADD2 TC1, IFC4X3 ADD2), the published IFC standard.

They were extracted by `gen/ifc_schema.py`, which reads the schemas through IfcOpenShell's
Python module at generation time. No IfcOpenShell code or data beyond the schema facts is
in these files, and IfcOpenShell is not used at build or run time.
