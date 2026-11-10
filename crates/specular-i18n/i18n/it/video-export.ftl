# Specular — Export slideshow as video (DESIGN.md §6.5). Italian. Machine-drafted, needs native review.

palette-export-slideshow = Esporta presentazione come video…

video-export-title = Esporta presentazione come video
video-export-resolution = Risoluzione
# $width and $height in pixels.
video-export-resolution-window = Finestra ({ $width } × { $height })
video-export-fps = Frequenza fotogrammi
# $fps frames per second, a number.
video-export-fps-value = { $fps } fps
video-export-quality = Qualità
video-export-quality-high = Alta
video-export-quality-standard = Standard
video-export-quality-compact = Compatta
video-export-images = { $count ->
    [one] Un’immagine, con ordine, transizione e didascalie della presentazione
   *[other] { $count } immagini, con ordine, transizione e didascalie della presentazione
}
# $length a duration ("2:40"), $size a file size ("38 MB").
video-export-summary = Circa { $length } di durata, circa { $size }
video-export-export = Esporta…
video-export-replace = Sostituisci
# $file a file name.
video-export-replace-question = { $file } esiste già. Sostituirlo?
# $percent a formatted percentage ("42%").
video-export-progress = Esportazione… { $percent }
video-export-cancelling = Annullamento…
# $file a file name, $size a file size ("38 MB").
video-export-saved = Salvato { $file } ({ $size })
video-export-skipped = { $count ->
    [one] Un’immagine non è stata letta ed è stata esclusa.
   *[other] { $count } immagini non sono state lette e sono state escluse.
}
video-export-failed = Impossibile esportare il video: { $reason }
video-export-cancelled = Esportazione annullata
video-export-no-images = Non è stato possibile leggere nessuna delle immagini.
video-export-show-in-folder = Mostra nella cartella

## Toolbar item and compatibility (machine-drafted, needs native review)
settings-viewing-toolbar-items-export-slideshow = Esporta presentazione come video
video-export-compatibility = Compatibilità
video-export-compatibility-current = Lettori attuali
video-export-compatibility-all = Tutti i lettori
