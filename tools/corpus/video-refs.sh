#!/usr/bin/env bash
# Generate the motion-photo / video reference set for crates/specular-video/tests/ into
# tools/corpus/out/video-ref/ (gitignored). The tests skip themselves when it is absent.
#
#   clips/<name>.mp4|.mov   synthetic clips (ffmpeg testsrc2, no third-party content) encoded
#                           with libx264 / libx265: profiles, B-frames and pyramids, multiple
#                           slices, weighted prediction, odd sizes (cropping), rotation, 10-bit,
#                           a QuickTime movie with an Apple content identifier, MBAFF.
#   clips/h264-jvt-*.mp4    interlaced JVT conformance streams (PAFF, field-only, MBAFF, both
#                           mixed) remuxed with one field per sample; h264-jvt-cvpa1-lone-end
#                           ends on an unpaired first field (not in the manifest).
#   src/                    those streams as fetched (SHA-256 pinned), .provenance.tsv: path,
#                           URL, licence.
#   ref/<name>.yuv         libavcodec decode of every frame in presentation order, native
#                           pix_fmt (yuv420p / yuv420p10le), the per-frame oracle.
#   motion/                 stills with the clips attached: Google Motion Photo (Container
#                           directory), MicroVideo (v1), Samsung SEFT trailer, a HEIC with an
#                           `mpvd` box (when heif-enc is installed), and a Live Photo pair (IMG_0001.JPG + IMG_0001.MOV).
#   manifest.tsv            name, codec, width, height, frames, pix_fmt
#
# Usage: tools/corpus/video-refs.sh [--clean]. Needs ffmpeg/ffprobe with libx264 and libx265,
# curl (unless tools/corpus/h264-conformance.sh fetched the JVT streams already),
# python3.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT="$ROOT/tools/corpus/out/video-ref"
MIN_FREE_GB=${MIN_FREE_GB:-8}
for a in "$@"; do
  case $a in
    --clean) rm -rf "$OUT"; echo "removed $OUT"; exit 0 ;;
    *) echo "usage: $0 [--clean]" >&2; exit 2 ;;
  esac
done
for t in ffmpeg ffprobe python3; do
  command -v $t >/dev/null || { echo "video-refs: $t not found" >&2; exit 1; }
done
gb=$(df -Pk "$ROOT" | awk 'NR==2{print int($4/1048576)}')
[ "$gb" -ge "$MIN_FREE_GB" ] || { echo "video-refs: only ${gb} GB free" >&2; exit 1; }
mkdir -p "$OUT/clips" "$OUT/ref" "$OUT/motion"
: > "$OUT/manifest.tsv"

FF=(ffmpeg -hide_banner -loglevel error -y)
# name size frames codec-args... (the container comes from the name's extension)
clip() {
  local name=$1 size=$2 frames=$3; shift 3
  local f="$OUT/clips/$name"
  [ -f "$f" ] || "${FF[@]}" -f lavfi -i "testsrc2=size=$size:rate=30" -frames:v "$frames" -an "$@" "$f"
  local stem=${name%.*}
  local pix
  pix=$(ffprobe -v error -select_streams v:0 -show_entries stream=pix_fmt -of csv=p=0 "$f")
  [ -f "$OUT/ref/$stem.yuv" ] || "${FF[@]}" -i "$f" -f rawvideo -pix_fmt "$pix" "$OUT/ref/$stem.yuv"
  local info
  info=$(ffprobe -v error -select_streams v:0 -count_frames \
    -show_entries stream=codec_name,width,height,nb_read_frames -of csv=p=0 "$f")
  echo -e "$stem\t${info//,/	}\t$pix" >> "$OUT/manifest.tsv"
}

