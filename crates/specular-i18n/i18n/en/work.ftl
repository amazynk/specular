# Specular — Background work: the indicator shown while something runs in the background
# (in the gallery's top bar and above the viewer's toolbar) and the panel it opens. English.

## The panel

# The panel's title.
work-title = Background work
# The palette entry that opens the panel, and the indicator's tooltip.
work-show = Show background work
# The panel when nothing runs any more.
work-empty = Nothing is running in the background.

## Jobs (the panel's row titles, the indicator and the announcements)

# Building the photo library: finding photos, reading their facts, signatures, groups.
work-kind-library = Photo library
work-kind-ai-search = AI search
# Finding and grouping faces (Local AI → People).
work-kind-faces = People
# Reading text in photos (Local AI → Text).
work-kind-text = Text in photos
# Scoring sharpness and exposure for culling (Local AI → Culling).
work-kind-culling = Culling scores
# Downloading a Local AI model.
work-kind-download = AI model download
# Looking for duplicates and similar shots in the open folder.
work-kind-similar = Similar shots

## Steps (what a job does now)

work-step-finding = Finding photos
work-step-reading = Reading photo details
work-step-signatures = Comparing photos
work-step-grouping = Grouping similar shots
work-step-text-index = Building the search index
work-step-embedding = Indexing photos for AI search
# After a different AI search model size was chosen: every photo is indexed again.
work-step-switching = Re-indexing for the new model size
work-step-faces = Finding faces
work-step-ocr = Reading text in photos
work-step-scoring = Scoring sharpness and exposure
work-step-downloading = Downloading
work-step-focus = Checking sharpness
# A step and what it is working on now. $step is one of the steps above; $item is a file
# or folder name.
work-step-item = { $step } · { $item }

## Progress

# How far a job is. $done and $total are numbers formatted for the locale ("1,240",
# "9,800"); $count is the total as a plain number, for the plural.
work-count-photos = { $done } of { $total } { $count ->
    [one] photo
   *[other] photos
}
# How far a job is in other units: $done and $total are formatted ("1,240", or "1.2 GB").
work-count = { $done } of { $total }
# A count followed by what is left: $count is a work-count line, $left a formatted number.
work-count-left = { $count } · { $left } to go
# How far a job is when the total is not known yet. $done is formatted; $count is the same
# number, plain, for the plural.
work-so-far-photos = { $done } { $count ->
    [one] photo
   *[other] photos
} so far
work-so-far = { $done } so far

## Time left (estimated from the recent rate)

# Shown until there are enough measurements to estimate the time left.
work-eta-estimating = Estimating…
work-eta-seconds = About { $n ->
    [one] { $n } second
   *[other] { $n } seconds
} left
work-eta-minutes = About { $n ->
    [one] { $n } minute
   *[other] { $n } minutes
} left
work-eta-hours = About { $h ->
    [one] { $h } hour
   *[other] { $h } hours
} left
# Hours and minutes: $h hours and $m minutes (1 to 59).
work-eta-hours-minutes = About { $h ->
    [one] { $h } hour
   *[other] { $h } hours
} { $m ->
    [one] { $m } minute
   *[other] { $m } minutes
} left

## Why a job waits

work-paused-browsing = Paused while you browse
work-paused-battery = Paused on battery power
work-paused-low-power = Paused in power saver mode
work-paused-heat = Paused while the computer is hot
work-paused-memory = Paused while memory is low
# Paused with the panel's Pause button.
work-paused-user = Paused

## What follows

# What the job does after this step. $steps is a list of steps ("Comparing photos and
# Grouping similar shots"), formatted for the locale.
work-then = Then: { $steps }

## Buttons

work-pause = Pause
work-resume = Resume
work-settings = Settings…

## The indicator

# The indicator's label: the main job and how far it is ("AI search · 1,240 of 9,800").
# $job is a job name; $done and $total are formatted numbers.
work-chip = { $job } · { $done } of { $total }
# The indicator when the total is not known yet: $done is a formatted number.
work-chip-so-far = { $job } · { $done }
# The indicator while the main job waits: $reason is one of the work-paused lines.
work-chip-paused = { $job } · { $reason }
# The indicator with other jobs running too: $chip is one of the lines above; $more is
# how many other jobs run.
work-chip-more = { $chip } (+{ $more })
# The indicator's name for screen readers: $status is the main job and how far it is.
work-chip-name = Background work: { $status }

## Announcements (read out politely by screen readers, once per job)

# $job is a job name.
work-started = { $job } started in the background
work-finished = { $job } finished
