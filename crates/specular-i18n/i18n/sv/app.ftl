# machine-drafted, needs native review
# Specular — bildvisarens gränssnitt, kommandopalett, felmeddelanden, Om. Svenska.

-app-name = Specular

## Verktygsfältets inforutor och tillgänglighetsetiketter

toolbar-open = Öppna bild…
toolbar-prev = Föregående bild
toolbar-next = Nästa bild
toolbar-back-gallery = Tillbaka till galleriet
toolbar-zoom-out = Zooma ut
toolbar-zoom-in = Zooma in
toolbar-zoom-level = Zoomnivå och förval
toolbar-zoom-fit = Anpassa till fönstret
toolbar-zoom-100 = Verklig storlek ({ PERCENT(100) })
toolbar-rotate-left = Rotera moturs
toolbar-rotate-right = Rotera medurs
toolbar-fullscreen-enter = Visa i helskärm
toolbar-fullscreen-exit = Avsluta helskärm
toolbar-info = Bildinformation
toolbar-settings = Inställningar
toolbar-delete = Flytta till papperskorgen
toolbar-copy = Kopiera bild
toolbar-palette = Kommandopalett
toolbar-go-to = Gå till bild…
toolbar-go-to-range = 1–{ $count }
# machine-drafted, needs native review
toolbar-customise = Anpassa verktygsfältet…
toolbar-more = Mer
toolbar-announce-moved = { $item }: position { $index } av { $count }
toolbar-announce-added = { $item } tillagd: position { $index } av { $count }
toolbar-announce-removed = { $item } borttagen från verktygsfältet
toolbar-announce-reset = Verktygsfältet återställt
# En inforuteetikett följd av dess kortkommando.
tooltip-with-shortcut = { $label } ({ $shortcut })
# Inforuta för en kontroll som inte kan användas just nu; $reason är ett av disabled-*-meddelandena.
tooltip-unavailable = { $label } (inte tillgänglig: { $reason })
# Varför en kontroll eller ett kommando inte är tillgängligt.
disabled-no-image = Ingen bild är öppen
disabled-still-loading = Bilden läses fortfarande in
disabled-decode-failed = Bilden kunde inte avkodas
disabled-no-other-images = Inga andra bilder i mappen

## Avläsningar

# Position i mappen, t.ex. "12 / 348".
readout-position = { $index } / { $count }
# Läses upp av skärmläsare för positionen.
readout-position-a11y = Bild { $index } av { $count }
# $zoom är en redan formaterad procentsats, t.ex. "150 %".
readout-zoom = { $zoom }
readout-zoom-a11y = Zoom { $zoom }
readout-zoom-fit = Anpassad ({ $zoom })
zoom-preset-fit = Anpassa till fönstret
zoom-preset-fill = Fyll fönstret
zoom-preset-actual = Verklig storlek
# machine-drafted, needs native review
# The last entry of the zoom presets: the window fills the screen (ticked while it does).
zoom-preset-fullscreen = Helskärm
# $zoom är en redan formaterad procentsats.
zoom-preset-percent = { $zoom }
# Bildens mått i pixlar, t.ex. "6000 × 4000".
readout-dimensions = { $width } × { $height }
# Antal bilder i aktuell mapp.
status-image-count =
    { $count ->
        [one] { $count } bild
       *[other] { $count } bilder
    }

## Kommandopalett

