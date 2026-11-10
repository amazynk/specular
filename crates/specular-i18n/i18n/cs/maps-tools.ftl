# machine-drafted, needs native review
# Specular — Maps tools: terrain (slope, aspect, contours, 3-D), time, wind and weather colours,
# online map services, photos and geotagging along GPS tracks, and map export.
# Čeština.

## Cheat sheet groups of the Maps tab.
cheatsheet-map-terrain = Terén
cheatsheet-map-time = Čas a počasí
cheatsheet-map-online = Online služby
cheatsheet-map-photos = Fotky a GPS
cheatsheet-map-output = Výstup

## Palette entries and their one-line descriptions.
palette-map-slope = Barvy sklonu
palette-map-slope-help = Obarvit výškový model podle strmosti.
palette-map-aspect = Barvy orientace svahů
palette-map-aspect-help = Obarvit výškový model podle směru, kterým jsou svahy orientovány.
palette-map-contours = Zobrazit nebo skrýt vrstevnice
palette-map-contours-help = Vrstevnice přes výškový model, s popisem každých několik čar.
palette-map-terrain-3d = Terén ve 3D
palette-map-terrain-3d-help = Zobrazit výškový model jako 3D povrch s jeho barvami.
palette-map-time = Zobrazit nebo skrýt časový posuvník
palette-map-time-help = Procházet časy mřížky nebo časy trasy.
palette-map-play = Přehrát nebo pozastavit čas
palette-map-play-help = Nechat časový posuvník běžet samotný, krok za krokem.
palette-map-wind = Zobrazit nebo skrýt šipky větru
palette-map-wind-help = Šipky rychlosti a směru větru ze složek u a v mřížky.
palette-map-weather-colours = Další barevná mapa počasí
palette-map-weather-colours-help = Barevné mapy pro teplotu, srážky, vítr a tlak.
palette-map-services = Online mapové služby…
palette-map-services-help = Přidat vrstvy ze serverů WMS, WMTS, WFS, OGC API nebo STAC.
palette-map-photos-on-track = Zobrazit nebo skrýt fotky na trase
palette-map-photos-on-track-help = Umístit fotky ze složky podél trasy GPS podle času pořízení.
palette-map-geotag = Geotagovat fotky podle trasy…
palette-map-geotag-help = Zapsat polohy z trasy do doprovodných souborů XMP fotek ve složce.
palette-map-export = Exportovat mapu…
palette-map-export-help = Uložit mapu jako PNG nebo PDF v měřítku, s legendou, měřítkem a severkou.
palette-map-print = Tisknout mapu v měřítku…
palette-map-print-help = Vytisknout mapu ve zvoleném měřítku, například 1:{ NUMBER(25000) }.
palette-map-copy-extent = Kopírovat rozsah
palette-map-copy-extent-help = Zkopírovat oblast na obrazovce jako ohraničující obdélník, nebo jako WKT.

## Why a map command is unavailable.
disabled-no-time = Mapa nemá žádné časy
disabled-no-wind = Mapa nemá složky větru (u a v)
disabled-no-track = Žádná trasa GPS s časy
disabled-not-grid = Mapa není mřížka hodnot

## Settings › Maps and Metadata.
settings-maps-services = Online mapové služby
settings-maps-services-help = Přidávat vrstvy ze serverů WMS, WMTS, WFS, OGC API a STAC. Vypnuto: nic se nestahuje.
settings-maps-services-allowed = Povolené mapové servery
settings-maps-services-allowed-help = Servery, se kterými jste souhlasili po vysvětlení, co vidí.
settings-metadata-basemap-service = Vrstva mapové služby
settings-metadata-basemap-service-help = Vrstva zvolená v panelu online služeb mapy.

