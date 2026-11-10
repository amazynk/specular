#!/bin/sh
# Sound files without a picture (tests/soundfiles.rs), made with ffmpeg 8.0 (libmp3lame,
# libvorbis, libopus) and tagged by ffmpeg or, where it can't (Ogg covers, ID3v2.2),
# by mutagen. Oracles:
#   NAME.pcm   ffmpeg's decode, s16le at the stream's rate and channels (MP3 gapless-trimmed
#              as ffmpeg does)
#   NAME.tags  what mutagen reads, mapped to Specular's keys (key=value lines, cover=MIME WxH)
# Run from this directory with the dev venv's Python (which has mutagen):
#   PY=/path/to/venv/bin/python sh gen_fixtures_soundfiles.sh
set -e
PY=${PY:-python3}
q="-nostdin -loglevel error -y"
D=0.3
# A chord with a little noise, so every coder has work in every band.
src() {
    echo "-f lavfi -i aevalsrc=0.3*sin(2*PI*440*t)+0.2*sin(2*PI*1250*t)+0.05*random(0)|0.3*sin(2*PI*660*t)+0.1*sin(2*PI*3300*t)+0.05*random(1):s=$1:d=$D"
}
mono() {
    echo "-f lavfi -i aevalsrc=0.4*sin(2*PI*523*t)+0.1*sin(2*PI*5000*t)+0.03*random(0):s=$1:d=$D"
}
pcm() { ffmpeg $q -i "$1" -map 0:a:0 -f s16le "${1%.*}.pcm"; }
tags="-metadata title=Tone -metadata artist=Specular -metadata album=Fixtures -metadata date=2026 -metadata track=3/12 -metadata genre=Ambient -metadata comment=generated"
# Covers: a 64x48 JPEG and a 40x40 PNG.
ffmpeg $q -f lavfi -i testsrc2=size=64x48:rate=1:duration=1 -frames:v 1 cover.jpg
ffmpeg $q -f lavfi -i testsrc=size=40x40:rate=1:duration=1 -frames:v 1 cover.png
cov="-map 0:a -map 1:v -c:v copy -disposition:v attached_pic"

# MP3: ID3v2.4 with a JPEG cover and an ID3v1 tag, CBR.
ffmpeg $q $(src 44100) -i cover.jpg $cov -c:a libmp3lame -b:a 128k $tags -write_id3v1 1 snd-mp3.mp3 && pcm snd-mp3.mp3
# MP3: VBR (Xing/LAME frame), ID3v2.3 with UTF-16 text and a PNG cover, mono 22.05 kHz.
ffmpeg $q $(mono 22050) -i cover.png $cov -c:a libmp3lame -q:a 4 -id3v2_version 3 \
    -metadata title="Été à Zürich" -metadata artist="Ñandú" snd-vbr.mp3 && pcm snd-vbr.mp3
# MP2 mono 32 kHz, untagged.
ffmpeg $q $(mono 32000) -c:a mp2 -b:a 96k -f mp2 snd-mp2.mp2 && pcm snd-mp2.mp2
# ADTS AAC-LC, with an ID3v2 tag in front (as taggers write).
ffmpeg $q $(src 44100) -c:a aac -b:a 96k -f adts snd-aac.aac && pcm snd-aac.aac
$PY -I - <<'EOF'
from mutagen.id3 import ID3, TIT2
t = ID3(); t.add(TIT2(encoding=3, text="ADTS")); t.save("snd-aac.aac")
EOF
# FLAC 16-bit stereo with Vorbis comments and a PNG cover; 24-bit mono 96 kHz.
ffmpeg $q $(src 44100) -i cover.png $cov -c:a flac $tags snd-flac.flac && pcm snd-flac.flac
ffmpeg $q $(mono 96000) -c:a flac -sample_fmt s32 -bits_per_raw_sample 24 snd-flac24.flac && pcm snd-flac24.flac
# WAVE: 16-bit stereo with LIST/INFO, float mono, 24-bit 5.1 (WAVE_FORMAT_EXTENSIBLE),
# G.711 µ-law, IMA ADPCM.
ffmpeg $q $(src 48000) -c:a pcm_s16le $tags snd-wav.wav && pcm snd-wav.wav
ffmpeg $q $(mono 44100) -c:a pcm_f32le snd-f32.wav && pcm snd-f32.wav
ffmpeg $q -f lavfi -i "aevalsrc=0.3*sin(2*PI*300*t)|0.3*sin(2*PI*500*t)|0.2*sin(2*PI*700*t)|0.1*sin(2*PI*60*t)|0.2*sin(2*PI*900*t)|0.2*sin(2*PI*1100*t):s=48000:d=0.1" \
    -c:a pcm_s24le snd-51.wav && pcm snd-51.wav
