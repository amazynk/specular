#!/usr/bin/env bash
# Video fidelity corpus (video decoders) for crates/specular-video/tests/video_fidelity.rs, into
# $SPECULAR_VIDEO_FIDELITY_CORPUS (default ${CARGO_TARGET_DIR:-target}/corpus/video-fidelity; never
# committed). About 60 MB kept; up to ~110 MB of zips pass through $OUT/.dl and are deleted.
#
#   <codec>/gen/    option matrices made here with FFmpeg's encoders (libx264, libx265,
#                   libaom, libvpx, mpeg2video, mpeg4, prores_ks, mjpeg, dvvideo, libtheora):
#                   chroma formats, bit depths, primaries/transfer/matrix, full and limited
#                   range, chroma siting, odd sizes, pixel aspect, display-matrix rotation
#                   (tools/corpus/video-fidelity-oracle.py gen)
#   h264/jvt/       ITU-T JVT conformance streams remuxed to MP4; NAME.mp4.md5 holds the
#                   JM reference decoder's per-frame MD5s from the zip's YUV
#   hevc/jctvc/     JCT-VC HEVC_v1 and RExt streams; NAME.mp4.whole holds HM's MD5 of the
#                   whole decoded YUV
#   vp9/libvpx/, vp8/libvpx/   libvpx test vectors with their per-frame .md5
#   jxl/conformance/  libjxl's cmyk_layers case and its sRGB reference (JPEG XL CMYK)
#   vc1/fate/, theora/fate/, mpeg2/fate/, mpeg4/fate/, dv/fate/, mjpeg/fate/   FFmpeg FATE
#                   and samples.ffmpeg.org files (the pins the video decoders' scripts use);
#                   elementary streams stay under src/ and are remuxed (or copied as .m2v)
#
# Every download is SHA-256 pinned. Needs curl, sha256sum, ffmpeg/ffprobe (with the encoders
# above), python3 with numpy.
#
# Usage: tools/corpus/fetch-video-fidelity.sh [--clean]
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_VIDEO_FIDELITY_CORPUS:-${CARGO_TARGET_DIR:-$ROOT/target}/corpus/video-fidelity}
PY=(python3 -I "$ROOT/tools/corpus/video-fidelity-oracle.py")
FF=(ffmpeg -hide_banner -loglevel error -nostdin -y)
MIN_FREE_GB=${MIN_FREE_GB:-8}
case ${1:-} in
  --clean) rm -rf "$OUT"; echo "removed $OUT"; exit 0 ;;
  "") ;;
  *) echo "usage: $0 [--clean]" >&2; exit 2 ;;
esac
for t in curl sha256sum ffmpeg ffprobe python3; do
  command -v $t >/dev/null || { echo "fetch-video-fidelity: $t not found" >&2; exit 1; }
done
mkdir -p "$OUT/.dl"
gb=$(df -Pk "$OUT" | awk 'NR==2{print int($4/1048576)}')
[ "$gb" -ge "$MIN_FREE_GB" ] || { echo "fetch-video-fidelity: only ${gb} GB free" >&2; exit 1; }

UA='specular-corpus-fetch/1 (test corpus)'
# get URL DEST SHA256
get() {
  local url=$1 dest=$2 want=$3
  if [ ! -s "$dest" ] || ! echo "$want  $dest" | sha256sum -c --status; then
    mkdir -p "$(dirname "$dest")"
    curl -fsSL --retry 3 -A "$UA" -o "$dest.part" "$url"
    mv "$dest.part" "$dest"
  fi
  echo "$want  $dest" | sha256sum -c --status || { echo "fetch-video-fidelity: $url hash mismatch" >&2; exit 1; }
}

"${PY[@]}" gen "$OUT"

