# A sound file's page: its visuals, meters and loudness (EBU R 128).

# The choices in the row at the top of the page.
sound-visual-waveform = Waveform
sound-visual-spectrogram = Spectrogram
sound-visual-spectrum = Spectrum
sound-visual-meters = Meters
sound-visual-loudness = Loudness
# Draws the spectrum as a curve instead of bars.
sound-visual-curve = Curve

# The page as a whole, for screen readers.
sound-page = Sound visuals
sound-page-reading = Reading the sound…
sound-page-unreadable = This sound can’t be read for its visuals.

# Channel names, short (under the meters).
sound-channel-mono = M
sound-channel-left = L
sound-channel-right = R
sound-channel-centre = C
sound-channel-lfe = LFE
sound-channel-left-surround = Ls
sound-channel-right-surround = Rs
sound-channel-left-back = Lb
sound-channel-right-back = Rb
sound-channel-back-centre = Cb

# The loudness numbers.
sound-momentary = Momentary
sound-short-term = Short-term
sound-integrated = Integrated
sound-loudness-range = Loudness range
sound-true-peak = True peak
# No value: silence, or not measured yet.
sound-no-value = –

# Units after a number.
sound-unit-lufs = LUFS
sound-unit-lu = LU
sound-unit-dbtp = dBTP
sound-unit-db = dB
# A frequency axis label in kilohertz: $value is a number, e.g. 2 or 1.5.
sound-axis-khz = { $value }k

# A meter read by a screen reader. $peak, $rms and $true_peak are levels with their unit;
# $clip is sound-clipped or empty.
sound-meter-value = Peak { $peak }, RMS { $rms }, true peak { $true_peak }. { $clip }
sound-clipped = Clipped.

# The Info panel's Sound group.
info-sound-loudness = Loudness
info-sound-loudness-range = Loudness range
info-sound-true-peak = True peak

# The command palette and the cheat sheet.
palette-cycle-sound-visuals = Next sound visuals
palette-cycle-sound-visuals-help = Waveform and spectrogram, then meters and loudness, then everything.
disabled-not-sound-file = This file is not a sound file

# Settings.
settings-viewing-sound-visuals = Sound visuals
settings-viewing-sound-visuals-help = What a sound file’s page shows. The page’s choices and the w key change it.

# The stereo panel (goniometer and phase correlation) and the lyrics column.
sound-visual-stereo = Stereo
sound-visual-lyrics = Lyrics
sound-correlation = Phase correlation
# The correlation for a screen reader: $value now, $average its slow average (signed numbers).
sound-correlation-value = { $value }, average { $average }
sound-lyrics = Lyrics
# The command palette and the cheat sheet.
palette-toggle-lyrics = Show lyrics
palette-toggle-lyrics-help = The song’s lyrics beside the sound visuals.
# Settings.
settings-viewing-sound-lyrics = Show lyrics
settings-viewing-sound-lyrics-help = A sound file’s lyrics show beside its visuals: from a .lrc file beside it, or from its tags. The y key changes it.
settings-viewing-sound-cover-colours = Colours from the cover
settings-viewing-sound-cover-colours-help = A sound file’s page takes its colours from the cover. Text stays readable.
settings-viewing-sound-backdrop = Blurred cover behind
settings-viewing-sound-backdrop-help = A sound file’s page shows its cover, blurred, behind it.
sound-loudness-measuring = Measuring loudness…
