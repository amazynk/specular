# machine-drafted, needs native review
# Specular — weergavevenster, opdrachtenpalet, foutmeldingen, Over. Nederlands (informeel: je/jouw).

-app-name = Specular

## Knopinfo en toegankelijkheidslabels van de werkbalk

toolbar-open = Afbeelding openen…
toolbar-prev = Vorige afbeelding
toolbar-next = Volgende afbeelding
toolbar-back-gallery = Terug naar de galerij
toolbar-zoom-out = Uitzoomen
toolbar-zoom-in = Inzoomen
toolbar-zoom-level = Zoomniveau en voorinstellingen
toolbar-zoom-fit = Passend in venster
toolbar-zoom-100 = Werkelijke grootte ({ PERCENT(100) })
toolbar-rotate-left = Linksom draaien
toolbar-rotate-right = Rechtsom draaien
toolbar-fullscreen-enter = Volledig scherm
toolbar-fullscreen-exit = Volledig scherm sluiten
toolbar-info = Afbeeldingsinformatie
toolbar-settings = Instellingen
toolbar-delete = Naar prullenbak verplaatsen
toolbar-copy = Afbeelding kopiëren
toolbar-palette = Opdrachtenpalet
toolbar-go-to = Ga naar afbeelding…
toolbar-go-to-range = 1–{ $count }
# machine-drafted, needs native review
toolbar-customise = Werkbalk aanpassen…
toolbar-more = Meer
toolbar-announce-moved = { $item }: positie { $index } van { $count }
toolbar-announce-added = { $item } toegevoegd: positie { $index } van { $count }
toolbar-announce-removed = { $item } van de werkbalk verwijderd
toolbar-announce-reset = Werkbalk teruggezet naar standaard
# Een knopinfolabel gevolgd door de sneltoets.
tooltip-with-shortcut = { $label } ({ $shortcut })
# Knopinfo van een besturingselement dat nu niet beschikbaar is; $reason is een van de disabled-*-berichten.
tooltip-unavailable = { $label } (niet beschikbaar: { $reason })
# Waarom een besturingselement of opdracht niet beschikbaar is.
disabled-no-image = Geen afbeelding geopend
disabled-still-loading = De afbeelding wordt nog geladen
disabled-decode-failed = Deze afbeelding kan niet worden gedecodeerd
disabled-no-other-images = Geen andere afbeeldingen in deze map

## Uitlezingen

# Positie in de map, bijv. "12 / 348".
readout-position = { $index } / { $count }
# Uitgesproken door schermlezers voor de positie.
readout-position-a11y = Afbeelding { $index } van { $count }
# $zoom is een al opgemaakt percentage, bijv. "150%".
readout-zoom = { $zoom }
readout-zoom-a11y = Zoom { $zoom }
readout-zoom-fit = Passend ({ $zoom })
zoom-preset-fit = Passend in venster
zoom-preset-fill = Venster vullen
zoom-preset-actual = Werkelijke grootte
# machine-drafted, needs native review
# The last entry of the zoom presets: the window fills the screen (ticked while it does).
zoom-preset-fullscreen = Volledig scherm
# $zoom is een al opgemaakt percentage.
zoom-preset-percent = { $zoom }
# Afmetingen in pixels, bijv. "6000 × 4000".
readout-dimensions = { $width } × { $height }
# Aantal afbeeldingen in de huidige map.
status-image-count =
    { $count ->
        [one] { $count } afbeelding
       *[other] { $count } afbeeldingen
    }

## Opdrachtenpalet

