# The play queue: what plays next, repeat and shuffle, playlists (M3U, PLS, XSPF), the
# queue panel and the sound transport's buttons.

# machine-drafted, needs native review
cheatsheet-video-queue = Fronta, opakování a náhodné pořadí
# machine-drafted, needs native review
palette-next-track = Další položka
# machine-drafted, needs native review
palette-next-track-help = Přehrát další položku fronty, jinak další soubor stejného druhu ve složce.
# machine-drafted, needs native review
palette-prev-track = Předchozí položka
# machine-drafted, needs native review
palette-prev-track-help = Přehrát předchozí položku. Po 3 sekundách začne tato znovu.
# machine-drafted, needs native review
palette-cycle-repeat = Opakovat
# machine-drafted, needs native review
palette-cycle-repeat-help = Opakování vypnuto, vše nebo jednu.
# machine-drafted, needs native review
palette-shuffle = Náhodné pořadí
# machine-drafted, needs native review
palette-shuffle-help = Přehrávat v náhodném pořadí, každou položku jednou za kolo.
# machine-drafted, needs native review
palette-play-queue = Fronta přehrávání
# machine-drafted, needs native review
palette-play-queue-help = Ukázat, co hraje dál: přeuspořádat, odebrat, uložit.
# machine-drafted, needs native review
palette-queue-play-next = Přehrát jako další
# machine-drafted, needs native review
palette-queue-play-next-help = Přehrát soubor (v galerii výběr) po právě hrající položce.
# machine-drafted, needs native review
palette-queue-add = Přidat do fronty
# machine-drafted, needs native review
palette-queue-add-help = Přidat soubor (v galerii výběr) na konec fronty.
# machine-drafted, needs native review
palette-save-queue = Uložit frontu jako seznam skladeb…
# machine-drafted, needs native review
palette-save-queue-help = Uložit frontu jako seznam skladeb M3U8 nebo XSPF.
# machine-drafted, needs native review
transport-prev-track = Předchozí položka
# machine-drafted, needs native review
transport-next-track = Další položka
# machine-drafted, needs native review
queue-title = Fronta přehrávání
# machine-drafted, needs native review
queue-summary = { $count ->
    [one] 1 položka
    [few] { $count } položky
    [many] { $count } položky
   *[other] { $count } položek
}
# machine-drafted, needs native review
queue-summary-time = { $count ->
    [one] 1 položka
    [few] { $count } položky
    [many] { $count } položky
   *[other] { $count } položek
} · { $time }
# machine-drafted, needs native review
queue-empty = Fronta je prázdná.
    Přidejte zvukové soubory a videa pomocí Přehrát jako další nebo Přidat do fronty, nebo otevřete seznam skladeb.
