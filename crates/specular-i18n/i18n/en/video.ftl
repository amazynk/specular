# Movies (MP4, MOV, M4V, 3GP, 3G2, MKV, WebM, MTS, M2TS): playback, sound, the transport bar, the gallery and the
# info panel.

# Keymap mode and cheat sheet tab while a movie is shown.
keys-mode-video = Movies
cheatsheet-video-sound = Sound
cheatsheet-video-speed = Speed and looping
cheatsheet-video-subtitles = Subtitles and timing
cheatsheet-video-seek-percent = Jump to { PERCENT(0) } to { PERCENT(90) }
cheatsheet-video-picture = Frames and picture

## Command palette

palette-play-pause-video = Play or pause video
palette-play-pause-video-help = Start or stop the movie shown (Space while a movie is shown).
palette-seek-forward = Forward 5 seconds
palette-seek-forward-help = Jump five seconds ahead in the movie.
palette-seek-back = Back 5 seconds
palette-seek-back-help = Jump five seconds back in the movie.
palette-volume-up = Louder
palette-volume-up-help = Turn the movie’s sound up.
palette-volume-down = Quieter
palette-volume-down-help = Turn the movie’s sound down.
palette-mute = Mute or unmute
palette-mute-help = Silence the movie’s sound, or bring it back.
palette-video-fullscreen = Play full screen
palette-video-fullscreen-help = Fill the screen with the movie, or go back to the window.
palette-seek-forward-long = Forward 10 seconds
palette-seek-forward-long-help = Jump ten seconds ahead in the movie.
palette-seek-back-long = Back 10 seconds
palette-seek-back-long-help = Jump ten seconds back in the movie.
# $percent is a whole number, e.g. 30; 0 is the start.
palette-seek-percent = Jump to { PERCENT($percent) }
palette-seek-percent-help = Jump to a tenth of the movie: 1 to 9 for { PERCENT(10) } to { PERCENT(90) }, 0 for the start.
palette-cycle-subtitles = Next subtitles
palette-cycle-subtitles-help = Show the next subtitle track, or none.
palette-open-subtitle = Open subtitle file…
palette-open-subtitle-help = Show subtitles from a file (SRT, WebVTT, ASS, SSA, SUP, IDX).
palette-subtitle-earlier = Subtitles earlier
palette-subtitle-earlier-help = Show subtitles 50 ms earlier.
palette-subtitle-later = Subtitles later
palette-subtitle-later-help = Show subtitles 50 ms later.
palette-subtitle-earlier-fine = Subtitles slightly earlier
palette-subtitle-earlier-fine-help = Show subtitles 10 ms earlier.
palette-subtitle-later-fine = Subtitles slightly later
palette-subtitle-later-fine-help = Show subtitles 10 ms later.
palette-subtitle-step-prev = Previous subtitle now
palette-subtitle-step-prev-help = Shift the subtitles so the previous one starts now.
palette-subtitle-step-next = Next subtitle now
palette-subtitle-step-next-help = Shift the subtitles so the next one starts now.
palette-subtitle-up = Subtitles higher
palette-subtitle-up-help = Move subtitles up the picture.
palette-subtitle-down = Subtitles lower
palette-subtitle-down-help = Move subtitles down the picture.
palette-subtitle-bigger = Larger subtitles
palette-subtitle-bigger-help = Make text subtitles { PERCENT(10) } larger.
palette-subtitle-smaller = Smaller subtitles
palette-subtitle-smaller-help = Make text subtitles { PERCENT(10) } smaller.
palette-subtitle-reset-place = Reset subtitle position and size
palette-subtitle-reset-place-help = Subtitles back at their usual place and size.
palette-subtitle-style = Subtitle style…
palette-subtitle-style-help = Set the subtitles’ position, size, colour and font in Settings.
palette-cycle-sound = Next sound track
palette-cycle-sound-help = Play the movie's next sound track.
palette-sound-earlier = Sound earlier
palette-sound-earlier-help = Play the sound 50 ms earlier.
palette-sound-later = Sound later
palette-sound-later-help = Play the sound 50 ms later.
palette-sound-earlier-fine = Sound slightly earlier
palette-sound-earlier-fine-help = Play the sound 10 ms earlier.
palette-sound-later-fine = Sound slightly later
palette-sound-later-fine-help = Play the sound 10 ms later.
palette-reset-delays = Reset subtitle and sound delays
palette-reset-delays-help = Subtitles and sound back in step with the picture as stored.
palette-prev-chapter = Previous chapter
palette-prev-chapter-help = Go to the start of this chapter, or the one before.
palette-next-chapter = Next chapter
palette-next-chapter-help = Go to the next chapter.
palette-ab-loop = A-B repeat
palette-ab-loop-help = Set the start, then the end of a part to repeat; again to stop.
palette-save-frame = Save frame…
palette-save-frame-help = Save the frame on screen as a PNG at full size.
palette-copy-frame = Copy frame
palette-copy-frame-help = Copy the frame on screen at full size.
palette-mini-player = Mini player
palette-mini-player-help = A small window that stays on top; again to go back.
palette-cycle-aspect = Picture shape
palette-cycle-aspect-help = Show the picture as stored, 4:3, 16:9, 1.85:1 or 2.39:1.
palette-cycle-crop = Crop picture
palette-cycle-crop-help = Crop black bars: to 4:3, 16:9, 1.85:1 or 2.39:1, or none.
palette-go-to-time = Go to time…
palette-go-to-time-help = Type where to go: 1:23:45, 83:20, 50%, +30 or -1:00.
palette-video-picture = Picture adjustments…
palette-video-picture-help = Brightness, contrast, saturation, gamma and hue while it plays.