palette-placeholder = Typ een opdracht of instelling…
palette-no-results = Geen overeenkomende opdrachten
# Een instelling wijzigen vanuit het palet, bijv. "Instellen: Muiswiel → Volgende afbeelding".
palette-set-setting = Instellen: { $setting } → { $value }
palette-next = Volgende afbeelding
palette-prev = Vorige afbeelding
palette-first = Eerste afbeelding
palette-last = Laatste afbeelding
palette-go-to = Ga naar afbeelding…
palette-zoom-in = Inzoomen
palette-zoom-out = Uitzoomen
palette-zoom-fit = Passend zoomen
palette-zoom-fill = Zoomen tot vullend
# machine-drafted, needs native review
palette-zoom-fit-width = Passend in breedte
palette-zoom-fit-height = Passend in hoogte
palette-zoom-100 = Zoomen naar { PERCENT(100) }
palette-zoom-200 = Zoomen naar { PERCENT(200) }
palette-zoom-300 = Zoomen naar { PERCENT(300) }
palette-zoom-400 = Zoomen naar { PERCENT(400) }
palette-zoom-rectangle = Zoomen naar selectie
palette-rotate-left = Linksom draaien
palette-rotate-right = Rechtsom draaien
palette-flip-horizontal = Horizontaal spiegelen
palette-flip-vertical = Verticaal spiegelen
palette-toggle-fullscreen = Volledig scherm aan/uit
palette-navigator = Navigator tonen of verbergen
palette-loupe = Loep
palette-pixel-grid = Pixelraster tonen of verbergen
palette-info-panel = Informatiepaneel tonen of verbergen
palette-copy-image = Afbeelding kopiëren
# machine-drafted, needs native review
palette-read-codes = Codes lezen
# machine-drafted, needs native review
palette-find-text = Tekst zoeken
# machine-drafted, needs native review
palette-face-boxes = Gezichten tonen
palette-copy-path = Bestandspad kopiëren
palette-move-to-trash = Naar prullenbak verplaatsen
palette-undo-trash = Verplaatsen naar prullenbak ongedaan maken
palette-open = Afbeelding openen…
# machine-drafted, needs native review
palette-open-address = Adres openen…
palette-settings = Instellingen
# machine-drafted, needs native review
palette-customise-toolbar = Werkbalk aanpassen…
palette-cheat-sheet = Sneltoetsen
palette-hud = Frametijd-HUD tonen of verbergen
palette-language = Taal
palette-theme = Thema
# machine-drafted, needs native review
palette-interface-larger = Grotere interface
palette-interface-smaller = Kleinere interface
palette-interface-reset = Interface op { PERCENT(100) }
palette-quit = { -app-name } afsluiten

## Overzicht van sneltoetsen

cheatsheet-title = Sneltoetsen
cheatsheet-navigation = Navigatie
cheatsheet-zoom-pan = Zoomen en verschuiven
cheatsheet-view = Weergave
cheatsheet-file = Bestand
cheatsheet-window = Venster
cheatsheet-help = Help
cheatsheet-hold = { $key } ingedrukt houden
cheatsheet-close-hint = Druk op { $key } of Esc om te sluiten

## Foutmeldingen

