#!/usr/bin/env bash
# Fetch the per-camera RAW set for the linear oracle run (raw_cameras test of specular-raw):
# one file per camera body / vendor family from raw.pixls.us (CC0 unless the site marks
# otherwise), pinned by SHA-256 (the site's filelist.sha256, October 2026). About 485 MB.
#
#   tools/quality/fetch-raw-cameras.sh            # -> $SPECULAR_QUALITY_CORPUS/raw-cameras
#   tools/quality/fetch-raw-cameras.sh --broad    # -> $SPECULAR_QUALITY_CORPUS/raw-cameras-broad
#   ONLY='LX3|FC220' tools/quality/fetch-raw-cameras.sh --broad   # just the matching files
#
# --broad fetches a second, wider sample instead (fidelity round 4, about 330 MB): 29 more
# bodies, older and smaller sensors from 16 makers (CRW, CR2, RAF incl. S2Pro and X-Trans,
# RWL, MRW, NRW, NEF incl. the D1, ORF, RW2, PEF, SRW, SR2, ARW 1 and 2) and DNG from Leica,
# Pentax, Ricoh, Apple, DJI and Adobe DNG Converter (lossless and lossy compressed). Run the
# raw_cameras test on it with SPECULAR_RAW_CAMERAS pointing there (oracle runner raw-broad).
#
# SPECULAR_QUALITY_CORPUS defaults to $CARGO_TARGET_DIR/corpus, else target/quality/corpus.
# Gitignored: nothing fetched here is committed. Downloads are untrusted data: this script
# only copies them. Stops when free space drops below MIN_FREE_GB (default 8).
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
base=${SPECULAR_QUALITY_CORPUS:-${CARGO_TARGET_DIR:-$root/target/quality}/corpus}
BROAD=0
[[ ${1:-} == --broad ]] && BROAD=1
out=$base/raw-cameras
(( BROAD )) && out=$base/raw-cameras-broad
MIN_FREE_GB=${MIN_FREE_GB:-8}
mkdir -p "$out"

FILES=(
  'Canon/EOS 1300D/IMG_0778.CR2|e064327405b3a736b1cc89d499bc5a049cfb2800bcb06a5072e6d0a82cd296d7'
  # The same body sold as the Rebel T6, at ISO 200: its highlights clip (where the white is).
  'Canon/EOS Rebel T6/IMG_2422.CR2|432e968a244816ed9c7bbae1807bbc66dc6245d294e9f11cb5b3ab1fc830df5a'
  'Canon/EOS RP/Canon_EOS_RP_CRAW_ISO_100_nocrop_nodual.CR3|bd3270610593d37ff8a399908bb9406a4e32f9151d55bf19ac1cf8f10d787b46'
  'Canon/EOS 300D/CRW_8395.CRW|9e7f62239fe728046af28d0d4a8a722d35d40146a79794a5bda1775306af4383'
  # Canon bodies whose data clips (white level and highlight checks): below the table white
  # (5D Mark III 15464, 6D 15025, 7D Mark II 13583) or at the 14-bit maximum where ColorData's
  # specular white sits lower (90D, R10); R5 data above its specular white, unclipped.
  'Canon/EOS 5D Mark III/5G4A9394.CR2|ec069b178b9383d80c72f8cdc1b79bd54ca451e7c20793b109d8ad73bef6bdf6'
  'Canon/EOS 6D/IMG_0001.CR2|08c7e7f32af01c4501a5f32ccecb7f35b8692e72566d60607e391e9f1053f4a5'
  'Canon/EOS 7D Mark II/capture000106.cr2|b9f71c56adb6e2861e6653efb312fb5cc88ba4aa17fa809c6a18b8db044360c3'
  'Canon/EOS 90D/Canon-90D-cRAW-ISO-100.CR3|c8c918b2d1a53e64d7d5009ebac89a920dbba5f766c2f396bb4ec95312b21034'
  'Canon/EOS R5/Canon_EOS_R5_CRAW_ISO_100_nocrop_nodual.CR3|2deae46b816e56ba91ae7a3892ca121290e12d5b22afb11f5b05a3d8f09f759f'
  'Canon/EOS R10/IMG_4361.CR3|15d017f0770b3da17180077617ffe07b786a1ec2cb36297f81a82a27a54756db'
  'Nikon/Z 50/DSC_0441.NEF|b25081cf9adcd51fec020b6d227ff094bc0327cbd26d76bad7e1dee3fb4c034a'
  'Sony/DSC-RX100M7/DSC00601.ARW|815532f017b94e31990e8111791131647c95f6953aa28c571783c5e17eb36523'
  'Fujifilm/X-S10/Fujifilm-X-S10-lossless-compressed.RAF|519e1ac6cedd0424c17896c4e5ba40810796927baf536055d39623996894c16f'
  'Olympus/E-M1MarkIII/_3160531.ORF|1e7a256eff14df75055dd7fe7a68aabefb4d8ed475317e80ebf7ff77d5bcfd3f'
  'OM System/OM-5/C1291096.ORF|ee0213ee4899d677d9bd538d8cfa4d0f69bdbacd462c1cf3ca3901036cf2fce7'
  'Panasonic/DC-GH5/_T012010.RW2|464b0cfc9c8960d632aba83f8b2f6d021c8e9bece1d18902e3b64eb2f1c6b0cf'
  'Pentax/K-3 Mark III/IMGP9854.PEF|29714bc4cc754a81d2ab154a4ecf1eb5fa125d3901b10f3bd88d731c674f69c4'
  'Pentax/K-3 Mark III/_IMG0114.DNG|1e3352ccba0db428fd2a183718fc857fb283b1d43ad704305e5094c19e0a0428'
  'Apple/iPhone 12 Pro/IMG_1361.DNG|e91e77a4533ed7cce551d83330676ea5c47dd5e55fb38adda7819366afdbdfc2'
  'Epson/R-D1s/EPSN3575.ERF|45bb0081baf1c813c7cd9e05b3e6ae734b90aba408dd1195118dc94dc6abbbb9'
  'Minolta/DiMAGE A2/PICT0881.MRW|8655e3428461377dd7c6b28ae98a0b23d659314997ed16333985278f99c9a4b4'
  'Kodak/DCS Pro 14nx/D7465857.DCR|0850ca7337169b609cef369f0de1e71952646d922b50cf1752b7ed6a2ef81ed5'
  'Hasselblad/CFV/RAW_HASSELBLAD_CFV.3FR|f7c0f941bd542ec638073bfa9febdc4efa01f4a4855d73c5d9218b975d1de91a'
)