disabled-not-video = This file is not a movie
disabled-no-sound = This movie has no sound to play

## Transport bar

# $position and $duration are times such as 0:12 and 1:30:05.
transport-time = { $position } / { $duration }
# The seek bar along the top of the bar (accessibility name and tooltip).
transport-seek-bar = Position in the movie
transport-loop-on = Loop the movie
transport-loop-off = Stop looping
transport-mute = Mute
transport-unmute = Unmute
transport-volume = Volume

## Toasts

# $percent is a whole number, e.g. 80.
toast-volume = Volume { PERCENT($percent) }
toast-muted = Sound muted
toast-unmuted = Sound on
toast-video-loop-on = Looping the movie
toast-video-loop-off = Playing the movie once
# Shown once per movie when there is no sound device or it goes away; the movie plays on.
toast-no-sound-device = No sound device: playing without sound
toast-sound-device-lost = The sound device went away: playing without sound
# The movie's sound is in a coding this version can't play; $codec names it, e.g. AAC.
toast-sound-unsupported = Playing without sound: { $codec } sound can’t be played yet
# The movie's sound decoder stopped part way; $reason is error-reason-internal.
toast-sound-stopped = The sound stopped: { $reason }

## Gallery and library

# The length on a movie's thumbnail, e.g. 1:05.
gallery-video-duration = { $duration }
# The gallery's filter and the library's "is:video" property.
gallery-filter-video = Videos
# The gallery's filter and the library's "is:sound" property (MP3, FLAC, M4A...).
gallery-filter-sound = Sound files

## Settings

settings-viewing-video-volume = Movie volume
settings-viewing-video-volume-help = How loud movies play. The Up and Down keys change it too.
settings-viewing-video-muted = Mute movies
settings-viewing-video-muted-help = Play movies without sound. The m key and the transport bar’s sound button change it too.
settings-viewing-video-resume = Resume videos
settings-viewing-video-resume-help = Videos longer than 3 minutes start where you left them.
settings-viewing-forget-playback = Forget playback positions…
settings-viewing-forget-playback-help = Videos start from the beginning again. Subtitle and sound delays stay.
settings-viewing-video-play-next = Play the next video
settings-viewing-video-play-next-help = When a video ends, the next video in the folder plays.
settings-viewing-subtitle-language = Subtitle language
settings-viewing-subtitle-language-help = A language code, such as he or en. Subtitles in it show by themselves. Choosing subtitles sets it; Off clears it.
settings-viewing-subtitle-size = Subtitle size
settings-viewing-subtitle-size-help = How large text subtitles are. { PERCENT(100) } is the usual size for the picture.
settings-viewing-subtitle-position = Subtitle position
settings-viewing-subtitle-position-help = How far above the bottom of the picture subtitles sit. Subtitles the file places itself keep their place.
settings-viewing-subtitle-color = Subtitle colour
settings-viewing-subtitle-color-help = The colour of text subtitles. Colours set in the file stay, unless Override subtitle styles is on.
settings-viewing-subtitle-color-white = White
settings-viewing-subtitle-color-yellow = Yellow
settings-viewing-subtitle-color-cyan = Cyan
settings-viewing-subtitle-color-green = Green
settings-viewing-subtitle-color-custom = Custom colour
settings-viewing-subtitle-custom-color = Custom subtitle colour
settings-viewing-subtitle-custom-color-help = Used when Subtitle colour is set to “Custom colour”.
settings-viewing-subtitle-outline = Subtitle outline
settings-viewing-subtitle-outline-help = The edge around each letter, so subtitles read over bright pictures.
settings-viewing-subtitle-outline-black = Black
settings-viewing-subtitle-outline-grey = Grey
settings-viewing-subtitle-outline-white = White
settings-viewing-subtitle-outline-none = None
settings-viewing-subtitle-background = Subtitle background
settings-viewing-subtitle-background-help = A dark box behind subtitles makes them easier to read.
settings-viewing-subtitle-background-none = None
settings-viewing-subtitle-background-box = Translucent box
settings-viewing-subtitle-font = Subtitle font
settings-viewing-subtitle-font-help = The font of text subtitles. Letters it lacks, such as Hebrew or Chinese, come from another font.
settings-viewing-subtitle-font-default = Interface font
settings-viewing-subtitle-override-styles = Override subtitle styles
settings-viewing-subtitle-override-styles-help = Your colour, outline and size apply to every subtitle, over the file’s own.
settings-viewing-sound-language = Sound language
settings-viewing-sound-language-help = A language code, such as en. A movie with several sound tracks plays this one first.
settings-viewing-subtitle-mode = Show subtitles
settings-viewing-subtitle-mode-help = Which subtitles start by themselves. Forced subtitles in the sound’s language always show.
settings-viewing-subtitle-mode-off = Off
settings-viewing-subtitle-mode-forced = Forced only
settings-viewing-subtitle-mode-always = Always
settings-viewing-subtitle-prefer-sdh = Prefer subtitles for the deaf and hard of hearing
settings-viewing-subtitle-prefer-sdh-help = SDH subtitles and closed captions come first. They describe sounds too.
settings-viewing-sound-prefer-description = Prefer audio description tracks
settings-viewing-sound-prefer-description-help = A sound track that describes the picture plays first, when the movie has one.

