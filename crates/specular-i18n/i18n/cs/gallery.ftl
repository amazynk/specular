# machine-drafted, needs native review
# Specular — galerie (režim miniatur, DESIGN §10) a Nastavení → Galerie. Čeština.

## Paleta a nabídky

palette-gallery = Galerie
gallery-palette-view = Zobrazení galerie…
gallery-palette-layout = Rozvržení
gallery-palette-sort = Řadit podle
gallery-palette-group = Seskupit podle
gallery-palette-filter = Filtrovat…
gallery-palette-quick-filters = Rychlé filtry
gallery-palette-details = Zobrazení podrobností
gallery-palette-info-overlay = Údaje na miniaturách
gallery-palette-stacks = Seskupit RAW+JPEG a série
gallery-palette-subfolders = Zahrnout podsložky
gallery-palette-compare = Porovnat vybrané
gallery-palette-rate = Hodnotit…
gallery-palette-settings = Nastavení galerie…
gallery-view-menu = Zobrazit
gallery-more-menu = Další
gallery-open = Otevřít
gallery-new-window = Otevřít v novém okně
gallery-reveal = Zobrazit ve složce
gallery-properties = Vlastnosti
gallery-copy-paths = Kopírovat cesty
gallery-select-all = Vybrat vše
gallery-select-none = Zrušit výběr
gallery-select-invert = Obrátit výběr
gallery-next-unrated = Další nehodnocený
gallery-stack-toggle = Rozbalit nebo sbalit skupinu
gallery-run-on = { $count ->
    [one] { $name } ({ $count } soubor)
    [few] { $name } ({ $count } soubory)
    [many] { $name } ({ $count } souboru)
   *[other] { $name } ({ $count } souborů)
}
gallery-depth-off = Jen tato složka
gallery-depth-all = Všechny podsložky
gallery-depth-levels = { $depth ->
    [one] { $depth } úroveň podsložek
    [few] { $depth } úrovně podsložek
    [many] { $depth } úrovně podsložek
   *[other] { $depth } úrovní podsložek
}

## Stav a prázdné stavy

gallery-items = { $count ->
    [one] { $count } položka
    [few] { $count } položky
    [many] { $count } položky
   *[other] { $count } položek
}
gallery-selected = Vybráno { $count } z { $total }
gallery-shown-of = Zobrazeno { $count } z { $total }
gallery-listing = Čtení složky… { $count }
gallery-empty = Tato složka neobsahuje žádné obrázky.
gallery-empty-filter = Filtru neodpovídají žádné obrázky.
gallery-list-error = Tuto složku nelze přečíst: { $reason }
gallery-no-folder = Není otevřena žádná složka
gallery-kind-folder = Složka
gallery-drag-count = { $count ->
    [one] { $count } soubor
    [few] { $count } soubory
    [many] { $count } souboru
   *[other] { $count } souborů
}
gallery-badge-pages = { $count ->
    [one] { $count } stránka
    [few] { $count } stránky
    [many] { $count } stránky
   *[other] { $count } stránek
}

## Lišta filtru

# machine-drafted, needs native review
# The filter field takes words (matched against names) and field:value terms; the field
# names and words of the query (rating, camera, ext, is:pick, …) stay in English.
gallery-filter-query-placeholder = Název, nebo rating>=3 camera:X-T5 …
gallery-filter-clear = Vymazat filtr
gallery-filter-rating = ★ { $stars } a více
gallery-filter-label = Štítek: { $label }
gallery-filter-picks = Vybrané
gallery-filter-hide-rejects = Skrýt zamítnuté
gallery-filter-unrated = Nehodnocené
gallery-filter-raw = RAW
gallery-filter-animated = Animované
gallery-filter-hdr = HDR
gallery-filter-landscape = Na šířku
gallery-filter-portrait = Na výšku

## Query errors and facets

