#!/usr/bin/env python3
"""Office preview corpus and oracle for specular-office (gitignored, never committed).

Usage:
  cargo run -p specular-office --example write_fixtures -- OUT/own-legacy
  PYTHON=<venv>/bin/python tools/corpus/gen-office.py [OUT]
OUT defaults to $SPECULAR_OFFICE_CORPUS, else tools/corpus/office. The venv needs python-docx,
openpyxl, python-pptx, odfpy, olefile and Pillow (a venv under target/).

Writes:
  real/         Apache POI test documents (Apache-2.0) at a pinned commit: Word, Excel and
                PowerPoint 97-2003 and OOXML, macro and strict variants, a package thumbnail,
                a 97-2003 summary-information thumbnail and password-protected packages
  own/          documents made here with python-docx, openpyxl, python-pptx and odfpy (with
                thumbnails added the way Office stores them), template/macro variants by
                content type, and iWork packages
  own-legacy/   the Rust fixture writer's 97-2003 files (see Usage)
  oracle.json   per file: what the libraries read (core properties through python-docx,
                openpyxl, python-pptx; meta.xml through odfpy; summary information through
                olefile), extended properties and the thumbnail through zipfile and
                ElementTree, and the thumbnail's SHA-256
Downloads are untrusted: nothing in them is run; they are only read by the libraries.
"""
import datetime
import hashlib
import io
import json
import os
import re
import sys
import urllib.request
import xml.etree.ElementTree as ET
import zipfile

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
OUT = sys.argv[1] if len(sys.argv) > 1 else os.environ.get(
    "SPECULAR_OFFICE_CORPUS", os.path.join(ROOT, "tools/corpus/office"))
POI = "https://raw.githubusercontent.com/apache/poi/bd15f5bc11c1433725d031642bdfaf64f4c2ce72/test-data/"
REAL = [
    "document/SampleDoc.doc", "document/SampleDoc.docx", "document/SimpleMacro.doc",
    "document/SimpleMacro.docm", "document/empty.doc", "document/protected_sample.docx",
    "document/bug53475-password-is-pass.docx", "document/PasswordProtected.doc",
    "spreadsheet/SampleSS.xls", "spreadsheet/SampleSS.xlsx", "spreadsheet/SampleSS.strict.xlsx",
    "spreadsheet/SampleSS.ods", "spreadsheet/Simple.xlsb", "spreadsheet/SimpleMacro.xlsm",
    "spreadsheet/protected_passtika.xlsx", "spreadsheet/sample.xlsx",
    "slideshow/SampleShow.ppt", "slideshow/SampleShow.pptx", "slideshow/SimpleMacro.pptm",
    "slideshow/empty.ppt", "slideshow/sample.pptx",
    "openxml4j/TestPackageThumbnail.docx", "hpsf/TestThumbnail.xls",
    "poifs/protected_agile.docx", "poifs/protect.xlsx",
]

for d in ("real", "own", "own-legacy"):
    os.makedirs(os.path.join(OUT, d), exist_ok=True)

for path in REAL:
    dst = os.path.join(OUT, "real", os.path.basename(path))
    if not os.path.exists(dst):
        try:
            with urllib.request.urlopen(POI + path, timeout=60) as r:
                data = r.read()
            open(dst, "wb").write(data)
        except Exception as e:
            print(f"gen-office: {path} not fetched: {e}", file=sys.stderr)

# ----- Own documents ----------------------------------------------------------------------------
import docx
import openpyxl
import pptx
from PIL import Image
from odf import meta as odfmeta, dc as odfdc
from odf.opendocument import OpenDocumentSpreadsheet, OpenDocumentText, load as odfload
from odf.text import P
from odf.table import Table


def picture(fmt, size=(96, 128)):
    im = Image.new("RGB", size, (240, 240, 230))
    for y in range(8, size[1] - 8, 10):
        for x in range(8, size[0] - 8):
            im.putpixel((x, y), (60, 60, 160))
    b = io.BytesIO()
    im.save(b, fmt)
    return b.getvalue()


JPEG, PNG = picture("JPEG"), picture("PNG")
WHEN = datetime.datetime(2024, 5, 6, 7, 8, 9)
REL_THUMB = "http://schemas.openxmlformats.org/package/2006/relationships/metadata/thumbnail"


