#!/usr/bin/env python3
"""Generate crates/formats/specular-raw/src/adobe_coeff.rs from dcraw's adobe_coeff() table.

The table carries the ColorMatrix values Adobe DNG Converter writes for each camera (XYZ ->
camera, scaled by 10000), plus optional black / maximum levels. Entries dcraw marks
`/* DJC */` are Dave Coffin's own measurements, not Adobe's, and are left out.

Two further tables are emitted next to it:

* PROFILES: complete DNG colour profiles (ColorMatrix1/2, ForwardMatrix1/2, calibration
  illuminants) read from camera-written DNG files on raw.pixls.us (CC0), listed with their
  SHA-256 in tools/dng-colour-sources.tsv. These are the manufacturer's own calibration for
  bodies the dcraw table predates; a raw file of the same body (PEF next to DNG) uses it.
  The same manifest lists DNGs made with Adobe DNG Converter from raw.pixls.us files of bodies
  newer than dcraw (tools/corpus/adobe-dng.sh regenerates them). Those carry Adobe's own
  profile, the same source as the dcraw table, and are emitted with `adobe: true`; they are
  pinned by a digest of their colour tags because the converter is not byte-reproducible
  (see dng_profiles()).
* SIBLINGS (below): bodies that share the sensor of a body in the dcraw table and reuse its
  Adobe matrix. Each entry was checked against an independent reference before it was added
  (see the comment on each line).

Usage (run Python isolated; the inputs are untrusted downloads):

    curl -o DIR/dcraw.c http://www.dechifro.org/dcraw/dcraw.c   # revision 1.478 (dcraw 9.28)
    tools/corpus/fetch-conformance.sh large_raw large_raw_adobe # the DNGs and source raws
    tools/corpus/adobe-dng.sh                                   # converter-made DNGs
    python3 -I tools/gen-adobe-coeff.py DIR/dcraw.c \
        --dng-sources tools/dng-colour-sources.tsv --dng-root tools/corpus/large/raw \
        > crates/formats/specular-raw/src/adobe_coeff.rs

Without dcraw.c (`python3 -I tools/gen-adobe-coeff.py --dng-sources ...`) it only checks the
manifest against the DNGs.

dcraw.c licence: "No license is required to download and use dcraw.c ... The functions that
process Foveon images have been RESTRICTED since Revision 1.237. All other code remains free for
all uses." adobe_coeff() is not restricted; the numbers themselves are Adobe's published DNG
Converter calibration data.
"""

import argparse
import hashlib
import json
import re
import struct
import sys

# Bodies dcraw covers through a shorter prefix, as listed in its comments. The lookup in
# color.rs only accepts whole-word matches, so these are emitted as explicit aliases.
FAMILIES = {
    "Canon EOS M5": ["Canon EOS M50"],
    "Fujifilm X-T1": ["Fujifilm X-T10"],
    "Fujifilm X-T2": ["Fujifilm X-T20"],
    "Fujifilm XQ": ["Fujifilm XQ1", "Fujifilm XQ2"],
    "Nikon 1 ": ["Nikon 1 J1", "Nikon 1 J2", "Nikon 1 S1", "Nikon 1 V1"],
    "Olympus E-M10": ["Olympus E-M10MarkII", "Olympus E-M10 Mark II", "Olympus E-M10MarkIII",
                      "Olympus E-M10 Mark III"],
    "Olympus E-M1Mark II": ["Olympus E-M1MarkII"],
    "Panasonic DMC-FZ3": ["Panasonic DMC-FZ35", "Panasonic DMC-FZ38"],
    "Panasonic DMC-FZ4": ["Panasonic DMC-FZ40", "Panasonic DMC-FZ45"],
    "Panasonic DMC-FZ7": ["Panasonic DMC-FZ70", "Panasonic DMC-FZ72"],
    "Panasonic DMC-G8": ["Panasonic DMC-G80", "Panasonic DMC-G81", "Panasonic DMC-G85"],
    "Leica X": ["Leica X (Typ 113)", "Leica X-U (Typ 113)"],
    "Phase One P 2": ["Phase One P 20", "Phase One P 20+", "Phase One P 21", "Phase One P 21+",
                      "Phase One P 25", "Phase One P 25+"],
    "Phase One P 30": ["Phase One P 30+"],
    "Phase One P 45": ["Phase One P 45+"],
    # Phase One writes "P40+" and "P65+"; specular names every back "P 40+" style.
    "Phase One P40": ["Phase One P 40", "Phase One P 40+"],
    "Phase One P65": ["Phase One P 65", "Phase One P 65+"],
    "Samsung NX30": ["Samsung NX300", "Samsung NX300M"],
    "Samsung NX2": ["Samsung NX20", "Samsung NX200", "Samsung NX210"],
    "Samsung NX10": ["Samsung NX100"],
    "Sony DSC-RX100M": ["Sony DSC-RX100M2", "Sony DSC-RX100M3", "Sony DSC-RX100M4",
                        "Sony DSC-RX100M5"],
    "Sony DSC-RX10": ["Sony DSC-RX10M2", "Sony DSC-RX10M3"],
    "Sony ILCE-6": ["Sony ILCE-6300", "Sony ILCE-6500"],
    "Sony ILCE-7S": ["Sony ILCE-7SM2"],
    "Sony ILCE": ["Sony ILCE-3000", "Sony ILCE-3500", "Sony ILCE-5000", "Sony ILCE-5100",
                  "Sony ILCE-6000", "Sony ILCE-QX1"],
    "Sony NEX": ["Sony NEX-C3", "Sony NEX-F3"],
}