## Info panel

info-video-group = Movie
info-video-duration = Length
info-video-codec = Video
# After the video coding when the video is interlaced, e.g. "H.264 · interlaced".
info-video-interlaced = interlaced
info-video-frame-rate = Frame rate
# $fps is a number such as 29.97.
info-video-fps = { $fps } frames per second
info-video-pixel-aspect = Pixel shape
# Non-square pixels (DVD, DV, anamorphic video): $h and $v are the pixel's width and height
# as a ratio, e.g. "8:9"; the frames are shown stretched to it.
info-video-pixel-aspect-value = { $h }:{ $v }, shown stretched
info-video-audio = Sound
info-video-no-audio = None
info-video-recorded = Recorded
info-video-camera = Camera
info-video-silent = Sound not played
# $codec names the sound's coding, e.g. AAC.
info-video-silent-value = { $codec } can’t be played yet

## Sound files (MP3, FLAC, M4A...): the info panel's Sound group

info-sound-group = Sound
info-sound-title = Title
info-sound-artist = Artist
info-sound-album = Album
info-sound-album-artist = Album artist
info-sound-composer = Composer
info-sound-genre = Genre
info-sound-date = Date
info-sound-track = Track
info-sound-disc = Disc
info-sound-comment = Comment
info-sound-copyright = Copyright
info-sound-encoder = Encoded with
info-sound-track-gain = Track gain
info-sound-album-gain = Album gain
info-sound-midi = MIDI
info-sound-instruments = Instruments
info-sound-samples = Samples
info-sound-coding = Coding
# $bits is a number such as 24.
info-sound-bits-value = { $bits }-bit
info-sound-bitrate = Bit rate
# $kbps is a number such as 320.
info-sound-kbps = { $kbps } kbit/s
# The cover picture's type and size, e.g. "JPEG 600 × 600".
info-sound-cover = Cover
# The kinds of tag read, e.g. "ID3v2.4, ID3v1".
info-sound-tags = Tags
info-sound-drm = Protected by DRM: Specular never decrypts it
toast-sound-drm = Playing without sound: it is protected by DRM

## Player menus: subtitles, sound tracks, chapters

# A delay as shown in the menus and toasts; $value is signed, e.g. +350 or −120.
player-delay-ms = { $value } ms
player-subtitles = Subtitles
player-sound-tracks = Sound tracks
player-chapters = Chapters
player-subtitles-off = Off
player-subtitles-open = Open subtitle file…
# The character set a subtitle file was read in, e.g. windows-1255; choosing it tries the next.
player-subtitles-charset = Character set: { $charset }
player-subtitles-style = Subtitle style…
player-subtitle-delay = Subtitle delay
player-sound-delay = Sound delay
player-delay-earlier = Earlier
player-delay-later = Later
player-delay-reset = Reset the delay
# The speed shown on the transport bar opens this menu.
player-speed = Playback speed
player-more = More
player-picture = Picture
player-go-to-time = Go to time…
# Shown in the empty Go to time field (hours, minutes, seconds).
player-go-to-time-hint = h:mm:ss
player-picture-open = Picture…
player-picture-reset = Reset picture
player-picture-brightness = Brightness
player-picture-contrast = Contrast
player-picture-saturation = Saturation
player-picture-gamma = Gamma
player-picture-hue = Hue
# $value is signed, e.g. +15 or −10.
player-picture-percent = { PERCENT($value) }
player-picture-degrees = { $value }°
# The −, value and + buttons of a picture adjustment (accessibility); $name is e.g. Contrast.
player-picture-less = { $name }: lower
player-picture-more = { $name }: raise
player-picture-reset-one = { $name }: reset

