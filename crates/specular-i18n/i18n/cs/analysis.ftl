# Specular — Medical analysis: segmentations, SUV, fusion, TMTV, time-intensity curves. Čeština. machine-drafted, needs native review

palette-segments = Zobrazit nebo skrýt segmentace
palette-segment-opacity = Další krytí výplně segmentace
palette-fusion = Fúzovat sérii stejného referenčního rámce
palette-fusion-colormap = Další barevná mapa fúze
palette-fusion-blend = Další krytí fúze
palette-fusion-nudge = Posunout fúzovanou sérii ručně
palette-tmtv = Nástroj TMTV (koule, kvádr, vypnuto)
palette-tmtv-threshold = Práh TMTV (SUV { NUMBER(2.5) } nebo { PERCENT(41) } SUVmax)
palette-time-curve = Křivka intenzita–čas poslední oblasti
palette-export-curve = Uložit křivku intenzita–čas jako CSV

analysis-searching = Stále hledám segmentace a série k fúzi
segments-none = Pro tento referenční rámec nebyla nalezena žádná segmentace
segments-shown = Segmentace zobrazeny
segments-hidden = Segmentace skryty
segments-fill = Výplň segmentace { PERCENT($percent) }
segments-failed = Segmentace nezobrazena: { $reason }
segment-entry = { $name } · { $ml } mL

suv-value = SUVbw { $value }
suv-factor = Faktor SUVbw { $factor }, { $minutes } min po injekci
suv-refused = Bez SUV: { $reason }
suv-preset = SUV 0–{ $upper }
suv-refused-not-pet = nejde o snímek PET
suv-refused-units = jednotky pixelů nejsou Bq/mL
suv-refused-corrected = bez korekce na atenuaci a rozpad
suv-refused-decay = nepodporovaná korekce rozpadu
suv-refused-weight = chybí hmotnost pacienta
suv-refused-dose = chybí podaná dávka
suv-refused-half-life = chybí poločas radionuklidu
suv-refused-injection = chybí čas injekce
suv-refused-scan-time = chybí čas akvizice
suv-refused-delay = injekce je až po snímání
suv-refused-scale = chybí faktor SUV od Philips
suv-refused-height = chybí výška pacienta
suv-refused-sex = chybí pohlaví pacienta
suv-refused-lbm = čistá tělesná hmotnost mimo rozsah

colormap-hot-iron = Žhavé železo
colormap-rainbow = Duha
colormap-viridis = Viridis
fusion-none = Žádná série PET, SPECT, CT ani MR tohoto referenčního rámce k fúzi
fusion-needed = Nejprve fúzujte sérii (Shift+U)
fusion-opening = Fúze { $modality }
fusion-off = Fúze vypnuta
fusion-refused = Nelze fúzovat: { $reason }
fusion-on = Fúzováno { $modality }: { $colormap }, { PERCENT($blend) }
fusion-hidden = Fúzované { $modality } skryto
fusion-window-suv = Okno fúze SUV { $low }–{ $high }
fusion-value = { $modality } { $value }
fusion-value-suv = { $modality } SUVbw { $value }
fusion-blend-toast = Krytí fúze { PERCENT($percent) }
fusion-moved = Posunuto ručně: { $x }, { $y }, { $z } mm, { $degrees }°
fusion-nudge-hint = Šipky posunou fúzovanou sérii o 1 mm, Shift+Up/Down podél zobrazení, [ a ] ji otáčí
fusion-nudge-off = Fúzovaná série ukotvena

tmtv-hint-sphere = TMTV: táhněte kouli od středu
tmtv-hint-box = TMTV: táhněte kvádr
tmtv-off = TMTV vypnuto
tmtv-threshold-suv = Práh SUV { $suv }
tmtv-threshold-share = Práh { PERCENT($share) } SUVmax
tmtv-result = TMTV { $ml } mL v { $count ->
        [one] { $count } oblasti
        [few] { $count } oblastech
        [many] { $count } oblasti
       *[other] { $count } oblastech
    }
tmtv-stats = SUVmax { $max }, SUVmean { $mean }, TLG { $tlg }
tmtv-outside = Oblast leží mimo objem
tmtv-no-pet = TMTV vyžaduje objem PET se SUV

analysis-mpr-only = Fúze a TMTV fungují v MPR (Shift+R)
curve-not-dynamic = Nejde o vícesnímkový obrázek ani dynamickou sérii
curve-not-mpr = Křivky intenzita–čas se kreslí na obrázku, mimo MPR
curve-no-roi = Nejprve nakreslete elipsu nebo obdélník
curve-loading = { $count ->
        [one] Načítání { $count } časového bodu…
        [few] Načítání { $count } časových bodů…
        [many] Načítání { $count } časového bodu…
       *[other] Načítání { $count } časových bodů…
    }
curve-title = Křivka intenzita–čas
curve-mean = Průměr
curve-max = Max.
curve-seconds = s
curve-saved = Křivka uložena
curve-none = Žádná křivka k uložení
curve-failed = Křivka nevytvořena: { $reason }