palette-placeholder = Skriv ett kommando eller en inställning…
palette-no-results = Inga matchande kommandon
# Ändra en inställning från paletten, t.ex. "Ställ in: Mushjul → Nästa bild".
palette-set-setting = Ställ in: { $setting } → { $value }
palette-next = Nästa bild
palette-prev = Föregående bild
palette-first = Första bilden
palette-last = Sista bilden
palette-go-to = Gå till bild…
palette-zoom-in = Zooma in
palette-zoom-out = Zooma ut
palette-zoom-fit = Zooma till anpassad
palette-zoom-fill = Zooma till fyllt fönster
# machine-drafted, needs native review
palette-zoom-fit-width = Anpassa till bredd
palette-zoom-fit-height = Anpassa till höjd
palette-zoom-100 = Zooma till { PERCENT(100) }
palette-zoom-200 = Zooma till { PERCENT(200) }
palette-zoom-300 = Zooma till { PERCENT(300) }
palette-zoom-400 = Zooma till { PERCENT(400) }
palette-zoom-rectangle = Zooma till markering
palette-rotate-left = Rotera moturs
palette-rotate-right = Rotera medurs
palette-flip-horizontal = Vänd vågrätt
palette-flip-vertical = Vänd lodrätt
palette-toggle-fullscreen = Växla helskärm
palette-navigator = Visa eller dölj navigatorn
palette-loupe = Lupp
palette-pixel-grid = Visa eller dölj pixelrutnät
palette-info-panel = Visa eller dölj informationspanelen
palette-copy-image = Kopiera bild
# machine-drafted, needs native review
palette-read-codes = Läs koder
# machine-drafted, needs native review
palette-find-text = Hitta text
# machine-drafted, needs native review
palette-face-boxes = Visa ansikten
palette-copy-path = Kopiera filsökväg
palette-move-to-trash = Flytta till papperskorgen
palette-undo-trash = Ångra flytt till papperskorgen
palette-open = Öppna bild…
# machine-drafted, needs native review
palette-open-address = Öppna adress…
palette-settings = Inställningar
# machine-drafted, needs native review
palette-customise-toolbar = Anpassa verktygsfältet…
palette-cheat-sheet = Kortkommandon
palette-hud = Visa eller dölj bildtids-HUD
palette-language = Språk
palette-theme = Tema
# machine-drafted, needs native review
palette-interface-larger = Större gränssnitt
palette-interface-smaller = Mindre gränssnitt
palette-interface-reset = Gränssnitt på { PERCENT(100) }
palette-quit = Avsluta { -app-name }

## Lathund

cheatsheet-title = Kortkommandon
cheatsheet-navigation = Navigering
cheatsheet-zoom-pan = Zooma och panorera
cheatsheet-view = Visa
cheatsheet-file = Arkiv
cheatsheet-window = Fönster
cheatsheet-help = Hjälp
cheatsheet-hold = Håll ned { $key }
cheatsheet-close-hint = Tryck på { $key } eller Esc för att stänga

## Fel

