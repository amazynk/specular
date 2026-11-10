#!/usr/bin/env bash
# Fetch and generate the OpenDocument corpus into tools/corpus/design2/odf/ (gitignored:
# nothing here is committed). The specular-odf corpus test reads it from
# SPECULAR_ODF_CORPUS, else tools/corpus/design2/odf, and skips when it is absent.
#
# Usage: tools/corpus/fetch-design2-odf.sh [--oracle DPI]
#
# What it holds (about 2 MB):
# - lo/: LibreOffice's own unit-test documents (sd, svx: drawings, presentations,
#   custom-shape command tests, flips, shears, gradients, backgrounds, bitmaps), MPL-2.0,
#   pinned to one core commit.
# - apps/: real diagrams from free software documentation: SDRangel
#   (GPL-3.0).
# - gen/: drawings generated here (flat XML written by this script, converted to .odg by
#   LibreOffice), and an .odt/.ods/.odp made from them for the thumbnail path. Generated
#   files differ per run (timestamps), so their inputs are what is pinned.
# Downloads are pinned by sha256 and are untrusted data: nothing in them is run.
#
# --oracle DPI: also writes LibreOffice's renders (PDF → PPM with pdftoppm) of every
# drawing and presentation to oracle/ref/, ours (the odf-render example) to oracle/ours/,
# and prints tools/corpus/design2-odf-oracle.py's comparison. LibreOffice runs with a
# private profile under target/, never the user's.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_ODF_CORPUS:-$ROOT/tools/corpus/design2/odf}
PROFILE="$ROOT/target/agent-corpus/design2-odf-lo"
mkdir -p "$OUT/lo" "$OUT/apps" "$OUT/gen" "$PROFILE"

die() { echo "fetch-design2-odf: $*" >&2; exit 1; }

LO=https://raw.githubusercontent.com/LibreOffice/core/c4fb2255077e9891cee5b2c77bcd61490ef522cf
SDR=https://raw.githubusercontent.com/f4exb/sdrangel/234575273a5b78a26db6e2aaaece38c2502417d0

# get DIR URL SHA256
get() {
  local dir=$1 url=$2 sum=$3 name
  name=$(basename "$url")
  local f="$OUT/$dir/$name"
  if [[ ! -f "$f" ]]; then
    curl -fsSL --retry 3 -o "$f.part" "$url" || die "download failed: $url"
    mv "$f.part" "$f"
  fi
  local got
  got=$(sha256sum "$f" | cut -d' ' -f1)
  if [[ "$sum" != "-" && "$got" != "$sum" ]]; then
    rm -f "$f"
    die "sha256 mismatch for $name: $got"
  fi
  [[ "$sum" == "-" ]] && echo "unpinned $dir/$name $got"
  return 0
}

while read -r path sum; do
  [[ -z "$path" ]] && continue
  get lo "$LO/$path" "$sum"
