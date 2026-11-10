#!/usr/bin/env bash
# Regenerates packaging/THIRD-PARTY.md: data and font licences, then every Rust crate licence
# (cargo-about, run for the shipped app only). Run after any dependency change; CI checks the file
# is current.
#
#   tools/release/third-party.sh [--check]
#
# cargo-about is taken from PATH or target/tools/bin; install it there with
#   cargo install --locked --features cli --root target/tools cargo-about
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
cd "$ROOT"
CHECK=0
[[ "${1:-}" == "--check" ]] && CHECK=1
export TMPDIR="${TMPDIR:-$ROOT/target/tmp}"
mkdir -p "$TMPDIR"
ABOUT=$(command -v cargo-about || echo "$ROOT/target/tools/bin/cargo-about")
[[ -x "$ABOUT" ]] || { echo "cargo-about not found (see header)" >&2; exit 1; }

FONTS=crates/specular-theme/fonts
OUT=packaging/THIRD-PARTY.md
TMP=$(mktemp "$TMPDIR/third-party.XXXXXX")
CRATES=$(mktemp "$TMPDIR/crates.XXXXXX")
APP_JSON=$(mktemp "$TMPDIR/about-app.XXXXXX")
HOST_JSON=$(mktemp "$TMPDIR/about-host.XXXXXX")
trap 'rm -f "$TMP" "$CRATES" "$APP_JSON" "$HOST_JSON"' EXIT

# Both shipped binaries: the viewer and the decode host, whose sandbox and archive crates
# (landlock, seccompiler, unrar, …) the viewer does not link.
for pair in "app/specular:$APP_JSON" "crates/specular-decode-host:$HOST_JSON"; do
    "$ABOUT" generate --locked -m "${pair%%:*}/Cargo.toml" -c packaging/about/about.toml \
        --format json -o "${pair#*:}" 2> >(grep -v -E '^\s*$' >&2)
done

# Merge the two crate lists and render them. Specular's own crates (the workspace's, under
# MIT OR Apache-2.0) are left out: they are covered by Specular's licence, stated at the top.
# Vendored workspace crates under another licence (rav1d, heif, h265) stay listed.
python3 -I - "$APP_JSON" "$HOST_JSON" "$CRATES" <<'EOF'
import json, sys

app, host, out = sys.argv[1:4]
d = json.load(open(app, encoding="utf-8"))
key = lambda u: (u["crate"]["name"], u["crate"]["version"])
have = {key(u) for l in d["licenses"] for u in l["used_by"]}
for l in json.load(open(host, encoding="utf-8"))["licenses"]:
    new = [u for u in l["used_by"] if key(u) not in have]
    if not new:
        continue
    have.update(key(u) for u in new)
    same = [x for x in d["licenses"] if x["id"] == l["id"] and x["text"] == l["text"]]
    if same:
        same[0]["used_by"] = sorted(same[0]["used_by"] + new, key=lambda u: "@".join(key(u)))
    else:
        last = max((i for i, x in enumerate(d["licenses"]) if x["id"] == l["id"]), default=len(d["licenses"]) - 1)
        d["licenses"].insert(last + 1, dict(l, used_by=new))
import subprocess
meta = json.loads(subprocess.run(
    ["cargo", "metadata", "--no-deps", "--format-version", "1", "--offline"],
    check=True, capture_output=True, text=True).stdout)
own = {p["name"] for p in meta["packages"] if p.get("license") == "MIT OR Apache-2.0"}
for l in d["licenses"]:
    l["used_by"] = [u for u in l["used_by"] if u["crate"]["name"] not in own]
d["licenses"] = [l for l in d["licenses"] if l["used_by"]]
counts = {}
for l in d["licenses"]:
    counts[l["id"]] = counts.get(l["id"], 0) + len(l["used_by"])
names = {l["id"]: l["name"] for l in d["licenses"]}
known = {o["id"] for o in d["overview"]}
overview = [dict(o, count=counts[o["id"]]) for o in d["overview"] if o["id"] in counts]
overview += [{"id": i, "name": names[i], "count": n} for i, n in counts.items() if i not in known]
overview.sort(key=lambda o: -o["count"])