# machine-drafted, needs native review
viewer-no-preview = Ingen förhandsvisning i filen.
error-cannot-open = Det går inte att öppna { $file }
error-cannot-open-reason = Det går inte att öppna { $file }: { $reason }
error-unsupported-format = { $file } har ett format som { -app-name } inte kan läsa än.
error-file-truncated = { $file } är ofullständig. Den del som kunde läsas visas.
error-file-too-large = { $file } är för stor för att öppnas ({ $megapixels } megapixlar; gränsen är { $limit } megapixlar).
error-not-found = { $file } finns inte längre.
error-decoder-stopped = avkodaren stoppade utan att skapa någon bild
# machine-drafted, needs native review
# Shown after "Can’t open FILE:" for brain data (GIFTI .func/.shape/.label, CIFTI) that is
# drawn on a surface: no .surf.gii with that many vertices was found in the same folder.
error-needs-surface = den kräver en hjärnyta (en .surf.gii-fil med { $vertices } hörn) i samma mapp
error-permission-denied = Du har inte behörighet att läsa { $file }.
error-details = Detaljer
crash-last-time = { -app-name } avslutades oväntat förra gången; en rapport sparades i { $path }
error-trash-failed = Det gick inte att flytta { $file } till papperskorgen: { $reason }
# machine-drafted, needs native review
# Orsaker, visas efter "Det går inte att öppna FILE:" och andra "Det gick inte att …:"-meddelanden,
# så de börjar med liten bokstav. De säger vad som hände och, där det hjälper, vad man kan göra.
error-reason-not-found = den finns inte där längre. Den kan ha flyttats, bytt namn eller tagits bort.
error-reason-permission = du har inte behörighet att läsa den. Kontrollera ägare och behörigheter.
error-reason-disk-full = disken är full. Frigör utrymme och försök igen.
error-reason-read-only = disken är skrivskyddad.
error-reason-truncated = filen är ofullständig. Den kanske fortfarande kopieras eller laddas ned.
error-reason-is-folder = det är en mapp, inte en fil.
# A remote store (a Zarr image on a server or in an S3 bucket).
error-reason-remote-denied = servern nekade åtkomst. Datan kan vara privat; för en S3-bucket måste du först konfigurera AWS-inloggningsuppgifter.
error-reason-remote-missing = det finns ingen Zarr-data på den adressen.
# $status is an HTTP status code ("500").
error-reason-remote-status = servern svarade med fel { $status }.
error-reason-offline = servern går inte att nå. Kontrollera adressen och anslutningen.
# machine-drafted, needs native review
error-reason-network-off = nätverket är avstängt i Inställningar › Integritet.
error-reason-timed-out = enheten eller servern svarade inte i tid. Kontrollera anslutningen och försök igen.
error-reason-memory = det finns inte tillräckligt med minne. Stäng andra bilder eller program och försök igen.
error-reason-unsupported = den har ett format som { -app-name } inte kan läsa än.
# $detail är avkodarens egna ord, tekniska och på engelska ("12-bit lossless JPEG").
error-reason-unsupported-detail = den använder en funktion som { -app-name } inte kan läsa än ({ $detail }).
# machine-drafted, needs native review
# A file whose extension several formats share (".img") and whose content is none of them.
# $formats lists the format names, e.g. "ERDAS IMAGINE, ADSC SMV, Analyze 7.5".
error-reason-not-one-of = den är ingen av de .{ $ext }-filer som { -app-name } läser ({ $formats }).
error-reason-damaged = filen är skadad eller är inte vad namnet säger ({ $detail }).
error-reason-too-large = den är för stor för att öppnas.
# machine-drafted, needs native review
error-reason-needs-memory = den behöver { $needed } ledigt RAM, men just nu är bara { $free } ledigt. Stäng några andra appar och försök igen, eller öppna den på en dator med mer minne.
error-needs-memory = Den här bilden behöver { $needed } ledigt RAM, men just nu är bara { $free } ledigt. Stäng några andra appar och försök igen, eller öppna den på en dator med mer minne.
error-needs-memory-short = Behöver { $needed } ledigt RAM
error-reason-cancelled = öppnandet avbröts.
error-reason-trash-access = den finns inte där längre, eller så har du inte behörighet att ändra den.
error-reason-restore-collision = det finns redan en fil med samma namn i mappen.
error-reason-trash-root = en hel enhet kan inte flyttas till papperskorgen.
error-reason-not-in-trash = den finns inte längre i papperskorgen.
error-reason-program-missing = programmet är inte installerat eller har flyttats. Välj det igen eller installera om det.
error-reason-program-not-allowed = programmet får inte köras. Kontrollera att det är körbart.
# machine-drafted, needs native review
error-reason-permission-write = du har inte behörighet att ändra den. Kontrollera ägare och behörigheter.
error-reason-file-has-errors = den innehåller fel. Åtgärda dem först så att inget i den går förlorat.
error-reason-not-dicom = det är inte en DICOM-fil.
error-reason-no-image = den innehåller ingen bild.
error-reason-needs-layout = det är en volym utan huvud. Dess storlek, sampeltyp och byteordning behövs för att öppna den.
error-reason-not-archive = det är inte ett arkiv som { -app-name } kan läsa.
error-reason-needs-sandbox = den här typen av arkiv öppnas bara av { -app-name }s skyddade läsare, som inte är tillgänglig.
error-reason-encrypted = den är krypterad och behöver sitt lösenord.
error-reason-wrong-password = lösenordet är fel.
error-reason-checksum = filen är skadad: innehållet stämmer inte med kontrollsumman.
error-reason-missing-entry = en fil i den saknas.
error-reason-busy = ett annat program använder den. Stäng den där och försök igen.
error-reason-catalogue-damaged = katalogfilen är skadad.
error-reason-cannot-open = dess fil kan inte öppnas. Kontrollera att mappen finns och att du kan skriva till den.
error-reason-newer-version = den skrevs av en nyare version av { -app-name }. Uppdatera { -app-name } för att ändra den.
error-reason-internal = något gick fel inuti { -app-name }. Loggen har detaljerna.
error-reason-volume-too-few = { $count ->
    [one] en snittbild är för lite för att bygga en volym.
   *[other] { $count } snittbilder är för få för att bygga en volym.
}
error-reason-volume-no-geometry = en snittbild anger inte var den ligger eller åt vilket håll den är vänd.
error-reason-volume-not-parallel = snittbilderna är inte parallella.
error-reason-volume-shared-position = flera bilder delar var och en av { $count ->
        [one] { $count } position
       *[other] { $count } positioner
    } (faser, ekon eller en tidsserie).
