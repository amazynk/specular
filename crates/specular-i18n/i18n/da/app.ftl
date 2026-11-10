# machine-drafted, needs native review
# Specular — billedfremviserens brugerflade, kommandopalet, fejlmeddelelser, Om. Dansk.

-app-name = Specular

## Værktøjslinjens værktøjstip og tilgængelighedsetiketter

toolbar-open = Åbn billede…
toolbar-prev = Forrige billede
toolbar-next = Næste billede
toolbar-back-gallery = Tilbage til galleriet
toolbar-zoom-out = Zoom ud
toolbar-zoom-in = Zoom ind
toolbar-zoom-level = Zoomniveau og forvalg
toolbar-zoom-fit = Tilpas til vinduet
toolbar-zoom-100 = Faktisk størrelse ({ PERCENT(100) })
toolbar-rotate-left = Roter mod uret
toolbar-rotate-right = Roter med uret
toolbar-fullscreen-enter = Fuld skærm
toolbar-fullscreen-exit = Afslut fuld skærm
toolbar-info = Billedoplysninger
toolbar-settings = Indstillinger
toolbar-delete = Flyt til papirkurven
toolbar-copy = Kopiér billede
toolbar-palette = Kommandopalet
toolbar-go-to = Gå til billede…
toolbar-go-to-range = 1–{ $count }
# machine-drafted, needs native review
toolbar-customise = Tilpas værktøjslinjen…
toolbar-more = Mere
toolbar-announce-moved = { $item }: position { $index } af { $count }
toolbar-announce-added = { $item } tilføjet: position { $index } af { $count }
toolbar-announce-removed = { $item } fjernet fra værktøjslinjen
toolbar-announce-reset = Værktøjslinjen er nulstillet
# Et værktøjstip efterfulgt af tastaturgenvejen.
tooltip-with-shortcut = { $label } ({ $shortcut })
# Værktøjstip for et element, der ikke kan bruges lige nu; $reason er en af disabled-*-meddelelserne.
tooltip-unavailable = { $label } (ikke tilgængelig: { $reason })
# Hvorfor et element eller en kommando ikke er tilgængelig (værktøjstip, kommandopalet, skærmlæsere).
disabled-no-image = Intet billede åbent
disabled-still-loading = Billedet indlæses stadig
disabled-decode-failed = Billedet kunne ikke afkodes
disabled-no-other-images = Ingen andre billeder i mappen

## Visninger

# Placering i mappen, f.eks. "12 / 348".
readout-position = { $index } / { $count }
# Læses op af skærmlæsere for placeringen.
readout-position-a11y = Billede { $index } af { $count }
# $zoom er en allerede formateret procentsats, f.eks. "150 %".
readout-zoom = { $zoom }
readout-zoom-a11y = Zoom { $zoom }
readout-zoom-fit = Tilpas ({ $zoom })
zoom-preset-fit = Tilpas til vinduet
zoom-preset-fill = Udfyld vinduet
zoom-preset-actual = Faktisk størrelse
# machine-drafted, needs native review
# The last entry of the zoom presets: the window fills the screen (ticked while it does).
zoom-preset-fullscreen = Fuld skærm
# $zoom er en allerede formateret procentsats.
zoom-preset-percent = { $zoom }
# Billedets mål i pixel, f.eks. "6000 × 4000".
readout-dimensions = { $width } × { $height }
# Antal billeder i den aktuelle mappe.
status-image-count =
    { $count ->
        [one] { $count } billede
       *[other] { $count } billeder
    }

## Kommandopalet

