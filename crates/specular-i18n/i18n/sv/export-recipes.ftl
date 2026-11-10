# Specular — Export recipes: saved ways to export, several run at once. machine-drafted, needs native review
# Every output is a new file (or replaces one only after asking); originals are never changed.

palette-export-recipes = Exportera med recept…
palette-export-recipes-help = Exportera markeringen med ett eller flera sparade recept.
palette-develop-export-recipes = Exportera med recept…
palette-develop-export-recipes-help = Exportera det redigerade fotot, eller flera, med sparade recept.

export-recipes-title = Exportera med recept
export-recipes-new = Nytt
export-recipes-edit = Redigera…
export-recipes-duplicate = Duplicera
export-recipes-delete = Ta bort…
export-recipes-start = Exportera
export-recipes-save = Spara
export-recipes-open-folder = Öppna mapp
export-recipes-choose-image = Välj bild…
export-recipes-no-images = Inga bilder att exportera.
export-recipes-none-checked = Bocka för minst ett recept.
export-recipes-checked = { $name }: på
export-recipes-unchecked = { $name }: av

## The starters.
export-recipes-starter-web = Webb, 2048 px
export-recipes-starter-print = Utskrift, full storlek
export-recipes-starter-email = E-post, liten
export-recipes-unnamed = Namnlöst recept
export-recipes-new-name = Nytt recept
# { $name } is the recipe copied.
export-recipes-copy-name = { $name } (kopia)

## The list's one-line summary.
export-recipes-summary-mark = vattenstämpel
export-recipes-size-full = Full storlek
export-recipes-size-long = { $px } px långsida
export-recipes-size-short = { $px } px kortsida
export-recipes-size-mp = { $mp } MP
export-recipes-size-exact = { $width } × { $height } px
export-recipes-size-percent = { PERCENT($percent) }
export-recipes-folder-same = Bredvid originalet
export-recipes-folder-sub = Mappen ”{ $name }”
export-recipes-folder-ask = Fråga varje gång

## The editor.
export-recipes-new-title = Nytt recept
export-recipes-edit-title = Redigera recept
export-recipes-name = Receptets namn
export-recipes-text = Egen text
export-recipes-pattern = Filnamn
export-recipes-mark-text-field = Vattenstämpelns text
export-recipes-subfolder = Mappnamn
export-recipes-format = Filformat
export-recipes-quality = Kvalitet
export-recipes-lossless = Förlustfri
export-recipes-bits = Bitdjup
export-recipes-bits-8 = 8 bitar
export-recipes-bits-16 = 16 bitar
export-recipes-space = Färgrymd
export-recipes-space-srgb = sRGB
export-recipes-space-p3 = Display P3
export-recipes-space-adobe = Adobe RGB (1998)
export-recipes-space-prophoto = ProPhoto RGB
export-recipes-profile = Profil
export-recipes-profile-embed = Bädda in
export-recipes-profile-none = Utelämna
export-recipes-folder = Mapp
export-recipes-folder-subfolder = Undermapp
export-recipes-collision = Namnet upptaget
export-recipes-collision-unique = Lägg till ett nummer
export-recipes-collision-overwrite = Ersätt (frågar)
export-recipes-collision-skip = Hoppa över
export-recipes-after = När det är klart
export-recipes-after-nothing = Inget
export-recipes-after-open = Öppna mappen
export-recipes-resize = Storlek
export-recipes-resize-long = Långsida
export-recipes-resize-short = Kortsida
export-recipes-resize-mp = Megapixel
export-recipes-resize-exact = Exakt storlek
export-recipes-resize-percent = Procent
export-recipes-edge = Sida
export-recipes-width = Bredd
export-recipes-height = Höjd
export-recipes-megapixels = Megapixel
export-recipes-percent = Procent
export-recipes-px = { $px } px
export-recipes-mp = { $mp } MP
export-recipes-pct = { PERCENT($percent) }
export-recipes-enlarge = Förstora
export-recipes-sharpen = Skärpa för
export-recipes-sharpen-off = Av
export-recipes-sharpen-screen = Skärm
export-recipes-sharpen-matte = Matt papper
export-recipes-sharpen-glossy = Glättat papper
export-recipes-amount = Styrka
export-recipes-amount-low = Låg
export-recipes-amount-standard = Normal
export-recipes-amount-high = Hög
export-recipes-metadata = Metadata
export-recipes-meta-all = Alla
export-recipes-meta-copyright = Bara upphovsrätt
export-recipes-meta-contact = Upphovsrätt och kontakt
export-recipes-meta-none = Inga
export-recipes-location = Plats
export-recipes-people = Personer i bilden
export-recipes-keep = Behåll
export-recipes-remove = Ta bort
export-recipes-mark = Vattenstämpel
export-recipes-mark-none = Ingen
export-recipes-mark-text = Text
export-recipes-mark-image = Bild (välj en)
export-recipes-position = Plats
export-recipes-pos-top-left = Uppe till vänster
export-recipes-pos-top = Uppe
export-recipes-pos-top-right = Uppe till höger
export-recipes-pos-left = Vänster
export-recipes-pos-centre = Mitten
export-recipes-pos-right = Höger
export-recipes-pos-bottom-left = Nere till vänster
export-recipes-pos-bottom = Nere
export-recipes-pos-bottom-right = Nere till höger
export-recipes-opacity = Opacitet
export-recipes-scale = Bredd
export-recipes-yes = Ja
export-recipes-no = Nej
export-recipes-error-name = Ge receptet ett namn.
export-recipes-error-mark-text = Skriv vattenstämpelns text.
export-recipes-error-mark-image = Välj vattenstämpelns bild.
export-recipes-error-subfolder = Skriv mappnamnet.
export-recipes-saved = Receptet ”{ $name }” sparades.

## Delete.
export-recipes-delete-title = Ta bort receptet ”{ $name }”?
export-recipes-delete-detail = Filer som det exporterade ligger kvar på disken.
export-recipes-delete-confirm = Ta bort
export-recipes-delete-keep = Behåll
export-recipes-deleted = Receptet ”{ $name }” togs bort.

## The run.
# { $name } is the recipe whose folder is asked for.
export-recipes-ask-title = Mapp för ”{ $name }”
export-recipes-asking = Välj mappen för varje recept som frågar.
export-recipes-no-folder = Ingen mapp vald. Inget exporterades.
export-recipes-planning = Filerna namnges…
export-recipes-plan-pattern = Receptet ”{ $name }”: { $reason }
export-recipes-replace-title = { $count ->
    [one] Ersätta 1 fil?
   *[other] Ersätta { $count } filer?
}
export-recipes-replace-detail = Original ersätts aldrig.
export-recipes-replace-confirm = Ersätt
export-recipes-replace-kept = Inget exporterades.
export-recipes-running = Exporterar…
export-recipes-progress = { $done } av { $total } filer
export-recipes-done = { $count ->
    [one] 1 fil exporterades.
   *[other] { $count } filer exporterades.
}
export-recipes-stopped = { $count ->
    [one] Stoppades. 1 fil exporterades.
   *[other] Stoppades. { $count } filer exporterades.
}
export-recipes-done-skipped = { $count ->
    [one] 1 överhoppad: namnet var upptaget.
   *[other] { $count } överhoppade: namnen var upptagna.
}
export-recipes-done-failed = { $count ->
    [one] 1 misslyckades:
   *[other] { $count } misslyckades:
}
# One failure: the source file, the recipe and why.
export-recipes-failure = { $file } ({ $recipe }): { $reason }