JVT=https://www.itu.int/wftp3/av-arch/jvt-site/draft_conformance
HEVC=https://www.itu.int/wftp3/av-arch/jctvc-site/bitstream_exchange/draft_conformance
ZIPS=(
  "h264/jvt|$JVT/AVCv1/CVFC1_Sony_C.zip|99aa2f1501cd1ca8af1a292bb1f1f1cb49e6189a3131bc5bc8168b3dd322905a"
  "h264/jvt|$JVT/AVCv1/CVPA1_TOSHIBA_B.zip|6f349eb8fae64ed2c8e00ba8b8e194ef32b1ebc498f47c348687b6ac7da3c9b3"
  "h264/jvt|$JVT/AVCv1/CAMA1_TOSHIBA_B.zip|081f1bdd1d38238d55d0e4d2373df8641eef0db3290d4cacefb1d6a5d991b0c6"
  "h264/jvt|$JVT/FRExt/FRExt1_Panasonic_D.zip|832ea7fae6fe6296c3f0377585a24cb3bf73fbfb51a3adbd852cf571a95ae16d"
  "h264/jvt|$JVT/FRExt/HCAFF1_HHI_B.zip|70b26364f98a21fddfe1daca09549985e3309d3c2146c3d75575587e9e60582d"
  "h264/jvt|$JVT/FRExt/HPCAMOLQ_BRCM_B.zip|5ca9d2b2b9f444821e97070461aa716e29d1c7090826bb61031b3c733ddd2010"
  "h264/jvt|$JVT/FRExt/Hi422FR1_SONY_A.zip|00160408cfcb21e3636d47d1f17833cf1ebe7a4b8dc2c8fadef897f509fc1d81"
  "h264/jvt|$JVT/Professional_profiles/PPH10I1_Panasonic_A.zip|5bc720fd8e152fa9c1e10645b3bfddb0e32c74dad68bb6145922353a60f1beb0"
  "h264/jvt|$JVT/Professional_profiles/PPH422I1_Panasonic_A.zip|ec62a70d172a7920ef67a61dde2a8b1234fca4b2e85391ad0ec01b7d97677e20"
  "hevc/jctvc|$HEVC/HEVC_v1/AMP_A_Samsung_7.zip|3b8c7e4f77759351893470d87cd5c8252146ae19523786510914a9e8d5c7e340"
  "hevc/jctvc|$HEVC/HEVC_v1/CONFWIN_A_Sony_1.zip|c3c823ffb9e526f5d9506ba2f1239b1f0323572850789230d762f4e4678fc298"
  "hevc/jctvc|$HEVC/HEVC_v1/DBLK_A_MAIN10_VIXS_4.zip|d86ec87f98fccbcec3758954a7cf430e8af8cf021d347dfbbed2f1d5dbb00f25"
  "hevc/jctvc|$HEVC/HEVC_v1/SAO_A_MediaTek_4.zip|e0b5c8f4ab4fb592971f06469db630d9e7ef72b4d37d729604e2fdaa9df706c9"
  "hevc/jctvc|$HEVC/HEVC_v1/WP_A_MAIN10_Toshiba_3.zip|26b3bc442265222032c1960cc02fe2bd55121fe933e70e5039521785073d33b2"
  "hevc/jctvc|$HEVC/RExt/Bitdepth_A_RExt_Sony_1.zip|34967e2047d59b0f3e96e7501712a056c01f369501d812b170c717ce1d417368"
  "hevc/jctvc|$HEVC/RExt/GENERAL_8b_400_RExt_Sony_1.zip|64d8eda9ecd8a86acbd9d91fe6e83927a8a155e8d13c891770f1d0d2a1792980"
  "hevc/jctvc|$HEVC/RExt/GENERAL_8b_444_RExt_Sony_2.zip|dd810f2b7e47e2dd4f516599b2a366084ad47f1af01fe2763caa9ab9ca1233c0"
  "hevc/jctvc|$HEVC/RExt/GENERAL_10b_422_RExt_Sony_1.zip|c22c21e0dbe5630fbb009b93799eb1caf7f0f7b445135684846194ef9713fe4d"
  "hevc/jctvc|$HEVC/RExt/GENERAL_10b_444_RExt_Sony_2.zip|7a5c47606350bfce116a9f8a9a00afa4101d6eb3317e46d8159eb3d300f03747"
  "hevc/jctvc|$HEVC/RExt/GENERAL_12b_420_RExt_Sony_1.zip|931932aa943957ce18ac693253e8ce8d07ebb1ef9cc32527e06e5cb8c59c3c71"
  "hevc/jctvc|$HEVC/RExt/GENERAL_12b_444_RExt_Sony_2.zip|a8bcb6c4f9006ee6a4ac0476d94d24a7c488ea8c99409c11e3e9d2740d06f212"
  "hevc/jctvc|$HEVC/RExt/Main_422_10_A_RExt_Sony_2.zip|7618ccc9c836eec9a981910f4e0e46dc36936d620755857005cf6f85bbd5cc84"
  "hevc/jctvc|$HEVC/RExt/CCP_8bit_RExt_QCOM_1.zip|e5a0216c23f3816107441366b7743160bd0e6a39ce15b14a9fe1a3ffea38bf88"
  "hevc/jctvc|$HEVC/RExt/ADJUST_IPRED_ANGLE_A_RExt_Mitsubishi_2.zip|8c915b0554265ccd6fb1347b41788006df9e7c66349fdeaa142d953340b12997"
)
for z in "${ZIPS[@]}"; do
  IFS='|' read -r dir url sum <<< "$z"
  name=$(basename "$url" .zip)
  [ -s "$OUT/$dir/$name.mp4" ] && continue
  get "$url" "$OUT/.dl/$name.zip" "$sum"
  # A stream FFmpeg cannot remux (or check) is left out.
  "${PY[@]}" conf "$OUT/.dl/$name.zip" "$OUT/$dir" "$name" 2>/dev/null || echo "fetch-video-fidelity: $name left out (FFmpeg cannot read it)"
  rm -f "$OUT/.dl/$name.zip"
