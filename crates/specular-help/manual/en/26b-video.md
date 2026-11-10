---
id: video
title: Videos
section: modes
contexts: video
keywords: wtv, dvr-ms, media center, tv recording, qcp, qcelp, purevoice, evrc, 3g2, au, snd, caf, w64, wave64, rf64, bw64, voc, 8svx, avr, sphere, sph, ircam, tta, true audio, shorten, shn, dsd, dsf, dff, dsdiff, tracker, module, mod, s3m, xm, it, y4m, yuv4mpeg, cdg, karaoke, cd+g, ffv1, uncompressed, v210, cinepak, quicktime animation, video 1, cineform, cfhd, hap, hap q, dirac, vc-2, flash screen video, flv, ogm, transcript, hard of hearing, playlist, queue, play queue, m3u, m3u8, pls, xspf, repeat, shuffle, next track, album, video, movie, closed captions, cc, generate subtitles, speech to text, transcribe, whisper, lyrics, synced lyrics, lrc, goniometer, phase correlation, stereo, sdh, forced subtitles, audio description, mp4, mov, m4v, 3gp, mkv, webm, mts, m2ts, avchd, ts, ogv, ogg, theora, ivf, jpeg xs, prores, mxf, xdcam, imx, xavc, ac-3, e-ac-3, dolby, dts, amr, amr-wb, awb, wavpack, wv, wvc, monkey's audio, ape, voice memo, vorbis, flac, play, pause, seek, scrub, sound, volume, mute, speed, loop, interlaced, audio, music, mp3, m4a, aac, wav, aiff, opus, oga, mka, waveform, spectrogram, cover, album art, id3, tags, spectrum, analyser, meters, level, peak, true peak, rms, clip, loudness, lufs, lra, r128, ebu, go to time, rotate, flip, zoom, brightness, contrast, saturation, gamma, hue, sound output, output device, speakers, surround, 5.1, 7.1, headphones, downmix, cast, casting, play on, tv, dlna, upnp, smart tv, network speaker, cast subtitles, cast speed, measuring loudness
---
MP4, MOV, M4V, 3GP, MKV, WebM, AVCHD (MTS, M2TS), Ogg (OGV) and IVF videos play in the viewer and show in the gallery with their length.

Compressed MKV, MKA and WebM tracks (header stripping, zlib, bzip2) play; LZO and encrypted ones don't.

Interlaced camcorder video is shown deinterlaced.

JPEG XS broadcast video in MP4 plays when it is 4:2:2 or 4:2:0.

ProRes plays in every profile. ProRes 4444 with alpha shows over the checkerboard.

MXF files from broadcast and cameras play: ProRes, MPEG-2 (XDCAM, IMX, Canon XF), H.264 (XAVC) and uncompressed video.

Windows Media Center TV recordings play: WTV and DVR-MS.
The Info panel shows the programme, episode, description, channel and broadcast date.

A recording with several sound tracks lets you choose one.
Copy-protected recordings don't play: they are encrypted.

FFV1 archive video plays in MKV, AVI and MOV, alpha included.

Uncompressed video plays in AVI, MOV, MKV and MXF.

YUV4MPEG2 (`.y4m`) plays: 4:2:0, 4:2:2, 4:4:4 and greyscale, 8 to 16 bits.

CD+G karaoke graphics (`.cdg`) play, with the song of the same name beside them (`.mp3`, `.ogg`, `.flac`, `.wav` or `.m4a`).

Old QuickTime and AVI codecs play: Animation, Cinepak, Video 1, RLE, Graphics (SMC) and Apple Video.

CineForm plays in MOV and AVI: YUV, RGB, RGBA and RAW. RAW shows without colour correction.

HAP plays in every variant: HAP, HAP Alpha, HAP Q, HAP Q Alpha and HAP R.

VC-2 and Dirac intra video plays in MKV, MOV and Ogg.

Flash Screen Video 1 and 2 play in FLV. OGM files play their video.

Canon Cinema RAW Light (`.crm`) and Apple ProRes RAW (MOV) play as raw video: each frame is developed from the sensor, with the clip's white balance. Their sound is not played.

A raw clip opened in [Develop](develop) develops its first frame.

DNxHD, Indeo, AVC-Intra without parameter sets and other RAW codings can't be played; the Info panel names them.

The frame-step, speed and loop keys of [animations](animation) work here too.

