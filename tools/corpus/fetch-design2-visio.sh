#!/usr/bin/env bash
# Fetch the Visio test corpus into tools/corpus/design2/visio/ (gitignored: nothing downloaded
# here is committed). The specular-visio corpus test reads it from SPECULAR_VISIO_CORPUS, else
# from tools/corpus/design2/visio, and skips when it is absent.
#
# Usage: tools/corpus/fetch-design2-visio.sh
#
# What it covers (about 3 MB): the libvisio and LibreOffice import test documents (VSDX, VSTX
# and binary VSD from Visio 5 to 2013: themes, quick styles, bitmaps, text fields, background
# colours, a recursion cycle). Licence: MPL-2.0 test data of the libvisio and LibreOffice
# projects (the documents only; no code). Every file is pinned by its SHA-256.
#
# Downloads are untrusted data: this script only stores them, never runs anything from them.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_VISIO_CORPUS:-$ROOT/tools/corpus/design2/visio}
mkdir -p "$OUT"

LIBVISIO=https://raw.githubusercontent.com/LibreOffice/libvisio/master/src/test/data
LO=https://raw.githubusercontent.com/LibreOffice/core/master/writerperfect/qa/unit/data/draw/libvisio/pass

die() { echo "fetch-design2-visio: $*" >&2; exit 1; }

# get BASE NAME SHA256
get() {
  local base=$1 name=$2 sum=$3 f="$OUT/$2"
  if [[ -f "$f" ]] && [[ "$sum" == "-" || "$(sha256sum "$f" | cut -d' ' -f1)" == "$sum" ]]; then
    return
  fi
  curl -fsSL --retry 3 -m 120 -o "$f.part" "$base/$name" || die "download failed: $name"
  if [[ "$sum" != "-" ]]; then
    [[ "$(sha256sum "$f.part" | cut -d' ' -f1)" == "$sum" ]] || { rm -f "$f.part"; die "checksum mismatch: $name"; }
  fi
  mv "$f.part" "$f"
}

while read -r src name sum; do
  [[ -z "$src" || "$src" == \#* ]] && continue
  case $src in
    libvisio) get "$LIBVISIO" "$name" "$sum" ;;
    lo) get "$LO" "$name" "$sum" ;;
  esac
done <<'LIST'
libvisio bgcolor.vsdx 7b89f56666ec41b4e1122d3ffcaa11ec1c102ec42c5a11b1a95617c4626c8ae0
libvisio blue-box.vsdx 6c67a8b3ccd2b81b3d9d41b74aa0ef5f31b506a714b761b4b4fb4c76ebeb7a94
libvisio color-boxes.vsdx 75072b91b5c3023eeea25d2480776e5a1ee8c336ca263adf3a6440e25f35f800
libvisio dwg.vsdx e05d55753bdc206dac427ec50e95fcc5672b2067e6ce9384682dfc508af5e189
libvisio fdo86664.vsdx fc628bbb1c4686489a6b5bf7c2f9987db870ac2bb12dd8a6c9c27ed501a338b5
libvisio office_varient4.vsdx bdb1d90a52d1d9cc67a71369d15f28fb20647a47e76e09977ed765faada383bf
libvisio qs-box.vsdx e8c253142d30f434229eb810eeecb48cd69886faf5e7c7555d91bb5f6d6c4ee8
libvisio recursion-cycle.vsdx 4a2ff4bc60c0b8683aae227d6eea72ed5f7eab2f434fa03518ed6467ad5e70c1
libvisio tab-short-prefix.vsdx 7ae4b2084f96758084c607af2b303e3580322b822aeabb2743406f14a3edb501
libvisio tdf136564-WhiteTextBackground.vsdx 6a4b29df7c637c8ae2a5ed43517c75aafa3d63167a5226e8a43261172b9de283
libvisio tdf154379-QuickStyleFillMatrix.vsdx 4a2c8a08567f9c6e59d4784affea1ecf024d6890dd8c2c9020e0c71d64d37783
libvisio testfile1.vsdx 2ebcf3e9ce4a599c749e5dc7d31030ef4d7d8b69dfbd2565b59b854133cdeec8
libvisio testfile3.vsdx 9ea7dd2a88ecc4e7f29ff5b022712db18baa5b607e44b32d73f2b79bd4029387
libvisio testfile4.vsdx cba7f2981207bcf553a13cc145ca07a5ff6cdeb8a553bb339e6a0757ef55230d
libvisio testfile5.vsdx f2e6ea832811a4e181f252baf84ac99402ff093ef10f48521b8012315191ee1a
libvisio testfile6.vsdx 102269177b3ca1d287959c5a857279bc3e84fae628f1cc8e023193719790b5b3
libvisio bitmaps.vsd 3cf4cf41f9639d4229415cb10b797f64acc496463e8df2b7d5ee75b49b512ce3
libvisio Visio11FormatLine.vsd 55ca3aae6fe1ac96150cea0783aeac8059eee2575c0c46760fd9f7828fa683bd
libvisio no-bgcolor.vsd e114f8252a297964a63b2390b76447ac81de5fc3190cf9b54f9caa86cdb531aa
libvisio Visio5PlanWithDimensions.vsd 7ca19851d2aab6b30b1e6a11f1d95f4e05c00d1557e4594d63a9a9cf22126b02
libvisio Visio11PlanWithDimensions.vsd 8fbd687000f3b7d3d3d409ff2e1a4a4ab95f5c38f5c6ca734ac3799a2b504a87
libvisio dwg.vsd 4d2c343313352067a6a709360fe146d6a7a1d22e8c42873de45011d0ee51a3c1
lo Visio2013drawing.vsdx d5930630e0f7a321a95a3fc4a8c2467218914f70f5a2dba0159438296892a396
lo Visio2013template.vstx 709ec988c94dff1b9e1cc405134414b37987f226a4ca1284e300db8f94f036e3
lo EDB-22679-1.vsd 96f5adcf4178ce16fae5d2aa1951726b1cfdb4d250efc7da82a71eb084c5c618
lo fdo57117-1.vsd 0be3bdd8b576ff28c280ac744a9538cee96cbc4b984cc796e8e12a155c9c6b77
LIST

