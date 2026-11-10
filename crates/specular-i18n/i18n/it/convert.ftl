# Specular — Batch convert (DESIGN.md §6.5). machine-drafted, needs native review
# Every output is a new file; the sources are never changed.

palette-convert = Converti…

## Settings → Convert
settings-section-convert = Conversione
settings-convert-format = Formato
settings-convert-format-help = Il formato scritto da Converti…; PNG e TIFF sono senza perdita, WebP e JPEG XL possono esserlo o no.
settings-convert-format-jpeg = JPEG
settings-convert-format-png = PNG
settings-convert-format-webp = WebP
settings-convert-format-jxl = JPEG XL
settings-convert-format-tiff = TIFF
settings-convert-jpeg-quality = Qualità JPEG
settings-convert-jpeg-quality-help = Valori più alti conservano più dettagli in file più grandi. A 90 è difficile distinguerlo dall’originale.
settings-convert-jpeg-full-chroma = Colore a piena risoluzione in JPEG
settings-convert-jpeg-full-chroma-help = Mantiene il colore a piena risoluzione (4:4:4) invece che a metà (4:2:0): bordi di colore più nitidi, file più grandi.
settings-convert-png-effort = Compressione PNG
settings-convert-png-effort-help = Valori più alti creano file più piccoli, più lentamente. 1 è il più veloce.
settings-convert-jxl-effort = Impegno JPEG XL
settings-convert-jxl-effort-help = Valori più alti creano file più piccoli, più lentamente.
settings-convert-tiff-compression = Compressione TIFF
settings-convert-tiff-compression-help = Deflate è senza perdita ed è letto da qualsiasi lettore TIFF.
settings-convert-tiff-compression-deflate = Deflate
settings-convert-tiff-compression-none = Nessuna
settings-convert-depth = Profondità di bit
settings-convert-depth-help = Automatica scrive 16 bit per campione quando l’immagine ne ha più di 8 e il formato li supporta (PNG, TIFF, JPEG XL).
settings-convert-depth-auto = Automatica
settings-convert-depth-8 = 8 bit
settings-convert-depth-16 = 16 bit
settings-convert-resize = Ridimensiona
settings-convert-resize-help = Le immagini vengono ridotte in luce lineare con un filtro di alta qualità.
settings-convert-resize-none = Dimensione originale
settings-convert-resize-fit = Adatta entro
settings-convert-resize-long-edge = Lato lungo
settings-convert-resize-percent = Percentuale
settings-convert-width = Larghezza
settings-convert-width-help = La larghezza massima che un’immagine può raggiungere con Adatta entro.
settings-convert-height = Altezza
settings-convert-height-help = L’altezza massima che un’immagine può raggiungere con Adatta entro.
settings-convert-long-edge = Lato lungo
settings-convert-long-edge-help = La lunghezza del lato più lungo dell’immagine con Lato lungo.
settings-convert-percent = Percentuale
settings-convert-percent-help = La dimensione rispetto all’originale con Percentuale.
settings-convert-upscale = Ingrandisci le immagini piccole
settings-convert-upscale-help = Se disattivato, le immagini già più piccole della dimensione richiesta mantengono la loro dimensione.
settings-convert-colour = Colore
settings-convert-colour-help = Mantieni salva il profilo colore dell’immagine insieme a essa. Converti in sRGB è adatto al web e ai dispositivi senza gestione del colore.
settings-convert-colour-keep = Mantieni il profilo
settings-convert-colour-srgb = Converti in sRGB
settings-convert-hdr = Immagini HDR
settings-convert-hdr-help = PNG e JPEG XL mantengono l’HDR così com’è; gli altri formati vengono sempre portati in SDR con tone mapping.
settings-convert-hdr-keep = Mantieni se possibile
settings-convert-hdr-tone-map = Tone mapping in SDR
settings-convert-metadata = Metadati
settings-convert-metadata-help = Exif e XMP, come la fotocamera, la data di scatto e la posizione. Il profilo colore viene sempre mantenuto.
settings-convert-metadata-all = Mantieni tutto
settings-convert-metadata-no-location = Tutto tranne la posizione
settings-convert-metadata-strip = Rimuovi
settings-convert-folder = Cartella di destinazione
settings-convert-folder-help = Dove finiscono i file convertiti. Se non è impostata, vanno in una sottocartella accanto a ogni immagine.
settings-convert-folder-unset = Sottocartella accanto alle immagini
settings-convert-subfolder = Nome della sottocartella
settings-convert-subfolder-help = La sottocartella usata quando non è impostata una cartella di destinazione. Vuoto: la cartella stessa delle immagini.
settings-convert-pattern = Nomi dei file
settings-convert-pattern-help = {"{"}name{"}"} è il nome originale, {"{"}n:03{"}"} il numero nel lotto, {"{"}date:%Y-%m-%d{"}"} la data di scatto, {"{"}w{"}"} e {"{"}h{"}"} la nuova dimensione, {"{"}ext{"}"} l’estensione del formato.
settings-convert-conflicts = Se un nome è già usato
settings-convert-conflicts-help = I file esistenti non vengono mai sostituiti.
settings-convert-conflicts-suffix = Aggiungi un numero
settings-convert-conflicts-skip = Salta l’immagine