## Playing {#play}
- [[key:play-pause-video]]: play or pause
- [[key:seek-forward]] and [[key:seek-back]]: jump 5 seconds
- [[key:seek-forward-long]] and [[key:seek-back-long]]: jump 10 seconds
- 1 to 9: jump to 10 % to 90 %; 0: the start
- [[key:prev-frame]] and [[key:next-frame]]: one frame back or on
- Drag the bar under the video to scrub.
- Point at the bar to see the time and a small picture there.

## Go to a time {#go-to-time}
- [[key:go-to-time]]: type a time, then Enter. Esc cancels.
- Or click the time on the playback bar, or More › Go to time….
- 1:23:45, 83:20 or 5025: that time from the start.
- 50%: halfway.
- +30 or -1:00: on or back from where it is.

A video longer than 3 minutes starts where you left it. The note "Resumed at …" has Start over.

Settings › Viewing: Resume videos, Forget playback positions…, Play the next video, Play the next sound file, Repeat, Shuffle.

The screen stays awake while a video plays.

The media keys and the system's media controls play, pause and go to the next or previous item.

The controls show the file's cover, else a frame of the video.

::: keys video

Video commands are unavailable for images.

## Full screen {#fullscreen}
- [[key:video-fullscreen]]: play full screen, or go back to the window
- Esc also goes back.
- Also: Full screen in the zoom menu, or the full-screen button on the playback bar.

The movie fills the screen on black. Bars and panels hide.

Move the mouse or press a key to show the controls. They hide after 2 seconds while playing.

Going back restores the window, the zoom and the panels.

Sound files have no picture: [[key:video-fullscreen]] does nothing there.

## Subtitles {#subtitles}
- [[key:cycle-subtitles]]: next subtitles, or none
- The CC button on the playback bar: choose subtitles, open a file, set the delay.

Subtitles show from the video itself and from files beside it with the same name: `Film.srt`, `Film.he.srt`.

Drop a subtitle file on the window to show it with the video.

SRT, WebVTT, ASS, SSA, MicroDVD (SUB), LRC lyrics, TTML, DFXP, SCC captions, Blu-ray (SUP) and DVD (IDX and SUB) subtitles show.

Hebrew and Arabic read right to left.

ASS styling shows; its animations and karaoke show as plain text.

A file in an older character set is detected. If it reads wrong, choose Character set in the menu.

Subtitles in your language show by themselves: Settings › Viewing › Subtitle language.

## Which subtitles start {#subtitle-choice}
Settings › Viewing › Show subtitles:
- Off: none start by themselves.
- Forced only: only forced subtitles (signs and foreign speech).
- Always: subtitles in the subtitle language.

Forced subtitles in the sound's language show in every mode.

Prefer subtitles for the deaf and hard of hearing puts SDH subtitles and captions first.

Files named like `Film.en.forced.srt` or `Film.en.sdh.srt` count as forced or SDH.

## Closed captions {#captions}
Captions in the video are in the CC menu: CC1 to CC4 and Text 1 to 4 for older captions, Service 1 and up for digital ones.

They come from H.264, HEVC and MPEG-2 video and from MOV caption tracks.

They stay off until you choose them, or until you prefer subtitles for the deaf and hard of hearing.

Digital captions show their windows, text, colours and edges. Window backgrounds, fonts and effects are left out.

## Subtitle look {#subtitle-look}
- [[key:subtitle-up]] and [[key:subtitle-down]]: subtitles higher or lower
- [[key:subtitle-bigger]] and [[key:subtitle-smaller]]: text subtitles 10 % larger or smaller
- [[key:subtitle-reset-place]]: usual place and size again
- Subtitle style… in the CC menu: all of these in Settings.

Settings › Viewing has the subtitles' position, size, colour, outline, background box and font.

Changes show on the subtitle on screen at once.

Position moves subtitles at the bottom of the picture. Blu-ray and DVD subtitles in its lower half move too.

Subtitles the file places itself (at a point, at the top or in the middle) stay where the file puts them. Those at the bottom with their own margin move with the rest.

Colours and sizes set in the file stay. Override subtitle styles applies yours to every subtitle.

White in a styled file counts as no colour: your colour shows.

Letters the chosen font lacks, such as Hebrew or Chinese, come from another font.

## Generated subtitles {#generated-subtitles}
- [[key:generate-subtitles]]: subtitles from what is said in the video
- [[key:save-subtitles]]: save them as an SRT or WebVTT file
- The CC menu has both.

Pick the language spoken, or Detect the language, or Translate to English.

Whisper large-v3 turbo can't translate. Use base or small for that.

The track "Generated" fills while you watch. It starts where you are, then reads the rest.

