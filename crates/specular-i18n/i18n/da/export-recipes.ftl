# Specular — Export recipes: saved ways to export, several run at once. machine-drafted, needs native review
# Every output is a new file (or replaces one only after asking); originals are never changed.

palette-export-recipes = Eksportér med opskrifter…
palette-export-recipes-help = Eksportér markeringen med en eller flere gemte opskrifter.
palette-develop-export-recipes = Eksportér med opskrifter…
palette-develop-export-recipes-help = Eksportér det redigerede foto, eller flere, med gemte opskrifter.

export-recipes-title = Eksportér med opskrifter
export-recipes-new = Ny
export-recipes-edit = Redigér…
export-recipes-duplicate = Duplikér
export-recipes-delete = Slet…
export-recipes-start = Eksportér
export-recipes-save = Gem
export-recipes-open-folder = Åbn mappe
export-recipes-choose-image = Vælg billede…
export-recipes-no-images = Ingen billeder at eksportere.
export-recipes-none-checked = Markér mindst én opskrift.
export-recipes-checked = { $name }: til
export-recipes-unchecked = { $name }: fra

## The starters.
export-recipes-starter-web = Web, 2048 px
export-recipes-starter-print = Tryk, fuld størrelse
export-recipes-starter-email = E-mail, lille
export-recipes-unnamed = Unavngivet opskrift
export-recipes-new-name = Ny opskrift
# { $name } is the recipe copied.
export-recipes-copy-name = { $name } (kopi)

## The list's one-line summary.
export-recipes-summary-mark = vandmærke
export-recipes-size-full = Fuld størrelse
export-recipes-size-long = { $px } px lange side
export-recipes-size-short = { $px } px korte side
export-recipes-size-mp = { $mp } MP
export-recipes-size-exact = { $width } × { $height } px
export-recipes-size-percent = { PERCENT($percent) }
export-recipes-folder-same = Ved siden af originalen
export-recipes-folder-sub = Mappen ”{ $name }”
export-recipes-folder-ask = Spørg hver gang

## The editor.
export-recipes-new-title = Ny opskrift
export-recipes-edit-title = Redigér opskrift
export-recipes-name = Opskriftens navn
export-recipes-text = Egen tekst
export-recipes-pattern = Filnavn
export-recipes-mark-text-field = Vandmærketekst
export-recipes-subfolder = Mappenavn
export-recipes-format = Format
export-recipes-quality = Kvalitet
export-recipes-lossless = Tabsfri
export-recipes-bits = Bitdybde
export-recipes-bits-8 = 8 bit
export-recipes-bits-16 = 16 bit
export-recipes-space = Farverum
export-recipes-space-srgb = sRGB
export-recipes-space-p3 = Display P3
export-recipes-space-adobe = Adobe RGB (1998)
export-recipes-space-prophoto = ProPhoto RGB
export-recipes-profile = Profil
export-recipes-profile-embed = Indlejr
export-recipes-profile-none = Udelad
export-recipes-folder = Mappe
export-recipes-folder-subfolder = Undermappe
export-recipes-collision = Navnet er optaget
export-recipes-collision-unique = Tilføj et nummer
export-recipes-collision-overwrite = Erstat (spørger)
export-recipes-collision-skip = Spring over
export-recipes-after = Når færdig
export-recipes-after-nothing = Intet
export-recipes-after-open = Åbn mappen
export-recipes-resize = Størrelse
export-recipes-resize-long = Lange side
export-recipes-resize-short = Korte side
export-recipes-resize-mp = Megapixel
export-recipes-resize-exact = Præcis størrelse
export-recipes-resize-percent = Procent
export-recipes-edge = Side
export-recipes-width = Bredde
export-recipes-height = Højde
export-recipes-megapixels = Megapixel
export-recipes-percent = Procent
export-recipes-px = { $px } px
export-recipes-mp = { $mp } MP
export-recipes-pct = { PERCENT($percent) }
export-recipes-enlarge = Forstør
export-recipes-sharpen = Skærp til
export-recipes-sharpen-off = Fra
export-recipes-sharpen-screen = Skærm
export-recipes-sharpen-matte = Mat papir
export-recipes-sharpen-glossy = Blankt papir
export-recipes-amount = Styrke
export-recipes-amount-low = Lav
export-recipes-amount-standard = Standard
export-recipes-amount-high = Høj
export-recipes-metadata = Metadata
export-recipes-meta-all = Alle
export-recipes-meta-copyright = Kun ophavsret
export-recipes-meta-contact = Ophavsret og kontakt
export-recipes-meta-none = Ingen
export-recipes-location = Placering
export-recipes-people = Viste personer
export-recipes-keep = Behold
export-recipes-remove = Fjern
export-recipes-mark = Vandmærke
export-recipes-mark-none = Intet
export-recipes-mark-text = Tekst
export-recipes-mark-image = Billede (vælg et)
export-recipes-position = Position
export-recipes-pos-top-left = Øverst til venstre
export-recipes-pos-top = Øverst
export-recipes-pos-top-right = Øverst til højre
export-recipes-pos-left = Venstre
export-recipes-pos-centre = Midten
export-recipes-pos-right = Højre
export-recipes-pos-bottom-left = Nederst til venstre
export-recipes-pos-bottom = Nederst
export-recipes-pos-bottom-right = Nederst til højre
export-recipes-opacity = Uigennemsigtighed
export-recipes-scale = Bredde
export-recipes-yes = Ja
export-recipes-no = Nej
export-recipes-error-name = Giv opskriften et navn.
export-recipes-error-mark-text = Skriv vandmærketeksten.
export-recipes-error-mark-image = Vælg vandmærkebilledet.
export-recipes-error-subfolder = Skriv mappenavnet.
export-recipes-saved = Opskriften ”{ $name }” er gemt.

## Delete.
export-recipes-delete-title = Slet opskriften ”{ $name }”?
export-recipes-delete-detail = Filer, den har eksporteret, bliver på disken.
export-recipes-delete-confirm = Slet
export-recipes-delete-keep = Behold
export-recipes-deleted = Opskriften ”{ $name }” er slettet.

## The run.
# { $name } is the recipe whose folder is asked for.
export-recipes-ask-title = Mappe til ”{ $name }”
export-recipes-asking = Vælg mappen for hver opskrift, der spørger.
export-recipes-no-folder = Ingen mappe valgt. Intet blev eksporteret.
export-recipes-planning = Navngiver filerne…
export-recipes-plan-pattern = Opskriften ”{ $name }”: { $reason }
export-recipes-replace-title = { $count ->
    [one] Erstat 1 fil?
   *[other] Erstat { $count } filer?
}
export-recipes-replace-detail = Originaler erstattes aldrig.
export-recipes-replace-confirm = Erstat
export-recipes-replace-kept = Intet blev eksporteret.
export-recipes-running = Eksporterer…
export-recipes-progress = { $done } af { $total } filer
export-recipes-done = { $count ->
    [one] 1 fil eksporteret.
   *[other] { $count } filer eksporteret.
}
export-recipes-stopped = { $count ->
    [one] Stoppet. 1 fil eksporteret.
   *[other] Stoppet. { $count } filer eksporteret.
}
export-recipes-done-skipped = { $count ->
    [one] 1 sprunget over: navnet var optaget.
   *[other] { $count } sprunget over: navnene var optaget.
}
export-recipes-done-failed = { $count ->
    [one] 1 mislykkedes:
   *[other] { $count } mislykkedes:
}
# One failure: the source file, the recipe and why.
export-recipes-failure = { $file } ({ $recipe }): { $reason }
