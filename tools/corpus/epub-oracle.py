#!/usr/bin/env python3
"""EPUB package oracle: what ebooklib reads from each book, for specular-epub's oracle test.

    venv/bin/python -I tools/corpus/epub-oracle.py DIR... [-o OUT.tsv]

Writes one line per fact, `<absolute path>\\t<key>\\t<value>`, to OUT (default
tools/corpus/epub/oracle.tsv). Keys: title, creator, language, identifier, spine
(`href\\tlinear`), toc (`title\\thref#fragment`), cover, error. Paths are archive paths
(resolved against the package file's folder). The Rust test
(crates/formats/specular-epub/tests/oracle.rs) reads the same books and counts mismatches.
"""

import os
import posixpath
import sys
import urllib.parse
import warnings
import zipfile

warnings.filterwarnings("ignore")

import ebooklib  # noqa: E402
from ebooklib import epub  # noqa: E402


def opf_dir(path):
    with zipfile.ZipFile(path) as z:
        c = z.read("META-INF/container.xml").decode("utf-8", "replace")
    i = c.find("full-path=")
    q = c[i + 10]
    p = c[i + 11:c.index(q, i + 11)]
    return posixpath.dirname(p)


def toc_dir(path, base):
    """The folder of the table of contents file (nav, else NCX): its hrefs are relative to it."""
    import re
    with zipfile.ZipFile(path) as z:
        opf = z.read(_opf_name(z)).decode("utf-8", "replace")
    items = re.findall(r"<(?:\w+:)?item\b[^>]*>", opf)
    for want in ("nav", "ncx"):
        for it in items:
            href = re.search(r'href\s*=\s*["\']([^"\']*)', it)
            if not href:
                continue
            if want == "nav" and re.search(r'properties\s*=\s*["\'][^"\']*\bnav\b', it):
                return posixpath.dirname(norm(base, href.group(1)))
            if want == "ncx" and "application/x-dtbncx+xml" in it:
                return posixpath.dirname(norm(base, href.group(1)))
    return base


def _opf_name(z):
    c = z.read("META-INF/container.xml").decode("utf-8", "replace")
    i = c.find("full-path=")
    q = c[i + 10]
    return c[i + 11:c.index(q, i + 11)]


def norm(base, href):
    href, _, frag = href.partition("#")
    p = posixpath.normpath(posixpath.join(base, urllib.parse.unquote(href))) if href else ""
    p = p.lstrip("/")
    return p + ("#" + urllib.parse.unquote(frag) if frag else "")


def clean(s):
    return " ".join((s or "").split())


def walk_toc(items, base, out):
    for it in items:
        if isinstance(it, tuple):
            sec, children = it
            out.append((clean(sec.title), norm(base, sec.href) if getattr(sec, "href", "") else ""))
            walk_toc(children, base, out)
        elif isinstance(it, list):
            walk_toc(it, base, out)
        else:
            out.append((clean(it.title), norm(base, it.href) if it.href else ""))


def facts(path):
    base = opf_dir(path)
    book = epub.read_epub(path, options={"ignore_ncx": False})
    out = []
    titles = book.get_metadata("DC", "title")
    if titles:
        out.append(("title", clean(titles[0][0])))
    for v, _ in book.get_metadata("DC", "creator"):
        out.append(("creator", clean(v)))
    for v, _ in book.get_metadata("DC", "language"):
        out.append(("language", clean(v)))
    uid = None
    for v, a in book.get_metadata("DC", "identifier"):
        if a.get("id") == book.IDENTIFIER_ID:
            uid = clean(v)
    if uid:
        out.append(("identifier", uid))
    for idref, linear in book.spine:
        item = book.get_item_with_id(idref)
        if item is not None:
            lin = "no" if str(linear).lower() in ("no", "false") else "yes"
            out.append(("spine", norm(base, item.get_name()) + "\t" + lin))
    toc = []
    walk_toc(book.toc, toc_dir(path, base), toc)
    for t, h in toc:
        out.append(("toc", t + "\t" + h))
    for item in book.get_items_of_type(ebooklib.ITEM_COVER):
        out.append(("cover", norm(base, item.get_name())))
    return out


def main():
    args = sys.argv[1:]
    out_path = os.path.join(os.path.dirname(os.path.abspath(__file__)), "epub", "oracle.tsv")
    if "-o" in args:
        i = args.index("-o")
        out_path = args[i + 1]
        del args[i:i + 2]
    books = []
    for d in args:
        for n in sorted(os.listdir(d)):
            if n.endswith(".epub"):
                books.append(os.path.abspath(os.path.join(d, n)))
    with open(out_path, "w", encoding="utf-8") as f:
        for b in books:
            try:
                rows = facts(b)
            except Exception as e:  # noqa: BLE001
                rows = [("error", type(e).__name__)]
            for k, v in rows:
                f.write(f"{b}\t{k}\t{v}\n")
    print(f"{len(books)} books -> {out_path}")


if __name__ == "__main__":
    main()
