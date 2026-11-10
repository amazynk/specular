# The play queue: what plays next, repeat and shuffle, playlists (M3U, PLS, XSPF), the
# queue panel and the sound transport's buttons.

# machine-drafted, needs native review
cheatsheet-video-queue = Kø, gentag og bland
# machine-drafted, needs native review
palette-next-track = Næste element
# machine-drafted, needs native review
palette-next-track-help = Afspil næste element i køen, ellers næste fil af samme slags i mappen.
# machine-drafted, needs native review
palette-prev-track = Forrige element
# machine-drafted, needs native review
palette-prev-track-help = Afspil forrige element. Efter 3 sekunder starter dette forfra.
# machine-drafted, needs native review
palette-cycle-repeat = Gentag
# machine-drafted, needs native review
palette-cycle-repeat-help = Gentag fra, alle eller ét.
# machine-drafted, needs native review
palette-shuffle = Bland
# machine-drafted, needs native review
palette-shuffle-help = Afspil i tilfældig rækkefølge, hvert element én gang pr. runde.
# machine-drafted, needs native review
palette-play-queue = Afspilningskø
# machine-drafted, needs native review
palette-play-queue-help = Vis, hvad der spiller herefter: omordn, fjern, gem.
# machine-drafted, needs native review
palette-queue-play-next = Afspil som næste
# machine-drafted, needs native review
palette-queue-play-next-help = Afspil filen (i galleriet markeringen) efter det, der spiller nu.
# machine-drafted, needs native review
palette-queue-add = Føj til kø
# machine-drafted, needs native review
palette-queue-add-help = Sæt filen (i galleriet markeringen) sidst i køen.
# machine-drafted, needs native review
palette-save-queue = Gem kø som afspilningsliste…
# machine-drafted, needs native review
palette-save-queue-help = Gem køen som afspilningsliste i M3U8 eller XSPF.
# machine-drafted, needs native review
transport-prev-track = Forrige element
# machine-drafted, needs native review
transport-next-track = Næste element
# machine-drafted, needs native review
queue-title = Afspilningskø
# machine-drafted, needs native review
queue-summary = { $count ->
    [one] 1 element
   *[other] { $count } elementer
}
# machine-drafted, needs native review
queue-summary-time = { $count ->
    [one] 1 element
   *[other] { $count } elementer
} · { $time }
# machine-drafted, needs native review
queue-empty = Køen er tom.
    Tilføj lydfiler og videoer med Afspil som næste eller Føj til kø, eller åbn en afspilningsliste.
# machine-drafted, needs native review
queue-empty-short = Køen er tom.
# machine-drafted, needs native review
queue-playing = Spiller
# machine-drafted, needs native review
queue-repeat = Gentag
# machine-drafted, needs native review
queue-repeat-off = Gentag: fra
# machine-drafted, needs native review
queue-repeat-all = Gentag: alle
# machine-drafted, needs native review
queue-repeat-one = Gentag: ét
# machine-drafted, needs native review
queue-shuffle = Bland
# machine-drafted, needs native review
queue-shuffle-on = Bland: til
# machine-drafted, needs native review
queue-shuffle-off = Bland: fra
# machine-drafted, needs native review
queue-save = Gem som afspilningsliste…
# machine-drafted, needs native review
queue-save-tip = Gem køen som afspilningsliste i M3U8 eller XSPF
# machine-drafted, needs native review
queue-clear = Ryd…
# machine-drafted, needs native review
queue-clear-tip = Ryd køen. Filerne bliver på disken.
# machine-drafted, needs native review
queue-remove = Fjern fra kø (filen bliver på disken)
# machine-drafted, needs native review
queue-network-item = Netværkselement, afspilles ikke: { $address }
# machine-drafted, needs native review
queue-artist-title = { $artist } – { $title }
# machine-drafted, needs native review
queue-at-end = Intet næste element.
# machine-drafted, needs native review
queue-at-start = Intet forrige element.
# machine-drafted, needs native review
queue-missing = { $name } findes ikke længere. Sprunget over.
# machine-drafted, needs native review
queue-network-not-played = Netværkselementer afspilles ikke endnu.
# machine-drafted, needs native review
queue-nothing-to-add = Intet at tilføje. Kun lydfiler, videoer og afspilningslister kommer i køen.
# machine-drafted, needs native review
queue-added = { $count ->
    [one] 1 element føjet til køen.
   *[other] { $count } elementer føjet til køen.
}
# machine-drafted, needs native review
queue-added-next = { $count ->
    [one] 1 element spiller som næste.
   *[other] { $count } elementer spiller som næste.
}
# machine-drafted, needs native review
queue-playlist-opened = { $name }: { $count ->
    [one] 1 element i køen.
   *[other] { $count } elementer i køen.
}
# machine-drafted, needs native review
queue-playlist-nothing = Intet i { $name } kan afspilles her. Køen viser, hvad den indeholder.
# machine-drafted, needs native review
queue-playlist-failed = { $name } kan ikke åbnes: { $reason }
# machine-drafted, needs native review
queue-playlist-unreadable = filen kan ikke læses.
# machine-drafted, needs native review
queue-playlist-damaged = det er ikke en afspilningsliste, Specular kan læse.
# machine-drafted, needs native review
queue-clear-ask = { $count ->
    [one] Ryd køen (1 element)?
   *[other] Ryd køen ({ $count } elementer)?
}
# machine-drafted, needs native review
queue-clear-detail = Kun listen ryddes. Filerne bliver på disken.
# machine-drafted, needs native review
queue-clear-ok = Ryd kø
# machine-drafted, needs native review
queue-cleared = Køen er ryddet.
# machine-drafted, needs native review
queue-save-title = Gem kø som afspilningsliste
# machine-drafted, needs native review
queue-default-name = Kø
# machine-drafted, needs native review
queue-format-m3u8 = M3U8-afspilningsliste
# machine-drafted, needs native review
queue-format-xspf = XSPF-afspilningsliste
# machine-drafted, needs native review
queue-saved = { $name } er gemt.
# machine-drafted, needs native review
queue-save-failed = { $name } kan ikke gemmes: { $reason }
# machine-drafted, needs native review
settings-viewing-sound-play-next = Afspil næste lydfil
# machine-drafted, needs native review
settings-viewing-sound-play-next-help = Når en lydfil slutter, afspilles næste lydfil i mappen.
# machine-drafted, needs native review
settings-viewing-play-repeat = Gentag
# machine-drafted, needs native review
settings-viewing-play-repeat-help = For lydfiler, videoer og køen.
# machine-drafted, needs native review
settings-viewing-play-repeat-off = Fra
# machine-drafted, needs native review
settings-viewing-play-repeat-all = Alle
# machine-drafted, needs native review
settings-viewing-play-repeat-one = Ét
# machine-drafted, needs native review
settings-viewing-play-shuffle = Bland
# machine-drafted, needs native review
settings-viewing-play-shuffle-help = Afspil i tilfældig rækkefølge, hvert element én gang pr. runde.