# A generated drawing exercising the geometry rows, transforms, fills, lines and text the
# public files do not (written by this script; no download).
python3 -I - "$OUT/synthetic-features.vsdx" <<'PY'
import sys, zipfile
NS = ("xmlns='http://schemas.microsoft.com/office/visio/2012/main' "
      "xmlns:r='http://schemas.openxmlformats.org/officeDocument/2006/relationships'")
def cells(**kw):
    return ''.join(f"<Cell N='{k}' V='{v}'/>" for k, v in kw.items())
def row(t, ix, **kw):
    return f"<Row T='{t}' IX='{ix}'>{cells(**kw)}</Row>"
def geom(rows, ix=0, **flags):
    return f"<Section N='Geometry' IX='{ix}'>{cells(**flags)}{''.join(rows)}</Section>"
def rect(w, h):
    return geom([row('MoveTo', 1, X=0, Y=0), row('LineTo', 2, X=w, Y=0), row('LineTo', 3, X=w, Y=h),
                 row('LineTo', 4, X=0, Y=h), row('LineTo', 5, X=0, Y=0)])
def text(s, **kw):
    ch = f"<Section N='Character'><Row IX='0'>{cells(**kw)}</Row></Section>" if kw else ''
    return f"{ch}<Text>{s}</Text>"
shapes = []
sid = [0]
def shape(body, **kw):
    # Visio writes the local pin; LibreOffice needs it.
    kw.setdefault('LocPinX', kw.get('Width', 0) / 2)
    kw.setdefault('LocPinY', kw.get('Height', 0) / 2)
    sid[0] += 1
    shapes.append(f"<Shape ID='{sid[0]}' Type='Shape' LineStyle='0' FillStyle='0' TextStyle='0'>{cells(**kw)}{body}</Shape>")
# Row 1: rectangle, rotated rectangle, flipped triangle, ellipse.
shape(rect(1.5, 1) + text('Plain box'), PinX=1.25, PinY=9.5, Width=1.5, Height=1, FillForegnd='#ffd966')
shape(rect(1.5, 1) + text('Rotated 30', Color='#c00000', Style=1), PinX=3.5, PinY=9.5, Width=1.5, Height=1,
      Angle=0.5235987755982988, FillForegnd='#9dc3e6')
shape(geom([row('MoveTo', 1, X=0, Y=0), row('LineTo', 2, X=1.5, Y=0), row('LineTo', 3, X=0, Y=1),
            row('LineTo', 4, X=0, Y=0)]), PinX=5.75, PinY=9.5, Width=1.5, Height=1, FlipX=1,
      FillForegnd='#a9d18e')
shape(geom([row('Ellipse', 1, X=0.75, Y=0.5, A=1.5, B=0.5, C=0.75, D=1)]) + text('Ellipse', Size=0.2),
      PinX=7.6, PinY=9.5, Width=1.5, Height=1, FillForegnd='#f4b183')