error-reason-volume-missing = { $count ->
        [one] ungefär { $count } snittbild saknas.
       *[other] ungefär { $count } snittbilder saknas.
    }
error-reason-volume-mismatch = snittbilderna skiljer sig i storlek eller pixelavstånd.
error-reason-volume-not-grey = snittbilderna är i färg, inte gråvärden.
error-reason-volume-skewed = volymens axlar är inte vinkelräta.
error-reason-volume-too-large = den behöver { $need } MiB och endast { $budget } MiB är tillåtet.
error-reason-url-not-specular = det är inte en { -app-name }-länk.
error-reason-url-too-long = den är för lång.
error-reason-url-damaged = den är skadad. Den kan ha kortats av eller ändrats när den kopierades.
error-reason-url-unknown = den begär något som den här versionen av { -app-name } inte känner till. Det kan hjälpa att uppdatera { -app-name }.
error-reason-url-not-absolute = dess filsökvägar måste vara fullständiga sökvägar.
error-reason-url-no-paths = den anger ingen fil.
error-reason-url-too-many = den anger för många filer.

## Aviseringar

# machine-drafted, needs native review
# A background task (a listing, an export, a file operation) stopped part way; $reason is
# error-reason-internal.
background-failed = Det gick inte att slutföra: { $reason }
toast-moved-to-trash = Flyttad till papperskorgen
toast-moved-to-trash-named = { $file } flyttades till papperskorgen
toast-moved-to-trash-count =
    { $count ->
        [one] { $count } bild flyttades till papperskorgen
       *[other] { $count } bilder flyttades till papperskorgen
    }
toast-undo = Ångra
toast-restored = { $file } återställdes
toast-copied-image = Bilden kopierades
toast-copied-path = Sökvägen kopierades
toast-copied-report = Rapporten kopierades
toast-copied-value = Värdet kopierades
toast-loop-start = Tillbaka till första bilden
toast-loop-end = Hoppade till sista bilden
# machine-drafted, needs native review
toast-interface-size = Gränssnittets storlek { PERCENT($percent) }
toast-language-changed = Språket ändrat till { $language }

## Tomma lägen

empty-drop = Släpp en bild här eller tryck på { $shortcut }
empty-open-button = Öppna bild…
empty-drop-release = Släpp för att öppna
empty-folder = Mappen innehåller inga bilder som { -app-name } kan öppna.
# machine-drafted, needs native review
# Under "Öppna bild…" i det tomma fönstret: var allt annat finns.
empty-hint-keys = Tryck på { $cheatsheet } för kortkommandon eller på { $palette } för att hitta ett kommando.
empty-hint-cheatsheet = Tryck på { $cheatsheet } för kortkommandon.
empty-hint-palette = Tryck på { $palette } för att hitta ett kommando.
# Under snurran när en fil tar mer än ett ögonblick att öppna.
viewer-opening = Öppnar { $file }…
fullscreen-exit-hint = Tryck på Esc för att avsluta helskärm
# Visas när helskärm aktiveras; Esc återställer först zoomen om bilden är inzoomad.
fullscreen-back-hint = Tryck på Esc för att återställa zoomen eller avsluta helskärm
# Lathund och kommandopalett: vad Esc gör, ett steg per tryck.
action-back = Stäng panelen (i en dialogruta först ett steg tillbaka), återställ zoom, lämna helskärm; i galleriet en mapp upp

## Språkväljare

