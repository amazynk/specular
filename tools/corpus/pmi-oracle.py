#!/usr/bin/env python3
"""Independent reading of STEP AP242 semantic PMI, for specular-decode-host's tests/pmi.rs.

Reads each STEP file of the PMI corpus (tools/corpus/fetch-pmi.sh) with its own Part 21
parser, not OpenCASCADE, and writes pmi-oracle.tsv next to them, one line per fact:

  file  dim   value  plus  minus          a dimension (DIMENSIONAL_SIZE / _LOCATION): its
                                          nominal value (or the mid of its limits), its
                                          plus/minus tolerance (minus as a magnitude), "-"
                                          where it has none
  file  range lower upper                 a dimension's limits
  file  fit   H7                          a dimension's class of tolerance (form variance
                                          and grade)
  file  tol   type value dia mods datums  a geometric tolerance: its type (POSITION, ...),
                                          magnitude, "D" for a cylindrical zone, its
                                          modifiers (sorted, comma separated) and its datum
                                          reference frame ("A|B|C", common datums "A-B")
  file  datum A                           a datum letter
  file  target N                          datum targets (count)
  file  views N                           saved views (camera models)

Lengths are in millimetres, angles in degrees. Usage:

  python3 -I tools/corpus/pmi-oracle.py [CORPUS]   (default: $SPECULAR_PMI_CORPUS, else
                                                     tools/corpus/pmi)

The files are untrusted: this script only reads them as text.
"""

import math
import os
import re
import sys

TOL_TYPES = [
    "ANGULARITY", "CIRCULAR_RUNOUT", "CIRCULARITY", "COAXIALITY", "CONCENTRICITY",
    "CYLINDRICITY", "FLATNESS", "LINE_PROFILE", "PARALLELISM", "PERPENDICULARITY",
    "POSITION", "ROUNDNESS", "STRAIGHTNESS", "SURFACE_PROFILE", "SYMMETRY", "TOTAL_RUNOUT",
]
DIMS = {
    "DIMENSIONAL_SIZE", "DIMENSIONAL_LOCATION", "ANGULAR_SIZE", "ANGULAR_LOCATION",
    "DIMENSIONAL_SIZE_WITH_PATH", "DIMENSIONAL_LOCATION_WITH_PATH",
    "DIRECTED_DIMENSIONAL_LOCATION", "DIMENSIONAL_SIZE_WITH_DATUM_FEATURE",
}


class Ref:
    __slots__ = ("n",)

    def __init__(self, n):
        self.n = n


class Enum:
    __slots__ = ("v",)

    def __init__(self, v):
        self.v = v


class Typed:
    __slots__ = ("name", "args")

    def __init__(self, name, args):
        self.name = name
        self.args = args


TOKEN = re.compile(
    r"\s*(?:(?P<str>'(?:[^']|'')*')|(?P<ref>#\d+)|(?P<enum>\.[A-Z0-9_]+\.)"
    r"|(?P<num>[-+]?\d+(?:\.\d*)?(?:[eE][-+]?\d+)?)|(?P<kw>!?[A-Z_][A-Z0-9_]*)"
    r"|(?P<p>[(),$*]))"
)


def tokens(text):
    pos = 0
    out = []
    while pos < len(text):
        m = TOKEN.match(text, pos)
        if not m:
            if text[pos:].strip() == "":
                break
            raise ValueError("bad token at %r" % text[pos:pos + 20])
        pos = m.end()
        kind = m.lastgroup
        out.append((kind, m.group(kind)))
    return out


def parse_list(toks, i):
    """Parses '(' ... ')' at toks[i]; returns (list, next index)."""
    assert toks[i] == ("p", "(")
    i += 1
    items = []
    while toks[i] != ("p", ")"):
        v, i = parse_value(toks, i)
        items.append(v)
        if toks[i] == ("p", ","):
            i += 1
    return items, i + 1


def parse_value(toks, i):
    kind, v = toks[i]
    if kind == "str":
        return v[1:-1].replace("''", "'"), i + 1
    if kind == "ref":
        return Ref(int(v[1:])), i + 1
    if kind == "enum":
        return Enum(v[1:-1]), i + 1
    if kind == "num":
        return float(v), i + 1
    if kind == "kw":
        args, i = parse_list(toks, i + 1)
        return Typed(v, args), i
    if v == "(":
        return parse_list(toks, i)
    if v in "$*":
        return None, i + 1
    raise ValueError("unexpected %r" % v)


