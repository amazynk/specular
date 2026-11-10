# machine-drafted, needs native review
## Astronomy interop: DS9 regions, plate solving, SAMP, HiPS sky surveys.

palette-science-load-regions = DS9-regio’s over het beeld leggen…
palette-science-load-regions-help = De vormen uit een DS9-regiobestand (.reg) op de afbeelding tekenen.
palette-science-export-regions = Regio’s opslaan als DS9…
palette-science-export-regions-help = De metingen en de overgelegde regio’s opslaan als DS9-regiobestand.
palette-science-edit-regions = Regio’s bewerken als metingen
palette-science-edit-regions-help = De overgelegde regio’s omzetten in metingen die je kunt verplaatsen en wijzigen.
palette-science-clear-regions = Overgelegde regio’s verwijderen
palette-science-clear-regions-help = De DS9-regio’s van de afbeelding verwijderen.
palette-science-plate-solve = Plate solving
palette-science-plate-solve-help = De hemelcoördinaten van het beeld bepalen met ASTAP of astrometry.net. Nogmaals uitvoeren om te stoppen.
palette-science-save-plate-solution = Plate-solve-oplossing opslaan
palette-science-save-plate-solution-help = De gevonden hemelcoördinaten naast de afbeelding opslaan, als .wcs-bestand.
palette-science-sky-survey = Hemelsurvey als achtergrond
palette-science-sky-survey-help = De volgende hemelsurvey rond het beeld tonen, of geen.
palette-science-samp-send-image = Afbeelding naar SAMP-toepassingen sturen
palette-science-samp-send-image-help = Deze afbeelding openen in DS9, Aladin en de andere toepassingen op de SAMP-hub.
palette-science-samp-send-position = Positie naar SAMP-toepassingen sturen
palette-science-samp-send-position-help = De andere toepassingen richten op de hemelpositie onder de aanwijzer.

disabled-samp-off = Maak eerst verbinding met een SAMP-hub (Instellingen › Integratie).

settings-integration-samp = Verbinden met een SAMP-hub
settings-integration-samp-help = Afbeeldingen, tabellen en hemelposities uitwisselen met DS9, TOPCAT, Aladin en andere astronomietoepassingen. Alleen een hub op deze computer wordt gebruikt.
settings-integration-astap-path = ASTAP-programma
settings-integration-astap-path-help = De plate solver ASTAP, als die niet vanzelf wordt gevonden.
settings-integration-solve-field-path = astrometry.net-programma
settings-integration-solve-field-path-help = Het programma solve-field van astrometry.net, als dat niet vanzelf wordt gevonden.

astro-regions-filter = DS9-regio’s
astro-regions-loaded = { $name }: { $count ->
        [one] { $count } regio.
       *[other] { $count } regio’s.
    }
astro-regions-refused = { $count ->
        [one] { $count } regel niet gelezen.
       *[other] { $count } regels niet gelezen.
    } Eerste: regel { $line }, { $reason }.
astro-regions-approximate = { $count ->
        [one] { $count } panda getekend als binnen- en buitenrand.
       *[other] { $count } panda’s getekend als hun binnen- en buitenrand.
    }
astro-regions-none-here = { $name }: geen enkele regio valt op deze afbeelding.
astro-regions-failed = { $name } kon niet worden gelezen: { $reason }
astro-regions-cleared = Regio’s verwijderd.
astro-regions-nothing = Geen regio’s of metingen om op te slaan.
astro-regions-saved = { $name } opgeslagen.
astro-regions-skipped = { $count ->
        [one] { $count } vorm buiten de hemelcoördinaten is weggelaten.
       *[other] { $count } vormen buiten de hemelcoördinaten zijn weggelaten.
    }
astro-regions-save-failed = De regio’s konden niet worden opgeslagen: { $reason }
astro-regions-edited = { $count ->
        [one] { $count } regio is nu een meting.
       *[other] { $count } regio’s zijn nu metingen.
    }
astro-regions-no-wcs = Hemelregio’s hebben een beeld met hemelcoördinaten nodig.

astro-solve-no-solver = Geen plate solver gevonden. Installeer ASTAP of astrometry.net, of kies er een in Instellingen › Integratie.
astro-solve-running = Oplossen met { $solver }… { $seconds } s
astro-solve-stop = Stoppen
astro-solve-stopped = Plate solving gestopt.
astro-solve-done = Opgelost: { $ra }, { $dec }, { $scale } per pixel.
astro-solve-failed = Plate solving mislukt: { $reason }
astro-solve-not-local = Alleen afbeeldingen in een map op deze computer kunnen worden opgelost.
astro-solve-none = Nog geen plate-solve-oplossing voor deze afbeelding.
astro-solve-saved = { $name } opgeslagen. De afbeelding zelf is niet gewijzigd.
astro-solve-sidecar = Hemelcoördinaten uit { $name }.

astro-samp-connected = Verbonden met de SAMP-hub.
astro-samp-no-hub = SAMP: { $reason }. Nieuwe poging zodra er een hub start.
astro-samp-lost = De SAMP-hub is gestopt. Nieuwe poging zodra er een start.
astro-samp-sent = { $count ->
        [one] Verstuurd naar { $count } toepassing.
       *[other] Verstuurd naar { $count } toepassingen.
    }
astro-samp-nobody = Geen toepassing op de hub luistert hiernaar.
astro-samp-failed = SAMP: { $reason }
astro-samp-refused = SAMP: { $url } staat niet op deze computer en wordt dus niet geopend.
astro-samp-pointed = Gericht op { $ra }, { $dec }.
astro-samp-off-image = { $ra }, { $dec } ligt niet op deze afbeelding.
astro-samp-no-table = SAMP: die tabel is hier niet overgelegd.
astro-samp-rows = { $count ->
        [one] { $count } catalogusbron gemarkeerd.
       *[other] { $count } catalogusbronnen gemarkeerd.
    }

astro-survey-off = Hemelsurvey uit.
astro-survey-on = Hemelsurvey: { $name }. { $credit }
astro-survey-failed = { $name } kan niet worden getoond: { $reason }
astro-survey-credit = { $name }: { $credit }

net-what-sky-survey = Hemelsurveytegels van { $host }