## The dialog
convert-title = Converti
convert-images = Immagini
convert-images-current = Questa immagine
convert-images-selection = Selezione ({ $count })
convert-images-folder = Cartella ({ $count })
convert-images-picked = Scelte ({ $count })
convert-quality = Qualità
convert-chroma = Dettaglio del colore
convert-chroma-half = 4:2:0 (più piccolo)
convert-chroma-full = 4:4:4 (più nitido)
convert-effort = Impegno
convert-on = Sì
convert-off = No
convert-pixels = { $value } px
convert-effort-fastest = { $value } (il più veloce)
convert-folder-subfolder = Sottocartella «{ $name }»
convert-folder-same = Stessa cartella dell’immagine
convert-choose-folder = Scegli la cartella…
convert-name = Nome
convert-name-preview = { $source } → { $output }
convert-name-error-unclosed = Una {"{"} non è chiusa.
convert-name-error-stray = Una {"}"} non ha la sua {"{"}. Scrivi {"}}"} per una parentesi graffa.
convert-name-error-unknown = { $token } non è una parte del nome. Usa name, n, date, w, h o ext.
convert-name-error-format = { $token } ha un formato non valido.
convert-name-error-empty = Inserisci uno schema per il nome.
convert-start = Converti
convert-stop = Interrompi
convert-hide = Nascondi
convert-close = Chiudi
convert-progress = Conversione di { $done } su { $total }
convert-progress-eta = Conversione di { $done } su { $total } · mancano circa { $time }
convert-eta-seconds = { $count ->
    [one] { $count } secondo
   *[other] { $count } secondi
}
convert-eta-minutes = { $count ->
    [one] { $count } minuto
   *[other] { $count } minuti
}
convert-done = { $count ->
    [one] { $count } immagine convertita
   *[other] { $count } immagini convertite
}
convert-done-skipped = { $count } saltate: il nome era già usato
convert-done-failed = { $count } non riuscite
convert-stopped = Interrotto dopo { $count }
convert-failure = { $file }: { $reason }
convert-failure-open = impossibile aprire ({ $error })
convert-failure-decode = impossibile leggere ({ $error })
convert-failure-encode = impossibile scrivere in questo formato ({ $error })
convert-failure-write = impossibile salvare ({ $error })
convert-failure-same = il nuovo file sostituirebbe l’originale
convert-report-more = e altri { $count }
convert-running = È già in corso una conversione.

## Convert encoders (machine-drafted, needs native review)
settings-convert-format-avif = AVIF
settings-convert-webp-lossless = WebP senza perdita
settings-convert-webp-lossless-help = Mantiene esattamente ogni pixel. Se disattivato, i file WebP sono molto più piccoli alla qualità WebP.
settings-convert-webp-quality = Qualità WebP
settings-convert-webp-quality-help = Valori più alti conservano più dettagli in file più grandi. Usato quando WebP non è senza perdita.
settings-convert-webp-effort = Impegno WebP
settings-convert-webp-effort-help = Valori più alti creano file più piccoli, più lentamente. Usato quando WebP non è senza perdita.
settings-convert-jxl-lossless = JPEG XL senza perdita
settings-convert-jxl-lossless-help = Mantiene esattamente ogni pixel. Se disattivato, i file JPEG XL sono molto più piccoli alla qualità JPEG XL.
settings-convert-jxl-quality = Qualità JPEG XL
settings-convert-jxl-quality-help = Valori più alti conservano più dettagli in file più grandi. A 90 è difficile distinguerlo dall’originale. Usato quando JPEG XL non è senza perdita.
settings-convert-avif-quality = Qualità AVIF
settings-convert-avif-quality-help = Valori più alti conservano più dettagli in file più grandi.
settings-convert-avif-effort = Impegno AVIF
settings-convert-avif-effort-help = Valori più alti creano file più piccoli, più lentamente. 1 è il più veloce.
convert-lossless = Senza perdita
