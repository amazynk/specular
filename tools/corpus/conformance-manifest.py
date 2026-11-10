#!/usr/bin/env python3
"""Regenerate the per-file tables in tools/corpus/conformance/MANIFEST.md.

Reads <root>/.provenance.tsv (rel-path, source URL, licence; last entry wins),
hashes every file that exists, and replaces everything after the
'<!-- BEGIN GENERATED -->' marker. Never looks at tools/corpus/real/.
Run with: python3 -I tools/corpus/conformance-manifest.py [ROOT ...]
"""
import hashlib, os, sys

HERE = os.path.dirname(os.path.abspath(__file__))
MARK = "<!-- BEGIN GENERATED -->"

FMT = {
    "jpg": "JPEG", "jpeg": "JPEG", "jpe": "JPEG", "mpo": "JPEG/MPO", "png": "PNG", "apng": "APNG",
    "webp": "WebP", "gif": "GIF", "bmp": "BMP", "dib": "BMP", "ico": "ICO", "cur": "CUR", "ani": "ANI",
    "qoi": "QOI", "pbm": "PNM", "pgm": "PNM", "ppm": "PNM", "pnm": "PNM", "pam": "PAM", "pfm": "PFM",
    "tga": "TGA", "ff": "Farbfeld", "farbfeld": "Farbfeld", "ras": "Sun Raster", "sun": "Sun Raster",
    "sgi": "SGI", "rgb": "SGI", "rgba": "SGI", "bw": "SGI", "fli": "FLI", "flc": "FLC", "pcx": "PCX",
    "xpm": "XPM", "xbm": "XBM", "wbmp": "WBMP", "svg": "SVG", "svgz": "SVGZ", "tif": "TIFF",
    "tiff": "TIFF", "btf": "BigTIFF", "psd": "PSD", "psb": "PSB", "dds": "DDS", "ktx2": "KTX2",
    "ktx": "KTX1", "basis": "Basis", "astc": "ASTC", "jxl": "JPEG XL", "avif": "AVIF",
    "avifs": "AVIF seq", "heic": "HEIC", "heif": "HEIF", "heics": "HEIF seq", "hif": "HEIF",
    "avci": "HEIF/AVC", "jp2": "JP2", "j2k": "J2K", "j2c": "J2K", "jpc": "J2K", "jph": "HTJ2K",
    "jhc": "HTJ2K", "jpx": "JPX", "jpf": "JPX", "exr": "EXR", "hdr": "Radiance", "pic": "Radiance",
    "zip": "ZIP", "zipx": "ZIP", "cbz": "CBZ", "rar": "RAR", "cbr": "CBR", "7z": "7z", "tar": "TAR",
    "xcf": "XCF", "icns": "ICNS", "iff": "IFF", "ilbm": "IFF", "lbm": "IFF", "pict": "PICT",
    "pct": "PICT",
    "cr2": "RAW/CR2", "cr3": "RAW/CR3", "crw": "RAW/CRW", "nef": "RAW/NEF", "arw": "RAW/ARW",
    "raf": "RAW/RAF", "orf": "RAW/ORF", "rw2": "RAW/RW2", "pef": "RAW/PEF", "dng": "RAW/DNG",
    "3fr": "RAW/3FR", "srw": "RAW/SRW", "x3f": "RAW/X3F", "mrw": "RAW/MRW", "dcr": "RAW/DCR",
    "erf": "RAW/ERF", "gpr": "RAW/GPR", "iiq": "RAW/IIQ",
}


def fmt_of(path):
    base = os.path.basename(path).lower()
    ext = base.rsplit(".", 1)[-1] if "." in base else ""
    return FMT.get(ext, ext.upper() or "?")


def feature_of(rel):
    # "source/a/b/name.ext" -> "a/b: name" (dir context + stem); short and grep-able.
    parts = rel.split("/")[1:]
    stem = parts[-1].rsplit(".", 1)[0]
    ctx = "/".join(p for p in parts[:-1] if p not in ("tests", "test", "Tests", "images", "data", "testFiles"))
    return (ctx + ": " if ctx else "") + stem


def sha256(p):
    h = hashlib.sha256()
    with open(p, "rb") as f:
        for b in iter(lambda: f.read(1 << 20), b""):
            h.update(b)
    return h.hexdigest()


def esc(s):
    return s.replace("|", "\\|")


def table(root, label):
    prov = {}
    tsv = os.path.join(root, ".provenance.tsv")
    if os.path.exists(tsv):
        with open(tsv, encoding="utf-8") as f:
            for line in f:
                cols = line.rstrip("\n").split("\t")
                if len(cols) >= 2:
                    prov[cols[0]] = (cols[1], cols[2] if len(cols) > 2 else "unknown")
        # dedupe in place (last entry wins), keep stable order
        with open(tsv, "w", encoding="utf-8") as f:
            for k in sorted(prov):
                f.write(f"{k}\t{prov[k][0]}\t{prov[k][1]}\n")
    rows, total = [], 0
    for rel in sorted(prov):
        p = os.path.join(root, rel)
        if not os.path.isfile(p) or os.path.islink(p):
            continue
        sz = os.path.getsize(p)
        total += sz
        url, lic = prov[rel]
        rows.append(f"| `{esc(rel)}` | {fmt_of(rel)} | {esc(feature_of(rel))} | {sz} | [src]({url}) | {esc(lic or 'unknown')} | `{sha256(p)}` |")
    out = [f"### {label}: {len(rows)} files, {total / 1e6:.1f} MB", ""]
    if rows:
        out += ["| Path | Format | Feature (from path) | Bytes | Source | Licence | SHA-256 |",
                "|---|---|---|---:|---|---|---|"] + rows
    else:
        out.append("_Nothing fetched yet; run `tools/corpus/fetch-conformance.sh --large large_raw large_files`._")
    return out


def main():
    conf = os.path.join(HERE, "conformance")
    large = os.path.join(HERE, "large")
    man = os.path.join(conf, "MANIFEST.md")
    with open(man, encoding="utf-8") as f:
        head = f.read().split(MARK)[0]
    body = [MARK, "", "## Per-file manifest (generated)", ""]
    body += table(conf, "tools/corpus/conformance/") + [""]
    body += table(large, "tools/corpus/large/") + [""]
    with open(man, "w", encoding="utf-8") as f:
        f.write(head + "\n".join(body))


if __name__ == "__main__":
    main()
