# Specular — Medical analysis: segmentations, SUV, fusion, TMTV, time-intensity curves. Italiano. machine-drafted, needs native review

palette-segments = Mostra o nascondi segmentazioni
palette-segment-opacity = Opacità di riempimento delle segmentazioni successiva
palette-fusion = Fondi le serie dello stesso sistema di riferimento
palette-fusion-colormap = Mappa colori di fusione successiva
palette-fusion-blend = Opacità di fusione successiva
palette-fusion-nudge = Sposta a mano la serie fusa
palette-tmtv = Strumento TMTV (sfera, riquadro, spento)
palette-tmtv-threshold = Soglia TMTV (SUV { NUMBER(2.5) } o { PERCENT(41) } del SUVmax)
palette-time-curve = Curva tempo-intensità dell’ultima regione
palette-export-curve = Salva la curva tempo-intensità come CSV

analysis-searching = Ricerca di segmentazioni e serie da fondere in corso
segments-none = Nessuna segmentazione trovata per questo sistema di riferimento
segments-shown = Segmentazioni visibili
segments-hidden = Segmentazioni nascoste
segments-fill = Riempimento segmentazioni { PERCENT($percent) }
segments-failed = Segmentazione non mostrata: { $reason }
segment-entry = { $name } · { $ml } mL

suv-value = SUVbw { $value }
suv-factor = Fattore SUVbw { $factor }, { $minutes } min dopo l’iniezione
suv-refused = Nessun SUV: { $reason }
suv-preset = SUV 0–{ $upper }
suv-refused-not-pet = non è un’immagine PET
suv-refused-units = le unità dei pixel non sono Bq/mL
suv-refused-corrected = non corretta per attenuazione e decadimento
suv-refused-decay = correzione del decadimento non supportata
suv-refused-weight = manca il peso del paziente
suv-refused-dose = manca la dose iniettata
suv-refused-half-life = manca l’emivita del radionuclide
suv-refused-injection = manca l’ora dell’iniezione
suv-refused-scan-time = manca l’ora di acquisizione
suv-refused-delay = l’iniezione è successiva alla scansione
suv-refused-scale = manca il fattore di scala SUV Philips
suv-refused-height = manca l’altezza del paziente
suv-refused-sex = manca il sesso del paziente
suv-refused-lbm = massa magra fuori intervallo

colormap-hot-iron = Ferro rovente
colormap-rainbow = Arcobaleno
colormap-viridis = Viridis
fusion-none = Nessuna serie PET, SPECT, CT o MR di questo sistema di riferimento da fondere
fusion-needed = Prima fondi una serie (Shift+U)
fusion-opening = Fusione di { $modality }
fusion-off = Fusione disattivata
fusion-refused = Impossibile fondere: { $reason }
fusion-on = { $modality } fusa: { $colormap }, { PERCENT($blend) }
fusion-hidden = { $modality } fusa nascosta
fusion-window-suv = Finestra fusa SUV { $low }–{ $high }
fusion-value = { $modality } { $value }
fusion-value-suv = { $modality } SUVbw { $value }
fusion-blend-toast = Opacità di fusione { PERCENT($percent) }
fusion-moved = Spostata a mano: { $x }, { $y }, { $z } mm, { $degrees }°
fusion-nudge-hint = Le frecce spostano la serie fusa di 1 mm, Shift+Up/Down lungo la vista, [ e ] la ruotano
fusion-nudge-off = Serie fusa fissata

tmtv-hint-sphere = TMTV: trascina una sfera dal centro
tmtv-hint-box = TMTV: trascina un riquadro
tmtv-off = TMTV disattivato
tmtv-threshold-suv = Soglia SUV { $suv }
tmtv-threshold-share = Soglia { PERCENT($share) } del SUVmax
tmtv-result = TMTV { $ml } mL in { $count ->
        [one] { $count } regione
       *[other] { $count } regioni
    }
tmtv-stats = SUVmax { $max }, SUVmean { $mean }, TLG { $tlg }
tmtv-outside = La regione è fuori dal volume
tmtv-no-pet = Il TMTV richiede un volume PET con SUV

analysis-mpr-only = Fusione e TMTV funzionano in MPR (Shift+R)
curve-not-dynamic = Non è un’immagine multiframe né una serie dinamica
curve-not-mpr = Le curve tempo-intensità si tracciano sull’immagine, fuori dall’MPR
curve-no-roi = Disegna prima un’ellisse o un rettangolo
curve-loading = { $count ->
        [one] Lettura di { $count } istante temporale…
       *[other] Lettura di { $count } istanti temporali…
    }
curve-title = Curva tempo-intensità
curve-mean = Media
curve-max = Max
curve-seconds = s
curve-saved = Curva salvata
curve-none = Nessuna curva da salvare
curve-failed = Curva non creata: { $reason }