palette-placeholder = Skriv en kommando eller indstilling…
palette-no-results = Ingen matchende kommandoer
# Ændring af en indstilling fra paletten, f.eks. "Angiv: Musehjul → Næste billede".
palette-set-setting = Angiv: { $setting } → { $value }
palette-next = Næste billede
palette-prev = Forrige billede
palette-first = Første billede
palette-last = Sidste billede
palette-go-to = Gå til billede…
palette-zoom-in = Zoom ind
palette-zoom-out = Zoom ud
palette-zoom-fit = Tilpas til vinduet
palette-zoom-fill = Udfyld vinduet
# machine-drafted, needs native review
palette-zoom-fit-width = Tilpas til bredden
palette-zoom-fit-height = Tilpas til højden
palette-zoom-100 = Zoom til { PERCENT(100) }
palette-zoom-200 = Zoom til { PERCENT(200) }
palette-zoom-300 = Zoom til { PERCENT(300) }
palette-zoom-400 = Zoom til { PERCENT(400) }
palette-zoom-rectangle = Zoom til markering
palette-rotate-left = Roter mod uret
palette-rotate-right = Roter med uret
palette-flip-horizontal = Spejlvend vandret
palette-flip-vertical = Spejlvend lodret
palette-toggle-fullscreen = Slå fuld skærm til/fra
palette-navigator = Vis eller skjul navigator
palette-loupe = Lup
palette-pixel-grid = Vis eller skjul pixelgitter
palette-info-panel = Vis eller skjul oplysningspanel
palette-copy-image = Kopiér billede
# machine-drafted, needs native review
palette-read-codes = Læs koder
# machine-drafted, needs native review
palette-find-text = Find tekst
# machine-drafted, needs native review
palette-face-boxes = Vis ansigter
palette-copy-path = Kopiér filsti
palette-move-to-trash = Flyt til papirkurven
palette-undo-trash = Fortryd flytning til papirkurven
palette-open = Åbn billede…
# machine-drafted, needs native review
palette-open-address = Åbn adresse…
palette-settings = Indstillinger
# machine-drafted, needs native review
palette-customise-toolbar = Tilpas værktøjslinjen…
palette-cheat-sheet = Tastaturgenveje
palette-hud = Vis eller skjul billedtids-HUD
palette-language = Sprog
palette-theme = Tema
# machine-drafted, needs native review
palette-interface-larger = Større brugerflade
palette-interface-smaller = Mindre brugerflade
palette-interface-reset = Brugerflade på { PERCENT(100) }
palette-quit = Afslut { -app-name }

## Oversigt over genveje

cheatsheet-title = Tastaturgenveje
cheatsheet-navigation = Navigation
cheatsheet-zoom-pan = Zoom og panorering
cheatsheet-view = Vis
cheatsheet-file = Fil
cheatsheet-window = Vindue
cheatsheet-help = Hjælp
cheatsheet-hold = Hold { $key } nede
cheatsheet-close-hint = Tryk på { $key } eller Esc for at lukke

## Fejl

