# The play queue: what plays next, repeat and shuffle, playlists (M3U, PLS, XSPF), the
# queue panel and the sound transport's buttons.

# machine-drafted, needs native review
cheatsheet-video-queue = Wachtrij, herhalen en willekeurig
# machine-drafted, needs native review
palette-next-track = Volgend item
# machine-drafted, needs native review
palette-next-track-help = Het volgende item van de wachtrij afspelen, anders het volgende bestand van dezelfde soort in de map.
# machine-drafted, needs native review
palette-prev-track = Vorig item
# machine-drafted, needs native review
palette-prev-track-help = Het vorige item afspelen. Na 3 seconden dit item opnieuw beginnen.
# machine-drafted, needs native review
palette-cycle-repeat = Herhalen
# machine-drafted, needs native review
palette-cycle-repeat-help = Herhalen uit, alles of één.
# machine-drafted, needs native review
palette-shuffle = Willekeurig afspelen
# machine-drafted, needs native review
palette-shuffle-help = In willekeurige volgorde afspelen, elk item één keer per ronde.
# machine-drafted, needs native review
palette-play-queue = Afspeelwachtrij
# machine-drafted, needs native review
palette-play-queue-help = Tonen wat hierna speelt: herschikken, verwijderen, opslaan.
# machine-drafted, needs native review
palette-queue-play-next = Hierna afspelen
# machine-drafted, needs native review
palette-queue-play-next-help = Het bestand (in de galerij de selectie) na het spelende item afspelen.
# machine-drafted, needs native review
palette-queue-add = Aan wachtrij toevoegen
# machine-drafted, needs native review
palette-queue-add-help = Het bestand (in de galerij de selectie) achteraan de wachtrij zetten.
# machine-drafted, needs native review
palette-save-queue = Wachtrij als afspeellijst opslaan…
# machine-drafted, needs native review
palette-save-queue-help = De wachtrij als M3U8- of XSPF-afspeellijst opslaan.
# machine-drafted, needs native review
transport-prev-track = Vorig item
# machine-drafted, needs native review
transport-next-track = Volgend item
# machine-drafted, needs native review
queue-title = Afspeelwachtrij
# machine-drafted, needs native review
queue-summary = { $count ->
    [one] 1 item
   *[other] { $count } items
}
# machine-drafted, needs native review
queue-summary-time = { $count ->
    [one] 1 item
   *[other] { $count } items
} · { $time }
# machine-drafted, needs native review
queue-empty = De wachtrij is leeg.
    Voeg geluidsbestanden en video’s toe met Hierna afspelen of Aan wachtrij toevoegen, of open een afspeellijst.
# machine-drafted, needs native review
queue-empty-short = De wachtrij is leeg.
# machine-drafted, needs native review
queue-playing = Speelt
# machine-drafted, needs native review
queue-repeat = Herhalen
# machine-drafted, needs native review
queue-repeat-off = Herhalen: uit
# machine-drafted, needs native review
queue-repeat-all = Herhalen: alles
# machine-drafted, needs native review
queue-repeat-one = Herhalen: één
# machine-drafted, needs native review
queue-shuffle = Willekeurig
# machine-drafted, needs native review
queue-shuffle-on = Willekeurig: aan
# machine-drafted, needs native review
queue-shuffle-off = Willekeurig: uit
# machine-drafted, needs native review
queue-save = Opslaan als afspeellijst…
# machine-drafted, needs native review
queue-save-tip = De wachtrij als M3U8- of XSPF-afspeellijst opslaan
# machine-drafted, needs native review
queue-clear = Leegmaken…
# machine-drafted, needs native review
queue-clear-tip = De wachtrij leegmaken. De bestanden blijven op de schijf.
# machine-drafted, needs native review
queue-remove = Uit wachtrij verwijderen (bestand blijft op de schijf)
# machine-drafted, needs native review
queue-network-item = Netwerkitem, wordt niet afgespeeld: { $address }
# machine-drafted, needs native review
queue-artist-title = { $artist } – { $title }
# machine-drafted, needs native review
queue-at-end = Geen volgend item.
# machine-drafted, needs native review
queue-at-start = Geen vorig item.
# machine-drafted, needs native review
queue-missing = { $name } is er niet meer. Overgeslagen.
# machine-drafted, needs native review
queue-network-not-played = Netwerkitems worden nog niet afgespeeld.
# machine-drafted, needs native review
queue-nothing-to-add = Niets toe te voegen. Alleen geluidsbestanden, video’s en afspeellijsten gaan in de wachtrij.
# machine-drafted, needs native review
queue-added = { $count ->
    [one] 1 item aan de wachtrij toegevoegd.
   *[other] { $count } items aan de wachtrij toegevoegd.
}
# machine-drafted, needs native review
queue-added-next = { $count ->
    [one] 1 item speelt hierna.
   *[other] { $count } items spelen hierna.
}
# machine-drafted, needs native review
queue-playlist-opened = { $name }: { $count ->
    [one] 1 item in de wachtrij.
   *[other] { $count } items in de wachtrij.
}
# machine-drafted, needs native review
queue-playlist-nothing = Niets in { $name } kan hier spelen. De wachtrij toont wat erin staat.
# machine-drafted, needs native review
queue-playlist-failed = Kan { $name } niet openen: { $reason }
# machine-drafted, needs native review
queue-playlist-unreadable = het bestand kan niet worden gelezen.
# machine-drafted, needs native review
queue-playlist-damaged = het is geen afspeellijst die Specular kan lezen.
# machine-drafted, needs native review
queue-clear-ask = { $count ->
    [one] Wachtrij leegmaken (1 item)?
   *[other] Wachtrij leegmaken ({ $count } items)?
}
# machine-drafted, needs native review
queue-clear-detail = Alleen de lijst wordt leeggemaakt. De bestanden blijven op de schijf.
# machine-drafted, needs native review
queue-clear-ok = Wachtrij leegmaken
# machine-drafted, needs native review
queue-cleared = Wachtrij leeggemaakt.
# machine-drafted, needs native review
queue-save-title = Wachtrij als afspeellijst opslaan
# machine-drafted, needs native review
queue-default-name = Wachtrij
# machine-drafted, needs native review
queue-format-m3u8 = M3U8-afspeellijst
# machine-drafted, needs native review
queue-format-xspf = XSPF-afspeellijst
# machine-drafted, needs native review
queue-saved = { $name } opgeslagen.
# machine-drafted, needs native review
queue-save-failed = Kan { $name } niet opslaan: { $reason }
# machine-drafted, needs native review
settings-viewing-sound-play-next = Volgend geluidsbestand afspelen
# machine-drafted, needs native review
settings-viewing-sound-play-next-help = Als een geluidsbestand afloopt, speelt het volgende geluidsbestand in de map.
# machine-drafted, needs native review
settings-viewing-play-repeat = Herhalen
# machine-drafted, needs native review
settings-viewing-play-repeat-help = Voor geluidsbestanden, video’s en de wachtrij.
# machine-drafted, needs native review
settings-viewing-play-repeat-off = Uit
# machine-drafted, needs native review
settings-viewing-play-repeat-all = Alles
# machine-drafted, needs native review
settings-viewing-play-repeat-one = Eén
# machine-drafted, needs native review
settings-viewing-play-shuffle = Willekeurig afspelen
# machine-drafted, needs native review
settings-viewing-play-shuffle-help = In willekeurige volgorde afspelen, elk item één keer per ronde.
