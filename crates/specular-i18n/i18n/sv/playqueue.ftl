# The play queue: what plays next, repeat and shuffle, playlists (M3U, PLS, XSPF), the
# queue panel and the sound transport's buttons.

# machine-drafted, needs native review
cheatsheet-video-queue = Kö, upprepa och blanda
# machine-drafted, needs native review
palette-next-track = Nästa objekt
# machine-drafted, needs native review
palette-next-track-help = Spela nästa objekt i kön, annars nästa fil av samma slag i mappen.
# machine-drafted, needs native review
palette-prev-track = Föregående objekt
# machine-drafted, needs native review
palette-prev-track-help = Spela föregående objekt. Efter 3 sekunder börjar det här om.
# machine-drafted, needs native review
palette-cycle-repeat = Upprepa
# machine-drafted, needs native review
palette-cycle-repeat-help = Upprepa av, alla eller ett.
# machine-drafted, needs native review
palette-shuffle = Blanda
# machine-drafted, needs native review
palette-shuffle-help = Spela i slumpvis ordning, varje objekt en gång per varv.
# machine-drafted, needs native review
palette-play-queue = Spelkö
# machine-drafted, needs native review
palette-play-queue-help = Visa vad som spelas härnäst: ordna om, ta bort, spara.
# machine-drafted, needs native review
palette-queue-play-next = Spela härnäst
# machine-drafted, needs native review
palette-queue-play-next-help = Spela filen (i galleriet urvalet) efter det som spelas nu.
# machine-drafted, needs native review
palette-queue-add = Lägg till i kön
# machine-drafted, needs native review
palette-queue-add-help = Lägg filen (i galleriet urvalet) sist i kön.
# machine-drafted, needs native review
palette-save-queue = Spara kön som spellista…
# machine-drafted, needs native review
palette-save-queue-help = Spara kön som spellista i M3U8 eller XSPF.
# machine-drafted, needs native review
transport-prev-track = Föregående objekt
# machine-drafted, needs native review
transport-next-track = Nästa objekt
# machine-drafted, needs native review
queue-title = Spelkö
# machine-drafted, needs native review
queue-summary = { $count ->
    [one] 1 objekt
   *[other] { $count } objekt
}
# machine-drafted, needs native review
queue-summary-time = { $count ->
    [one] 1 objekt
   *[other] { $count } objekt
} · { $time }
# machine-drafted, needs native review
queue-empty = Kön är tom.
    Lägg till ljudfiler och videor med Spela härnäst eller Lägg till i kön, eller öppna en spellista.
# machine-drafted, needs native review
queue-empty-short = Kön är tom.
# machine-drafted, needs native review
queue-playing = Spelas
# machine-drafted, needs native review
queue-repeat = Upprepa
# machine-drafted, needs native review
queue-repeat-off = Upprepa: av
# machine-drafted, needs native review
queue-repeat-all = Upprepa: alla
# machine-drafted, needs native review
queue-repeat-one = Upprepa: ett
# machine-drafted, needs native review
queue-shuffle = Blanda
# machine-drafted, needs native review
queue-shuffle-on = Blanda: på
# machine-drafted, needs native review
queue-shuffle-off = Blanda: av
# machine-drafted, needs native review
queue-save = Spara som spellista…
# machine-drafted, needs native review
queue-save-tip = Spara kön som spellista i M3U8 eller XSPF
# machine-drafted, needs native review
queue-clear = Töm…
# machine-drafted, needs native review
queue-clear-tip = Töm kön. Filerna ligger kvar på disken.
# machine-drafted, needs native review
queue-remove = Ta bort ur kön (filen ligger kvar på disken)
# machine-drafted, needs native review
queue-network-item = Nätverksobjekt, spelas inte: { $address }
# machine-drafted, needs native review
queue-artist-title = { $artist } – { $title }
# machine-drafted, needs native review
queue-at-end = Inget nästa objekt.
# machine-drafted, needs native review
queue-at-start = Inget föregående objekt.
# machine-drafted, needs native review
queue-missing = { $name } finns inte längre. Hoppades över.
# machine-drafted, needs native review
queue-network-not-played = Nätverksobjekt spelas inte än.
# machine-drafted, needs native review
queue-nothing-to-add = Inget att lägga till. Bara ljudfiler, videor och spellistor går in i kön.
# machine-drafted, needs native review
queue-added = { $count ->
    [one] 1 objekt lades till i kön.
   *[other] { $count } objekt lades till i kön.
}
# machine-drafted, needs native review
queue-added-next = { $count ->
    [one] 1 objekt spelas härnäst.
   *[other] { $count } objekt spelas härnäst.
}
# machine-drafted, needs native review
queue-playlist-opened = { $name }: { $count ->
    [one] 1 objekt i kön.
   *[other] { $count } objekt i kön.
}
# machine-drafted, needs native review
queue-playlist-nothing = Inget i { $name } kan spelas här. Kön visar vad den innehåller.
# machine-drafted, needs native review
queue-playlist-failed = Det går inte att öppna { $name }: { $reason }
# machine-drafted, needs native review
queue-playlist-unreadable = filen kan inte läsas.
# machine-drafted, needs native review
queue-playlist-damaged = det är ingen spellista som Specular kan läsa.
# machine-drafted, needs native review
queue-clear-ask = { $count ->
    [one] Tömma kön (1 objekt)?
   *[other] Tömma kön ({ $count } objekt)?
}
# machine-drafted, needs native review
queue-clear-detail = Bara listan töms. Filerna ligger kvar på disken.
# machine-drafted, needs native review
queue-clear-ok = Töm kön
# machine-drafted, needs native review
queue-cleared = Kön tömdes.
# machine-drafted, needs native review
queue-save-title = Spara kön som spellista
# machine-drafted, needs native review
queue-default-name = Kö
# machine-drafted, needs native review
queue-format-m3u8 = M3U8-spellista
# machine-drafted, needs native review
queue-format-xspf = XSPF-spellista
# machine-drafted, needs native review
queue-saved = { $name } sparades.
# machine-drafted, needs native review
queue-save-failed = Det går inte att spara { $name }: { $reason }
# machine-drafted, needs native review
settings-viewing-sound-play-next = Spela nästa ljudfil
# machine-drafted, needs native review
settings-viewing-sound-play-next-help = När en ljudfil tar slut spelas nästa ljudfil i mappen.
# machine-drafted, needs native review
settings-viewing-play-repeat = Upprepa
# machine-drafted, needs native review
settings-viewing-play-repeat-help = För ljudfiler, videor och kön.
# machine-drafted, needs native review
settings-viewing-play-repeat-off = Av
# machine-drafted, needs native review
settings-viewing-play-repeat-all = Alla
# machine-drafted, needs native review
settings-viewing-play-repeat-one = Ett
# machine-drafted, needs native review
settings-viewing-play-shuffle = Blanda
# machine-drafted, needs native review
settings-viewing-play-shuffle-help = Spela i slumpvis ordning, varje objekt en gång per varv.
