# machine-drafted, needs native review
# Specular — přehrávání animací a režim čtení (archivy, komiksy). Čeština.

disabled-not-animated = Tento obrázek není animovaný
disabled-in-archive = Stránky uvnitř archivu nelze přesunout do koše

## Paleta příkazů a přehled zkratek

palette-play-pause = Přehrát nebo pozastavit animaci
palette-next-frame = Další snímek
palette-prev-frame = Předchozí snímek
palette-faster = Přehrávat rychleji
palette-slower = Přehrávat pomaleji
palette-normal-speed = Normální rychlost přehrávání
palette-loop-forever = Opakovat animaci donekonečna
palette-right-to-left = Pořadí stránek zprava doleva
# machine-drafted, needs native review
palette-contents = Obsah
palette-spreads = Dvojstrany

## Ovládání animace (lišta nad panelem nástrojů)

# Název skupiny ovládacích prvků přehrávání pro usnadnění přístupu.
transport-region = Ovládání animace
transport-play = Přehrát
transport-pause = Pozastavit
transport-prev-frame = Předchozí snímek
transport-next-frame = Další snímek
transport-slower = Pomaleji
transport-faster = Rychleji
# $index je zobrazený snímek, $count počet snímků.
transport-frame = Snímek { $index } / { $count }
# Počet snímků není znám, dokud se animace jednou nepřehraje.
transport-frame-unknown = Snímek { $index }
# $speed je naformátované číslo, např. 1,5.
transport-speed = { $speed }×
transport-speed-name = Rychlost přehrávání { $speed }×
# machine-drafted, needs native review
motion-badge = Pohyb
motion-badge-live = Live
motion-play = Přehrát pohyblivý klip
motion-play-live = Přehrát Live Photo
motion-stop = Zpět na statickou fotografii
toast-motion-failed = Pohyblivý klip nelze přehrát: { $reason }

## Oznámení

toast-speed = Rychlost přehrávání { $speed }×
toast-loop-forever-on = Opakování donekonečna
toast-loop-forever-off = Opakování podle souboru
toast-right-to-left-on = Stránky zprava doleva
toast-right-to-left-off = Stránky zleva doprava
toast-spreads-on = Dvojstrany
toast-spreads-off = Jedna stránka

## Archivy

# Název okna pro stránku archivu.
title-archive-page = { $page } — { $archive }
# Údaj o pozici při zobrazení dvou stránek.
readout-position-spread = { $first }–{ $last } / { $count }
# $format je RAR.
archive-needs-sandbox = Archivy { $format } umí otevřít jen izolovaný hostitel dekodérů, který toto sestavení zatím neobsahuje.
archive-empty = Tento archiv neobsahuje žádné obrázky.
archive-failed = Archiv nelze otevřít: { $error }
archive-encrypted = Tento archiv je zašifrovaný. Otevřete ho znovu a zadejte heslo.
archive-wrong-password = Heslo k tomuto archivu je nesprávné.
password-title = Je vyžadováno heslo
password-archive = Archiv „{ $archive }“ je zašifrovaný.
password-label = Heslo
password-show = Zobrazit heslo
password-wrong = Nesprávné heslo. Zkuste to znovu.
password-open = Otevřít
password-cancel = Zrušit

## Nastavení

settings-viewing-animation-autoplay = Automaticky přehrávat animace
settings-viewing-animation-autoplay-help = Spustit animované obrázky (GIF, APNG, WebP…) ihned po otevření.
settings-viewing-animation-loop = Opakování animace
settings-viewing-animation-loop-help = Kolikrát se animace opakují.
settings-viewing-animation-loop-file = Podle souboru
settings-viewing-animation-loop-forever = Donekonečna
settings-viewing-animation-loop-once = Přehrát jednou
settings-viewing-animation-short-delays = Velmi krátké prodlevy snímků
settings-viewing-animation-short-delays-help = Snímky, které mají trvat 10 ms nebo méně: zpomalit na 100 ms jako webové prohlížeče, nebo přehrát tak, jak jsou uloženy.
settings-viewing-animation-short-delays-browser = Jako webové prohlížeče (100 ms)
settings-viewing-animation-short-delays-as-stored = Jak jsou uloženy
settings-browsing-reading-order = Pořadí stránek v archivech
settings-browsing-reading-order-help = Komiksové archivy se čtou zprava doleva, pokud to uvádí jejich ComicInfo.xml (manga), není-li zde nastaveno jinak.
settings-browsing-reading-order-auto = Podle archivu
settings-browsing-reading-order-left-to-right = Zleva doprava
settings-browsing-reading-order-right-to-left = Zprava doleva
settings-browsing-page-layout = Stránky v archivech
settings-browsing-page-layout-help = Zobrazit jednu stránku nebo dvě vedle sebe; obálka se vždy zobrazuje samostatně.
settings-browsing-page-layout-auto = Podle archivu
settings-browsing-page-layout-single = Jedna stránka
settings-browsing-page-layout-spreads = Dvojstrany

