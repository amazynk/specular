# machine-drafted, needs native review
# Specular — Develop (non-destructive photo editing). Italiano.

# Shown once when an edit opens and some of its modules were made with a version that no
# longer renders: they were moved to the nearest version that does. $count: modules moved.
# machine-drafted, needs native review
toast-develop-edit-migrated = { $count ->
    [one] Un modulo di questa modifica è stato aggiornato a una versione più recente. Il risultato potrebbe apparire leggermente diverso.
    [many] { $count } moduli di questa modifica sono stati aggiornati a una versione più recente. Il risultato potrebbe apparire leggermente diverso.
   *[other] { $count } moduli di questa modifica sono stati aggiornati a una versione più recente. Il risultato potrebbe apparire leggermente diverso.
}
# Shown when an edit holds modules this version of Specular cannot render (made with a newer
# Specular). They are kept and saved unchanged, but skipped. $count: modules skipped.
# machine-drafted, needs native review
toast-develop-edit-newer = { $count ->
    [one] Un modulo di questa modifica richiede una versione più recente di Specular. Viene conservato ma non mostrato.
    [many] { $count } moduli di questa modifica richiedono una versione più recente di Specular. Vengono conservati ma non mostrati.
   *[other] { $count } moduli di questa modifica richiedono una versione più recente di Specular. Vengono conservati ma non mostrati.
}

## Develop's controls (sliders, curve, colour wheels). Numbers come formatted in the UI
## language's digits.

# machine-drafted, needs native review
# A slider's value in exposure values. $value: "+0.70".
develop-value-ev = { $value } EV
# machine-drafted, needs native review
# machine-drafted, needs native review
# An angle. $value: "2.5".
develop-value-degrees = { $value }°
# machine-drafted, needs native review
# A colour temperature. $value: "5,000".
develop-value-kelvin = { $value } K
# machine-drafted, needs native review
# The curve editor's selected point. $index, $count: point numbers; $input, $output: "25 %".
develop-curve-point = Punto { $index } di { $count }: ingresso { $input }, uscita { $output }
# machine-drafted, needs native review
# The curve editor's keys, read by screen readers.
develop-curve-keys = Le frecce spostano il punto. Pag su e Pag giù ne scelgono un altro. Ins aggiunge un punto, Canc lo rimuove.
# machine-drafted, needs native review
# A colour wheel with no colour.
develop-wheel-neutral = Neutro
# machine-drafted, needs native review
# A colour wheel's colour. $hue: degrees; $strength: "20 %".
develop-wheel-value = Tonalità { $hue }°, intensità { $strength }
# machine-drafted, needs native review
# A colour wheel's keys, read by screen readers.
develop-wheel-keys = Le frecce spostano il colore. Canc lo azzera.

## Develop's panel.

# machine-drafted, needs native review
develop-title = Sviluppo
# machine-drafted, needs native review
develop-undo = Annulla
# machine-drafted, needs native review
develop-redo = Ripeti
# machine-drafted, needs native review
develop-before-after = Prima e dopo
# machine-drafted, needs native review
develop-close = Esci da Sviluppo
# machine-drafted, needs native review
develop-history = Cronologia
# machine-drafted, needs native review
# A card's on/off switch. $module: the module's name.
develop-module-on = { $module } attivo
develop-card-menu = Opzioni di { $module }
develop-card-changed = Modificato
# machine-drafted, needs native review
# The speed-edit readout over the image. $control: "Exposure"; $value: "+0.70 EV".
develop-speed-hud = { $control }: { $value }
# machine-drafted, needs native review
develop-filmstrip = Pellicola

## Develop's module cards and controls.

