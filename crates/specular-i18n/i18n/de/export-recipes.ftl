# Specular — Export recipes: saved ways to export, several run at once. machine-drafted, needs native review
# Every output is a new file (or replaces one only after asking); originals are never changed.

palette-export-recipes = Mit Rezepten exportieren…
palette-export-recipes-help = Die Auswahl mit einem oder mehreren gespeicherten Rezepten exportieren.
palette-develop-export-recipes = Mit Rezepten exportieren…
palette-develop-export-recipes-help = Das bearbeitete Foto, oder mehrere, mit gespeicherten Rezepten exportieren.

export-recipes-title = Mit Rezepten exportieren
export-recipes-new = Neu
export-recipes-edit = Bearbeiten…
export-recipes-duplicate = Duplizieren
export-recipes-delete = Löschen…
export-recipes-start = Exportieren
export-recipes-save = Speichern
export-recipes-open-folder = Ordner öffnen
export-recipes-choose-image = Bild wählen…
export-recipes-no-images = Keine Bilder zum Exportieren.
export-recipes-none-checked = Mindestens ein Rezept ankreuzen.
export-recipes-checked = { $name }: an
export-recipes-unchecked = { $name }: aus

## The starters.
export-recipes-starter-web = Web, 2048 px
export-recipes-starter-print = Druck, volle Größe
export-recipes-starter-email = E-Mail, klein
export-recipes-unnamed = Unbenanntes Rezept
export-recipes-new-name = Neues Rezept
# { $name } is the recipe copied.
export-recipes-copy-name = { $name } (Kopie)

## The list's one-line summary.
export-recipes-summary-mark = Wasserzeichen
export-recipes-size-full = Volle Größe
export-recipes-size-long = { $px } px lange Seite
export-recipes-size-short = { $px } px kurze Seite
export-recipes-size-mp = { $mp } MP
export-recipes-size-exact = { $width } × { $height } px
export-recipes-size-percent = { PERCENT($percent) }
export-recipes-folder-same = Neben dem Original
export-recipes-folder-sub = Ordner „{ $name }“
export-recipes-folder-ask = Jedes Mal fragen