done

VPX=https://storage.googleapis.com/downloads.webmproject.org/test_data/libvpx
VECTORS=(
  "vp9|vp90-2-02-size-66x66.webm|749e4260de357b2704117389cd0a620f73f8229043d214528d1cc7d81524faaa|96cdfc13db1458a93a3cecaf73acc5cee5ec8d5112a0406f7321d88fcee486e0"
  "vp9|vp90-2-02-size-08x10.webm|1b10e6d0fd7a709dc2338940e823f5469421652ea38ba51a248d65450bf75ad4|8abb2559635dea816c62775081f0bed8da098c605886a0123b7188a41735e4bc"
  "vp9|vp91-2-04-yuv444.webm|00cfe11797b6b76e3217862d0a3bc7ef1244aa8481e635d8781160e84c6321a1|b5d93e845e93c33f54607910189ff8940b0278f4dc88f01b55d42901b62d3b0f"
  "vp9|vp91-2-04-yuv422.webm|b5b1c12f6a828715bcdc15e7485c2c43a0afb53488093ed4fadbc02f9bed78bb|93f07cc9cf7ecf9cdf077ad9ac5783c683ded4b26a47b0d12990d1bd95e839ae"
  "vp9|vp91-2-04-yuv440.webm|41dc9dc46a16a73d6d57c143a5bcfe291d8269f8ae66bddf02936271893bce59|ca6a16e7af9aba6298a522ca21e6f145e904c9868ef969bd76b6f448c7e01d31"
  "vp9|vp92-2-20-10bit-yuv420.webm|c4b56b148d5039aa824fde3d4877dbd2604d0de7f77af96f4ba1ade537396a38|e177753ed4f53f3aef499a51f4ce022c80661887c61d8976645349918f7ce897"
  "vp9|vp93-2-20-10bit-yuv444.webm|7f6046b7766c45c0a39aa69d50eb412c1d5e446e468ad47c9405e09ef03856f6|7b07f01ba246c648a5d7404e319e7a0c1108154ebd3f7f1bab5be2cb8a50452e"
  "vp9|vp93-2-20-12bit-yuv422.webm|fb6580101973a240a5f7dd6cbc9afd3e675fef102c3a2678db1c4007152c50ba|557017b6df3f4804a2fd8e8d297331cb09012e62046c7a052ebd2f5d30c7be53"
  "vp9|vp92-2-20-12bit-yuv420.webm|89a9601f4e13c0de213968d54b6cf97fdb91698ada7d2d7a827025b05174de91|7e4b5411bbd8bb6617983bd2b092492656974ab3231cf0c8dd140cce83679cec"
  "vp8|vp80-00-comprehensive-005.ivf|698f865dfdd150cbd0c8010e0c483f84ddcf1bd4450c8233ac31c4c7518bbc83|a971e8ce90105d3081f95732713a8e1a71e5cf3c5935c79a7744ac94a580568b"
  "vp8|vp80-00-comprehensive-016.ivf|f1864da217b83dda5d31cf6bce15b0e1ec58138825f461c0d5aa2b66b2c53320|8e9cef6a30e477fac4e5a4c4e9dbfa9be1d98edaade5e3918a4e566f766f78ce"
  "vp8|vp80-05-sharpness-1438.ivf|db49abcff1ecb2a7792e5671791e4550917255377c7357cd2e39a74ea791b88f|6d9e4d841e8851f99c0b3e63198fa0a756bf06bbcd3bf327b2b2e7b2418fb89c"
)
for v in "${VECTORS[@]}"; do
  IFS='|' read -r codec name sum md5sum <<< "$v"
  get "$VPX/$name" "$OUT/$codec/libvpx/$name" "$sum"
  get "$VPX/$name.md5" "$OUT/$codec/libvpx/$name.md5" "$md5sum"
