# Specular — Klantgalerijen: een statische HTML-map met foto's, en de keuze van de klant teruglezen.

# machine-drafted, needs native review

palette-client-gallery = Klantgalerij…
palette-client-gallery-help = De foto's als webpaginamap voor een klant opslaan, met een keuze die de klant kan terugsturen.
palette-import-client-picks = Keuze van klant importeren…
palette-import-client-picks-help = De foto's markeren die een klant in de galerij koos.

webgallery-heading = Klantgalerij
webgallery-subtitle = { $count ->
    [one] Eén foto
   *[other] { $count } foto's
}
webgallery-title-label = Titel
webgallery-title-default = Proefafdrukken
webgallery-watermark-label = Watermerktekst (leeg: geen)
webgallery-source = Foto's
webgallery-source-current = Deze foto
webgallery-source-selection = Selectie
webgallery-source-album = Album ‘{ $name }’
webgallery-source-search = Zoekresultaten
webgallery-source-folder = Deze map
webgallery-source-picks = Gekozen
webgallery-source-value = { $what } ({ $count })
webgallery-theme = Thema
webgallery-theme-light = Licht
webgallery-theme-dark = Donker
webgallery-theme-minimal = Sober
webgallery-size = Lange zijde
webgallery-size-value = { $px } px
webgallery-format = Formaat
webgallery-captions = Onder elke foto
webgallery-captions-off = Niets
webgallery-captions-name = Bestandsnaam
webgallery-captions-title = Titel
webgallery-captions-all = Bestandsnaam, titel en bijschrift
webgallery-exif = Camera en belichting tonen
webgallery-location = Locatie (gps) in de foto's behouden
webgallery-note-private = Een statische galerij kan niet met een wachtwoord worden beveiligd.
webgallery-note-share = Iedereen met de map of het adres ervan ziet elke foto. Deel de link alleen met uw klant, of gebruik de toegangscontrole van uw webhost.
webgallery-note-picks = Klanten kiezen met een hartje. De keuze blijft in hun browser tot ze u de lijst sturen.
webgallery-write = Galerij maken…
webgallery-busy = { $done } van { $total } schrijven…
webgallery-stop = Stoppen
webgallery-done = { $count ->
    [one] Galerij gemaakt: één foto in { $folder }
   *[other] Galerij gemaakt: { $count } foto's in { $folder }
}
webgallery-done-failed = { $failed ->
    [one] Eén foto kon niet worden geschreven.
   *[other] { $failed } foto's konden niet worden geschreven.
}
webgallery-stopped = Galerij gestopt. Er is niets bewaard.
webgallery-failed = Galerij niet gemaakt: { $reason }
webgallery-nothing = Er kon geen enkele foto worden geschreven.

webgallery-page-count = { $count ->
    [one] Eén foto
   *[other] { $count } foto's
}
webgallery-page-hint = Tik op het hartje bij de foto's die u wilt. Uw keuze blijft in deze browser tot u hem verstuurt.
webgallery-page-picked = Gekozen: {"{"}n{"}"}
webgallery-page-pick = Kiezen
webgallery-page-copy = Mijn keuze kopiëren
webgallery-page-copied = Gekopieerd. Plak de lijst in een bericht.
webgallery-page-copy-by-hand = Kopieer de geselecteerde lijst en plak hem in een bericht.
webgallery-page-download-txt = picks.txt downloaden
webgallery-page-download-json = picks.json downloaden
webgallery-page-clear = Keuze wissen
webgallery-page-clear-ask = Uw hele keuze in deze browser wissen?
webgallery-page-no-storage = Deze browser bewaart uw keuze niet. Verstuur hem voordat u de pagina sluit.
webgallery-page-noscript = Kiezen vraagt JavaScript. De foto's openen wel.
webgallery-page-previous = Vorige
webgallery-page-next = Volgende
webgallery-page-close = Sluiten
webgallery-page-position = {"{"}i{"}"} van {"{"}n{"}"}
webgallery-page-viewer = Fotoviewer
webgallery-page-picks = Uw keuze

clientpicks-heading = Keuze van klant importeren
clientpicks-subtitle = { $count ->
    [one] Vergeleken met één getoonde foto
   *[other] Vergeleken met de { $count } getoonde foto's
}
clientpicks-paste-label = Namen (plak de lijst van de klant)
clientpicks-choose = Keuzebestand kiezen…
clientpicks-from-file = Uit { $name }
clientpicks-apply-as = Markeren
clientpicks-as-pick = Als gekozen
clientpicks-as-stars = { $stars ->
    [one] Eén ster
   *[other] { $stars } sterren
}
clientpicks-as-label = Kleurlabel { $label }
clientpicks-empty = Plak de lijst die de klant stuurde, of kies diens picks.txt of picks.json.
clientpicks-matched = { $matched } van { $total } namen passen bij foto's hier.
clientpicks-missing = Niet gevonden: { $names }
clientpicks-ambiguous = In meer dan één map, ongemoeid gelaten: { $names }
clientpicks-more = { $names } en nog { $more }
clientpicks-apply = Toepassen
clientpicks-applied = { $count ->
    [one] Eén foto gemarkeerd.
   *[other] { $count } foto's gemarkeerd.
}
clientpicks-unchanged = Die foto's waren al gemarkeerd.
clientpicks-read-failed = Kan { $name } niet lezen: { $reason }