# Bodies with the same sensor as a body in the table: (alias, source prefix, evidence). They get
# the source's matrix only (levels can differ between bodies). Evidence column: the sensor
# lineage, and the mean CIE76 colour difference over the 24 ColorChecker patches between the
# sibling matrix and LibRaw 0.21.5's own matrix for the alias, used as a measuring oracle only
# (no numbers are taken from it), versus the generic fallback.
SIBLINGS = [
    ("Nikon Z 7", "Nikon D850", "45.7 MP BSI sensor derived from the D850; dE 2.05 vs generic 6.05"),
    ("Nikon Z 7_2", "Nikon D850", "Z 7II, same sensor as Z 7; dE 2.05 vs generic 6.05"),
    ("Nikon Z 50", "Nikon D500", "20.9 MP DX sensor derived from the D500; dE 1.82 vs 4.32"),
    ("Nikon Z fc", "Nikon D500", "Z 50 sensor; dE 1.82 vs generic 4.32"),
    ("Canon EOS RP", "Canon EOS 6D Mark II", "26.2 MP sensor derived from the 6D II; dE 2.67 vs 3.66"),
    ("Canon EOS R", "Canon EOS 5D Mark IV", "30.3 MP sensor derived from the 5D IV; dE 3.36 vs 4.71"),
    ("Canon EOS 250D", "Canon EOS 200D", "24.1 MP sensor of the 200D (same part); dE 2.46 vs generic 5.51"),
    ("Canon EOS 850D", "Canon EOS 800D", "24.1 MP sensor of the 800D (same part); dE 2.63 vs generic 5.51"),
]

# DNG tags read for PROFILES.
T_MAKE, T_MODEL, T_SOFTWARE = 0x010F, 0x0110, 0x0131
T_UNIQUE = 0xC614
T_CM = (0xC621, 0xC622)
T_ILLUM = (0xC65A, 0xC65B)
T_FM = (0xC714, 0xC715)

EXPECTED_SHA256 = "d18d9e43a096eea04eee2148e53068f8fa45ce95395d97128b1aa37b477eab43"