done

FATE=https://fate-suite.ffmpeg.org
SAMPLES=https://samples.ffmpeg.org
FILES=(
  "vc1/fate/SA00040.vc1|$FATE/vc1/SA00040.vc1|898035e63cfe0ddb1cc2c8749d7dafdfcc1b12fe5dac7e5b65941e6c4e41c860"
  "vc1/fate/SA10091.vc1|$FATE/vc1/SA10091.vc1|c60a0da89873b40d7e436f1c29757c6cc71f3c51f2ac2ea8c6bd023762c94b6c"
  "vc1/fate/SMM0015.rcv|$FATE/vc1/SMM0015.rcv|67baabd762ed35f70cd70c492e83e8744d068a5a567d9b19cc509851bf9f1d44"
  "vc1/fate/sb-adv-q60.avi|$SAMPLES/V-codecs/WMV9/sb_wmv9_samples/sb-adv-q60.avi|2185e8bd57d7481a65b04b57e0989caa5b628202e6046889fe1e6fd41eeedb0b"
  "theora/fate/vp31.avi|$FATE/vp3/vp31.avi|457f17e9ebe46ca7f217f853976199c77c8321c170b40ce8531317137f0cf5c3"
  "theora/fate/offset_test.ogv|$FATE/vp3/offset_test.ogv|83ba8d258cc3a1715be473925faf757469f67d2e29a553cf6d5a521488393f3e"
  "mpeg2/fate/hhi_burst_422_short.bits|$FATE/mpeg2/hhi_burst_422_short.bits|bd7b917cb5cf2871c4a3bdd8e0ac9c973792627ee64f6b0afa7b4e4a78e2ddb1"
  "mpeg2/fate/sony-ct3.bs|$FATE/mpeg2/sony-ct3.bs|fa622b9d9a0fe24dee7eea5ebe653bd3f98d5605a4257d16c11985022eaf2875"
  "mpeg2/fate/tcela-6.bits|$FATE/mpeg2/tcela-6.bits|3b267fd327ce97ace05119022e5eda0a222d9cd65c0e360c46c8a2cd0664049a"
  "mpeg2/fate/mpeg2_field_encoding.ts|$FATE/mpeg2/mpeg2_field_encoding.ts|9688333a145b9e2c2139b9cab6cd4966acd19788fd620b4b31db5c2891547cb8"
  "mpeg4/fate/packed_bframes.avi|$FATE/mpeg4/packed_bframes.avi|450a89377f024ce11d176d2be0fa3756df005aa45294866b383fee18a17cbf48"
  "mpeg4/fate/mpeg4_sstp_dpcm.m4v|$FATE/mpeg4/mpeg4_sstp_dpcm.m4v|c7389f15ceb3381be18ab21c932ef3cda952466bcd2ade824faf36a880ea860e"
  "dv/fate/dvcpro50.avi|$SAMPLES/V-codecs/DV50/dvcpro50.avi|5e01035411030b45bc3e42db5aa804bc647451fd8e6fb848adf7aa6ac0a1f914"
  "dv/fate/shots0000.dv|$SAMPLES/DV-raw/shots0000.dv|eb0ef4218cb28462d44b580bd230337f748f62343db5fde9559f0408128a548d"
  "mjpeg/fate/mjpeg_field_order.avi|$FATE/mjpeg/mjpeg_field_order.avi|ce3e326dc7ce75311b20d85b27ab333986cd27a7f9686887019f40abf8136aaf"
  "mjpeg/fate/mjpa.mov|$SAMPLES/V-codecs/MJPEGs/mjpa-b.mov/mjpa.mov|0820856c1a59414f84393b0a741f1481d8f071330a4a867b61cca1a4895bd8d3"
)
for f in "${FILES[@]}"; do
  IFS='|' read -r rel url sum <<< "$f"
  case $rel in
    *.bits|*.bs)
      # Bare MPEG-2 video, which the app opens as .m2v.
      get "$url" "$OUT/$(dirname "$rel")/src/$(basename "$rel")" "$sum"
      [ -s "$OUT/${rel%.*}.m2v" ] || cp "$OUT/$(dirname "$rel")/src/$(basename "$rel")" "$OUT/${rel%.*}.m2v"
      # And as a programme stream (the I picture has no PTS there).
      [ -s "$OUT/${rel%.*}-ps.mpg" ] || "${FF[@]}" -i "$OUT/${rel%.*}.m2v" -c copy -f vob "$OUT/${rel%.*}-ps.mpg" ;;
    *.vc1|*.rcv|*.m4v)
      # Elementary streams: kept under src/ (skipped by the harness), remuxed to Matroska.
      get "$url" "$OUT/$(dirname "$rel")/src/$(basename "$rel")" "$sum"
      mkv="$OUT/${rel%.*}.mkv"
      [ -s "$mkv" ] || "${PY[@]}" remux "$OUT/$(dirname "$rel")/src/$(basename "$rel")" "$mkv" ;;
    *) get "$url" "$OUT/$rel" "$sum" ;;
  esac
done
# JPEG XL CMYK: libjxl's conformance case cmyk_layers (black and alpha extra channels, an
# embedded CMYK profile) and libjxl's sRGB rendering of it, for specular-decode's jxl_cmyk.
JXLC=https://raw.githubusercontent.com/libjxl/conformance/master/testcases/cmyk_layers
get "$JXLC/input.jxl" "$OUT/jxl/conformance/cmyk_layers.jxl" d732c8836bf1abeadf310d2e07387a32813ed4690d32650c1c25e541b80eed4a
get "$JXLC/ref.png" "$OUT/jxl/conformance/cmyk_layers.png" b1191c6aa010480368c77a21ecd3398fbd587d25c5cdf913bfd1119faecd0ef0
rmdir "$OUT/.dl" 2>/dev/null || true
echo "fetch-video-fidelity: $(find "$OUT" -type f -not -name '.*' | wc -l) files, $(du -sh "$OUT" | cut -f1) in $OUT"
