#!/usr/bin/env python3
"""Writes the synthetic EPUB fixtures of specular-epub next to this script.

Run with a Python that has fontTools installed (needed for the sample font):

    python3 -I make.py

The sample font is a static, subset instance of the bundled Inter (SIL OFL 1.1, see
crates/specular-theme/fonts/Inter-OFL.txt). Pictures are tiny real PNGs; the crate's tests
read only their IHDR size.
"""

import hashlib
import io
import os
import struct
import sys
import zipfile
import zlib

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", "..", "..", ".."))
INTER = os.path.join(ROOT, "crates", "specular-theme", "fonts", "InterVariable.ttf")

CONTAINER = """<?xml version="1.0"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles><rootfile full-path="{opf}" media-type="application/oebps-package+xml"/></rootfiles>
</container>"""


def png(w, h, rgb):
    raw = b"".join(b"\x00" + bytes(rgb) * w for _ in range(h))

    def chunk(t, d):
        c = struct.pack(">I", len(d)) + t + d
        return c + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)

    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b""))


def sample_font():
    from fontTools import subset
    from fontTools.ttLib import TTFont
    from fontTools.varLib import instancer

    f = TTFont(INTER)
    axes = {a.axisTag: a.defaultValue for a in f["fvar"].axes}
    f = instancer.instantiateVariableFont(f, axes)
    opts = subset.Options()
    opts.name_IDs = ["*"]
    opts.layout_features = ["kern", "liga"]
    s = subset.Subsetter(opts)
    s.populate(text="ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789 .,;:!?'-")
    s.subset(f)
    out = io.BytesIO()
    f.save(out)
    return out.getvalue()


def idpf_obfuscate(data, uid):
    key = hashlib.sha1("".join(c for c in uid if c not in " \t\r\n").encode()).digest()
    b = bytearray(data)
    for i in range(min(1040, len(b))):
        b[i] ^= key[i % 20]
    return bytes(b)


def adobe_obfuscate(data, uid):
    key = bytes.fromhex(uid.replace("urn:uuid:", "").replace("-", ""))
    b = bytearray(data)
    for i in range(min(1024, len(b))):
        b[i] ^= key[i % 16]
    return bytes(b)


def write_epub(name, files, opf_path="OEBPS/content.opf", mimetype=True):
    path = os.path.join(HERE, name)
    with zipfile.ZipFile(path, "w") as z:
        if mimetype:
            z.writestr(zipfile.ZipInfo("mimetype"), "application/epub+zip", zipfile.ZIP_STORED)
        z.writestr("META-INF/container.xml", CONTAINER.format(opf=opf_path), zipfile.ZIP_DEFLATED)
        for n, d in files.items():
            if isinstance(d, str):
                d = d.encode("utf-8")
            z.writestr(n, d, zipfile.ZIP_DEFLATED)
    print(f"{name}: {os.path.getsize(path)} bytes")


def xhtml(title, body, head="", lang="en", epub3=True):
    ns = ' xmlns:epub="http://www.idpf.org/2007/ops"' if epub3 else ""
    return (f'<?xml version="1.0" encoding="utf-8"?>\n<!DOCTYPE html>\n'
            f'<html xmlns="http://www.w3.org/1999/xhtml"{ns} xml:lang="{lang}" lang="{lang}">'
            f"<head><title>{title}</title>{head}</head><body>{body}</body></html>")


LOREM = ("Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor "
         "incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud "
         "exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. ")

CSS = """
body { margin: 0 1em; }
h1 { font-size: 1.8em; text-align: center; page-break-before: always; }
p.first { text-indent: 0; }
p { text-indent: 1.5em; margin: 0.3em 0; text-align: justify; }
.note { color: #800000; font-style: italic; }
div.fig { text-align: center; margin: 1em 0; }
table { border-collapse: collapse; }
td, th { border: 1px solid #888; padding: 2px 4px; }
@media print { p { color: red } }
"""


