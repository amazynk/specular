# machine-drafted, needs native review
# Specular — Duplicati e scatti simili (categorie della galleria). Italiano.

similar-duplicates = Duplicati
similar-similar = Scatti simili

## Voci del menu Vista e della tavolozza
similar-menu-entry = { $name } — { $groups ->
    [one] { $groups } gruppo
   *[other] { $groups } gruppi
}, { $size } da liberare
similar-menu-scanning = { $name } — analisi in corso ({ PERCENT($percent) })
similar-menu-none = { $name } — nessuno trovato

## Intestazione
similar-summary = { $groups ->
    [one] { $groups } gruppo da esaminare
   *[other] { $groups } gruppi da esaminare
}, { $files ->
    [one] { $files } file
   *[other] { $files } file
} da scartare, { $size } da liberare
similar-progress = { $resolved } di { $total } risolti
similar-skipped = { $count } saltati
similar-scanning = Ricerca di duplicati e scatti simili… { $done } di { $total }
# machine-drafted, needs native review
similar-checking-sharpness = Verifica della nitidezza… { $done } di { $total }
# machine-drafted, needs native review
# L’elenco durante la prima scansione (l’avanzamento è nell’intestazione sopra).
similar-waiting = I gruppi compariranno qui al termine della scansione.
# machine-drafted, needs native review
similar-stopped = Scansione interrotta: nessun gruppo creato
similar-stopped-sharpness = Verifica della nitidezza interrotta: scelte migliori dalle miniature
similar-stopped-empty = La scansione è stata interrotta. Ciò che ha misurato viene conservato: Ripeti scansione prosegue da lì.
similar-scan-again = Ripeti scansione
similar-empty-duplicates = Nessun duplicato in questa cartella.
similar-empty-similar = Nessuno scatto simile in questa cartella.
similar-all-done = Tutti i gruppi esaminati.

## Scatti simili: quanto devono somigliarsi le foto per essere raggruppate
similar-sensitivity = Sensibilità
similar-sensitivity-strict = Rigida
similar-sensitivity-loose = Ampia

## Piè di pagina dell’elenco dei gruppi
similar-accept-all = Accetta tutti i suggerimenti
similar-show-skipped = Mostra gruppi saltati
similar-discard-with = Scarta: { $action }
similar-action-trash = Sposta nel cestino
similar-action-move = Sposta in una cartella…
similar-action-move-to = Sposta in «{ $folder }»
similar-action-reject = Segna solo come scartate
similar-choose-folder = Scegli la cartella per i file scartati…

## Conferma in blocco
similar-confirm = Accettare i suggerimenti di { $groups ->
    [one] { $groups } gruppo
   *[other] { $groups } gruppi
}? { $files ->
    [one] { $files } file
   *[other] { $files } file
} ({ $size }): { $action }.
similar-confirm-yes = { $files ->
    [one] Accetta e scarta { $files } file
   *[other] Accetta e scarta { $files } file
}
similar-cancel = Annulla

## Un gruppo
similar-group-title = Gruppo { $index } di { $total }
similar-group-photos = { $count ->
    [one] { $count } foto
   *[other] { $count } foto
}
similar-to-free = { $size } da liberare

## Azioni e stati di revisione
similar-keep = Tieni
similar-discard = Scarta
similar-undecided = Da decidere
similar-accept-next = Accetta e avanti
similar-not-similar = Non simili
similar-skip = Salta
similar-compare = Confronta
similar-zoom = { PERCENT(100) }
similar-back = Indietro
similar-done = Fine
similar-suggested-keep = Suggerito: tieni
similar-suggested-discard = Suggerito: scarta

## Notifiche
similar-toast-accepted = { $count ->
    [one] { $count } gruppo accettato
   *[other] { $count } gruppi accettati
}
similar-toast-rejected = { $count ->
    [one] { $count } file segnato come scartato
   *[other] { $count } file segnati come scartati
}
similar-toast-protected = Le foto valutate, contrassegnate, etichettate, con parole chiave o modificate vengono sempre tenute.
# machine-drafted, needs native review
similar-zoom-already = Questa foto è già mostrata al { PERCENT(100) } o più grande

## Accessibilità
similar-a11y-member = { $name }: { $state }, { $reason }

## Suggerimenti tasti
similar-hint-list = ↑↓ scegli · Invio esamina · A accetta tutti · Tab cambia · Esc chiudi

## Perché una foto viene tenuta
similar-reason-rated = Valutata
similar-reason-flagged = Contrassegnata come scelta
similar-reason-labelled = Ha un’etichetta colore
similar-reason-tagged = Ha parole chiave
similar-reason-edited = Ha modifiche
similar-reason-original = L’originale
similar-reason-raw-original = Originale RAW
similar-reason-highest-resolution = Risoluzione più alta
similar-reason-deeper-colour = Colore più profondo
similar-reason-least-compressed = Meno compressa
similar-reason-has-metadata = Conserva i metadati
similar-reason-oldest = Copia più vecchia
similar-reason-sharpest = Più nitida
similar-reason-steadiest = Meno mossa
similar-reason-best-exposure = Esposta meglio
similar-reason-least-noise = Meno rumore
similar-reason-also-good = Anche questa è buona

## Perché si suggerisce di scartare una foto
similar-reason-same-file = File identico
similar-reason-derivative = Copia derivata
similar-reason-rendering-of-raw = Sviluppo del RAW
similar-reason-smaller-copy = Copia più piccola
similar-reason-shallower-colour = Minore profondità colore
similar-reason-more-compressed = Più compressa
similar-reason-no-metadata = Metadati rimossi
similar-reason-newer-copy = Copia più recente
similar-reason-blurrier = Più sfocata
similar-reason-motion-blur = Mossa
similar-reason-overexposed = Sovraesposta
similar-reason-underexposed = Sottoesposta
similar-reason-noisier = Più rumore
similar-reason-lower-resolution = Risoluzione più bassa