done <<'EOF'
sd/qa/unit/data/objectwithtext.fodg 45118e7425da1fc9f6e4dc9f7c498d53e8e3853fba3db7276e7805689fcace1a
sd/qa/unit/data/tdf112126.odg d30e780efab6af6ad430feae0d5864922a40ba460008e249352ab310702f80fe
sd/qa/unit/data/tdf119956.odg 8b483b9b59cec19310c48ad8fbc1a1984f497dcce597cc463e48ca9a4f27083d
sd/qa/unit/data/tdf123841.odg 26b7782a71b3fd52bcde3403cedacdd9a8235ed22b9488a3ff3dd0a060e00c4a
sd/qa/unit/data/tdf154989.odg 296a0480f89dbd775a8616bb1b97221d4c5e55e5c12598e830a4a0f0d1315e0f
sd/qa/unit/data/tdf159666.odg c85c43f9749cb8b2bcbc6aa48d36b93746b5dd509183036b1b64dccde894e569
sd/qa/unit/data/tdf67248.odg 8720d818066de8195a7dcaa377400f0dda1b37459043318caab96ee544175647
sd/qa/unit/data/tdf98839_ShearVFlipH.odg 0240ed1d1912690972c6dd25f2b6fb78d6c537c2e15d34477cc9be9e45b5bf85
sd/qa/unit/data/transparent-text.fodg 7547f2834cd6d46a86f2f05584429c60a07a5634dd53bbef2217efd16c164956
sd/qa/unit/data/tdf101242_ODF.odg 8eaec7b4721eafac1def62dc2932bb1aa43fe4da033481de1570948ec99f7d5c
sd/qa/unit/data/slide_with_text.odp 4042d79e4d59750c2590b01b817fb826ab593407496cb0c3fdcc889acb06e96f
sd/qa/unit/data/fdo71961.odp 38680d2b9c122f2f8ec7302b4669a8db29fa523548b3c90b0ecb7a3363bac010
sd/qa/unit/data/odp/closed-shapes.odp 9256ad84ff1b41ffceef8aefdd79ed6dbbebbfd0ff18852099d93ca43eff2dcb
sd/qa/unit/data/odp/shapes-test.odp 643b9011d19caa28e03ce127fc12039bd77bc034770770922d52225b0f45ec70
sd/qa/unit/data/odp/preset-shapes-export.odp 5e5ffe96c78bb809d3363a9fa926d3e06093f11f6dff95e98ade4bb150936e18
sd/qa/unit/data/odp/rotate_flip.odp 1dbd57ea2eab79cc8c75e538ec0b1b2687ebb771bddd6f50125955d85fcb681d
sd/qa/unit/data/odp/group_rotation.odp 15f00cff8c5326f3b58932bb38217bcf350a3d4c1f45e09098b1a8d16dc92eb7
sd/qa/unit/data/odp/background.odp 545ce0108d8fe01de86db9ce9becf5f5d940a58e87e19e63555d545e890dfcbc
sd/qa/unit/data/odp/slide-tile-background.odp d28d89819f7499f33fe67dffbcd8a5e71bba29b6d6eddf7c7ffed7f5295bb0fc
sd/qa/unit/data/odp/fillbitmap2.odp f7f98cc5b2bcbbdc0fac2f47d62923fbbed7299e025b06ca43dfd36860631ce1
sd/qa/unit/data/odp/LineStylesOwn.odp d7285e2e1f05f3fbfb3ea5a31363ce547efd99071fed03d5b967b5a19e66d17f
sd/qa/unit/data/odp/image_transparency.odp e414ee8c52f0176277500ac9b3598ffc12913c58ac4948e0347859e77b1314b6
sd/qa/unit/data/odp/4-3.odp 391cdd3f1ce25bd9149e06a50a250553cc20c93190177d9b212d2663925d463d
sd/qa/unit/data/odg/gradient-angle.fodg 3f8f9ef6cd6e7926824b2a549a674948b5f7b38f79f80c7a9bd8578b81d090ca
sd/qa/unit/data/odg/diagonalLine.fodg bc50bf7114f52c03971d08b270dc018d6fc696057593cd8b19b5a0f2e436cbde
sd/qa/unit/data/odg/FitToFrameText.odg 45e8f565c9dca5f7c8cd2b18256a15b0c8276762770dd1406d6e89db526235cd
sd/qa/unit/data/odg/two_columns.odg 8bad155ceb328dcf1388767678dfb1dd0dfdfa163eb06c17e41c2b1f6c692098
sd/qa/unit/data/odg/tdf126319.odg 6c96f0b2ff8be6ca6d68912538312426edb2899fd7da5911cfce66f365e8a297
sd/qa/unit/data/odg/tdf147119.odg 9ed19bea69a0f4854cb080d8abd8cf8db9523549b5a08bc525e641f482a909ff
svx/qa/unit/data/tdf121845_Two_commands_U.odg 40015d194de376e53067122c3a987f1bacc37d26df9f944be9c91e31a226785e
svx/qa/unit/data/tdf122964_MultipleMoveTo.odg 96de6b813d0220d5c6d4ae715f2eb103b1c402b29dc660202de2a158bcb659e1
svx/qa/unit/data/tdf125782_QuadraticCurveTo.odg 40ad95cb92b9d0bcb947e3f820814ad5705ce499427f68c137f6814a4bb6afd5
svx/qa/unit/data/tdf129532_MatrixFlipV.odg 4d322b6c54cc166d7cd787c38bf814c38f93aa73083465124b2d945cb7439f94
svx/qa/unit/data/tdf130076_FlipOnSectorSection.odg 9c29670d9478ffcbef2acb881b60829f8327dd67ca6b3d8f2cd124779f10fbf7
svx/qa/unit/data/tdf136176_rot30_flip.odg d25eba1d13c4f453de40da02c50bbce22b0b4a8371d764bb49db70df5157ecb3
svx/qa/unit/data/tdf139968_shear45_rot30.odg 74358c77b6be612e62a140f403b05d35f976600f9dd619a7111d86aca91ebb96
svx/qa/unit/data/tdf98584_ShearVertical.odg 6ac02e014ce1771a20915b4b57814ea2baa556ed0b7de55bc0c4b4741035096e
svx/qa/unit/data/FontWork.odg d0deab7641ecdb780e42ab7be27da390c900bb689de689b7798d723ce7db3bd0
svx/qa/unit/data/tdf148707_two_commands_B_V.odp 52153bd2878ebad48102501589f368491c87b8a51ec3504aa580c029c2f91bca
svx/qa/unit/data/tdf121952_Toggle_direction_command_X.odp e94d68c81a001385bc2847fc4595e1adb610b2887564cc0c1bc35b73b750eeb1
svx/qa/unit/data/chart.ods b621896905418b1134d86fcb535fffcea93f2c39e4ffd0fa32fef19394444a81
svx/qa/unit/data/unodraw-writer-image.odt 4fc51249211d0140512e785adbecd2b563e52addc155ce81edb06d0d4471889a
EOF
get apps "$SDR/doc/img/mimo_steering_angle.odg" 265b096c71629302136e5e6bbab3714e068e79a77efacef6a0d75a195b1cff67
get apps "$SDR/doc/model/SDRangelFlow.odg" 09de41cf580f4c766042db06b200558b3cbe17ac51b5499d1c46dc8344c2dd6d