ffmpeg $q $(mono 8000) -c:a pcm_mulaw snd-ulaw.wav && pcm snd-ulaw.wav
ffmpeg $q $(src 22050) -c:a adpcm_ima_wav snd-ima.wav && pcm snd-ima.wav
# AIFF 16-bit stereo with an ID3 chunk; AIFF-C `sowt`.
ffmpeg $q $(src 44100) -c:a pcm_s16be -write_id3v2 1 $tags snd-aiff.aiff && pcm snd-aiff.aiff
ffmpeg $q $(mono 48000) -c:a pcm_s16le snd-sowt.aifc && pcm snd-sowt.aifc
# AC-3 and E-AC-3 stereo 48 kHz; DTS core stereo 48 kHz.
ffmpeg $q $(src 48000) -c:a ac3 -b:a 192k snd-ac3.ac3 && pcm snd-ac3.ac3
ffmpeg $q $(src 48000) -c:a eac3 -b:a 192k snd-eac3.eac3 && pcm snd-eac3.eac3
ffmpeg $q $(src 48000) -strict -2 -c:a dca -b:a 768k snd-dts.dts && pcm snd-dts.dts
# M4A: AAC with iTunes items and a JPEG cover; ALAC.
ffmpeg $q $(src 44100) -i cover.jpg $cov -c:a aac -b:a 96k $tags -metadata disc=1/2 -f ipod snd-m4a.m4a && pcm snd-m4a.m4a
ffmpeg $q $(src 44100) -c:a alac -f ipod snd-alac.m4a && pcm snd-alac.m4a
# Ogg Vorbis and Opus with comments; a cover in Opus by METADATA_BLOCK_PICTURE (mutagen).
ffmpeg $q $(src 44100) -c:a libvorbis -q:a 3 $tags snd-vorbis.ogg && pcm snd-vorbis.ogg
ffmpeg $q $(src 48000) -c:a libopus -b:a 64k $tags snd-opus.opus && pcm snd-opus.opus
$PY -I - <<'EOF'
import base64
from mutagen.oggopus import OggOpus
from mutagen.flac import Picture
p = Picture(); p.type = 3; p.mime = "image/jpeg"; p.width = 64; p.height = 48
p.data = open("cover.jpg", "rb").read()
f = OggOpus("snd-opus.opus")
f["METADATA_BLOCK_PICTURE"] = [base64.b64encode(p.write()).decode()]
f.save()
EOF
# Matroska sound: FLAC.
ffmpeg $q $(src 44100) -c:a flac -metadata title=Matroska snd-mka.mka && pcm snd-mka.mka
rm -f cover.jpg cover.png

# The tag oracle: what mutagen reads, in Specular's keys.
$PY -I - snd-*.mp3 snd-*.mp2 snd-*.aac snd-*.flac snd-*.wav snd-*.aiff snd-*.aifc snd-*.m4a snd-*.ogg snd-*.opus <<'EOF'
import sys, mutagen
from mutagen.id3 import ID3
ID3_KEYS = {"TIT2": "title", "TPE1": "artist", "TALB": "album", "TPE2": "album-artist",
            "TCOM": "composer", "TCON": "genre", "TRCK": "track", "TPOS": "disc",
            "TCOP": "copyright", "TSSE": "encoder", "TDRC": "date", "TYER": "date"}
VC_KEYS = {"title": "title", "artist": "artist", "album": "album", "albumartist": "album-artist",
           "composer": "composer", "genre": "genre", "date": "date", "comment": "comment",
           "description": "comment", "copyright": "copyright", "encoder": "encoder"}
