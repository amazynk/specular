# The play queue: what plays next, repeat and shuffle, playlists (M3U, PLS, XSPF), the
# queue panel and the sound transport's buttons.

# machine-drafted, needs native review
cheatsheet-video-queue = Jono, toisto ja satunnaistoisto
# machine-drafted, needs native review
palette-next-track = Seuraava kohde
# machine-drafted, needs native review
palette-next-track-help = Toista jonon seuraava kohde, muuten kansion seuraava samanlainen tiedosto.
# machine-drafted, needs native review
palette-prev-track = Edellinen kohde
# machine-drafted, needs native review
palette-prev-track-help = Toista edellinen kohde. 3 sekunnin jälkeen tämä alkaa alusta.
# machine-drafted, needs native review
palette-cycle-repeat = Toista
# machine-drafted, needs native review
palette-cycle-repeat-help = Toisto pois, kaikki tai yksi.
# machine-drafted, needs native review
palette-shuffle = Satunnaistoisto
# machine-drafted, needs native review
palette-shuffle-help = Toista satunnaisessa järjestyksessä, jokainen kohde kerran kierroksella.
# machine-drafted, needs native review
palette-play-queue = Toistojono
# machine-drafted, needs native review
palette-play-queue-help = Näytä, mitä toistetaan seuraavaksi: järjestä, poista, tallenna.
# machine-drafted, needs native review
palette-queue-play-next = Toista seuraavaksi
# machine-drafted, needs native review
palette-queue-play-next-help = Toista tiedosto (galleriassa valinta) nyt soivan kohteen jälkeen.
# machine-drafted, needs native review
palette-queue-add = Lisää jonoon
# machine-drafted, needs native review
palette-queue-add-help = Lisää tiedosto (galleriassa valinta) jonon loppuun.
# machine-drafted, needs native review
palette-save-queue = Tallenna jono soittolistaksi…
# machine-drafted, needs native review
palette-save-queue-help = Tallenna jono M3U8- tai XSPF-soittolistaksi.
# machine-drafted, needs native review
transport-prev-track = Edellinen kohde
# machine-drafted, needs native review
transport-next-track = Seuraava kohde
# machine-drafted, needs native review
queue-title = Toistojono
# machine-drafted, needs native review
queue-summary = { $count ->
    [one] 1 kohde
   *[other] { $count } kohdetta
}
# machine-drafted, needs native review
queue-summary-time = { $count ->
    [one] 1 kohde
   *[other] { $count } kohdetta
} · { $time }
# machine-drafted, needs native review
queue-empty = Jono on tyhjä.
    Lisää äänitiedostoja ja videoita toiminnoilla Toista seuraavaksi tai Lisää jonoon, tai avaa soittolista.
# machine-drafted, needs native review
queue-empty-short = Jono on tyhjä.
# machine-drafted, needs native review
queue-playing = Soi
# machine-drafted, needs native review
queue-repeat = Toista
# machine-drafted, needs native review
queue-repeat-off = Toisto: pois
# machine-drafted, needs native review
queue-repeat-all = Toisto: kaikki
# machine-drafted, needs native review
queue-repeat-one = Toisto: yksi
# machine-drafted, needs native review
queue-shuffle = Satunnainen
# machine-drafted, needs native review
queue-shuffle-on = Satunnainen: päällä
# machine-drafted, needs native review
queue-shuffle-off = Satunnainen: pois
# machine-drafted, needs native review
queue-save = Tallenna soittolistaksi…
# machine-drafted, needs native review
queue-save-tip = Tallenna jono M3U8- tai XSPF-soittolistaksi
# machine-drafted, needs native review
queue-clear = Tyhjennä…
# machine-drafted, needs native review
queue-clear-tip = Tyhjennä jono. Tiedostot jäävät levylle.
# machine-drafted, needs native review
queue-remove = Poista jonosta (tiedosto jää levylle)
# machine-drafted, needs native review
queue-network-item = Verkkokohde, ei toisteta: { $address }
# machine-drafted, needs native review
queue-artist-title = { $artist } – { $title }
# machine-drafted, needs native review
queue-at-end = Ei seuraavaa kohdetta.
# machine-drafted, needs native review
queue-at-start = Ei edellistä kohdetta.
# machine-drafted, needs native review
queue-missing = { $name } ei ole enää tallessa. Ohitettiin.
# machine-drafted, needs native review
queue-network-not-played = Verkkokohteita ei vielä toisteta.
# machine-drafted, needs native review
queue-nothing-to-add = Ei lisättävää. Jonoon menevät vain äänitiedostot, videot ja soittolistat.
# machine-drafted, needs native review
queue-added = { $count ->
    [one] 1 kohde lisättiin jonoon.
   *[other] { $count } kohdetta lisättiin jonoon.
}
# machine-drafted, needs native review
queue-added-next = { $count ->
    [one] 1 kohde soi seuraavaksi.
   *[other] { $count } kohdetta soi seuraavaksi.
}
# machine-drafted, needs native review
queue-playlist-opened = { $name }: { $count ->
    [one] 1 kohde jonossa.
   *[other] { $count } kohdetta jonossa.
}
# machine-drafted, needs native review
queue-playlist-nothing = Mitään soittolistasta { $name } ei voi toistaa täällä. Jono näyttää sen sisällön.
# machine-drafted, needs native review
queue-playlist-failed = Tiedostoa { $name } ei voi avata: { $reason }
# machine-drafted, needs native review
queue-playlist-unreadable = tiedostoa ei voi lukea.
# machine-drafted, needs native review
queue-playlist-damaged = se ei ole soittolista, jota Specular osaa lukea.
# machine-drafted, needs native review
queue-clear-ask = { $count ->
    [one] Tyhjennetäänkö jono (1 kohde)?
   *[other] Tyhjennetäänkö jono ({ $count } kohdetta)?
}
# machine-drafted, needs native review
queue-clear-detail = Vain luettelo tyhjennetään. Tiedostot jäävät levylle.
# machine-drafted, needs native review
queue-clear-ok = Tyhjennä jono
# machine-drafted, needs native review
queue-cleared = Jono tyhjennettiin.
# machine-drafted, needs native review
queue-save-title = Tallenna jono soittolistaksi
# machine-drafted, needs native review
queue-default-name = Jono
# machine-drafted, needs native review
queue-format-m3u8 = M3U8-soittolista
# machine-drafted, needs native review
queue-format-xspf = XSPF-soittolista
# machine-drafted, needs native review
queue-saved = { $name } tallennettiin.
# machine-drafted, needs native review
queue-save-failed = Tiedostoa { $name } ei voi tallentaa: { $reason }
# machine-drafted, needs native review
settings-viewing-sound-play-next = Toista seuraava äänitiedosto
# machine-drafted, needs native review
settings-viewing-sound-play-next-help = Kun äänitiedosto loppuu, kansion seuraava äänitiedosto alkaa.
# machine-drafted, needs native review
settings-viewing-play-repeat = Toista
# machine-drafted, needs native review
settings-viewing-play-repeat-help = Äänitiedostoille, videoille ja jonolle.
# machine-drafted, needs native review
settings-viewing-play-repeat-off = Pois
# machine-drafted, needs native review
settings-viewing-play-repeat-all = Kaikki
# machine-drafted, needs native review
settings-viewing-play-repeat-one = Yksi
# machine-drafted, needs native review
settings-viewing-play-shuffle = Satunnaistoisto
# machine-drafted, needs native review
settings-viewing-play-shuffle-help = Toista satunnaisessa järjestyksessä, jokainen kohde kerran kierroksella.
