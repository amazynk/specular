#!/bin/sh
# The fmt500e corpus: a PlayStation movie (ffmpeg's FATE suite), tracker modules from
# modland (per-module copyright; dev corpus only), then the oracle's decodes beside them.
#   tools/corpus/fetch-fmt500e.sh <folder>
#   python3 tools/corpus/fmt500e-oracle.py <folder>
#   SPECULAR_FMT500E_CORPUS=<folder> cargo test -p specular-video --test fmt500e -- --nocapture
set -eu
out=${1:?folder}
get() { # folder, name, url
  mkdir -p "$out/$1"
  [ -s "$out/$1/$2" ] && return 0
  echo "fetching $1/$2"
  curl -fL --progress-bar -o "$out/$1/$2" "$3"
}
fate=https://fate-suite.ffmpeg.org
get str descent-partial.str $fate/psx-str/descent-partial.str

ml=https://ftp.modland.com/pub/modules
get trackers fonetag.669 "$ml/Composer%20669/Brother%20Mike/fonetag.669"
get trackers bliss.mtm "$ml/Multitracker/Andrew%20Clear/peaceful%20state%20of%20bliss.mtm"
get trackers galaxy.mtm "$ml/Multitracker/Benny/galaxy%20express%20999.mtm"
get trackers blackbox.okt "$ml/Oktalyzer/Blackbird/black%20box.okta"
get trackers piano.okt "$ml/Oktalyzer/FBY/piano-story.okta"
get trackers mb16.med "$ml/OctaMED%20MMD0/Alan%20Mackey/mb-music%2016.mmd0"
get trackers fairy.med "$ml/OctaMED%20MMD1/Aanstalten/fairy%20tale.mmd1"
get trackers hometown.med "$ml/OctaMED%20MMD2/Andy%20O%27Brien/hometown.mmd2"
get trackers mandzuria.med "$ml/OctaMED%20MMD3/Asarhad/hills%20of%20mandzuria.mmd3"
get trackers buzzare.ult "$ml/Ultratracker/Hacket%20Tobitts/buzzare.ult"
get trackers westend.stm "$ml/Screamtracker%202/GRC/west%20end%20girls.stm"
get trackers sunny.stm "$ml/Screamtracker%202/Alex%20Young/sunny%20rain.stm"
get trackers backinbusiness.ult "$ml/Ultratracker/Brad%20D.%20Oldham/back%20in%20business.ult"
get trackers communion.far "$ml/Farandole%20Composer/Marc/second-hand%20communion.far"
get trackers pond.far "$ml/Farandole%20Composer/Dafydd/pond%20in%20the%20palace%20of%20pythagoras.far"

pil=https://raw.githubusercontent.com/python-pillow/Pillow/main/Tests
get fonts 10x20-ISO8859-1.pcf $pil/fonts/10x20-ISO8859-1.pcf
get fonts ter-x20b.pcf $pil/fonts/ter-x20b.pcf
get fonts courB08.bdf $pil/images/courB08.bdf
# Bitmap fonts the system has (console PSF, Wine FON, X11 PCF), copied unpacked.
sys() { # folder, name, gzip or plain file
  mkdir -p "$out/$1"; [ -s "$out/$1/$2" ] && return 0; [ -f "$3" ] || return 0
  case "$3" in *.gz) gzip -dc "$3" > "$out/$1/$2" ;; *) cp "$3" "$out/$1/$2" ;; esac
}
sys fonts Lat15-Terminus16.psf /usr/share/consolefonts/Lat15-Terminus16.psf.gz
sys fonts Uni2-VGA16.psf /usr/share/consolefonts/Uni2-VGA16.psf.gz
sys fonts coure.fon /usr/share/wine/fonts/coure.fon
sys fonts cvgasys.fon /usr/share/wine/fonts/cvgasys.fon
sys fonts 6x13.pcf /usr/share/fonts/X11/misc/6x13-ISO8859-1.pcf.gz
nb=https://raw.githubusercontent.com/jupyter/nbformat/main/tests
get notebooks test4.ipynb $nb/test4.ipynb
get notebooks test3.ipynb $nb/test3.ipynb
get notebooks test4plus.ipynb $nb/test4plus.ipynb
dd=https://raw.githubusercontent.com/jgraph/drawio-diagrams/master/examples
for n in uml-deployment-example aws-simple-architecture infographic-example-1 timeline-example sysml-requirements-diagram concept-map-uml-diagrams-overview; do get diagrams $n.drawio $dd/$n.drawio; done
get lottie exploding-star.json https://raw.githubusercontent.com/Samsung/rlottie/master/example/resource/1643-exploding-star.json
# SWF: Ruffle visual tests (MIT/Apache-2.0), each with the picture Ruffle drew.
rf=https://raw.githubusercontent.com/ruffle-rs/ruffle/master/tests/tests/swfs/visual
for t in simple_shapes/gradients/gradients simple_shapes/gradients/radial simple_shapes/overlaps simple_shapes/winding_rule simple_shapes/layers simple_shapes/masks gradient_radial_same_ratios define_bits_lossless2_rgb15 color_transform_issue_9698 gradient_issue_9892 fonts/glyph; do
  n=$(echo $t | tr / -)
  for f in test.swf output.expected.png test.toml; do get swf "$n.$f" "$rf/$t/$f"; done
done
# 3D: assimp test models (BSD-3 repository, per-model notes), a MagicaVoxel sample (MIT).
am=https://raw.githubusercontent.com/assimp/assimp/master/test/models
get models faerie.md2 $am/MD2/faerie.md2
get models test.x $am/X/test.x
get models Wuson.ac $am/AC/Wuson.ac
get models jeep1.ms3d $am/MS3D/jeep1.ms3d
get models 01_01.bvh $am/BVH/01_01.bvh
get models monu1.vox https://raw.githubusercontent.com/ephtracy/voxel-model/master/vox/monument/monu1.vox
# Molecules: MDAnalysis test data (GPL-licensed repository; dev corpus only, never shipped).
md=https://raw.githubusercontent.com/MDAnalysis/mdanalysis/develop/testsuite/MDAnalysisTests/data
get mol Molecule.mol2 $md/mol2/Molecule.mol2
get mol adk_oplsaa.gro $md/adk_oplsaa.gro
rl=https://raw.githubusercontent.com/Samsung/rlottie/master/example/resource
for n in gears emoji_wink a_cup_of_coffee hourglass dna like telegram; do get lottie $n.json $rl/$n.json; done