## The editor.
export-recipes-new-title = Neues Rezept
export-recipes-edit-title = Rezept bearbeiten
export-recipes-name = Name des Rezepts
export-recipes-text = Eigener Text
export-recipes-pattern = Dateiname
export-recipes-mark-text-field = Wasserzeichentext
export-recipes-subfolder = Ordnername
export-recipes-format = Dateiformat
export-recipes-quality = Qualität
export-recipes-lossless = Verlustfrei
export-recipes-bits = Bittiefe
export-recipes-bits-8 = 8 Bit
export-recipes-bits-16 = 16 Bit
export-recipes-space = Farbraum
export-recipes-space-srgb = sRGB
export-recipes-space-p3 = Display P3
export-recipes-space-adobe = Adobe RGB (1998)
export-recipes-space-prophoto = ProPhoto RGB
export-recipes-profile = Profil
export-recipes-profile-embed = Einbetten
export-recipes-profile-none = Weglassen
export-recipes-folder = Ordner
export-recipes-folder-subfolder = Unterordner
export-recipes-collision = Name vergeben
export-recipes-collision-unique = Eine Nummer anhängen
export-recipes-collision-overwrite = Ersetzen (fragt)
export-recipes-collision-skip = Überspringen
export-recipes-after = Danach
export-recipes-after-nothing = Nichts
export-recipes-after-open = Den Ordner öffnen
export-recipes-resize = Größe
export-recipes-resize-long = Lange Seite
export-recipes-resize-short = Kurze Seite
export-recipes-resize-mp = Megapixel
export-recipes-resize-exact = Genaue Größe
export-recipes-resize-percent = Prozent
export-recipes-edge = Seite
export-recipes-width = Breite
export-recipes-height = Höhe
export-recipes-megapixels = Megapixel
export-recipes-percent = Prozent
export-recipes-px = { $px } px
export-recipes-mp = { $mp } MP
export-recipes-pct = { PERCENT($percent) }
export-recipes-enlarge = Vergrößern
export-recipes-sharpen = Schärfen für
export-recipes-sharpen-off = Aus
export-recipes-sharpen-screen = Bildschirm
export-recipes-sharpen-matte = Mattes Papier
export-recipes-sharpen-glossy = Glänzendes Papier
export-recipes-amount = Stärke
export-recipes-amount-low = Gering
export-recipes-amount-standard = Normal
export-recipes-amount-high = Hoch
export-recipes-metadata = Metadaten
export-recipes-meta-all = Alle
export-recipes-meta-copyright = Nur Urheberrecht
export-recipes-meta-contact = Urheberrecht und Kontakt
export-recipes-meta-none = Keine
export-recipes-location = Ort
export-recipes-people = Abgebildete Personen
export-recipes-keep = Behalten
export-recipes-remove = Entfernen
export-recipes-mark = Wasserzeichen
export-recipes-mark-none = Keins
export-recipes-mark-text = Text
export-recipes-mark-image = Bild (eines wählen)
export-recipes-position = Position
export-recipes-pos-top-left = Oben links
export-recipes-pos-top = Oben
export-recipes-pos-top-right = Oben rechts
export-recipes-pos-left = Links
export-recipes-pos-centre = Mitte
export-recipes-pos-right = Rechts
export-recipes-pos-bottom-left = Unten links
export-recipes-pos-bottom = Unten
export-recipes-pos-bottom-right = Unten rechts
export-recipes-opacity = Deckkraft
export-recipes-scale = Breite
export-recipes-yes = Ja
export-recipes-no = Nein
export-recipes-error-name = Dem Rezept einen Namen geben.
export-recipes-error-mark-text = Den Wasserzeichentext eingeben.
export-recipes-error-mark-image = Das Wasserzeichenbild wählen.
export-recipes-error-subfolder = Den Ordnernamen eingeben.
export-recipes-saved = Rezept „{ $name }“ gespeichert.

## Delete.
export-recipes-delete-title = Das Rezept „{ $name }“ löschen?
export-recipes-delete-detail = Damit exportierte Dateien bleiben auf der Festplatte.
export-recipes-delete-confirm = Löschen
export-recipes-delete-keep = Behalten
export-recipes-deleted = Rezept „{ $name }“ gelöscht.

## The run.
# { $name } is the recipe whose folder is asked for.
export-recipes-ask-title = Ordner für „{ $name }“
export-recipes-asking = Den Ordner für jedes Rezept wählen, das danach fragt.
export-recipes-no-folder = Kein Ordner gewählt. Es wurde nichts exportiert.
export-recipes-planning = Dateien werden benannt…
export-recipes-plan-pattern = Rezept „{ $name }“: { $reason }
export-recipes-replace-title = { $count ->
    [one] 1 Datei ersetzen?
   *[other] { $count } Dateien ersetzen?
}
export-recipes-replace-detail = Originale werden nie ersetzt.
export-recipes-replace-confirm = Ersetzen
export-recipes-replace-kept = Es wurde nichts exportiert.
export-recipes-running = Wird exportiert…
export-recipes-progress = { $done } von { $total } Dateien
export-recipes-done = { $count ->
    [one] 1 Datei exportiert.
   *[other] { $count } Dateien exportiert.
}
export-recipes-stopped = { $count ->
    [one] Angehalten. 1 Datei exportiert.
   *[other] Angehalten. { $count } Dateien exportiert.
}
export-recipes-done-skipped = { $count ->
    [one] 1 übersprungen: der Name war vergeben.
   *[other] { $count } übersprungen: die Namen waren vergeben.
}
export-recipes-done-failed = { $count ->
    [one] 1 fehlgeschlagen:
   *[other] { $count } fehlgeschlagen:
}
# One failure: the source file, the recipe and why.
export-recipes-failure = { $file } ({ $recipe }): { $reason }