# machine-drafted, needs native review
develop-module-exposure = Esposizione
develop-module-light = Luce
develop-look-profile = Profilo
develop-look-standard = Standard
develop-look-film = Pellicola
develop-look-natural = Naturale
develop-module-grading = Gradazione colore
develop-add-tool = Aggiungi uno strumento
# machine-drafted, needs native review
develop-module-white-balance = Bilanciamento del bianco
# machine-drafted, needs native review
develop-module-tone = Tono
# machine-drafted, needs native review
develop-module-contrast = Contrasto e gamma
# machine-drafted, needs native review
develop-module-colour = Colore
# machine-drafted, needs native review
develop-module-curve = Curva
# machine-drafted, needs native review
develop-module-crop = Ritaglio e raddrizzamento
# machine-drafted, needs native review
# A module this version has no card for.
develop-module-other = Altro modulo
# machine-drafted, needs native review
develop-exposure = Esposizione
# machine-drafted, needs native review
develop-black-level = Livello del nero
# machine-drafted, needs native review
develop-temperature = Temperatura
# machine-drafted, needs native review
develop-tint = Tinta
# machine-drafted, needs native review
develop-highlights = Luci
# machine-drafted, needs native review
develop-shadows = Ombre
# machine-drafted, needs native review
develop-contrast = Contrasto
# machine-drafted, needs native review
develop-whites = Bianchi
# machine-drafted, needs native review
develop-blacks = Neri
# machine-drafted, needs native review
# The view transform: how scene brightness becomes screen brightness.
# machine-drafted, needs native review
# machine-drafted, needs native review
# machine-drafted, needs native review
# machine-drafted, needs native review
# HDR card: how far above SDR white the HDR picture may go, and which picture to show.
develop-hdr = HDR
# machine-drafted, needs native review
develop-hdr-limit = Limite HDR
# machine-drafted, needs native review
develop-hdr-preview = Mostra
# machine-drafted, needs native review
develop-hdr-preview-hdr = HDR
# machine-drafted, needs native review
develop-hdr-preview-sdr = SDR
# machine-drafted, needs native review
develop-vibrance = Vividezza
# machine-drafted, needs native review
develop-saturation = Saturazione
# machine-drafted, needs native review
develop-colour-wheels = Ruote colore
# machine-drafted, needs native review
develop-curve = Curva tonale
# machine-drafted, needs native review
develop-module-local-contrast = Contrasto locale
# machine-drafted, needs native review
# A switch: exposure set from the photo's own brightness.
develop-auto-exposure = Esposizione automatica
# machine-drafted, needs native review
# How far the tone equalizer's highlights and shadows spread across an area.
develop-smoothing = Levigatura
# machine-drafted, needs native review
# Local contrast: fine detail, and larger-scale contrast.
develop-texture = Texture
# machine-drafted, needs native review
develop-clarity = Chiarezza
# machine-drafted, needs native review
# Levels on the curve card: the input that becomes black, the middle and white.
develop-levels-black = Punto del nero
# machine-drafted, needs native review
develop-levels-mid = Punto medio
# machine-drafted, needs native review
develop-levels-white = Punto del bianco
# machine-drafted, needs native review
develop-module-detail = Dettaglio
# machine-drafted, needs native review
develop-module-denoise = Riduzione rumore
# machine-drafted, needs native review
develop-module-sharpen = Nitidezza
# machine-drafted, needs native review
develop-noise-luminance = Rumore di luminanza
# machine-drafted, needs native review
develop-noise-detail = Dettaglio rumore
# machine-drafted, needs native review
develop-noise-colour = Rumore colore
# machine-drafted, needs native review
develop-sharpen-amount = Fattore nitidezza
# machine-drafted, needs native review
develop-sharpen-radius = Raggio nitidezza
# machine-drafted, needs native review
develop-sharpen-detail = Dettaglio nitidezza
# machine-drafted, needs native review
develop-sharpen-masking = Maschera
# machine-drafted, needs native review
develop-capture-amount = Nitidezza di acquisizione
# machine-drafted, needs native review
develop-capture-radius = Raggio di acquisizione
# machine-drafted, needs native review
develop-straighten = Raddrizza
# machine-drafted, needs native review
# History rows of the crop tool.
develop-crop = Ritaglia
# machine-drafted, needs native review
develop-crop-turn = Ruota
# machine-drafted, needs native review
develop-crop-flip = Rifletti
# machine-drafted, needs native review
# The lens card: the camera's own corrections (its switch) and manual ones.
develop-module-lens = Correzioni obiettivo
develop-module-optics = Ottica
# machine-drafted, needs native review
develop-lens-distortion = Distorsione
# machine-drafted, needs native review
develop-lens-vignette = Vignettatura
# machine-drafted, needs native review
# Colour fringes at the edges of the frame (lateral chromatic aberration), per colour.
develop-lens-tca-red = Frangia rossa
# machine-drafted, needs native review
develop-lens-tca-blue = Frangia blu
# machine-drafted, needs native review
# The Lensfun lens profile for raws without corrections of their own: a picker and a note.
develop-lensfun-profile = Profilo obiettivo
# machine-drafted, needs native review
# The picker's first option: the profile matched to the file's lens name.
develop-lensfun-auto = Automatico
# machine-drafted, needs native review
develop-lensfun-off = Nessuno
# machine-drafted, needs native review
# $lens: "Canon EF 24-70mm f/2.8L II USM". Lensfun is a name; keep it.
develop-lensfun-used = Lensfun: { $lens }
# machine-drafted, needs native review
develop-lensfun-none-chosen = Nessun profilo obiettivo.
# machine-drafted, needs native review
# $lens: the lens name the file gives.
develop-lensfun-no-match = Nessun profilo Lensfun per { $lens }.
# machine-drafted, needs native review
develop-lensfun-no-profile = Nessun profilo Lensfun per questo obiettivo.
# machine-drafted, needs native review
develop-lensfun-no-name = Il file non indica l’obiettivo. Sceglilo qui sopra.
# machine-drafted, needs native review
# $count: how many lenses match equally well (2 or more).
develop-lensfun-ambiguous = { $count } obiettivi corrispondono. Scegline uno qui sopra.

## The crop tool over the photo (key R). Said to screen readers, or shown over the photo.

