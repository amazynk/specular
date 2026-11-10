#!/usr/bin/env bash
# The JPEG XS corpus in tools/corpus/jpegxs/ (gitignored), checked against the ISO/IEC 21122-5
# reference software (the oracle):
#   synth/        synthetic images encoded by the reference encoder across profiles and
#                 options (Light, Main, High, MLS, Bayer with Star-Tetrix, TDC, NLT, columns…)
#   conformance/  ISO/IEC 21122-4 conformance codestreams (and one 21122-3 .jxs file), about
#                 75 MB taken out of the 1.9 GB tests.zip with HTTP range requests
#   sequences/    the first frames of two TDC sequences, for temporal prediction
#   heif/         every codestream of synth/ and conformance/ as a HEIF file with a jxs1 primary
#                 item (.jxsi), some in two extents or in idat; written by jpegxs-wrap.py
#   mp4/          each sequence as an MP4 movie with a jxsm track (jpegxs-wrap.py); no tool at
#                 hand writes either container, so they are built from the box definitions
# Each directory gets manifest.tsv with the SHA-256 of the reference decoder's samples.
#
# Usage: tools/corpus/fetch-jpegxs.sh [--4k] [--keep-ref]
#   --4k        adds synth/4k-main422-10.jxs (4096×2160 4:2:2 10-bit) for timing
#   --keep-ref  keeps the reference decoder's samples as NAME.ref (≈1 GB) to locate mismatches
#   OUT defaults to tools/corpus/jpegxs; the reference software is built in ORACLE
#   (default $CARGO_TARGET_DIR/jpegxs-oracle, else target/jpegxs-oracle) with cmake and a
#   C compiler.
# Then:
#   SPECULAR_JPEGXS_CORPUS=tools/corpus/jpegxs cargo test -p specular-jpegxs --test oracle -- --nocapture
#   SPECULAR_JPEGXS_CORPUS=tools/corpus/jpegxs cargo test -p specular-video --test jpegxs_mp4 -- --include-ignored --nocapture
#   SPECULAR_JPEGXS_TIMING=tools/corpus/jpegxs/synth/4k-main422-10.jxs cargo test -p specular-jpegxs --test timing -- --nocapture
#
# The reference software's licence (in the zip) allows evaluation and conformance testing; it is
# only built and run here, nothing of it is linked or copied into Specular. Downloads are
# untrusted data: they are only stored and hashed; Python runs isolated (-I).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${OUT:-$ROOT/tools/corpus/jpegxs}
ORACLE=${ORACLE:-${CARGO_TARGET_DIR:-$ROOT/target}/jpegxs-oracle}
PYTHON=${PYTHON:-python3}
MIN_FREE_GB=${MIN_FREE_GB:-8}
REF_URL=https://standards.iso.org/iso-iec/21122/-5/ed-3/en/ISO_IEC_21122-5_ED-3.zip
REF_SHA=36efc26b92a09b7447bc6e580c902d5c19ee9e919b324609768e390d38621835
TESTS_URL=https://standards.iso.org/iso-iec/21122/-4/ed-3/en/tests.zip
BIG=""
KEEP=""
for a in "$@"; do
  case $a in
    --4k) BIG=--4k ;;
    --keep-ref) KEEP=--keep-ref ;;
    *) echo "fetch-jpegxs: unknown option $a" >&2; exit 2 ;;
  esac
done

mkdir -p "$OUT/synth" "$OUT/conformance" "$OUT/sequences" "$ORACLE"
avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
  echo "fetch-jpegxs: less than ${MIN_FREE_GB} GB free; stopping" >&2
  exit 1
fi

# The reference software (ISO/IEC 21122-5 3rd edition).
BIN=$ORACLE/build/bin
if [[ ! -x $BIN/jxs_decoder || ! -x $BIN/jxs_encoder ]]; then
  zip=$ORACLE/ref.zip
  if [[ ! -s $zip ]] || ! echo "$REF_SHA  $zip" | sha256sum -c --quiet - 2>/dev/null; then
    curl -fsSL --retry 3 -o "$zip.part" "$REF_URL"
    echo "$REF_SHA  $zip.part" | sha256sum -c --quiet -
    mv "$zip.part" "$zip"
  fi
  rm -rf "$ORACLE/src" "$ORACLE/build"
  mkdir -p "$ORACLE/src" "$ORACLE/build"
  (cd "$ORACLE/src" && unzip -q "$zip")
  cmake -S "$ORACLE/src" -B "$ORACLE/build" -DCMAKE_BUILD_TYPE=Release >/dev/null
  cmake --build "$ORACLE/build" -j 4 >/dev/null
fi

# Conformance codestreams and sequence frames, pinned by SHA-256.
while read -r dir sha member; do
  echo "$sha $member $OUT/$dir/${member/\//_}"