# machine-drafted, needs native review
viewer-no-preview = Ingen forhåndsvisning i filen.
error-cannot-open = Kan ikke åbne { $file }
error-cannot-open-reason = Kan ikke åbne { $file }: { $reason }
error-unsupported-format = { $file } er i et format, som { -app-name } endnu ikke kan læse.
error-file-truncated = { $file } er ufuldstændig. Den del, der kunne læses, vises.
error-file-too-large = { $file } er for stor til at blive åbnet ({ $megapixels } megapixel; grænsen er { $limit } megapixel).
error-not-found = { $file } findes ikke længere.
error-decoder-stopped = afkoderen stoppede uden at levere et billede
# machine-drafted, needs native review
# Shown after "Can’t open FILE:" for brain data (GIFTI .func/.shape/.label, CIFTI) that is
# drawn on a surface: no .surf.gii with that many vertices was found in the same folder.
error-needs-surface = den kræver en hjerneoverflade (en .surf.gii-fil med { $vertices } hjørnepunkter) i samme mappe
error-permission-denied = Du har ikke tilladelse til at læse { $file }.
error-details = Detaljer
crash-last-time = { -app-name } lukkede uventet sidste gang; en rapport blev gemt i { $path }
error-trash-failed = { $file } kunne ikke flyttes til papirkurven: { $reason }
# machine-drafted, needs native review
# Årsager, vist efter "Kan ikke åbne FIL:" og andre fejlmeddelelser, så de begynder med
# lille bogstav. De siger, hvad der skete, og hvor det hjælper, hvad man kan gøre.
error-reason-not-found = den findes ikke længere. Den kan være flyttet, omdøbt eller slettet.
error-reason-permission = du har ikke tilladelse til at læse den. Kontrollér ejeren og tilladelserne.
error-reason-disk-full = disken er fuld. Frigør plads, og prøv igen.
error-reason-read-only = disken er skrivebeskyttet.
error-reason-truncated = filen er ufuldstændig. Den er måske stadig ved at blive kopieret eller downloadet.
error-reason-is-folder = det er en mappe, ikke en fil.
# A remote store (a Zarr image on a server or in an S3 bucket).
error-reason-remote-denied = serveren nægtede adgang. Dataene kan være private; for en S3-bucket skal du først konfigurere AWS-loginoplysninger.
error-reason-remote-missing = der er ingen Zarr-data på den adresse.
# $status is an HTTP status code ("500").
error-reason-remote-status = serveren svarede med fejl { $status }.
error-reason-offline = serveren kan ikke nås. Kontrollér adressen og forbindelsen.
# machine-drafted, needs native review
error-reason-network-off = netværket er slået fra i Indstillinger › Privatliv.
error-reason-timed-out = drevet eller serveren svarede ikke i tide. Kontrollér forbindelsen, og prøv igen.
error-reason-memory = der er ikke nok hukommelse. Luk andre billeder eller programmer, og prøv igen.
error-reason-unsupported = den er i et format, som { -app-name } endnu ikke kan læse.
# $detail er dekoderens egne ord, tekniske og på engelsk ("12-bit lossless JPEG").
error-reason-unsupported-detail = den bruger en funktion, som { -app-name } endnu ikke kan læse ({ $detail }).
# machine-drafted, needs native review
# A file whose extension several formats share (".img") and whose content is none of them.
# $formats lists the format names, e.g. "ERDAS IMAGINE, ADSC SMV, Analyze 7.5".
error-reason-not-one-of = den er ingen af de .{ $ext }-filer, som { -app-name } læser ({ $formats }).
error-reason-damaged = filen er beskadiget eller er ikke det, navnet siger ({ $detail }).
error-reason-too-large = den er for stor til at blive åbnet.
# machine-drafted, needs native review
error-reason-needs-memory = den kræver { $needed } fri RAM, men kun { $free } er fri lige nu. Luk nogle andre programmer, og prøv igen, eller åbn den på en computer med mere hukommelse.
error-needs-memory = Dette billede kræver { $needed } fri RAM, men kun { $free } er fri lige nu. Luk nogle andre programmer, og prøv igen, eller åbn det på en computer med mere hukommelse.
error-needs-memory-short = Kræver { $needed } fri RAM
error-reason-cancelled = åbningen blev stoppet.
error-reason-trash-access = den findes ikke længere, eller du har ikke tilladelse til at ændre den.
error-reason-restore-collision = der findes allerede en fil med samme navn i mappen.
error-reason-trash-root = et helt drev kan ikke flyttes til papirkurven.
error-reason-not-in-trash = den er ikke længere i papirkurven.
error-reason-program-missing = programmet er ikke installeret eller er blevet flyttet. Vælg det igen, eller geninstaller det.
error-reason-program-not-allowed = programmet har ikke tilladelse til at køre. Kontrollér, at det er eksekverbart.
# machine-drafted, needs native review
error-reason-permission-write = du har ikke tilladelse til at ændre den. Kontrollér ejeren og tilladelserne.
error-reason-file-has-errors = den indeholder fejl. Ret dem først, så intet i den går tabt.
error-reason-not-dicom = den er ikke en DICOM-fil.
error-reason-no-image = den indeholder intet billede.
error-reason-needs-layout = den er en volumen uden header. Dens størrelse, sampletype og byterækkefølge skal angives for at åbne den.
error-reason-not-archive = den er ikke et arkiv, som { -app-name } kan læse.
error-reason-needs-sandbox = denne type arkiv åbnes kun af { -app-name }s beskyttede læser, som ikke er tilgængelig.
error-reason-encrypted = den er krypteret og kræver sin adgangskode.
error-reason-wrong-password = adgangskoden er forkert.
error-reason-checksum = filen er beskadiget: indholdet passer ikke med kontrolsummen.
error-reason-missing-entry = en fil i den mangler.
error-reason-busy = et andet program bruger den. Luk den der, og prøv igen.
error-reason-catalogue-damaged = katalogfilen er beskadiget.
error-reason-cannot-open = dens fil kan ikke åbnes. Kontrollér, at mappen findes, og at du kan skrive i den.
error-reason-newer-version = den er skrevet af en nyere version af { -app-name }. Opdater { -app-name } for at ændre den.
error-reason-internal = noget gik galt i { -app-name }. Loggen har detaljerne.
error-reason-volume-too-few = { $count ->
    [one] { $count } snit er for få til at opbygge en volumen.
   *[other] { $count } snit er for få til at opbygge en volumen.
}
error-reason-volume-no-geometry = et snit angiver ikke, hvor det ligger, eller hvilken vej det vender.
error-reason-volume-not-parallel = snittene er ikke parallelle.
error-reason-volume-shared-position = flere billeder deler hver af { $count ->
        [one] { $count } position
       *[other] { $count } positioner
    } (faser, ekkoer eller en tidsserie).