# machine-drafted, needs native review
develop-crop-open = Ritaglia. Trascina i bordi, oppure Ctrl+trascina lungo una linea che deve essere orizzontale. Invio conferma, Esc torna indietro.
# machine-drafted, needs native review
develop-crop-kept = Ritaglio confermato
# machine-drafted, needs native review
develop-crop-cancelled = Ritaglio annullato
# machine-drafted, needs native review
develop-crop-reset = Ritaglio e raddrizzamento azzerati
# machine-drafted, needs native review
# $angle: "-1.5".
develop-crop-straightened = Raddrizzata di { $angle }°
# machine-drafted, needs native review
# $aspect: "3:2", or one of the two names below.
develop-crop-aspect = Proporzioni { $aspect }
# machine-drafted, needs native review
develop-crop-aspect-free = libere
# machine-drafted, needs native review
develop-crop-aspect-original = originali
# machine-drafted, needs native review
develop-crop-turned-right = Ruotata a destra
# machine-drafted, needs native review
develop-crop-turned-left = Ruotata a sinistra
# machine-drafted, needs native review
develop-crop-flipped-horizontal = Riflessa da sinistra a destra
# machine-drafted, needs native review
develop-crop-flipped-vertical = Riflessa dall’alto in basso
# machine-drafted, needs native review
# The label over the photo. $aspect: as above; $guide: one of the guide names below.
develop-crop-label = Proporzioni { $aspect } · Guida: { $guide }
# machine-drafted, needs native review
develop-crop-keys = A proporzioni · X scambia · O guida · [ ] ruota · H V rifletti · Invio conferma · Esc indietro
# machine-drafted, needs native review
develop-crop-guide-thirds = terzi
# machine-drafted, needs native review
develop-crop-guide-golden = sezione aurea
# machine-drafted, needs native review
develop-crop-guide-diagonal = diagonali
# machine-drafted, needs native review
develop-crop-guide-grid = griglia

## Perspective: keystone and upright (its own card; guides are drawn in the crop tool).

# machine-drafted, needs native review
develop-module-perspective = Prospettiva
# machine-drafted, needs native review
# Upright: how the camera's tilt is found.
develop-persp-upright = A piombo
# machine-drafted, needs native review
develop-persp-mode-off = Disattivato
# machine-drafted, needs native review
develop-persp-mode-auto = Automatico
# machine-drafted, needs native review
develop-persp-mode-level = Livella
# machine-drafted, needs native review
develop-persp-mode-vertical = Verticali
# machine-drafted, needs native review
develop-persp-mode-full = Completo
# machine-drafted, needs native review
develop-persp-mode-guided = Guidato
# machine-drafted, needs native review
develop-persp-vertical = Verticale
# machine-drafted, needs native review
develop-persp-horizontal = Orizzontale
# machine-drafted, needs native review
develop-persp-rotate = Ruota
# machine-drafted, needs native review
develop-persp-aspect = Proporzioni
# machine-drafted, needs native review
develop-persp-scale = Scala
# machine-drafted, needs native review
develop-persp-offset-x = Laterale
# machine-drafted, needs native review
develop-persp-offset-y = Su o giù
# machine-drafted, needs native review
# Zoom so no empty edges show.
develop-persp-auto-scale = Evita bordi vuoti
# machine-drafted, needs native review
develop-persp-guides = Guide
# machine-drafted, needs native review
develop-persp-focal = Lunghezza focale
# machine-drafted, needs native review
develop-persp-draw-guides = Disegna guide
# machine-drafted, needs native review
# $n: guides drawn; $max: 4.
develop-persp-draw-guides-n = Disegna guide ({ $n } di { $max })
# machine-drafted, needs native review
develop-persp-reset = Azzera prospettiva
# machine-drafted, needs native review
# $mode: one of the mode names above.
develop-persp-mode-set = A piombo: { $mode }
# machine-drafted, needs native review
develop-persp-analysing = Ricerca delle linee…
# machine-drafted, needs native review
# $n: the guides drawn.
develop-persp-reset-confirm = Azzerare la prospettiva? Le tue { $n } guide vengono rimosse. Annulla le riporta.
# machine-drafted, needs native review
develop-persp-reset-ok = Azzera
# machine-drafted, needs native review
develop-persp-reset-done = Prospettiva azzerata
# machine-drafted, needs native review
# $n: the guide's number.
develop-persp-guide-remove-confirm = Rimuovere la guida { $n }? Annulla la riporta.
# machine-drafted, needs native review
develop-persp-guide-remove-ok = Rimuovi
# machine-drafted, needs native review
develop-persp-guide-removed = Guida rimossa
# machine-drafted, needs native review
# $vertical, $horizontal: the slider values.
develop-persp-keystone-now = Verticale { $vertical }, orizzontale { $horizontal }
# machine-drafted, needs native review
# $error: what went wrong.
develop-persp-failed = A piombo non riuscito: { $error }
# machine-drafted, needs native review
# $mode: a mode name; the angles in degrees ("2.5").
develop-persp-solved = { $mode }: inclinazione { $vertical }°, deviazione { $horizontal }°, rotazione { $rotate }°
# machine-drafted, needs native review
# $mode: a mode name.
develop-perspective-few-lines = { $mode } richiede almeno due linee rette distinguibili. Prova un’altra modalità o disegna guide.
# machine-drafted, needs native review
# $n: 4.
develop-persp-guides-full = Al massimo { $n } guide. Trascina un’estremità per spostarne una.
# machine-drafted, needs native review
develop-persp-guides-on = Guide. Trascina lungo una linea che deve essere verticale o orizzontale.
# machine-drafted, needs native review
develop-persp-guides-off = Ritaglia
# machine-drafted, needs native review
develop-persp-guide-moved = Guida spostata
# machine-drafted, needs native review
# $n: how many guides now.
develop-persp-guide-added = Guida { $n } aggiunta
# machine-drafted, needs native review
develop-persp-guides-keys = Trascina per aggiungere · Trascina un’estremità per spostare · Canc rimuove · Shift+R ritaglia
# machine-drafted, needs native review
# $n: guides drawn; $max: 4.
develop-persp-guides-label = Guide: { $n } di { $max }
# machine-drafted, needs native review
# The three colour wheels.
develop-wheel-shadows = Ombre
# machine-drafted, needs native review
develop-wheel-midtones = Mezzitoni
# machine-drafted, needs native review
develop-wheel-highlights = Luci
# machine-drafted, needs native review
# The luminance slider under each colour wheel (read by screen readers).
develop-luminance-shadows = Luminanza ombre
# machine-drafted, needs native review
develop-luminance-midtones = Luminanza mezzitoni
# machine-drafted, needs native review
develop-luminance-highlights = Luminanza luci
# machine-drafted, needs native review
# The fourth colour wheel: the whole picture.
develop-wheel-global = Globale
# machine-drafted, needs native review
develop-luminance-global = Luminanza globale
# machine-drafted, needs native review
# Colour card: colourfulness changed evenly, and the lightness of colourful areas.
develop-chroma = Intensità del colore
# machine-drafted, needs native review
develop-brilliance = Luminosità del colore