language-picker-title = Språk
# machine-drafted, needs native review
language-picker-search = Sök språk
# $percent är en redan formaterad procentsats, t.ex. "87 %".
language-completeness = { $percent } översatt
language-system-default = Systemets standard ({ $language })
language-pseudo-accented = Pseudospråk: med accenter och längre
language-pseudo-rtl = Pseudospråk: speglat höger till vänster

## Om

about-title = Om { -app-name }
about-version = Version { $version }
about-tagline = En snabb och trogen bildvisare.
about-copyright = © { $year } Upphovspersonerna bakom { -app-name }
about-license = Licensierad under MIT License / Apache License 2.0.
about-geonames = Ortnamn från GeoNames (geonames.org), licensierade under Creative Commons Erkännande 4.0.
about-unicode = Språkdata från Unicode CLDR via ICU4X. © Unicode, Inc.
# machine-drafted, needs native review
about-lensfun = Objektivprofiler från Lensfun-databasen (lensfun.github.io), licensierade under Creative Commons Attribution-ShareAlike 3.0.
about-third-party = Licenser för tredje part
about-third-party-file = { -app-name } innehåller programvara och data från tredje part under egna licenser; se THIRD-PARTY.md, som installeras med { -app-name }.
about-copy-info = Kopiera systeminformation
about-close = Stäng

## Visarens gränssnitt (steg 1)

app-close = Stäng
# Märke på en bild vars fil tog slut för tidigt.
app-truncated-badge = Ofullständig fil
app-info-summary = Sammanfattning
app-info-all-fields = Alla fält
app-info-copy-value = Kopiera värde
app-info-copy-field = Kopiera värdet för { $field }
app-info-loading = Läser metadata…
# machine-drafted, needs native review
# Alla fält, för en fil som inte lagrar några metadata utöver sina pixlar.
app-info-no-fields = Den här filen har inga metadatafält.
# Gain-map row of the info summary (machine-drafted, needs native review)
app-info-gain-map = HDR-förstärkningskarta
app-info-gain-map-headroom = Upp till { $headroom }× SDR-ljusstyrka
app-copy-failed = Det gick inte att kopiera: { $reason }
app-restore-failed = Det gick inte att återställa { $file }: { $reason }
app-open-dialog-title = Öppna bild
app-open-dialog-filter = Filer som stöds
app-open-dialog-all = Alla filer
app-open-group-images = Bilder
app-open-group-raw = Kamera-RAW
app-open-group-video = Video
app-open-group-sound = Ljud
app-open-group-documents = Dokument
app-open-group-drawings = Ritningar och CAD
app-open-group-models = 3D-modeller
app-open-group-maps = Kartor och spår
app-open-group-science = Vetenskap och medicin
app-open-group-fonts = Typsnitt
app-open-group-other = Övrigt

## Renderarinformation (Om, Inställningar › Avancerat)

# machine-drafted, needs native review
renderer-title = Renderarinformation
renderer-section-adapter = Grafikadapter
renderer-section-limits = Gränser och funktioner
renderer-section-display = Skärm
renderer-section-memory = Minne
renderer-section-decoders = Valfria avkodare
renderer-unknown = Okänd
# The PCI vendor and device IDs ("1002:747e").
renderer-ids = Leverantörs- och enhets-ID
# Wayland, X11, Windows or macOS.
renderer-display-server = Skärmserver
renderer-features = Använda funktioner
renderer-surface-format = Fönsterformat
renderer-surface-formats = Erbjudna format
renderer-present-mode = Presentationsläge
renderer-present-modes = Erbjudna lägen
renderer-hdr = HDR
# $headroom is how much brighter than white the display goes ("4.0").
renderer-hdr-on = På ({ $headroom }× headroom)
renderer-hdr-off = Av (skärmen kan visa HDR)
renderer-hdr-unavailable = Inte tillgängligt på den här skärmen
renderer-decode-budget = Avkodning får använda
renderer-decode-budget-detail = { PERCENT(80) } av det lediga minnet, delat av alla bilder som öppnas.
renderer-display-cache = Visningscache

## Prestanda-HUD (F12)