X264=(-c:v libx264 -pix_fmt yuv420p)
X265=(-c:v libx265 -pix_fmt yuv420p -x265-params log-level=error)
clip h264-cbp.mp4 320x240 30 "${X264[@]}" -profile:v baseline -x264-params keyint=15
clip h264-main-bframes.mp4 320x240 45 "${X264[@]}" -profile:v main -bf 3 -x264-params keyint=20:b-pyramid=none
clip h264-high-pyramid.mp4 352x288 60 "${X264[@]}" -profile:v high -bf 3 -x264-params keyint=30:b-pyramid=normal:ref=4:weightp=2
clip h264-high-slices.mp4 640x360 30 "${X264[@]}" -profile:v high -x264-params slices=4:keyint=10:8x8dct=1
clip h264-cavlc-odd.mp4 642x362 20 "${X264[@]}" -profile:v high -x264-params cabac=0:keyint=8
clip h264-1080p.mp4 1920x1080 90 "${X264[@]}" -profile:v high -preset fast -bf 2
# Rotation is a container property: remux a clip with a display matrix (90° clockwise).
[ -f "$OUT/clips/h264-rot90.mp4" ] || "${FF[@]}" -display_rotation -90 \
  -i "$OUT/clips/h264-cbp.mp4" -c copy "$OUT/clips/h264-rot90.mp4"
clip hevc-main.mp4 320x240 45 "${X265[@]}" -tag:v hvc1 -x265-params log-level=error:keyint=20:bframes=3
clip hevc-main-odd.mp4 642x362 30 "${X265[@]}" -tag:v hvc1 -x265-params log-level=error:keyint=30:bframes=4:b-pyramid=1:ref=4
clip hevc-weighted.mp4 320x240 30 "${X265[@]}" -tag:v hvc1 -x265-params log-level=error:keyint=30:weightp=1:weightb=1
clip hevc-main10.mp4 320x240 30 -c:v libx265 -pix_fmt yuv420p10le -tag:v hvc1 -x265-params log-level=error:keyint=15:bframes=2
clip hevc-1080p.mp4 1920x1080 90 "${X265[@]}" -tag:v hvc1 -preset fast
# Software-decoder coverage (specular-hevc inter path): AMP / rect partitions, transform skip,
# sign hiding off, multiple slices per picture, constrained intra prediction, Main 10 with
# B-pyramid and weighted prediction, and a short 4K clip for throughput.
clip hevc-tools.mp4 416x240 30 "${X265[@]}" -tag:v hvc1 -x265-params log-level=error:keyint=30:bframes=3:amp=1:rect=1:tskip=1:signhide=0:ref=3:max-merge=5:tu-inter-depth=3:tu-intra-depth=3:wpp=0
clip hevc-slices.mp4 640x360 24 "${X265[@]}" -tag:v hvc1 -x265-params log-level=error:keyint=12:slices=3:bframes=2:constrained-intra=1
clip hevc-main10-pyramid.mp4 352x288 30 -c:v libx265 -pix_fmt yuv420p10le -tag:v hvc1 -x265-params log-level=error:keyint=30:bframes=4:b-pyramid=1:weightp=1:weightb=1:ref=3
clip hevc-4k.mp4 3840x2160 30 "${X265[@]}" -tag:v hvc1 -preset fast
clip live.mov 480x360 45 "${X265[@]}" -tag:v hvc1 -movflags use_metadata_tags \
  -metadata com.apple.quicktime.content.identifier=5C1F2E8A-0D6B-4C2B-9E7A-3F1D2B6C8E90
# Interlaced H.264 from x264: MBAFF (x264 has no field pictures), CABAC with B-pyramid and
# weighted prediction, CAVLC at an odd width (interlaced x264 needs heights in 4s), bottom
# field first.
XI=(-flags +ildct+ilme)
clip h264-mbaff.mp4 352x288 30 "${X264[@]}" "${XI[@]}" -profile:v high -bf 3 \
  -x264-params interlaced=1:tff=1:keyint=15:b-pyramid=normal:weightp=2:ref=4
clip h264-mbaff-cavlc-odd.mp4 642x364 20 "${X264[@]}" "${XI[@]}" -profile:v main \
  -x264-params interlaced=1:bff=1:cabac=0:keyint=10:bf=2

