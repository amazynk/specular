# Specular — Export recipes: saved ways to export, several run at once. machine-drafted, needs native review
# Every output is a new file (or replaces one only after asking); originals are never changed.

palette-export-recipes = Exporteren met recepten…
palette-export-recipes-help = De selectie exporteren met een of meer opgeslagen recepten.
palette-develop-export-recipes = Exporteren met recepten…
palette-develop-export-recipes-help = De bewerkte foto, of meerdere, exporteren met opgeslagen recepten.

export-recipes-title = Exporteren met recepten
export-recipes-new = Nieuw
export-recipes-edit = Bewerken…
export-recipes-duplicate = Dupliceren
export-recipes-delete = Verwijderen…
export-recipes-start = Exporteren
export-recipes-save = Opslaan
export-recipes-open-folder = Map openen
export-recipes-choose-image = Afbeelding kiezen…
export-recipes-no-images = Geen afbeeldingen om te exporteren.
export-recipes-none-checked = Vink minstens één recept aan.
export-recipes-checked = { $name }: aan
export-recipes-unchecked = { $name }: uit

## The starters.
export-recipes-starter-web = Web, 2048 px
export-recipes-starter-print = Afdrukken, volledige grootte
export-recipes-starter-email = E-mail, klein
export-recipes-unnamed = Naamloos recept
export-recipes-new-name = Nieuw recept
# { $name } is the recipe copied.
export-recipes-copy-name = { $name } (kopie)

## The list's one-line summary.
export-recipes-summary-mark = watermerk
export-recipes-size-full = Volledige grootte
export-recipes-size-long = { $px } px lange zijde
export-recipes-size-short = { $px } px korte zijde
export-recipes-size-mp = { $mp } MP
export-recipes-size-exact = { $width } × { $height } px
export-recipes-size-percent = { PERCENT($percent) }
export-recipes-folder-same = Naast het origineel
export-recipes-folder-sub = Map ‘{ $name }’
export-recipes-folder-ask = Elke keer vragen