error-reason-volume-missing = { $count ->
        [one] omkring { $count } snit mangler.
       *[other] omkring { $count } snit mangler.
    }
error-reason-volume-mismatch = snittene har forskellig størrelse eller pixelafstand.
error-reason-volume-not-grey = snittene er i farver, ikke gråtoner.
error-reason-volume-skewed = volumenens akser står ikke vinkelret på hinanden.
error-reason-volume-too-large = den kræver { $need } MiB, og kun { $budget } MiB er tilladt.
error-reason-url-not-specular = det er ikke et { -app-name }-link.
error-reason-url-too-long = det er for langt.
error-reason-url-damaged = det er beskadiget. Det kan være blevet afkortet eller ændret, da det blev kopieret.
error-reason-url-unknown = det beder om noget, som denne version af { -app-name } ikke kender. Det kan hjælpe at opdatere { -app-name }.
error-reason-url-not-absolute = dets filstier skal være fulde stier.
error-reason-url-no-paths = det angiver ingen fil.
error-reason-url-too-many = det angiver for mange filer.

## Meddelelser

# machine-drafted, needs native review
# A background task (a listing, an export, a file operation) stopped part way; $reason is
# error-reason-internal.
background-failed = Kunne ikke fuldføre: { $reason }
toast-moved-to-trash = Flyttet til papirkurven
toast-moved-to-trash-named = { $file } blev flyttet til papirkurven
toast-moved-to-trash-count =
    { $count ->
        [one] { $count } billede flyttet til papirkurven
       *[other] { $count } billeder flyttet til papirkurven
    }
toast-undo = Fortryd
toast-restored = { $file } er gendannet
toast-copied-image = Billede kopieret
toast-copied-path = Sti kopieret
toast-copied-report = Rapport kopieret
toast-copied-value = Værdi kopieret
toast-loop-start = Tilbage til første billede
toast-loop-end = Sprang til sidste billede
# machine-drafted, needs native review
toast-interface-size = Brugerfladens størrelse { PERCENT($percent) }
toast-language-changed = Sproget er ændret til { $language }

## Tomme tilstande

empty-drop = Slip et billede her, eller tryk på { $shortcut }
empty-open-button = Åbn billede…
empty-drop-release = Slip for at åbne
empty-folder = Mappen indeholder ingen billeder, som { -app-name } kan åbne.
# machine-drafted, needs native review
# Under "Åbn billede…" i det tomme vindue: hvor alt andet findes.
empty-hint-keys = Tryk på { $cheatsheet } for at se tastaturgenveje, eller på { $palette } for at finde en kommando.
empty-hint-cheatsheet = Tryk på { $cheatsheet } for at se tastaturgenveje.
empty-hint-palette = Tryk på { $palette } for at finde en kommando.
# Under ventesymbolet, når en fil er mere end et øjeblik om at åbne.
viewer-opening = Åbner { $file }…
fullscreen-exit-hint = Tryk på Esc for at afslutte fuld skærm
# Vises ved skift til fuld skærm; Esc nulstiller først zoomet, hvis der er zoomet ind.
fullscreen-back-hint = Tryk på Esc for at nulstille zoom eller afslutte fuld skærm
# Genvejsoversigt og kommandopalet: hvad Esc gør, ét trin pr. tryk.
action-back = Luk panelet (i en dialogboks først et trin tilbage), nulstil zoom, afslut fuld skærm; i galleriet én mappe op