MP4_KEYS = {"\xa9nam": "title", "\xa9ART": "artist", "\xa9alb": "album", "aART": "album-artist",
            "\xa9wrt": "composer", "\xa9gen": "genre", "\xa9day": "date", "\xa9cmt": "comment",
            "cprt": "copyright", "\xa9too": "encoder"}
def img_size(b):
    if b[:2] == b"\xff\xd8":
        i = 2
        while i < len(b):
            m, n = b[i + 1], int.from_bytes(b[i + 2:i + 4], "big")
            if m in (0xC0, 0xC1, 0xC2):
                return "image/jpeg", int.from_bytes(b[i + 7:i + 9], "big"), int.from_bytes(b[i + 5:i + 7], "big")
            i += 2 + n
    if b[:4] == b"\x89PNG":
        return "image/png", int.from_bytes(b[16:20], "big"), int.from_bytes(b[20:24], "big")
    return "?", 0, 0
for name in sys.argv[1:]:
    out = {}
    cover = None
    try:
        f = mutagen.File(name)
        tags = getattr(f, "tags", None) if f is not None else None
    except mutagen.MutagenError:
        # ADTS with an ID3 tag: mutagen reads the tag alone.
        f = None
        try:
            tags = ID3(name)
        except mutagen.MutagenError:
            tags = None
    if isinstance(tags, ID3):
        for fr in tags.values():
            k = ID3_KEYS.get(fr.FrameID)
            if k and k not in out:
                out[k] = "; ".join(str(x) for x in fr.text)
            if fr.FrameID == "COMM" and fr.desc in ("", "comment", "ID3v1 Comment") and "comment" not in out:
                out["comment"] = "; ".join(fr.text)
            if fr.FrameID == "APIC" and cover is None:
                cover = fr.data
    elif tags is not None and hasattr(tags, "items") and name.endswith(".m4a"):
        for k, v in tags.items():
            if k in MP4_KEYS:
                out[MP4_KEYS[k]] = "; ".join(v)
            elif k in ("trkn", "disk"):
                n, t = v[0]
                out["track" if k == "trkn" else "disc"] = f"{n}/{t}" if t else str(n)
            elif k == "covr":
                cover = bytes(v[0])
    elif tags is not None:
        d = {}
        for k, v in tags:
            d.setdefault(k.lower(), []).append(v)
        for k, v in d.items():
            if k in VC_KEYS and VC_KEYS[k] not in out:
                out[VC_KEYS[k]] = "; ".join(v)
        if "tracknumber" in d:
            n = d["tracknumber"][0]
            t = (d.get("tracktotal") or d.get("totaltracks") or [None])[0]
            out["track"] = f"{n}/{t}" if t and "/" not in n else n
        if "encoder" not in out and getattr(tags, "vendor", None):
            out["encoder"] = tags.vendor
        pics = getattr(f, "pictures", None)
        if pics:
            cover = pics[0].data
        elif "metadata_block_picture" in d:
            import base64
            from mutagen.flac import Picture
            cover = Picture(base64.b64decode(d["metadata_block_picture"][0])).data
    if (name.endswith(".wav") and not out) or name.endswith((".aiff", ".aifc")):
        # mutagen reads no LIST/INFO nor AIFF text chunks: ffprobe for what it lacks.
        import json, subprocess
        j = json.loads(subprocess.run(["ffprobe", "-v", "error", "-show_entries", "format_tags",
                                       "-of", "json", name], capture_output=True).stdout)
        INFO = {"title": "title", "artist": "artist", "album": "album", "date": "date",
                "genre": "genre", "comment": "comment", "track": "track", "encoder": "encoder",
                "copyright": "copyright"}
        for k, v in j.get("format", {}).get("tags", {}).items():
            if k in INFO and INFO[k] not in out:
                out[INFO[k]] = v
    with open(name.rsplit(".", 1)[0] + ".tags", "w") as o:
        for k in sorted(out):
            o.write(f"{k}={out[k]}\n")
        if cover is not None:
            m, w, h = img_size(cover)
            o.write(f"cover={m} {w}x{h}\n")
EOF
