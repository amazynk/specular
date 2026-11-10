# Specular — Medical analysis: segmentations, SUV, fusion, TMTV, time-intensity curves. Svenska. machine-drafted, needs native review

palette-segments = Visa eller dölj segmenteringar
palette-segment-opacity = Nästa opacitet för segmenteringsfyllning
palette-fusion = Fusionera serierna med samma referensram
palette-fusion-colormap = Nästa färgkarta för fusion
palette-fusion-blend = Nästa opacitet för fusion
palette-fusion-nudge = Flytta den fusionerade serien för hand
palette-tmtv = TMTV-verktyg (sfär, låda, av)
palette-tmtv-threshold = TMTV-tröskel (SUV { NUMBER(2.5) } eller { PERCENT(41) } av SUVmax)
palette-time-curve = Tid–intensitetskurva för senaste området
palette-export-curve = Spara tid–intensitetskurvan som CSV

analysis-searching = Letar fortfarande efter segmenteringar och serier att fusionera
segments-none = Ingen segmentering av den här referensramen hittades
segments-shown = Segmenteringar visas
segments-hidden = Segmenteringar dolda
segments-fill = Segmenteringsfyllning { PERCENT($percent) }
segments-failed = Segmenteringen visas inte: { $reason }
segment-entry = { $name } · { $ml } mL

suv-value = SUVbw { $value }
suv-factor = SUVbw-faktor { $factor }, { $minutes } min efter injektion
suv-refused = Inget SUV: { $reason }
suv-preset = SUV 0–{ $upper }
suv-refused-not-pet = inte en PET-bild
suv-refused-units = pixelenheterna är inte Bq/mL
suv-refused-corrected = inte korrigerad för attenuering och sönderfall
suv-refused-decay = sönderfallskorrigering som inte stöds
suv-refused-weight = ingen patientvikt
suv-refused-dose = ingen injicerad dos
suv-refused-half-life = ingen halveringstid för radionukliden
suv-refused-injection = ingen injektionstid
suv-refused-scan-time = ingen förvärvstid
suv-refused-delay = injektionen är efter undersökningen
suv-refused-scale = ingen SUV-skalfaktor från Philips
suv-refused-height = ingen patientlängd
suv-refused-sex = inget patientkön
suv-refused-lbm = fettfri massa utanför intervallet

colormap-hot-iron = Glödande järn
colormap-rainbow = Regnbåge
colormap-viridis = Viridis
fusion-none = Ingen PET-, SPECT-, CT- eller MR-serie med den här referensramen att fusionera
fusion-needed = Fusionera en serie först (Shift+U)
fusion-opening = Fusionerar { $modality }
fusion-off = Fusion av
fusion-refused = Kan inte fusionera: { $reason }
fusion-on = Fusionerad { $modality }: { $colormap }, { PERCENT($blend) }
fusion-hidden = Fusionerad { $modality } dold
fusion-window-suv = Fusionsfönster SUV { $low }–{ $high }
fusion-value = { $modality } { $value }
fusion-value-suv = { $modality } SUVbw { $value }
fusion-blend-toast = Fusionsopacitet { PERCENT($percent) }
fusion-moved = Flyttad för hand: { $x }, { $y }, { $z } mm, { $degrees }°
fusion-nudge-hint = Pilarna flyttar den fusionerade serien 1 mm, Shift+Up/Down längs vyn, [ och ] vrider den
fusion-nudge-off = Fusionerad serie låst

tmtv-hint-sphere = TMTV: dra en sfär från dess mitt
tmtv-hint-box = TMTV: dra en låda
tmtv-off = TMTV av
tmtv-threshold-suv = Tröskel SUV { $suv }
tmtv-threshold-share = Tröskel { PERCENT($share) } av SUVmax
tmtv-result = TMTV { $ml } mL i { $count ->
        [one] { $count } område
       *[other] { $count } områden
    }
tmtv-stats = SUVmax { $max }, SUVmean { $mean }, TLG { $tlg }
tmtv-outside = Området ligger utanför volymen
tmtv-no-pet = TMTV kräver en PET-volym med SUV

analysis-mpr-only = Fusion och TMTV fungerar i MPR (Shift+R)
curve-not-dynamic = Inte en bild med flera bildrutor eller en dynamisk serie
curve-not-mpr = Tid–intensitetskurvor ritas på bilden, utanför MPR
curve-no-roi = Rita en ellips eller rektangel först
curve-loading = { $count ->
        [one] Läser { $count } tidpunkt…
       *[other] Läser { $count } tidpunkter…
    }
curve-title = Tid–intensitetskurva
curve-mean = Medel
curve-max = Max
curve-seconds = s
curve-saved = Kurvan sparad
curve-none = Ingen kurva att spara
curve-failed = Kurvan skapades inte: { $reason }