# machine-drafted, needs native review
queue-empty-short = Fronta je prázdná.
# machine-drafted, needs native review
queue-playing = Hraje
# machine-drafted, needs native review
queue-repeat = Opakovat
# machine-drafted, needs native review
queue-repeat-off = Opakovat: ne
# machine-drafted, needs native review
queue-repeat-all = Opakovat: vše
# machine-drafted, needs native review
queue-repeat-one = Opakovat: jednu
# machine-drafted, needs native review
queue-shuffle = Náhodně
# machine-drafted, needs native review
queue-shuffle-on = Náhodně: ano
# machine-drafted, needs native review
queue-shuffle-off = Náhodně: ne
# machine-drafted, needs native review
queue-save = Uložit jako seznam skladeb…
# machine-drafted, needs native review
queue-save-tip = Uložit frontu jako seznam skladeb M3U8 nebo XSPF
# machine-drafted, needs native review
queue-clear = Vyprázdnit…
# machine-drafted, needs native review
queue-clear-tip = Vyprázdnit frontu. Soubory zůstanou na disku.
# machine-drafted, needs native review
queue-remove = Odebrat z fronty (soubor zůstane na disku)
# machine-drafted, needs native review
queue-network-item = Síťová položka, nepřehrává se: { $address }
# machine-drafted, needs native review
queue-artist-title = { $artist } – { $title }
# machine-drafted, needs native review
queue-at-end = Žádná další položka.
# machine-drafted, needs native review
queue-at-start = Žádná předchozí položka.
# machine-drafted, needs native review
queue-missing = { $name } už neexistuje. Přeskočeno.
# machine-drafted, needs native review
queue-network-not-played = Síťové položky se zatím nepřehrávají.
# machine-drafted, needs native review
queue-nothing-to-add = Nic k přidání. Do fronty patří jen zvukové soubory, videa a seznamy skladeb.
# machine-drafted, needs native review
queue-added = { $count ->
    [one] Do fronty přidána 1 položka.
    [few] Do fronty přidány { $count } položky.
    [many] Do fronty přidáno { $count } položky.
   *[other] Do fronty přidáno { $count } položek.
}
# machine-drafted, needs native review
queue-added-next = { $count ->
    [one] 1 položka hraje jako další.
    [few] { $count } položky hrají jako další.
    [many] { $count } položky hraje jako další.
   *[other] { $count } položek hraje jako další.
}
# machine-drafted, needs native review
queue-playlist-opened = { $name }: { $count ->
    [one] 1 položka ve frontě.
    [few] { $count } položky ve frontě.
    [many] { $count } položky ve frontě.
   *[other] { $count } položek ve frontě.
}
# machine-drafted, needs native review
queue-playlist-nothing = Nic ze seznamu { $name } zde nelze přehrát. Fronta ukazuje, co obsahuje.
# machine-drafted, needs native review
queue-playlist-failed = { $name } nelze otevřít: { $reason }
# machine-drafted, needs native review
queue-playlist-unreadable = soubor nelze přečíst.
# machine-drafted, needs native review
queue-playlist-damaged = není to seznam skladeb, který Specular umí přečíst.
# machine-drafted, needs native review
queue-clear-ask = { $count ->
    [one] Vyprázdnit frontu (1 položka)?
    [few] Vyprázdnit frontu ({ $count } položky)?
    [many] Vyprázdnit frontu ({ $count } položky)?
   *[other] Vyprázdnit frontu ({ $count } položek)?
}
# machine-drafted, needs native review
queue-clear-detail = Vyprázdní se jen seznam. Soubory zůstanou na disku.
# machine-drafted, needs native review
queue-clear-ok = Vyprázdnit frontu
# machine-drafted, needs native review
queue-cleared = Fronta vyprázdněna.
# machine-drafted, needs native review
queue-save-title = Uložit frontu jako seznam skladeb
# machine-drafted, needs native review
queue-default-name = Fronta
# machine-drafted, needs native review
queue-format-m3u8 = Seznam skladeb M3U8
# machine-drafted, needs native review
queue-format-xspf = Seznam skladeb XSPF
# machine-drafted, needs native review
queue-saved = { $name } uloženo.
# machine-drafted, needs native review
queue-save-failed = { $name } nelze uložit: { $reason }
# machine-drafted, needs native review
settings-viewing-sound-play-next = Přehrát další zvukový soubor
# machine-drafted, needs native review
settings-viewing-sound-play-next-help = Když zvukový soubor skončí, přehraje se další zvukový soubor ve složce.
# machine-drafted, needs native review
settings-viewing-play-repeat = Opakovat
# machine-drafted, needs native review
settings-viewing-play-repeat-help = Pro zvukové soubory, videa a frontu.
# machine-drafted, needs native review
settings-viewing-play-repeat-off = Ne
# machine-drafted, needs native review
settings-viewing-play-repeat-all = Vše
# machine-drafted, needs native review
settings-viewing-play-repeat-one = Jednu
# machine-drafted, needs native review
settings-viewing-play-shuffle = Náhodné pořadí
# machine-drafted, needs native review
settings-viewing-play-shuffle-help = Přehrávat v náhodném pořadí, každou položku jednou za kolo.
