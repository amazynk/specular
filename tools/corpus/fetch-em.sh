#!/usr/bin/env bash
# Fetch the electron-microscopy and scanning-probe test corpus (Gatan DM3/DM4, MRC, Gwyddion
# .gwy, Igor .ibw) into tools/corpus/em/ (gitignored: nothing here is committed), generate the
# open-time files, and write the oracles' expected planes next to them. The specular-em and
# specular-spm corpus tests read it from SPECULAR_EM_CORPUS (a worktree has no corpus of its
# own), else tools/corpus/em, and skip when it is absent.
#
# Usage: tools/corpus/fetch-em.sh [SECTION...]
#   SECTION is a function name below (e.g. src_rsciio). Default: every section, then the
#   oracle (src_oracle).
#   OUT defaults to $SPECULAR_EM_CORPUS, else tools/corpus/em.
#   PYTHON (default python3) needs numpy, ncempy, rosettasciio, mrcfile, gwyfile and igor2 for
#   the generated files and the oracle. A venv under target/ is fine:
#     python3 -m venv target/em-venv && target/em-venv/bin/pip install --no-cache-dir \
#       numpy ncempy rosettasciio mrcfile gwyfile igor2
#   then PYTHON=target/em-venv/bin/python.
#
# What it covers (about 250 MB, plus about 300 MB of generated open-time files):
#   dm/rsciio      RosettaSciIO's DigitalMicrograph test files, v0.15.0: 1-D, 2-D, 3-D and 4-D,
#                  every data type, DM3 and DM4, stacks, several images, packed complex FFT
#   dm/scifi       pycroscopy SciFiDatasets DM3/DM4 (EELS, spectrum images, survey images)
#   mrc/rsciio     RosettaSciIO's MRC test files (Direct Electron 4D-STEM and movies)
#   mrc/mrcfile    mrcfile's test maps (EMDB maps, EPU and FEI extended headers)
#   mrc/scifi      a Velox-exported 4D-STEM MRC
#   gwy/           Gwyddion files (SciFiDatasets, gwyfile's test file)
#   ibw/           Igor binary waves: igor2's tests (versions 2, 3 and 5, both byte orders)
#                  and Asylum / Nanoprobe AFM waves from SciFiDatasets
#   generated/     crates/formats/specular-em/gen/make_samples.py big (open-time files)
#   expected/      the oracles' planes and calibrations (ncempy, mrcfile, gwyfile, igor2)
#
# Every download is pinned by sha256. Downloads are untrusted data: this script only copies
# files, never runs anything from them (the oracle reads them with python -I from a script
# outside the corpus). Stops if free space on the target filesystem drops below MIN_FREE_GB
# (default 8).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_EM_CORPUS:-$ROOT/tools/corpus/em}
MIN_FREE_GB=${MIN_FREE_GB:-8}
PYTHON=${PYTHON:-python3}
mkdir -p "$OUT"