## Colour mixer: hue, saturation and brightness per colour.

# machine-drafted, needs native review
develop-module-colour-mixer = Miscelatore colore
# machine-drafted, needs native review
develop-colour-red = Rosso
# machine-drafted, needs native review
develop-colour-orange = Arancione
# machine-drafted, needs native review
develop-colour-yellow = Giallo
# machine-drafted, needs native review
develop-colour-green = Verde
# machine-drafted, needs native review
develop-colour-aqua = Acqua
# machine-drafted, needs native review
develop-colour-blue = Blu
# machine-drafted, needs native review
develop-colour-purple = Viola
# machine-drafted, needs native review
develop-colour-magenta = Magenta
# machine-drafted, needs native review
# $colour: one of the colour names above.
develop-mixer-hue = Tonalità – { $colour }
# machine-drafted, needs native review
develop-mixer-saturation = Saturazione – { $colour }
# machine-drafted, needs native review
develop-mixer-brightness = Luminanza – { $colour }

## Point colour: pick a colour in the photo, then shift it and its neighbours.

# machine-drafted, needs native review
develop-module-point-colour = Colore punto
# machine-drafted, needs native review
develop-point-pick = Scegli un colore
# machine-drafted, needs native review
# On the pick button once a colour is picked.
develop-point-pick-again = Scegli un altro colore
# machine-drafted, needs native review
# Said and shown while picking.
develop-point-picking = Fai clic sul colore da cambiare. Esc annulla.
# machine-drafted, needs native review
develop-point-picked = Colore scelto
# machine-drafted, needs native review
develop-point-cancelled = Scelta annullata
# machine-drafted, needs native review
# How far from the picked colour changes reach: overall, then by hue, saturation and lightness.
develop-point-range = Intervallo
# machine-drafted, needs native review
develop-point-hue-range = Intervallo di tinta
# machine-drafted, needs native review
develop-point-saturation-range = Intervallo di saturazione
# machine-drafted, needs native review
develop-point-lightness-range = Intervallo di luminosità
# machine-drafted, needs native review
develop-point-hue = Spostamento tonalità
# machine-drafted, needs native review
develop-point-saturation = Spostamento saturazione
# machine-drafted, needs native review
develop-point-lightness = Spostamento luminosità

## Black and white.

# machine-drafted, needs native review
develop-module-black-white = Bianco e nero
# machine-drafted, needs native review
# $colour: a colour name; how light that colour turns in grey.
develop-bw-mix = Luminanza – { $colour }

## LUT: a colour look-up table file.

