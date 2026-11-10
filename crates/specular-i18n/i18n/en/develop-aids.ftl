# Specular — Develop's darkroom aids: scopes, clipping overlays, soft proof, print. English.

# The scope dock at the top of the Develop panel: its group and tab names.
develop-scopes = Scopes
develop-scope-histogram = Histogram
develop-scope-waveform = Waveform
develop-scope-parade = Parade
develop-scope-vectorscope = Vectorscope
# The waveform's chip: luma only, or red, green and blue over each other.
develop-scope-mode = Waveform shows
develop-scope-luma = Luma
develop-scope-rgb = RGB
develop-scope-scale = Histogram scale
develop-scope-log = Log
develop-scope-linear = Linear
develop-scopes-hide = Hide scopes
develop-scopes-hidden = Scopes hidden

# The Clipping card and its rows; also the picture's right-click menu.
develop-clipping = Clipping
develop-clip-display = Display clipping
develop-clip-scene = Scene clipping
develop-clip-gamut = Gamut warning

# Said when an aid is turned on or off. $what: its name, e.g. "Display clipping".
develop-aid-on = { $what } on
develop-aid-off = { $what } off

# The Soft proof card.
develop-proof = Soft proof
develop-proof-profile = Profile
develop-proof-file = Choose a profile file…
develop-proof-intent = Intent
develop-proof-perceptual = Perceptual
develop-proof-relative = Relative colorimetric
develop-proof-bpc = Black point compensation
develop-proof-paper = Simulate paper white
develop-proof-ink = Simulate ink black
# While the system's colour profile folders are being read.
develop-proof-looking = Looking…
develop-proof-none = None found
# $name: the profile's name.
develop-proof-on = Soft proof on: { $name }
develop-proof-off = Soft proof off
develop-proof-using = Proofing with { $name }
develop-proof-choose = Choose a printer profile to soft-proof with.
develop-proof-unreadable = That file is not a colour profile Specular can read.
develop-proof-not-output = That profile does not describe a printer or screen, so it cannot be proofed.
# $reason: why, e.g. "That file is not a colour profile Specular can read."
develop-proof-failed = Soft proof stopped. { $reason }
# Shown when Develop opens and the printer profile used last is gone; $name: its file name.
develop-proof-missing = Proof profile { $name } not found. Soft proof is off.

# The picture's right-click menu in Develop.
develop-aids = View
develop-print = Print…

# Palette and keys.
palette-develop-scopes = Next scope
palette-develop-scopes-help = Histogram, waveform, parade, vectorscope, then none.
palette-develop-clip-display = Display clipping
palette-develop-clip-display-help = Red where the picture is pure white, blue where pure black.
palette-develop-clip-scene = Scene clipping
palette-develop-clip-scene-help = Orange over the look's white, cyan under its black.
palette-develop-clip-gamut = Gamut warning
palette-develop-clip-gamut-help = Green where colours fall outside the screen's or the proof profile's range.
palette-develop-soft-proof = Soft proof
palette-develop-soft-proof-help = Show the photo as the chosen printer profile prints it.

# The print dialog, printing from Develop.
print-profile = Colour profile
print-profile-srgb = sRGB
print-intent = Intent
print-intent-perceptual = Perceptual
print-intent-relative = Relative colorimetric
print-intent-relative-bpc = Relative colorimetric, black point compensation
print-profile-none = Choose a printer profile in Develop's Soft proof card first.
# $reason: why, in the colour engine's words.
print-profile-failed = The printer profile cannot be used: { $reason }