# Generated drawings: flat XML written by the generator below, converted by LibreOffice.
python3 -I - "$OUT/gen" <<'PYEOF'
import sys
out = sys.argv[1]
NS = ('xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" '
      'xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" '
      'xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" '
      'xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0" '
      'xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0" '
      'xmlns:svg="urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0" '
      'xmlns:xlink="http://www.w3.org/1999/xlink"')
STYLES = '''<office:font-face-decls>
 <style:font-face style:name="Liberation Sans" svg:font-family="'Liberation Sans'"/>
 <style:font-face style:name="Liberation Serif" svg:font-family="'Liberation Serif'"/>
</office:font-face-decls>
<office:styles>
 <draw:gradient draw:name="g1" draw:style="linear" draw:start-color="#ff0000" draw:end-color="#0000ff" draw:angle="90deg" draw:border="0%"/>
 <draw:gradient draw:name="g2" draw:style="radial" draw:cx="50%" draw:cy="50%" draw:start-color="#ffffff" draw:end-color="#008000" draw:border="0%"/>
 <draw:gradient draw:name="g3" draw:style="axial" draw:start-color="#ffff00" draw:end-color="#800080" draw:angle="0deg" draw:border="0%"/>
 <draw:gradient draw:name="g4" draw:style="ellipsoid" draw:cx="30%" draw:cy="30%" draw:start-color="#000080" draw:end-color="#ffffff" draw:angle="30deg" draw:border="10%"/>
 <draw:hatch draw:name="h1" draw:style="double" draw:color="#cc0000" draw:distance="0.2cm" draw:rotation="450"/>
 <draw:stroke-dash draw:name="d1" draw:style="rect" draw:dots1="1" draw:dots1-length="0.3cm" draw:distance="0.2cm"/>
 <draw:marker draw:name="Arrow" svg:viewBox="0 0 20 30" svg:d="M10 0l-10 30h20z"/>
 <style:default-style style:family="graphic">
  <style:graphic-properties draw:fill="solid" draw:fill-color="#729fcf" svg:stroke-color="#3465a4" svg:stroke-width="0cm" draw:stroke="solid"/>
  <style:paragraph-properties fo:text-align="center"/>
  <style:text-properties style:font-name="Liberation Sans" fo:font-size="18pt"/>
 </style:default-style>
 <style:style style:name="standard" style:family="graphic"><style:graphic-properties draw:textarea-vertical-align="middle"/></style:style>
</office:styles>
<office:automatic-styles>
 <style:page-layout style:name="PM0"><style:page-layout-properties fo:page-width="21cm" fo:page-height="29.7cm"/></style:page-layout>
 <style:style style:name="dp1" style:family="drawing-page"/>
 <style:style style:name="gr1" style:family="graphic" style:parent-style-name="standard"><style:graphic-properties draw:fill="solid" draw:fill-color="#ffcc00" svg:stroke-color="#000000" svg:stroke-width="0.1cm"/></style:style>
 <style:style style:name="gr2" style:family="graphic" style:parent-style-name="standard"><style:graphic-properties draw:fill="gradient" draw:fill-gradient-name="g1" draw:stroke="none"/></style:style>
 <style:style style:name="gr3" style:family="graphic" style:parent-style-name="standard"><style:graphic-properties draw:fill="gradient" draw:fill-gradient-name="g2" draw:stroke="dash" draw:stroke-dash="d1" svg:stroke-width="0.05cm" svg:stroke-color="#ff0000"/></style:style>
 <style:style style:name="gr4" style:family="graphic" style:parent-style-name="standard"><style:graphic-properties draw:fill="none" svg:stroke-color="#000080" svg:stroke-width="0.08cm" draw:marker-end="Arrow" draw:marker-end-width="0.4cm"/></style:style>
 <style:style style:name="gr5" style:family="graphic" style:parent-style-name="standard"><style:graphic-properties draw:fill="gradient" draw:fill-gradient-name="g3" draw:opacity="60%" draw:shadow="visible" draw:shadow-offset-x="0.3cm" draw:shadow-offset-y="0.3cm" draw:shadow-color="#808080"/></style:style>
 <style:style style:name="gr6" style:family="graphic" style:parent-style-name="standard"><style:graphic-properties draw:fill="none" draw:stroke="none" draw:textarea-vertical-align="top" fo:padding-top="0.1cm" fo:padding-left="0.1cm"/></style:style>
 <style:style style:name="gr7" style:family="graphic" style:parent-style-name="standard"><style:graphic-properties draw:fill="hatch" draw:fill-hatch-name="h1" draw:fill-hatch-solid="true" draw:fill-color="#ffffcc"/></style:style>
 <style:style style:name="gr8" style:family="graphic" style:parent-style-name="standard"><style:graphic-properties draw:fill="gradient" draw:fill-gradient-name="g4" svg:stroke-width="0.2cm" draw:stroke-linejoin="round" svg:stroke-linecap="round" svg:stroke-color="#008080"/></style:style>
 <style:style style:name="P1" style:family="paragraph"><style:paragraph-properties fo:text-align="start"/></style:style>
 <style:style style:name="P2" style:family="paragraph"><style:paragraph-properties fo:text-align="end" fo:line-height="150%"/></style:style>
 <style:style style:name="T1" style:family="text"><style:text-properties fo:font-weight="bold" fo:color="#c9211e" style:font-name="Liberation Serif" fo:font-size="24pt"/></style:style>
 <style:style style:name="T2" style:family="text"><style:text-properties fo:font-style="italic" style:text-underline-style="solid" fo:font-size="14pt"/></style:style>
</office:automatic-styles>
<office:master-styles><style:master-page style:name="Default" style:page-layout-name="PM0" draw:style-name="dp1"/></office:master-styles>'''