# machine-drafted, needs native review
develop-module-lut = LUT
# machine-drafted, needs native review
develop-lut-choose = Scegli un file LUT…
# machine-drafted, needs native review
# The file chosen. $name: its file name.
develop-lut-file = File: { $name }
# machine-drafted, needs native review
# The file can't be used. $name: its file name.
develop-lut-unreadable = Impossibile leggere { $name }
# machine-drafted, needs native review
# The file dialog.
develop-lut-dialog = Scegli un file LUT
# machine-drafted, needs native review
develop-lut-kind = File LUT
# machine-drafted, needs native review
# The encoding the LUT was made for.
develop-lut-space = Creata per
# machine-drafted, needs native review
develop-lut-space-linear = Lineare (Rec.2020)
# machine-drafted, needs native review
develop-lut-space-acescg = ACES lineare (ACEScg)
# machine-drafted, needs native review
develop-lut-space-acescct = ACES log (ACEScct)
# machine-drafted, needs native review
develop-lut-space-srgb = Foto (sRGB)
# machine-drafted, needs native review
develop-lut-space-rec709 = Video (Rec.709)
# machine-drafted, needs native review
develop-lut-interpolation = Metodo
# machine-drafted, needs native review
develop-lut-tetrahedral = Preciso
# machine-drafted, needs native review
develop-lut-trilinear = Semplice
# machine-drafted, needs native review
develop-lut-strength = Intensità

## Develop's history and status.

# machine-drafted, needs native review
develop-history-original = Originale
# machine-drafted, needs native review
# A module switched on or off. $module: its name.
develop-history-on = { $module } attivo
# machine-drafted, needs native review
develop-history-off = { $module } disattivo
# machine-drafted, needs native review
# Said to screen readers after a click on a history row. $step: the row's name.
develop-at-original = Di nuovo all’originale
# machine-drafted, needs native review
develop-at-step = Di nuovo a: { $step }
# machine-drafted, needs native review
develop-undone = Annullato: { $step }
# machine-drafted, needs native review
develop-redone = Ripetuto: { $step }
# machine-drafted, needs native review
develop-entered = Sviluppo. Le modifiche non cambiano mai il file.
# machine-drafted, needs native review
develop-left = Uscito da Sviluppo
# machine-drafted, needs native review
develop-split-on = Prima e dopo affiancati
# machine-drafted, needs native review
develop-split-off = Solo la modifica
# machine-drafted, needs native review
develop-showing-before = Viene mostrato l’originale
# machine-drafted, needs native review
develop-showing-after = Viene mostrata la modifica
# machine-drafted, needs native review
# Labels over the image.
develop-before = Prima
# machine-drafted, needs native review
develop-after = Dopo
# machine-drafted, needs native review
develop-working = Sviluppo in corso…
# machine-drafted, needs native review
develop-preview-failed = Impossibile mostrare la modifica per questo file

## Develop's commands (palette, cheat sheet, Settings → Keys) and why they are unavailable.