Choose the menu row again to stop. What it made stays; ask again to go on from there.

Closed before it finished? It goes on when you open the video again.

Songs and recordings get a transcript in the lyrics column. Save it as LRC, SRT or WebVTT.

It runs on this computer. It needs the Speech to text model: Settings › Local AI.

Silence is skipped. Music without words gives no subtitles.

Finished subtitles are kept. The video offers them again the next time, even renamed.

Save subtitles… asks where to save. Nothing is written beside the video unasked.

## Subtitle and sound timing {#align}
- [[key:subtitle-earlier]] and [[key:subtitle-later]]: subtitles 50 ms earlier or later
- [[key:subtitle-earlier-fine]] and [[key:subtitle-later-fine]]: 10 ms
- [[key:sound-earlier]] and [[key:sound-later]]: sound 50 ms earlier or later
- [[key:sound-earlier-fine]] and [[key:sound-later-fine]]: 10 ms
- [[key:subtitle-step-prev]] and [[key:subtitle-step-next]]: the previous or next subtitle starts now
- [[key:reset-delays]]: both back to zero

Each video keeps its own delays.

## Sound tracks {#sound-tracks}
- [[key:cycle-sound]]: the next sound track
- The sound button on the playback bar lists the tracks and the sound delay.

Settings › Viewing › Sound language picks a track for you.

Prefer audio description tracks puts a track that describes the picture first. Such tracks are marked "audio description".

## Chapters {#chapters}
- [[key:prev-chapter]] and [[key:next-chapter]]: previous or next chapter
- The chapters button lists them. Marks on the bar show where they start.

## Repeat a part {#ab}
- [[key:ab-loop]]: set the start, then the end; again to stop

## Frames {#frames}
- [[key:save-frame]]: save the frame as a PNG at full size
- [[key:copy-frame]]: copy it

## Mini player {#mini}
- [[key:mini-player]]: a small window on top of the others; again or Esc to go back

The controls show when you point at it.

## Play on a TV or speaker {#cast}
- [[key:cast-menu]], or More › Play on…: the TVs and speakers on your network
- Pick one. The file plays there from where it was here.
- The playback bar then drives that device: play, pause, seek, volume and speed.
- A speed the device does not offer: it plays at normal speed, and says so once.
- Text subtitles you chose go with the file to devices that show them.
- Changing the subtitles or their delay sends them again. The device reloads the file at the same place.
- Blu-ray, DVD and DVB subtitles are pictures. They are not sent: devices take text subtitles only.
- Stop casting, or This device: back here at the same place.
- In the gallery: right-click a movie or sound file › Play on….
- Devices are looked for on every network you are connected to, only while the menu is open. With the network off, nothing is.
- Devices that play files from the network (DLNA) work. The file is not converted: a device that can't play its type says so.
- Only that file (and its subtitles) is shared, at a secret address, only to that device, and only while it plays there.

## Picture {#picture}
- [[key:cycle-aspect]]: as stored, 4:3, 16:9, 1.85:1 or 2.39:1
- [[key:cycle-crop]]: crop black bars to one of those shapes
- [[key:rotate-left]] and [[key:rotate-right]]: turn a quarter turn
- [[key:flip-horizontal]] and [[key:flip-vertical]]: mirror
- The turn is kept for that file. Phone videos come upright by themselves.
- Wheel, pinch or [[key:zoom-in]] and [[key:zoom-out]]: zoom while it plays. Drag to pan.
- [[key:zoom-fit]] or a double click in full screen: fit again.
- Subtitles and the playback bar stay in place.
- [[key:video-picture]]: brightness, contrast, saturation, gamma and hue.
- Also in More › Picture on the playback bar. Click a value to reset it.
- Picture changes last until Specular closes. The file is not changed.

## Sound {#sound}
- [[key:volume-up]] and [[key:volume-down]]: louder or quieter, up to 100 %
- [[key:mute]]: mute or unmute
- The slider beside the mute button: click, drag or turn the wheel over it

Videos play at half to twice their speed.

Click the speed on the playback bar to choose 0.5× to 2×.

The sound plays at every speed, at its normal pitch.

Without a sound device, videos still play, silently.

Sound in AAC, HE-AAC, AC-3, E-AC-3, DTS, AMR, AMR-WB, Opus, Vorbis, Speex, FLAC, MP2, MP3, Nellymoser, Flash ADPCM and PCM plays, including 20 and 24-bit DVD sound.

The older AAC profiles, Main and LTP, play.

