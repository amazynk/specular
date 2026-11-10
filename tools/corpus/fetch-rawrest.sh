#!/usr/bin/env bash
# Fetch the camera RAW corpus for the rarer RAW cases: Phase One IIQ sensor calibration (black
# tables, flat fields, defects, IQ3 100MP colour), Kodak DC50 (RADC) and DC120, Canon CRW with
# a CMYG filter (Pro70, G1), Sony SRF (DSC-F828), Sigma X3F (SD9/SD10 previews, vignetting,
# sd Quattro AF pixels), Leica D-Lux 7 and V-Lux 5, ARRI ALEXA Mini and Canon D30/G6 white
# balance, GoPro GPR (VC-5), Fujifilm SuperCCD SR, IIQ in .TIF, monochrome DNG. Into
# tools/corpus/rawrest/ (gitignored: nothing here is committed). Samples from raw.pixls.us
# (CC0 unless the site marks otherwise), pinned by URL and SHA-256 (from the site's
# filelist.sha256 in October 2026). About 900 MB.
#
# Usage: tools/corpus/fetch-rawrest.sh
#   OUT defaults to $SPECULAR_RAWREST_CORPUS, else tools/corpus/rawrest.
# Then compare with LibRaw (rawpy):
#   python3 -m venv $CARGO_TARGET_DIR/venv && $CARGO_TARGET_DIR/venv/bin/pip install rawpy numpy
#   SPECULAR_RAWREST_CORPUS=tools/corpus/rawrest SPECULAR_RAWPY=$CARGO_TARGET_DIR/venv/bin/python \
#     cargo test -p specular-raw --test rawrest -- --ignored --nocapture
#
# Downloads are untrusted data: this script only copies them. Stops if free space drops below
# MIN_FREE_GB (default 8).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_RAWREST_CORPUS:-$ROOT/tools/corpus/rawrest}
MIN_FREE_GB=${MIN_FREE_GB:-8}
mkdir -p "$OUT"
: > "$OUT/.provenance.tsv"

