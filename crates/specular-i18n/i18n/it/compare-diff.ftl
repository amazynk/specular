# Compare: split, signed difference, flicker, alignment, metrics and the pixel probe
# (DESIGN.md §6.3).

## Actions (palette, cheat sheet, remote control)
palette-compare-with = Confronta con…
palette-compare-with-help = Confronta questa immagine con un’altra a tua scelta.
palette-compare-side = Confronta affiancate
palette-compare-side-help = Mostra le immagini confrontate una accanto all’altra.
palette-compare-split = Confronta con divisione
palette-compare-split-help = Mostra la seconda immagine sopra la prima, tagliata da un divisore trascinabile.
palette-compare-split-direction = Ruota il divisore
palette-compare-split-direction-help = Metti il divisore in orizzontale invece che in verticale, o torna indietro.
palette-compare-overlay = Confronta in sovrapposizione
palette-compare-overlay-help = Mostra la seconda immagine sopra la prima, in dissolvenza.
palette-compare-difference = Confronta la differenza
palette-compare-difference-help = Mostra dove le immagini differiscono; di nuovo per il tipo successivo.
palette-compare-flicker = Alterna le immagini confrontate
palette-compare-flicker-help = Mostra subito l’altra immagine.
palette-compare-auto-flicker = Alternanza automatica
palette-compare-auto-flicker-help = Alterna in automatico le due immagini, o ferma l’alternanza.
palette-compare-align = Allinea le immagini confrontate
palette-compare-align-help = Allinea immagini di dimensioni diverse: in alto a sinistra, centrate, scalate o trovate in automatico.
palette-compare-metric-space = Cambia spazio di misura
palette-compare-metric-space-help = Misura in luce lineare o in sRGB codificato.
palette-compare-whole-image = Misura l’intera immagine
palette-compare-whole-image-help = Misura l’intera immagine invece della parte visibile, o torna indietro.
palette-compare-probe = Sonda pixel del confronto
palette-compare-probe-help = Mostra i valori di entrambe le immagini sotto il puntatore.
palette-compare-link = Collega o scollega le immagini confrontate
palette-compare-link-help = Ingrandisci e sposta ciascuna delle due immagini da sola, o di nuovo insieme.
palette-compare-close = Chiudi il confronto
palette-compare-close-help = Torna alla galleria.
disabled-compare-closed = Apri prima il confronto
disabled-compare-not-two = Funziona con due immagini

## Cheat sheet
cheatsheet-compare-split = Divisione o affiancate
cheatsheet-compare-split-direction = Divisore orizzontale o verticale
cheatsheet-compare-auto-flicker = Alternanza automatica
cheatsheet-compare-align = Allineamento successivo
cheatsheet-compare-space = Spazio di misura
cheatsheet-compare-whole = Intera immagine o parte visibile
cheatsheet-compare-probe = Sonda pixel
cheatsheet-compare-link = Collega o scollega due immagini

## Settings
settings-gallery-compare-metric-space = Spazio di misura del confronto
settings-gallery-compare-metric-space-help = Dove il confronto misura MAE, MSE, PSNR e SSIM. La luce lineare è ciò che mostra lo schermo. sRGB coincide con altri strumenti sui file a 8-bit.
settings-gallery-compare-metric-space-linear = Luce lineare
settings-gallery-compare-metric-space-srgb = sRGB codificato
settings-gallery-compare-flicker-limit = Alternanza automatica più rapida
settings-gallery-compare-flicker-limit-help = Cambi di immagine al secondo al massimo. Oltre 3, il lampeggio può scatenare crisi epilettiche.

## Toolbar and captions
compare-split = Divisione
compare-split-down = Divisore verticale
compare-split-across = Divisore orizzontale
compare-link = Collegate
compare-auto-flicker = Auto
compare-flicker-rate = { $rate } cambi al secondo
compare-gain = Guadagno ×{ $gain }
compare-probe = Sonda
compare-kind-signed = Errore con segno
compare-kind-on-image = Errore sull’immagine
compare-align-top-left = In alto a sinistra
compare-align-centre = Centrate
compare-align-scaled = B scalata su A
compare-align-auto = Allineamento automatico
compare-space-linear = Luce lineare
compare-space-srgb = sRGB
compare-space-help-linear = Misurato in luce lineare, come visualizzato dopo la mappatura dei toni. Qui il PSNR non è il consueto PSNR sRGB a 8-bit.
compare-space-help-srgb = Misurato sui valori sRGB codificati, come fanno altri strumenti per i file a 8-bit. L’HDR sopra il bianco SDR conta come bianco.
compare-space-help-common = ΔE2000 è sempre CIELAB (D65), con il bianco SDR come L* 100 e l’HDR tagliato. Le immagini oltre { $size } pixel sono misurate su una copia di { $size } pixel.
compare-region-view = Parte visibile
compare-region-whole = Intera immagine
compare-ssim = SSIM { $ssim }
compare-ssim-small = SSIM richiede 11 pixel
compare-delta-e = ΔE { $mean } ({ PERCENT(95) }: { $p95 }, max { $max })
compare-delta-e-hdr = HDR tagliato per ΔE
compare-aligned-centre = Centrate, { $width } × { $height } confrontati
compare-aligned-scaled = B scalata { $scale }×, { $width } × { $height } confrontati
compare-aligned-auto = Allineate { $angle }° { $scale }×, { $width } × { $height } confrontati
compare-aligning = Allineamento…
compare-align-failed = Impossibile allineare queste immagini. Si torna in alto a sinistra.
compare-split-label = { $b } su { $a }, divisore al { PERCENT($percent) }
compare-showing-which = Mostra { $which }: { $name }
compare-auto-flicker-on = Alternanza, { $rate } cambi al secondo
compare-auto-flicker-off = Alternanza fermata
compare-linked = Immagini collegate
compare-unlinked = Immagini scollegate: ognuna si sposta da sola
compare-with-title = Confronta con
compare-probe-delta-e = ΔE2000
compare-probe-a11y = A: { $a }. B: { $b }. ΔE2000 { $delta }.
compare-a11y-divider = Divisore
compare-a11y-space = Spazio di misura
compare-a11y-align = Allineamento