def rewrite(src, dst, add=None, main_type=None, thumb=None):
    """Copies a package, optionally adding a thumbnail part (with its relationship and
    content type) or changing the main part's content type."""
    zin = zipfile.ZipFile(src)
    # A template that has a thumbnail already (python-pptx's) gets ours in its place.
    has_rel = b"/metadata/thumbnail" in zin.read("_rels/.rels")
    out = io.BytesIO()
    zout = zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED)
    for item in zin.infolist():
        data = zin.read(item.filename)
        if item.filename == "[Content_Types].xml":
            s = data.decode()
            if main_type:
                s = re.sub(r'(ContentType=")([^"]*\.main\+xml|[^"]*\.main)(")',
                           lambda m: m.group(1) + main_type + m.group(3), s, count=1)
            if thumb:
                ext = thumb[0].rsplit(".", 1)[1]
                mime = {"jpeg": "image/jpeg", "png": "image/png"}[ext]
                if f'Extension="{ext}"' not in s:
                    s = s.replace("</Types>", f'<Default Extension="{ext}" ContentType="{mime}"/></Types>')
            data = s.encode()
        if thumb and item.filename.startswith("docProps/thumbnail."):
            continue
        if item.filename == "_rels/.rels" and thumb and has_rel:
            s = re.sub(r'(Type="[^"]*/metadata/thumbnail" Target=")[^"]*(")',
                       lambda m: m.group(1) + thumb[0] + m.group(2), data.decode())
            s = re.sub(r'(Target=")[^"]*(" Type="[^"]*/metadata/thumbnail")',
                       lambda m: m.group(1) + thumb[0] + m.group(2), s)
            data = s.encode()
        elif item.filename == "_rels/.rels" and thumb:
            s = data.decode().replace(
                "</Relationships>",
                f'<Relationship Id="rIdThumb" Type="{REL_THUMB}" Target="{thumb[0]}"/></Relationships>')
            data = s.encode()
        zout.writestr(item, data)
    if thumb:
        zout.writestr(thumb[0], thumb[1])
    for name, data in (add or {}).items():
        zout.writestr(name, data)
    zout.close()
    open(dst, "wb").write(out.getvalue())


own = os.path.join(OUT, "own")
tmp = os.path.join(own, "_tmp")

d = docx.Document()
d.add_paragraph("Hello")
cp = d.core_properties
cp.title, cp.author, cp.subject, cp.keywords = "Docx title", "Ann", "Plans", "a, b"
cp.last_modified_by, cp.revision, cp.created, cp.modified = "Bob", 3, WHEN, WHEN
d.save(tmp)
rewrite(tmp, os.path.join(own, "thumb.docx"), thumb=("docProps/thumbnail.jpeg", JPEG))
rewrite(tmp, os.path.join(own, "nothumb.docx"))
rewrite(tmp, os.path.join(own, "macro.docm"), thumb=("docProps/thumbnail.png", PNG),
        main_type="application/vnd.ms-word.document.macroEnabled.main+xml")
rewrite(tmp, os.path.join(own, "template.dotx"),
        main_type="application/vnd.openxmlformats-officedocument.wordprocessingml.template.main+xml")

wb = openpyxl.Workbook()
wb.active.title = "One"
wb.create_sheet("Two")
wb.create_sheet("Three")
wb.properties.title, wb.properties.creator, wb.properties.lastModifiedBy = "Sheets", "Eve", "Fay"
wb.properties.created = wb.properties.modified = WHEN
wb.save(tmp)
rewrite(tmp, os.path.join(own, "thumb.xlsx"), thumb=("docProps/thumbnail.jpeg", JPEG))
rewrite(tmp, os.path.join(own, "nothumb.xlsx"))
rewrite(tmp, os.path.join(own, "macro.xlsm"),
        main_type="application/vnd.ms-excel.sheet.macroEnabled.main+xml")
rewrite(tmp, os.path.join(own, "template.xltx"), thumb=("docProps/thumbnail.png", PNG),
        main_type="application/vnd.openxmlformats-officedocument.spreadsheetml.template.main+xml")

p = pptx.Presentation()
for _ in range(3):
    p.slides.add_slide(p.slide_layouts[6])
cp = p.core_properties
cp.title, cp.author, cp.created, cp.modified = "Deck", "Gus", WHEN, WHEN
p.save(tmp)
rewrite(tmp, os.path.join(own, "thumb.pptx"), thumb=("docProps/thumbnail.jpeg", JPEG))
rewrite(tmp, os.path.join(own, "show.ppsx"),
        main_type="application/vnd.openxmlformats-officedocument.presentationml.slideshow.main+xml")