## Terrain.
# A compass point, short: n, ne, e, se, s, sw, w, nw.
map-compass = { $point ->
    [ne] SV
    [e] V
    [se] JV
    [s] J
    [sw] JZ
    [w] Z
    [nw] SZ
   *[n] S
}
map-slope-aspect = Sklon { $slope }° · orientace { $direction } ({ $aspect }°)
map-slope-flat = Sklon { $slope }° · rovina
map-slope-on = Barvy sklonu
map-aspect-on = Barvy orientace svahů
map-relief-off = Barvy výšek
map-contours-on = Vrstevnice zapnuty
map-contours-off = Vrstevnice vypnuty
map-legend-slope = Sklon
map-legend-aspect = Orientace: kam svahy směřují
map-legend-degrees = { $degrees }°
map-legend-degrees-more = { $degrees }° a více
map-terrain-3d-on = Terén ve 3D. Esc se vrátí na mapu.
map-terrain-3d-on-exaggerated = Terén ve 3D, výšky × { $times }. Esc se vrátí na mapu.
map-terrain-exaggeration = Výšky převýšeny { $times }×.
map-terrain-drape = Barvy mapy

## Time, weather and wind.
map-time-grid = { $when } · { $step } z { $steps }
map-time-track = { $when } · { $elapsed } od startu
map-time-speed = × { $times }
map-time-play = Přehrát
map-time-pause = Pozastavit
map-weather-map = Barvy: { $name }
map-weather-file = Barvy ze souboru
map-wind-on = Šipky větru zapnuty
map-wind-off = Šipky větru vypnuty
map-wind-at = Vítr { $speed } { $unit } od { $degrees }° ({ $direction })

## Copy extent.
map-extent-copied = Ohraničující obdélník zkopírován. Zkopírujte znovu pro WKT.
map-extent-copied-wkt = Rozsah zkopírován jako WKT

## Online map services.
map-services-title = Online mapové služby
map-service-off = Online mapové služby jsou vypnuté.
map-service-off-why = Zapněte je, chcete-li přidávat vrstvy ze serverů na internetu.
map-service-turn-on = Zapnout
map-service-close = Zavřít
map-service-back = Zpět
map-service-cancel = Zrušit
map-service-connect = Připojit
map-service-allow = Povolit
map-service-remove = Odebrat
map-service-download = Stáhnout
map-service-list-hint = Enter otevře server. Delete jej odebere.
map-service-add = Přidat server…
map-service-add-sub = WMS, WMTS, WFS, OGC API nebo STAC
map-service-add-hint = Adresa serveru. Enter se připojí.
map-service-bad-address = Není webová adresa (http nebo https, bez uživatelského jména).
map-service-server-sub = { $kind } · { $host }
map-service-consent-title = Online mapová služba
map-service-consent-server = Specular se spojí se serverem { $host }.
map-service-consent-sees = Tento server vidí vaši internetovou adresu a oblast mapy, o kterou žádáte.
map-service-consent-private = Vaše soubory a jejich názvy se neodesílají.
map-service-asking = Dotaz na { $host }…
map-service-downloading = Stahování z { $host }…
map-service-layers-hint = Vrstvy { $kind }. Enter jednu použije.
map-service-no-layers = Server neuvádí žádné vrstvy.
map-service-offer-tiles = Dlaždice: stane se podkladovou mapou mapy fotek
map-service-offer-features = Prvky: přidají se do této mapy pro oblast na obrazovce
map-service-offer-search = Snímky: hledat v oblasti na obrazovce
map-service-refused-mercator = Nenabízí se ve Web Mercator (EPSG:3857), který podkladová mapa potřebuje.
map-service-refused-format = Nenabízí se jako obrázky PNG nebo JPEG.
map-service-refused-geojson = Nenabízí se jako GeoJSON, formát, který se zde čte.
map-service-items-hint = Nalezené snímky: { $count }. Enter jeden stáhne.
map-service-no-items = Zde nebyly nalezeny žádné snímky.
map-service-item-cloud = { $when } · oblačnost { PERCENT($cloud) }
map-service-item-no-cog = Bez Cloud-Optimized GeoTIFF
map-service-download-ask = Stáhnout { $size } z { $host }?
map-service-download-ask-unknown = Stáhnout tento snímek z { $host }? Jeho velikost není známa.
map-service-download-cache = Uloží se do mapové mezipaměti a otevře.
map-service-size-mb = { $size } MB
map-service-size-gb = { $size } GB
map-service-too-big = Snímek má { $size }, je příliš velký ke stažení.
map-service-opened = Otevírá se stažený snímek
map-service-basemap = { $layer } je nyní podkladovou mapou mapy fotek. Mapa fotek se před prvním stažením zeptá.
map-service-status = Server odpověděl { $status }.
map-service-unknown = Není mapová služba, kterou tato verze čte.
map-service-exception = Server hlásí: { $message }
map-service-malformed = Odpověď serveru je poškozená: { $reason }
map-service-remove-ask = Odebrat „{ $server }“ z uložených serverů?
map-service-remove-detail = Nic na disku se neztratí. Vrstvy již na mapě zůstanou až do jejího zavření.
map-service-removed = { $server } odebrán
map-service-saved = Uložené mapové servery
map-service-saved-none = Zatím žádné. Přidejte server v panelu online služeb mapy.
map-service-keywords = WMS WMTS WFS OGC API STAC servery mapové služby