gallery-query-unknown-field = Neznámé pole „{ $field }“
gallery-query-unknown-field-suggest = Neznámé pole „{ $field }“: měli jste na mysli „{ $suggestion }“?
gallery-query-missing-value = „{ $field }“ potřebuje hodnotu
gallery-query-bad-number = „{ $value }“ není číslo ({ $field })
gallery-query-bad-date = „{ $value }“ není datum ({ $field }): použijte 2024, 2024-05 nebo 2024-05-12
gallery-query-bad-word = „{ $value }“ se nehodí pro { $field }: použijte { $words }
# machine-drafted, needs native review
gallery-query-bad-exif-tag = „{ $value }“ není název značky Exif
gallery-query-no-compare = { $field } zde nelze porovnávat pomocí < nebo >
gallery-query-unterminated = Chybí uzavírací uvozovka
gallery-query-unavailable = „{ $field }“ vyžaduje knihovnu (zde není k dispozici)
gallery-facets = Fasety
gallery-facets-counting = Počítání…
gallery-facets-empty = Není co počítat
gallery-facets-search = Hledat ve fasetách
gallery-facets-no-match = Žádné hodnoty neodpovídají
# The gallery's top bar button (its tooltip): back to the start screen.
gallery-home = Úvodní obrazovka
gallery-facet-format = Formát
gallery-facet-camera = Fotoaparát
gallery-facet-lens = Objektiv
gallery-facet-rating = Hodnocení
gallery-facet-year = Rok
gallery-facet-orientation = Orientace
gallery-facet-label = Štítek
gallery-facet-properties = Vlastnosti
gallery-facet-square = Čtvercové
gallery-facet-alpha = Průhlednost
gallery-facet-gps = Poloha
gallery-facet-multipage = Více stran
# Screen readers: a facet value, e.g. “Camera: X-T5, 12 files”.
gallery-facet-a11y-value = { $facet }: { $value }, { $count ->
    [one] { $count } soubor
    [few] { $count } soubory
    [many] { $count } souboru
   *[other] { $count } souborů
}

## Skupiny

# An event group's header: the first and the last day of a shooting session.
gallery-group-event = { $start } – { $end }
gallery-group-event-place = { $place } · { $dates }
gallery-group-this-folder = Tato složka
gallery-group-unknown = Neznámé
gallery-group-unrated = Nehodnocené

## Třídění

gallery-stars = { $stars ->
    [one] { $stars } hvězdička
    [few] { $stars } hvězdičky
    [many] { $stars } hvězdičky
   *[other] { $stars } hvězdiček
}
gallery-rate-none = Bez hodnocení
gallery-pick = Vybrat
gallery-reject = Zamítnout
gallery-unflag = Bez příznaku
gallery-label-red = Červená
gallery-label-yellow = Žlutá
gallery-label-green = Zelená
gallery-label-blue = Modrá
gallery-label-none = Bez štítku
gallery-compare-hint = K porovnání vyberte 2 až 4 obrázky.

## Soubory

gallery-trash-count = { $count ->
    [one] Přesunout { $count } soubor do koše
    [few] Přesunout { $count } soubory do koše
    [many] Přesunout { $count } souboru do koše
   *[other] Přesunout { $count } souborů do koše
}
gallery-moved-to = { $count ->
    [one] { $count } soubor přesunut do složky { $folder }
    [few] { $count } soubory přesunuty do složky { $folder }
    [many] { $count } souboru přesunuto do složky { $folder }
   *[other] { $count } souborů přesunuto do složky { $folder }
}
gallery-copied-to = { $count ->
    [one] { $count } soubor zkopírován do složky { $folder }
    [few] { $count } soubory zkopírovány do složky { $folder }
    [many] { $count } souboru zkopírováno do složky { $folder }
   *[other] { $count } souborů zkopírováno do složky { $folder }
}
gallery-move-failed = Soubor { $file } nelze přesunout: { $reason }
gallery-drop-move = Přesunout do složky { $folder }
gallery-drop-copy = Kopírovat do složky { $folder }
gallery-move-here = Přesunout vybrané sem
gallery-copy-here = Kopírovat vybrané sem
gallery-not-an-image = { $file } není obrázek, který { -app-name } umí otevřít.
gallery-cache-cleared = Mezipaměť miniatur vymazána
# machine-drafted, needs native review
gallery-thumb-failed = Bez miniatury: { $reason }
gallery-cache-clear-failed = Mezipaměť miniatur nelze vymazat: { $reason }

