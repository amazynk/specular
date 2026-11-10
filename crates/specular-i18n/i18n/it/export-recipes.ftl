# Specular — Export recipes: saved ways to export, several run at once. machine-drafted, needs native review
# Every output is a new file (or replaces one only after asking); originals are never changed.

palette-export-recipes = Esporta con ricette…
palette-export-recipes-help = Esporta la selezione con una o più ricette salvate.
palette-develop-export-recipes = Esporta con ricette…
palette-develop-export-recipes-help = Esporta la foto modificata, o più foto, con ricette salvate.

export-recipes-title = Esporta con ricette
export-recipes-new = Nuova
export-recipes-edit = Modifica…
export-recipes-duplicate = Duplica
export-recipes-delete = Elimina…
export-recipes-start = Esporta
export-recipes-save = Salva
export-recipes-open-folder = Apri cartella
export-recipes-choose-image = Scegli immagine…
export-recipes-no-images = Nessuna immagine da esportare.
export-recipes-none-checked = Spunta almeno una ricetta.
export-recipes-checked = { $name }: attiva
export-recipes-unchecked = { $name }: disattiva

## The starters.
export-recipes-starter-web = Web, 2048 px
export-recipes-starter-print = Stampa, dimensione piena
export-recipes-starter-email = Email, piccola
export-recipes-unnamed = Ricetta senza nome
export-recipes-new-name = Nuova ricetta
# { $name } is the recipe copied.
export-recipes-copy-name = { $name } (copia)

## The list's one-line summary.
export-recipes-summary-mark = filigrana
export-recipes-size-full = Dimensione piena
export-recipes-size-long = { $px } px lato lungo
export-recipes-size-short = { $px } px lato corto
export-recipes-size-mp = { $mp } MP
export-recipes-size-exact = { $width } × { $height } px
export-recipes-size-percent = { PERCENT($percent) }
export-recipes-folder-same = Accanto all’originale
export-recipes-folder-sub = Cartella «{ $name }»
export-recipes-folder-ask = Chiedi ogni volta

## The editor.
export-recipes-new-title = Nuova ricetta
export-recipes-edit-title = Modifica ricetta
export-recipes-name = Nome della ricetta
export-recipes-text = Testo personalizzato
export-recipes-pattern = Nome del file
export-recipes-mark-text-field = Testo della filigrana
export-recipes-subfolder = Nome della cartella
export-recipes-format = Formato
export-recipes-quality = Qualità
export-recipes-lossless = Senza perdita
export-recipes-bits = Profondità di bit
export-recipes-bits-8 = 8 bit
export-recipes-bits-16 = 16 bit
export-recipes-space = Spazio colore
export-recipes-space-srgb = sRGB
export-recipes-space-p3 = Display P3
export-recipes-space-adobe = Adobe RGB (1998)
export-recipes-space-prophoto = ProPhoto RGB
export-recipes-profile = Profilo
export-recipes-profile-embed = Incorpora
export-recipes-profile-none = Ometti
export-recipes-folder = Cartella
export-recipes-folder-subfolder = Sottocartella
export-recipes-collision = Nome già usato
export-recipes-collision-unique = Aggiungi un numero
export-recipes-collision-overwrite = Sostituisci (chiede)
export-recipes-collision-skip = Salta
export-recipes-after = Alla fine
export-recipes-after-nothing = Niente
export-recipes-after-open = Apri la cartella
export-recipes-resize = Dimensione
export-recipes-resize-long = Lato lungo
export-recipes-resize-short = Lato corto
export-recipes-resize-mp = Megapixel
export-recipes-resize-exact = Dimensione esatta
export-recipes-resize-percent = Percentuale
export-recipes-edge = Lato
export-recipes-width = Larghezza
export-recipes-height = Altezza
export-recipes-megapixels = Megapixel
export-recipes-percent = Percentuale
export-recipes-px = { $px } px
export-recipes-mp = { $mp } MP
export-recipes-pct = { PERCENT($percent) }
export-recipes-enlarge = Ingrandisci
export-recipes-sharpen = Nitidezza per
export-recipes-sharpen-off = No
export-recipes-sharpen-screen = Schermo
export-recipes-sharpen-matte = Carta opaca
export-recipes-sharpen-glossy = Carta lucida
export-recipes-amount = Intensità
export-recipes-amount-low = Bassa
export-recipes-amount-standard = Normale
export-recipes-amount-high = Alta
export-recipes-metadata = Metadati
export-recipes-meta-all = Tutti
export-recipes-meta-copyright = Solo copyright
export-recipes-meta-contact = Copyright e contatti
export-recipes-meta-none = Nessuno
export-recipes-location = Luogo
export-recipes-people = Persone ritratte
export-recipes-keep = Mantieni
export-recipes-remove = Rimuovi
export-recipes-mark = Filigrana
export-recipes-mark-none = Nessuna
export-recipes-mark-text = Testo
export-recipes-mark-image = Immagine (scegline una)
export-recipes-position = Posizione
export-recipes-pos-top-left = In alto a sinistra
export-recipes-pos-top = In alto
export-recipes-pos-top-right = In alto a destra
export-recipes-pos-left = A sinistra
export-recipes-pos-centre = Al centro
export-recipes-pos-right = A destra
export-recipes-pos-bottom-left = In basso a sinistra
export-recipes-pos-bottom = In basso
export-recipes-pos-bottom-right = In basso a destra
export-recipes-opacity = Opacità
export-recipes-scale = Larghezza
export-recipes-yes = Sì
export-recipes-no = No
export-recipes-error-name = Dai un nome alla ricetta.
export-recipes-error-mark-text = Scrivi il testo della filigrana.
export-recipes-error-mark-image = Scegli l’immagine della filigrana.
export-recipes-error-subfolder = Scrivi il nome della cartella.
export-recipes-saved = Ricetta «{ $name }» salvata.

## Delete.
export-recipes-delete-title = Eliminare la ricetta «{ $name }»?
export-recipes-delete-detail = I file che ha esportato restano sul disco.
export-recipes-delete-confirm = Elimina
export-recipes-delete-keep = Mantieni
export-recipes-deleted = Ricetta «{ $name }» eliminata.

## The run.
# { $name } is the recipe whose folder is asked for.
export-recipes-ask-title = Cartella per «{ $name }»
export-recipes-asking = Scegli la cartella per ogni ricetta che la chiede.
export-recipes-no-folder = Nessuna cartella scelta. Non è stato esportato nulla.
export-recipes-planning = Assegnazione dei nomi ai file…
export-recipes-plan-pattern = Ricetta «{ $name }»: { $reason }
export-recipes-replace-title = { $count ->
    [one] Sostituire 1 file?
   *[other] Sostituire { $count } file?
}
export-recipes-replace-detail = Gli originali non vengono mai sostituiti.
export-recipes-replace-confirm = Sostituisci
export-recipes-replace-kept = Non è stato esportato nulla.
export-recipes-running = Esportazione…
export-recipes-progress = { $done } di { $total } file
export-recipes-done = { $count ->
    [one] 1 file esportato.
   *[other] { $count } file esportati.
}
export-recipes-stopped = { $count ->
    [one] Interrotto. 1 file esportato.
   *[other] Interrotto. { $count } file esportati.
}
export-recipes-done-skipped = { $count ->
    [one] 1 saltato: il nome era già usato.
   *[other] { $count } saltati: i nomi erano già usati.
}
export-recipes-done-failed = { $count ->
    [one] 1 non riuscito:
   *[other] { $count } non riusciti:
}
# One failure: the source file, the recipe and why.
export-recipes-failure = { $file } ({ $recipe }): { $reason }