def reflow3():
    uid = "urn:uuid:8f1c3c2a-6a4e-4d6b-9a39-2f0a7d1e5b10"
    opf = f"""<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="bookid" xml:lang="en">
 <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
  <dc:identifier id="bookid">{uid}</dc:identifier>
  <dc:identifier>isbn:9780000000002</dc:identifier>
  <dc:title id="t1">A Small Reflowable Book</dc:title>
  <dc:title id="t2">Subtitle Here</dc:title>
  <meta refines="#t1" property="title-type">main</meta>
  <meta refines="#t2" property="title-type">subtitle</meta>
  <dc:creator id="c1">Ada Writer</dc:creator>
  <meta refines="#c1" property="role" scheme="marc:relators">aut</meta>
  <meta refines="#c1" property="file-as">Writer, Ada</meta>
  <dc:creator id="c2">Ivan Painter</dc:creator>
  <meta refines="#c2" property="role" scheme="marc:relators">ill</meta>
  <dc:language>en</dc:language>
  <dc:publisher>Specular Fixtures</dc:publisher>
  <dc:date>2026-10-19</dc:date>
  <dc:subject>Testing</dc:subject>
  <dc:description>&lt;p&gt;A book &lt;b&gt;for tests&lt;/b&gt;.&lt;/p&gt;</dc:description>
  <dc:rights>Public domain</dc:rights>
  <meta property="dcterms:modified">2026-10-19T00:00:00Z</meta>
  <meta property="belongs-to-collection">Fixture Series</meta>
 </metadata>
 <manifest>
  <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
  <item id="css" href="css/style.css" media-type="text/css"/>
  <item id="cover" href="images/cover.png" media-type="image/png" properties="cover-image"/>
  <item id="pic" href="images/pic.png" media-type="image/png"/>
  <item id="ch1" href="text/ch1.xhtml" media-type="application/xhtml+xml"/>
  <item id="ch2" href="text/ch2.xhtml" media-type="application/xhtml+xml"/>
  <item id="notes" href="text/notes.xhtml" media-type="application/xhtml+xml"/>
 </manifest>
 <spine>
  <itemref idref="ch1"/>
  <itemref idref="notes" linear="no"/>
  <itemref idref="ch2"/>
 </spine>
</package>"""
    nav = xhtml("Contents", """<nav epub:type="toc" id="toc"><h1>Contents</h1><ol>
 <li><a href="text/ch1.xhtml">Chapter One</a><ol>
   <li><a href="text/ch1.xhtml#lists">Lists</a></li>
   <li><a href="text/ch1.xhtml#table">A table</a></li></ol></li>
 <li><a href="text/ch2.xhtml">Chapter Two</a><ol>
   <li><a href="text/ch2.xhtml#deep">Deep in chapter two</a></li></ol></li>
 <li><span>Back matter</span><ol><li><a href="text/notes.xhtml">Notes</a></li></ol></li>
</ol></nav><nav epub:type="landmarks"><ol><li><a epub:type="cover" href="text/ch1.xhtml">Cover</a></li></ol></nav>""")
    link = '<link rel="stylesheet" type="text/css" href="../css/style.css"/>'
    ch1 = xhtml("Chapter One", f"""
<h1 id="c1">Chapter One</h1>
<p class="first">The <em>first</em> paragraph has <strong>bold</strong>, <i>italic</i>, <u>underlined</u>,
<s>struck</s> and <code>code</code> text, a footnote<sup>1</sup>, H<sub>2</sub>O, an entity&nbsp;&mdash;&nbsp;dash, and a <a href="ch2.xhtml#deep">link</a>.</p>
<p>{LOREM * 2}</p>
<p class="note" style="text-align:right">A note in colour, right aligned.</p>
<div class="fig"><img src="../images/pic.png" alt="a picture"/></div>
<p>Inline <img src="../images/pic.png" width="16" height="16" alt="icon"/> picture and a<br/>line break.</p>
<hr/>
<h2 id="lists">Lists</h2>
<ul><li>Apple</li><li>Pear<ul><li>Nested</li></ul></li></ul>
<ol start="3"><li>Third</li><li>Fourth</li></ol>
<ol style="list-style-type: upper-roman"><li>One</li><li>Two</li></ol>
<dl><dt>Term</dt><dd>Definition of the term.</dd></dl>
<blockquote><p>A quotation, indented on both sides.</p></blockquote>
<pre>  preformatted
    text  kept</pre>
<h2 id="table">A table</h2>
<table border="1"><caption>Fruit</caption><thead><tr><th>Name</th><th>Colour</th></tr></thead>
<tbody><tr><td>Apple</td><td>Red or green</td></tr><tr><td colspan="2">Spanning both columns</td></tr></tbody></table>
<p style="display:none">Hidden text.</p>
""", head=link)
    ch2 = xhtml("Chapter Two", "<h1>Chapter Two</h1>" + "".join(
        f"<p>{LOREM}</p>" for _ in range(40)) + '<p id="deep">The deep anchor.</p>' + "".join(
        f"<p>{LOREM}</p>" for _ in range(10)), head=link)
    notes = xhtml("Notes", '<h1>Notes</h1><p id="n1">1. A note.</p>', head=link)
    write_epub("reflow3.epub", {
        "OEBPS/content.opf": opf, "OEBPS/nav.xhtml": nav, "OEBPS/css/style.css": CSS,
        "OEBPS/images/cover.png": png(60, 90, (40, 80, 160)),
        "OEBPS/images/pic.png": png(120, 60, (200, 120, 40)),
        "OEBPS/text/ch1.xhtml": ch1, "OEBPS/text/ch2.xhtml": ch2, "OEBPS/text/notes.xhtml": notes,
    })


