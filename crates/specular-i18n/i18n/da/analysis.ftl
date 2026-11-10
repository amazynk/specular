# Specular — Medical analysis: segmentations, SUV, fusion, TMTV, time-intensity curves. Dansk. machine-drafted, needs native review

palette-segments = Vis eller skjul segmenteringer
palette-segment-opacity = Næste dækevne for segmenteringsfyld
palette-fusion = Fusionér serien med samme referenceramme
palette-fusion-colormap = Næste farvekort for fusion
palette-fusion-blend = Næste dækevne for fusion
palette-fusion-nudge = Flyt den fusionerede serie manuelt
palette-tmtv = TMTV-værktøj (kugle, kasse, fra)
palette-tmtv-threshold = TMTV-tærskel (SUV { NUMBER(2.5) } eller { PERCENT(41) } af SUVmax)
palette-time-curve = Tid-intensitet-kurve for det seneste område
palette-export-curve = Gem tid-intensitet-kurven som CSV

analysis-searching = Leder stadig efter segmenteringer og serier at fusionere
segments-none = Der blev ikke fundet nogen segmentering for denne referenceramme
segments-shown = Segmenteringer vist
segments-hidden = Segmenteringer skjult
segments-fill = Segmenteringsfyld { PERCENT($percent) }
segments-failed = Segmentering ikke vist: { $reason }
segment-entry = { $name } · { $ml } mL

suv-value = SUVbw { $value }
suv-factor = SUVbw-faktor { $factor }, { $minutes } min efter injektion
suv-refused = Ingen SUV: { $reason }
suv-preset = SUV 0–{ $upper }
suv-refused-not-pet = ikke et PET-billede
suv-refused-units = pixelenhederne er ikke Bq/mL
suv-refused-corrected = ikke korrigeret for attenuering og henfald
suv-refused-decay = henfaldskorrektion understøttes ikke
suv-refused-weight = ingen patientvægt
suv-refused-dose = ingen injiceret dosis
suv-refused-half-life = ingen halveringstid for radionuklidet
suv-refused-injection = intet injektionstidspunkt
suv-refused-scan-time = intet optagelsestidspunkt
suv-refused-delay = injektionen er efter skanningen
suv-refused-scale = ingen Philips SUV-skaleringsfaktor
suv-refused-height = ingen patienthøjde
suv-refused-sex = intet patientkøn
suv-refused-lbm = fedtfri masse uden for området

colormap-hot-iron = Glødende jern
colormap-rainbow = Regnbue
colormap-viridis = Viridis
fusion-none = Ingen PET-, SPECT-, CT- eller MR-serie med denne referenceramme at fusionere
fusion-needed = Fusionér først en serie (Shift+U)
fusion-opening = Fusionerer { $modality }
fusion-off = Fusion fra
fusion-refused = Kan ikke fusionere: { $reason }
fusion-on = Fusioneret { $modality }: { $colormap }, { PERCENT($blend) }
fusion-hidden = Fusioneret { $modality } skjult
fusion-window-suv = Fusionsvindue SUV { $low }–{ $high }
fusion-value = { $modality } { $value }
fusion-value-suv = { $modality } SUVbw { $value }
fusion-blend-toast = Fusionsdækevne { PERCENT($percent) }
fusion-moved = Flyttet manuelt: { $x }, { $y }, { $z } mm, { $degrees }°
fusion-nudge-hint = Pile flytter den fusionerede serie 1 mm, Shift+Up/Down langs visningen, [ og ] drejer den
fusion-nudge-off = Fusioneret serie låst

tmtv-hint-sphere = TMTV: træk en kugle fra dens centrum
tmtv-hint-box = TMTV: træk en kasse
tmtv-off = TMTV fra
tmtv-threshold-suv = Tærskel SUV { $suv }
tmtv-threshold-share = Tærskel { PERCENT($share) } af SUVmax
tmtv-result = TMTV { $ml } mL i { $count ->
        [one] { $count } område
       *[other] { $count } områder
    }
tmtv-stats = SUVmax { $max }, SUVmean { $mean }, TLG { $tlg }
tmtv-outside = Området ligger uden for volumenet
tmtv-no-pet = TMTV kræver et PET-volumen med SUV

analysis-mpr-only = Fusion og TMTV virker i MPR (Shift+R)
curve-not-dynamic = Ikke et billede med flere billeder eller en dynamisk serie
curve-not-mpr = Tid-intensitet-kurver tegnes på billedet, uden for MPR
curve-no-roi = Tegn først en ellipse eller et rektangel
curve-loading = { $count ->
        [one] Læser { $count } tidspunkt…
       *[other] Læser { $count } tidspunkter…
    }
curve-title = Tid-intensitet-kurve
curve-mean = Middel
curve-max = Maks.
curve-seconds = s
curve-saved = Kurve gemt
curve-none = Ingen kurve at gemme
curve-failed = Kurve ikke lavet: { $reason }
