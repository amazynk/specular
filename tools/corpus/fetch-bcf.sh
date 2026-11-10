#!/usr/bin/env bash
# Fetch the buildingSMART BCF-XML schemas (XSDs) and official test cases into
# tools/corpus/bcf/ (gitignored: nothing downloaded here is committed). The specular-bim tests
# read them from SPECULAR_BCF_CORPUS, else tools/corpus/bcf, and skip when they are absent:
# the official files are read, written back and read again, and what Specular writes is
# validated against the XSDs with xmllint when it is installed.
#
# Usage: tools/corpus/fetch-bcf.sh
#
# Source: https://github.com/buildingSMART/BCF-XML at pinned commits of release_2_1 and
# release_3_0, about 2.5 MB. Licence: (c) buildingSMART International Ltd., CC BY-ND 4.0;
# kept out of the repository. Every file is checked against its SHA-256 below; a mismatch
# stops the script. Downloads are untrusted data: this script only copies them.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_BCF_CORPUS:-$ROOT/tools/corpus/bcf}
mkdir -p "$OUT"

die() { echo "fetch-bcf: $*" >&2; exit 1; }

n=0
while IFS=$'\t' read -r sha commit path out; do
  [[ -z "$sha" ]] && continue
  dest="$OUT/$out"
  if [[ -f "$dest" ]] && echo "$sha  $dest" | sha256sum -c --status; then
    n=$((n + 1))
    continue
  fi
  mkdir -p "$(dirname "$dest")"
  url="https://raw.githubusercontent.com/buildingSMART/BCF-XML/$commit/${path// /%20}"
  echo "fetch-bcf: $out"
  curl -fsSL --retry 3 -o "$dest.part" "$url" || die "download failed: $url"
  echo "$sha  $dest.part" | sha256sum -c --status || { rm -f "$dest.part"; die "SHA-256 mismatch: $out"; }
  mv "$dest.part" "$dest"
  n=$((n + 1))