AAC with 960-sample frames (DAB+ radio) plays; with HE-AAC it stays silent.

Musepack sound in MKV and MKA files plays.

Opus ambisonics (360° sound) plays as plain stereo, without head tracking.

WMA, Dolby TrueHD, AC-4 and RealAudio stay silent; the Info panel names them.

DTS plays its core, up to 5.1.
DTS that uses ADPCM prediction or high-frequency VQ stays silent: the published standard leaves out their code books.

5.1 and 7.1 sound plays to each speaker when the device has them. Otherwise it is mixed down to stereo ([Sound output](#sound-output)).

AC-3 and E-AC-3 (DVDs, camcorders, TV) keep the movie's own mix and dialogue levels.

AMR and AMR-WB (phone videos and voice recordings) play with their comfort noise; lost frames are concealed.

QCELP (PureVoice) and EVRC speech from CDMA phones plays: `.qcp` files, 3G2 videos and QuickTime movies.
SMV and QCELP-8 speech stays silent; the Info panel names them.

## Stream quality {#stream-quality}
A network stream with several qualities starts with the one its speed allows.

- [[key:stream-quality]]: automatic, then each quality from the lowest
- A change applies from the next part of the stream.

## Sound output {#sound-output}
The sound button's menu › Output lists the sound devices and the speakers. Also in Settings › Viewing.

System default follows the system's choice when it changes.

If the chosen device goes, sound moves to the default. It comes back when the device does.

A change while playing goes on at the same place, without a click.

Speakers:
- Automatic: as many as the device says it has.
- Stereo, 5.1 or 7.1: as you set.
- On Linux the system's mixer hides the speakers: choose 5.1 or 7.1.
- 7.1 sound on 5.1 speakers puts the back pair into the surrounds.

Headphones: choose Stereo. Surround is then mixed down for two ears.

Info shows the device, its rate and channels, and whether the sound was resampled or downmixed.

## Levelling, dialogue and equaliser {#sound-effects}
- [[key:dialogue-boost]]: dialogue boost on or off
- [[key:equaliser]]: open or close the equaliser

The sound button's menu has both, and Mono.

Volume levelling plays every file equally loud, from its ReplayGain, R128 (Opus) or Sound Check tags.

Automatic uses the album's level when an album plays in order.

Settings › Viewing can measure files without tags while they play.

Dialogue boost makes speech clearer and loud scenes quieter. It lifts the centre channel of 5.1 and 7.1.

AC-3 and E-AC-3 use their own range control instead.

The equaliser has ten bands from 31 Hz to 16 kHz, ±12 dB each.

Pick a preset, or move the bands and save your own. Boosts take headroom first, so they don't distort.

Balance and Mono are in Settings › Viewing. Mono plays both channels in each ear.

## Sound files {#sound-files}
MP3, MP2, AAC, M4A, FLAC, WAV, AIFF, Ogg Vorbis, Opus, MKA, AC-3, DTS, AMR (`.amr`, `.awb`), WavPack (`.wv`) and Monkey's Audio (`.ape`) files open like videos.

Also: Sun AU (`.au`, `.snd`), Apple CAF, Wave64 (`.w64`), RF64 and BW64, Creative Voice (`.voc`), Amiga 8SVX, AVR, NIST SPHERE (`.sph`), IRCAM (`.sf`), TTA, Shorten (`.shn`) and DSD (`.dsf`, `.dff`).

DSD plays as 88.2 or 96 kHz PCM. DST-compressed `.dff` files stay silent.

Tracker modules (MOD, S3M, XM, IT) play through Specular's own player, as 48 kHz stereo.
A `.mod` from a camcorder is a video; Specular tells them apart by their content.

TAK, OptimFROG, Bink and Smacker files don't open: they have no open description to build a player from.

A WavPack hybrid file plays lossless when its correction file (`.wvc`, same name) is beside it, else lossy.

Speech files open too: GSM (`.gsm`, and GSM in WAV), HTK (`.htk`) and signed linear (`.sln`, `.sln16` and the like).
HTK feature files (MFCC and others) are not sound; the Info panel says so.

MIDI files (`.mid`, `.midi`, `.kar`, `.rmi`) play through a SoundFont: one named like the song or in its folder, else one installed on the system.
Without one they play with simple built-in sounds.

Instrument files (SoundFont `.sf2`, DLS `.dls`, SFZ `.sfz`, FastTracker `.xi`) play their samples one after another.
The Info panel lists the instruments and samples.
An SFZ file needs its sample files beside it.

Speech files open too: GSM (`.gsm`, and GSM in WAV), HTK (`.htk`) and signed linear (`.sln`, `.sln16` and the like).
HTK feature files (MFCC and others) are not sound; the Info panel says so.

MIDI files (`.mid`, `.midi`, `.kar`, `.rmi`) play through a SoundFont: one named like the song or in its folder, else one installed on the system.
Without one they play with simple built-in sounds.

Instrument files (SoundFont `.sf2`, DLS `.dls`, SFZ `.sfz`, FastTracker `.xi`) play their samples one after another.
The Info panel lists the instruments and samples.
An SFZ file needs its sample files beside it.

Monkey's Audio files from version 3.80 on play.
Files from 3.80 to 3.92 made at the fast or high level stay silent, as do files from before 3.80; the Info panel says why.

A sound file opens on its own page: the cover, the waveform and the spectrogram.

The playhead follows the sound. The video keys play, seek and change the volume.

Click the waveform or spectrogram to play from that point.

The Info panel shows the tags: title, artist, album, track and more.

In the gallery, a sound file shows its cover, else its waveform.

In the gallery's filter, `is:sound` finds sound files and `is:video` finds videos.

Files longer than 30 minutes show a sampled waveform.

DRM-protected files (iTunes M4P) never play.

## Queue, repeat and shuffle {#queue}
When a sound file ends, the next sound file in the folder plays.
Videos do the same when Play the next video is on.
A selection of two or more files in the gallery plays in its own order.

One sound file follows the next without a gap.
Settings › Viewing can crossfade between files, never within an album.

- [[key:next-track]] and [[key:prev-track]]: next or previous item
- Previous after 3 seconds starts the item over.
- [[key:cycle-repeat]]: repeat off, all or one
- [[key:shuffle]]: random order, each item once a round; Previous goes back through what played
- [[key:play-queue]]: the play queue
- Sound files have these buttons on their playback bar.

Play next and Add to queue are in the gallery's right-click menu and the command palette.
They take the selection, or the file shown. A folder adds its sound files and videos.

The queue plays on to its end, videos included, whatever the folder holds.
In the queue: drag a row to move it, double-click to play it, × or Delete to remove it.

Removing an item or clearing the queue never touches the files.
The queue stays until you clear it or close Specular.

## Playlists {#playlists}
M3U, M3U8, PLS and XSPF playlists open as the queue and start playing.
Paths may be relative to the playlist. M3U files in Latin-1 read right.

Network addresses are listed but not played.
Save as playlist… in the queue writes M3U8 or XSPF.

## Sound visuals {#sound-visuals}
- [[key:cycle-sound-visuals]]: overview, meters, or everything

The row at the top turns each visual on or off.

From the keyboard:

- Tab from the picture reaches the row. Arrows move along it; Enter or Space turns one on or off.
- Tab again reaches synced lyrics. Up and Down move; Enter plays from that line.
- Esc leaves.

Waveform: each channel's peak and RMS level. The played part is orange.

Spectrogram: frequency over time, low notes at the bottom.

Spectrum: third-octave bars at the playhead. Curve shows every frequency instead. Peaks hold for a moment.

Meters: peak, RMS and true peak of each channel, in dB below full scale.

A lit box above a meter means that channel clipped. Click the meters to clear it.

Loudness: momentary, short-term and integrated loudness in LUFS (EBU R 128), loudness range and true peak, with the loudness over time.

The Info panel shows the integrated loudness, loudness range and true peak.

A long recording shows its page first. "Measuring loudness…" stands where the whole file's numbers will go.

Scroll over the waveform, spectrogram or loudness to zoom in time. Drag to move along.

A zoomed view follows the playhead. With reduce motion it turns pages instead of scrolling.

While the sound plays, spectrum, meters and stereo show what you hear, after the equaliser and volume. Paused, they show the file.

Stereo: the goniometer and the phase correlation. Mono sound is a vertical line; wide stereo spreads out.

Phase correlation runs from −1 to +1. Near +1 is mono-like, near 0 is wide, below 0 cancels when played in mono. The mark is the slow average.

The page takes its colours from the cover and shows it, blurred, behind. Settings › Viewing turns each off.

## Lyrics {#lyrics}
- [[key:lyrics]]: show or hide the lyrics

Lyrics come from a .lrc file with the same name beside the sound file, else from its tags.

Timed lyrics scroll with the song. The line sung is bold. Click a line to play from it.

Plain lyrics scroll with the wheel.

The Library finds songs by words in their lyrics.