die() { echo "fetch-em: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

provenance() { printf '%s\t%s\t%s\n' "$1" "$2" "$3" >> "$OUT/.provenance.tsv"; }

# pinned URL DEST SHA256 LICENCE: downloads once and checks the hash (a mismatch deletes the
# download and stops).
pinned() {
  local url=$1 dest=$OUT/$2 sha=$3 lic=$4
  if [[ -f $dest ]] && [[ $(sha256sum "$dest" | cut -d' ' -f1) == "$sha" ]]; then
    return
  fi
  check_space
  mkdir -p "$(dirname "$dest")"
  curl -fsSL --retry 3 -o "$dest.part" "$url"
  local got
  got=$(sha256sum "$dest.part" | cut -d' ' -f1)
  if [[ $got != "$sha" ]]; then
    rm -f "$dest.part"
    die "$2: sha256 $got, expected $sha"
  fi
  mv "$dest.part" "$dest"
  provenance "$2" "$url" "$lic"
}

# list BASE_URL DEST_DIR LICENCE < "SHA256 RELPATH" lines (RELPATH may hold spaces and
# parentheses).
list() {
  local base=$1 dir=$2 lic=$3 sha rel n=0
  while read -r sha rel; do
    [[ -z $sha || $sha == \#* ]] && continue
    local enc=${rel// /%20}
    enc=${enc//(/%28}
    enc=${enc//)/%29}
    pinned "$base/$enc" "$dir/$rel" "$sha" "$lic"
    n=$((n + 1))
  done
  echo "  $n files -> $dir"
}

need_python() {
  "$PYTHON" -I -c "import $1" 2>/dev/null || die "$PYTHON lacks '$1' (see the header for a venv)"
}

# ------------------------------------------------------------------------------- sections

RSCIIO=https://github.com/hyperspy/rosettasciio/raw/v0.15.0/rsciio/tests/data
SCIFI=https://raw.githubusercontent.com/pycroscopy/SciFiDatasets/f751c3ce35924a94948989864a4a7ac87adecd65/data/microscopy
MRCFILE=https://raw.githubusercontent.com/ccpem/mrcfile/a2a8c6b569a57b7f18b023b5056fa7a14f2f99c2/tests/test_data
IGOR2=https://raw.githubusercontent.com/AFM-analysis/igor2/cff4854154d537c87a85b429af42cbe8ccc76a69/tests/data
GWYFILE=https://raw.githubusercontent.com/tuxu/gwyfile/423cad9e9d24c9b3014a75a84b5b8433ae08117f/tests

src_rsciio() {       # RosettaSciIO test data, v0.15.0 (GPL-3.0); hashes from its registry.txt
  list "$RSCIIO/digitalmicrograph" dm/rsciio GPL-3.0 <<'LIST'
1415584aeadae829aba952c580a9683fefe2772d59d5e222d7ec227d3bb8693e 1D/test-1.dm3
ba470b29e4406d697179914a661b2c7bc58d0d7b9fe8825290f29b4fdfcf57c8 1D/test-1.dm4
c31852a04274d16a5a104e5427e92fd49151da5c8cd74f1219b300439ffc2aa9 1D/test-10.dm3
78a2913d8cc96d16464a5af06d68cd0731cb50905402aa63f99354f3ab2e9f06 1D/test-10.dm4
c35c07a49e315435c42878701c84e988a82bdb3867a1b5e451dde143b05e7bd3 1D/test-11.dm3
91d49e7eba683927ae0af9447d8e7a671333541c85704724d32d8e106aa85acd 1D/test-11.dm4
79611255b02fee1419da6191a905c19f023400055a9404f044cce782e8e369a5 1D/test-12.dm3
44629706682a6a3e0f071589485cf1c986d1b32ea6d16bdc265dc578fc9a927b 1D/test-12.dm4
dc808cf6c95542295aec0dc95e47644690def0bfb1c176eaeda680c4fcfdafb0 1D/test-13.dm3
461f1bb50bd44ca53f5c059770db442b0e151d0eeb8fab5da79f1a863f02d767 1D/test-13.dm4
3a3455711d363d86b43eec2aa7b969db2fe8dec1cae50f4429ace638d51b0a29 1D/test-14.dm3
03f05f4f8226503a322ac2d74e5d845879df65fcedffb3e116f6677afad85acb 1D/test-14.dm4
6c32573ae204f2da2deae5e813c0b29a44c8a3c704cf2f72eed603f2d4014c07 1D/test-2.dm3
017119cbcf8d7e268bdaef733ff86daa33460bde42d5fbbaac2b53b2d73f77e2 1D/test-2.dm4
3902b4ee2035cfe226bad397c7e312d6a633a44cf161a5be10db8b977a37d9ff 1D/test-23.dm3
c899e09664a92e242354cd96b0329e10a83bf042e6c61abb1f8a5ff9404e5e56 1D/test-23.dm4
112ca6907371bfbf27fc8678379d870f0296402fb217fe6bafb2057a06efdbd7 1D/test-27.dm4
d4a779711b719b60c06d027c72d726521c105ac9ffa045d27cd97cb96584ff8e 1D/test-28.dm4
3ee6fe3f7ceb49870c380d68abfa342d4a571b8d21180553a9a1783c95b2007e 1D/test-3.dm3
f80265cb571a500bd865767ea7b085707f203484a7aae708d88ab2285097871e 1D/test-3.dm4
fe8a97809d2d13acd9db90d254112fc38891c40d3c5e475bb64f0917eae75c13 1D/test-5.dm3
943b1061bd01c5ca750c92f48d81e4ae238339868fe49df2b10a34afead6234d 1D/test-5.dm4
41e649f9c45470b3f4474c2727f6fa9e98ae16dfdd809106757207b1858b0f6a 1D/test-6.dm3
73e60b211c15f24a3be84eb2f19c3745d0c3c813e1318cfe2da12010120183c6 1D/test-6.dm4
95a286a19235cef03e564e0cbe2d84b0d65ca59d18fc4681fbd61d3fe14c85c8 1D/test-7.dm3
324ebccd429985f2f66e56b5fad9a7ee0ad914b4b457ce6aec925040392e16cb 1D/test-7.dm4
20448cb1118cd9f448668a96894c6fee8c196b9652f1ba3c8e36cb8ca3e5f890 1D/test-8.dm3
a858718122b11c5abca86262fd9797c413382ae6d15e5d1e78f7a4020ebe1bee 1D/test-8.dm4
857381bdab24a21acd0df7431d8afd31c81b0103a698219d9e8cc7d1874d8887 1D/test-9.dm3
a737483ec4e0592e24cc6dab6fbb417def44ac63741b955c183b7d8295176fa5 1D/test-9.dm4
e051f16cb70d57d70c50239d5d9f9c97789a355c2b605ddcfe8e981aaa3cf114 1D/test-EDS_spectrum.dm3
2cb9960a8027300ac1884a3a3b45d10d964da63cc805364d2787519c7e1ec96f 1D/test-EELS_spectrum.dm3
716e4aa36615de43ec7f13098157f647dbeabebff872645fd1f49724189eb174 1D/test-MonarcCL_spectrum-ccd.dm4
0ce4715d3435973db9c9cf25c5a2753ef90506f648f81d3af77d34d976c4b63b 1D/test-MonarcCL_spectrum-pmt.dm4
bcf857ca89b99852fe083988433b92726004c9807f519d72cb3ee978f1e6c35a 1D/test-MonoCL_spectrum-ccd.dm4
1af41146cf4c7a02f30f03d1870c6ed83d66def3b1339e7032b75c2e0f6f0b35 1D/test-MonoCL_spectrum-pmt.dm4
cd025111a163cad9ef6fb0331b740ddefa2f7024c3f691723bd5235a313aa12e 2D/multi_signal.dm3
19832d1fc253c0d4b48f3fbbf5761c1057244d79f621e9d7bd91b819c0df5115 2D/test-1.dm3
c02359ad86d741d90e7ec981ef0a38a682f38f5156a196d95992c419f490255f 2D/test-1.dm4
d0dc8be061e6338b2f192e185c0fea6d90a91eaeea7796534128780160da3824 2D/test-10.dm3
6b699af5e227db37cbfe90da6c4f1e64654e7dda21161428410449db5b89b3ee 2D/test-10.dm4
6a7f82f6ab1167cbf84312e5160e8a3f25acb329304ceacb750292eb956d8daa 2D/test-11.dm3
4e5da4a58564b9b542cf3b3a7d3b0acd7d2ef797a2679659ed44ea3acab36c83 2D/test-11.dm4
9799ab2a92d759c330ad211caaaa3d44973372624ec45bd88a138cc7640d4a02 2D/test-12.dm3
73f4c17b2d49a433f49cedc3263a172310e0481b6304becc5a3fc86283c31d42 2D/test-12.dm4
6ac618a9f0348005cf5a878bc9ed759917bbd47f7b7c84c0cc137666ba7108bd 2D/test-13.dm3
0ed868dc3a3c105c0cc53e806c898d19de953475849dbae399161da39c86b6ee 2D/test-13.dm4
8ea4751d82e0e5094f79569b018ed95548ffdf162892728a69242d970972ed6f 2D/test-14.dm3
81734f1a46fbf750a9002ec554d4e4e87feb22eb9fb28935f7ee5a0a225a97c7 2D/test-14.dm4
f40485142df39dbdc06d7225983f3e0676c8a3a1caa614fbe5c1918c8a33b1c7 2D/test-2.dm3
db23e0fa74d1c50ea1ef3c4df847e952aa15cc97eb25ea96deede46ba1e0edcd 2D/test-2.dm4
d76f77fcaf513d80f0c1cc888c5bb7b94d65c718145f44bd216d4a981ac64273 2D/test-23.dm3
dd1ab701c314d9d543af26347d53b3c8d1e3b331a961c5b24f4ba28593a81015 2D/test-23.dm4
4f517cfca2603e6454760d16ee4313bcf8d5456e64c47cd10b0ddaad2656ebee 2D/test-27.dm4
ada979b6cb3b57e1e0bbb2f28c3478c55c3a59d0917efe23cfebc825deed3569 2D/test-28.dm4
0b1bd42517e5ebb4330974123704612c52aed5dc1570d6bf05b21246871ff2bd 2D/test-3.dm3
782c65bc04c20c04226d77e9fd4d85e658c2a3a80d7bd12b9f2dd6cadae97792 2D/test-3.dm4
502751ef7481454f59282118a04b655913da509651e7ff91fc19f9a14f4e37de 2D/test-5.dm3
faf0fa45df6cf6de08462491300e378f45c988abc8b711828c68cabe4ff080cc 2D/test-5.dm4
a93e145838a912a46352ea8c695b832516a1d95e8c09ef20c083d1b4583e8f00 2D/test-6.dm3
21cc563062b8ecbff22b9981657213f7e7815ddf03cdab402fd7b9f8aa9e4408 2D/test-6.dm4
ae59c7a95df6d02a9e3b46f58a887f34ca88d1f215f76b31172ca474d2a99828 2D/test-7.dm3
7c86d95e68ce8009a7d75ec18c39ec91b97803123e13db96dabb17acb413114c 2D/test-7.dm4
824491e25662e012d5b7a7936a5b171f1eb3e2899bf9ec4b28f9a7d037baf2da 2D/test-8.dm3
289d58cb13b6deb0932b9aaf256aa04d13d5e017fa92c09aa7b01a59ecdd7132 2D/test-8.dm4
c5c62146c004cf47806d0e650b9ce33e3ce3d429a42f15e2de33273b8515aeab 2D/test-9.dm3
9360d3880ea0a08da091dcf4e545ed95bd62f15f8f8f36ed42d0ed2a9f2e2d14 2D/test-9.dm4
4ea04a10ad46595de79a20c283555f299dd07b4667f6112b44db1699f28993b6 2D/test-MonarcCL_mono-image.dm4
56b5ea782b8c4bb6b4fd85c7a0f9d9318f0ac96a0879bb27cf4ae2cd2be405ce 2D/test-MonarcCL_spectrum-SI.dm4
b8bda6ae96c718c0fdb63e996ce68911d4bc74f20f86cf4a6c26046412ed6132 2D/test-MonoCL_spectrum-SI.dm4
fcba806af75bc3c2e50e7d7b0626cba545c1464f39e99c8922b225b0b0b0224c 2D/test_STEM_image.dm3
ebd6c192ce39e6bd5e67e2c6d53a36809307723023b16c9e149c7afbc3ef601a 2D/test_diffraction_pattern.dm3
f43a2860beb3ef33aed76f0f784d975c115839063e4dd5a1efea90b02243c3aa 2D/test_diffraction_pattern_tags_removed.dm3
ef7c9bd2c8afafa9c17583d70b07dad883c5c3f006857ed8544c0c727cc84267 2D/test_fft_packed_complex8.dm4
f9b8203f384cc355437594b649d4ea3a12cddeb231ebb65508a841a379fa171a 3D/EELS_SI.dm4
2f15abc94e008f3433caf2c0340311762020793f86356cec8a94d0dbea343c5a 3D/test-1.dm3
9bf4ac7d415700fea0d70f2113703a98da33ce7f05e1f2cd998047298f2f85b4 3D/test-1.dm4
aa4fabe34e533fddf7bd4b100bfeb5bfc890b1d7eecb394612f728af473b8e33 3D/test-10.dm3
977fdf87ab3ff220708e1a9381b8822317e21b04ffa0a6f9f2dc733276628782 3D/test-10.dm4
db1563c228c50c541f3a5a6b5c01f67b9a80e0d49c479036b52e79610acd0e97 3D/test-11.dm3
9ae59809275e0ee04f57cdd0b64eb636ebc729c14fb6696ac572bc4fe68e8005 3D/test-11.dm4
501bfedf28226228b1f93973616d7186906fde84780a42d3ba50d4fa19a1f53d 3D/test-12.dm3
fda142c998624b1133798f3c61321dc862b242122734975f341f10fc68be70a3 3D/test-12.dm4
f10f0ba22b06f7b67ff31e8f0ab5eaaf54bf738fe775e87cf82d9d046f7b86aa 3D/test-13.dm3
a5806b630dbda38551e0df039096ed3cdc7de17bad17c57ea5578f640aa9b5ab 3D/test-13.dm4
47eb186bdb4a284de48925e3e80788fe7078e9d9174ea3ebdce2d6c3eb6f741c 3D/test-14.dm3
c52aa3d5d4e32333ef817626335292087aeb395dea09877fbfe27320b04d3145 3D/test-14.dm4
b58bddb07242d3587b45166d5a7c6120079c9f3c3dfe69ac5c7e009eb8511981 3D/test-2.dm3
4635cedce026b3405bb016ac637419f8a7854a4c2d6197d284fbfdb1e0f8b6d3 3D/test-2.dm4
dd262538157bfedda3ad3d62ded2175cd65d7f962e653d5cc47123cf07df994f 3D/test-23.dm3
b9318c392de13089eb0eeb9fcb21347989f92faec1f5eb39382976c1aa4ee866 3D/test-23.dm4
9ff9889f51a266a422db0dfd2d556b3018bd1452486df534cf4accfa9b37f2ee 3D/test-27.dm4
694b3e3f66e2bcb7f7bdf82eb64a6dd2e072b29c3f54318f7ee922a177b27ee5 3D/test-28.dm4
ef5e1890fd37a699ae6b774ad13c61397bab91b786ac42d586bb4177a398929d 3D/test-3.dm3
28a615ac72932620c4174d2865e1ec9683ccbcd7c27ce9a7cad988ec7f61c18e 3D/test-3.dm4
1531f61547da126a283571c92f3ded1faf2ab8f064c5876ba8d418ecbdf7c3f4 3D/test-5.dm3
b83a4757c0ce01135866195ac9ed413453ff495873bc57c26fedff0e3b672bea 3D/test-5.dm4
e21b728c964db0285f68840acc5c474c0eaec4f7ebfb18bbb35851e5d35771a7 3D/test-6.dm3
233f1ac5739b46b0fa0ee4d5359540040650e4e43a36eb194e51d649100b797b 3D/test-6.dm4
24d75f233db16ccd95bb0627252c6e6411c338b70cbba73fa1388c8540f63c40 3D/test-7.dm3
8904e35d4a83b9344aed16ab4e56d36026ab0e2ea44e09074803a51669c84922 3D/test-7.dm4
4c2434f60ba8923f4fedfe3dd554fa00893c2de2eac779154339ba0c1d3b77d5 3D/test-8.dm3
e4c82571e7511dd4155de95baa8d407976b0f282a83e626644ff1b1a8f80e9f8 3D/test-8.dm4
4a87b9be4145a95d0fb53145e6af36860c0fed547483c2346876ad5315fae999 3D/test-9.dm3
c74a0141b471db0961f1893fd5c09e6c8d0041d7e4714cb12dca44161a65f631 3D/test-9.dm4
7a35359858a07501e993c696ab6ab4ff6222c89be36754d1b6fd542e6a8fcd8c Fei HAADF-DE_location.dm3
d5bbf4c9c049fb7c20a272c7653fbdeed8e8727bca0e7fc980b1231a131cbc66 Fei HAADF-FR_location.dm3
5e135032dec4ea847db9600f8fd7d16a99ea7aa23d3cde6ab61eed62a218242e Fei HAADF-MX_location.dm3
3264325b6f79457737f6ff71e3979ebe508971a592c24e15d9ee4ba876244e56 Fei HAADF-UK_location.dm3
41070d0fd25a838a504f705e1431735192b7a97ca7dd15d9328af5e939fe74a2 test_stackbuilder_imagestack.dm3
LIST
  list "$RSCIIO/mrc" mrc/rsciio GPL-3.0 <<'LIST'
3f798cf43ff1f3c39750b379a22962a4c6d2d546f1298461fabc6bced0cc876f 20241021_00405_0_Virt 0_sum.mrc
3f798cf43ff1f3c39750b379a22962a4c6d2d546f1298461fabc6bced0cc876f 20241021_00405_1_Virt 1_sum.mrc
4962c3ead6547d819e22447765e93e357911bacd2905191d97d339b5cc0dd1dc 20241021_00405_ext1_Ext 1.mrc
56c87ae1703a91ecd1f5a95de520fdaa9b83968c1bac4f040b68b1c6314a5d89 20241021_00405_final.mrc
e7b2a476bb64818454edc428d21e4d40da08e0a6362bfbe2d95a74084c22f718 20241021_00405_movie.mrc
9339446e7e981065718e5071a53aa7a5a6cf9fe5834f75a0bccb9a3365121823 4DSTEMscan.mrc
4d91c16ccce7b2bab77ce39f7b89b1bf62406c0291f12e703bb78f95d5c8c414 Custom_movie.mrc
873e9f862eb8f2372589be366317c9940ed71e94e440a219be07c0db977de758 HAADFscan.mrc
b55a28aa5826a2410770dd435f80c55844a471aba2501423670ca8ebd3ddc58e ROI_Random_Scan_Sum.mrc
714ff824b10c4ad976e18f71c45b52dacef0509160d2ade7c16e1dbdc863457d ROI_Random_Scan_movie.mrc
LIST
}

src_scifi() {        # pycroscopy SciFiDatasets (MIT)
  list "$SCIFI/em/tem" dm/scifi MIT <<'LIST'
aa24012809d4f817c3ce212af12eee5f5a987516124b376ffa618b39321335ce DMReader_EELS_STO.dm3
027d027cf1352b438f7b370bb9418408bd58c5cc769fa51b968e19907df8a7de DMReader_EELS_STO.dm4
d9862b002252c99fff7c2325ed50a3030c6c164c1ac0b0cfa74b881094772e0f DMReader_Image_SI-Survey.dm3
f67ed49e930c10fcef0d0cdcdc9814c2c282ecb43629d0c45150073cf1dc1961 DMReader_SI-HAADF.dm3
cd38e33895da5c58f411e83979376913276387d6aa6085834576f552f8bf8bfe DMReader_SpectrumImage_SI-EELS.dm3
LIST
  list "$SCIFI/em/tem" mrc/scifi MIT <<'LIST'
02d88eff1902c2c8dac635d581d720528299222521d3c6735ae8ceb45145a29b MRCReader_4DSTEM_Velox.mrc
LIST
  list "$SCIFI/spm" gwy MIT <<'LIST'
83ed6d08fa326d3dda3a755a211749bda061b9bcff3c18436ae37e54c7182085 GwiddionReader_bipm.gwy
b3776ef655a38c6584ac53b5354332d6dff8e5ce99b8d265fd2c212190a15d38 GwiddionReader_chip.gwy
eb23c79ba471c91c6edb263c73d6d40a87075d9d144900338423ab5db040cf3a GwiddionReader_samples.gwy
eb0bb8895a53bb53f6cc93471dbd4952ba31298d8317166c89efd72cc1a6c2d4 afm/PTO_110_Virgin0001.gwy
LIST
  list "$SCIFI/spm" ibw MIT <<'LIST'
80ad81492cedb3cd6683333abaac4b424094eb10df39f56a86a512ccde272751 afm/IgorIBWReader_ForceCurve.ibw
4c6bc09ccf0fa50227c5bf1af92b189ff2e4a4abfcd5f5ae1ddcc853cdad5bc6 afm/IgorIBWReader_ImageStack_BTFO_DSO.ibw
25435c3fc9eb73fefe5952ec9feefdb396c58c2b28e592f38d1b7aeac3672dc2 afm/IgorIBWReader_P0089OP0027.ibw
d3d17bbc8d5fe14920e5cc301e9669bc06ece54103788ad68e5282df442f240d afm/20230110-152047_ScV6Sn6-2023-01-10-STM_NANOPROBE_AtomManipulation--37_1-Aux2(V)_3_Forward_Down_Trace.ibw
e0a483272133d437e3b0f68e94c32a6789995f9b8594151c464b353968e1d4f9 afm/20230110-152047_ScV6Sn6-2023-01-10-STM_NANOPROBE_AtomManipulation--37_1-I_3_Backward_Down.ibw
LIST
}

src_mrcfile() {      # mrcfile's test maps (BSD-3-Clause; EMDB maps are CC0)
  list "$MRCFILE" mrc/mrcfile BSD-3-Clause <<'LIST'
c423c29e4704aaadcb6c10c954295f826de86ca7f6e3fb8a479a73da9949e32a EMD-3001.map
351d5090d4c56eb5fc41796842ad64abecc238b8da6181f8857be5844dbbc262 EMD-3197.map
c52b35f70216ba6ffed7523c70810806117676f463ee4f15c1da18697caf8653 epu2.9_example.mrc
e9f168012031b3a6ea47fbed8eca8b9c8a040b4d1f4fbb2e3326bdb6d9307efe fei-extended.mrc
LIST
}

src_igor2() {        # igor2's test waves (LGPL-3.0)
  list "$IGOR2" ibw/igor2 LGPL-3.0 <<'LIST'
d0f278a4aa39d12b68236b19504e5c0ef04ef8809f2825220f0821a6f36c931c mac-double.ibw
2b7ed80c79369aac0556051ee6fb8f13209556b94e040d37740de300ca665d46 mac-textWave.ibw
1c9433043339c105c29145964b983bb5b0b440a6c0f4aa319bb097567ed1aa9e mac-version2.ibw
3b00215222eb413fc09ec06ee9c5eda55a06b0a2a09ec5965981b64a66de9091 mac-version3Dependent.ibw
76f7135f707eb1266a6765800b60f362a7c703bcbdc9707867f351a917b9ad45 mac-version5.ibw
01f9a88313ed3a14ab582fac0486282158b2a15941673cbbac8f7a14e7d0c9d0 mac-zeroPointWave.ibw
48873e760352ba40d430048499cee417c0e2d9c4e5f0077059ad4d6a1e53cec7 win-double.ibw
555960bdc94409569847d48f641052d6c9ddf8024f7289573d60b8a13763f300 win-textWave.ibw
8b4c5caa5f6087bf0afae40d67ee117cd39a3f1ea9e16b0c595b96d845cdbb82 win-version2.ibw
981383c28a78e5064711bb2aa534775fe2c4242b3ef9aaca27b540178446ec4f win-version5.ibw
1a4d987bf72226310b7c7aa34cf6ccab22c82f6461d5fa53f068c86e27b3c2db win-zeroPointWave.ibw
LIST
}

src_gwyfile() {      # gwyfile's test file (MIT)
  list "$GWYFILE" gwy/gwyfile MIT <<'LIST'
14bd5af25db1a6baeed2204a7caa10ea01de5f75ce888673a72b038212bdd9f9 test.gwy
LIST
}

src_generated() {    # open-time files written by the oracle libraries (large DM, MRC, gwy, ibw)
  need_python mrcfile
  need_python gwyfile
  check_space
  "$PYTHON" -I "$ROOT/crates/formats/specular-em/gen/make_samples.py" big "$OUT/generated"
  provenance generated "crates/formats/specular-em/gen/make_samples.py" "generated"
}

src_oracle() {       # the oracles' planes for every file above
  need_python ncempy
  need_python mrcfile
  need_python gwyfile
  need_python igor2
  check_space
  "$PYTHON" -I "$ROOT/crates/formats/specular-em/gen/oracle.py" "$OUT"
}

main() {
  if (( $# == 0 )); then
    set -- src_rsciio src_scifi src_mrcfile src_igor2 src_gwyfile src_generated src_oracle
  fi
  for s in "$@"; do
    declare -F "$s" > /dev/null || die "unknown section $s"
    echo "$s"
    "$s"
  done
}

main "$@"