s = ["""## Rust crates

The Specular binaries (the viewer and `specular-decode-host`) statically link the following
crates. Where a crate is offered under a choice of licences, the licence Specular uses is the one
listed. Each licence text appears once, followed by the crates it covers.

| Licence | Crates |
|---|---|
"""]
s += [f"| {o['name']} | {o['count']} |\n" for o in overview]
s.append("\n")
for l in d["licenses"]:
    used = ", ".join(" ".join(key(u)) for u in l["used_by"])
    s.append(f"### {l['name']}\n\nUsed by: {used}\n\n```text\n{l['text']}\n```\n\n")
open(out, "w", encoding="utf-8").write("".join(s))
EOF

geonames=$(sed -n 's/^geonames-attribution = //p' crates/specular-metadata/i18n/en/metadata.ftl)
# UnRAR (C++, compiled into specular-decode-host through unrar_sys): its licence text ships verbatim.
unrar_dir=$(cargo metadata --locked --format-version 1 --manifest-path crates/specular-decode-host/Cargo.toml |
    python3 -I -c 'import json, os, sys
for p in json.load(sys.stdin)["packages"]:
    if p["name"] == "unrar_sys":
        print(os.path.dirname(p["manifest_path"]))')
unrar_license="$unrar_dir/vendor/unrar/license.txt"
[[ -f "$unrar_license" ]] || { echo "UnRAR licence not found at $unrar_license" >&2; exit 1; }
# libwebp and libjxl (C/C++, compiled into the viewer for Convert… by libwebp-sys and jpegxl-src):
# their licence texts, and those of the libraries libjxl bundles, ship verbatim.
crate_dir() {
    cargo metadata --locked --format-version 1 --manifest-path app/specular/Cargo.toml |
        python3 -I -c 'import json, os, sys
for p in json.load(sys.stdin)["packages"]:
    if p["name"] == sys.argv[1]:
        print(os.path.dirname(p["manifest_path"]))' "$1"
}
webp_dir=$(crate_dir libwebp-sys)
jxl_dir=$(crate_dir jpegxl-src)/libjxl
for f in "$webp_dir/vendor/COPYING" "$jxl_dir/LICENSE" "$jxl_dir/third_party/highway/LICENSE" \
    "$jxl_dir/third_party/brotli/LICENSE" "$jxl_dir/third_party/skcms/LICENSE"; do
    [[ -f "$f" ]] || { echo "Encoder licence not found at $f" >&2; exit 1; }
done
verbatim() {
    printf '### %s\n\n%s\n\n```text\n' "$1" "$2"
    cat "$3"
    printf '\n```\n\n'
}
{
cat <<EOF
# Third-party notices

<!-- Generated by tools/release/third-party.sh. Do not edit by hand. -->

Specular is © The Specular authors and is licensed under the
[MIT License](LICENSE-MIT) or the [Apache License 2.0](LICENSE-APACHE), at your option.

Specular includes the data, fonts and software listed below, each under its own licence.

## Data

### GeoNames (place names for offline reverse geocoding)

$geonames

- Source: GeoNames, <https://www.geonames.org/>, via the \`reverse_geocoder\` crate.
- Licence: Creative Commons Attribution 4.0 International (CC BY 4.0),
  <https://creativecommons.org/licenses/by/4.0/>. Changes: converted by \`reverse_geocoder\` from
  the GeoNames cities export into the city list it embeds; Specular uses it unmodified.

### Unicode CLDR (locale data via ICU4X)

Locale data from the Unicode Common Locale Data Repository, © Unicode, Inc., under the
Unicode License v3 (text in the Rust crates section below, \`icu_*\` crates).

### Natural Earth (coastlines on georeferenced science images)

Coastlines from Natural Earth (<https://www.naturalearthdata.com/>), 1:110m physical vectors,
\`ne_110m_coastline\`, in the public domain. Packed by
\`crates/formats/specular-gis/gen/make_coastlines.py\` into
\`crates/formats/specular-gis/src/coastline110.bin\` (coordinates rounded to 0.001°, repeated
points dropped).

### EPSG Geodetic Parameter Dataset (map projections and datum shifts)

Coordinate reference systems, projections and datum transformations from the EPSG Geodetic
Parameter Dataset (<https://epsg.org/>), version 12.029, © International Association of Oil &
Gas Producers (IOGP), used under the EPSG Dataset terms of use
(<https://epsg.org/terms-of-use.html>). Read from PROJ's \`proj.db\` (data only, no PROJ code) and
packed by \`crates/formats/specular-gis/gen/epsg.py\` into \`crates/formats/specular-gis/src/epsg.bin\`
(parameters as published; unused fields left out).

### Colour maps (false colour and colour bars)

The 256-entry colour-map tables in \`crates/specular-inspect/src/colormap/tables.rs\` are
generated by \`tools/gen-colormaps.py\` from the published data below. Changes: the values are
copied as 32-bit floats into Rust arrays (jet is sampled at 256 points from its piecewise
definition; the cmocean tables are rounded to six decimals and curl's 512 rows resampled to
256 as cmocean does); nothing else is altered.

- viridis: Stéfan van der Walt and Nathaniel Smith, released into the public domain under
  CC0 1.0 (<https://creativecommons.org/publicdomain/zero/1.0/>), taken from matplotlib's
  \`_cm_listed.py\`.
- cividis: Jamie R. Nuñez, Christopher R. Anderton and Ryan S. Renslow, "Optimizing
  colormaps with consideration for color vision deficiency to enable accurate interpretation
  of scientific data", PLoS ONE 13(7): e0199239 (2018), as distributed with matplotlib
  (\`_cm_listed.py\`, matplotlib 3.10). jet: matplotlib's \`_cm.py\` definition. Both under the
  matplotlib licence below. Copyright (c) 2012- Matplotlib Development Team; All Rights
  Reserved.
- batlow, roma, vik, oslo, lajolla: Fabio Crameri, Scientific colour maps 8.0,
  doi:10.5281/zenodo.8035877, <https://www.fabiocrameri.ch/colourmaps/>, as the 256-entry
  tables of the cmcrameri 1.10 package, under the MIT licence below.
- thermal, haline, solar, ice, deep, dense, algae, rain, speed, balance, curl, phase: Kristen
  M. Thyng, Chad A. Greene, Robert D. Hetland, Heather M. Zimmerle and Steven F. DiMarco,
  "True colors of oceanography: guidelines for effective and accurate colormap selection",
  Oceanography 29(3): 9–13 (2016), doi:10.5670/oceanog.2016.66, as the tables of the cmocean
  4.0.3 package (<https://github.com/matplotlib/cmocean>), under the MIT licence below.

\`\`\`text
MIT License

Colormaps in cmcrameri/cm/cmaps:
Copyright (c) 2020 Fabio Crameri

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
\`\`\`

\`\`\`text
The MIT License (MIT)

Copyright (c) 2015 Kristen M. Thyng

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
\`\`\`

\`\`\`text
License agreement for matplotlib.

1. This LICENSE AGREEMENT is between the Matplotlib Development Team ("MDT"), and the
Individual or Organization ("Licensee") accessing and otherwise using matplotlib software in
source or binary form and its associated documentation.

2. Subject to the terms and conditions of this License Agreement, MDT hereby grants Licensee
a nonexclusive, royalty-free, world-wide license to reproduce, analyze, test, perform and/or
display publicly, prepare derivative works, distribute, and otherwise use matplotlib alone or
in any derivative version, provided, however, that MDT's License Agreement and MDT's notice
of copyright, i.e., "Copyright (c) 2012- Matplotlib Development Team; All Rights Reserved"
are retained in matplotlib alone or in any derivative version prepared by Licensee.

3. In the event Licensee prepares a derivative work that is based on or incorporates
matplotlib or any part thereof, and wants to make the derivative work available to others as
provided herein, then Licensee hereby agrees to include in any such work a brief summary of
the changes made to matplotlib.

4. MDT is making matplotlib available to Licensee on an "AS IS" basis. MDT MAKES NO
REPRESENTATIONS OR WARRANTIES, EXPRESS OR IMPLIED. BY WAY OF EXAMPLE, BUT NOT LIMITATION,
MDT MAKES NO AND DISCLAIMS ANY REPRESENTATION OR WARRANTY OF MERCHANTABILITY OR FITNESS FOR
ANY PARTICULAR PURPOSE OR THAT THE USE OF MATPLOTLIB WILL NOT INFRINGE ANY THIRD PARTY
RIGHTS.

5. MDT SHALL NOT BE LIABLE TO LICENSEE OR ANY OTHER USERS OF MATPLOTLIB FOR ANY INCIDENTAL,
SPECIAL, OR CONSEQUENTIAL DAMAGES OR LOSS AS A RESULT OF MODIFYING, DISTRIBUTING, OR
OTHERWISE USING MATPLOTLIB, OR ANY DERIVATIVE THEREOF, EVEN IF ADVISED OF THE POSSIBILITY
THEREOF.

6. This License Agreement will automatically terminate upon a material breach of its terms
and conditions.

7. Nothing in this License Agreement shall be deemed to create any relationship of agency,
partnership, or joint venture between MDT and Licensee. This License Agreement does not grant
permission to use MDT trademarks or trade name in a trademark sense to endorse or promote
products or services of Licensee, or any third party.

8. By copying, installing or otherwise using matplotlib, Licensee agrees to be bound by the
terms and conditions of this License Agreement.
\`\`\`

### Speex codebooks (Speex sound)

The Speex codebooks and filter tables in \`crates/formats/specular-speex/src/tables.rs\`, and
the constants listed in that crate's \`PROVENANCE.md\`, are copied as data from libspeex 1.2.1.
The decoder is Specular's own.

\`\`\`text
Copyright 2002-2008 Xiph.org Foundation
Copyright 2002-2008 Jean-Marc Valin
Copyright 2005-2007 Analog Devices Inc.
Copyright 2005-2008 Commonwealth Scientific and Industrial Research Organisation (CSIRO)
Copyright 1993, 2002, 2006 David Rowe
Copyright 2003 EpicGames
Copyright 1992-1994 Jutta Degener, Carsten Bormann

Redistribution and use in source and binary forms, with or without modification, are
permitted provided that the following conditions are met:

- Redistributions of source code must retain the above copyright notice, this list of
  conditions and the following disclaimer.

- Redistributions in binary form must reproduce the above copyright notice, this list of
  conditions and the following disclaimer in the documentation and/or other materials
  provided with the distribution.

- Neither the name of the Xiph.org Foundation nor the names of its contributors may be used
  to endorse or promote products derived from this software without specific prior written
  permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS \`\`AS IS'' AND ANY EXPRESS
OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF
MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE
FOUNDATION OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL,
EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE
GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING
NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED
OF THE POSSIBILITY OF SUCH DAMAGE.
\`\`\`

### Nellymoser tables (Flash sound)

The four Nellymoser tables in \`crates/specular-video/src/nelly_data.rs\` are copied as data
from FFmpeg 8.0's \`libavcodec/nellymoser.c\` (from the nelly2pcm project). The decoder is
Specular's own.

\`\`\`text
Copyright (c) 2007 a840bda5870ba11f19698ff6eb9581dfb0f95fa5,
                   539459aeb7d425140b62a3ec7dbf6dc8e408a306, and
                   520e17cd55896441042b14df2566a6eb610ed444
Copyright (c) 2007 Loic Minier <lool at dooz.org>
                   Benjamin Larsson

Permission is hereby granted, free of charge, to any person obtaining a copy of this software
and associated documentation files (the "Software"), to deal in the Software without
restriction, including without limitation the rights to use, copy, modify, merge, publish,
distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the
Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or
substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING
BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
\`\`\`

### Lensfun lens database (lens corrections in Develop)

Lens profiles for raws without corrections of their own come from the Lensfun project's lens
database (<https://lensfun.github.io/>), its \`data/db\` XML files at commit
\`52cf1a7b0e82e07e0ecaa3391ee5106a6dd1ee54\` (2026-11-04), © the Lensfun contributors.

- Licence: Creative Commons Attribution-ShareAlike 3.0 Unported (CC BY-SA 3.0),
  <https://creativecommons.org/licenses/by-sa/3.0/>; the licence text ships next to the data
  (\`crates/specular-lensfun/data/COPYING.CC_BY-SA_3.0\`).
- Changes: none to the files' contents; they are joined into one zstd-compressed bundle
  (\`crates/specular-lensfun/data/PROVENANCE.md\`). Any change Specular makes to them is shared
  under the same licence.
- Only the data is used. The Lensfun library is not part of Specular; the parser and the
  correction maths are Specular's own.

### CineForm codebooks (CineForm video, GoPro GPR raws)

The run/magnitude codebooks 9 and 17 in \`crates/formats/specular-cineform/src/tables.rs\` are
copied as data from the GoPro CineForm SDK (\`Common/table9.inc\`, \`Common/table17.inc\`;
dual-licensed Apache-2.0 or MIT, used under MIT). Codebook 17 is also the VC-5 codebook that
GoPro GPR raws use. The decoders are Specular's own.

\`\`\`text
Copyright 2017 GoPro, Inc.

Permission is hereby granted, free of charge, to any person obtaining a copy of this software
and associated documentation files (the "Software"), to deal in the Software without
restriction, including without limitation the rights to use, copy, modify, merge, publish,
distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the
Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or
substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING
BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
\`\`\`

### Basis Universal tables (KTX2 and .basis textures)

The block configurations, partition seeds and DCT constants in
\`crates/formats/specular-texture/src/basis/\` (\`hdr6x6i_tables.rs\`, \`xuastc_tables.rs\`,
\`xuastc_cfg.bin\`, \`xuastc_dct.rs\`, and the format tables in \`xubc7.rs\`) are extracted from
Binomial's UASTC HDR 6x6i, XUASTC LDR and XUBC7 specifications on the basis_universal wiki
(<https://github.com/BinomialLLC/basis_universal>), © Binomial LLC, under the Apache License
2.0 (text under "Rust crates" below). No Basis Universal source code is used
(\`crates/formats/specular-texture/PROVENANCE.md\`).

### BCn block decoders (DDS, KTX and other GPU textures)

\`crates/formats/specular-texture/src/bcn.rs\` is a fork of bcdec_rs 0.2.0, itself a port of
bcdec.

\`\`\`text
bcdec_rs: Copyright (c) 2023 SMG
bcdec:    Copyright (c) 2022 Sergii Kudlai

Permission is hereby granted, free of charge, to any person obtaining a copy of this software
and associated documentation files (the "Software"), to deal in the Software without
restriction, including without limitation the rights to use, copy, modify, merge, publish,
distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the
Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or
substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING
BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
\`\`\`

### QOA tables (Quite OK Audio)

The scale factor and dequantisation tables in \`crates/specular-video/src/qoa_sound.rs\` come
from the QOA specification and reference header (<https://qoaformat.org/>), under the MIT
License (same text as above).

\`\`\`text
Copyright (c) 2023, Dominic Szablewski - https://phoboslab.org
\`\`\`

### Monkey's Audio tables (Monkey's Audio sound)

The two range-coder symbol-width tables in \`crates/formats/specular-ape/src/tables.rs\` are
copied as data from the Monkey's Audio SDK 13.43 (\`Source/MACLib/UnBitArrayBase.h\`). The
decoder is Specular's own.

\`\`\`text
Copyright 2000-2026 Matthew T. Ashland

Redistribution and use in source and binary forms, with or without modification, are
permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice, this list of
   conditions and the following disclaimer.

2. Redistributions in binary form must reproduce the above copyright notice, this list of
   conditions and the following disclaimer in the documentation and/or other materials
   provided with the distribution.

3. Neither the name of the copyright holder nor the names of its contributors may be used to
   endorse or promote products derived from this software without specific prior written
   permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY EXPRESS
OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF
MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE
COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL,
EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE
GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING
NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED
OF THE POSSIBILITY OF SUCH DAMAGE.
\`\`\`

### Adobe Glyph List names (Type 1 fonts)

The glyph name to Unicode mapping in \`crates/formats/specular-font/src/type1.rs\` uses names
from the Adobe Glyph List (<https://github.com/adobe-type-tools/agl-aglfn>).

\`\`\`text
Copyright 2002-2019 Adobe (http://www.adobe.com/).

Redistribution and use in source and binary forms, with or without modification, are
permitted provided that the following conditions are met:

Redistributions of source code must retain the above copyright notice, this list of
conditions and the following disclaimer.

Redistributions in binary form must reproduce the above copyright notice, this list of
conditions and the following disclaimer in the documentation and/or other materials provided
with the distribution.

Neither the name of Adobe nor the names of its contributors may be used to endorse or promote
products derived from this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY EXPRESS
OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF
MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE
COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL,
EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE
GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING
NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED
OF THE POSSIBILITY OF SUCH DAMAGE.
\`\`\`

### S-57 object catalogue (nautical charts)

The S-57 object class and attribute codes, acronyms and names in
\`crates/formats/specular-gis/src/s57_catalogue.rs\` are transcribed from GDAL's data files
\`s57objectclasses.csv\` and \`s57attributes.csv\` (<https://gdal.org/>). No GDAL code is used.
GDAL is under an MIT-style licence:

\`\`\`text
Copyright (c) the GDAL/OGR project contributors

Permission is hereby granted, free of charge, to any person obtaining a copy of this software
and associated documentation files (the "Software"), to deal in the Software without
restriction, including without limitation the rights to use, copy, modify, merge, publish,
distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the
Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or
substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING
BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
\`\`\`

## Fonts

All bundled fonts are under the SIL Open Font License 1.1. The fonts are embedded unmodified;
under the OFL they may be bundled with any software but not sold on their own.

EOF
for f in "$FONTS"/*-OFL.txt; do
    name=$(basename "$f" -OFL.txt)
    printf '### %s\n\n```text\n' "$name"
    cat "$f"
    printf '\n```\n\n'
done
cat <<'EOF'
## Modified Rust crates

### resvg

resvg 0.48.1 (MIT OR Apache-2.0, © 2017 the Resvg Authors; licence texts in the Rust crates
section below) is vendored to `third_party/resvg` and used through `[patch.crates-io]`, with one
change, recorded in `third_party/resvg/patches/specular-raster-hook.patch`: a
`set_raster_decoder` hook so that raster images inside SVG files are decoded by Specular's own
decoders instead of the third-party image crates resvg would otherwise link.

### jpegxl-src

jpegxl-src 0.12.0 (BSD-3-Clause, from jpegxl-rs) is vendored to `third_party/jpegxl-src` and used
through `[patch.crates-io]`, with changes to its build script library for Windows, recorded in
`third_party/jpegxl-src/patches/specular-windows-build.patch`: with `CMAKE_GENERATOR=Ninja`, libjxl
is built with clang-cl directly instead of Visual Studio's ClangCL toolset; it uses the same C
runtime as Rust (dynamic unless `+crt-static`) instead of always the static one; and a CMake build
folder left by the other generator is started afresh. The crate only drives the build; the libjxl
it compiles is unchanged (see libjxl below).

## Decode host

`specular-decode-host` is the sandboxed helper that ships next to the viewer and runs C/C++ file
parsers.

### UnRAR

RAR and CBR archives are read with UnRAR by Alexander L. Roshal, compiled into
`specular-decode-host`. It is used only to extract archives; Specular does not create RAR archives.


Provenance: UnRAR 7.1.0 as vendored by the `unrar_sys` 0.5.8 crate (MIT OR Apache-2.0 bindings;
upstream commit 8fe139781b92d0678afecc568496f5805401a6c4), copied unchanged to
`third_party/unrar_sys` and used through `[patch.crates-io]`.

**Modified UnRAR source.** Specular builds UnRAR with three patches,
`third_party/unrar_sys/patches/specular-handle-io.patch`, `specular-password.patch` and
`specular-no-win32k.patch`, applied at build time (the vendored files stay pristine). The first
changes `file.hpp`, `file.cpp` and `volume.cpp` so that UnRAR reads
archives only through file handles the viewer opened and passed to the sandboxed decode host
(UnRAR then opens nothing by path and creates no files), and so that further volumes are
requested through the same hook instead of UnRAR's volume callbacks. The second patch changes
`dll.cpp` and `file.cpp` so that, before an archive is opened, UnRAR asks the same hook for the
archive's password (byte by byte, wiped after use), which reaches encrypted headers and files
without a C-string password. The third, for Windows, changes `os.hpp` and `isnt.cpp` so that
UnRAR maps case and converts OEM text with kernel32 (`third_party/unrar_sys/specular`) instead
of user32 and asks WMI nothing, which lets the decode host run with Win32k system calls disabled.
Decompression code is not changed. As the UnRAR licence requires for distributing modified UnRAR source, its paragraph 2 is
reproduced here, in the licence text below, in the patch and in the patched source comments:

> UnRAR source code may be used in any software to handle
> RAR archives without limitations free of charge, but cannot be
> used to develop RAR (WinRAR) compatible archiver and to
> re-create RAR compression algorithm, which is proprietary.
> Distribution of modified UnRAR source code in separate form
> or as a part of other software is permitted, provided that
> full text of this paragraph, starting from "UnRAR source code"
> words, is included in license, or in documentation if license
> is not available, and in source code comments of resulting package.


```text
EOF
cat "$unrar_license"
cat <<'EOF'

```

### libheif

HEIC/HEIF images are decoded with libheif (LGPL-3.0) when it is installed on the system. Specular
does not ship or link libheif: `specular-decode-host` loads the system copy at run time.

### Open CASCADE Technology (OCCT)

STEP and IGES models are read with Open CASCADE Technology (LGPL-2.1 with the Open CASCADE
exception, © Open Cascade SAS) when it is installed on the system. Specular does not ship OCCT
or include its code: its own small bridge library (`libspecular-occt`, Specular's licence) links
the system's OCCT shared libraries, and `specular-decode-host` loads the bridge, and with it
OCCT, at run time. Replacing or removing the system's OCCT needs no change to Specular.
Source: https://dev.opencascade.org/ and https://github.com/Open-Cascade-SAS/OCCT.

### libCZI

Zeiss CZI files are read with libCZI (LGPL-3.0, © Carl Zeiss Microscopy GmbH) when it is
installed on the system. Specular does not ship libCZI or include its code: its own small bridge
library (`libspecular-czi`, Specular's licence) links the system's libCZI shared library, and
`specular-decode-host` loads the bridge, and with it libCZI, at run time. Replacing or removing
the system's libCZI needs no change to Specular. Specular only reads CZI files.
Source: https://github.com/ZEISS/libczi.

## Encoders

Convert… writes WebP (lossy) with libwebp and JPEG XL with libjxl, both compiled into the viewer.
They encode pixels Specular has already decoded; they never read files.

EOF
verbatim libwebp "WebP lossy encoding. libwebp is © Google Inc., under the BSD 3-Clause licence." \
    "$webp_dir/vendor/COPYING"
verbatim libjxl "JPEG XL encoding. libjxl is © the JPEG XL Project Authors, under the BSD 3-Clause licence." \
    "$jxl_dir/LICENSE"
verbatim Highway "SIMD library bundled with libjxl (Apache-2.0 or BSD-3-Clause; Apache-2.0 text below)." \
    "$jxl_dir/third_party/highway/LICENSE"
verbatim Brotli "Compression library bundled with libjxl (MIT)." "$jxl_dir/third_party/brotli/LICENSE"
verbatim skcms "Colour management library bundled with libjxl (BSD-3-Clause)." \
    "$jxl_dir/third_party/skcms/LICENSE"
cat "$CRATES"
} >"$TMP"

if [[ $CHECK == 1 ]]; then
    if ! diff -q "$OUT" "$TMP" >/dev/null; then
        echo "$OUT is out of date: run tools/release/third-party.sh" >&2
        exit 1
    fi
    echo "$OUT is current"
else
    mv "$TMP" "$OUT"
    echo "wrote $OUT ($(grep -c '^### ' "$OUT") sections)"
fi
