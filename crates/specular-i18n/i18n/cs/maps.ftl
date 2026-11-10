# machine-drafted, needs native review
# Specular — Maps (the Maps profile): the map view of georeferenced and vector map files, its
# layer panel, attribute table, coordinates, measuring, elevation profile and map furniture.
# English.

keys-mode-map = Mapy
# Cheat sheet groups of the Maps tab.
cheatsheet-map-layers = Vrstvy
cheatsheet-map-coordinates = Souřadnice
cheatsheet-map-measure = Měření

## Palette entries and their one-line descriptions.
palette-map-layers = Zobrazit nebo skrýt panel vrstev
palette-map-layers-help = Vrstvy mapy: pořadí, krytí, viditelnost a styl.
palette-map-add-layer = Přidat vrstvu ze souboru…
palette-map-add-layer-help = Vykreslit přes tuto mapu další mapový soubor, například trasu GPX přes shapefile.
palette-map-zoom-to-layer = Přiblížit na vrstvu
palette-map-zoom-to-layer-help = Zobrazit celou vrstvu vybranou v panelu vrstev.
palette-map-attribute-table = Zobrazit nebo skrýt atributovou tabulku
palette-map-attribute-table-help = Prvky vybrané vrstvy jako tabulka: řazení, filtrování, hledání a výběr.
palette-map-coordinate-format = Další formát souřadnic
palette-map-coordinate-format-help = Desetinné stupně, stupně minuty sekundy, UTM nebo MGRS.
palette-map-units = Metrické nebo imperiální jednotky
palette-map-units-help = Délky a plochy v metrech, nebo ve stopách a mílích.
palette-map-copy-coordinate = Kopírovat souřadnici
palette-map-copy-coordinate-help = Zkopírovat polohu pod ukazatelem, nebo středu mapy.
palette-map-go-to-coordinate = Přejít na souřadnici…
palette-map-go-to-coordinate-help = Zadejte polohu v libovolném formátu a mapa se na ni vystředí.
palette-map-measure-distance = Změřit vzdálenost
palette-map-measure-distance-help = Klikejte na body na mapě: délka po zemském povrchu a azimut.
palette-map-measure-area = Změřit plochu
palette-map-measure-area-help = Klikejte na rohy oblasti: její velikost a obvod na zemském povrchu.
palette-map-elevation-profile = Výškový profil
palette-map-elevation-profile-help = Nakreslete čáru přes výškový model a zobrazte graf jeho výšek.
palette-map-overview = Zobrazit nebo skrýt přehledovou mapu
palette-map-overview-help = Malá mapa celého rozsahu s vyznačenou částí na obrazovce.

## Why a map command is unavailable.
disabled-not-map = Pouze pro mapy, se zapnutými mapovými nástroji
disabled-no-lonlat = Mapa nemá známý souřadnicový systém
disabled-no-features = Mapa nemá žádné prvky
disabled-no-heights = Mapa nemá žádné výšky

## Settings › Maps.
settings-section-maps = Mapy
settings-maps-coordinate-format = Formát souřadnic
settings-maps-coordinate-format-help = Jak se polohy zobrazují a kopírují. Přejít na souřadnici čte každý formát.
settings-maps-coordinate-format-dd = Desetinné stupně
settings-maps-coordinate-format-dms = Stupně, minuty, sekundy
settings-maps-coordinate-format-utm = UTM
settings-maps-coordinate-format-mgrs = MGRS / USNG
settings-maps-units = Jednotky
settings-maps-units-help = Délky a plochy na mapách: měřítko a měření. Automaticky se řídí nastavením Metadata › Jednotky.
settings-maps-units-auto = Automaticky
settings-maps-units-metric = Metrické
settings-maps-units-imperial = Imperiální

## The pointer readout. $position is written in the chosen format.
map-position = { $format ->
    [utm] UTM { $position }
    [mgrs] MGRS { $position }
    [dms] { $position }
   *[dd] Šířka, délka { $position }
}
map-height = Výška { $height }
map-height-label = Výška
# A length or area with its unit symbol ("12.3 km", "4.5 ha").
map-length = { $value } { $unit }
map-scale-label = Měřítko: { $length }
map-north-label = Severka: sever je { $degrees }° po směru hodinových ručiček od svislice
# The coordinate formats, for the toast after "Next coordinate format".
map-format-name = { $format ->
    [dms] stupně, minuty, sekundy
    [utm] UTM
    [mgrs] MGRS
   *[dd] desetinné stupně
}
map-format-toast = Souřadnice: { $format }
map-units-metric = Jednotky: metry a kilometry
map-units-imperial = Jednotky: stopy a míle
toast-copied-coordinate = Souřadnice zkopírována