def doc(pages):
    return ('<?xml version="1.0" encoding="UTF-8"?>\n<office:document ' + NS +
            ' office:version="1.3" office:mimetype="application/vnd.oasis.opendocument.graphics">' +
            STYLES + '<office:body><office:drawing>' + pages +
            '</office:drawing></office:body></office:document>')

def page(name, shapes):
    return f'<draw:page draw:name="{name}" draw:style-name="dp1" draw:master-page-name="Default">{shapes}</draw:page>'

BASIC = '''
<draw:rect draw:style-name="gr1" svg:x="1.5cm" svg:y="1.5cm" svg:width="5cm" svg:height="3cm"><text:p>Rect</text:p></draw:rect>
<draw:rect draw:style-name="gr2" draw:corner-radius="0.5cm" svg:x="8cm" svg:y="1.5cm" svg:width="5cm" svg:height="3cm"/>
<draw:ellipse draw:style-name="gr3" svg:x="14.5cm" svg:y="1.5cm" svg:width="5cm" svg:height="3cm"/>
<draw:ellipse draw:style-name="gr1" draw:kind="section" draw:start-angle="30" draw:end-angle="300" svg:x="1.5cm" svg:y="6cm" svg:width="4cm" svg:height="4cm"/>
<draw:ellipse draw:style-name="gr4" draw:kind="arc" draw:start-angle="0" draw:end-angle="180" svg:x="7cm" svg:y="6cm" svg:width="4cm" svg:height="4cm"/>
<draw:ellipse draw:style-name="gr1" draw:kind="cut" draw:start-angle="200" draw:end-angle="20" svg:x="13cm" svg:y="6cm" svg:width="2.5cm" svg:height="4cm"/>
<draw:line draw:style-name="gr4" svg:x1="16cm" svg:y1="6cm" svg:x2="19cm" svg:y2="10cm"/>
<draw:polygon draw:style-name="gr7" svg:x="1.5cm" svg:y="11.5cm" svg:width="5cm" svg:height="4cm" svg:viewBox="0 0 5000 4000" draw:points="0,4000 2500,0 5000,4000 2500,2500"/>
<draw:polyline draw:style-name="gr4" svg:x="8cm" svg:y="11.5cm" svg:width="5cm" svg:height="4cm" svg:viewBox="0 0 5000 4000" draw:points="0,0 1000,4000 2000,0 3000,4000 4000,0 5000,4000"/>
<draw:path draw:style-name="gr5" svg:x="14.5cm" svg:y="11.5cm" svg:width="5cm" svg:height="4cm" svg:viewBox="0 0 5000 4000" svg:d="M0 2000C0 0 2500 0 2500 2000S5000 4000 5000 2000L5000 4000 0 4000z"/>
<draw:regular-polygon draw:style-name="gr8" draw:corners="6" draw:concave="false" svg:x="1.5cm" svg:y="17cm" svg:width="4cm" svg:height="4cm"/>
<draw:regular-polygon draw:style-name="gr2" draw:corners="5" draw:concave="true" draw:sharpness="50%" svg:x="7cm" svg:y="17cm" svg:width="4cm" svg:height="4cm"/>
<draw:rect draw:style-name="gr1" draw:transform="rotate (0.5236) translate (13cm 20cm)" svg:width="5cm" svg:height="2cm"><text:p>Rotated</text:p></draw:rect>
<draw:frame draw:style-name="gr6" svg:x="1.5cm" svg:y="22.5cm" svg:width="17cm" svg:height="5cm"><draw:text-box>
 <text:p text:style-name="P1"><text:span text:style-name="T1">Bold red serif</text:span> and plain text that wraps across the box width to test word wrapping in the frame.</text:p>
 <text:p text:style-name="P2"><text:span text:style-name="T2">Italic underlined</text:span><text:line-break/>right aligned<text:s text:c="3"/>line</text:p>
 <text:list><text:list-item><text:p text:style-name="P1">First bullet</text:p></text:list-item><text:list-item><text:p text:style-name="P1">Second bullet</text:p></text:list-item></text:list>
</draw:text-box></draw:frame>'''