# machine-drafted, needs native review
viewer-no-preview = Geen voorbeeld in dit bestand.
error-cannot-open = Kan { $file } niet openen
error-cannot-open-reason = Kan { $file } niet openen: { $reason }
error-unsupported-format = { $file } heeft een indeling die { -app-name } nog niet kan lezen.
error-file-truncated = { $file } is onvolledig. Het leesbare deel wordt getoond.
error-file-too-large = { $file } is te groot om te openen ({ $megapixels } megapixel; de limiet is { $limit } megapixel).
error-not-found = { $file } bestaat niet meer.
error-decoder-stopped = de decoder is gestopt zonder een afbeelding te maken
# machine-drafted, needs native review
# Shown after "Can’t open FILE:" for brain data (GIFTI .func/.shape/.label, CIFTI) that is
# drawn on a surface: no .surf.gii with that many vertices was found in the same folder.
error-needs-surface = er is een hersenoppervlak nodig (een .surf.gii-bestand met { $vertices } hoekpunten) in dezelfde map
error-permission-denied = Je hebt geen toestemming om { $file } te lezen.
error-details = Details
crash-last-time = { -app-name } is de vorige keer onverwacht afgesloten; er is een rapport opgeslagen in { $path }
error-trash-failed = Kan { $file } niet naar de prullenbak verplaatsen: { $reason }
# machine-drafted, needs native review
# Redenen, getoond na "Kan FILE niet openen:" en andere "Kan … niet …:"-meldingen, dus ze
# beginnen met een kleine letter. Ze zeggen wat er gebeurde en, waar het helpt, wat je kunt doen.
error-reason-not-found = het is er niet meer. Het is misschien verplaatst, hernoemd of verwijderd.
error-reason-permission = je hebt geen toestemming om het te lezen. Controleer de eigenaar en de rechten.
error-reason-disk-full = de schijf is vol. Maak ruimte vrij en probeer het opnieuw.
error-reason-read-only = de schijf is alleen-lezen.
error-reason-truncated = het bestand is onvolledig. Misschien wordt het nog gekopieerd of gedownload.
error-reason-is-folder = het is een map, geen bestand.
# A remote store (a Zarr image on a server or in an S3 bucket).
error-reason-remote-denied = de server weigerde toegang. De gegevens zijn misschien privé; stel voor een S3-bucket eerst AWS-inloggegevens in.
error-reason-remote-missing = er zijn geen Zarr-gegevens op dat adres.
# $status is an HTTP status code ("500").
error-reason-remote-status = de server antwoordde met fout { $status }.
error-reason-offline = de server is niet bereikbaar. Controleer het adres en de verbinding.
# machine-drafted, needs native review
error-reason-network-off = het netwerk staat uit in Instellingen › Privacy.
error-reason-timed-out = het station of de server antwoordde niet op tijd. Controleer de verbinding en probeer het opnieuw.
error-reason-memory = er is niet genoeg geheugen. Sluit andere afbeeldingen of programma's en probeer het opnieuw.
error-reason-unsupported = het heeft een indeling die { -app-name } nog niet kan lezen.
# $detail zijn de eigen woorden van de decoder, technisch en in het Engels ("12-bit lossless JPEG").
error-reason-unsupported-detail = het gebruikt een functie die { -app-name } nog niet kan lezen ({ $detail }).
# machine-drafted, needs native review
# A file whose extension several formats share (".img") and whose content is none of them.
# $formats lists the format names, e.g. "ERDAS IMAGINE, ADSC SMV, Analyze 7.5".
error-reason-not-one-of = het is geen van de .{ $ext }-bestanden die { -app-name } leest ({ $formats }).
error-reason-damaged = het bestand is beschadigd of is niet wat de naam zegt ({ $detail }).
error-reason-too-large = het is te groot om te openen.
# machine-drafted, needs native review
error-reason-needs-memory = het heeft { $needed } vrij RAM nodig, maar er is nu maar { $free } vrij. Sluit een paar andere apps en probeer het opnieuw, of open het op een computer met meer geheugen.
error-needs-memory = Deze afbeelding heeft { $needed } vrij RAM nodig, maar er is nu maar { $free } vrij. Sluit een paar andere apps en probeer het opnieuw, of open de afbeelding op een computer met meer geheugen.
error-needs-memory-short = Heeft { $needed } vrij RAM nodig
error-reason-cancelled = het openen is gestopt.
error-reason-trash-access = het is er niet meer, of je hebt geen toestemming om het te wijzigen.
error-reason-restore-collision = er staat al een bestand met dezelfde naam in de map.
error-reason-trash-root = een volledig station kan niet naar de prullenbak worden verplaatst.
error-reason-not-in-trash = het staat niet meer in de prullenbak.
error-reason-program-missing = het programma is niet geïnstalleerd of is verplaatst. Kies het opnieuw of installeer het opnieuw.
error-reason-program-not-allowed = het programma mag niet worden uitgevoerd. Controleer of het uitvoerbaar is.
# machine-drafted, needs native review
error-reason-permission-write = je hebt geen toestemming om het te wijzigen. Controleer de eigenaar en de rechten.
error-reason-file-has-errors = het bevat fouten. Herstel die eerst, zodat er niets in verloren gaat.
error-reason-not-dicom = het is geen DICOM-bestand.
error-reason-no-image = het bevat geen afbeelding.
error-reason-needs-layout = het is een volume zonder header. De afmetingen, het sampletype en de bytevolgorde zijn nodig om het te openen.
error-reason-not-archive = het is geen archief dat { -app-name } kan lezen.
error-reason-needs-sandbox = dit soort archief wordt alleen geopend door de beveiligde lezer van { -app-name }, en die is niet beschikbaar.
error-reason-encrypted = het is versleuteld en heeft het wachtwoord nodig.
error-reason-wrong-password = het wachtwoord is onjuist.
error-reason-checksum = het bestand is beschadigd: de inhoud komt niet overeen met de controlesom.
error-reason-missing-entry = er ontbreekt een bestand in.
error-reason-busy = een ander programma gebruikt het. Sluit het daar en probeer het opnieuw.
error-reason-catalogue-damaged = het catalogusbestand is beschadigd.
error-reason-cannot-open = het bestand kan niet worden geopend. Controleer of de map bestaat en of je erin kunt schrijven.
error-reason-newer-version = het is geschreven door een nieuwere versie van { -app-name }. Werk { -app-name } bij om het te wijzigen.
error-reason-internal = er is iets misgegaan in { -app-name }. Het logboek bevat de details.
error-reason-volume-too-few = { $count ->
    [one] één plak is te weinig om een volume op te bouwen.
   *[other] { $count } plakken zijn te weinig om een volume op te bouwen.
}
error-reason-volume-no-geometry = een plak geeft niet aan waar hij ligt of welke kant hij op wijst.
error-reason-volume-not-parallel = de plakken zijn niet evenwijdig.
error-reason-volume-shared-position = meerdere afbeeldingen delen { $count ->
        [one] { $count } positie
       *[other] elk van de { $count } posities
    } (fasen, echo’s of een tijdreeks).