# machine-drafted, needs native review
keys-mode-develop = Sviluppo
# machine-drafted, needs native review
# The cheat sheet's heading over the speed-edit keys.
cheatsheet-develop-speed = Modifica rapida: tieni premuto, poi scorri
# machine-drafted, needs native review
palette-develop = Sviluppo
# machine-drafted, needs native review
palette-develop-help = Modifica questa foto senza cambiare il file. Di nuovo per uscire.
# machine-drafted, needs native review
palette-develop-before-after = Prima e dopo
# machine-drafted, needs native review
palette-develop-before-after-help = Mostra l’originale, o di nuovo la modifica.
# machine-drafted, needs native review
palette-develop-split = Prima e dopo affiancati
# machine-drafted, needs native review
palette-develop-split-help = Mostra l’originale e la modifica uno accanto all’altra.
# machine-drafted, needs native review
palette-develop-panel = Vai al pannello Sviluppo
# machine-drafted, needs native review
palette-develop-panel-help = Sposta la tastiera nel pannello. Esc torna indietro.
# machine-drafted, needs native review
palette-develop-speed-exposure = Modifica rapida esposizione
# machine-drafted, needs native review
palette-develop-speed-exposure-help = Tieni premuto, poi scorri o premi le frecce per cambiare l’esposizione.
# machine-drafted, needs native review
palette-develop-speed-contrast = Modifica rapida contrasto
# machine-drafted, needs native review
palette-develop-speed-contrast-help = Tieni premuto, poi scorri o premi le frecce per cambiare il contrasto.
# machine-drafted, needs native review
palette-develop-speed-highlights = Modifica rapida luci
# machine-drafted, needs native review
palette-develop-speed-highlights-help = Tieni premuto, poi scorri o premi le frecce per cambiare le luci.
# machine-drafted, needs native review
palette-develop-speed-shadows = Modifica rapida ombre
# machine-drafted, needs native review
palette-develop-speed-shadows-help = Tieni premuto, poi scorri o premi le frecce per cambiare le ombre.
# machine-drafted, needs native review
palette-develop-speed-whites = Modifica rapida bianchi
# machine-drafted, needs native review
palette-develop-speed-whites-help = Tieni premuto, poi scorri o premi le frecce per cambiare i bianchi.
# machine-drafted, needs native review
palette-develop-speed-blacks = Modifica rapida neri
# machine-drafted, needs native review
palette-develop-speed-blacks-help = Tieni premuto, poi scorri o premi le frecce per cambiare i neri.
# machine-drafted, needs native review
palette-develop-speed-temperature = Modifica rapida temperatura
# machine-drafted, needs native review
palette-develop-speed-temperature-help = Tieni premuto, poi scorri o premi le frecce per cambiare la temperatura del bilanciamento del bianco.
# machine-drafted, needs native review
palette-develop-speed-vibrance = Modifica rapida vividezza
# machine-drafted, needs native review
palette-develop-speed-vibrance-help = Tieni premuto, poi scorri o premi le frecce per cambiare la vividezza.
# machine-drafted, needs native review
palette-develop-crop = Ritaglia e raddrizza
# machine-drafted, needs native review
palette-develop-crop-help = Ritaglia, raddrizza, ruota o rifletti la foto. Invio conferma, Esc torna indietro.
# machine-drafted, needs native review
disabled-not-photo = Solo per le foto
# machine-drafted, needs native review
disabled-develop-off = Sviluppo è disattivato nei profili Medicina e Scienza
# machine-drafted, needs native review
develop-module-corrections = Correzioni
# machine-drafted, needs native review
develop-module-raw = Raw
# machine-drafted, needs native review
develop-off = No
# machine-drafted, needs native review
develop-on = Sì
# machine-drafted, needs native review
develop-highlight-method = Luci bruciate
# machine-drafted, needs native review
develop-highlight-opposed = Ricostruisci colore
# machine-drafted, needs native review
develop-highlight-rebuild = Ricostruisci aree grandi
# machine-drafted, needs native review
develop-highlight-clip = Taglia a bianco
# machine-drafted, needs native review
develop-highlight-threshold = Livello di clipping
# machine-drafted, needs native review
develop-module-hot-pixels = Pixel caldi
# machine-drafted, needs native review
develop-hot-threshold = Soglia pixel caldi
# machine-drafted, needs native review
develop-module-ca = Aberrazione cromatica
# machine-drafted, needs native review
develop-ca-mode = Frange
# machine-drafted, needs native review
develop-ca-auto = Automatica
# machine-drafted, needs native review
develop-ca-manual = Manuale
# machine-drafted, needs native review
develop-ca-red = Spostamento rosso
# machine-drafted, needs native review
develop-ca-blue = Spostamento blu
# machine-drafted, needs native review
develop-module-defringe = Rimuovi frange
# machine-drafted, needs native review
develop-defringe = Rimuovi frange
# machine-drafted, needs native review
develop-defringe-purple = Tinta viola
# machine-drafted, needs native review
develop-defringe-green = Tinta verde
# machine-drafted, needs native review
develop-defringe-range = Intervallo di tinta
# machine-drafted, needs native review
develop-defringe-edge = Contrasto dei bordi
# machine-drafted, needs native review
develop-module-flat-field = Fotogrammi di calibrazione
# machine-drafted, needs native review
develop-calibration = Calibrazione
# machine-drafted, needs native review
develop-calibration-auto = Dalla cartella
# machine-drafted, needs native review
develop-calibration-chosen = File scelti
# machine-drafted, needs native review
develop-flat-choose = Scegli un flat…
# machine-drafted, needs native review
develop-dark-choose = Scegli un dark…
# machine-drafted, needs native review
develop-flat-file = Fotogramma flat: { $name }
# machine-drafted, needs native review
develop-dark-file = Fotogramma dark: { $name }
# machine-drafted, needs native review
develop-flat-use = Fotogramma flat
# machine-drafted, needs native review
develop-flat-use-full = Vignettatura e polvere
# machine-drafted, needs native review
develop-flat-use-vignetting = Solo vignettatura
# machine-drafted, needs native review
develop-flat-dialog = Scegli un flat
# machine-drafted, needs native review
develop-dark-dialog = Scegli un dark
# machine-drafted, needs native review
develop-raw-kind = File raw
# machine-drafted, needs native review
develop-flat-frame = Fotogramma flat
# machine-drafted, needs native review
develop-dark-frame = Fotogramma dark