app-hud-scale = Skala
app-hud-level = Nivå
app-hud-level-thumbnail = Inbäddad miniatyr
app-hud-level-full = Full upplösning
# $denominator är 2, 4 eller 8: bilden avkodad i 1/2, 1/4 eller 1/8 storlek.
app-hud-level-scaled = 1/{ $denominator }
app-hud-open = Öppna
app-hud-decode = Avkoda
app-hud-upload = Överföring
app-hud-frame = Bildruta
app-hud-threads = Trådar
app-hud-threads-value = { $threads } ({ $queued } i kö)
app-hud-tiles = Rutor
app-hud-tiles-value = { $tiles } av { $capacity }
app-hud-ram = Minne
app-hud-vram = GPU-minne
app-hud-gpu = GPU

# Bekräftelse för papperskorgen
app-confirm-trash-title = Flytta { $file } till papperskorgen?
# machine-drafted, needs native review
app-confirm-trash-sidecars = Sidofilerna hamnar också i papperskorgen: { $files }
app-cancel = Avbryt
# Varning innan en mycket stor bild avkodas. $need och $available är minnesstorlekar ("3,2 GiB").
toast-low-memory = Bilden kan behöva cirka { $need } minne, men bara { $available } är ledigt. Den kan läsas in långsamt eller misslyckas.

# Maskinvarurapporten (Inställningar → Avancerat). Värdena fylls i av appen.
hw-report-title = Maskinvarurapport
hw-report-copy = Kopiera rapport
hw-report-version = Version
hw-report-cpu = CPU
hw-report-cores = Kärnor
hw-report-cores-value = { $logical } logiska, { $physical } fysiska
hw-report-performance-cores = Prestandakärnor
hw-report-cpu-features = Snabba CPU-vägar
hw-report-none = Inga
hw-report-memory = Minne
hw-report-memory-value = { $total } totalt, { $available } ledigt
hw-report-graphics-api = Grafik-API
hw-report-driver = Drivrutin
hw-report-max-texture = Största textur
hw-report-pixels = { $width } × { $height } px
hw-report-decode-threads = Avkodningstrådar
hw-report-ram-cache = Bildcache i minnet
hw-report-vram-cache = Cache i GPU-minnet
hw-report-processing = Bearbetning
hw-report-path-jpeg-decode = JPEG-avkodning
hw-report-path-ycbcr-to-rgb = Färgkonvertering
hw-report-path-png-unfilter = PNG-avkodning
hw-report-path-heic-decode = HEIC-avkodning
hw-report-path-inference = Bildanalys
hw-report-engine-cpu = CPU
hw-report-engine-cpu-parallel = CPU, parallellt
hw-report-engine-gpu = GPU
hw-report-engine-hw-decoder = maskinvaruavkodare
hw-report-engine-npu = NPU
hw-report-build-debug = felsökningsversion
hw-report-build-release = releaseversion

## Tillgänglighet (skärmläsare)
# Bilden som en skärmläsare presenterar den: filnamn och storlek i pixlar.
a11y-image = { $name }, { $width } × { $height } pixlar
# Området där korta aviseringar visas.
a11y-notifications = Aviseringar
# machine-drafted, needs native review
a11y-transport-playback = Uppspelning
a11y-transport-pages = Sidor
a11y-settings-sections = Avsnitt