error-reason-volume-missing = { $count ->
        [one] er ontbreekt ongeveer { $count } plak.
       *[other] er ontbreken ongeveer { $count } plakken.
    }
error-reason-volume-mismatch = de plakken verschillen in grootte of pixelafstand.
error-reason-volume-not-grey = de plakken zijn in kleur, niet in grijswaarden.
error-reason-volume-skewed = de assen van het volume staan niet haaks op elkaar.
error-reason-volume-too-large = het heeft { $need } MiB nodig en er is maar { $budget } MiB toegestaan.
error-reason-url-not-specular = het is geen { -app-name }-link.
error-reason-url-too-long = hij is te lang.
error-reason-url-damaged = hij is beschadigd. Misschien is hij bij het kopiëren afgekapt of gewijzigd.
error-reason-url-unknown = hij vraagt om iets wat deze versie van { -app-name } niet kent. { -app-name } bijwerken kan helpen.
error-reason-url-not-absolute = de bestandspaden moeten volledige paden zijn.
error-reason-url-no-paths = hij noemt geen bestand.
error-reason-url-too-many = hij noemt te veel bestanden.

## Meldingen

# machine-drafted, needs native review
# A background task (a listing, an export, a file operation) stopped part way; $reason is
# error-reason-internal.
background-failed = Kon niet voltooien: { $reason }
toast-moved-to-trash = Naar prullenbak verplaatst
toast-moved-to-trash-named = { $file } naar prullenbak verplaatst
toast-moved-to-trash-count =
    { $count ->
        [one] { $count } afbeelding naar prullenbak verplaatst
       *[other] { $count } afbeeldingen naar prullenbak verplaatst
    }
toast-undo = Ongedaan maken
toast-restored = { $file } teruggezet
toast-copied-image = Afbeelding gekopieerd
toast-copied-path = Pad gekopieerd
toast-copied-report = Rapport gekopieerd
toast-copied-value = Waarde gekopieerd
toast-loop-start = Terug naar de eerste afbeelding
toast-loop-end = Naar de laatste afbeelding
# machine-drafted, needs native review
toast-interface-size = Interfacegrootte { PERCENT($percent) }
toast-language-changed = Taal gewijzigd in { $language }

## Lege toestanden

empty-drop = Sleep hier een afbeelding naartoe of druk op { $shortcut }
empty-open-button = Afbeelding openen…
empty-drop-release = Loslaten om te openen
empty-folder = Deze map bevat geen afbeeldingen die { -app-name } kan openen.
# machine-drafted, needs native review
# Onder "Afbeelding openen…" in het lege venster: waar al het andere te vinden is.
empty-hint-keys = Druk op { $cheatsheet } voor sneltoetsen of op { $palette } om een opdracht te zoeken.
empty-hint-cheatsheet = Druk op { $cheatsheet } voor sneltoetsen.
empty-hint-palette = Druk op { $palette } om een opdracht te zoeken.
# Onder de laadindicator wanneer het openen van een bestand meer dan een moment duurt.
viewer-opening = { $file } openen…
fullscreen-exit-hint = Druk op Esc om volledig scherm te sluiten
# Getoond bij het openen van volledig scherm; Esc zet eerst de zoom terug als er is ingezoomd.
fullscreen-back-hint = Druk op Esc om de zoom terug te zetten of volledig scherm te sluiten
# Sneltoetsenoverzicht en opdrachtenpalet: wat Esc doet, één stap per druk.
action-back = Paneel sluiten (in een dialoogvenster eerst een stap terug), zoom herstellen, volledig scherm verlaten; in de galerij een map omhoog