## Sprogvælger

language-picker-title = Sprog
# machine-drafted, needs native review
language-picker-search = Søg efter sprog
# $percent er en allerede formateret procentsats, f.eks. "87 %".
language-completeness = { $percent } oversat
language-system-default = Systemstandard ({ $language })
language-pseudo-accented = Pseudosprog: accenter og længere tekst
language-pseudo-rtl = Pseudosprog: spejlvendt højre mod venstre

## Om

about-title = Om { -app-name }
about-version = Version { $version }
about-tagline = En hurtig og tro billedfremviser.
about-copyright = © { $year } { -app-name }-forfatterne
about-license = Udgivet under MIT License / Apache License 2.0.
about-geonames = Stednavne fra GeoNames (geonames.org), udgivet under Creative Commons Attribution 4.0.
about-unicode = Landestandarddata fra Unicode CLDR via ICU4X. © Unicode, Inc.
# machine-drafted, needs native review
about-lensfun = Objektivprofiler fra Lensfun-databasen (lensfun.github.io), licenseret under Creative Commons Attribution-ShareAlike 3.0.
about-third-party = Tredjepartslicenser
about-third-party-file = { -app-name } indeholder software og data fra tredjeparter under deres egne licenser; se THIRD-PARTY.md, som installeres sammen med { -app-name }.
about-copy-info = Kopiér systemoplysninger
about-close = Luk

## Fremviserens brugerflade (Stage 1-appen)

app-close = Luk
# Mærke på et billede, hvis fil sluttede for tidligt.
app-truncated-badge = Ufuldstændig fil
app-info-summary = Oversigt
app-info-all-fields = Alle felter
app-info-copy-value = Kopiér værdi
app-info-copy-field = Kopiér værdien af { $field }
app-info-loading = Læser metadata…
# machine-drafted, needs native review
# Alle felter, for en fil, der ikke gemmer andre metadata end sine pixels.
app-info-no-fields = Filen har ingen metadatafelter.
# Gain-map row of the info summary (machine-drafted, needs native review)
app-info-gain-map = HDR-gainmap
app-info-gain-map-headroom = Op til { $headroom }× SDR-lysstyrke
app-copy-failed = Kunne ikke kopiere: { $reason }
app-restore-failed = { $file } kunne ikke gendannes: { $reason }
app-open-dialog-title = Åbn billede
app-open-dialog-filter = Understøttede filer
app-open-dialog-all = Alle filer
app-open-group-images = Billeder
app-open-group-raw = Kamera-RAW
app-open-group-video = Video
app-open-group-sound = Lyd
app-open-group-documents = Dokumenter
app-open-group-drawings = Tegninger og CAD
app-open-group-models = 3D-modeller
app-open-group-maps = Kort og spor
app-open-group-science = Videnskab og medicin
app-open-group-fonts = Skrifttyper
app-open-group-other = Andet

## Rendererinformation (Om, Indstillinger › Avanceret)

# machine-drafted, needs native review
renderer-title = Rendererinformation
renderer-section-adapter = Grafikadapter
renderer-section-limits = Grænser og funktioner
renderer-section-display = Skærm
renderer-section-memory = Hukommelse
renderer-section-decoders = Valgfrie dekodere
renderer-unknown = Ukendt
# The PCI vendor and device IDs ("1002:747e").
renderer-ids = Leverandør- og enheds-id
# Wayland, X11, Windows or macOS.
renderer-display-server = Skærmserver
renderer-features = Anvendte funktioner
renderer-surface-format = Vinduesformat
renderer-surface-formats = Tilbudte formater
renderer-present-mode = Præsentationstilstand
renderer-present-modes = Tilbudte tilstande
renderer-hdr = HDR
# $headroom is how much brighter than white the display goes ("4.0").
renderer-hdr-on = Til ({ $headroom }× headroom)
renderer-hdr-off = Fra (skærmen kan vise HDR)
renderer-hdr-unavailable = Ikke tilgængelig på denne skærm
renderer-decode-budget = Afkodning må bruge
renderer-decode-budget-detail = { PERCENT(80) } af den ledige hukommelse, delt af alle billeder, der er ved at blive åbnet.
renderer-display-cache = Visningscache