BROAD_FILES=(
  'Canon/EOS 10D/CRW_7673.CRW|b0730eb3a67cc2281a32e0b2ee104199246928d954a68b7e5b72366514c73d02'
  'Canon/EOS 350D/IMG_1707.CR2|8cbb84e04d93b005fe082da9c954122a612b5281af00aa088d767850f343fd38'
  'Canon/EOS M/IMG_4624.CR2|27d151efe86f04bea8fcf6ec9f3a722ae3ee24a3f56a6ebbd27ee89b215cf569'
  'Canon/PowerShot G9/IMG_1188.CR2|ad27a4d751984cd709380e3be274a4a5243d36ca89c5fc711736ba8a9f5c932f'
  'Fujifilm/FinePix S2Pro/DSCF0002.RAF|9fe1bf1361f015d9db862228e4a26b23d69e81015ed4a032eaefb8242ae9ad42'
  'Fujifilm/FinePix S5600/RAW_FUJI_S5600S5200.RAF|8d24ac05af61c8af0a2e7e97166e0db142b5cfb126655462704bf3de03d30d23'
  'Fujifilm/X-T1/20171229_110916.RAF|e994a1fd6e87e392432fe146a35b0b88584dc2bd50bee2c8c7e886ac2b59fcde'
  'Leica/D-LUX 4/L1010119.RWL|71e55de649611a0e4cdedf53793ec177f5c093c5258a2922e1581f021211bc59'
  'Leica/M8/L1030132.DNG|7fdcca2371bb07fbf9483fcb08e3b5ab093eca1ca3924cbc86d6863736c31153'
  'Leica/M9/L1049390.DNG|d1d162fce62210b8951c862189a6098329b5163b7f7492dc397c6e53bba04239'
  'Minolta/DiMAGE 7/RAW_MINOLTA_DIMAGE7.MRW|4c5d4274bebb548f5f7b3001770903bc4023e7bfe4aeeebbd9d6fc08e81b03bb'
  'Nikon/Coolpix P7000/RAW_NIKON_P7000.NRW|09165ec1031b36e6ae43f2263b4400ec4d9c3444ae252d258d002f7e96dbea25'
  'Nikon/D1/RAW_NIKON_D1.NEF|7886d8b0e1257897faa7404b98fe1086ee2d95606531b6285aed83a0939b768f'
  'Nikon/D70/20170902_0047.NEF|dd6405aeb33b0cd5bf66c98ba98ccbb478a765450cfd130810e470dab8d1f4b4'
  'Nikon/D3100/DSC_0359.NEF|e8113b6e7285155f9f2b88f81566dab63a59cf5559d6915113b74e8c3a8680f6'
  'Olympus/E-1/E_1__C106743_gredos.ORF|042286653fbae5b085bef4a4e626145385ea6e82e817f166b4904abcef64457c'
  'Olympus/E-PL1/PB120976.ORF|d8895526c8215925468d503d1879b18c2e3e38140bffeba77351b9e55683aac9'
  'Panasonic/DMC-LX3/Lumix_LX3_16_9.rw2|bc8d4795a5604d605daf8cbffb7dbbadaf5c3f4e6472d88f12fea00bf90f78fa'
  'Pentax/K-x/IMGP6432.PEF|51a3f22d702353b246684dca79f83c79cfdbdf9892702ae5ac55dd469e239bb6'
  'Pentax/K10D/IMGP5398.DNG|e7ea961edfea725ad2753f261d20da5e6addd5d641e5bcbc373999710f27941f'
  'Ricoh/GR/R0003611.DNG|7ceec1ea31c56b6e0414202504581b915f17818ffe018b4c8bba1393332c405e'
  'Samsung/NX100/RAW_SAMSUNG_NX100.SRW|3149a901d154cc37175166c7ebce559b100e8ba872023c6b159028dcbae5d03b'
  'Sony/DSC-R1/_DSC1477.SR2|921c5f2513dd1671a9089e73fc778fc3573bc4cb70a3a555d0414661d0e62274'
  'Sony/DSLR-A100/_DSC0258.ARW|4df7981ac4baec9648b4f106751ea4b09a6e27bc70a15f8ef09bec051fec362d'
  'Sony/NEX-5N/DSC02519.ARW|5074d2a29755fac544cb6cb435153569613f849f1c1dd777cacd6455be046788'
  'Apple/iPhone 7 Plus/IMG_0739.DNG|e8c7ebd8f22f7281d165496b91c8fea8a8e79c8113172fb499e1d66d6045332e'
  'DJI/FC220/DJI_0004.DNG|85a4fc27cca3c1358abd22898d6426dc3448060ca5143e7bd255cb5fff64392d'
  'Adobe DNG Converter/Canon EOS 5D Mark III/5G4A9394-compressed-lossless.DNG|3118116735d4f6dd01fd9d6a80ee9aac52a2c2debdc6dcd112bd5e3059044a55'
  'Adobe DNG Converter/Canon EOS 5D Mark III/5G4A9394-compressed-lossy.DNG|159326856c29073e845c3c5a9ecf98c6474f43ca15798a88ad5e2baecd0664b7'
)
(( BROAD )) && FILES=("${BROAD_FILES[@]}")
# ONLY=<extended regex>: just the files whose path matches (one body or a few).
if [[ -n ${ONLY:-} ]]; then
  picked=()
  for e in "${FILES[@]}"; do [[ ${e%|*} =~ $ONLY ]] && picked+=("$e"); done
  FILES=("${picked[@]}")
fi

for e in "${FILES[@]}"; do
  rel=${e%|*}
  sha=${e##*|}
  dest=$out/$rel
  mkdir -p "$(dirname "$dest")"
  if [[ ! -s $dest ]]; then
    avail_kb=$(df -Pk "$out" | awk 'NR==2{print $4}')
    if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
      echo "fetch-raw-cameras: less than ${MIN_FREE_GB} GB free; stopping" >&2
      exit 1
    fi
    enc=$(python3 -I -c 'import sys,urllib.parse;print(urllib.parse.quote(sys.argv[1]))' "$rel")
    curl -fsSL --retry 3 -A 'specular-corpus-fetch/1 (test corpus)' -o "$dest.part" \
      "https://raw.pixls.us/data/$enc"
    mv "$dest.part" "$dest"
  fi
  echo "$sha  $dest" | sha256sum -c --quiet - || { echo "checksum mismatch: $rel" >&2; exit 1; }
done
echo "raw cameras in $out: ${#FILES[@]}"