## Taalkeuze

language-picker-title = Taal
# machine-drafted, needs native review
language-picker-search = Talen zoeken
# $percent is een al opgemaakt percentage, bijv. "87%".
language-completeness = { $percent } vertaald
language-system-default = Systeemstandaard ({ $language })
language-pseudo-accented = Pseudotaal: met accenten en langer
language-pseudo-rtl = Pseudotaal: gespiegeld, van rechts naar links

## Over

about-title = Over { -app-name }
about-version = Versie { $version }
about-tagline = Een snelle, getrouwe afbeeldingsviewer.
about-copyright = © { $year } De auteurs van { -app-name }
about-license = Gelicentieerd onder de MIT License / Apache License 2.0.
about-geonames = Plaatsnamen van GeoNames (geonames.org), gelicentieerd onder Creative Commons Naamsvermelding 4.0.
about-unicode = Taal- en regiogegevens uit de Unicode CLDR via ICU4X. © Unicode, Inc.
# machine-drafted, needs native review
about-lensfun = Lensprofielen uit de Lensfun-database (lensfun.github.io), gelicentieerd onder Creative Commons Attribution-ShareAlike 3.0.
about-third-party = Licenties van derden
about-third-party-file = { -app-name } bevat software en gegevens van derden onder hun eigen licenties; zie THIRD-PARTY.md, dat met { -app-name } wordt geïnstalleerd.
about-copy-info = Systeeminformatie kopiëren
about-close = Sluiten

## Weergavevenster (Stage 1-app)

app-close = Sluiten
# Badge op een afbeelding waarvan het bestand te vroeg eindigde.
app-truncated-badge = Onvolledig bestand
app-info-summary = Overzicht
app-info-all-fields = Alle velden
app-info-copy-value = Waarde kopiëren
app-info-copy-field = Waarde van { $field } kopiëren
app-info-loading = Metagegevens lezen…
# machine-drafted, needs native review
# Alle velden, voor een bestand dat naast zijn pixels geen metagegevens bevat.
app-info-no-fields = Dit bestand heeft geen metagegevensvelden.
# Gain-map row of the info summary (machine-drafted, needs native review)
app-info-gain-map = HDR-gainmap
app-info-gain-map-headroom = Tot { $headroom }× SDR-helderheid
app-copy-failed = Kopiëren mislukt: { $reason }
app-restore-failed = Kan { $file } niet terugzetten: { $reason }
app-open-dialog-title = Afbeelding openen
app-open-dialog-filter = Ondersteunde bestanden
app-open-dialog-all = Alle bestanden
app-open-group-images = Afbeeldingen
app-open-group-raw = Camera-RAW
app-open-group-video = Video
app-open-group-sound = Geluid
app-open-group-documents = Documenten
app-open-group-drawings = Tekeningen en CAD
app-open-group-models = 3D-modellen
app-open-group-maps = Kaarten en routes
app-open-group-science = Wetenschap en medisch
app-open-group-fonts = Lettertypen
app-open-group-other = Overig

## Rendererinformatie (Over, Instellingen › Geavanceerd)

# machine-drafted, needs native review
renderer-title = Rendererinformatie
renderer-section-adapter = Grafische adapter
renderer-section-limits = Limieten en functies
renderer-section-display = Beeldscherm
renderer-section-memory = Geheugen
renderer-section-decoders = Optionele decoders
renderer-unknown = Onbekend
# The PCI vendor and device IDs ("1002:747e").
renderer-ids = Leveranciers- en apparaat-ID
# Wayland, X11, Windows or macOS.
renderer-display-server = Displayserver
renderer-features = Gebruikte functies
renderer-surface-format = Vensterformaat
renderer-surface-formats = Aangeboden formaten
renderer-present-mode = Presentatiemodus
renderer-present-modes = Aangeboden modi
renderer-hdr = HDR
# $headroom is how much brighter than white the display goes ("4.0").
renderer-hdr-on = Aan ({ $headroom }× ruimte boven wit)
renderer-hdr-off = Uit (het beeldscherm kan HDR tonen)
renderer-hdr-unavailable = Niet beschikbaar op dit beeldscherm
renderer-decode-budget = Decoderen mag gebruiken
renderer-decode-budget-detail = { PERCENT(80) } van het vrije geheugen, gedeeld door alle afbeeldingen die worden geopend.
renderer-display-cache = Weergavecache