## Ydelses-HUD (F12)

app-hud-scale = Skala
app-hud-level = Niveau
app-hud-level-thumbnail = Indlejret miniature
app-hud-level-full = Fuld opløsning
# $denominator er 2, 4 eller 8: billedet afkodet i 1/2, 1/4 eller 1/8 størrelse.
app-hud-level-scaled = 1/{ $denominator }
app-hud-open = Åbning
app-hud-decode = Afkodning
app-hud-upload = Overførsel
app-hud-frame = Billede
app-hud-threads = Tråde
app-hud-threads-value = { $threads } ({ $queued } i kø)
app-hud-tiles = Fliser
app-hud-tiles-value = { $tiles } af { $capacity }
app-hud-ram = Hukommelse
app-hud-vram = GPU-hukommelse
app-hud-gpu = GPU

# Bekræftelse af flytning til papirkurven
app-confirm-trash-title = Vil du flytte { $file } til papirkurven?
# machine-drafted, needs native review
app-confirm-trash-sidecars = Sidefilerne kommer også i papirkurven: { $files }
app-cancel = Annuller
# En advarsel, før et meget stort billede afkodes. $need og $available er
# hukommelsesstørrelser ("3,2 GiB").
toast-low-memory = Billedet kan kræve omkring { $need } hukommelse, men kun { $available } er ledig. Det kan blive indlæst langsomt eller slet ikke.

# Hardwarerapporten (Indstillinger → Avanceret). Værdierne udfyldes af appen.
hw-report-title = Hardwarerapport
hw-report-copy = Kopiér rapport
hw-report-version = Version
hw-report-cpu = CPU
hw-report-cores = Kerner
hw-report-cores-value = { $logical } logiske, { $physical } fysiske
hw-report-performance-cores = Ydelseskerner
hw-report-cpu-features = Hurtige CPU-veje
hw-report-none = Ingen
hw-report-memory = Hukommelse
hw-report-memory-value = { $total } i alt, { $available } ledig
hw-report-graphics-api = Grafik-API
hw-report-driver = Driver
hw-report-max-texture = Største tekstur
hw-report-pixels = { $width } × { $height } px
hw-report-decode-threads = Afkodningstråde
hw-report-ram-cache = Billedcache i hukommelsen
hw-report-vram-cache = Cache i GPU-hukommelsen
hw-report-processing = Behandling
hw-report-path-jpeg-decode = JPEG-afkodning
hw-report-path-ycbcr-to-rgb = Farvekonvertering
hw-report-path-png-unfilter = PNG-afkodning
hw-report-path-heic-decode = HEIC-afkodning
hw-report-path-inference = Billedanalyse
hw-report-engine-cpu = CPU
hw-report-engine-cpu-parallel = CPU, parallelt
hw-report-engine-gpu = GPU
hw-report-engine-hw-decoder = hardwareafkoder
hw-report-engine-npu = NPU
hw-report-build-debug = fejlfindingsbuild
hw-report-build-release = udgivelsesbuild

## Tilgængelighed (skærmlæsere)
# Billedet, som en skærmlæser annoncerer det: filnavn og størrelse i pixel.
a11y-image = { $name }, { $width } × { $height } pixel
# Området, hvor korte meddelelser vises.
a11y-notifications = Meddelelser
# machine-drafted, needs native review
a11y-transport-playback = Afspilning
a11y-transport-pages = Sider
a11y-settings-sections = Afsnit