def ncx2():
    uid = "urn:uuid:11111111-2222-3333-4444-555555555555"
    opf = f"""<?xml version="1.0"?>
<package xmlns="http://www.idpf.org/2007/opf" xmlns:opf="http://www.idpf.org/2007/opf" version="2.0" unique-identifier="uid">
 <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
  <dc:title>An EPUB 2 Book</dc:title>
  <dc:creator opf:role="aut" opf:file-as="Author, Old">Old Author</dc:creator>
  <dc:language>fr</dc:language>
  <dc:identifier id="uid" opf:scheme="UUID">{uid}</dc:identifier>
  <meta name="cover" content="cover-img"/>
  <meta name="calibre:series" content="Old Series"/>
 </metadata>
 <manifest>
  <item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/>
  <item id="cover-img" href="cover.png" media-type="image/png"/>
  <item id="coverpage" href="cover.xhtml" media-type="application/xhtml+xml"/>
  <item id="c1" href="chapter%201.html" media-type="application/xhtml+xml"/>
 </manifest>
 <spine toc="ncx"><itemref idref="coverpage"/><itemref idref="c1"/></spine>
 <guide><reference type="cover" title="Cover" href="cover.xhtml"/></guide>
</package>"""
    ncx = """<?xml version="1.0"?>
<ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1"><head/>
<docTitle><text>An EPUB 2 Book</text></docTitle>
<navMap>
 <navPoint id="n1" playOrder="1"><navLabel><text>Cover</text></navLabel><content src="cover.xhtml"/></navPoint>
 <navPoint id="n2" playOrder="2"><navLabel><text>Chapitre un</text></navLabel><content src="chapter%201.html"/>
  <navPoint id="n3" playOrder="3"><navLabel><text>Section</text></navLabel><content src="chapter%201.html#s1"/></navPoint>
 </navPoint>
</navMap></ncx>"""
    cover = xhtml("Cover", '<div><img src="cover.png" alt="Cover" style="height:100%"/></div>', epub3=False)
    # Old-style HTML: unclosed paragraphs, entities, Latin-1.
    c1 = ('<?xml version="1.0" encoding="iso-8859-1"?><html><head><title>Un</title></head><body>'
          '<h1>Chapitre un</h1><p>Caf\xe9 &eacute;t&eacute;<p id="s1">Deuxi\xe8me paragraphe'
          '<p>' + LOREM + '</body></html>').encode("latin-1")
    write_epub("ncx2.epub", {
        "OEBPS/content.opf": opf, "OEBPS/toc.ncx": ncx, "OEBPS/cover.xhtml": cover,
        "OEBPS/cover.png": png(50, 75, (10, 120, 60)), "OEBPS/chapter 1.html": c1,
    })