## Prestatie-HUD (F12)

app-hud-scale = Schaal
app-hud-level = Niveau
app-hud-level-thumbnail = Ingesloten miniatuur
app-hud-level-full = Volledige resolutie
# $denominator is 2, 4 of 8: de afbeelding gedecodeerd op 1/2, 1/4 of 1/8 van de grootte.
app-hud-level-scaled = 1/{ $denominator }
app-hud-open = Openen
app-hud-decode = Decoderen
app-hud-upload = Uploaden
app-hud-frame = Frame
app-hud-threads = Threads
app-hud-threads-value = { $threads } ({ $queued } in wachtrij)
app-hud-tiles = Tegels
app-hud-tiles-value = { $tiles } van { $capacity }
app-hud-ram = Geheugen
app-hud-vram = GPU-geheugen
app-hud-gpu = GPU

# Bevestiging prullenbak
app-confirm-trash-title = { $file } naar de prullenbak verplaatsen?
# machine-drafted, needs native review
app-confirm-trash-sidecars = De sidecarbestanden gaan ook naar de prullenbak: { $files }
app-cancel = Annuleren
# Waarschuwing voordat een zeer grote afbeelding wordt gedecodeerd. $need en $available zijn
# geheugengroottes ("3,2 GiB").
toast-low-memory = Deze afbeelding heeft mogelijk ongeveer { $need } geheugen nodig, maar er is maar { $available } beschikbaar. Laden kan traag gaan of mislukken.

# Het hardwarerapport (Instellingen → Geavanceerd). De waarden vult de app in.
hw-report-title = Hardwarerapport
hw-report-copy = Rapport kopiëren
hw-report-version = Versie
hw-report-cpu = CPU
hw-report-cores = Kernen
hw-report-cores-value = { $logical } logisch, { $physical } fysiek
hw-report-performance-cores = Prestatiekernen
hw-report-cpu-features = Snelle CPU-paden
hw-report-none = Geen
hw-report-memory = Geheugen
hw-report-memory-value = { $total } totaal, { $available } beschikbaar
hw-report-graphics-api = Grafische API
hw-report-driver = Stuurprogramma
hw-report-max-texture = Grootste textuur
hw-report-pixels = { $width } × { $height } px
hw-report-decode-threads = Decodeerthreads
hw-report-ram-cache = Afbeeldingscache in geheugen
hw-report-vram-cache = Cache in GPU-geheugen
hw-report-processing = Verwerking
hw-report-path-jpeg-decode = JPEG decoderen
hw-report-path-ycbcr-to-rgb = Kleurconversie
hw-report-path-png-unfilter = PNG decoderen
hw-report-path-heic-decode = HEIC decoderen
hw-report-path-inference = Beeldanalyse
hw-report-engine-cpu = CPU
hw-report-engine-cpu-parallel = CPU, parallel
hw-report-engine-gpu = GPU
hw-report-engine-hw-decoder = hardwaredecoder
hw-report-engine-npu = NPU
hw-report-build-debug = debugbuild
hw-report-build-release = releasebuild

## Toegankelijkheid (schermlezers)
# De afbeelding zoals een schermlezer die aankondigt: bestandsnaam en grootte in pixels.
a11y-image = { $name }, { $width } × { $height } pixels
# Het gebied waar korte meldingen verschijnen.
a11y-notifications = Meldingen
# machine-drafted, needs native review
a11y-transport-playback = Afspelen
a11y-transport-pages = Pagina’s
a11y-settings-sections = Secties

