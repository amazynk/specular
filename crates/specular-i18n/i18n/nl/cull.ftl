# machine-drafted, needs native review
# Specular — Selectiemodus (DESIGN.md §6.3).

palette-cull = Selectiemodus openen of verlaten
cheatsheet-cull = Selectiemodus
settings-section-cull = Selectiemodus
settings-cull-auto-enter = RAW-mappen selecteren
settings-cull-auto-enter-help = Een map met vooral RAW-bestanden direct in de selectiemodus openen.
settings-cull-auto-advance = Volgende afbeelding na markeren
settings-cull-auto-advance-help = Ga verder na kiezen, afwijzen of een sterrenwaardering. Labels en markering wissen blijven op de afbeelding.
settings-cull-show = Tonen bij RAW+JPEG-paren
settings-cull-show-help = Een RAW en een JPEG met dezelfde naam worden als één afbeelding geselecteerd en markeringen gelden voor beide. De JPEG opent sneller.
settings-cull-show-jpeg = JPEG
settings-cull-show-raw = RAW

# machine-drafted, needs native review
settings-cull-bursts = Reeksen stapelen
settings-cull-bursts-help = Opnamen die kort na elkaar met één camera zijn gemaakt, worden als één stapel getoond. S opent of sluit een stapel, B houdt de getoonde afbeelding en wijst de rest van de stapel af.
settings-cull-burst-gap = Reekspauze
settings-cull-burst-gap-help = De langste pauze tussen twee opnamen van één reeks.
settings-cull-watch = Nieuwe bestanden tonen
settings-cull-watch-help = Bestanden die tijdens het selecteren in de map verschijnen (een aangesloten camera, een import) worden achteraan toegevoegd zodra ze volledig zijn geschreven.
settings-cull-follow-newest = Nieuwste bestand volgen
settings-cull-follow-newest-help = Naar elk nieuw bestand springen zodra het binnenkomt. T schakelt dit tijdens het selecteren.
settings-cull-slot-1 = Map 1
settings-cull-slot-1-help = Alt+1 kopieert de afbeelding (met de RAW of JPEG en de sidecars) naar deze map, Alt+Shift+1 verplaatst ze erheen.
settings-cull-slot-1-unset = Niet ingesteld
settings-cull-slot-2 = Map 2
settings-cull-slot-2-help = Alt+2 kopieert de afbeelding (met de RAW of JPEG en de sidecars) naar deze map, Alt+Shift+2 verplaatst ze erheen.
settings-cull-slot-2-unset = Niet ingesteld
settings-cull-slot-3 = Map 3
settings-cull-slot-3-help = Alt+3 kopieert de afbeelding (met de RAW of JPEG en de sidecars) naar deze map, Alt+Shift+3 verplaatst ze erheen.
settings-cull-slot-3-unset = Niet ingesteld
settings-cull-collisions = Als een naam bezet is
settings-cull-collisions-help = Bestanden in de mappen worden nooit vervangen.
settings-cull-collisions-suffix = Een nummer toevoegen
settings-cull-collisions-skip = De afbeelding overslaan
# machine-drafted, needs native review
# Selectiemodus gekozen zonder geopende map of galerijafbeeldingen.
cull-nothing = Niets om te selecteren — open eerst een map met foto's
cull-slot-unset = Geen map { $slot } ingesteld — kies er een in Instellingen → Selectiemodus
cull-slot-missing = Map { $slot } is niet beschikbaar: { $reason }
cull-copied = Gekopieerd naar { $folder }
cull-moved = Verplaatst naar { $folder }
cull-slot-renamed = Opgeslagen als { $name } in { $folder }
cull-slot-skipped = { $folder } bevat al { $name } — overgeslagen
cull-slot-same = De afbeelding staat al in { $folder }
cull-slot-failed = { $op ->
    [move] { $file } kon niet worden verplaatst
   *[copy] { $file } kon niet worden gekopieerd
}: { $reason }
cull-copy-kept = { $count ->
    [one] Een sindsdien gewijzigde kopie behouden
   *[other] { $count } sindsdien gewijzigde kopieën behouden
}
cull-undo-failed = Ongedaan maken lukt niet voor { $file }: { $reason }
cull-burst-open = Reeks { $index } van { $count }
cull-burst-closed = Reeks van { $count }
cull-burst-toggle = Reeks openen of sluiten
cull-burst-best = 1 behouden, { $count } afgewezen
cull-burst-none = Geen deel van een reeks
cull-follow-on = Nieuwe bestanden volgen aan
cull-follow-off = Nieuwe bestanden volgen uit
cull-new-files = { $count ->
    [one] 1 nieuw bestand
   *[other] { $count } nieuwe bestanden
}
cull-key-copy = Kopiëren naar map 1–3
cull-key-move = Verplaatsen naar map 1–3
cull-key-burst = Reeks openen of sluiten
cull-key-best = Deze houden, rest van de reeks afwijzen
cull-key-follow = Nieuwe bestanden volgen
cull-key-subframes = Subframetabel tonen of verbergen
cull-key-sort = Sorteren op de volgende kolom
cull-key-sort-reverse = Volgorde omkeren
cull-key-metric = Volgende meetwaarde in de grafiek
cull-key-sigma-down = Automatisch afwijzen: k verlagen
cull-key-sigma-up = Automatisch afwijzen: k verhogen
cull-key-auto-reject = Voorgestelde frames afwijzen
cull-key-move-rejects = Afgewezen frames naar een submap verplaatsen
cull-key-same-stretch = Dezelfde stretch voor elk frame