done <<'PINS' | "$PYTHON" -I "$ROOT/tools/corpus/jpegxs-zip-range.py" many "$TESTS_URL"
conformance 6358dcfe4296740445411ab0fef48bb7e981eb70a5a0f93039bceb54b08f93c3 1.jxs
conformance 6cb1d6fd7b443666d9a277f9238a3500a69a0162169733fab2e249dce1dfb4cc 2.jxc
conformance 1b4c53dfc5c7fd94bd71240d5181a54af69445089f2873185b4f88c18a494721 4.jxc
conformance debb93844d0bf31095434ba6417d7e60307a4155d9bcf8b16110f72f78342912 5.jxc
conformance c819bae0f0c730ac370304c5d3d906690df75f7938841825988528b1e8b72d26 6.jxc
conformance f8538f94bed9600a3ae90c29b8c3511103e84671755aebeeb650f8beb3e25be8 7.jxc
conformance 71e00a71da1c2cf9a46f85995c9befb48c46716e6c0e335f396caf592294d862 8.jxc
conformance 45434e298ef58308e1ee7eb8255d756b6c61285e61a55db822d39802fe01fb87 9.jxc
conformance 6fd179cadff60012868ab5fbc6f01d0cdddb48f4e8eb5ab5d431f6cfe19a612c 12.jxc
conformance 9b495c8c24c3078846e518faab3204af6f10385f64bcc3702e4d9126f1d88efd 36.jxc
conformance a4d1b5b148d0d04de32b93b6e0f1d00429f0ef9458cbe93ba14f52be92b787e4 37.jxc
conformance 69a5a3430d90e33ec3fcb288c39668ad8fe7fcbafaad72ec1d1fa10dd4bf1665 38.jxc
conformance 28ade1f698ac0ce81f145b9f9f9a6b6299537339b5cbf9fd389c721608e3af44 39.jxc
conformance 9ffcedbd46b3acc0a91fbe4a36e7af9b8ec47e4e4ef40c3aa961eb8ff1f2a9db 40.jxc
conformance 370e7b3a8d6c6ab3b3e65589e6211465a494f76ba094a8965a6cfb9bbd70eb1c 41.jxc
conformance 7c83f928cfc2368cbadc7e4766f9ec8b363df6607c420d0c018d2cf94acc99c3 42.jxc
conformance c552b9e7e13bf8d97ccf4748b18a3d88a159362b5f8da0ba1d7f2e7c0c5e9dca 43.jxc
conformance fa9babcecb8bce4eb5f1744d286f31f3968480b23ac18a07de8c7c84ad9d98ae 48.jxc
conformance 0671b2f8eef67e7d120af9674cac6181e5f2aeb1279f1f370a410af9c158713f 49.jxc
conformance 21f5a0949fdaf3741c0210e000945fcf2c1bb97dcc96e6db70e15974773eb95f 66.jxc
conformance 8fc22956043ba274f963050cbc907526db822999b3f6986c734b9ec95a57fddf 67.jxc
conformance 5c910bb206d7116ccc9a465f44e96014e4bae70cdb34222a67a58ee94e407596 200.jxc
conformance a324d9b19edd81c89e1468724c4c4f91faa0c9fdf8d0fd8fa6bc4e01a2775510 201.jxc
conformance c24f794b8bfc5568eb4396c373379d84a3b3d85a3d8e002f755435f40a43eba7 202.jxc
conformance 090ae2cc2242e6e0c5e25d0e45fccc6e4717803dc4182967225ac8756835bf3a 203.jxc
conformance f6c2e279c9d207a26f44ec473aa62698317aa10d4dfdba7a80cf1c310f270c75 204.jxc
conformance 8a148bce837f020846865e4bdf296b50d698b8d040306817faf13485278f0623 205.jxc
conformance 02a793b879edf82b3d1bfd28dd4bdaaf379b6a9660017f5c6a6884ac8d721f30 206.jxc
conformance 3c6cc8d03ea5a7a84023e58b450153832482593c92b514f3d81d0edbbdab91e7 207.jxc
conformance 2d0331d8dd46fdb793b8055ad5598a5bbf0229130604501c1dee48dbda3aeef1 208.jxc
conformance dd7329633bff5a3b0faa2e096e5909605ed54a444c18140c2356dc084cdeac00 209.jxc
conformance 6498b4462b7d218b57826afc4a14e7be322ca8b43839f8d5712709e3ff079e34 210.jxc
conformance 74f7b752e3d220ab6279245211306e61e7d1b7a06f5ffa09e0c964e38b26348c 211.jxc
conformance 568b191f7e34a0fb84e2a5b280c1a6a70c1722f70578a8e5c727e8a11e8b4c09 212.jxc
conformance 62fe137ac58ba65146224fdbf536f0e106fd767067e15e572ed6ed0c705dd289 213.jxc
conformance 78976229631beecd6a47b9d4750e0bddc1bba655013973478d92a8f8c22c27f7 214.jxc
conformance 28c76686efadac3181625325f6d638e4a16cd5982f322645ede3beeb57ce5a45 215.jxc
conformance 3e2314519de0062f5cab828285d68890ac4c9063a7d72e21f7decea7c78cea0d 216.jxc
conformance 0bee98963d20fd1410a5b1e3dcb8f6a23f6137de1f379744f86f5cefc65e0479 217.jxc
conformance 09ad2a6e89755157f2e588119ad88e9a34f39b9ab0394c92ef691dcc75f56587 218.jxc
conformance cf33bb8d2fd516df5ee1d06e463898e702af9ebd9d112ae8d0d5769a14ad0126 301/frame_00000000.jxc
conformance a726112b5fdc34d8cedb71ce9094776c89d1dcdb8e1e303e1a82166248f07b67 302/frame_00000000.jxc
sequences a726112b5fdc34d8cedb71ce9094776c89d1dcdb8e1e303e1a82166248f07b67 302/frame_00000000.jxc
sequences d5d48df43148bbd32c7ac3ba5bc5fca3a3f70eaa4ec8b518da701fb70ac9e9b9 302/frame_00000001.jxc
sequences 440c5ce2d056acf5fd1fb0a7d45126f2f8cb96645226a068a7a10a5745dfc268 302/frame_00000002.jxc
sequences 0fd3e467e75dbbe14c04430ce90643418104d54bda2c7d5153c93aaffcba2ac7 302/frame_00000003.jxc
conformance d5efb402e83e7cb6b6ba5c3deaa1e3d614cc087bafb4764d3d0a6eebcaa7bf6a 303/frame_00000000.jxc
conformance 85940fd0d396c5f69a0128f0885909e5618c4b558a6652f3428a868a601324e4 304/frame_00000000.jxc
conformance a726112b5fdc34d8cedb71ce9094776c89d1dcdb8e1e303e1a82166248f07b67 305/frame_00000000.jxc
conformance 05a3349be113b1cb7920d26ff986987b08c48fc35f1f4c0daf082c13028fbaff 306/frame_0000000b.jxc
conformance 9b378a18b0405da2e84007eb788a45ec2aef0317c7508ec564f8ffe8463cfc02 306/frame_0000000t.jxc
conformance 9f9802f123a992a46d11ec4253596c1e460b51aa2506cea83529edf21ba8dd71 307/frame_00000000.jxc
conformance c168736956f8155f1e9aa343a6acbd587783c4b5c0ad1b1acd8fdfd30446e3b2 308/frame_00000000.jxc
sequences c168736956f8155f1e9aa343a6acbd587783c4b5c0ad1b1acd8fdfd30446e3b2 308/frame_00000000.jxc
sequences 204f9daf16ffcbf1466ea5ed71accad6cefa5cd1da0655560b2d9ca22faa68a4 308/frame_00000001.jxc
sequences 8b4d21156f65abb43c8901db5cb5cedde40590f9871843f5ade81dba8c747b98 308/frame_00000002.jxc
sequences 7722030efa92d09b43d8e12abe5b562844c39720f70f7cfaead082635779b622 308/frame_00000003.jxc
sequences 965779f3fdc49b50e25aab2d3ac823b276bc65f5ff699def9664d80d3c4a61b2 308/frame_00000004.jxc
sequences 5508ed60eeb146bcdbc1d91fad3fb228f94524c45d90f0fba9283487a153a4a1 308/frame_00000005.jxc
sequences c37108d8a96eec9e2a3029e6700effc81ce3d72b98312b29bfa773caa064d628 308/frame_00000006.jxc
sequences cf4cc04c2a866320cfeff7fbe5e31dc84154aa1f1c8d627a0be0e0a3dfa2e73a 308/frame_00000007.jxc
conformance 334aab5a46714b8a1f538712ccbc813ee5f9ccd065263dc9b40bec089b5223aa 309.jxc
conformance cdfa332d713e5f68508b7a2e72d63816db09f0da383d4cabb7306a6119db5341 310.jxc
PINS

"$PYTHON" -I "$ROOT/tools/corpus/jpegxs-oracle.py" synth "$BIN" "$OUT/synth" $BIG $KEEP
"$PYTHON" -I "$ROOT/tools/corpus/jpegxs-oracle.py" decode "$BIN" "$OUT/conformance" $KEEP
"$PYTHON" -I "$ROOT/tools/corpus/jpegxs-oracle.py" sequences "$BIN" "$OUT/sequences" $KEEP
"$PYTHON" -I "$ROOT/tools/corpus/jpegxs-wrap.py" corpus "$OUT"
echo "fetch-jpegxs: corpus in $OUT"