# Row 2: ArcTo (rounded box), EllipticalArcTo, relative rows with a cubic, polyline.
shape(geom([row('MoveTo', 1, X=0.25, Y=0), row('LineTo', 2, X=1.25, Y=0), row('ArcTo', 3, X=1.5, Y=0.25, A=-0.0732),
            row('LineTo', 4, X=1.5, Y=0.75), row('ArcTo', 5, X=1.25, Y=1, A=-0.0732), row('LineTo', 6, X=0.25, Y=1),
            row('ArcTo', 7, X=0, Y=0.75, A=-0.0732), row('LineTo', 8, X=0, Y=0.25), row('ArcTo', 9, X=0.25, Y=0, A=-0.0732)])
      + text('Rounded', Size=0.15), PinX=1.25, PinY=7.5, Width=1.5, Height=1, FillForegnd='#d9d2e9', LineWeight=0.03)
shape(geom([row('MoveTo', 1, X=0, Y=0.5), row('EllipticalArcTo', 2, X=1.5, Y=0.5, A=0.75, B=1, C=0, D=1.5),
            row('LineTo', 3, X=0, Y=0.5)]), PinX=3.5, PinY=7.5, Width=1.5, Height=1, FillForegnd='#c5e0b4')
shape(geom([row('RelMoveTo', 1, X=0, Y=0), row('RelCubBezTo', 2, X=1, Y=0, A=0.2, B=1, C=0.8, D=-0.6),
            row('RelLineTo', 3, X=1, Y=1), row('RelLineTo', 4, X=0, Y=1), row('RelLineTo', 5, X=0, Y=0)]),
      PinX=5.75, PinY=7.5, Width=1.5, Height=1, FillForegnd='#bdd7ee')
shape(geom([row('MoveTo', 1, X=0, Y=0), row('PolylineTo', 2, X=1.5, Y=0, A='POLYLINE(1, 1, 0.3, 1, 0.75, 0.2, 1.2, 1)')],
           NoFill=1), PinX=7.6, PinY=7.5, Width=1.5, Height=1, LineWeight=0.02, LineColor='#2f5597')
# Row 3: dashes, arrows, gradient, transparency over a box.
for i, pat in enumerate([2, 3, 4]):
    shape(geom([row('MoveTo', 1, X=0, Y=0), row('LineTo', 2, X=2, Y=0)], NoFill=1), PinX=1.5, PinY=5.9 - i * 0.3,
          Width=2, Height=0, LinePattern=pat, LineWeight=0.02, LineColor='#7030a0')
shape(geom([row('MoveTo', 1, X=0, Y=0), row('LineTo', 2, X=2, Y=0)], NoFill=1), PinX=4.0, PinY=5.6, Width=2,
      Height=0, EndArrow=5, BeginArrow=5, EndArrowSize=3, LineWeight=0.015, LineColor='#000000')
shape(rect(1.5, 1), PinX=7.4, PinY=5.6, Width=1.5, Height=1, FillForegnd='#ff0000', FillBkgnd='#0000ff', FillPattern=25)
shape(rect(1.2, 0.8), PinX=7.0, PinY=5.3, Width=1.2, Height=0.8, FillForegnd='#00b050', FillForegndTrans=0.5)
# Row 4: paragraph alignment and wrapping.
para = ("<Section N='Paragraph'><Row IX='0'>" + cells(HorzAlign=0) + "</Row><Row IX='1'>" + cells(HorzAlign=2) +
        "</Row></Section>")
shape(rect(3, 1.5) + para + "<Section N='Character'><Row IX='0'>" + cells(Size=0.1667) + "</Row><Row IX='1'>" +
      cells(Size=0.1667, Style=2, Color='#0070c0') + "</Row></Section>" +
      "<Text><pp IX='0'/><cp IX='0'/>Left aligned paragraph that wraps over two lines\n<pp IX='1'/><cp IX='1'/>Right italic\n</Text>",
      PinX=2, PinY=3.2, Width=3, Height=1.5, FillForegnd='#ffffff', VerticalAlign=0)
shape(rect(3, 1.5) + "<Section N='Character'><Row IX='0'>" + cells(Size=0.25, Style=5) + "</Row></Section><Text>Bottom</Text>",
      PinX=6, PinY=3.2, Width=3, Height=1.5, FillForegnd='#fff2cc', VerticalAlign=2)
