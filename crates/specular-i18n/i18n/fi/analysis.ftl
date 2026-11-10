# Specular — Medical analysis: segmentations, SUV, fusion, TMTV, time-intensity curves. Suomi. machine-drafted, needs native review

palette-segments = Näytä tai piilota segmentoinnit
palette-segment-opacity = Seuraava segmentoinnin täytön peittävyys
palette-fusion = Fuusioi saman viitekehyksen sarjat
palette-fusion-colormap = Seuraava fuusion värikartta
palette-fusion-blend = Seuraava fuusion peittävyys
palette-fusion-nudge = Siirrä fuusioitua sarjaa käsin
palette-tmtv = TMTV-työkalu (pallo, laatikko, pois)
palette-tmtv-threshold = TMTV-kynnys (SUV { NUMBER(2.5) } tai { PERCENT(41) } SUVmax-arvosta)
palette-time-curve = Viimeisimmän alueen aika-intensiteettikäyrä
palette-export-curve = Tallenna aika-intensiteettikäyrä CSV-muodossa

analysis-searching = Etsitään vielä segmentointeja ja fuusioitavia sarjoja
segments-none = Tämän viitekehyksen segmentointia ei löytynyt
segments-shown = Segmentoinnit näkyvissä
segments-hidden = Segmentoinnit piilotettu
segments-fill = Segmentoinnin täyttö { PERCENT($percent) }
segments-failed = Segmentointia ei näytetty: { $reason }
segment-entry = { $name } · { $ml } mL

suv-value = SUVbw { $value }
suv-factor = SUVbw-kerroin { $factor }, { $minutes } min injektion jälkeen
suv-refused = Ei SUV-arvoa: { $reason }
suv-preset = SUV 0–{ $upper }
suv-refused-not-pet = ei PET-kuva
suv-refused-units = pikseliyksikkö ei ole Bq/mL
suv-refused-corrected = ei vaimennus- ja hajoamiskorjattu
suv-refused-decay = hajoamiskorjausta ei tueta
suv-refused-weight = potilaan paino puuttuu
suv-refused-dose = injektoitu annos puuttuu
suv-refused-half-life = radionuklidin puoliintumisaika puuttuu
suv-refused-injection = injektioaika puuttuu
suv-refused-scan-time = kuvausaika puuttuu
suv-refused-delay = injektio on kuvauksen jälkeen
suv-refused-scale = Philipsin SUV-skaalauskerroin puuttuu
suv-refused-height = potilaan pituus puuttuu
suv-refused-sex = potilaan sukupuoli puuttuu
suv-refused-lbm = rasvaton massa sallitun alueen ulkopuolella

colormap-hot-iron = Hehkuva rauta
colormap-rainbow = Sateenkaari
colormap-viridis = Viridis
fusion-none = Tässä viitekehyksessä ei ole fuusioitavaa PET-, SPECT-, CT- tai MR-sarjaa
fusion-needed = Fuusioi ensin sarja (Shift+U)
fusion-opening = Fuusioidaan { $modality }
fusion-off = Fuusio pois
fusion-refused = Fuusio ei onnistu: { $reason }
fusion-on = Fuusioitu { $modality }: { $colormap }, { PERCENT($blend) }
fusion-hidden = Fuusioitu { $modality } piilotettu
fusion-window-suv = Fuusion ikkuna SUV { $low }–{ $high }
fusion-value = { $modality } { $value }
fusion-value-suv = { $modality } SUVbw { $value }
fusion-blend-toast = Fuusion peittävyys { PERCENT($percent) }
fusion-moved = Siirretty käsin: { $x }, { $y }, { $z } mm, { $degrees }°
fusion-nudge-hint = Nuolet siirtävät fuusioitua sarjaa 1 mm, Shift+Up/Down näkymän suunnassa, [ ja ] kääntävät sitä
fusion-nudge-off = Fuusioitu sarja lukittu

tmtv-hint-sphere = TMTV: vedä pallo keskipisteestä
tmtv-hint-box = TMTV: vedä laatikko
tmtv-off = TMTV pois
tmtv-threshold-suv = Kynnys SUV { $suv }
tmtv-threshold-share = Kynnys { PERCENT($share) } SUVmax-arvosta
tmtv-result = TMTV { $ml } mL, { $count ->
        [one] { $count } alue
       *[other] { $count } aluetta
    }
tmtv-stats = SUVmax { $max }, SUVmean { $mean }, TLG { $tlg }
tmtv-outside = Alue on tilavuuden ulkopuolella
tmtv-no-pet = TMTV vaatii PET-tilavuuden, jossa on SUV

analysis-mpr-only = Fuusio ja TMTV toimivat MPR:ssä (Shift+R)
curve-not-dynamic = Ei monikehyksinen kuva eikä dynaaminen sarja
curve-not-mpr = Aika-intensiteettikäyrät piirretään kuvaan, MPR:n ulkopuolella
curve-no-roi = Piirrä ensin ellipsi tai suorakulmio
curve-loading = { $count ->
        [one] Luetaan { $count } aikapiste…
       *[other] Luetaan { $count } aikapistettä…
    }
curve-title = Aika-intensiteettikäyrä
curve-mean = Keskiarvo
curve-max = Maks.
curve-seconds = s
curve-saved = Käyrä tallennettu
curve-none = Ei tallennettavaa käyrää
curve-failed = Käyrää ei luotu: { $reason }