CUSTOM = '''
<draw:custom-shape draw:style-name="gr1" svg:x="2cm" svg:y="2cm" svg:width="6cm" svg:height="4cm"><text:p>Hex</text:p>
 <draw:enhanced-geometry draw:type="hexagon" draw:modifiers="5400" svg:viewBox="0 0 21600 21600" draw:enhanced-path="M ?f0 0 L ?f1 0 21600 10800 ?f1 21600 ?f0 21600 0 10800 Z N" draw:text-areas="?f3 ?f3 ?f4 ?f5">
  <draw:equation draw:name="f0" draw:formula="$0 "/><draw:equation draw:name="f1" draw:formula="21600-$0 "/><draw:equation draw:name="f2" draw:formula="$0 *100/234"/><draw:equation draw:name="f3" draw:formula="?f2 +1700"/><draw:equation draw:name="f4" draw:formula="21600-?f3 "/><draw:equation draw:name="f5" draw:formula="21600-?f3 "/>
 </draw:enhanced-geometry></draw:custom-shape>
<draw:custom-shape draw:style-name="gr2" svg:x="10cm" svg:y="2cm" svg:width="6cm" svg:height="6cm">
 <draw:enhanced-geometry svg:viewBox="0 0 21600 21600" draw:type="smiley" draw:modifiers="17520" draw:enhanced-path="U 10800 10800 10800 10800 0 360 Z N U 7305 7515 1165 1165 0 360 Z N U 14295 7515 1165 1165 0 360 Z N M 4870 ?f1 C 8680 ?f2 12920 ?f2 16730 ?f1 F N">
  <draw:equation draw:name="f0" draw:formula="$0 -15510"/><draw:equation draw:name="f1" draw:formula="17520-?f0 "/><draw:equation draw:name="f2" draw:formula="15510+?f0 "/>
 </draw:enhanced-geometry></draw:custom-shape>
<draw:custom-shape draw:style-name="gr5" svg:x="2cm" svg:y="10cm" svg:width="8cm" svg:height="5cm">
 <draw:enhanced-geometry svg:viewBox="0 0 21600 21600" draw:type="mso-spt100" draw:mirror-horizontal="true" draw:enhanced-path="M 0 0 L 21600 10800 0 21600 Z N"/></draw:custom-shape>
<draw:custom-shape draw:style-name="gr1" svg:x="12cm" svg:y="10cm" svg:width="6cm" svg:height="5cm">
 <draw:enhanced-geometry svg:viewBox="0 0 21600 21600" draw:type="can" draw:modifiers="5400" draw:enhanced-path="M 0 ?f2 L 0 ?f6 X ?f5 21600 ?f3 21600 Y 21600 ?f6 L 21600 ?f2 Y ?f3 0 ?f5 0 X 0 ?f2 Z N">
  <draw:equation draw:name="f0" draw:formula="$0 *2/4"/><draw:equation draw:name="f1" draw:formula="?f0 *1/2"/><draw:equation draw:name="f2" draw:formula="?f1"/><draw:equation draw:name="f3" draw:formula="10800"/><draw:equation draw:name="f4" draw:formula="0"/><draw:equation draw:name="f5" draw:formula="10800"/><draw:equation draw:name="f6" draw:formula="21600-?f1 "/>
 </draw:enhanced-geometry></draw:custom-shape>
<draw:custom-shape draw:style-name="gr1" svg:x="1cm" svg:y="17cm" svg:width="5cm" svg:height="5cm"><draw:enhanced-geometry svg:viewBox="0 0 21600 21600" draw:enhanced-path="M 10800 10800 A 0 0 21600 21600 21600 10800 10800 0 Z N"/></draw:custom-shape>
<draw:custom-shape draw:style-name="gr1" svg:x="7cm" svg:y="17cm" svg:width="5cm" svg:height="5cm"><draw:enhanced-geometry svg:viewBox="0 0 21600 21600" draw:enhanced-path="M 10800 10800 W 0 0 21600 21600 21600 10800 10800 0 Z N"/></draw:custom-shape>
<draw:custom-shape draw:style-name="gr8" svg:x="13cm" svg:y="17cm" svg:width="5cm" svg:height="5cm"><draw:enhanced-geometry svg:viewBox="0 0 21600 21600" draw:enhanced-path="M 10800 10800 T 10800 10800 10800 10800 0 90 Z N"/></draw:custom-shape>
<draw:g><draw:rect draw:style-name="gr1" svg:x="2cm" svg:y="23cm" svg:width="3cm" svg:height="3cm"/><draw:ellipse draw:style-name="gr3" svg:x="4cm" svg:y="24cm" svg:width="3cm" svg:height="3cm"/></draw:g>
<draw:connector draw:style-name="gr4" svg:x1="8cm" svg:y1="24cm" svg:x2="16cm" svg:y2="27cm"/>'''