## Read codes (QR codes in the picture) (machine-drafted, needs native review)
codes-placeholder = Koder i dette billede
codes-looking = Leder efter koder…
codes-none = Der blev ikke fundet koder i dette billede.
codes-failed = Billedet kunne ikke læses: { $reason }
codes-copy = Kopiér “{ $text }”
# $kind is "QR", $version a number.
codes-kind = { $kind } { $version }
# Data Matrix: its size in modules, e.g. "Data Matrix 12×12".
# machine-drafted, needs native review
codes-kind-size = { $kind } { $rows }×{ $cols }
codes-open-row = Åbn link på { $host }…
codes-network-off = Intet netværk er slået til, så links åbnes ikke.
codes-open-title = Åbn et link til { $host }?
codes-open-detail = Det åbnes i din webbrowser. Åbn kun links, du stoler på.
codes-open = Åbn i browser
# machine-drafted, needs native review
# Find text: the text in the picture shown, read by the Local AI pack's Text models.
ocr-placeholder = Tekst i dette billede
# machine-drafted, needs native review
ocr-looking = Læser teksten…
# machine-drafted, needs native review
ocr-none = Der blev ikke fundet tekst i dette billede.
# machine-drafted, needs native review
ocr-failed = Teksten kunne ikke læses: { $reason }
# machine-drafted, needs native review
ocr-needs-pack = Find tekst kræver tekstmodellerne fra Lokal AI-pakken (Indstillinger › Lokal AI).
# machine-drafted, needs native review
ocr-loading = Indlæser tekstmodellerne…
# machine-drafted, needs native review
# A row: choosing it copies the line. $text is the line (one line, shortened).
ocr-copy = Kopiér “{ $text }”
# machine-drafted, needs native review
ocr-copy-all = Kopiér al tekst
# machine-drafted, needs native review
# A row's note: how sure the reading is, and the add-on that read it. $percent is formatted.
ocr-confidence = { $percent } sikker

## Hardware report panel (machine-drafted, needs native review)
hw-report-section-system = System
hw-report-section-gpu = GPU
hw-report-section-decoding = Afkodning
hw-report-operating-system = Operativsystem
hw-report-adapter = Adapter
hw-report-gpu-type = Type
hw-report-gpu-init = Opstartstid
hw-report-max-layers = Teksturlag
hw-report-compression = Teksturkomprimering
hw-report-unified-memory = Hukommelse delt med CPU'en
hw-report-display = Skærm
hw-report-hw-decode = Hardwareafkodning
hw-report-core-cluster = Kernegruppe
hw-report-power = Strøm
hw-report-calibration = Hastighedsmålinger
hw-report-codec-others = Andre
hw-report-performance-cores-value = { $count } af { $total }
hw-report-cluster-value = { $name }: { $physical } fysiske, { $logical } logiske
hw-report-thermal-nominal = Normal temperatur
hw-report-thermal-fair = Varm
hw-report-thermal-serious = Meget varm, sænker farten
hw-report-thermal-critical = Overophedet
hw-report-power-battery = På batteri ({ $percent })
hw-report-power-battery-unknown = På batteri
hw-report-power-mains = Tilsluttet strøm
hw-report-power-mains-percent = Tilsluttet strøm ({ $percent })
hw-report-power-saving = Strømbesparelse slået til
hw-report-gpu-discrete = Dedikeret
hw-report-gpu-integrated = Integreret
hw-report-gpu-virtual = Virtuel
hw-report-gpu-software = Software, på CPU'en
hw-report-gpu-other = Anden
hw-report-gpu-unavailable = Ikke tilgængelig
hw-report-ms = { $ms } ms
hw-report-yes = Ja
hw-report-no = Nej
hw-report-unified-platform-differs = Systemet oplyser noget andet
hw-report-display-value = { $fps } Hz, HDR-råderum { $now } nu, op til { $max }
hw-report-hw-decode-value = { $api } på { $device }
hw-report-hw-decode-checking = Kontrollerer stadig…
hw-report-codec-up-to = Op til { $width } × { $height }
hw-report-codec-none = Ikke tilgængelig
hw-report-codec-h264-encode = H.264-kodning
hw-report-supported = understøttet: { $list }
hw-report-unsupported = ikke understøttet: { $list }
hw-report-calibration-done = Målt på denne computer
hw-report-calibration-pending = Endnu ikke målt
hw-report-paths-power-saving = Strømbesparelse: veje, der bruger mindre energi, foretrækkes
hw-report-paths-caption = Veje for et billede på { $mpx } megapixel
hw-report-col-task = Opgave
hw-report-col-path = Vej
hw-report-col-time = Tid
hw-report-col-cpu = CPU-tid (alle tråde)
hw-report-col-runs = Målinger
hw-report-col-use = Bruges til
hw-report-runs-untested = Ikke testet
hw-report-use-viewing = Visning ({ $reason })
hw-report-use-background = Baggrund ({ $reason })
hw-report-reason-only = eneste mulighed
hw-report-reason-faster = hurtigst
hw-report-reason-cheaper = mindst CPU-tid
hw-report-reason-lower-power = mindst energi
hw-report-reason-explore = afprøves
hw-report-reason-forced = valgt i Indstillinger
hw-report-reason-prefer-gpu = Foretræk GPU
hw-report-reason-cpu-only = Kun CPU
hw-report-reason-calibrate = måles
hw-report-reason-scaled = formindsket
hw-report-trust-unverified = Endnu ikke kontrolleret
hw-report-trust-blocked = Slået fra efter en fejl
hw-report-trust-absent = Findes ikke på denne computer