die() { echo "fetch-rawrest: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

# SHA-256 and raw.pixls.us path.
FILES=(
  # Phase One IIQ: black tables, flat fields and defects (P 20+ .. IQ180), IQ3 100MP colour
  'b0391cb29ebb0773ef53553572df2abffc9093b37d919aeab76d15e72751569d Phase One/P25+/CF028662.IIQ'
  '672a1bf9c2b9b43484f0549ce23a6bf8963efd4338cbc08592a2694c8a463eb6 Phase One/H 25/H25_Outdoor_.IIQ'
  '2dd56f2a7a57a7fc2123301009c1a7a10be002ed84a18470fc4d37e41760a66d Phase One/IQ3 100MP/Phase_One_IQ3_100MP_IIQ_S.IIQ'
  '5c25ca03da8c458bc5e0e9f453f92bd716a00744ce43bc635f3ada1698f55974 Phase One/P45+/CF125838.IIQ'
  'a724cd059480f801a5d9a9176ed38edd643dfd5bf38ca90c6243164b062826b8 Phase One/P65+/CF027310.IIQ'
  'c0c59c596a67abea79405b6b8eb2212a93d95fe4b41e46dad34b02e5eb376ae2 Phase One/IQ180/230214_9561.IIQ'
  '6422a8d863276fcc756e2f529ac406337f00ad05a19ef83f7893fafc358ff8b5 Phase One/IQ150/CF006803.IIQ'
  '61aa32c5947d386f0d5ee465c5183185df6c9cd7ff7eb66ffec12e84b487878c Phase One AS/P40+/CF100585.IIQ'
  # Kodak DC50 (RADC), DC120 (plain and JPEG-compressed)
  '37e290dbd0053f00e508d02a6b3a2a990432dad1eb74c40a52ca899f0f225ecc Kodak/DC50/RAW_KODAK_DC50.KDC'
  '967c3af47289e6cc5ab4f9bd24135b7e5386f574fc76deb3b13e884bfda87856 Kodak/DC120/P000439.kdc'
  '83d6888375aad48658e34cf4c2a3569bd203f3765105475ca98e04912d51d830 Kodak/DC120/RAW_KODAK_DC120_WITH_JPEG.KDC'
  # Canon CRW with a CMYG filter
  'f39be5acf14057ce04527fd8d0574e57ec8ed5fed082d7bfaff763075a44ee8a Canon/PowerShot G1/crw_1693.crw'
  '3e583286dd5e63b03aca3de6671b7921fc9fd814a7f4cbed50a20c028728c802 Canon/PowerShot Pro70/RAW_CANON_PRO70_SRGB.CRW'
  # Sony SRF (RGBE)
  '07739f94bcab0c7e198d418ac65172d0dce04bb84a7683ecffd0bfe4b32f6d79 Sony/DSC-F828/DSC06227.SRF'
  # Sigma X3F
  '9faed9efe3bb53f34534b487da3045f01d58935f7377450097b6063947964109 Sigma/Sigma SD9/RAW_SIGMA_SD9_SRGB.X3F'
  'ea436ba459a7478d7b338428656fae905469de269f928749f3388db2b7b0d53f Sigma/Sigma SD10/RAW_SIGMA_SD10.X3F'
  'bf26469746e5937ca4f9cfb34a022891f9c8518febd676355930b805b9307b48 Sigma/sd Quattro/sample3.X3F'
  'c6823f88e7dae554d3ec951d2366aab4b9ba2f8d64ad2d93028b2f9c50c811f2 Sigma/sd Quattro H/SDIM0061.X3F'
  '44508f7df4191aacaecb6461e1841dae002e5b71a4f95a00aa3f52d1c6d6bc5f Sigma/DP1/RAW_SIGMA_DP1.X3F'
  '74c0421f53e82b6a37a9ed63c7963f7cf34e49b5a51e70ff0ea7b41f726b2141 Sigma/SIGMA SD1 Merrill/SDIM0042.X3F'
  # GoPro GPR (DNG with VC-5 compression)
  '191901b64412083146256ccebeacd954783f6c40a4556ef355a067915136159b GoPro/HERO5 Black/GOPR0020.GPR'
  '17a24d42735464525773048172c888ea24d13ba4b907e64580b63c3bab987ae5 GoPro/HERO6 Black/GOPR8921.GPR'
  'f7d8a5b693bc173a777164dbad48921c8d145d602d5422925a13090404514b66 GoPro/HERO7 Black/GOPR0055.GPR'
  'bf6f67d0c7096f347b864310f6d58be35237c3d0e715b47a1b30e650e9d785b1 GoPro/HERO9 Black/GOPR0017.GPR'
  'f3eb9413ed30c92ac6e1834b8228ecf5c9a546c142e8e7ec01146c13035cb532 GoPro/FUSION/GPBK1322.GPR'
  # Colour matrices and white balance
  '6540d067665a842d903d2c44ec101e6f95f65759d4754a94f38817d7a2fd72f4 Leica/V-Lux 5/_1000200.RWL'
  '4f6db6492da83f7f2a5012591e370557249d1ed44d4c948784016605b799327c Leica/D-Lux 7/L1023208.RWL'
  '4d487a43d1259049d6f7145049ec33da378681dab1278a8eb3941ac231cc844a Arri/Alexa Mini/A004C002_190722_R20I.478.ari'
  '4ee482f2b6f9f1a8d370703ea04e9f2abaa49356973a7f56b229af93a78e8a2e Canon/EOS D30/CRW_2444.CRW'
  '244378e1f5660b6cc6cefc93a75ed4928d0b28aa6304ff327b61c748538f4fef Canon/PowerShot G6/CRW_2097.CRW'
  # Image and RAW variants: SuperCCD SR (F700: S and R rows interleaved; S3Pro and
  # S5Pro: separate blocks), newer GPR bodies, IIQ in .TIF (uncompressed H 25, P 25, IQ140
  # IIQ S), monochrome LinearRaw DNG (Leica, film scanner). About 210 MB.
  'c3686a66f9159043bd27afb0d749d566c2912a0374319e38f68fa6255ea565b4 Fujifilm/FinePix F700/RAW_FUJI_F700.RAF'
  '038e5a13a22513f53d143e328a66e2de8071809565db696ecac84b4364fcb75b Fujifilm/FinePix S3Pro/DSCF3694.RAF'
  'aca2b2ebcf90d248b4f822ed78bfeb73e79f1b64164a87dc2051a96726cdfa0c Fujifilm/FinePix S5Pro/2018_06150003.RAF'
  'bfd992594af380748f3d46e37814dae595356cb6fce182d4188230ca978e6a9f GoPro/HERO8 Black/GOPR0009.GPR'
  '92a47ef8a30b9e22440b0eb71c519e271e66e64c51765415b9de644c902b358f GoPro/HERO10 Black/GOPR0018.GPR'
  'd7a6be163c847cb8456e6858a99aef3b85dc3d8836bca1da1b8886effe0d43e1 GoPro/HERO11 Black/GOPR0123.GPR'
  '0683dbd9203c8b13040bc37eca41089c779c609cca2de76709c010dbee834bee Phase One/H 25/H25_IT8.7-2_Card.TIF'
  '1d73775444698df6012f2d1ce014fa0c4bab79d976be82d48d36f3f59985b72f Phase One/P25/CF082166.TIF'
  '4000a298fd41c38bc20e4f09049c15f9d4de1ddfbb636d0af84ab076bbf2c9c7 Phase One/IQ140/Phase_One_IQ140_sample_files_04_Diciembre_2017_005_IIQ_S.TIF'
  '3fb74d1fc402daaa48a9c3f1d6279d7e85fb58dfdaf816f036927d49ab228977 Leica/M Monochrom (Typ 246)/M2462362.DNG'
  '3e61f54fb982e00e384ac88ca64c8ad4049887cc2c4fb055114b648fd5e45815 Plustek/OpticFilm 8100/SF8_processed_-_Mon_Jun_17_19-45-39_2019.dng'
)

for entry in "${FILES[@]}"; do
  sha=${entry%% *}
  rel=${entry#* }
  out="$OUT/$rel"
  url="https://raw.pixls.us/data/$(python3 -I -c 'import sys,urllib.parse;print(urllib.parse.quote(sys.argv[1]))' "$rel")"
  mkdir -p "$(dirname "$out")"
  if [[ ! -s $out ]]; then
    check_space
    curl -fsSL --retry 3 -A 'specular-corpus-fetch/1 (test corpus)' -o "$out.part" "$url" ||
      die "download failed: $url"
    mv "$out.part" "$out"
  fi
  echo "$sha  $out" | sha256sum -c --quiet - || die "checksum mismatch: $rel"
  printf '%s\t%s\t%s\n' "$rel" "$url" 'CC0-1.0 (raw.pixls.us)' >> "$OUT/.provenance.tsv"
done
echo "fetch-rawrest: $(du -sh "$OUT" | cut -f1) in $OUT"