## Go to coordinate.
map-goto-hint = Desetinné stupně, DMS, UTM nebo MGRS. Enter tam přejde.
map-goto-empty = Zadejte polohu.
map-goto-unrecognised = Není to poloha ve známém formátu.
map-goto-range = Zeměpisná šířka nebo délka mimo rozsah.
map-goto-no-way = Tato mapa neumí umístit polohu.
map-goto-outside = Tato poloha je mimo mapu.
map-goto-toast = Vystředěno na { $position }

## The layer panel.
map-layers-title = Vrstvy: zobrazeno { $shown } z { $total }
# Under a layer's name: its kind, features and opacity.
map-layer-sub = { $kind } · { $features } · { PERCENT($opacity) }
# A layer's row for screen readers.
map-layer-row = { $name }: { $kind }, { $features ->
    [one] { $features } prvek
    [few] { $features } prvky
    [many] { $features } prvku
   *[other] { $features } prvků
}, krytí { PERCENT($opacity) }
map-kind-area = Plochy
map-kind-line = Linie
map-kind-point = Body
map-kind-empty = Žádné prvky
map-layer-visible = Zobrazit nebo skrýt vrstvu
map-layer-opacity-down = Méně krycí
map-layer-opacity-up = Více krycí
map-layer-colour = Další barva
map-layer-width = Další šířka čáry
map-layer-point = Další velikost bodu
map-layer-up = Posunout nahoru
map-layer-down = Posunout dolů
map-layer-remove = Odebrat
map-layer-keep = Ponechat
map-layer-remove-ask = Odebrat { $name } z mapy? Soubor zůstane na disku.
map-layer-adding = Přidávání vrstvy…
map-removed = { $name } odebrán z mapy
map-added = { $layers ->
    [one] Přidáno: { $name }
    [few] Přidáno: { $name }, { $layers } vrstvy
    [many] Přidáno: { $name }, { $layers } vrstvy
   *[other] Přidáno: { $name }, { $layers } vrstev
}
map-add-failed = { $name } nelze přidat: { $reason }
map-add-filter = Mapové soubory
map-add-not-map = není to mapa.
map-add-not-vector = nemá žádné prvky.
map-add-no-crs = nemá soubor projekce (.prj).

## Identify.
map-identify-more = a { $count } dalších v atributové tabulce
map-cell = Buňka { $x }, { $y }
map-band = Pásmo { $n }
map-no-data = Žádná data

## The attribute table.
map-table-title = { $layer }: { $shown } z { $total } prvků, vybráno { $selected }
map-table-search = Hledat, nebo pole = hodnota
map-table-selected-only = Jen vybrané
map-table-empty = Žádné prvky neodpovídají.
map-table-many = mnoho
map-table-stats-number = { $field }: hodnot { $count }, prázdných { $empty } · min { $min } · max { $max } · průměr { $mean } · součet { $sum } · sm. odch. { $sd }
map-table-stats-text = { $field }: hodnot { $count }, prázdných { $empty } · různých { $distinct }
map-table-stats-text-top = { $field }: hodnot { $count }, prázdných { $empty } · různých { $distinct } · nejčastější { $top } ({ $times }×)

## Measuring and the elevation profile.
map-measure-hint-distance = Klikejte na body podél cesty.
map-measure-hint-area = Klikejte na rohy oblasti.
map-measure-hint-profile = Klikejte na body podél čáry.
map-measure-keys = Enter dokončí. Backspace odebere poslední bod. Esc zavře.
map-measure-distance = Vzdálenost { $length }
map-measure-bearing = Azimut { $bearing }
map-measure-segment = Poslední úsek { $length }, azimut { $bearing }
map-measure-area = Plocha { $area }
map-measure-perimeter = Obvod { $length }
map-bearing-value = { $degrees }°
map-profile-summary = Délka { $length } · nejnižší { $min } · nejvyšší { $max } · stoupání { $climb } · klesání { $descent }
map-profile-no-heights = Délka { $length }. Podél čáry nejsou žádné výšky.
map-profile-at = { $distance }: { $height }
map-profile-at-none = { $distance }: bez výšky