## Keyboard cheat sheet tabs and sections (machine-drafted, needs native review)
cheatsheet-mode-gallery = Galleri
cheatsheet-mode-compare = Sammenlign
cheatsheet-gallery-browse = Gennemse
cheatsheet-gallery-select = Vælg
cheatsheet-gallery-mark = Bedøm og flag
cheatsheet-gallery-move = Flyt mellem billeder
cheatsheet-gallery-page = Side op eller ned
cheatsheet-gallery-ends = Første eller sidste billede
cheatsheet-gallery-up = Op til den overordnede mappe
cheatsheet-gallery-history-back = Tilbage til den forrige mappe
cheatsheet-gallery-history-forward = Frem til den næste mappe
cheatsheet-gallery-back = Luk den øverste visning, og gå så tilbage
cheatsheet-gallery-viewer = Tilbage til fremviseren
cheatsheet-gallery-album-add = Føj til det seneste album
cheatsheet-gallery-select-one = Vælg eller fravælg
cheatsheet-gallery-quick-look = Hurtigt kig
cheatsheet-gallery-filter = Filtrer
cheatsheet-gallery-bigger = Større miniaturer
cheatsheet-gallery-smaller = Mindre miniaturer
cheatsheet-gallery-default-size = Standardstørrelse for miniaturer
cheatsheet-gallery-refresh = Opdater mappen
cheatsheet-gallery-menu = Genvejsmenu
cheatsheet-compare-views = Sammenlign
cheatsheet-compare-overlay = Overlejring eller side om side
cheatsheet-compare-difference = Forskel (igen: næste slags)
cheatsheet-compare-step-down = Mindre blanding eller forskelsforstærkning
cheatsheet-compare-step-up = Mere blanding eller forskelsforstærkning
cheatsheet-compare-pick = Billedet, tasterne virker på
cheatsheet-compare-close = Luk sammenligning
cheatsheet-channels = Kanaler
cheatsheet-medical-window = Vindue og niveau
cheatsheet-medical-measure = Mål og annotér
cheatsheet-medical-frames = Billeder og serier
# machine-drafted, needs native review
cheatsheet-medical-3d = 3D og reformatering
cheatsheet-medical-analysis = Segmentering, fusion og PET
# machine-drafted, needs native review
cheatsheet-medical-studies = Undersøgelser og deling
cheatsheet-palette-only-title = I kommandopaletten ({ $key })
cheatsheet-palette-only-title-nokey = I kommandopaletten
cheatsheet-palette-only-tag = Palet
cheatsheet-palette-only-line = { $action }: ingen tast; i kommandopaletten ({ $key })
cheatsheet-palette-only-line-nokey = { $action }: ingen tast; i kommandopaletten
# Esc i genvejsoversigten (palettens lange beskrivelse passer ikke).
cheatsheet-back = Luk eller gå tilbage

## Hardware video encoding (machine-drafted, needs native review)
hw-report-path-video-encode = Videoeksport
hw-report-engine-hw-encoder = hardwarekoder

## Path timing units (machine-drafted, needs native review)
hw-report-task-thumbnail = { $task } (pr. miniature på { $size })
hw-report-task-frame = { $task } (pr. billede på { $size })

## Hardware report, software-only formats (machine-drafted, needs native review)
hw-report-codec-software = Kun software

about-natural-earth = Kortdata fra Natural Earth (naturalearthdata.com), offentligt domæne.
app-info-show-on-map = Vis på kort