## The editor.
export-recipes-new-title = Nieuw recept
export-recipes-edit-title = Recept bewerken
export-recipes-name = Naam van recept
export-recipes-text = Eigen tekst
export-recipes-pattern = Bestandsnaam
export-recipes-mark-text-field = Watermerktekst
export-recipes-subfolder = Mapnaam
export-recipes-format = Formaat
export-recipes-quality = Kwaliteit
export-recipes-lossless = Verliesvrij
export-recipes-bits = Bitdiepte
export-recipes-bits-8 = 8 bits
export-recipes-bits-16 = 16 bits
export-recipes-space = Kleurruimte
export-recipes-space-srgb = sRGB
export-recipes-space-p3 = Display P3
export-recipes-space-adobe = Adobe RGB (1998)
export-recipes-space-prophoto = ProPhoto RGB
export-recipes-profile = Profiel
export-recipes-profile-embed = Insluiten
export-recipes-profile-none = Weglaten
export-recipes-folder = Map
export-recipes-folder-subfolder = Submap
export-recipes-collision = Naam bezet
export-recipes-collision-unique = Een nummer toevoegen
export-recipes-collision-overwrite = Vervangen (vraagt)
export-recipes-collision-skip = Overslaan
export-recipes-after = Na afloop
export-recipes-after-nothing = Niets
export-recipes-after-open = De map openen
export-recipes-resize = Grootte
export-recipes-resize-long = Lange zijde
export-recipes-resize-short = Korte zijde
export-recipes-resize-mp = Megapixels
export-recipes-resize-exact = Exacte grootte
export-recipes-resize-percent = Percentage
export-recipes-edge = Zijde
export-recipes-width = Breedte
export-recipes-height = Hoogte
export-recipes-megapixels = Megapixels
export-recipes-percent = Percentage
export-recipes-px = { $px } px
export-recipes-mp = { $mp } MP
export-recipes-pct = { PERCENT($percent) }
export-recipes-enlarge = Vergroten
export-recipes-sharpen = Verscherpen voor
export-recipes-sharpen-off = Uit
export-recipes-sharpen-screen = Scherm
export-recipes-sharpen-matte = Mat papier
export-recipes-sharpen-glossy = Glanzend papier
export-recipes-amount = Sterkte
export-recipes-amount-low = Laag
export-recipes-amount-standard = Normaal
export-recipes-amount-high = Hoog
export-recipes-metadata = Metagegevens
export-recipes-meta-all = Alle
export-recipes-meta-copyright = Alleen copyright
export-recipes-meta-contact = Copyright en contact
export-recipes-meta-none = Geen
export-recipes-location = Locatie
export-recipes-people = Afgebeelde personen
export-recipes-keep = Behouden
export-recipes-remove = Verwijderen
export-recipes-mark = Watermerk
export-recipes-mark-none = Geen
export-recipes-mark-text = Tekst
export-recipes-mark-image = Afbeelding (kies er een)
export-recipes-position = Plaats
export-recipes-pos-top-left = Linksboven
export-recipes-pos-top = Boven
export-recipes-pos-top-right = Rechtsboven
export-recipes-pos-left = Links
export-recipes-pos-centre = Midden
export-recipes-pos-right = Rechts
export-recipes-pos-bottom-left = Linksonder
export-recipes-pos-bottom = Onder
export-recipes-pos-bottom-right = Rechtsonder
export-recipes-opacity = Dekking
export-recipes-scale = Breedte
export-recipes-yes = Ja
export-recipes-no = Nee
export-recipes-error-name = Geef het recept een naam.
export-recipes-error-mark-text = Typ de watermerktekst.
export-recipes-error-mark-image = Kies de watermerkafbeelding.
export-recipes-error-subfolder = Typ de mapnaam.
export-recipes-saved = Recept ‘{ $name }’ opgeslagen.

## Delete.
export-recipes-delete-title = Het recept ‘{ $name }’ verwijderen?
export-recipes-delete-detail = Bestanden die het exporteerde blijven op de schijf.
export-recipes-delete-confirm = Verwijderen
export-recipes-delete-keep = Behouden
export-recipes-deleted = Recept ‘{ $name }’ verwijderd.

## The run.
# { $name } is the recipe whose folder is asked for.
export-recipes-ask-title = Map voor ‘{ $name }’
export-recipes-asking = Kies de map voor elk recept dat erom vraagt.
export-recipes-no-folder = Geen map gekozen. Er is niets geëxporteerd.
export-recipes-planning = Bestanden een naam geven…
export-recipes-plan-pattern = Recept ‘{ $name }’: { $reason }
export-recipes-replace-title = { $count ->
    [one] 1 bestand vervangen?
   *[other] { $count } bestanden vervangen?
}
export-recipes-replace-detail = Originelen worden nooit vervangen.
export-recipes-replace-confirm = Vervangen
export-recipes-replace-kept = Er is niets geëxporteerd.
export-recipes-running = Bezig met exporteren…
export-recipes-progress = { $done } van { $total } bestanden
export-recipes-done = { $count ->
    [one] 1 bestand geëxporteerd.
   *[other] { $count } bestanden geëxporteerd.
}
export-recipes-stopped = { $count ->
    [one] Gestopt. 1 bestand geëxporteerd.
   *[other] Gestopt. { $count } bestanden geëxporteerd.
}
export-recipes-done-skipped = { $count ->
    [one] 1 overgeslagen: de naam was bezet.
   *[other] { $count } overgeslagen: de namen waren bezet.
}
export-recipes-done-failed = { $count ->
    [one] 1 mislukt:
   *[other] { $count } mislukt:
}
# One failure: the source file, the recipe and why.
export-recipes-failure = { $file } ({ $recipe }): { $reason }