def parse_instance(body):
    """'TYPE(...)' or '(A(...) B(...))' -> {TYPE: args}."""
    toks = tokens(body)
    parts = {}
    if toks[0] == ("p", "("):
        i = 1
        while toks[i] != ("p", ")"):
            name = toks[i][1]
            args, i = parse_list(toks, i + 1)
            parts[name] = args
    else:
        args, _ = parse_list(toks, 1)
        parts[toks[0][1]] = args
    return parts


def statements(text):
    """The DATA section's instances: id -> body text."""
    out = {}
    data = text.find("DATA;")
    i = data + 5
    start = i
    n = len(text)
    quoted = False
    while i < n:
        c = text[i]
        if quoted:
            if c == "'":
                quoted = False
        elif c == "'":
            quoted = True
        elif c == "/" and text.startswith("/*", i):
            j = text.find("*/", i + 2)
            i = n if j < 0 else j + 2
            continue
        elif c == ";":
            stmt = text[start:i].strip()
            start = i + 1
            if stmt.startswith("ENDSEC"):
                break
            m = re.match(r"#(\d+)\s*=\s*(.*)$", stmt, re.S)
            if m:
                out[int(m.group(1))] = m.group(2)
        i += 1
    return out


class Model:
    def __init__(self, text):
        self.raw = statements(text)
        self.cache = {}

    def get(self, n):
        if n not in self.cache:
            body = self.raw.get(n)
            self.cache[n] = parse_instance(re.sub(r"\s+", " ", body)) if body else {}
        return self.cache[n]

    def ids(self, *names):
        """Instances with any of these entity names (by text, then parsed)."""
        pat = re.compile(r"(?<![A-Z0-9_])(%s)\s*\(" % "|".join(names))
        return [k for k, v in self.raw.items() if pat.search(v)]

    def by_type(self, name):
        return [k for k in self.ids(name) if name in self.get(k)]


PREFIX = {"EXA": 1e18, "PETA": 1e15, "TERA": 1e12, "GIGA": 1e9, "MEGA": 1e6, "KILO": 1e3,
          "HECTO": 1e2, "DECA": 1e1, "DECI": 1e-1, "CENTI": 1e-2, "MILLI": 1e-3,
          "MICRO": 1e-6, "NANO": 1e-9, "PICO": 1e-12, "FEMTO": 1e-15, "ATTO": 1e-18}


def unit_scale(m, ref, depth=0):
    """('length', factor to mm), ('angle', factor to degrees) or (None, 1)."""
    if not isinstance(ref, Ref) or depth > 4:
        return None, 1.0
    u = m.get(ref.n)
    if "SI_UNIT" in u:
        prefix, name = u["SI_UNIT"][-2:]
        p = PREFIX.get(prefix.v, 1.0) if isinstance(prefix, Enum) else 1.0
        if isinstance(name, Enum) and name.v == "METRE":
            return "length", p * 1000.0
        if isinstance(name, Enum) and name.v == "RADIAN":
            return "angle", p * 180.0 / math.pi
        return None, 1.0
    if "CONVERSION_BASED_UNIT" in u:
        by = u["CONVERSION_BASED_UNIT"][1]
        if isinstance(by, Ref):
            e = m.get(by.n)
            mwu = next((a for k, a in e.items() if k.endswith("MEASURE_WITH_UNIT") and a), None)
            if mwu:
                v = mwu[0].args[0] if isinstance(mwu[0], Typed) else mwu[0]
                kind, f = unit_scale(m, mwu[1], depth + 1)
                if kind and isinstance(v, float):
                    return kind, v * f
    return None, 1.0


def measure(m, ref):
    """The value of a measure (simple or complex): lengths in millimetres, angles in degrees;
    (value, name)."""
    if not isinstance(ref, Ref):
        return None, ""
    e = m.get(ref.n)
    mwu = None
    for k, args in e.items():
        if k.endswith("MEASURE_WITH_UNIT") and args:
            mwu = args
    name = ""
    if "REPRESENTATION_ITEM" in e and e["REPRESENTATION_ITEM"]:
        name = e["REPRESENTATION_ITEM"][0] or ""
    elif "MEASURE_REPRESENTATION_ITEM" in e and len(e["MEASURE_REPRESENTATION_ITEM"]) >= 3:
        args = e["MEASURE_REPRESENTATION_ITEM"]
        name, mwu = args[0] or "", args[1:]
    if not mwu:
        return None, name
    v = mwu[0]
    if isinstance(v, Typed):
        v = v.args[0] if v.args else None
    if not isinstance(v, float):
        return None, name
    _, f = unit_scale(m, mwu[1])
    return v * f, name


