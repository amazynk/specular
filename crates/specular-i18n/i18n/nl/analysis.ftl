# Specular — Medical analysis: segmentations, SUV, fusion, TMTV, time-intensity curves. Nederlands. machine-drafted, needs native review

palette-segments = Segmentaties tonen of verbergen
palette-segment-opacity = Volgende dekking van de segmentatievulling
palette-fusion = Series met hetzelfde referentiekader fuseren
palette-fusion-colormap = Volgende fusiekleurenkaart
palette-fusion-blend = Volgende fusiedekking
palette-fusion-nudge = Gefuseerde serie met de hand verschuiven
palette-tmtv = TMTV-gereedschap (bol, kader, uit)
palette-tmtv-threshold = TMTV-drempel (SUV { NUMBER(2.5) } of { PERCENT(41) } van SUVmax)
palette-time-curve = Tijd-intensiteitscurve van het laatste gebied
palette-export-curve = Tijd-intensiteitscurve opslaan als CSV

analysis-searching = Nog op zoek naar segmentaties en series om te fuseren
segments-none = Geen segmentatie van dit referentiekader gevonden
segments-shown = Segmentaties getoond
segments-hidden = Segmentaties verborgen
segments-fill = Segmentatievulling { PERCENT($percent) }
segments-failed = Segmentatie niet getoond: { $reason }
segment-entry = { $name } · { $ml } mL

suv-value = SUVbw { $value }
suv-factor = SUVbw-factor { $factor }, { $minutes } min na injectie
suv-refused = Geen SUV: { $reason }
suv-preset = SUV 0–{ $upper }
suv-refused-not-pet = geen PET-beeld
suv-refused-units = de pixeleenheid is niet Bq/mL
suv-refused-corrected = niet gecorrigeerd voor attenuatie en verval
suv-refused-decay = niet-ondersteunde vervalcorrectie
suv-refused-weight = geen patiëntgewicht
suv-refused-dose = geen geïnjecteerde dosis
suv-refused-half-life = geen halveringstijd van het radionuclide
suv-refused-injection = geen injectietijd
suv-refused-scan-time = geen acquisitietijd
suv-refused-delay = de injectie is na de scan
suv-refused-scale = geen Philips-SUV-schaalfactor
suv-refused-height = geen patiëntlengte
suv-refused-sex = geen geslacht van de patiënt
suv-refused-lbm = vetvrije massa buiten bereik

colormap-hot-iron = Gloeiend ijzer
colormap-rainbow = Regenboog
colormap-viridis = Viridis
fusion-none = Geen PET-, SPECT-, CT- of MR-serie van dit referentiekader om te fuseren
fusion-needed = Fuseer eerst een serie (Shift+U)
fusion-opening = { $modality } fuseren
fusion-off = Fusie uit
fusion-refused = Kan niet fuseren: { $reason }
fusion-on = { $modality } gefuseerd: { $colormap }, { PERCENT($blend) }
fusion-hidden = Gefuseerde { $modality } verborgen
fusion-window-suv = Gefuseerd venster SUV { $low }–{ $high }
fusion-value = { $modality } { $value }
fusion-value-suv = { $modality } SUVbw { $value }
fusion-blend-toast = Fusiedekking { PERCENT($percent) }
fusion-moved = Met de hand verschoven: { $x }, { $y }, { $z } mm, { $degrees }°
fusion-nudge-hint = Pijltjes verschuiven de gefuseerde serie 1 mm, Shift+Up/Down langs de weergave, [ en ] draaien
fusion-nudge-off = Gefuseerde serie vastgezet

tmtv-hint-sphere = TMTV: sleep een bol vanuit het midden
tmtv-hint-box = TMTV: sleep een kader
tmtv-off = TMTV uit
tmtv-threshold-suv = Drempel SUV { $suv }
tmtv-threshold-share = Drempel { PERCENT($share) } van SUVmax
tmtv-result = TMTV { $ml } mL in { $count ->
        [one] { $count } gebied
       *[other] { $count } gebieden
    }
tmtv-stats = SUVmax { $max }, SUVmean { $mean }, TLG { $tlg }
tmtv-outside = Het gebied ligt buiten het volume
tmtv-no-pet = TMTV heeft een PET-volume met SUV nodig

analysis-mpr-only = Fusie en TMTV werken in MPR (Shift+R)
curve-not-dynamic = Geen afbeelding met meerdere frames of dynamische serie
curve-not-mpr = Tijd-intensiteitscurven worden op de afbeelding getekend, buiten MPR
curve-no-roi = Teken eerst een ellips of rechthoek
curve-loading = { $count ->
        [one] { $count } tijdpunt lezen…
       *[other] { $count } tijdpunten lezen…
    }
curve-title = Tijd-intensiteitscurve
curve-mean = Gemiddelde
curve-max = Max
curve-seconds = s
curve-saved = Curve opgeslagen
curve-none = Geen curve om op te slaan
curve-failed = Curve niet gemaakt: { $reason }