## Player: toasts and dialogs

# $time is a position such as 12:34.
toast-video-resumed = Resumed at { $time }
toast-video-start-over = Start over
# $track names a subtitle track, e.g. "Hebrew" or "English (forced)".
toast-subtitles = Subtitles: { $track }
toast-subtitles-off = Subtitles off
toast-subtitles-none = This video has no subtitles
toast-subtitles-empty = These subtitles have no text to show
# $reason is one of the subtitles-reason-* messages.
toast-subtitles-unreadable = These subtitles can’t be shown: { $reason }
subtitles-reason-not-subtitles = it is not a subtitle file
# $kind is a format's name as stored, e.g. S_DVBSUB.
subtitles-reason-unsupported = { $kind } subtitles can’t be read yet
subtitles-reason-damaged = the file is damaged
subtitles-reason-unreadable = { $reason }
# $delay is a signed delay such as +350 ms.
toast-subtitle-delay = Subtitles { $delay }
toast-sound-delay = Sound { $delay }
toast-delays-reset = Subtitles and sound in step again
# $percent is a whole number, e.g. 12.
toast-subtitle-position = Subtitles { PERCENT($percent) } from the bottom
# $percent is a whole number, e.g. 120.
toast-subtitle-size = Subtitle size { PERCENT($percent) }
toast-subtitle-place-reset = Subtitles at their usual place and size
# $track names a sound track, e.g. "English (5.1)".
toast-sound-track = Sound: { $track }
toast-sound-one-track = This video has one sound track
toast-no-chapters = This video has no chapters
# $chapter is the chapter's name.
toast-chapter = { $chapter }
toast-ab-a = Repeat from { $time }: press again for the end
toast-ab-b = Repeating { $a } to { $b }
toast-ab-off = Repeat off
# $file is the saved picture's name.
toast-frame-saved = Saved { $file }
toast-frame-failed = The frame can’t be saved: { $reason }
# $shape is 4:3, 16:9, 1.85:1, 2.39:1 or player-shape-stored.
toast-video-shape = Picture shape: { $shape }
player-shape-stored = as stored
toast-video-crop = Cropped to { $shape }
toast-video-crop-off = Crop off
# $count is how many videos had a position kept.
toast-playback-forgotten = { $count ->
    [one] Forgot where 1 video was left
   *[other] Forgot where { $count } videos were left
}
player-forget-ask = { $count ->
    [one] Forget where 1 video was left?
   *[other] Forget where { $count } videos were left?
}
player-forget-detail = They start from the beginning again. The videos stay; subtitle and sound delays stay too.
player-forget-ok = Forget
player-subtitles-open-title = Open subtitle file
player-subtitles-file-kind = Subtitles
player-save-frame-title = Save frame
# A track without a language or name: $number counts from 1.
player-track-number = Track { $number }
# $track is a track's label; forced subtitles show only foreign speech and signs.
player-track-forced = { $track } (forced)
# $track is a track's label; SDH: subtitles for the deaf and hard of hearing.
player-track-sdh = { $track } (SDH)
# $track is a sound track's label; it describes the picture for blind viewers.
player-track-description = { $track } (audio description)
# $track is a sound track's label; mixed for the hard of hearing (clearer speech).
player-track-hearing = { $track } (hard of hearing)
# A CEA-608 closed caption channel, CC1 to CC4.
player-track-cc = CC{ $number }
# A CEA-608 text service channel, 1 to 4.
player-track-cc-text = Text { $number }
# A CEA-708 caption service, 1 to 63.
player-track-cc-service = Service { $number }
# A caption channel and its language: "CC1 (English)".
player-track-with-language = { $track } ({ $language })
player-channels-mono = mono
player-channels = { $count } channels
player-chapter-number = Chapter { $number }

# A network stream played in parts (HLS, DASH, a Matroska or WebM web file): the part on
# screen as a time range of the whole stream, e.g. "1:15–1:25".
info-video-part = Part on screen
info-video-part-value = { $start }–{ $end }