done <<'LIST'
95f04f8dae16cb72b6a0215793b5853151c196c7cf1d3895635e251feae62887	92fdcf4bfaa8f0576e7562730f341fc7781e2065	Test Cases/v2.1/Markup/Document Reference External/Document Reference External.bcf	cases/2.1/markup-document-reference-external.bcf
eb83967b96fe04dc44e9d127df02d7136ca8a6f55c661db2f63f23ada4ee45b7	92fdcf4bfaa8f0576e7562730f341fc7781e2065	Test Cases/v2.1/Markup/ExternalBIMSnippet/ExternalBIMSnippet.bcf	cases/2.1/markup-externalbimsnippet.bcf
3c616e12d658ee9d3ad1ddef0c98ea98638657ab6ee47c4c239ca66d50548591	92fdcf4bfaa8f0576e7562730f341fc7781e2065	Test Cases/v2.1/Markup/InternalBIMSnippet/InternalBIMSnippet.bcf	cases/2.1/markup-internalbimsnippet.bcf
c2898379705620ba4ced2091c6ab5c80cc49aadfc6d105173001d939cf8a5767	92fdcf4bfaa8f0576e7562730f341fc7781e2065	Test Cases/v2.1/Markup/MaximumInformation/MaximumInformation.bcf	cases/2.1/markup-maximuminformation.bcf
f49e251c1ef1c7384113f0c5a4da6d3197fed6b62b183eedc79d886b2f4402d4	92fdcf4bfaa8f0576e7562730f341fc7781e2065	Test Cases/v2.1/Markup/MinimumInformation/MinimumInformation.bcf	cases/2.1/markup-minimuminformation.bcf
81a93d244ffd820c8a9eee83704baef7d70029f03c9aa1520d4fcef54c5fd683	92fdcf4bfaa8f0576e7562730f341fc7781e2065	Test Cases/v2.1/Markup/PDFFile/PDFFile.bcf	cases/2.1/markup-pdffile.bcf
791731747a029953263695ae69f3d5a6269eed550693e14c4f39ff454b705111	92fdcf4bfaa8f0576e7562730f341fc7781e2065	Test Cases/v2.1/Markup/related topics with both topics in the same file/case.bcf	cases/2.1/markup-related-topics-with-both-topics-in-the-same-file.bcf
bd59a8336c2d9a59323cb6431e7a4b23479f488eff633a25717cc1cca1095d3b	92fdcf4bfaa8f0576e7562730f341fc7781e2065	Test Cases/v2.1/Markup/related topics with the other topic missing/case.bcf	cases/2.1/markup-related-topics-with-the-other-topic-missing.bcf
edad1a82e7c3ae66649617b0657441bc7469d01639cb0c210977ddee4164120f	92fdcf4bfaa8f0576e7562730f341fc7781e2065	Test Cases/v2.1/Markup/user assignment/case.bcf	cases/2.1/markup-user-assignment.bcf
badfc502947bd01e68cb7ca01ee4376ad79a4f463de6c7b3690ce51c98970046	92fdcf4bfaa8f0576e7562730f341fc7781e2065	Test Cases/v2.1/Project/ExtensionSchema/ExtensionSchema.bcf	cases/2.1/project-extensionschema.bcf
2cf82bfff0015c9505e0d3b95ce09c3883e5a8630bf1215d9e2c0d6270a8210a	92fdcf4bfaa8f0576e7562730f341fc7781e2065	Test Cases/v2.1/Visualization/all components and spaces visible/case.bcf	cases/2.1/visualization-all-components-and-spaces-visible.bcf
182359e429288591e97d4d1c19c93d749faec6508713c77965a28c45be37949e	92fdcf4bfaa8f0576e7562730f341fc7781e2065	Test Cases/v2.1/Visualization/All parts of curtain wall visible/All parts of curtain walls visible.bcf	cases/2.1/visualization-all-parts-of-curtain-wall-visible.bcf
e057f49f745bdebded926720e49cc5d12e867b4d13d9ff0df3f4aa8ac536cead	92fdcf4bfaa8f0576e7562730f341fc7781e2065	Test Cases/v2.1/Visualization/All parts of decomposed object except one visible/All parts except one visible.bcf	cases/2.1/visualization-all-parts-of-decomposed-object-except-one-visible.bcf
238aab74daecc0ef34c30ac79c667bf8de772ebaa928e2d26ec9c4c6a4aed0a9	92fdcf4bfaa8f0576e7562730f341fc7781e2065	Test Cases/v2.1/Visualization/Component Selection/Component Selection.bcf	cases/2.1/visualization-component-selection.bcf
1f8337c6d768332e940a4c4a9840046d9a51abcca26ebb9ac82115b8432d963f	92fdcf4bfaa8f0576e7562730f341fc7781e2065	Test Cases/v2.1/Visualization/One part of decomposed object visible/One part visible.bcf	cases/2.1/visualization-one-part-of-decomposed-object-visible.bcf
bc350fe80cffef2ee0ed14e4d6b70d0d3da7684ca95873bc8bd3b0734a78b0c1	92fdcf4bfaa8f0576e7562730f341fc7781e2065	Test Cases/v2.1/Visualization/OrthogonalCamera/OrthogonalCamera.bcf	cases/2.1/visualization-orthogonalcamera.bcf
a6493920b095ba812be025d2eab7268c8e2a068cdeff625ecc64829e84c3ebbc	92fdcf4bfaa8f0576e7562730f341fc7781e2065	Test Cases/v2.1/Visualization/PerspectiveCamera/PerspectiveCamera.bcf	cases/2.1/visualization-perspectivecamera.bcf
2160dbdba128fad8293ffc452778eb61070d531f5c4ff4c97fc2602791b6ce76	92fdcf4bfaa8f0576e7562730f341fc7781e2065	Test Cases/v2.1/Visualization/single invisible wall/case.bcf	cases/2.1/visualization-single-invisible-wall.bcf
e41c7349b98eabb23f03691812af329c15d809b29b58cbe9d0c21b30bfd740d5	92fdcf4bfaa8f0576e7562730f341fc7781e2065	Test Cases/v2.1/Visualization/Single visible space/case.bcf	cases/2.1/visualization-single-visible-space.bcf
75e8848e4afaf82186a89c7646f1b4be25db0d1ff04879f1160340207b4fb04e	92fdcf4bfaa8f0576e7562730f341fc7781e2065	Test Cases/v2.1/Visualization/Single visible wall/case.bcf	cases/2.1/visualization-single-visible-wall.bcf
0390c4b3ad14c0f268d5c496514bf8239644c0cfc54b1bc300f18c83f3152403	bc48611d0d7a1587f028a2b69677a1aafd5cd0a8	Test Cases/v3.0/Markup/Document Reference External/document reference external.bcf	cases/3.0/markup-document-reference-external.bcf
5c00d9deb69c5c60cb2f6f1c5b029a3a529e65ed509f1cf9588a88bd648d75c3	bc48611d0d7a1587f028a2b69677a1aafd5cd0a8	Test Cases/v3.0/Markup/Document reference internal/Document reference internal.bcf	cases/3.0/markup-document-reference-internal.bcf
4fd20571501fc6bcf39ff8d8d9c3c0f22d542c919c645ef3391bf566760a05cf	bc48611d0d7a1587f028a2b69677a1aafd5cd0a8	Test Cases/v3.0/Markup/Due Date/DueDate.bcf	cases/3.0/markup-due-date.bcf
a085748e109b60f2d8885ede65a1e62b69d63943573b0ef136caf7009be229ec	bc48611d0d7a1587f028a2b69677a1aafd5cd0a8	Test Cases/v3.0/Markup/Labels/Labels.bcf	cases/3.0/markup-labels.bcf
ee46d1d80468e15b7fc5ed76e3a14e66066de9990dd2207cc2c2ad144cdbaa7c	bc48611d0d7a1587f028a2b69677a1aafd5cd0a8	Test Cases/v3.0/Markup/Milestone/Milestone.bcf	cases/3.0/markup-milestone.bcf
58b1625e8793c8b474806d5ded35673b5d62beca910334f0cfeb552ff81d1236	bc48611d0d7a1587f028a2b69677a1aafd5cd0a8	Test Cases/v3.0/Markup/Minimum information/Minimum information.bcf	cases/3.0/markup-minimum-information.bcf
56e61c65520bcfc60e5ac22452ee73b98286ea4c9199dbd7beecf686a93cbe85	bc48611d0d7a1587f028a2b69677a1aafd5cd0a8	Test Cases/v3.0/Markup/Related topics with both topics in the same file/Related topics with both topics in the same file.bcf	cases/3.0/markup-related-topics-with-both-topics-in-the-same-file.bcf
3f665c791f90f71fa5aee9535b8c01be087fd3a9bf24bc459526f4c5306e100e	bc48611d0d7a1587f028a2b69677a1aafd5cd0a8	Test Cases/v3.0/Markup/User assignment/User assignment.bcf	cases/3.0/markup-user-assignment.bcf
c101d19430b860e0186cf8950ab0335fcbaa94974f505edca4ef5a1fe25e7b51	bc48611d0d7a1587f028a2b69677a1aafd5cd0a8	Test Cases/v3.0/Visualization/all components and spaces visible/all components and spaces visible.bcf	cases/3.0/visualization-all-components-and-spaces-visible.bcf
47933df0b521bf97d65e6776471f6a08bcbf8e3e90d0fe1f862f2e28713ad9ba	bc48611d0d7a1587f028a2b69677a1aafd5cd0a8	Test Cases/v3.0/Visualization/All parts of curtain wall visible/all parts of curtain walls visible.bcf	cases/3.0/visualization-all-parts-of-curtain-wall-visible.bcf
cca4371c8f6c4e4b223ac8b472ebd2ffe22b6c839c9ff3ee8066f5b0048a7606	bc48611d0d7a1587f028a2b69677a1aafd5cd0a8	Test Cases/v3.0/Visualization/All parts of decomposed object except one visible/all parts except one visible.bcf	cases/3.0/visualization-all-parts-of-decomposed-object-except-one-visible.bcf
93441fd46c66fa8037dd309d3cba94ac8c54788b2f0e5c0de2ed999e1599b7c7	bc48611d0d7a1587f028a2b69677a1aafd5cd0a8	Test Cases/v3.0/Visualization/Component selection/component selection.bcf	cases/3.0/visualization-component-selection.bcf
361eb1b2949918a325fbdd3a2b28c706019ad9e32095738ce483381d23c017fb	bc48611d0d7a1587f028a2b69677a1aafd5cd0a8	Test Cases/v3.0/Visualization/One part of decomposed object visible/one part of decomposed object visible.bcf	cases/3.0/visualization-one-part-of-decomposed-object-visible.bcf
4819d8310b64c024a8ae09f9bbe30598bc567bcf04ee9d091c8cce1bcb1ecac8	bc48611d0d7a1587f028a2b69677a1aafd5cd0a8	Test Cases/v3.0/Visualization/Orthogonal camera/orthogonal camera.bcf	cases/3.0/visualization-orthogonal-camera.bcf
e9c71573d93ab9a6e9378427a8f4a52a9dfc317a137ce4dd4ea6a4b1b3e1e24f	bc48611d0d7a1587f028a2b69677a1aafd5cd0a8	Test Cases/v3.0/Visualization/Perspective camera/perspective camera.bcf	cases/3.0/visualization-perspective-camera.bcf
0f81d353de9f9b9e63a888a8f12b1530bd87ace62db46f3502a23e17e0160b91	bc48611d0d7a1587f028a2b69677a1aafd5cd0a8	Test Cases/v3.0/Visualization/Single invisible wall/single invisible wall.bcf	cases/3.0/visualization-single-invisible-wall.bcf
fffb9bb617ca316960c07b796b63b3f8f4702b02cb372a6dc8d427b327e38b3c	bc48611d0d7a1587f028a2b69677a1aafd5cd0a8	Test Cases/v3.0/Visualization/Single visible space/single visible space.bcf	cases/3.0/visualization-single-visible-space.bcf
08513c08d950693d36fe37f5c3d78fd37d0a92978f165c9fd4c1fd08fb2e30c2	bc48611d0d7a1587f028a2b69677a1aafd5cd0a8	Test Cases/v3.0/Visualization/Single visible wall/single visible wall.bcf	cases/3.0/visualization-single-visible-wall.bcf
aabe73f5ca7aad37bb6343836a9fedff21233bf10bb9873727fd89236ba22e94	bc48611d0d7a1587f028a2b69677a1aafd5cd0a8	Test Cases/v3.0/Visualization/Topics with different models visible/Topics with different models visible.bcf	cases/3.0/visualization-topics-with-different-models-visible.bcf
15e9af22065870d93ad45431147748adc5b32ef830d6dfe7b4eb10cf5e6c4441	92fdcf4bfaa8f0576e7562730f341fc7781e2065	Schemas/markup.xsd	schemas/2.1/markup.xsd
a1b45e394cadf076209f22ec9f2225039932f580a50cd898c14c4cb9b1c9d5d2	92fdcf4bfaa8f0576e7562730f341fc7781e2065	Schemas/project.xsd	schemas/2.1/project.xsd
bb1d108e349423fef4a69ecc1648797e7e73a18ee994bcdf98f978a6a3adf899	92fdcf4bfaa8f0576e7562730f341fc7781e2065	Schemas/version.xsd	schemas/2.1/version.xsd
ab11687511d9595fc136bcec57b4f640f0e8364d913f9b54c8dc267a13c6f247	92fdcf4bfaa8f0576e7562730f341fc7781e2065	Schemas/visinfo.xsd	schemas/2.1/visinfo.xsd
105da6cd7452e6a7c9ea54a6f0ac2f0420e7d8c6fb8d2ac9f6f2075edae4178f	bc48611d0d7a1587f028a2b69677a1aafd5cd0a8	Schemas/documents.xsd	schemas/3.0/documents.xsd
df9b49fc2368aba41cec8259fad266fe82d735b5319469834e7fe19bf0d83153	bc48611d0d7a1587f028a2b69677a1aafd5cd0a8	Schemas/extensions.xsd	schemas/3.0/extensions.xsd
6c80f1c566ace8b405ae0ab530438aec2014bf35dd14cfc3c4cf0c5aa0b9d169	bc48611d0d7a1587f028a2b69677a1aafd5cd0a8	Schemas/markup.xsd	schemas/3.0/markup.xsd
44cf05c63fddcba335589cbbf5824458118561ffc23bb6c3e72a1f3217cce68a	bc48611d0d7a1587f028a2b69677a1aafd5cd0a8	Schemas/project.xsd	schemas/3.0/project.xsd
93d20dfcc6d7f451ae85a8277559b0347478c278218a42f0ef41dcc5550a2090	bc48611d0d7a1587f028a2b69677a1aafd5cd0a8	Schemas/shared-types.xsd	schemas/3.0/shared-types.xsd
cef0dcc112239457776cab33a5adeed0cf61c344e05bb5c6c4674c2df3643fa6	bc48611d0d7a1587f028a2b69677a1aafd5cd0a8	Schemas/version.xsd	schemas/3.0/version.xsd
0855bc002310b4a9decd67b1b21631bd2e0368f0878183da8448e34d807e7418	bc48611d0d7a1587f028a2b69677a1aafd5cd0a8	Schemas/visinfo.xsd	schemas/3.0/visinfo.xsd
LIST
echo "fetch-bcf: $n files in $OUT ($(du -sh "$OUT" | cut -f1))"