def fixed():
    opf = """<?xml version="1.0"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id" prefix="rendition: http://www.idpf.org/vocab/rendition/#">
 <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
  <dc:identifier id="id">fixed-1</dc:identifier><dc:title>Fixed Layout</dc:title><dc:language>ja</dc:language>
  <meta property="rendition:layout">pre-paginated</meta>
  <meta property="dcterms:modified">2026-01-01T00:00:00Z</meta>
 </metadata>
 <manifest>
  <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
  <item id="p1" href="p1.xhtml" media-type="application/xhtml+xml"/>
  <item id="p2" href="p2.svg" media-type="image/svg+xml"/>
  <item id="p3" href="p3.xhtml" media-type="application/xhtml+xml" properties="svg"/>
  <item id="p4" href="p4.xhtml" media-type="application/xhtml+xml"/>
  <item id="img1" href="i1.png" media-type="image/png"/>
  <item id="img3" href="i3.png" media-type="image/png"/>
 </manifest>
 <spine page-progression-direction="rtl">
  <itemref idref="p1" properties="page-spread-right"/><itemref idref="p2"/><itemref idref="p3"/><itemref idref="p4"/>
 </spine>
</package>"""
    vp = '<meta name="viewport" content="width=800, height=1200"/>'
    p1 = xhtml("1", '<div><img src="i1.png" alt=""/></div>', head=vp, lang="ja")
    p2 = ('<svg xmlns="http://www.w3.org/2000/svg" width="800" height="1200" viewBox="0 0 800 1200">'
          '<rect width="800" height="1200" fill="#c33"/></svg>')
    p3 = xhtml("3", '<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" '
                    'width="100%" height="100%" viewBox="0 0 800 1200"><image width="800" height="1200" '
                    'xlink:href="i3.png"/></svg>', head=vp, lang="ja")
    p4 = xhtml("4", '<p style="font-size:40px">日本語 text on a fixed page.</p>', head=vp, lang="ja")
    nav = xhtml("Nav", '<nav epub:type="toc"><ol><li><a href="p1.xhtml">Start</a></li>'
                       '<li><a href="p4.xhtml">Text page</a></li></ol></nav>')
    write_epub("fixed.epub", {
        "OPS/package.opf": opf, "OPS/nav.xhtml": nav, "OPS/p1.xhtml": p1, "OPS/p2.svg": p2,
        "OPS/p3.xhtml": p3, "OPS/p4.xhtml": p4,
        "OPS/i1.png": png(80, 120, (250, 250, 0)), "OPS/i3.png": png(80, 120, (0, 200, 200)),
    }, opf_path="OPS/package.opf")


def fonts(font):
    open(os.path.join(HERE, "sample.ttf"), "wb").write(font)
    print(f"sample.ttf: {len(font)} bytes")
    for kind, uid, alg, obf in [
        ("idpf", "urn:uuid:a1b2c3d4-0000-4000-8000-123456789abc  ", "http://www.idpf.org/2008/embedding", idpf_obfuscate),
        ("adobe", "urn:uuid:a1b2c3d4-0000-4000-8000-123456789abc", "http://ns.adobe.com/pdf/enc#RC", adobe_obfuscate),
    ]:
        opf = f"""<?xml version="1.0"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id">
 <metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:identifier id="id">{uid}</dc:identifier>
 <dc:title>Fonts {kind}</dc:title><dc:language>en</dc:language></metadata>
 <manifest><item id="f" href="fonts/sample.ttf" media-type="application/vnd.ms-opentype"/>
 <item id="c" href="c.xhtml" media-type="application/xhtml+xml"/>
 <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/></manifest>
 <spine><itemref idref="c"/></spine></package>"""
        enc = f"""<?xml version="1.0"?>
<encryption xmlns="urn:oasis:names:tc:opendocument:xmlns:container" xmlns:enc="http://www.w3.org/2001/04/xmlenc#">
 <enc:EncryptedData><enc:EncryptionMethod Algorithm="{alg}"/>
 <enc:CipherData><enc:CipherReference URI="OEBPS/fonts/sample.ttf"/></enc:CipherData></enc:EncryptedData>
</encryption>"""
        style = ("<style>@font-face { font-family: 'Book Sample'; src: url(fonts/sample.ttf); }"
                 "p { font-family: 'Book Sample', serif; }</style>")
        c = xhtml("c", "<p>Sample font text in the book face.</p>", head=style)
        nav = xhtml("n", '<nav epub:type="toc"><ol><li><a href="c.xhtml">C</a></li></ol></nav>')
        write_epub(f"fonts-{kind}.epub", {
            "META-INF/encryption.xml": enc, "OEBPS/content.opf": opf, "OEBPS/c.xhtml": c,
            "OEBPS/nav.xhtml": nav, "OEBPS/fonts/sample.ttf": obf(font, uid.strip() if kind == "adobe" else uid),
        })