open(f'{out}/shapes.fodg', 'w').write(doc(page('Shapes', BASIC) + page('Custom', CUSTOM)))
# The same pages as a presentation (thumbnail carrier and slide rendering).
pres = (doc(page('Shapes', BASIC) + page('Custom', CUSTOM))
        .replace('opendocument.graphics', 'opendocument.presentation')
        .replace('<office:drawing>', '<office:presentation>')
        .replace('</office:drawing>', '</office:presentation>'))
open(f'{out}/slides.fodp', 'w').write(pres)
PYEOF

LOCMD=(timeout 300 soffice "-env:UserInstallation=file://$PROFILE" --headless)
"${LOCMD[@]}" --convert-to odg --outdir "$OUT/gen" "$OUT/gen/shapes.fodg" > /dev/null 2>&1 \
  || die "LibreOffice conversion failed"
# Thumbnail carriers: a presentation, a text document and a spreadsheet.
"${LOCMD[@]}" --convert-to odp --outdir "$OUT/gen" "$OUT/gen/slides.fodp" > /dev/null 2>&1 || true
printf 'Specular thumbnail test\n\nA paragraph of text for the first page.\n' > "$OUT/gen/text.txt"
"${LOCMD[@]}" --convert-to odt --outdir "$OUT/gen" "$OUT/gen/text.txt" > /dev/null 2>&1 || true
printf 'a,b,c\n1,2,3\n4,5,6\n' > "$OUT/gen/table.csv"
"${LOCMD[@]}" --convert-to ods --outdir "$OUT/gen" "$OUT/gen/table.csv" > /dev/null 2>&1 || true
rm -f "$OUT/gen/text.txt" "$OUT/gen/table.csv"