rewrite(tmp, os.path.join(own, "macro.pptm"), thumb=("docProps/thumbnail.png", PNG),
        main_type="application/vnd.ms-powerpoint.presentation.macroEnabled.main+xml")
os.remove(tmp)


def odf_doc(doc, path, thumb):
    doc.meta.addElement(odfdc.Title(text="ODF title"))
    doc.meta.addElement(odfmeta.InitialCreator(text="Hal"))
    doc.meta.addElement(odfdc.Creator(text="Ivy"))
    doc.meta.addElement(odfmeta.CreationDate(text="2024-05-06T07:08:09"))
    doc.meta.addElement(odfmeta.Keyword(text="k1"))
    doc.meta.addElement(odfmeta.Keyword(text="k2"))
    doc.meta.addElement(odfmeta.DocumentStatistic(pagecount="2", wordcount="17", tablecount="1"))
    if thumb:
        doc.addThumbnail(PNG)
    doc.save(path)


t = OpenDocumentText()
t.text.addElement(P(text="Hello"))
odf_doc(t, os.path.join(own, "thumb.odt"), True)
s = OpenDocumentSpreadsheet()
s.spreadsheet.addElement(Table(name="T"))
odf_doc(s, os.path.join(own, "nothumb.ods"), False)


def iwork(path, members):
    with zipfile.ZipFile(path, "w", zipfile.ZIP_STORED) as z:
        for n, data in members:
            z.writestr(n, data)


iwork(os.path.join(own, "doc.pages"), [("Index/Document.iwa", b"\0" * 32),
                                        ("Metadata/Properties.plist", b"bplist00"),
                                        ("preview-micro.jpg", picture("JPEG", (16, 24))),
                                        ("preview.jpg", JPEG)])
iwork(os.path.join(own, "sheet.numbers"), [("Index/Document.iwa", b"\0" * 32),
                                            ("Index/CalculationEngine.iwa", b"\0" * 8),
                                            ("preview.jpg", JPEG)])
iwork(os.path.join(own, "slides.key"), [("Index/Document.iwa", b"\0" * 32),
                                         ("Index/Slide-1.iwa", b"\0" * 8)])
iwork(os.path.join(own, "old.key"), [("index.apxl.gz", b"\x1f\x8b"),
                                      ("QuickLook/Thumbnail.jpg", JPEG)])

# ----- Oracle -----------------------------------------------------------------------------------
import olefile


def iso(v):
    if v is None or v == "":
        return None
    if isinstance(v, datetime.datetime):
        if v.tzinfo:
            v = v.astimezone(datetime.timezone.utc).replace(tzinfo=None)
        return v.replace(microsecond=0).isoformat()
    return str(v)


def text(v):
    if isinstance(v, bytes):
        v = v.decode("cp1252", "replace")
    v = None if v is None else str(v).strip().split("\0")[0].strip()
    return v or None


def opc(path):
    """Extended properties and the thumbnail, read with zipfile and ElementTree."""
    z = zipfile.ZipFile(path)
    out = {}
    names = z.namelist()
    if "docProps/app.xml" in names:
        root = ET.fromstring(z.read("docProps/app.xml"))
        for el in root:
            tag = el.tag.split("}")[-1]
            if tag in ("Pages", "Words", "Slides", "Notes", "Characters", "HiddenSlides"):
                out[tag.lower() if tag != "HiddenSlides" else "hidden_slides"] = int(el.text)
    thumb = None
    if "_rels/.rels" in names:
        for r in ET.fromstring(z.read("_rels/.rels")):
            if r.get("Type", "").endswith("/metadata/thumbnail"):
                thumb = r.get("Target").lstrip("/")
    if thumb is None:
        thumb = next((n for n in names if n.startswith("docProps/thumbnail.")), None)
    if thumb and thumb in names:
        out["preview_sha256"] = hashlib.sha256(z.read(thumb)).hexdigest()
    else:
        out["preview_sha256"] = None
    return out


def core_by_hand(path):
    """docProps/core.xml through ElementTree, for packages the libraries refuse."""
    z = zipfile.ZipFile(path)
    if "docProps/core.xml" not in z.namelist():
        return {}
    vals = {el.tag.split("}")[-1]: el.text for el in ET.fromstring(z.read("docProps/core.xml"))}
    when = lambda k: iso(datetime.datetime.fromisoformat(vals[k].replace("Z", "+00:00"))) if vals.get(k) else None
    return {"title": text(vals.get("title")), "author": text(vals.get("creator")),
            "subject": text(vals.get("subject")), "keywords": text(vals.get("keywords")),
            "last_modified_by": text(vals.get("lastModifiedBy")),
            "created": when("created"), "modified": when("modified")}