## Read codes (QR codes in the picture) (machine-drafted, needs native review)
codes-placeholder = Koder i den här bilden
codes-looking = Letar efter koder…
codes-none = Inga koder hittades i den här bilden.
codes-failed = Det gick inte att läsa bilden: { $reason }
codes-copy = Kopiera ”{ $text }”
# $kind is "QR", $version a number.
codes-kind = { $kind } { $version }
# Data Matrix: its size in modules, e.g. "Data Matrix 12×12".
# machine-drafted, needs native review
codes-kind-size = { $kind } { $rows }×{ $cols }
codes-open-row = Öppna länk på { $host }…
codes-network-off = Inget nätverk är på, så länkar öppnas inte.
codes-open-title = Öppna en länk till { $host }?
codes-open-detail = Den öppnas i din webbläsare. Öppna bara länkar du litar på.
codes-open = Öppna i webbläsare
# machine-drafted, needs native review
# Find text: the text in the picture shown, read by the Local AI pack's Text models.
ocr-placeholder = Text i den här bilden
# machine-drafted, needs native review
ocr-looking = Läser texten…
# machine-drafted, needs native review
ocr-none = Ingen text hittades i den här bilden.
# machine-drafted, needs native review
ocr-failed = Det gick inte att läsa texten: { $reason }
# machine-drafted, needs native review
ocr-needs-pack = Hitta text kräver textmodellerna i det lokala AI-paketet (Inställningar › Lokal AI).
# machine-drafted, needs native review
ocr-loading = Läser in textmodellerna…
# machine-drafted, needs native review
# A row: choosing it copies the line. $text is the line (one line, shortened).
ocr-copy = Kopiera ”{ $text }”
# machine-drafted, needs native review
ocr-copy-all = Kopiera all text
# machine-drafted, needs native review
# A row's note: how sure the reading is, and the add-on that read it. $percent is formatted.
ocr-confidence = { $percent } säker

## Hardware report panel (machine-drafted, needs native review)
hw-report-section-system = System
hw-report-section-gpu = GPU
hw-report-section-decoding = Avkodning
hw-report-operating-system = Operativsystem
hw-report-adapter = Adapter
hw-report-gpu-type = Typ
hw-report-gpu-init = Starttid
hw-report-max-layers = Texturlager
hw-report-compression = Texturkomprimering
hw-report-unified-memory = Minne delat med CPU:n
hw-report-display = Skärm
hw-report-hw-decode = Maskinvaruavkodning
hw-report-core-cluster = Kärngrupp
hw-report-power = Ström
hw-report-calibration = Hastighetsmätningar
hw-report-codec-others = Övriga
hw-report-performance-cores-value = { $count } av { $total }
hw-report-cluster-value = { $name }: { $physical } fysiska, { $logical } logiska
hw-report-thermal-nominal = Normal temperatur
hw-report-thermal-fair = Varm
hw-report-thermal-serious = Het, sänker takten
hw-report-thermal-critical = Överhettad
hw-report-power-battery = På batteri ({ $percent })
hw-report-power-battery-unknown = På batteri
hw-report-power-mains = Ansluten till ström
hw-report-power-mains-percent = Ansluten till ström ({ $percent })
hw-report-power-saving = Energisparläge på
hw-report-gpu-discrete = Dedikerad
hw-report-gpu-integrated = Integrerad
hw-report-gpu-virtual = Virtuell
hw-report-gpu-software = Programvara, på CPU:n
hw-report-gpu-other = Annan
hw-report-gpu-unavailable = Inte tillgänglig
hw-report-ms = { $ms } ms
hw-report-yes = Ja
hw-report-no = Nej
hw-report-unified-platform-differs = Systemet rapporterar något annat
hw-report-display-value = { $fps } Hz, HDR-marginal { $now } nu, upp till { $max }
hw-report-hw-decode-value = { $api } på { $device }
hw-report-hw-decode-checking = Kontrollerar fortfarande…
hw-report-codec-up-to = Upp till { $width } × { $height }
hw-report-codec-none = Inte tillgänglig
hw-report-codec-h264-encode = H.264-kodning
hw-report-supported = stöds: { $list }
hw-report-unsupported = stöds inte: { $list }
hw-report-calibration-done = Uppmätt på den här datorn
hw-report-calibration-pending = Inte uppmätt än
hw-report-paths-power-saving = Energisparläge: vägar som drar mindre energi föredras
hw-report-paths-caption = Vägar för en bild på { $mpx } megapixel
hw-report-col-task = Uppgift
hw-report-col-path = Väg
hw-report-col-time = Tid
hw-report-col-cpu = CPU-tid (alla trådar)
hw-report-col-runs = Mätningar
hw-report-col-use = Används för
hw-report-runs-untested = Inte testad
hw-report-use-viewing = Visning ({ $reason })
hw-report-use-background = Bakgrund ({ $reason })
hw-report-reason-only = enda alternativet
hw-report-reason-faster = snabbast
hw-report-reason-cheaper = minst CPU-tid
hw-report-reason-lower-power = minst energi
hw-report-reason-explore = provas
hw-report-reason-forced = inställt i Inställningar
hw-report-reason-prefer-gpu = Föredra GPU
hw-report-reason-cpu-only = Endast CPU
hw-report-reason-calibrate = mäts
hw-report-reason-scaled = förminskad
hw-report-trust-unverified = Inte kontrollerad än
hw-report-trust-blocked = Avstängd efter ett fel
hw-report-trust-absent = Finns inte på den här datorn