## Vlastnosti

gallery-prop-name = Název
gallery-prop-folder = Složka
gallery-prop-size = Velikost souboru
gallery-prop-dimensions = Rozměry
gallery-prop-format = Formát
gallery-prop-date = Datum
gallery-prop-camera = Fotoaparát
gallery-prop-lens = Objektiv
gallery-prop-rating = Hodnocení
gallery-prop-label = Štítek
gallery-prop-items = Položky

## Čtečky obrazovky

gallery-a11y-list = { $folder }, { $count ->
    [one] { $count } položka
    [few] { $count } položky
    [many] { $count } položky
   *[other] { $count } položek
}
gallery-a11y-dims = { $width } na { $height }
gallery-a11y-folder = složka
gallery-a11y-up = Nahoru do { $folder }
gallery-a11y-archive = archiv
gallery-a11y-stack = skupina souborů: { $count }
gallery-a11y-filter-bar = Filtr
gallery-a11y-filter-field = Filtrovat podle názvu nebo pole
gallery-a11y-chip = Výraz filtru { $term }
gallery-a11y-chip-remove = Stisknutím odeberete
gallery-a11y-results = { $count ->
    [0] Žádné odpovídající položky
    [one] { $count } odpovídající položka
    [few] { $count } odpovídající položky
    [many] { $count } odpovídající položky
   *[other] { $count } odpovídajících položek
}

## Nastavení → Galerie