def core(c):
    return {"title": text(c.title), "author": text(c.author), "subject": text(c.subject),
            "keywords": text(c.keywords), "last_modified_by": text(c.last_modified_by),
            "created": iso(c.created), "modified": iso(c.modified)}


oracle = {}
for sub in ("real", "own", "own-legacy"):
    for name in sorted(os.listdir(os.path.join(OUT, sub))):
        path = os.path.join(OUT, sub, name)
        rel = f"{sub}/{name}"
        ext = name.rsplit(".", 1)[-1].lower()
        e = {}
        try:
            if olefile.isOleFile(path):
                ole = olefile.OleFileIO(path)
                if ole.exists("EncryptedPackage") and ole.exists("EncryptionInfo"):
                    e["encrypted"] = True
                else:
                    m = ole.get_metadata()
                    e.update({"title": text(m.title), "author": text(m.author),
                              "subject": text(m.subject), "keywords": text(m.keywords),
                              "last_modified_by": text(m.last_saved_by),
                              "created": iso(m.create_time), "modified": iso(m.last_saved_time),
                              "pages": m.num_pages, "words": m.num_words,
                              "slides": m.slides, "company": text(m.company)})
                    th = m.thumbnail
                    e["clip"] = None if th is None else th.hex()
            elif ext in ("docx", "docm", "dotx", "dotm"):
                try:
                    e.update(core(docx.Document(path).core_properties))
                except ValueError:
                    # python-docx only opens .docx content types: the core part by hand.
                    e.update(core_by_hand(path))
                e.update(opc(path))
            elif ext in ("xlsx", "xlsm", "xltx", "xltm"):
                wbk = openpyxl.load_workbook(path, read_only=True)
                pr = wbk.properties
                e.update({"title": text(pr.title), "author": text(pr.creator),
                          "last_modified_by": text(pr.lastModifiedBy),
                          "created": iso(pr.created), "modified": iso(pr.modified),
                          "sheets": len(wbk.sheetnames)})
                e.update(opc(path))
            elif ext == "xlsb":
                e.update(core_by_hand(path))
                e.update(opc(path))
            elif ext in ("pptx", "pptm", "ppsx", "potx"):
                try:
                    pr = pptx.Presentation(path)
                    e.update(core(pr.core_properties))
                    e["slides_counted"] = len(pr.slides)
                except ValueError:
                    e.update(core_by_hand(path))
                e.update(opc(path))
            elif ext in ("odt", "ods", "odp", "odg"):
                doc = odfload(path)
                vals = {}
                for el in doc.meta.childNodes:
                    vals.setdefault(el.qname[1], []).append(el)
                get = lambda k: text(str(vals[k][0])) if k in vals else None
                e.update({"title": get("title"), "author": get("initial-creator") or get("creator"),
                          "last_modified_by": get("creator"), "created": iso(get("creation-date"))})
                kw = [str(k) for k in vals.get("keyword", [])]
                e["keywords"] = ", ".join(kw) if kw else None
                if "document-statistic" in vals:
                    st = vals["document-statistic"][0]
                    for a, k in (("page-count", "pages"), ("word-count", "words"), ("table-count", "sheets")):
                        v = st.getAttrNS("urn:oasis:names:tc:opendocument:xmlns:meta:1.0", a)
                        if v is not None:
                            e[k] = int(v)
                z = zipfile.ZipFile(path)
                e["preview_sha256"] = (hashlib.sha256(z.read("Thumbnails/thumbnail.png")).hexdigest()
                                       if "Thumbnails/thumbnail.png" in z.namelist() else None)
            elif ext in ("pages", "numbers", "key"):
                z = zipfile.ZipFile(path)
                for n in ("preview.jpg", "QuickLook/Thumbnail.jpg", "QuickLook/Thumbnail.png",
                          "preview-web.jpg", "preview-micro.jpg"):
                    if n in z.namelist():
                        e["preview_sha256"] = hashlib.sha256(z.read(n)).hexdigest()
                        break
                else:
                    e["preview_sha256"] = None
            else:
                continue
        except Exception as ex:
            e["oracle_error"] = f"{type(ex).__name__}: {ex}"
        oracle[rel] = e

json.dump(oracle, open(os.path.join(OUT, "oracle.json"), "w"), indent=1, sort_keys=True)
print(f"gen-office: {len(oracle)} files in {OUT}")
