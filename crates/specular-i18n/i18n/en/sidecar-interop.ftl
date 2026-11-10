# Other apps' sidecars beside the photos (darktable, Lightroom, Capture One, digiKam,
# RawTherapee): read into the Library; only marks are written into them.

## Info panel

# $app is the app's own name: darktable, Lightroom, Capture One, digiKam, RawTherapee.
info-sidecar-from = From { $app } sidecar
info-sidecar-from-other = From another app's sidecar
info-sidecar-file = Sidecar file: { $file }
info-sidecar-stars = { $count ->
    [one] 1 star
   *[other] { $count } stars
}
info-sidecar-rejected = Rejected
info-sidecar-picked = Picked
info-sidecar-keywords = Keywords: { $keywords }
info-sidecar-nothing = Nothing Specular reads
info-sidecar-duplicates = darktable duplicates
info-sidecar-duplicate = { $number }: { $what }

## Settings › Library

settings-library-prefer-sidecars-in = Prefer other apps' sidecars in
settings-library-prefer-sidecars-in-help = Folders, separated by ;. There, ratings, labels, picks and keywords from darktable, Lightroom and other apps' sidecars replace yours. Elsewhere they only fill in what you haven't set.

## Writing marks, keywords and positions

sidecar-other-apps-only = Its sidecars belong to other apps. Specular doesn't add keywords or positions to them.