def letters_of(m, ref):
    """A datum reference compartment's base: 'A', or 'A-B' for a common datum."""
    if isinstance(ref, Typed):  # COMMON_DATUM_LIST((...))
        return letters_of(m, ref.args[0] if ref.args else None)
    if isinstance(ref, list):
        return "-".join(filter(None, (letters_of(m, r) for r in ref)))
    if not isinstance(ref, Ref):
        return ""
    e = m.get(ref.n)
    if "DATUM" in e:
        return e["DATUM"][4] or ""
    for k in ("DATUM_REFERENCE_ELEMENT", "DATUM_REFERENCE_COMPARTMENT"):
        if k in e:
            return letters_of(m, e[k][4])
    if "DATUM_REFERENCE" in e:
        return letters_of(m, e["DATUM_REFERENCE"][1])
    return ""


def frame(m, args):
    """A GEOMETRIC_TOLERANCE_WITH_DATUM_REFERENCE's datums: 'A|B|C'."""
    out = []
    for r in args[0] if args and isinstance(args[0], list) else []:
        e = m.get(r.n) if isinstance(r, Ref) else {}
        if "DATUM_SYSTEM" in e:
            for c in e["DATUM_SYSTEM"][4] or []:
                out.append(letters_of(m, c))
        elif "DATUM_REFERENCE" in e:
            out.append((int(e["DATUM_REFERENCE"][0] or 0), letters_of(m, r)))
    if out and isinstance(out[0], tuple):
        out = [s for _, s in sorted(out)]
    return "|".join(out)


def tolerances(m, facts):
    zones = {}
    for z in m.by_type("TOLERANCE_ZONE"):
        args = m.get(z)["TOLERANCE_ZONE"]
        form = m.get(args[5].n).get("TOLERANCE_ZONE_FORM", [""])[0] if isinstance(args[5], Ref) else ""
        for t in args[4] or []:
            if isinstance(t, Ref):
                zones[t.n] = (form or "").lower()
    for t in m.ids("GEOMETRIC_TOLERANCE", *["%s_TOLERANCE" % x for x in TOL_TYPES]):
        e = m.get(t)
        kind = next((x for x in TOL_TYPES if "%s_TOLERANCE" % x in e), None)
        if kind is None:
            continue
        base = e.get("GEOMETRIC_TOLERANCE") or e["%s_TOLERANCE" % kind]
        value, _ = measure(m, base[2])
        mods = []
        for k in ("GEOMETRIC_TOLERANCE_WITH_MODIFIERS", "GEOMETRIC_TOLERANCE_WITH_MAXIMUM_TOLERANCE"):
            if k in e and e[k]:
                mods += [x.v for x in e[k][0] if isinstance(x, Enum)]
        # All around / all over: the toleranced shape aspect says.
        target = m.get(base[3].n) if len(base) > 3 and isinstance(base[3], Ref) else {}
        if "ALL_AROUND_SHAPE_ASPECT" in target:
            mods.append("ALL_AROUND")
        if "ALL_OVER_SHAPE_ASPECT" in target:
            mods.append("ALL_OVER")
        if "MODIFIED_GEOMETRIC_TOLERANCE" in e:
            mods += [x.v for x in e["MODIFIED_GEOMETRIC_TOLERANCE"] if isinstance(x, Enum)]
        refs = e.get("GEOMETRIC_TOLERANCE_WITH_DATUM_REFERENCE")
        if refs is None and len(base) >= 5 and isinstance(base[4], list):
            # A simple instance of a subtype with datums: its fifth attribute.
            refs = [base[4]]
        datums = frame(m, refs)
        dia = "D" if "cylindrical" in zones.get(t, "") or "circular" in zones.get(t, "") else "-"
        # Older (AP203/AP214) names of the material modifiers; RFS is the default, no modifier.
        old = {"MAXIMUM_MATERIAL_CONDITION": "MAXIMUM_MATERIAL_REQUIREMENT",
               "LEAST_MATERIAL_CONDITION": "LEAST_MATERIAL_REQUIREMENT"}
        mods = sorted({old.get(x, x) for x in mods if x != "REGARDLESS_OF_FEATURE_SIZE"})
        facts.append(("tol", kind, num(value), dia, ",".join(mods) or "-", datums or "-"))