## Read codes (QR codes in the picture) (machine-drafted, needs native review)
codes-placeholder = Codes in deze afbeelding
codes-looking = Codes zoeken…
codes-none = Geen codes gevonden in deze afbeelding.
codes-failed = Kan de afbeelding niet lezen: { $reason }
codes-copy = ‘{ $text }’ kopiëren
# $kind is "QR", $version a number.
codes-kind = { $kind } { $version }
# Data Matrix: its size in modules, e.g. "Data Matrix 12×12".
# machine-drafted, needs native review
codes-kind-size = { $kind } { $rows }×{ $cols }
codes-open-row = Link naar { $host } openen…
codes-network-off = Er staat geen netwerk aan, dus links gaan niet open.
codes-open-title = Een link naar { $host } openen?
codes-open-detail = De link opent in je webbrowser. Open alleen links die je vertrouwt.
codes-open = Openen in browser
# machine-drafted, needs native review
# Find text: the text in the picture shown, read by the Local AI pack's Text models.
ocr-placeholder = Tekst in deze afbeelding
# machine-drafted, needs native review
ocr-looking = Tekst lezen…
# machine-drafted, needs native review
ocr-none = Geen tekst gevonden in deze afbeelding.
# machine-drafted, needs native review
ocr-failed = Kan de tekst niet lezen: { $reason }
# machine-drafted, needs native review
ocr-needs-pack = Tekst zoeken heeft de tekstmodellen van het lokale AI-pakket nodig (Instellingen › Lokale AI).
# machine-drafted, needs native review
ocr-loading = Tekstmodellen laden…
# machine-drafted, needs native review
# A row: choosing it copies the line. $text is the line (one line, shortened).
ocr-copy = ‘{ $text }’ kopiëren
# machine-drafted, needs native review
ocr-copy-all = Alle tekst kopiëren
# machine-drafted, needs native review
# A row's note: how sure the reading is, and the add-on that read it. $percent is formatted.
ocr-confidence = { $percent } zeker

## Hardware report panel (machine-drafted, needs native review)
hw-report-section-system = Systeem
hw-report-section-gpu = GPU
hw-report-section-decoding = Decoderen
hw-report-operating-system = Besturingssysteem
hw-report-adapter = Adapter
hw-report-gpu-type = Type
hw-report-gpu-init = Opstarttijd
hw-report-max-layers = Textuurlagen
hw-report-compression = Textuurcompressie
hw-report-unified-memory = Geheugen gedeeld met de CPU
hw-report-display = Beeldscherm
hw-report-hw-decode = Hardwaredecodering
hw-report-core-cluster = Kerngroep
hw-report-power = Voeding
hw-report-calibration = Snelheidsmetingen
hw-report-codec-others = Overige
hw-report-performance-cores-value = { $count } van { $total }
hw-report-cluster-value = { $name }: { $physical } fysiek, { $logical } logisch
hw-report-thermal-nominal = Normale temperatuur
hw-report-thermal-fair = Warm
hw-report-thermal-serious = Heet, wordt afgeremd
hw-report-thermal-critical = Oververhit
hw-report-power-battery = Op batterij ({ $percent })
hw-report-power-battery-unknown = Op batterij
hw-report-power-mains = Aangesloten op het stopcontact
hw-report-power-mains-percent = Aangesloten op het stopcontact ({ $percent })
hw-report-power-saving = Energiebesparing aan
hw-report-gpu-discrete = Toegewijd
hw-report-gpu-integrated = Geïntegreerd
hw-report-gpu-virtual = Virtueel
hw-report-gpu-software = Software, op de CPU
hw-report-gpu-other = Anders
hw-report-gpu-unavailable = Niet beschikbaar
hw-report-ms = { $ms } ms
hw-report-yes = Ja
hw-report-no = Nee
hw-report-unified-platform-differs = Het systeem meldt iets anders
hw-report-display-value = { $fps } Hz, HDR-ruimte nu { $now }, tot { $max }
hw-report-hw-decode-value = { $api } op { $device }
hw-report-hw-decode-checking = Nog bezig met controleren…
hw-report-codec-up-to = Tot { $width } × { $height }
hw-report-codec-none = Niet beschikbaar
hw-report-codec-h264-encode = H.264-codering
hw-report-supported = ondersteund: { $list }
hw-report-unsupported = niet ondersteund: { $list }
hw-report-calibration-done = Gemeten op deze computer
hw-report-calibration-pending = Nog niet gemeten
hw-report-paths-power-saving = Energie besparen: paden die minder energie verbruiken krijgen voorrang
hw-report-paths-caption = Paden voor een afbeelding van { $mpx } megapixel
hw-report-col-task = Taak
hw-report-col-path = Pad
hw-report-col-time = Tijd
hw-report-col-cpu = CPU-tijd (alle threads)
hw-report-col-runs = Metingen
hw-report-col-use = Gebruikt voor
hw-report-runs-untested = Niet getest
hw-report-use-viewing = Weergave ({ $reason })
hw-report-use-background = Achtergrond ({ $reason })
hw-report-reason-only = de enige optie
hw-report-reason-faster = snelst
hw-report-reason-cheaper = minste CPU-tijd
hw-report-reason-lower-power = minste energie
hw-report-reason-explore = wordt uitgeprobeerd
hw-report-reason-forced = ingesteld in Instellingen
hw-report-reason-prefer-gpu = Bij voorkeur de GPU
hw-report-reason-cpu-only = Alleen de CPU
hw-report-reason-calibrate = wordt gemeten
hw-report-reason-scaled = verkleind
hw-report-trust-unverified = Nog niet gecontroleerd
hw-report-trust-blocked = Uitgeschakeld na een fout
hw-report-trust-absent = Niet aanwezig op deze computer

