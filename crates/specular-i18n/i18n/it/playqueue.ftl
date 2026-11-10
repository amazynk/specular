# The play queue: what plays next, repeat and shuffle, playlists (M3U, PLS, XSPF), the
# queue panel and the sound transport's buttons.

# machine-drafted, needs native review
cheatsheet-video-queue = Coda, ripetizione e casuale
# machine-drafted, needs native review
palette-next-track = Elemento successivo
# machine-drafted, needs native review
palette-next-track-help = Riproduci l’elemento successivo della coda, altrimenti il file successivo dello stesso tipo nella cartella.
# machine-drafted, needs native review
palette-prev-track = Elemento precedente
# machine-drafted, needs native review
palette-prev-track-help = Riproduci l’elemento precedente. Dopo 3 secondi, riprendi questo dall’inizio.
# machine-drafted, needs native review
palette-cycle-repeat = Ripeti
# machine-drafted, needs native review
palette-cycle-repeat-help = Ripetizione disattivata, tutti o uno.
# machine-drafted, needs native review
palette-shuffle = Riproduzione casuale
# machine-drafted, needs native review
palette-shuffle-help = Riproduci in ordine casuale, ogni elemento una volta per giro.
# machine-drafted, needs native review
palette-play-queue = Coda di riproduzione
# machine-drafted, needs native review
palette-play-queue-help = Mostra cosa verrà riprodotto: riordina, rimuovi, salva.
# machine-drafted, needs native review
palette-queue-play-next = Riproduci dopo
# machine-drafted, needs native review
palette-queue-play-next-help = Riproduci il file (nella galleria, la selezione) dopo l’elemento in corso.
# machine-drafted, needs native review
palette-queue-add = Aggiungi alla coda
# machine-drafted, needs native review
palette-queue-add-help = Aggiungi il file (nella galleria, la selezione) alla fine della coda.
# machine-drafted, needs native review
palette-save-queue = Salva la coda come playlist…
# machine-drafted, needs native review
palette-save-queue-help = Salva la coda come playlist M3U8 o XSPF.
# machine-drafted, needs native review
transport-prev-track = Elemento precedente
# machine-drafted, needs native review
transport-next-track = Elemento successivo
# machine-drafted, needs native review
queue-title = Coda di riproduzione
# machine-drafted, needs native review
queue-summary = { $count ->
    [one] 1 elemento
   *[other] { $count } elementi
}
# machine-drafted, needs native review
queue-summary-time = { $count ->
    [one] 1 elemento
   *[other] { $count } elementi
} · { $time }
# machine-drafted, needs native review
queue-empty = La coda è vuota.
    Aggiungi file audio e video con Riproduci dopo o Aggiungi alla coda, oppure apri una playlist.
# machine-drafted, needs native review
queue-empty-short = La coda è vuota.
# machine-drafted, needs native review
queue-playing = In riproduzione
# machine-drafted, needs native review
queue-repeat = Ripeti
# machine-drafted, needs native review
queue-repeat-off = Ripeti: no
# machine-drafted, needs native review
queue-repeat-all = Ripeti: tutti
# machine-drafted, needs native review
queue-repeat-one = Ripeti: uno
# machine-drafted, needs native review
queue-shuffle = Casuale
# machine-drafted, needs native review
queue-shuffle-on = Casuale: sì
# machine-drafted, needs native review
queue-shuffle-off = Casuale: no
# machine-drafted, needs native review
queue-save = Salva come playlist…
# machine-drafted, needs native review
queue-save-tip = Salva la coda come playlist M3U8 o XSPF
# machine-drafted, needs native review
queue-clear = Svuota…
# machine-drafted, needs native review
queue-clear-tip = Svuota la coda. I file restano sul disco.
# machine-drafted, needs native review
queue-remove = Rimuovi dalla coda (il file resta sul disco)
# machine-drafted, needs native review
queue-network-item = Elemento di rete, non riprodotto: { $address }
# machine-drafted, needs native review
queue-artist-title = { $artist } – { $title }
# machine-drafted, needs native review
queue-at-end = Nessun elemento successivo.
# machine-drafted, needs native review
queue-at-start = Nessun elemento precedente.
# machine-drafted, needs native review
queue-missing = { $name } non c’è più. Saltato.
# machine-drafted, needs native review
queue-network-not-played = Gli elementi di rete non vengono ancora riprodotti.
# machine-drafted, needs native review
queue-nothing-to-add = Niente da aggiungere. Nella coda vanno solo file audio, video e playlist.
# machine-drafted, needs native review
queue-added = { $count ->
    [one] 1 elemento aggiunto alla coda.
   *[other] { $count } elementi aggiunti alla coda.
}
# machine-drafted, needs native review
queue-added-next = { $count ->
    [one] 1 elemento verrà riprodotto dopo.
   *[other] { $count } elementi verranno riprodotti dopo.
}
# machine-drafted, needs native review
queue-playlist-opened = { $name }: { $count ->
    [one] 1 elemento in coda.
   *[other] { $count } elementi in coda.
}
# machine-drafted, needs native review
queue-playlist-nothing = Niente in { $name } si può riprodurre qui. La coda mostra cosa contiene.
# machine-drafted, needs native review
queue-playlist-failed = Impossibile aprire { $name }: { $reason }
# machine-drafted, needs native review
queue-playlist-unreadable = il file non si può leggere.
# machine-drafted, needs native review
queue-playlist-damaged = non è una playlist che Specular sa leggere.
# machine-drafted, needs native review
queue-clear-ask = { $count ->
    [one] Svuotare la coda (1 elemento)?
   *[other] Svuotare la coda ({ $count } elementi)?
}
# machine-drafted, needs native review
queue-clear-detail = Viene svuotato solo l’elenco. I file restano sul disco.
# machine-drafted, needs native review
queue-clear-ok = Svuota coda
# machine-drafted, needs native review
queue-cleared = Coda svuotata.
# machine-drafted, needs native review
queue-save-title = Salva la coda come playlist
# machine-drafted, needs native review
queue-default-name = Coda
# machine-drafted, needs native review
queue-format-m3u8 = Playlist M3U8
# machine-drafted, needs native review
queue-format-xspf = Playlist XSPF
# machine-drafted, needs native review
queue-saved = { $name } salvato.
# machine-drafted, needs native review
queue-save-failed = Impossibile salvare { $name }: { $reason }
# machine-drafted, needs native review
settings-viewing-sound-play-next = Riproduci il file audio successivo
# machine-drafted, needs native review
settings-viewing-sound-play-next-help = Quando un file audio finisce, parte il file audio successivo della cartella.
# machine-drafted, needs native review
settings-viewing-play-repeat = Ripeti
# machine-drafted, needs native review
settings-viewing-play-repeat-help = Per file audio, video e la coda.
# machine-drafted, needs native review
settings-viewing-play-repeat-off = No
# machine-drafted, needs native review
settings-viewing-play-repeat-all = Tutti
# machine-drafted, needs native review
settings-viewing-play-repeat-one = Uno
# machine-drafted, needs native review
settings-viewing-play-shuffle = Riproduzione casuale
# machine-drafted, needs native review
settings-viewing-play-shuffle-help = Riproduci in ordine casuale, ogni elemento una volta per giro.
