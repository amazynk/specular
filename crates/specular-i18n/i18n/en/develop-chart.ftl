# Specular — Develop: the camera profile, and camera profiles made from a chart shot.

# The raw's camera profile (White balance card): a choice and its status line.
develop-input-profile = Camera profile
develop-input-profile-auto = Camera standard
develop-input-profile-matrix = Matrix only
develop-input-profile-file = Profile file…
develop-input-profile-named = File: { $name }
develop-input-profile-in-use = Using { $name }.
develop-input-profile-changed = Using { $name }. The file changed since it was chosen.
develop-input-profile-missing = The profile file is missing. The camera matrix is used.
develop-input-profile-unusable = The profile file can't be used: { $reason }. The camera matrix is used.
develop-input-profile-dialog = Choose a camera profile
develop-input-profile-kind = DNG camera profiles

# "Make profile from chart…": the panel over the picture.
develop-chart-start = Make profile from chart…
develop-chart-title = Profile from chart
develop-chart-kind = Chart
develop-chart-kind-cc24 = ColorChecker 24
develop-chart-kind-sg = ColorChecker SG
develop-chart-kind-it8 = IT8.7/2
develop-chart-kind-spyder = SpyderCHECKR
develop-chart-ref = Reference
develop-chart-ref-load = Load file…
develop-chart-hint-xrite = Load the chart's reference file (CGATS text). It comes with the chart, or from X-Rite's website for your chart's edition.
develop-chart-hint-it8 = Load your target's reference file. Each batch has its own, from the target's maker.
develop-chart-spyder = SpyderCHECKR isn't supported: its colour values are Datacolor's.
develop-chart-step-reference = Choose the chart and load its reference file.
# $number: 1 to 4.
develop-chart-step-corner = Click the centre of { $name } ({ $number } of 4).
develop-chart-measuring = Measuring the chart…
develop-chart-step-save = Save the profile, or mark the corners again.
develop-chart-corner-dark-skin = dark skin (top left)
develop-chart-corner-bluish-green = bluish green (top right)
develop-chart-corner-black = black (bottom right)
develop-chart-corner-white = white (bottom left)

# Why a reference file can't be used.
develop-chart-ref-too-large = The file is too large for a reference file.
develop-chart-ref-not-cgats = Not a CGATS reference file.
develop-chart-ref-no-ids = The file has no patch names.
develop-chart-ref-no-colour = The file has no L*a*b* or XYZ values.
develop-chart-ref-bad-row = Row { $row } of the file is damaged.
develop-chart-ref-count = The file says { $declared } patches but has { $found }.
develop-chart-ref-duplicate = Patch { $id } is in the file twice.
develop-chart-ref-illuminant = The values are for { $name } light. D50 or D65 is needed.
develop-chart-ref-wrong-chart = The file has { $found } patches. This chart has { $expected }.
develop-chart-ref-names = The patch names don't match this chart.
develop-chart-ref-corner = Corner patch { $id } is missing from the file.
develop-chart-ref-unreadable = The file can't be read: { $reason }
develop-chart-ref-dialog = Choose the chart's reference file
develop-chart-ref-kind = Chart reference files

# Why the chart can't be measured.
develop-chart-not-found = The chart wasn't found there. Mark the corners again.
develop-chart-too-few = Only { $count } patches can be used. Light the chart evenly and avoid glare.
develop-chart-no-greys = No grey patch can be used. Expose so the greys aren't clipped.
develop-chart-degenerate = The patches are too alike to make a profile.

# The report. ΔE2000: colour difference; under 1 is hard to see.
develop-chart-report-after = ΔE2000 with the profile: mean { $mean }, median { $median }, max { $max }
develop-chart-report-before = Before: mean { $mean }, median { $median }, max { $max }
develop-chart-report-cv = Checked on patches left out of the fit: mean { $mean }
develop-chart-left-out = Left out: { $id } ({ $why })
develop-chart-left-clipped = clipped
develop-chart-left-uneven = glare or dust
develop-chart-left-outside = outside the photo
develop-chart-uneven-light = The greys vary by { PERCENT($percent) }: the light may be uneven.
develop-chart-use-camera = Use for photos from { $camera }
develop-chart-again = Mark again
develop-chart-save = Save…
develop-chart-close = Close
develop-chart-save-title = Save camera profile
develop-chart-default-name = { $camera } chart profile
develop-chart-saved = Saved { $name } as { $file }.
develop-chart-save-failed = The profile can't be saved: { $reason }