if [[ "${1:-}" == "--oracle" ]]; then
  DPI=${2:-48}
  ORA="$OUT/oracle"
  rm -rf "$ORA"
  mkdir -p "$ORA/ref" "$ORA/ours" "$ORA/pdf"
  nice cargo build -q -j 2 -p specular-odf --example odf-render
  RENDER="${CARGO_TARGET_DIR:-$ROOT/target}/debug/examples/odf-render"
  find "$OUT" -path "$ORA" -prune -o -type f \( -name '*.odg' -o -name '*.fodg' -o -name '*.odp' \) -print | sort |
  while read -r f; do
    stem=$(basename "${f%.*}")
    "${LOCMD[@]}" --convert-to pdf --outdir "$ORA/pdf" "$f" > /dev/null 2>&1 || continue
    pdftoppm -r "$DPI" "$ORA/pdf/$stem.pdf" "$ORA/ref/$stem" || true
    "$RENDER" "$f" "$ORA/ours" "$DPI" || true
  done
  python3 -I "$ROOT/tools/corpus/design2-odf-oracle.py" "$ORA/ours" "$ORA/ref"
fi
echo "fetch-design2-odf: $(find "$OUT" -type f -not -path '*/oracle/*' | wc -l) files, $(du -sh "$OUT" | cut -f1)"