## Photos and geotagging.
map-geotag-title = Geotagovat fotky podle trasy
map-photos-on = Fotky na trase
map-photos-off = Fotky na trase skryty
map-geotag-offset = Hodiny fotoaparátu: { $offset } od času GPS. ← → mění o minutu, se Shift o hodinu, s Alt o sekundu.
map-geotag-reading = Čtení fotek…
map-geotag-summary = Polohu dostane { $count } z { $total } fotek.
map-geotag-sidecars = Polohy se zapíšou do doprovodných souborů XMP vedle fotek. Soubory fotek se nemění.
map-geotag-writing = Zapisování…
map-geotag-write = Zapsat soubory XMP…
map-geotag-guess = Odhadnout posun
map-geotag-will-write = Dostane { $position }
map-geotag-has-position = Polohu již má: ponechá se
map-geotag-no-time = Bez času pořízení: ponechá se
map-geotag-outside = Pořízeno { $offset } mimo trasu: ponechá se
map-geotag-gap = V mezeře trasy ({ $seconds } s od fixu): ponechá se
map-geotag-nothing = Při tomto posunu nedostane polohu žádná fotka.
map-geotag-confirm = Zapsat polohy do { $count ->
    [one] { $count } souboru XMP
    [few] { $count } souborů XMP
    [many] { $count } souboru XMP
   *[other] { $count } souborů XMP
}? Samotné fotky se nemění.
map-geotag-confirm-ok = Zapsat soubory XMP
map-geotag-done = { $count ->
    [one] Zapsán { $count } soubor XMP
    [few] Zapsány { $count } soubory XMP
    [many] Zapsáno { $count } souboru XMP
   *[other] Zapsáno { $count } souborů XMP
}
map-geotag-done-failed = Zapsané soubory XMP: { $count }; nezdařilo se: { $failed }: { $reason }
map-geotag-not-sidecar = Není název doprovodného souboru.

## Export and printing.
map-export-title = Exportovat mapu
map-print-title = Tisknout mapu v měřítku
map-export-keys = ↑ ↓ vybírá, ← → mění, Enter provede.
map-export-format = Formát
map-export-scale = Měřítko
map-export-scale-value = 1:{ $n }
map-export-resolution = Rozlišení
map-export-dpi = { $dpi } dpi
map-export-paper = Papír
map-export-size = { $width } × { $height } px · { $across } × { $down } km v terénu
map-export-save = Uložit…
map-export-print = Tisknout…
map-export-too-large = Při tomto měřítku příliš velké: { $width } × { $height } px. Zvolte menší měřítko nebo rozlišení.
map-export-no-scale = Tato mapa nemá měřítko v terénu.
map-export-no-gpu = Export potřebuje grafický procesor.
map-export-subtitle = Měřítko { $scale } · { $crs }
map-export-data = Data: { $sources }
map-export-saved = Uloženo { $name }
map-export-printing = Tisk…
map-export-printed = Odesláno na tiskárnu
map-export-print-failed = Nevytištěno: { $reason }
