# Generated subtitles: speech to text on this computer (Local AI, Whisper).

## The player's subtitle (CC) menu
player-subtitles-generate = Generate subtitles…
# While subtitles are being generated; choosing it stops. $percent: "34 %".
player-subtitles-generating = Generating subtitles… { $percent }
player-subtitles-save = Save subtitles…

## Command palette
palette-generate-subtitles = Generate subtitles…
palette-generate-subtitles-help = Subtitles from the speech, made on this computer.
palette-save-subtitles = Save generated subtitles…
palette-save-subtitles-help = Save the generated subtitles as an SRT or WebVTT file (or LRC for a song).

## The language sheet
stt-choose-language = Language spoken
stt-detect = Detect the language
stt-translate = Translate to English
# Why Translate to English is greyed out with Whisper large-v3 turbo.
stt-translate-turbo = Large-v3 turbo can't translate. Use base or small.
stt-settings = Local AI settings…

## The track's name in the menu. $language: the language's name.
stt-track = Generated ({ $language })
stt-track-pending = Generated
stt-track-translated = Generated (English, from { $language })
# Generating was stopped before the end; what was made shows.
stt-track-partial = Generated ({ $language }, unfinished)

## Notices
stt-needs-model = Speech to text is not installed. Download it in Settings › Local AI.
stt-started = Generating subtitles. They appear as they are ready.
stt-stopped = Stopped generating subtitles.
stt-done = Subtitles ready.
# Opened again: an unfinished job goes on where it was.
stt-resumed = Continuing the generated subtitles.
stt-failed = Subtitles could not be generated: { $reason }
stt-no-sound = This video has no sound that can be read.
stt-saved = Subtitles saved as { $name }.
stt-save-failed = Could not save the subtitles: { $reason }

## Save dialog
stt-save-title = Save subtitles
stt-file-srt = SubRip subtitles
stt-file-vtt = WebVTT subtitles
# Synced lyrics (.lrc); a song's transcript saved beside it becomes its lyrics.
stt-file-lrc = LRC lyrics

## Background work panel
work-kind-speech = Generated subtitles
work-step-transcribing = Turning speech into subtitles

## Settings › Local AI
settings-ai-speech = Speech to text
settings-ai-speech-help = Make subtitles from what is said in a video. Choose Generate subtitles… in the player's subtitle menu.
settings-ai-speech-size = Model
settings-ai-speech-size-help = Whisper base is fastest. Small reads better. Large-v3 turbo reads best, needs a good graphics card and can't translate.
settings-ai-speech-size-recommended = Default
settings-ai-speech-size-small = Whisper base
settings-ai-speech-size-standard = Whisper small
settings-ai-speech-size-large = Whisper large-v3 turbo
