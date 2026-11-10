#!/usr/bin/env bash
# Fetch the OCR models for the specular-infer tests and the oracle (never committed).
#
# Usage: tools/corpus/fetch-ocr.sh
#   OUT defaults to $SPECULAR_OCR_DIR, else target/ocr. Each PaddleOCR safetensors export goes
#   in a folder named as its Hugging Face repository (model.safetensors, config.json,
#   preprocessor_config.json); Tesseract's heb.traineddata in tessdata_best/.
#
# Files come at pinned commits (the ones crates/specular-ai/manifest/models.toml offers) and
# are checked against their sha256 (Apache-2.0). Downloads are untrusted data: this script
# only copies and hashes them. Resumes a partial download (curl -C -). Stops when free space
# drops below MIN_FREE_GB (default 8).
#
# The PaddleOCR reference pipeline (tools/corpus/ocr-oracle.py paddle) needs the Paddle
# inference exports of the same models (PaddlePaddle/<name> without _safetensors) in
# OUT/paddle/<name>; fetch those with PADDLE=1.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_OCR_DIR:-$ROOT/target/ocr}
MIN_FREE_GB=${MIN_FREE_GB:-8}
mkdir -p "$OUT"

die() { echo "fetch-ocr: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

# get URL DEST SHA256: downloads one file (resuming) and checks its hash.
get() {
  local url=$1 dest=$2 sha=$3
  check_space
  mkdir -p "$(dirname "$OUT/$dest")"
  if [[ -f "$OUT/$dest" ]] && echo "$sha  $OUT/$dest" | sha256sum -c --quiet - 2>/dev/null; then
    echo "  $dest (cached)"
    return
  fi
  echo "  $dest"
  curl -fL --retry 3 -C - -o "$OUT/$dest.part" "$url"
  echo "$sha  $OUT/$dest.part" | sha256sum -c --quiet - || die "$dest: sha256 mismatch"
  mv "$OUT/$dest.part" "$OUT/$dest"
}

# paddle NAME: the Paddle inference export (reference pipeline only), at its current commit.
paddle() {
  local name=$1 sha
  sha=$(curl -fsSL "https://huggingface.co/api/models/PaddlePaddle/$name" | python3 -I -c 'import json,sys; print(json.load(sys.stdin)["sha"])')
  mkdir -p "$OUT/paddle/$name"
  for f in inference.json inference.pdiparams inference.yml; do
    check_space
    [[ -s "$OUT/paddle/$name/$f" ]] || curl -fL --retry 3 -o "$OUT/paddle/$name/$f" \
      "https://huggingface.co/PaddlePaddle/$name/resolve/$sha/$f"
  done
}

get https://huggingface.co/PaddlePaddle/PP-OCRv6_small_det_safetensors/resolve/eae2ee920a39fb3087637d3dbb58df1896ec1f24/model.safetensors \
    PP-OCRv6_small_det_safetensors/model.safetensors 89a96a8adc4e9cd0c994098edc76022e496d35844392562b4694c8fbc583f2da
get https://huggingface.co/PaddlePaddle/PP-OCRv6_small_det_safetensors/resolve/eae2ee920a39fb3087637d3dbb58df1896ec1f24/config.json \
    PP-OCRv6_small_det_safetensors/config.json b42d76ab6325d5234e1afc9987b73dc592b0ec8437c8a29d46ba6209b79252eb
get https://huggingface.co/PaddlePaddle/PP-OCRv6_small_det_safetensors/resolve/eae2ee920a39fb3087637d3dbb58df1896ec1f24/preprocessor_config.json \
    PP-OCRv6_small_det_safetensors/preprocessor_config.json 74421569a28a78d417db320f9bc039ce0997a1defe2b35f259dfc74299c9f1ed
get https://huggingface.co/PaddlePaddle/PP-OCRv6_small_rec_safetensors/resolve/fe049fb103f57443fe8840c54ed06b702f3c1de5/model.safetensors \
    PP-OCRv6_small_rec_safetensors/model.safetensors f65a332afe5aa663f0b9d5706f4ae8457b5b4058a842d5c1eb22df505c27d642
get https://huggingface.co/PaddlePaddle/PP-OCRv6_small_rec_safetensors/resolve/fe049fb103f57443fe8840c54ed06b702f3c1de5/config.json \
    PP-OCRv6_small_rec_safetensors/config.json 8693fd8485e8543e13d0f6dde3891a3f4af9a47fc4e3a391fbed71f8015c899e
get https://huggingface.co/PaddlePaddle/PP-OCRv6_small_rec_safetensors/resolve/fe049fb103f57443fe8840c54ed06b702f3c1de5/preprocessor_config.json \
    PP-OCRv6_small_rec_safetensors/preprocessor_config.json 2b24fa36f548893f26a931cf44f1a1a6b2b14ed7e2f8f8e28e6848801a8278db
get https://huggingface.co/PaddlePaddle/PP-OCRv6_tiny_rec_safetensors/resolve/6f2d2d51b4b4226d7a2329a02f416f4994106f3a/model.safetensors \
    PP-OCRv6_tiny_rec_safetensors/model.safetensors cc3892aba0fbd89afbf6a76d8b7817bb58802668be7a1384ca761ce65612f3f7
get https://huggingface.co/PaddlePaddle/PP-OCRv6_tiny_rec_safetensors/resolve/6f2d2d51b4b4226d7a2329a02f416f4994106f3a/config.json \
    PP-OCRv6_tiny_rec_safetensors/config.json d8a130cc18e1833aa5f352e057b5cd0e254c46efc5709823f73d53a8b5a42969
get https://huggingface.co/PaddlePaddle/PP-OCRv6_tiny_rec_safetensors/resolve/6f2d2d51b4b4226d7a2329a02f416f4994106f3a/preprocessor_config.json \
    PP-OCRv6_tiny_rec_safetensors/preprocessor_config.json 196f42619e2e9e0e93a6dc1a622fa1c664628e1c63ce116b376275b3e192125f
get https://huggingface.co/PaddlePaddle/PP-OCRv5_mobile_det_safetensors/resolve/c5041d225cf951ff06900ab81a3c7d543c45e2ad/model.safetensors \
    PP-OCRv5_mobile_det_safetensors/model.safetensors c49eea87d579f931c79bf29ac1ffb1b3166f86f6811b8e46b982bb66e3f0f287
get https://huggingface.co/PaddlePaddle/PP-OCRv5_mobile_det_safetensors/resolve/c5041d225cf951ff06900ab81a3c7d543c45e2ad/config.json \
    PP-OCRv5_mobile_det_safetensors/config.json f4f66922bfb21ce8be118fee59f8f7b0fddbfed950041d45dd234f92c8862acd
get https://huggingface.co/PaddlePaddle/PP-OCRv5_mobile_det_safetensors/resolve/c5041d225cf951ff06900ab81a3c7d543c45e2ad/preprocessor_config.json \
    PP-OCRv5_mobile_det_safetensors/preprocessor_config.json 0246cb8d114eec1447a9c358a1db636dcb8f315718d9781e0937f70a89a56900
get https://huggingface.co/PaddlePaddle/arabic_PP-OCRv5_mobile_rec_safetensors/resolve/4941f944a2d298f4cefa46c894e7ef129ba84c39/model.safetensors \
    arabic_PP-OCRv5_mobile_rec_safetensors/model.safetensors eb7ed1ed54b54955a03f7d85f6dbc625cc9fddd9c42ff3571d722ffc0ae5f059
get https://huggingface.co/PaddlePaddle/arabic_PP-OCRv5_mobile_rec_safetensors/resolve/4941f944a2d298f4cefa46c894e7ef129ba84c39/config.json \
    arabic_PP-OCRv5_mobile_rec_safetensors/config.json a16ce65d6c7f308446e8190e0709a97190b0978b4f44c9dcac7da6721d94fa79
get https://huggingface.co/PaddlePaddle/arabic_PP-OCRv5_mobile_rec_safetensors/resolve/4941f944a2d298f4cefa46c894e7ef129ba84c39/preprocessor_config.json \
    arabic_PP-OCRv5_mobile_rec_safetensors/preprocessor_config.json 20a387443ab993ff6eec0b178da49834ab8b88e85395764e5bffb8beb11e4151
get https://huggingface.co/PaddlePaddle/korean_PP-OCRv5_mobile_rec_safetensors/resolve/6ef525a357645ce46495c7ed1fef622c4e009e7a/model.safetensors \
    korean_PP-OCRv5_mobile_rec_safetensors/model.safetensors d636a01a66c6efd9f344865fd43111f8d12098131e4d416c6c7488e1022fb162
get https://huggingface.co/PaddlePaddle/korean_PP-OCRv5_mobile_rec_safetensors/resolve/6ef525a357645ce46495c7ed1fef622c4e009e7a/config.json \
    korean_PP-OCRv5_mobile_rec_safetensors/config.json 96481635294a6f65675f454e45367e43e375a559949f7dfddbe5d72cbd343404
get https://huggingface.co/PaddlePaddle/korean_PP-OCRv5_mobile_rec_safetensors/resolve/6ef525a357645ce46495c7ed1fef622c4e009e7a/preprocessor_config.json \
    korean_PP-OCRv5_mobile_rec_safetensors/preprocessor_config.json b2ec2b91bcdbe548bee7aae5952921a36377caececf74705280df82dbb637d3d
get https://huggingface.co/PaddlePaddle/cyrillic_PP-OCRv5_mobile_rec_safetensors/resolve/365e51a40970de46b7f2cba9cfc2fe529d2575cc/model.safetensors \
    cyrillic_PP-OCRv5_mobile_rec_safetensors/model.safetensors 228aa1232b9ed1d8a0d08afacc7496f3f364282b486fc326f5719b2fa047c305
get https://huggingface.co/PaddlePaddle/cyrillic_PP-OCRv5_mobile_rec_safetensors/resolve/365e51a40970de46b7f2cba9cfc2fe529d2575cc/config.json \
    cyrillic_PP-OCRv5_mobile_rec_safetensors/config.json 6d4be67342d159800fa7f3c8d68f251e6e579d6fa2fcc61fc1cefba8c4b5c6bf
get https://huggingface.co/PaddlePaddle/cyrillic_PP-OCRv5_mobile_rec_safetensors/resolve/365e51a40970de46b7f2cba9cfc2fe529d2575cc/preprocessor_config.json \
    cyrillic_PP-OCRv5_mobile_rec_safetensors/preprocessor_config.json b22843f37ade911056a16ae926e062ba53eb53a6e2f395003c4231d2a4dc48a8
get https://huggingface.co/PaddlePaddle/eslav_PP-OCRv5_mobile_rec_safetensors/resolve/feb3757f69257ed5320044604217032a7b90b789/model.safetensors \
    eslav_PP-OCRv5_mobile_rec_safetensors/model.safetensors 10736dfff02329facb09dbbd3477c85d722b8485961c2736ba3f196ac96e70bf
get https://huggingface.co/PaddlePaddle/eslav_PP-OCRv5_mobile_rec_safetensors/resolve/feb3757f69257ed5320044604217032a7b90b789/config.json \
    eslav_PP-OCRv5_mobile_rec_safetensors/config.json ac924b9044d133ce249ab884386acdf0eed0a1d13f14f481d65384a9fae40dc8
get https://huggingface.co/PaddlePaddle/eslav_PP-OCRv5_mobile_rec_safetensors/resolve/feb3757f69257ed5320044604217032a7b90b789/preprocessor_config.json \
    eslav_PP-OCRv5_mobile_rec_safetensors/preprocessor_config.json 967a5b30e5ef946835c1ba984d871702ee5d9f513e4120047bfa588c1ec13a7c
get https://huggingface.co/PaddlePaddle/latin_PP-OCRv5_mobile_rec_safetensors/resolve/47901138f5f32dad06282fa88d2c672fa816d450/model.safetensors \
    latin_PP-OCRv5_mobile_rec_safetensors/model.safetensors bd390b3cdb176e856ac7feab5f26d8dcaed2d7714a1501eb1fce773eccdc126d
get https://huggingface.co/PaddlePaddle/latin_PP-OCRv5_mobile_rec_safetensors/resolve/47901138f5f32dad06282fa88d2c672fa816d450/config.json \
    latin_PP-OCRv5_mobile_rec_safetensors/config.json 5134f6532a92c6d36f8e077deda9d0ea9593a9eb2267f76d34b6bc944d63ac98
get https://huggingface.co/PaddlePaddle/latin_PP-OCRv5_mobile_rec_safetensors/resolve/47901138f5f32dad06282fa88d2c672fa816d450/preprocessor_config.json \
    latin_PP-OCRv5_mobile_rec_safetensors/preprocessor_config.json 048983b9bcb8877f8bb2b6f0fa3ac538fd75e3b801f98594b996a0366d280c93
get https://huggingface.co/PaddlePaddle/el_PP-OCRv5_mobile_rec_safetensors/resolve/0d7a6ed5020d57efffb993b7979328481bb04c3b/model.safetensors \
    el_PP-OCRv5_mobile_rec_safetensors/model.safetensors 4b2d76d32d7b4e7acdd1c9c08bac156a9c2afb4ce9e7c6000c424cca7f116807
get https://huggingface.co/PaddlePaddle/el_PP-OCRv5_mobile_rec_safetensors/resolve/0d7a6ed5020d57efffb993b7979328481bb04c3b/config.json \
    el_PP-OCRv5_mobile_rec_safetensors/config.json b3bc991f5f3df59b7ade54ee6e4bfb9790095b953c099d00335e026f413516ce
get https://huggingface.co/PaddlePaddle/el_PP-OCRv5_mobile_rec_safetensors/resolve/0d7a6ed5020d57efffb993b7979328481bb04c3b/preprocessor_config.json \
    el_PP-OCRv5_mobile_rec_safetensors/preprocessor_config.json ec36342d540e5130d8b02e07fb420ed7d72059ad0d6657669215df7bce7016b3
get https://huggingface.co/PaddlePaddle/th_PP-OCRv5_mobile_rec_safetensors/resolve/4cc4c996b8690bd623683e8d2be06eaf5919584b/model.safetensors \
    th_PP-OCRv5_mobile_rec_safetensors/model.safetensors aead64f31f8772532573ee2452d59050cc6b4902b5d18ba99a2679f3afbf8a23
get https://huggingface.co/PaddlePaddle/th_PP-OCRv5_mobile_rec_safetensors/resolve/4cc4c996b8690bd623683e8d2be06eaf5919584b/config.json \
    th_PP-OCRv5_mobile_rec_safetensors/config.json 09f418d0a4578706aeeec88e26af006f7caf3ec4c336f98ce68deaee24bea41f
get https://huggingface.co/PaddlePaddle/th_PP-OCRv5_mobile_rec_safetensors/resolve/4cc4c996b8690bd623683e8d2be06eaf5919584b/preprocessor_config.json \
    th_PP-OCRv5_mobile_rec_safetensors/preprocessor_config.json 661f968f5d6e1128af11040a8d1d8799cac95ec3bb3c03b5779517fbea1c0231
get https://huggingface.co/PaddlePaddle/devanagari_PP-OCRv5_mobile_rec_safetensors/resolve/4b1f19893a36d00408b1151460aa1e45afec8903/model.safetensors \
    devanagari_PP-OCRv5_mobile_rec_safetensors/model.safetensors 7c27e77aa17d5027d7b2705fda120a63573e01f46139eaa94d04abb0158468f7
get https://huggingface.co/PaddlePaddle/devanagari_PP-OCRv5_mobile_rec_safetensors/resolve/4b1f19893a36d00408b1151460aa1e45afec8903/config.json \
    devanagari_PP-OCRv5_mobile_rec_safetensors/config.json c512021fa79373c2b37df36691644fb8e635b41ae1fe1aab0c1beb9bce146983
get https://huggingface.co/PaddlePaddle/devanagari_PP-OCRv5_mobile_rec_safetensors/resolve/4b1f19893a36d00408b1151460aa1e45afec8903/preprocessor_config.json \
    devanagari_PP-OCRv5_mobile_rec_safetensors/preprocessor_config.json 21afdbbf7babfc1033b985b139cfcb43625392d9fe569a1a5ed8b6118c20bf8b
get https://huggingface.co/PaddlePaddle/ta_PP-OCRv5_mobile_rec_safetensors/resolve/d1a8d42f3938fc52e2c88b7ca1b8cb62856820d1/model.safetensors \
    ta_PP-OCRv5_mobile_rec_safetensors/model.safetensors f4bf5157774957d9cb62686fa56164993d817bcfe2309c2e5587ff79aea6dfb4
get https://huggingface.co/PaddlePaddle/ta_PP-OCRv5_mobile_rec_safetensors/resolve/d1a8d42f3938fc52e2c88b7ca1b8cb62856820d1/config.json \
    ta_PP-OCRv5_mobile_rec_safetensors/config.json 9529c4aa7f67172ffda2328cba42d5c713e10a1af62b11ee975d8a6cd8a2a900
get https://huggingface.co/PaddlePaddle/ta_PP-OCRv5_mobile_rec_safetensors/resolve/d1a8d42f3938fc52e2c88b7ca1b8cb62856820d1/preprocessor_config.json \
    ta_PP-OCRv5_mobile_rec_safetensors/preprocessor_config.json 042baad2fa6d537d0e2261be1dce9d1b5141d9c00977cdecbf4a7a706698a1cd
get https://huggingface.co/PaddlePaddle/te_PP-OCRv5_mobile_rec_safetensors/resolve/1949a6f874f1d73559900d0f531847eb7a7b947a/model.safetensors \
    te_PP-OCRv5_mobile_rec_safetensors/model.safetensors 8981a7984ba6f62f83c19e244cf4123318d777c3d13256d1c20f5b9618f7aa6f
get https://huggingface.co/PaddlePaddle/te_PP-OCRv5_mobile_rec_safetensors/resolve/1949a6f874f1d73559900d0f531847eb7a7b947a/config.json \
    te_PP-OCRv5_mobile_rec_safetensors/config.json d3972cc67ae894e280d39e771239d179bb971c4e9861852387821bd2cf2c4f06
get https://huggingface.co/PaddlePaddle/te_PP-OCRv5_mobile_rec_safetensors/resolve/1949a6f874f1d73559900d0f531847eb7a7b947a/preprocessor_config.json \
    te_PP-OCRv5_mobile_rec_safetensors/preprocessor_config.json 57c7332a47e36f499013b5d4e6b8a66bfda769f5a6d5c1863789b8a387601303
get https://raw.githubusercontent.com/tesseract-ocr/tessdata_best/e2aad9b983032bb1beff9133104a67cdbb87ca4d/heb.traineddata \
    tessdata_best/heb.traineddata dbaa827aea6bc21215638447f17783a1004987c2d0bf5573d111fee397abdae5

if [[ ${PADDLE:-0} == 1 ]]; then
  for n in PP-OCRv6_small_det PP-OCRv6_small_rec PP-OCRv6_tiny_rec arabic_PP-OCRv5_mobile_rec \
    korean_PP-OCRv5_mobile_rec cyrillic_PP-OCRv5_mobile_rec latin_PP-OCRv5_mobile_rec \
    el_PP-OCRv5_mobile_rec th_PP-OCRv5_mobile_rec devanagari_PP-OCRv5_mobile_rec; do
    paddle "$n"
  done
fi
echo "fetch-ocr: done ($OUT)"