## Keyboard cheat sheet tabs and sections (machine-drafted, needs native review)
cheatsheet-mode-gallery = Galerij
cheatsheet-mode-compare = Vergelijken
cheatsheet-gallery-browse = Bladeren
cheatsheet-gallery-select = Selecteren
cheatsheet-gallery-mark = Beoordelen en markeren
cheatsheet-gallery-move = Tussen afbeeldingen bewegen
cheatsheet-gallery-page = Pagina omhoog of omlaag
cheatsheet-gallery-ends = Eerste of laatste afbeelding
cheatsheet-gallery-up = Omhoog naar de bovenliggende map
cheatsheet-gallery-history-back = Terug naar de vorige map
cheatsheet-gallery-history-forward = Vooruit naar de volgende map
cheatsheet-gallery-back = Bovenste weergave sluiten, dan terug
cheatsheet-gallery-viewer = Terug naar de viewer
cheatsheet-gallery-album-add = Aan het laatste album toevoegen
cheatsheet-gallery-select-one = Selecteren of deselecteren
cheatsheet-gallery-quick-look = Snel bekijken
cheatsheet-gallery-filter = Filteren
cheatsheet-gallery-bigger = Grotere miniaturen
cheatsheet-gallery-smaller = Kleinere miniaturen
cheatsheet-gallery-default-size = Standaardgrootte van miniaturen
cheatsheet-gallery-refresh = Map vernieuwen
cheatsheet-gallery-menu = Contextmenu
cheatsheet-compare-views = Vergelijken
cheatsheet-compare-overlay = Overlay of naast elkaar
cheatsheet-compare-difference = Verschil (nogmaals: volgende soort)
cheatsheet-compare-step-down = Minder overvloeien of verschilversterking
cheatsheet-compare-step-up = Meer overvloeien of verschilversterking
cheatsheet-compare-pick = Afbeelding waarop de toetsen werken
cheatsheet-compare-close = Vergelijken sluiten
cheatsheet-channels = Kanalen
cheatsheet-medical-window = Venster en niveau
cheatsheet-medical-measure = Meten en annoteren
cheatsheet-medical-frames = Frames en series
# machine-drafted, needs native review
cheatsheet-medical-3d = 3D en reconstructie
cheatsheet-medical-analysis = Segmentatie, fusie en PET
# machine-drafted, needs native review
cheatsheet-medical-studies = Onderzoeken en delen
cheatsheet-palette-only-title = In het opdrachtenpalet ({ $key })
cheatsheet-palette-only-title-nokey = In het opdrachtenpalet
cheatsheet-palette-only-tag = Palet
cheatsheet-palette-only-line = { $action }: geen toets; in het opdrachtenpalet ({ $key })
cheatsheet-palette-only-line-nokey = { $action }: geen toets; in het opdrachtenpalet
# Esc in het sneltoetsenoverzicht (de lange beschrijving uit het palet past niet).
cheatsheet-back = Sluiten of teruggaan

## Hardware video encoding (machine-drafted, needs native review)
hw-report-path-video-encode = Video-export
hw-report-engine-hw-encoder = hardware-encoder

## Path timing units (machine-drafted, needs native review)
hw-report-task-thumbnail = { $task } (per miniatuur van { $size })
hw-report-task-frame = { $task } (per frame van { $size })

## Hardware report, software-only formats (machine-drafted, needs native review)
hw-report-codec-software = Alleen software

about-natural-earth = Kaartgegevens van Natural Earth (naturalearthdata.com), publiek domein.
app-info-show-on-map = Op kaart tonen