# machine-drafted, needs native review
# machine-drafted, needs native review
develop-mask-add = Aggiungi maschera
# machine-drafted, needs native review
develop-mask-kind-ellipse = Ellisse
# machine-drafted, needs native review
develop-mask-kind-rectangle = Rettangolo
# machine-drafted, needs native review
develop-mask-kind-polygon = Poligono
# machine-drafted, needs native review
develop-mask-kind-linear = Sfumatura lineare
# machine-drafted, needs native review
develop-mask-kind-radial = Sfumatura radiale
# machine-drafted, needs native review
develop-mask-kind-brush = Pennello
# machine-drafted, needs native review
develop-mask-kind-luminance = Intervallo di luminanza
# machine-drafted, needs native review
develop-mask-kind-hue = Intervallo di tinta
# machine-drafted, needs native review
develop-mask-kind-chroma = Intervallo di saturazione
# machine-drafted, needs native review
develop-mask-kind-colour = Intervallo di colore
# machine-drafted, needs native review
develop-mask-kind-other = Altra maschera
# machine-drafted, needs native review
develop-mask-row = { $kind } { $n }
# machine-drafted, needs native review
develop-mask-mode = Fusione
# machine-drafted, needs native review
develop-blend-normal = Normale
# machine-drafted, needs native review
develop-blend-multiply = Moltiplica
# machine-drafted, needs native review
develop-blend-screen = Scolora
# machine-drafted, needs native review
develop-blend-overlay = Sovrapponi
# machine-drafted, needs native review
develop-blend-soft-light = Luce soffusa
# machine-drafted, needs native review
develop-blend-lightness = Solo luminosità
# machine-drafted, needs native review
develop-blend-colour = Solo colore
# machine-drafted, needs native review
develop-mask-amount = Intensità maschera
# machine-drafted, needs native review
develop-mask-invert = Inverti maschera
# machine-drafted, needs native review
develop-mask-off = Disattivato
# machine-drafted, needs native review
develop-mask-on = Attivato
# machine-drafted, needs native review
develop-mask-refine = Rifinisci bordi
# machine-drafted, needs native review
develop-mask-show = Mostra maschera
# machine-drafted, needs native review
develop-mask-show-red = Rosso
# machine-drafted, needs native review
develop-mask-show-grey = Grigio
# machine-drafted, needs native review
develop-mask-remove = Rimuovi maschera
# machine-drafted, needs native review
develop-mask-combine-add = Aggiungi
# machine-drafted, needs native review
develop-mask-combine-subtract = Sottrai
# machine-drafted, needs native review
develop-mask-combine-intersect = Interseca
# machine-drafted, needs native review
develop-mask-row-opacity = Opacità
# machine-drafted, needs native review
develop-mask-row-invert = Inverti
# machine-drafted, needs native review
develop-mask-feather = Sfumatura bordo
# machine-drafted, needs native review
develop-mask-roundness = Arrotondamento
# machine-drafted, needs native review
develop-mask-brush-size = Dimensione pennello
# machine-drafted, needs native review
develop-mask-brush-feather = Sfumatura pennello
# machine-drafted, needs native review
develop-mask-brush-flow = Flusso
# machine-drafted, needs native review
develop-mask-brush-mode = Pennello
# machine-drafted, needs native review
develop-mask-brush-paint = Dipingi
# machine-drafted, needs native review
develop-mask-brush-erase = Cancella
# machine-drafted, needs native review
develop-mask-range-from = Da
# machine-drafted, needs native review
develop-mask-range-full-from = Pieno da
# machine-drafted, needs native review
develop-mask-range-full-to = Pieno fino a
# machine-drafted, needs native review
develop-mask-range-to = A
# machine-drafted, needs native review
develop-mask-range-test = Verifica
# machine-drafted, needs native review
develop-mask-range-input = Ingresso
# machine-drafted, needs native review
develop-mask-range-output = Uscita
# machine-drafted, needs native review
develop-mask-colour-pick = Preleva un colore
# machine-drafted, needs native review
develop-mask-colour-pick-more = Preleva un altro colore
# machine-drafted, needs native review
develop-mask-colour-range = Intervallo
# machine-drafted, needs native review
develop-mask-colour-falloff = Attenuazione
# machine-drafted, needs native review
develop-mask-edit = Modifica sull’immagine
# machine-drafted, needs native review
develop-mask-row-remove = Rimuovi { $name }
# machine-drafted, needs native review
develop-mask-remove-confirm = Rimuovere la maschera di { $module }? Le sue forme spariscono. La foto sul disco resta.
# machine-drafted, needs native review
develop-mask-row-remove-confirm = Rimuovere { $name } da { $module }? Le altre maschere e la foto sul disco restano.
# machine-drafted, needs native review
develop-mask-remove-ok = Rimuovi
# machine-drafted, needs native review
develop-mask-removed = Maschera rimossa.
# machine-drafted, needs native review
develop-mask-added = { $name } aggiunto.
# machine-drafted, needs native review
develop-mask-full = Questa maschera ha già tutte le parti che può contenere.
# machine-drafted, needs native review
develop-mask-editing = Modifica di { $name }. Trascina le maniglie, o premi Tab e le frecce.
# machine-drafted, needs native review
develop-mask-painting = Pittura di { $name }. Trascina sull’immagine.
# machine-drafted, needs native review
develop-mask-picking = Fai clic sull’immagine per prelevare un colore.
# machine-drafted, needs native review
develop-mask-picked = Colore prelevato.
# machine-drafted, needs native review
develop-mask-done = Modifica della maschera terminata.
# machine-drafted, needs native review
develop-mask-handles = Maniglie della maschera
# machine-drafted, needs native review
develop-handle-centre = Centro
# machine-drafted, needs native review
develop-handle-width = Larghezza
# machine-drafted, needs native review
develop-handle-height = Altezza
# machine-drafted, needs native review
develop-handle-rotate = Ruota
# machine-drafted, needs native review
develop-handle-feather = Sfumatura bordo
# machine-drafted, needs native review
develop-handle-inner = Bordo interno
# machine-drafted, needs native review
develop-handle-start = Inizio
# machine-drafted, needs native review
develop-handle-end = Fine
# machine-drafted, needs native review
develop-handle-point = Punto { $n }
# machine-drafted, needs native review
develop-instance = Livello
# machine-drafted, needs native review
develop-instance-n = { $n }
# machine-drafted, needs native review
develop-instance-chosen = Livello { $n }.
# machine-drafted, needs native review
develop-instance-new = Nuovo livello
# machine-drafted, needs native review
develop-instance-duplicate = Duplica livello
# machine-drafted, needs native review
develop-instance-delete = Elimina livello
# machine-drafted, needs native review
develop-instance-earlier = Sposta prima
# machine-drafted, needs native review
develop-instance-later = Sposta dopo
# machine-drafted, needs native review
develop-instance-delete-confirm = Eliminare il livello { $n } di { $module }? Le sue impostazioni e maschere spariscono. La foto sul disco resta.
# machine-drafted, needs native review
develop-instance-delete-ok = Elimina
# machine-drafted, needs native review
develop-instance-deleted = Livello eliminato.
# machine-drafted, needs native review
develop-history-mask = Maschera: { $module }
# machine-drafted, needs native review
develop-history-mask-shapes = Forme maschera: { $module }
# machine-drafted, needs native review
develop-history-instance-new = Nuovo: { $module }
# machine-drafted, needs native review
develop-history-instance-delete = Eliminato: { $module }
# machine-drafted, needs native review
develop-history-instance-move = Ordine: { $module }
# Other tools' edits and sidecar conflicts.
# machine-drafted, needs native review
develop-others-title = Altri programmi
# machine-drafted, needs native review
develop-darktable-edited = Modificata in darktable:
# machine-drafted, needs native review
develop-darktable-open = Apri in darktable
# machine-drafted, needs native review
develop-darktable-opening = Apertura in darktable
# machine-drafted, needs native review
develop-darktable-failed = darktable non si è avviato: { $reason }
# machine-drafted, needs native review
develop-lightroom-import = Importa da Lightroom
# machine-drafted, needs native review
develop-lightroom-imported = { $count ->
    [one] Importata 1 impostazione Lightroom. Il risultato è approssimativo.
    [many] Importate { $count } impostazioni Lightroom. Il risultato è approssimativo.
   *[other] Importate { $count } impostazioni Lightroom. Il risultato è approssimativo.
}
# machine-drafted, needs native review
develop-lightroom-nothing = Niente da importare: queste impostazioni Lightroom qui non cambiano nulla.
# machine-drafted, needs native review
develop-history-lightroom = Importato da Lightroom (approssimativo)
# machine-drafted, needs native review
develop-conflict-title = La modifica di { $file } è cambiata fuori da Specular
# machine-drafted, needs native review
develop-conflict-file = File collaterale: salvato { $time }
# machine-drafted, needs native review
develop-conflict-library = Libreria: salvata { $time }
# machine-drafted, needs native review
develop-conflict-writer = Il file collaterale è stato scritto da { $writer }.
# machine-drafted, needs native review
develop-conflict-detail = Tienine una. L’altra viene sostituita.
# machine-drafted, needs native review
develop-conflict-use-file = Usa quella del file
# machine-drafted, needs native review
develop-conflict-keep-library = Tieni quella della libreria
# machine-drafted, needs native review
develop-conflict-used-file = Uso la modifica del file collaterale
# machine-drafted, needs native review
develop-conflict-kept-library = Tengo la modifica della libreria
# machine-drafted, needs native review
toast-develop-edit-newer-process = Questa modifica viene da uno Specular più recente. Qui può apparire diversa.
# machine-drafted, needs native review
info-darktable-edited = Modificata in darktable
# machine-drafted, needs native review
info-darktable-no-modules = Nessun modulo attivo
# machine-drafted, needs native review
info-darktable-group = File
# machine-drafted, needs native review
toast-develop-edit-partial = Questa modifica viene da uno Specular più recente. Le parti che questa versione non sa mostrare sono omesse. Le modifiche qui non vengono salvate.
# machine-drafted, needs native review
toast-develop-sidecar-unreadable = Il file di modifica accanto a questa foto non è leggibile. Resta com’è. Le modifiche qui non vengono salvate.
# machine-drafted, needs native review
develop-demosaic = Dettagli raw
develop-demosaic-directional = Standard
develop-demosaic-dual = Aree uniformi morbide
develop-demosaic-fast = Veloce
develop-dual-threshold = Contrasto aree uniformi
