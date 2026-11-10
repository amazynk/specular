# Read-only analysis in medical mode: segmentations, SUV, fusion, TMTV, time-intensity curves.
# Numbers and overlays the user asks for; no findings.

palette-segments = Show or hide segmentations
palette-segment-opacity = Next segmentation fill opacity
palette-fusion = Fuse the series of the same frame of reference
palette-fusion-colormap = Next fusion colour map
palette-fusion-blend = Next fusion opacity
palette-fusion-nudge = Move the fused series by hand
palette-tmtv = TMTV tool (sphere, box, off)
palette-tmtv-threshold = TMTV threshold (SUV { NUMBER(2.5) } or { PERCENT(41) } of SUVmax)
palette-time-curve = Time-intensity curve of the last region
palette-export-curve = Save the time-intensity curve as CSV

analysis-searching = Still looking for segmentations and series to fuse
segments-none = No segmentation of this frame of reference was found
segments-shown = Segmentations shown
segments-hidden = Segmentations hidden
segments-fill = Segmentation fill { PERCENT($percent) }
segments-failed = Segmentation not shown: { $reason }
segment-entry = { $name } · { $ml } mL

suv-value = SUVbw { $value }
suv-factor = SUVbw factor { $factor }, { $minutes } min after injection
suv-refused = No SUV: { $reason }
suv-preset = SUV 0–{ $upper }
suv-refused-not-pet = not a PET image
suv-refused-units = the pixel units are not Bq/mL
suv-refused-corrected = not corrected for attenuation and decay
suv-refused-decay = unsupported decay correction
suv-refused-weight = no patient weight
suv-refused-dose = no injected dose
suv-refused-half-life = no radionuclide half-life
suv-refused-injection = no injection time
suv-refused-scan-time = no acquisition time
suv-refused-delay = the injection is after the scan
suv-refused-scale = no Philips SUV scale factor
suv-refused-height = no patient height
suv-refused-sex = no patient sex
suv-refused-lbm = lean body mass out of range

colormap-hot-iron = Hot iron
colormap-rainbow = Rainbow
colormap-viridis = Viridis
fusion-none = No PET, SPECT, CT or MR series of this frame of reference to fuse
fusion-needed = Fuse a series first (Shift+U)
fusion-opening = Fusing { $modality }
fusion-off = Fusion off
fusion-refused = Cannot fuse: { $reason }
fusion-on = Fused { $modality }: { $colormap }, { PERCENT($blend) }
fusion-hidden = Fused { $modality } hidden
fusion-window-suv = Fused window SUV { $low }–{ $high }
fusion-value = { $modality } { $value }
fusion-value-suv = { $modality } SUVbw { $value }
fusion-blend-toast = Fusion opacity { PERCENT($percent) }
fusion-moved = Moved by hand: { $x }, { $y }, { $z } mm, { $degrees }°
fusion-nudge-hint = Arrows move the fused series 1 mm, Shift+Up/Down along the view, [ and ] turn it
fusion-nudge-off = Fused series fixed

tmtv-hint-sphere = TMTV: drag a sphere from its centre
tmtv-hint-box = TMTV: drag a box
tmtv-off = TMTV off
tmtv-threshold-suv = Threshold SUV { $suv }
tmtv-threshold-share = Threshold { PERCENT($share) } of SUVmax
tmtv-result = TMTV { $ml } mL in { $count ->
        [one] { $count } region
       *[other] { $count } regions
    }
tmtv-stats = SUVmax { $max }, SUVmean { $mean }, TLG { $tlg }
tmtv-outside = The region is outside the volume
tmtv-no-pet = TMTV needs a PET volume with SUV

analysis-mpr-only = Fusion and TMTV work in MPR (Shift+R)
curve-not-dynamic = Not a multi-frame image or a dynamic series
curve-not-mpr = Time-intensity curves are drawn on the image, outside MPR
curve-no-roi = Draw an ellipse or rectangle first
curve-loading = { $count ->
        [one] Reading { $count } time point…
       *[other] Reading { $count } time points…
    }
curve-title = Time-intensity curve
curve-mean = Mean
curve-max = Max
curve-seconds = s
curve-saved = Curve saved
curve-none = No curve to save
curve-failed = Curve not made: { $reason }