def read_ifd0(data: bytes) -> dict:
    """IFD0 of a TIFF/DNG file: {tag: [values]}; rationals as num / den (exact doubles)."""
    if data[:4] == b"II*\0":
        e = "<"
    elif data[:4] == b"MM\0*":
        e = ">"
    else:
        raise ValueError("not a TIFF file")
    off = struct.unpack_from(e + "I", data, 4)[0]
    n = struct.unpack_from(e + "H", data, off)[0]
    if n > 4096:
        raise ValueError("implausible IFD0 entry count")
    sizes = {1: 1, 2: 1, 3: 2, 4: 4, 5: 8, 6: 1, 7: 1, 8: 2, 9: 4, 10: 8, 11: 4, 12: 8}
    tags = {}
    for i in range(n):
        tag, typ, cnt = struct.unpack_from(e + "HHI", data, off + 2 + 12 * i)
        size = sizes.get(typ)
        if size is None or cnt > 1 << 20:
            continue
        pos = off + 2 + 12 * i + 8
        if size * cnt > 4:
            pos = struct.unpack_from(e + "I", data, pos)[0]
        if pos + size * cnt > len(data):
            raise ValueError(f"tag {tag:#x} out of bounds")
        if typ == 2:
            tags[tag] = data[pos : pos + cnt].split(b"\0")[0].decode("latin-1").strip()
        elif typ in (5, 10):
            f = "I" if typ == 5 else "i"
            v = struct.unpack_from(e + f * (2 * cnt), data, pos)
            tags[tag] = [v[2 * k] / v[2 * k + 1] if v[2 * k + 1] else None for k in range(cnt)]
        else:
            f = {1: "B", 3: "H", 4: "I", 6: "b", 7: "B", 8: "h", 9: "i", 11: "f", 12: "d"}[typ]
            tags[tag] = list(struct.unpack_from(e + f * cnt, data, pos))
    return tags


def profile_digest(ill, cm, fm) -> str:
    """SHA-256 of the colour data the generator takes from a DNG (illuminants, ColorMatrix1/2,
    ForwardMatrix1/2 as exact doubles), independent of the rest of the file."""
    text = json.dumps({"illuminant": ill, "cm": cm, "fm": fm}, separators=(",", ":"))
    return hashlib.sha256(text.encode()).hexdigest()


def dng_profiles(manifest: str, root: str) -> list:
    """(prefix, illuminants, [cm1, cm2], [fm1, fm2], provenance, adobe) per manifest line.

    Camera-written DNGs (no source column) are pinned by the SHA-256 of the whole file. DNGs
    made with Adobe DNG Converter (source column = the raw file they were converted from) are
    pinned by profile_digest(): the converter writes fresh UUIDs and timestamps on every run,
    so the files are not byte-reproducible, but the colour data is. Their Software tag must
    name the converter version given in the note."""
    out = []
    for line in open(manifest, encoding="utf-8"):
        line = line.rstrip("\n")
        if not line.strip() or line.startswith("#"):
            continue
        cols = line.split("\t")
        if len(cols) not in (4, 5):
            raise SystemExit(f"{manifest}: expected 4 or 5 columns: {line!r}")
        prefix, rel, sha, note = cols[:4]
        source = cols[4] if len(cols) == 5 else ""
        data = open(f"{root}/{rel}", "rb").read()
        t = read_ifd0(data)
        cm = [t.get(k) for k in T_CM]
        fm = [t.get(k) for k in T_FM]
        for m in cm + fm:
            if m is not None and (len(m) != 9 or None in m):
                raise SystemExit(f"{rel}: not a 3-colour matrix")
        if cm[0] is None:
            raise SystemExit(f"{rel}: no ColorMatrix1")
        ill = [t.get(k, [0])[0] for k in T_ILLUM]
        software = t.get(T_SOFTWARE, "?")
        if source:
            digest = profile_digest(ill, cm, fm)
            what = "colour-profile digest"
            ver = software.removesuffix(" (Windows)").removesuffix(" (Macintosh)")
            if not ver.startswith("Adobe DNG Converter") or ver not in note:
                raise SystemExit(f"{rel}: Software {software!r} is not the converter in the note")
        else:
            digest = hashlib.sha256(data).hexdigest()
            what = "sha256"
        if digest != sha:
            raise SystemExit(f"{rel}: {what} {digest} does not match the manifest")
        who = t.get(T_UNIQUE) or f"{t.get(T_MAKE, '')} {t.get(T_MODEL, '')}".strip()
        if source:
            prov = f"{rel} ({who}, profile digest {sha[:16]}...): {note}"
        else:
            prov = f"{rel} (sha256 {sha[:16]}..., {who}, {software}): {note}"
        out.append((prefix, ill, cm, fm, prov, bool(source)))
    return out