## Pages of multi-image files (ICNS sizes, texture layers, faces and slices, TIFF pages) (machine-drafted, needs native review)
palette-next-page = Další strana
palette-prev-page = Předchozí strana
palette-first-page = První strana
palette-last-page = Poslední strana
disabled-one-page = Obrázek má jen jednu stranu
page-indicator = Strana { $index } z { $count }
page-indicator-labelled = { $page }, { $label }
page-label-icon = { $width }×{ $height }
page-label-icon-density = { $width }×{ $height } @{ $density }x
page-label-layer = Vrstva { $index }
page-label-face = Stěna { $face }
page-label-sphere = Sférická mapa
page-label-slice = Řez { $index }
page-label-join = { $a } · { $b }
page-label-slide = Preparát
page-label-slide-label = Štítek
page-label-slide-macro = Přehled
page-label-slide-thumbnail = Miniatura
# machine-drafted, needs native review
page-label-thermal = Termosnímek
page-label-visible = Viditelná fotografie
# machine-drafted, needs native review
page-label-left-view = Levý pohled
# machine-drafted, needs native review
page-label-right-view = Pravý pohled
# machine-drafted, needs native review
# An object of a CERN ROOT file in the page picker: its path in the file and its class, e.g.
# "run1/hits (TH2D)".
page-label-object = { $name } ({ $class })
# machine-drafted, needs native review
# A view of one step of a circuit board job ("panel · Top").
page-label-board-step = { $step } · { $view }
# machine-drafted, needs native review
# A focal plane and a time point of a remote microscopy image (OMERO), counted from 1.
page-label-z = Z { $index } z { $count }
page-label-t = T { $index } z { $count }
# machine-drafted, needs native review
# Views of a remote-sensing product (an ENVI cube, a Sentinel-2 product) in the page picker.
page-label-product-composite = Výchozí pásma
page-label-product-bands = Všechna pásma
page-label-product-true-colour = Skutečné barvy
page-label-product-false-colour = Nepravé barvy (blízké infračervené)
page-label-product-swir = Krátkovlnné infračervené
page-label-product-agriculture = Zemědělství
page-label-product-scene-class = Klasifikace scény
page-label-product-cloud = Pravděpodobnost oblačnosti
# A product view at a pixel size, e.g. "True colour, 10 m".
page-label-product-at = { $view }, { $metres } m
title-file-page = { $file } — { $page }
info-page-group = Strana
info-page-position = Pozice
info-page-contents = Obsah

# machine-drafted, needs native review
# OpenColorIO (Settings › Colour): why scene-linear images are shown without it.
ocio-failed = OCIO se nepoužívá: { $reason }
ocio-builtin-config = $OCIO odkazuje na vestavěnou konfiguraci (ocio://); uložte ji jako soubor a vyberte tento soubor.

## Page labels for icons and collections (machine-drafted, needs native review)
page-label-size = { $width }×{ $height }
page-label-size-bits = { $width }×{ $height } · { $bits } bitů

## Page labels for Photoshop layers (machine-drafted, needs native review)
page-label-merged = Sloučený obraz
page-label-from-layers = Sestaveno z vrstev
page-label-layer-hidden = { $name } (skrytá)
page-label-layer-unnamed = Nepojmenovaná vrstva { $index }