settings-section-gallery = Galerie
settings-gallery-layout = Rozvržení
settings-gallery-layout-help = Čtverce ořízne miniatury do mřížky; zarovnané řádky zachovají tvar každého obrázku; bez ořezu vloží celé obrázky do stejných buněk.
settings-gallery-layout-squares = Čtverce
settings-gallery-layout-justified = Zarovnané řádky
settings-gallery-layout-uncropped = Bez ořezu
settings-gallery-layout-details = Podrobnosti
settings-gallery-thumbnail-size = Velikost miniatur
settings-gallery-thumbnail-size-help = V galerii také Ctrl+kolečko myši, sevření prstů nebo Ctrl+plus a minus.
settings-gallery-captions = Zobrazovat názvy souborů
settings-gallery-captions-help = Popisek pod každou miniaturou.
settings-gallery-badges = Zobrazovat odznaky
settings-gallery-badges-help = Malé značky pro RAW, animaci, HDR, více stránek, hodnocení a štítek.
settings-gallery-sort-order = Řadit podle
settings-gallery-sort-order-help = Pořadí v galerii. Složky jsou vždy první.
settings-gallery-sort-order-name = Název
settings-gallery-sort-order-date-taken = Datum pořízení
settings-gallery-sort-order-modified = Datum změny
settings-gallery-sort-order-size = Velikost souboru
settings-gallery-sort-order-type = Typ souboru
settings-gallery-sort-order-dimensions = Rozměry
settings-gallery-sort-descending = Obrácené pořadí
settings-gallery-sort-descending-help = Řadit od posledního k prvnímu.
settings-gallery-group-by = Seskupit podle
settings-gallery-group-by-help = Záhlaví oddílů, která při posouvání zůstávají nahoře.
settings-gallery-group-by-none = Neseskupovat
settings-gallery-group-by-day = Den
settings-gallery-group-by-month = Měsíc
settings-gallery-group-by-year = Rok
settings-gallery-group-by-event = Událost
settings-gallery-group-by-folder = Složka
settings-gallery-group-by-camera = Fotoaparát
settings-gallery-group-by-format = Formát
settings-gallery-group-by-rating = Hodnocení
settings-gallery-group-by-target = Objekt
settings-gallery-group-by-filter = Filtr
settings-gallery-per-folder-view = Pamatovat si zobrazení pro každou složku
settings-gallery-per-folder-view-help = Rozvržení, velikost, řazení a seskupení se ukládají pro každou složku zvlášť; jinak se všude použije jedno zobrazení.
settings-gallery-open-folders-in-gallery = Otevírat složky v galerii
settings-gallery-open-folders-in-gallery-help = Po otevření složky zobrazit nejprve její miniatury místo prvního obrázku.
settings-gallery-escape-opens-gallery = Esc otevře galerii
settings-gallery-escape-opens-gallery-help = Když už není co zavřít, Esc v prohlížeči přejde do galerie, místo aby nic neudělal.
settings-gallery-wrap-around = Šipky přecházejí dokola
settings-gallery-wrap-around-help = Na konci seznamu šipky přejdou na začátek a ze začátku na konec. Při držení klávesy se pohyb na konci zastaví.
settings-gallery-show-hidden = Zobrazovat skryté soubory
settings-gallery-show-hidden-help = Zahrnout soubory a složky, jejichž název začíná tečkou.
settings-gallery-show-other-files = Zobrazovat ostatní soubory
settings-gallery-show-other-files-help = Zahrnout soubory, které { -app-name } neumí otevřít, s obecnou ikonou.
settings-gallery-show-archives = Zobrazovat archivy
settings-gallery-show-archives-help = Zahrnout komiksy a jiné archivy (zip, cbz, rar, 7z…) jako dlaždice. Přímé otevření archivu funguje i tak.
settings-gallery-subfolder-depth = Úrovně podsložek
settings-gallery-subfolder-depth-help = Zobrazovat i obrázky z podsložek až do tohoto počtu úrovní (0: jen tato složka).
settings-gallery-stacks = Seskupovat související soubory
settings-gallery-stacks-help = Zobrazit dvojice RAW+JPEG a série jako jednu dlaždici. Skupinu rozbalíte klávesou S.
settings-gallery-info-overlay = Údaje na miniaturách
settings-gallery-info-overlay-help = Rozměry a expozice přes každou miniaturu.
settings-gallery-prefer-embedded = Upřednostňovat vložené náhledy
settings-gallery-prefer-embedded-help = Použít náhled uložený v souboru, je-li dost velký. Mnohem rychlejší, o něco méně přesné.
settings-gallery-sharper-raw-thumbnails = Ostřejší miniatury RAW
settings-gallery-sharper-raw-thumbnails-help = Upravené fotografie RAW se pro miniatury vyvolávají v 1024 px. Ostřejší, o něco pomalejší. Vypnuto: 512 px.
settings-gallery-rating-sidecars = Zapisovat hodnocení do souborů XMP
settings-gallery-rating-sidecars-help = Hodnocení, štítky a příznaky se ukládají i do souboru .xmp vedle obrázku, který čtou jiné fotografické aplikace.
settings-gallery-cache-size = Velikost mezipaměti miniatur
settings-gallery-cache-size-help = Když je mezipaměť plná, nejstarší miniatury se odstraní.
settings-gallery-cache-location = Umístění mezipaměti miniatur
settings-gallery-cache-location-help = Nejlépe funguje složka na rychlém disku.
settings-gallery-clear-cache = Vymazat mezipaměť miniatur

## Compare
# machine-drafted, needs native review

compare-side = Vedle sebe
compare-overlay = Překryv
compare-difference = Rozdíl
compare-flicker = Blikání
compare-blend = Prolnutí
compare-kind-absolute = Absolutní chyba
compare-kind-squared = Kvadratická chyba
compare-kind-relative = Relativní chyba
compare-scale-down = Méně citlivé
compare-scale-up = Citlivější
compare-scale = Plný rozsah { $value }
compare-space = Lineární světlo, jak se zobrazuje
compare-space-help = Chyby a metriky se měří v lineárním světle tak, jak se obrázek zobrazuje, po mapování tónů, pro každý kanál včetně alfa.
compare-metrics = MAE { $mae }  ·  MSE { $mse }  ·  PSNR { $psnr } dB
compare-sampled = vzorkováno v { $width } × { $height }
compare-aligned = Zarovnáno vlevo nahoře, porovnáno { $width } × { $height }
compare-vs = { $b } oproti { $a }
compare-overlay-label = { $b } přes { $a }, { PERCENT($percent) }
compare-pending = Měření…
# machine-drafted, needs native review
compare-diff-failed = Rozdíl nelze zobrazit: { $reason }
compare-no-overlap = V zobrazení není co porovnat
compare-zoom-fit = Přizpůsobit
compare-zoom = { PERCENT($percent) }
compare-showing = Zobrazeno: { $name }
compare-a11y-group = Porovnání
compare-a11y-blend = Prolnutí mezi obrázky
compare-a11y-zoom = Zvětšení
gallery-palette-compare-side = Porovnat vedle sebe
gallery-palette-compare-overlay = Porovnat jako překryv
gallery-palette-compare-difference = Porovnat rozdíl: { $kind }
gallery-palette-compare-flicker = Blikat mezi porovnávanými obrázky
gallery-palette-compare-actual = Porovnat při { PERCENT(100) }