## Keyboard cheat sheet tabs and sections (machine-drafted, needs native review)
cheatsheet-mode-gallery = Galleri
cheatsheet-mode-compare = Jämför
cheatsheet-gallery-browse = Bläddra
cheatsheet-gallery-select = Markera
cheatsheet-gallery-mark = Betygsätt och flagga
cheatsheet-gallery-move = Flytta mellan bilder
cheatsheet-gallery-page = Sida upp eller ner
cheatsheet-gallery-ends = Första eller sista bilden
cheatsheet-gallery-up = Upp till överordnad mapp
cheatsheet-gallery-history-back = Tillbaka till föregående mapp
cheatsheet-gallery-history-forward = Framåt till nästa mapp
cheatsheet-gallery-back = Stäng vyn överst och gå sedan tillbaka
cheatsheet-gallery-viewer = Tillbaka till bildvisaren
cheatsheet-gallery-album-add = Lägg till i senaste albumet
cheatsheet-gallery-select-one = Markera eller avmarkera
cheatsheet-gallery-quick-look = Snabbtitt
cheatsheet-gallery-filter = Filtrera
cheatsheet-gallery-bigger = Större miniatyrer
cheatsheet-gallery-smaller = Mindre miniatyrer
cheatsheet-gallery-default-size = Standardstorlek för miniatyrer
cheatsheet-gallery-refresh = Uppdatera mappen
cheatsheet-gallery-menu = Snabbmeny
cheatsheet-compare-views = Jämför
cheatsheet-compare-overlay = Överlagring eller sida vid sida
cheatsheet-compare-difference = Skillnad (igen: nästa typ)
cheatsheet-compare-step-down = Mindre blandning eller skillnadsförstärkning
cheatsheet-compare-step-up = Mer blandning eller skillnadsförstärkning
cheatsheet-compare-pick = Bilden som tangenterna påverkar
cheatsheet-compare-close = Stäng jämförelsen
cheatsheet-channels = Kanaler
cheatsheet-medical-window = Fönster och nivå
cheatsheet-medical-measure = Mät och kommentera
cheatsheet-medical-frames = Bildrutor och serier
# machine-drafted, needs native review
cheatsheet-medical-3d = 3D och rekonstruktion
cheatsheet-medical-analysis = Segmentering, fusion och PET
# machine-drafted, needs native review
cheatsheet-medical-studies = Undersökningar och delning
cheatsheet-palette-only-title = I kommandopaletten ({ $key })
cheatsheet-palette-only-title-nokey = I kommandopaletten
cheatsheet-palette-only-tag = Palett
cheatsheet-palette-only-line = { $action }: ingen tangent; i kommandopaletten ({ $key })
cheatsheet-palette-only-line-nokey = { $action }: ingen tangent; i kommandopaletten
# Esc i kortkommandoöversikten (palettens långa beskrivning får inte plats).
cheatsheet-back = Stäng eller gå tillbaka

## Hardware video encoding (machine-drafted, needs native review)
hw-report-path-video-encode = Videoexport
hw-report-engine-hw-encoder = maskinvarukodare

## Path timing units (machine-drafted, needs native review)
hw-report-task-thumbnail = { $task } (per miniatyr på { $size })
hw-report-task-frame = { $task } (per bildruta på { $size })

## Hardware report, software-only formats (machine-drafted, needs native review)
hw-report-codec-software = Endast programvara

about-natural-earth = Kartdata från Natural Earth (naturalearthdata.com), allmän egendom.
app-info-show-on-map = Visa på karta