def fmt_matrix(m) -> str:
    if m is None:
        return "None"
    return "Some([" + ", ".join(repr(float(v)) for v in m) + "])"


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("dcraw", nargs="?", help="dcraw.c; without it, only check --dng-sources")
    ap.add_argument("--dng-sources", help="manifest of reference DNGs (TSV)")
    ap.add_argument("--dng-root", default="tools/corpus/large/raw")
    args = ap.parse_args()
    path = args.dcraw
    profiles = dng_profiles(args.dng_sources, args.dng_root) if args.dng_sources else []
    if path is None:
        for p in profiles:
            print(f"ok\t{p[0]}\t{p[4]}")
        return 0
    raw = open(path, "rb").read()
    digest = hashlib.sha256(raw).hexdigest()
    if digest != EXPECTED_SHA256:
        print(f"warning: sha256 {digest} differs from the reviewed revision", file=sys.stderr)
    src = raw.decode("latin-1")
    rev = re.search(r"\$Revision: ([0-9.]+) \$", src)
    date = re.search(r"\$Date: ([0-9/]+)", src)
    start = src.index("void CLASS adobe_coeff")
    body = src[src.index("table[] = {", start) : src.index("double cam_xyz[4][3];", start)]
    entry = re.compile(
        r'\{\s*"([^"]+)",\s*(-?\w+),\s*(-?\w+),\s*(/\*.*?\*/)?\s*\{([^}]*)\}\s*\}', re.S
    )
    rows = []
    djc = 0
    for m in entry.finditer(body):
        prefix, black, maximum, comment, nums = m.groups()
        if comment and "DJC" in comment:
            djc += 1
            continue
        vals = [int(v) for v in nums.replace("\n", " ").split(",") if v.strip()]
        vals += [0] * (12 - len(vals))
        matrix = None
        # Four-colour (CMYG) entries use all 12 values; only 3-colour matrices are kept.
        if vals[0] != 0 and not any(vals[9:]):
            matrix = vals[:9]
        for alias in FAMILIES.get(prefix, []):
            rows.append((alias, int(black, 0), int(maximum, 0), matrix, ""))
        rows.append((prefix, int(black, 0), int(maximum, 0), matrix, ""))

    by_prefix = {}
    for r in rows:
        by_prefix.setdefault(r[0], r)
    with_profile = {p[0] for p in profiles}
    superseded = [s for s in SIBLINGS if s[0] in with_profile]
    for alias, source, _ in SIBLINGS:
        if alias in with_profile:
            continue
        if alias in by_prefix:
            raise SystemExit(f"sibling {alias} is already in the table")
        src_matrix = by_prefix[source][3]
        if src_matrix is None:
            raise SystemExit(f"sibling source {source} has no matrix")
        rows.append((alias, 0, 0, src_matrix, source))

    out = []
    out.append("//! GENERATED by tools/gen-adobe-coeff.py -- do not edit by hand.")
    out.append("//!")
    out.append("//! Per-camera colour matrices as written by Adobe DNG Converter (ColorMatrix for the")
    out.append("//! D65 calibration illuminant, XYZ -> camera, x10000), with black / maximum levels, taken")
    out.append(f"//! from dcraw.c revision {rev.group(1) if rev else '?'} ({date.group(1) if date else '?'}),")
    out.append(f"//! adobe_coeff() table, sha256 {digest}.")
    out.append(f"//! {len(rows)} entries; {djc} non-Adobe (`/* DJC */`) entries excluded.")
    out.append("//! dcraw.c: \"All other code [than the Foveon functions] remains free for all uses.\"")
    out.append("//! Lookup: first entry whose prefix matches \"Make Model\" (normalised make) as whole")
    out.append("//! words, in order; bodies dcraw's comments list under a shorter prefix are expanded into")
    out.append("//! explicit aliases (see FAMILIES in the script).")
    out.append("//!")
    out.append("//! Same-sensor siblings appended to TABLE (matrix of the named body, levels not copied):")
    for alias, source, why in SIBLINGS:
        if alias not in with_profile:
            out.append(f"//! - {alias} <- {source}: {why}.")
    if len(superseded) == len(SIBLINGS):
        out.append("//! (none at present)")
    if superseded:
        out.append("//!")
        out.append("//! Siblings dropped because the body now has its own profile (PROFILES below):")
        out.append("//! " + ", ".join(s[0] for s in superseded) + ".")
    out.append("//!")
    camera = [p for p in profiles if not p[5]]
    adobe = [p for p in profiles if p[5]]
    out.append(f"//! PROFILES: {len(profiles)} DNG colour profiles (manifest tools/dng-colour-sources.tsv).")
    out.append(f"//! {len(camera)} read from camera-written reference DNGs (in-camera profile; files fetched by")
    out.append("//! tools/corpus/fetch-conformance.sh large_raw, all CC0-1.0 from raw.pixls.us):")
    for p in camera:
        out.append(f"//! - {p[0]}: {p[4]}")
    if adobe:
        out.append("//!")
        out.append(f"//! {len(adobe)} read from DNGs made with Adobe DNG Converter from raw.pixls.us files (CC0-1.0;")
        out.append("//! fetched by fetch-conformance.sh large_raw_adobe, converted by tools/corpus/adobe-dng.sh):")
        out.append("//! Adobe's own profile for the body, the same source as TABLE, so `adobe: true`.")
        out.append("//! Their ForwardMatrix is left out (`fm: [None, None]`): Adobe Standard pairs it with a")
        out.append("//! HueSatMap / LookTable that restores saturation, which this table does not carry, so on its own")
        out.append("//! it renders far too flat (mean dE76 ~15 on a ColorChecker vs ~1 for the ColorMatrix).")
        out.append("//! (A DNG's own profile is applied in full: see profile.rs.)")
        out.append("//!")
        for p in adobe:
            out.append(f"//! - {p[0]}: {p[4]}")
    out.append("")
    out.append("pub(crate) struct Entry {")
    out.append("    pub prefix: &'static str,")
    out.append("    pub black: u16,")
    out.append("    pub maximum: u16,")
    out.append("    pub matrix: Option<[i16; 9]>,")
    out.append("    /// Non-empty: the matrix is that body's (same sensor), not this one's.")
    out.append("    pub sibling_of: &'static str,")
    out.append("}")
    out.append("")
    out.append("/// A DNG colour profile: ColorMatrix1/2 (XYZ -> camera), ForwardMatrix1/2 (camera -> XYZ D50)")
    out.append("/// and the EXIF light-source codes of the two calibrations.")
    out.append("pub(crate) struct Profile {")
    out.append("    pub prefix: &'static str,")
    out.append("    pub illuminant: [u32; 2],")
    out.append("    pub cm: [Option<[f64; 9]>; 2],")
    out.append("    pub fm: [Option<[f64; 9]>; 2],")
    out.append("    /// Made by Adobe DNG Converter (Adobe's profile), not written by the camera.")
    out.append("    pub adobe: bool,")
    out.append("}")
    out.append("")
    out.append("#[rustfmt::skip]")
    out.append("pub(crate) static PROFILES: &[Profile] = &[")
    for prefix, ill, cm, fm, _, is_adobe in profiles:
        if is_adobe:
            fm = [None, None]  # see the header: Adobe Standard's FM needs its HueSatMap
        out.append(f'    Profile {{ prefix: "{prefix}", illuminant: [{ill[0]}, {ill[1]}],')
        out.append(f"        cm: [{fmt_matrix(cm[0])},")
        out.append(f"             {fmt_matrix(cm[1])}],")
        out.append(f"        fm: [{fmt_matrix(fm[0])}, {fmt_matrix(fm[1])}],")
        out.append(f"        adobe: {'true' if is_adobe else 'false'} }},")
    out.append("];")
    out.append("")
    out.append("#[rustfmt::skip]")
    out.append("pub(crate) static TABLE: &[Entry] = &[")
    for prefix, black, maximum, matrix, sib in rows:
        m = "None" if matrix is None else "Some([" + ", ".join(str(v) for v in matrix) + "])"
        out.append(
            f'    Entry {{ prefix: "{prefix}", black: {black}, maximum: {maximum}, matrix: {m}, '
            f'sibling_of: "{sib}" }},'
        )
    out.append("];")
    print("\n".join(out))
    return 0


if __name__ == "__main__":
    sys.exit(main())