def num(v):
    return "-" if v is None else "%.6g" % v


def dimensions(m, facts):
    reps = {}
    for d in m.by_type("DIMENSIONAL_CHARACTERISTIC_REPRESENTATION"):
        a = m.get(d)["DIMENSIONAL_CHARACTERISTIC_REPRESENTATION"]
        if isinstance(a[0], Ref) and isinstance(a[1], Ref):
            reps[a[0].n] = a[1].n
    pm = {}
    for p in m.by_type("PLUS_MINUS_TOLERANCE"):
        a = m.get(p)["PLUS_MINUS_TOLERANCE"]
        if isinstance(a[1], Ref):
            pm[a[1].n] = a[0]
    for d in m.ids(*DIMS):
        e = m.get(d)
        if not any(k in e for k in DIMS):
            continue
        value = None
        limits = {}
        rep = m.get(reps[d]) if d in reps else {}
        items = rep.get("SHAPE_DIMENSION_REPRESENTATION", [None, []])[1] or []
        for it in items:
            v, name = measure(m, it)
            if v is None:
                continue
            lname = name.lower()
            if "upper" in lname:
                limits["upper"] = v
            elif "lower" in lname:
                limits["lower"] = v
            elif value is None:
                value = v
        plus = minus = None
        if d in pm and isinstance(pm[d], Ref):
            tv = m.get(pm[d].n)
            if "TOLERANCE_VALUE" in tv:
                lo, _ = measure(m, tv["TOLERANCE_VALUE"][0])
                hi, _ = measure(m, tv["TOLERANCE_VALUE"][1])
                plus, minus = hi, (abs(lo) if lo is not None else None)
            elif "LIMITS_AND_FITS" in tv:
                a = tv["LIMITS_AND_FITS"]
                # Form variance and grade ("G" and "6", or "G6" in the first).
                facts.append(("fit", ("%s%s" % (a[0] or "", a[2] or "")).upper()))
        if "lower" in limits and "upper" in limits:
            facts.append(("range", num(limits["lower"]), num(limits["upper"])))
            value = (limits["lower"] + limits["upper"]) / 2
        facts.append(("dim", num(value), num(plus), num(minus)))


def datums(m, facts):
    seen = set()
    for d in m.by_type("DATUM"):
        letter = m.get(d)["DATUM"][4]
        if letter and letter not in seen:
            seen.add(letter)
            facts.append(("datum", letter))
    targets = len(m.by_type("DATUM_TARGET")) + len(m.by_type("PLACED_DATUM_TARGET_FEATURE"))
    facts.append(("target", str(targets)))


def views(m, facts):
    n = len(m.ids("CAMERA_MODEL_D3", "CAMERA_MODEL_D3_MULTI_CLIPPING"))
    facts.append(("views", str(n)))


def main():
    root = sys.argv[1] if len(sys.argv) > 1 else os.environ.get(
        "SPECULAR_PMI_CORPUS",
        os.path.join(os.path.dirname(os.path.abspath(__file__)), "pmi"))
    lines = []
    for dirpath, _, files in sorted(os.walk(root)):
        for f in sorted(files):
            if not f.lower().endswith((".stp", ".step")):
                continue
            path = os.path.join(dirpath, f)
            rel = os.path.relpath(path, root)
            with open(path, encoding="latin-1") as fh:
                m = Model(fh.read())
            facts = []
            dimensions(m, facts)
            tolerances(m, facts)
            datums(m, facts)
            views(m, facts)
            lines += ["\t".join((rel,) + fact) for fact in facts]
    with open(os.path.join(root, "pmi-oracle.tsv"), "w") as out:
        out.write("\n".join(lines) + "\n")
    print("pmi-oracle: %d facts for %d files" % (len(lines), len({l.split("\t")[0] for l in lines})))


if __name__ == "__main__":
    main()