# Interlaced JVT conformance streams (ITU-T H.264.1, the copy FFmpeg's FATE suite mirrors;
# fetched with pinned SHA-256, see .provenance.tsv), remuxed to MP4 with one field per sample
# where the stream codes fields: field pictures (PAFF), MBAFF frames and both mixed. `ffmpeg
# -c copy` from Annex B leaves composition times equal to decode times (no reordering) behind
# an edit list and gives every sample a frame's duration (fields too, a tick at the end), so
# jvt_mp4 rewrites the timing from libavcodec's own output order: a sample with one field lasts
# half a frame, a frame starts at its first field's time and lasts both, no edit list.
JVT=https://fate-suite.ffmpeg.org/h264-conformance
JVT_LICENCE='ITU-T H.264.1 conformance bitstream (JVT), via the FFmpeg FATE suite'
mkdir -p "$OUT/src"
# jvt_fetch FILE SHA256: the stream into src/ (from tools/corpus/conformance/h264/ when
# h264-conformance.sh has fetched it already).
jvt_fetch() {
  local f=$1 sha=$2 dest="$OUT/src/${1##*/}"
  if [ ! -s "$dest" ]; then
    local local_copy="$ROOT/tools/corpus/conformance/h264/$f"
    if [ -s "$local_copy" ]; then
      cp "$local_copy" "$dest.part"
    else
      curl -fsSL --retry 3 -A 'specular-corpus-fetch/1 (test corpus)' -o "$dest.part" "$JVT/$f"
    fi
    echo "$sha  $dest.part" | sha256sum -c --quiet - || { echo "video-refs: checksum mismatch: $f" >&2; exit 1; }
    mv "$dest.part" "$dest"
    printf '%s\t%s\t%s\n' "src/${f##*/}" "$JVT/$f" "$JVT_LICENCE" >> "$OUT/.provenance.tsv"
  fi
}
# jvt_mp4 SRC MP4 [lone-end]: the Annex B stream SRC remuxed to MP4 with presentation times;
# lone-end: SRC ends on a first field without its pair.
jvt_mp4() {
  local src=$1 dst=$2
  [ -f "$dst" ] && return
  "${FF[@]}" -i "$src" -c copy -an -f mp4 "$dst.part"
  ffprobe -v error -show_entries packet=pos -of csv=p=0 "$src" > "$dst.packets"
  ffprobe -v error -show_entries frame=pkt_pos -of csv=p=0 "$src" > "$dst.frames"
  local rate
  rate=$(ffprobe -v error -select_streams v:0 -show_entries stream=avg_frame_rate -of csv=p=0 "$src")
  python3 - "$dst.part" "$dst.packets" "$dst.frames" "$rate" "${3:-}" <<'EOF'
import struct, sys
from fractions import Fraction
path, packets, frames, rate, lone_end = sys.argv[1:]
lone_end = lone_end == 'lone-end'
pkt = [int(l.split(',')[0]) for l in open(packets) if l.strip()]
out_pos = [int(l.split(',')[0]) for l in open(frames) if l.strip()]
data = bytearray(open(path, 'rb').read())

def boxes(lo, hi):
    p = lo
    while p + 8 <= hi:
        size, kind = struct.unpack('>I4s', data[p:p + 8])
        yield p, size, kind
        p += size

def find(lo, hi, kind):
    return next((b for b in boxes(lo, hi) if b[2] == kind), None)

moov = find(0, len(data), b'moov')
assert moov and moov[0] > find(0, len(data), b'mdat')[0], 'moov must follow mdat'
trak = find(moov[0] + 8, moov[0] + moov[1], b'trak')
edts = find(trak[0] + 8, trak[0] + trak[1], b'edts')
if edts:  # no edit list: every sample is presented
    data[edts[0] + 4:edts[0] + 8] = b'free'
path_boxes = [moov, trak]
mdia = find(trak[0] + 8, trak[0] + trak[1], b'mdia'); path_boxes.append(mdia)
minf = find(mdia[0] + 8, mdia[0] + mdia[1], b'minf'); path_boxes.append(minf)
stbl = find(minf[0] + 8, minf[0] + minf[1], b'stbl'); path_boxes.append(stbl)
lo, hi = stbl[0] + 8, stbl[0] + stbl[1]
assert find(lo, hi, b'ctts') is None
stts = find(lo, hi, b'stts')
n = struct.unpack('>I', data[stts[0] + 12:stts[0] + 16])[0]
durs = []
for k in range(n):
    c, d = struct.unpack('>II', data[stts[0] + 16 + 8 * k:stts[0] + 24 + 8 * k])
    durs += [d] * c
assert len(durs) == len(pkt), (len(durs), len(pkt))
mdhd = find(mdia[0] + 8, mdia[0] + mdia[1], b'mdhd')
v1 = data[mdhd[0] + 8] == 1
timescale = struct.unpack('>I', data[mdhd[0] + (28 if v1 else 20):][:4])[0]
frame = timescale / Fraction(rate)  # libavcodec's frame rate for the raw stream
assert frame.denominator == 1 and frame % 2 == 0, (timescale, rate)
frame = int(frame)
index = {p: i for i, p in enumerate(pkt)}
starts = [index[p] for p in out_pos]  # output order -> the sample starting that frame
is_start = set(starts)
assert len(is_start) == len(starts), 'two frames from one sample'
assert 0 in is_start, 'the first sample makes no frame'
# Each frame is its own sample and the samples after it that start no frame (its second
# field): one sample lasts the frame, two a field each.
span = {}
for i in range(len(pkt)):
    if i in is_start:
        cur = i
        span[cur] = [i]
    else:
        span[cur].append(i)
# libavcodec drops a first field left without its pair at the end: a frame of its own here,
# presented last.
if lone_end:
    lone = len(pkt) - 1
    assert lone not in span, 'libavcodec made a frame of the lone field'
    span[max(span)].remove(lone)
    span[lone] = [lone]
    starts.append(lone)
assert all(len(v) <= 2 for v in span.values()), 'more than two samples for a frame'
durs = [0] * len(pkt)
for v in span.values():
    for i in v:
        durs[i] = frame // len(v)
dts = [sum(durs[:i]) for i in range(len(durs))]
cts = [0] * len(pkt)
t = 0
for s in starts:
    for i in span[s]:
        cts[i] = t + sum(durs[j] for j in span[s] if j < i)
    t += frame


def runs(values):
    out = []
    for v in values:
        if out and out[-1][1] == v:
            out[-1][0] += 1
        else:
            out.append([1, v])
    return out


def table(kind, values):
    r = runs(values)
    return struct.pack('>I4sII', 16 + 8 * len(r), kind, 0, len(r)) + b''.join(
        struct.pack('>II', c, v) for c, v in r)


shift = max(0, max(d - c for c, d in zip(cts, dts)))
new = table(b'stts', durs) + table(b'ctts', [c + shift - d for c, d in zip(cts, dts)])
total = frame * len(starts)
if v1:
    data[mdhd[0] + 32:mdhd[0] + 40] = struct.pack('>Q', total)
else:
    data[mdhd[0] + 24:mdhd[0] + 28] = struct.pack('>I', total)
data[stts[0]:stts[0] + stts[1]] = new
for b in path_boxes:
    data[b[0]:b[0] + 4] = struct.pack('>I', b[1] + len(new) - stts[1])
open(path, 'wb').write(data)
EOF
  rm -f "$dst.packets" "$dst.frames"
  mv "$dst.part" "$dst"
}
# jvt NAME FILE SHA256 [lone-end]: clips/h264-jvt-<name>.mp4 and its reference; with
# lone-end, the stream cut before its last second field that libavcodec reads as a packet of
# its own (a clip ending on an unpaired first field), kept out of the manifest (the tests
# name it).
jvt() {
  local name=$1 f=$2 sha=$3 cut=${4:-}
  jvt_fetch "$f" "$sha"
  local src="$OUT/src/${f##*/}" stem="h264-jvt-$name"
  if [ -n "$cut" ]; then
    stem="$stem-lone-end"
    local s2="$OUT/src/$stem.264"
    if [ ! -f "$s2" ]; then
      ffprobe -v error -show_entries packet=pos -of csv=p=0 "$src" > "$s2.packets"
      ffprobe -v error -show_entries frame=pkt_pos -of csv=p=0 "$src" > "$s2.frames"
      python3 - "$src" "$s2" "$s2.packets" "$s2.frames" <<'EOF'
import sys
src, dst, packets, frames = sys.argv[1:]
pkt = [int(l.split(',')[0]) for l in open(packets) if l.strip()]
out = {int(l.split(',')[0]) for l in open(frames) if l.strip()}
# A packet no frame comes out of is a second field; cut the stream where the last one starts.
cut = max(p for p in pkt if p not in out)
open(dst, 'wb').write(open(src, 'rb').read()[:cut])
EOF
      rm -f "$s2.packets" "$s2.frames"
    fi
    src=$s2
  fi
  local mp4="$OUT/clips/$stem.mp4"
  jvt_mp4 "$src" "$mp4" "$cut"
  local pix
  pix=$(ffprobe -v error -select_streams v:0 -show_entries stream=pix_fmt -of csv=p=0 "$mp4")
  [ -f "$OUT/ref/$stem.yuv" ] || "${FF[@]}" -i "$mp4" -fps_mode passthrough -f rawvideo \
    -pix_fmt "$pix" "$OUT/ref/$stem.yuv"
  [ -n "$cut" ] && return
  local info
  info=$(ffprobe -v error -select_streams v:0 -count_frames \
    -show_entries stream=codec_name,width,height,nb_read_frames -of csv=p=0 "$mp4")
  echo -e "$stem\t${info//,/	}\t$pix" >> "$OUT/manifest.tsv"
}
# Field pictures and frames mixed per picture (PAFF): CAVLC, CABAC, CABAC with weighted
# prediction.
jvt cvpa1 CVPA1_TOSHIBA_B.264 23c2b3299efed3475956b0f7e6a098387ceba1cf3f0b17aff8906daadaf09dc3
jvt capa1 CAPA1_TOSHIBA_B.264 aa102d2b94fe48304d6cafbfe8b0427312853e079083e351470983149c1b8a6c
jvt camp-picaff camp_mot_picaff0_full.26l cdf09622b0879240864f1258dd3ae0a0fe03690028780d4be834f528ecdade83
jvt sharp-paff Sharp_MP_PAFF_1r2.jvt 541df810a033d64a2706d25cc335b80d562041af586f3f622d2d1d79566bd588
# Field pictures only: CAVLC I/P, CABAC with B fields.
jvt cvfi1 CVFI1_Sony_D.jsv dd90b73308db2e3c5cfd4a4e98dac088fd02938cd37ac49deda55dc7b894c2fa
jvt cafi1 CAFI1_SVA_C.264 e29f14280edf5d10ad9857efc43114d829ed692dd156abf61bd68b2ab8f43d37
# MBAFF: CAVLC, CABAC, CABAC with weighted prediction, High with the 8x8 transform.
jvt cvma1 CVMA1_TOSHIBA_B.264 2e5af580fdeb99b2ae7cae616bd75a1c32353590a889f190bd435880711b33ce
jvt cama1 CAMA1_TOSHIBA_B.264 554a13ca140965b1e3401a90a4726dad5dcd374f8978f8b24a96f2430c9365cc
jvt camp-mbaff camp_mot_mbaff0_full.26l 9c6ba659900a635da5a93e0bb68325fdea77db90ec8fcb4b19d9a54cad6e539e
jvt hcamff1 FRext/HCAMFF1_HHI.264 329608bd31d8f56a8250e69b17d2add59faeff86815d135761e227d491d774fc
# PAFF and MBAFF mixed (field pictures between MBAFF frames).
jvt capama3 CAPAMA3_Sand_F.264 a6490cfa63b5a3a8ce7e0a9d64af5dc2b32a1e66d2f0630bbcb182dceeef336c
jvt cvmp-picaff cvmp_mot_picaff0_full_B.26l 872fbc427c37e13c5c9fd49e6094f1cc59a3074226e687d225fa417dedd28ab6
# High with scaling matrices (VA takes them in raster order): PAFF with the default lists,
# field pictures with coded lists, PAFF and MBAFF mixed with lists changing per picture
# (CAVLC, CABAC), and with coded sequence and picture lists over 300 frames.
jvt hcaff1 FRext/HCAFF1_HHI.264 752efafeb176bbf622565e5b3853c82e35bb50143e7175239802576b18e9f22c
jvt freh7 FRext/Freh7_B.264 75495d41eba792d752e54dbf2273a1adeac71ef6fc70d1d6f3332a3b47ffca46
jvt frext2-panasonic FRext/FRExt2_Panasonic.avc ba31155d719727ff4f2cfe16881ba30a298782f012d1d4c45765344354fae3be
jvt frext4-panasonic FRext/FRExt4_Panasonic.avc 80c13db18b18122cd4cb772ab460933a9858a7c32dd887f55df52ac3a9323acb
jvt hpcamapalq FRext/HPCAMAPALQ_BRCM_B.264 173b7e5d6fd6123f033512d0a9e3faf648740b375e3549f80c938018a9e77da0
# A clip ending on a first field without its second field.
jvt cvpa1 CVPA1_TOSHIBA_B.264 23c2b3299efed3475956b0f7e6a098387ceba1cf3f0b17aff8906daadaf09dc3 lone-end

