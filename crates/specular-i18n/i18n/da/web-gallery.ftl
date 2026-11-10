# Specular — Kundegallerier: en statisk HTML-mappe med fotos, og at læse kundens valg tilbage.

# machine-drafted, needs native review

palette-client-gallery = Kundegalleri…
palette-client-gallery-help = Gem fotoene som en websidemappe til en kunde, med valg som kunden kan sende tilbage.
palette-import-client-picks = Importér kundens valg…
palette-import-client-picks-help = Markér de fotos, en kunde valgte i sit galleri.

webgallery-heading = Kundegalleri
webgallery-subtitle = { $count ->
    [one] Ét foto
   *[other] { $count } fotos
}
webgallery-title-label = Titel
webgallery-title-default = Prøvebilleder
webgallery-watermark-label = Vandmærketekst (tom: intet)
webgallery-source = Fotos
webgallery-source-current = Dette foto
webgallery-source-selection = Markering
webgallery-source-album = Albummet “{ $name }”
webgallery-source-search = Søgeresultater
webgallery-source-folder = Denne mappe
webgallery-source-picks = Valgte
webgallery-source-value = { $what } ({ $count })
webgallery-theme = Tema
webgallery-theme-light = Lyst
webgallery-theme-dark = Mørkt
webgallery-theme-minimal = Enkelt
webgallery-size = Lange side
webgallery-size-value = { $px } px
webgallery-format = Format
webgallery-captions = Under hvert foto
webgallery-captions-off = Intet
webgallery-captions-name = Filnavn
webgallery-captions-title = Titel
webgallery-captions-all = Filnavn, titel og billedtekst
webgallery-exif = Vis kamera og eksponering
webgallery-location = Behold placeringen (GPS) i fotoene
webgallery-note-private = Et statisk galleri kan ikke beskyttes med en adgangskode.
webgallery-note-share = Alle med mappen eller dens adresse kan se hvert foto. Del kun linket med din kunde, eller brug din webhosts adgangskontrol.
webgallery-note-picks = Kunder vælger med et hjerte. Valgene bliver i deres browser, til de sender dig listen.
webgallery-write = Opret galleri…
webgallery-busy = Skriver { $done } af { $total }…
webgallery-stop = Stop
webgallery-done = { $count ->
    [one] Galleri oprettet: ét foto i { $folder }
   *[other] Galleri oprettet: { $count } fotos i { $folder }
}
webgallery-done-failed = { $failed ->
    [one] Ét foto kunne ikke skrives.
   *[other] { $failed } fotos kunne ikke skrives.
}
webgallery-stopped = Galleriet blev stoppet. Intet blev gemt.
webgallery-failed = Galleriet blev ikke oprettet: { $reason }
webgallery-nothing = Intet foto kunne skrives.

webgallery-page-count = { $count ->
    [one] Ét foto
   *[other] { $count } fotos
}
webgallery-page-hint = Tryk på hjertet ved de fotos, du vil have. Dine valg bliver i denne browser, til du sender dem.
webgallery-page-picked = Valgt: {"{"}n{"}"}
webgallery-page-pick = Vælg
webgallery-page-copy = Kopiér mine valg
webgallery-page-copied = Kopieret. Indsæt listen i en besked.
webgallery-page-copy-by-hand = Kopiér den markerede liste, og indsæt den i en besked.
webgallery-page-download-txt = Hent picks.txt
webgallery-page-download-json = Hent picks.json
webgallery-page-clear = Ryd valg
webgallery-page-clear-ask = Rydde alle dine valg i denne browser?
webgallery-page-no-storage = Denne browser gemmer ikke dine valg. Send dem, før du lukker siden.
webgallery-page-noscript = Valg kræver JavaScript. Fotoene åbner stadig.
webgallery-page-previous = Forrige
webgallery-page-next = Næste
webgallery-page-close = Luk
webgallery-page-position = {"{"}i{"}"} af {"{"}n{"}"}
webgallery-page-viewer = Fotoviser
webgallery-page-picks = Dine valg

clientpicks-heading = Importér kundens valg
clientpicks-subtitle = { $count ->
    [one] Sammenlignet med ét vist foto
   *[other] Sammenlignet med de { $count } viste fotos
}
clientpicks-paste-label = Navne (indsæt kundens liste)
clientpicks-choose = Vælg fil med valg…
clientpicks-from-file = Fra { $name }
clientpicks-apply-as = Markér dem
clientpicks-as-pick = Som valgte
clientpicks-as-stars = { $stars ->
    [one] Én stjerne
   *[other] { $stars } stjerner
}
clientpicks-as-label = Etiket { $label }
clientpicks-empty = Indsæt listen, kunden sendte, eller vælg kundens picks.txt eller picks.json.
clientpicks-matched = { $matched } af { $total } navne passer til fotos her.
clientpicks-missing = Ikke fundet: { $names }
clientpicks-ambiguous = I mere end én mappe, ikke ændret: { $names }
clientpicks-more = { $names } og { $more } mere
clientpicks-apply = Anvend
clientpicks-applied = { $count ->
    [one] Ét foto markeret.
   *[other] { $count } fotos markeret.
}
clientpicks-unchanged = De fotos var allerede markeret.
clientpicks-read-failed = Kan ikke læse { $name }: { $reason }