def drm():
    opf = """<?xml version="1.0"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id">
 <metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:identifier id="id">drm-1</dc:identifier>
 <dc:title>Locked</dc:title><dc:language>en</dc:language></metadata>
 <manifest><item id="c" href="c.xhtml" media-type="application/xhtml+xml"/></manifest>
 <spine><itemref idref="c"/></spine></package>"""

    def enc(key_info):
        return f"""<?xml version="1.0"?>
<encryption xmlns="urn:oasis:names:tc:opendocument:xmlns:container" xmlns:enc="http://www.w3.org/2001/04/xmlenc#" xmlns:ds="http://www.w3.org/2000/09/xmldsig#">
 <enc:EncryptedData><enc:EncryptionMethod Algorithm="http://www.w3.org/2001/04/xmlenc#aes128-cbc"/>
 {key_info}<enc:CipherData><enc:CipherReference URI="OEBPS/c.xhtml"/></enc:CipherData></enc:EncryptedData>
</encryption>"""

    body = bytes(range(7, 71))
    cases = {
        "drm-adobe.epub": {"META-INF/rights.xml": "<adept:rights xmlns:adept='http://ns.adobe.com/adept'/>",
                           "META-INF/encryption.xml": enc("<ds:KeyInfo><resource xmlns='http://ns.adobe.com/adept'>urn:uuid:x</resource></ds:KeyInfo>")},
        "drm-fairplay.epub": {"META-INF/sinf.xml": "<fairplay:sinf xmlns:fairplay='http://itunes.apple.com/ns/epub'/>",
                              "META-INF/encryption.xml": enc("")},
        "drm-lcp.epub": {"META-INF/license.lcpl": "{}",
                         "META-INF/encryption.xml": enc("<ds:KeyInfo><ds:RetrievalMethod URI='license.lcpl#/encryption/content_key'/></ds:KeyInfo>")},
        "drm-unknown.epub": {"META-INF/encryption.xml": enc("")},
    }
    for name, extra in cases.items():
        files = {"OEBPS/content.opf": opf, "OEBPS/c.xhtml": body}
        files.update(extra)
        write_epub(name, files)


def rtl():
    opf = """<?xml version="1.0"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id">
 <metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:identifier id="id">rtl-1</dc:identifier>
 <dc:title>ספר</dc:title><dc:language>he</dc:language>
 <meta property="dcterms:modified">2026-01-01T00:00:00Z</meta></metadata>
 <manifest><item id="c" href="c.xhtml" media-type="application/xhtml+xml"/>
 <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/></manifest>
 <spine page-progression-direction="rtl"><itemref idref="c"/></spine></package>"""
    c = xhtml("c", '<div dir="rtl"><h1>פרק ראשון</h1><p>שלום עולם, זהו ספר לבדיקה עם מילה English באמצע.</p>'
                   '<ul><li>אחד</li><li>שניים</li></ul></div>', lang="he")
    nav = xhtml("n", '<nav epub:type="toc"><ol><li><a href="c.xhtml">פרק ראשון</a></li></ol></nav>', lang="he")
    write_epub("rtl.epub", {"OEBPS/content.opf": opf, "OEBPS/c.xhtml": c, "OEBPS/nav.xhtml": nav})


def main():
    reflow3()
    ncx2()
    fixed()
    fonts(sample_font())
    drm()
    rtl()


if __name__ == "__main__":
    sys.exit(main())