# A group with a nested, rotated member.
grp = (f"<Shape ID='100' Type='Group' LineStyle='0' FillStyle='0' TextStyle='0'>{cells(PinX=4.25, PinY=1.2, Width=3, Height=1, LocPinX=1.5, LocPinY=0.5, Angle=-0.2)}"
       f"<Shapes><Shape ID='101' Type='Shape' LineStyle='0' FillStyle='0' TextStyle='0'>{cells(PinX=0.75, PinY=0.5, Width=1.5, Height=1, LocPinX=0.75, LocPinY=0.5, FillForegnd='#ff9999')}{rect(1.5, 1)}</Shape>"
       f"<Shape ID='102' Type='Shape' LineStyle='0' FillStyle='0' TextStyle='0'>{cells(PinX=2.25, PinY=0.5, Width=1.5, Height=1, LocPinX=0.75, LocPinY=0.5, FillForegnd='#99ccff')}{rect(1.5, 1)}{text('In group')}</Shape></Shapes></Shape>")
shapes.append(grp)
doc = (f"<?xml version='1.0' encoding='utf-8'?><VisioDocument {NS}><FaceNames><FaceName NameU='Arial'/></FaceNames>"
       "<StyleSheets><StyleSheet ID='0' NameU='No Style'>" + cells(LineWeight=0.01, LineColor=0, LinePattern=1, FillForegnd=1, FillBkgnd=0,
       FillPattern=1, LineCap=0, BeginArrow=0, EndArrow=0, BeginArrowSize=2, EndArrowSize=2, LeftMargin=0.0556, RightMargin=0.0556,
       TopMargin=0.0556, BottomMargin=0.0556, VerticalAlign=1) +
       "<Section N='Character'><Row IX='0'>" + cells(Font='Arial', Size=0.1389, Color=0, Style=0) + "</Row></Section>"
       "<Section N='Paragraph'><Row IX='0'>" + cells(HorzAlign=1, SpLine=-1.2) + "</Row></Section></StyleSheet></StyleSheets></VisioDocument>")
pages = (f"<?xml version='1.0' encoding='utf-8'?><Pages {NS}><Page ID='0' NameU='Features' Name='Features'><PageSheet>"
         + cells(PageWidth=8.5, PageHeight=11) + "</PageSheet><Rel r:id='rId1'/></Page></Pages>")
page = f"<?xml version='1.0' encoding='utf-8'?><PageContents {NS}><Shapes>{''.join(shapes)}</Shapes></PageContents>"
R = "http://schemas.microsoft.com/visio/2010/relationships/"
def rels(*items):
    return ("<?xml version='1.0' encoding='utf-8'?><Relationships xmlns='http://schemas.openxmlformats.org/package/2006/relationships'>"
            + ''.join(f"<Relationship Id='{i}' Type='{t}' Target='{g}'/>" for i, t, g in items) + "</Relationships>")
ct = ("<?xml version='1.0' encoding='utf-8'?><Types xmlns='http://schemas.openxmlformats.org/package/2006/content-types'>"
      "<Default Extension='rels' ContentType='application/vnd.openxmlformats-package.relationships+xml'/>"
      "<Default Extension='xml' ContentType='application/xml'/>"
      "<Override PartName='/visio/document.xml' ContentType='application/vnd.ms-visio.drawing.main+xml'/>"
      "<Override PartName='/visio/pages/pages.xml' ContentType='application/vnd.ms-visio.pages+xml'/>"
      "<Override PartName='/visio/pages/page1.xml' ContentType='application/vnd.ms-visio.page+xml'/></Types>")
with zipfile.ZipFile(sys.argv[1], 'w', zipfile.ZIP_DEFLATED) as z:
    for name, data in [
        ('[Content_Types].xml', ct),
        ('_rels/.rels', rels(('rId1', R + 'document', 'visio/document.xml'))),
        ('visio/document.xml', doc),
        ('visio/_rels/document.xml.rels', rels(('rId1', R + 'pages', 'pages/pages.xml'))),
        ('visio/pages/pages.xml', pages),
        ('visio/pages/_rels/pages.xml.rels', rels(('rId1', R + 'page', 'page1.xml'))),
        ('visio/pages/page1.xml', page),
    ]:
        zi = zipfile.ZipInfo(name, (2026, 1, 1, 0, 0, 0))
        zi.compress_type = zipfile.ZIP_DEFLATED
        z.writestr(zi, data)
PY

echo "fetch-design2-visio: $(find "$OUT" -maxdepth 1 -type f | wc -l) files, $(du -sh "$OUT" | cut -f1)"