# A still to attach the clips to.
still="$OUT/motion/still.jpg"
[ -f "$still" ] || "${FF[@]}" -f lavfi -i "testsrc2=size=640x480" -frames:v 1 -q:v 3 "$still"
if command -v heif-enc >/dev/null; then
  [ -f "$OUT/motion/still.heic" ] || heif-enc -q 60 "$still" -o "$OUT/motion/still.heic" >/dev/null
fi

python3 - "$OUT" <<'EOF'
import os, struct, sys, shutil
out = sys.argv[1]
m = os.path.join(out, "motion")
still = open(os.path.join(m, "still.jpg"), "rb").read()
clip = open(os.path.join(out, "clips", "h264-main-bframes.mp4"), "rb").read()
hevc = open(os.path.join(out, "clips", "hevc-main.mp4"), "rb").read()

def with_xmp(jpeg, xmp):
    ns = b"http://ns.adobe.com/xap/1.0/\0"
    body = ns + xmp.encode()
    seg = b"\xff\xe1" + struct.pack(">H", len(body) + 2) + body
    return jpeg[:2] + seg + jpeg[2:]

def write(name, data):
    open(os.path.join(m, name), "wb").write(data)

container = ('<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">'
  '<rdf:Description xmlns:GCamera="http://ns.google.com/photos/1.0/camera/" '
  'xmlns:Container="http://ns.google.com/photos/1.0/container/" '
  'xmlns:Item="http://ns.google.com/photos/1.0/container/item/" '
  'GCamera:MotionPhoto="1" GCamera:MotionPhotoVersion="1" GCamera:MotionPhotoPresentationTimestampUs="500000">'
  '<Container:Directory><rdf:Seq>'
  '<rdf:li rdf:parseType="Resource"><Container:Item Item:Mime="image/jpeg" Item:Semantic="Primary" Item:Length="0" Item:Padding="0"/></rdf:li>'
  '<rdf:li rdf:parseType="Resource"><Container:Item Item:Mime="video/mp4" Item:Semantic="MotionPhoto" Item:Length="%d" Item:Padding="0"/></rdf:li>'
  '</rdf:Seq></Container:Directory></rdf:Description></rdf:RDF></x:xmpmeta>')