## Strip tooltips. machine-drafted, needs native review
cull-tip-burst-open = Reeks openen
cull-tip-burst-close = Reeks sluiten
cull-tip-exit = Selectiemodus verlaten
cull-tip-label = Label: { $label }

## Astronomy subframes. machine-drafted, needs native review
cull-astro-panel = Subframes
cull-astro-title = { $count ->
    [one] 1 subframe
   *[other] { $count } subframes
}
cull-astro-pending = { $count } worden gemeten
cull-astro-col-name = Frame
cull-astro-col-filter = Filter
cull-astro-col-exposure = Belicht.
cull-astro-col-hfd = HFD
cull-astro-col-fwhm = FWHM
cull-astro-col-ecc = Exc.
cull-astro-col-stars = Sterren
cull-astro-col-bg = Achtergrond
cull-astro-col-snr = SNR
cull-astro-col-noise = Ruis
cull-astro-same-stretch = Dezelfde stretch
cull-astro-own-stretch = Eigen stretch
cull-astro-suggest-high = { $metric } boven { $value } (mediaan + { $k }σ): { $count ->
    [one] 1 frame
   *[other] { $count } frames
}
cull-astro-suggest-low = { $metric } onder { $value } (mediaan − { $k }σ): { $count ->
    [one] 1 frame
   *[other] { $count } frames
}
cull-astro-suggest-none = Geen frame voorbij { $k }σ
cull-astro-suggest-few = Automatisch afwijzen heeft 3 gemeten frames nodig
cull-astro-suggested = voorgesteld om af te wijzen
cull-astro-k = k { $k }
cull-astro-reject = { $count } afwijzen
cull-astro-move = Afgewezen verplaatsen…
cull-astro-chart = { $metric } over { $count ->
        [one] { $count } frame
       *[other] { $count } frames
    }, mediaan { $median }.
cull-astro-sort-by = Sorteren op { $column }
cull-astro-ascending = oplopend
cull-astro-descending = aflopend
cull-astro-sorted = Gesorteerd op { $column }
cull-astro-reversed = Volgorde omgekeerd
cull-astro-metric = Grafiek: { $metric }
cull-astro-k-set = Automatisch afwijzen bij { $k }σ
cull-astro-none = Geen subframes hier
cull-astro-same-on = Dezelfde stretch voor elk frame
cull-astro-same-off = Elk frame apart gestretcht
cull-astro-line-hfd = HFD { $value } px
cull-astro-line-fwhm = FWHM { $value } px
cull-astro-line-fwhm-arcsec = FWHM { $value } px ({ $arcsec }″)
cull-astro-line-ecc = excentriciteit { $value }
cull-astro-line-stars = { $count ->
    [one] 1 ster
   *[other] { $count } sterren
}
cull-astro-measuring = sterren meten…
cull-astro-reject-confirm = { $count ->
    [one] Het voorgestelde frame afwijzen?
   *[other] De { $count } voorgestelde frames afwijzen?
}
cull-astro-reject-detail = { $rule }. Ongedaan maken zet de markeringen terug.
cull-astro-reject-ok = Afwijzen
cull-astro-nothing-to-reject = Geen frame om af te wijzen
cull-astro-rejected = { $count ->
    [one] 1 frame afgewezen
   *[other] { $count } frames afgewezen
}
cull-move-rejects-confirm = { $count ->
    [one] Het afgewezen frame naar de map ‘{ $folder }’ verplaatsen?
   *[other] De { $count } afgewezen frames naar de map ‘{ $folder }’ verplaatsen?
}
cull-move-rejects-detail = De bestanden worden verplaatst, niet verwijderd. Ongedaan maken zet ze terug.
cull-move-rejects-ok = Verplaatsen
cull-move-rejects-none = Geen afgewezen frames om te verplaatsen
cull-moved-rejects = { $count ->
    [one] 1 frame verplaatst naar ‘{ $folder }’
   *[other] { $count } frames verplaatst naar ‘{ $folder }’
}