## Search value list (machine-drafted, needs native review)
gallery-picker-hint = Mezerník nebo Ctrl+Enter vybere více

## Search value picker, part 2 (machine-drafted, needs native review)
gallery-facet-profile = Barevný profil
gallery-facet-date = Datum pořízení
gallery-facet-picked = Vybrané
gallery-facet-rejected = Zamítnuté
gallery-facet-flagged = Vybrané nebo zamítnuté
gallery-facet-rated = Hodnocené
gallery-facet-labelled = Se štítkem

## Search value picker, part 3 (machine-drafted, needs native review)

gallery-facet-bits = Bitová hloubka
gallery-facet-pages = Stránky
gallery-facet-width = Šířka
gallery-facet-height = Výška
gallery-facet-megapixels = Megapixely
gallery-picker-bits = { $bits }bitová
gallery-picker-pages = { $pages ->
    [one] { $pages } stránka
    [few] { $pages } stránky
    [many] { $pages } stránky
   *[other] { $pages } stránek
}
gallery-picker-pixels = { $px } px
gallery-picker-megapixels = { $mp } Mpx
gallery-picker-group-any = Libovolný ve skupině
gallery-picker-group-keep = Navrženo ponechat
gallery-picker-group-discard = Navrženo vyřadit
gallery-picker-ops = Porovnat
gallery-picker-op-eq = Rovno
gallery-picker-op-ge = Alespoň
gallery-picker-op-le = Nejvýše
gallery-picker-ge = alespoň { $value }
gallery-picker-gt = více než { $value }
gallery-picker-le = nejvýše { $value }
gallery-picker-lt = méně než { $value }

# Map view.
gallery-facet-place = Místo
gallery-palette-map = Mapa
map-title = Mapa
map-summary = Fotky s polohou: { $count }
map-photos = Fotky: { $count }
map-zoom-in = Přiblížit
map-zoom-out = Oddálit
map-show-all = Zobrazit vše
map-done = Hotovo
map-credit = Mapová data: Natural Earth
# machine-drafted, needs native review
# With a base map's tiles on screen: $credit is the tile source's credit ("© OpenStreetMap
# contributors").
map-credit-tiles = Mapa: { $credit } · Natural Earth
# The one-time explanation before Specular downloads map tiles from a server; one idea per
# line. $host is the server's name, $size the cache size ("256 MB").
map-basemap-consent-title = Online mapové dlaždice
map-basemap-consent-server = Mapové dlaždice pocházejí z { $host }.
map-basemap-consent-sees = Tento server vidí vaši internetovou adresu a oblasti, které prohlížíte.
map-basemap-consent-private = Vaše fotky, jejich názvy ani polohy se neodesílají.
map-basemap-consent-cache = Dlaždice se ukládají v tomto počítači, až { $size }.
map-basemap-allow = Použít online mapu
map-basemap-decline = Ponechat offline mapu
# Why the base map is not shown; $reason is one of the lines below or a file error.
map-basemap-error = Podkladová mapa se nezobrazuje: { $reason }
map-basemap-bad-address = adresa serveru dlaždic musí obsahovat {"{"}z{"}"}, {"{"}x{"}"} a {"{"}y{"}"}.
map-basemap-unknown-tiles = dlaždice v mapovém souboru jsou neznámého druhu.
map-loading = Načítání poloh fotek…
map-empty = Žádná z těchto fotek nemá polohu
map-hidden = Polohy fotek jsou skryté (Nastavení › Metadata a soukromí)
map-a11y-group = Mapa poloh fotek
map-a11y-status = Zobrazená místa: { $count }, úroveň přiblížení { $zoom }