write("PXL_motion.MP.jpg", with_xmp(still, container % len(clip)) + clip)
write("PXL_motion_hevc.MP.jpg", with_xmp(still, container % len(hevc)) + hevc)

micro = ('<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">'
  '<rdf:Description xmlns:GCamera="http://ns.google.com/photos/1.0/camera/" GCamera:MicroVideo="1" '
  'GCamera:MicroVideoVersion="1" GCamera:MicroVideoOffset="%d" GCamera:MicroVideoPresentationTimestampUs="0"/>'
  '</rdf:RDF></x:xmpmeta>')
write("MVIMG_micro.jpg", with_xmp(still, micro % len(clip)) + clip)

# Samsung: the clip after a "MotionPhoto_Data" marker, then the SEFH directory and SEFT.
name = b"MotionPhoto_Data"
block = struct.pack("<HHI", 0, 0x0a30, len(name)) + name + hevc
body = still + block
dir_start = len(body)
entry = struct.pack("<HHII", 0, 0x0a30, dir_start - len(still), len(block))
sefh = b"SEFH" + struct.pack("<II", 106, 1) + entry
trailer = sefh + struct.pack("<I", len(sefh)) + b"SEFT"
write("SM_motion.jpg", body + trailer)

# HEIC motion photo: the clip in a top-level `mpvd` box after the still's boxes.
heic = os.path.join(m, "still.heic")
if os.path.exists(heic):
    h = open(heic, "rb").read()
    write("PXL_heic.MP.heic", h + struct.pack(">I", len(hevc) + 8) + b"mpvd" + hevc)

# Live Photo pair.
write("IMG_0001.JPG", still)
shutil.copy(os.path.join(out, "clips", "live.mov"), os.path.join(m, "IMG_0001.MOV"))
EOF
echo "video-refs: $(wc -l < "$OUT/manifest.tsv") clips in $OUT"
