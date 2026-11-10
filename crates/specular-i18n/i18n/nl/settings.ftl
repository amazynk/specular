# machine-drafted, needs native review
# Specular — Instellingenvenster. Nederlands (informeel: je/jouw).

## Vensteronderdelen

settings-title = Instellingen
settings-search-placeholder = Instellingen zoeken
settings-search-no-results = Geen instellingen gevonden voor { $query }
settings-reset = Herstellen
settings-reset-section = Sectie herstellen
settings-modified = Gewijzigd
settings-default-value = Standaard: { $value }
settings-restart-required = Werkt na opnieuw starten
settings-restart-now = Nu opnieuw starten
settings-undo = Ongedaan maken
settings-redo = Opnieuw
settings-value-on = Aan
settings-value-off = Uit
settings-value-auto = Automatisch
settings-value-system = Systeem volgen
settings-value-system-language = Systeemstandaard
settings-value-follow-language = Interfacetaal volgen
settings-value-default-location = Standaardlocatie
settings-choose = Kiezen…
settings-clear = Wissen
settings-file-reloaded = settings.toml is buiten { -app-name } gewijzigd en opnieuw geladen.
settings-warning-unknown-key = Onbekende instelling { $key } in settings.toml. Die blijft in het bestand staan, maar wordt genegeerd.
settings-warning-invalid-value = Ongeldige waarde voor { $key }: { $reason } De standaardwaarde wordt gebruikt.
settings-warning-newer-version = settings.toml is geschreven door een nieuwere versie van { -app-name }. Instellingen die deze versie niet kent, blijven bewaard.
settings-warning-syntax = settings.toml kan niet worden gelezen (regel { $line }): { $reason } De standaardwaarden worden gebruikt tot het is opgelost.

## Validatiefouten (ook getoond door `specular --set`)

settings-error-unknown-key = Er is geen instelling met de naam { $key }.
settings-error-unknown-key-suggestion = Er is geen instelling met de naam { $key }. Bedoel je { $suggestion }?
settings-error-not-bool = Verwacht: aan of uit (true of false).
settings-error-not-integer = Verwacht: een geheel getal.
settings-error-not-number = Verwacht: een getal.
settings-error-out-of-range = Moet tussen { $min } en { $max } liggen.
settings-error-out-of-range-auto = Moet 0 (automatisch) zijn of tussen { $min } en { $max } liggen.
settings-error-unknown-option = Verwacht een van: { $options }.
settings-error-bad-colour = Verwacht: een kleur zoals #1E1E1E of #1E1E1E80.
settings-error-bad-language = Verwacht: een taalcode zoals de, pt-BR of zh-Hans, of ‘system’.
settings-error-bad-path = Verwacht: een pad naar een bestand of map.
settings-error-not-settable = Dit item is een opdracht en heeft geen waarde.
settings-error-syntax = Verwacht: sleutel=waarde, bijvoorbeeld viewing.wheel_behaviour=navigate.
# machine-drafted, needs native review
settings-error-bad-text = Mag geen tabs, regeleinden of andere stuurtekens bevatten.
settings-error-invalid = { $reason }
settings-error-tile-url = Vereist een http- of https-adres met {"{"}z{"}"}, {"{"}x{"}"} en {"{"}y{"}"}.

## Eenheden

unit-percent = { PERCENT($value) }
unit-milliseconds = { $value } ms
unit-seconds = { $value } s
unit-mebibytes = { $value } MiB
# machine-drafted, needs native review
unit-luminance = { $value } cd/m²
unit-images =
    { $value ->
        [one] { $value } afbeelding
       *[other] { $value } afbeeldingen
    }
unit-threads =
    { $value ->
        [one] { $value } thread
       *[other] { $value } threads
    }
unit-bytes =
    { $value ->
        [one] { $value } byte
       *[other] { $value } bytes
    }
unit-kilobytes = { $value } kB
unit-megabytes = { $value } MB
unit-gigabytes = { $value } GB
unit-terabytes = { $value } TB
unit-kibibytes = { $value } KiB
unit-gibibytes = { $value } GiB
unit-tebibytes = { $value } TiB
unit-symbol-mm = mm
unit-symbol-cm = cm
unit-symbol-m = m
unit-symbol-km = km
unit-symbol-mm2 = mm²
unit-symbol-cm2 = cm²
unit-symbol-m2 = m²
unit-symbol-km2 = km²
unit-symbol-ha = ha
unit-symbol-ft = ft
unit-symbol-ft2 = ft²
unit-symbol-mi = mi
unit-symbol-mi2 = mi²
unit-symbol-ac = ac

## Secties

settings-section-general = Algemeen
settings-section-viewing = Weergave
settings-section-browsing = Bladeren
settings-section-colour = Kleur
settings-section-metadata = Metagegevens en privacy
# machine-drafted, needs native review
settings-section-privacy = Privacy
settings-section-performance = Prestaties
settings-section-accessibility = Toegankelijkheid
settings-section-keys = Toetsen
settings-section-advanced = Geavanceerd

## Algemeen

settings-general-language = Interfacetaal
settings-general-language-help = Wijzigingen werken meteen. ‘Systeemstandaard’ volgt de taal van je besturingssysteem.
settings-general-region-formats = Regionale notatie
settings-general-region-formats-help = Hoe datums, tijden, getallen en groottes worden geschreven. Standaard volgen ze de interfacetaal.
settings-general-theme = Thema
settings-general-theme-help = Lichte of donkere weergave van vensters en bedieningselementen. Afbeeldingen worden nooit gewijzigd.
settings-general-theme-system = Systeem volgen
settings-general-theme-light = Licht
settings-general-theme-dark = Donker
# machine-drafted, needs native review
settings-general-on-start = Als { -app-name } zonder bestand opent
settings-general-on-start-help = Wat er wordt getoond als je { -app-name } los opent. Een bestand dat je vanuit je bestandsbeheerder of de opdrachtregel opent, verschijnt altijd in de viewer.
settings-general-on-start-start-screen = Het startscherm
settings-general-on-start-gallery = Galerij van de laatste map
settings-general-on-start-library = Bibliotheek
settings-general-start-tips = ‘Om te proberen’ tonen op het startscherm
settings-general-start-tips-help = Een rij kaarten met handige functies en hun toetsen. ‘Welkom tonen’ in het opdrachtenpalet haalt ze terug.
settings-general-tips = Tips tonen
settings-general-tips-help = Een korte tip, één keer, wanneer een functie voor het eerst kan helpen, zoals de toets die een map als galerij toont. Nooit tijdens een diavoorstelling.
settings-general-tips-shown = Al getoonde tips
settings-general-tips-shown-help = De eenmalige tips die al zijn gegeven, zodat ze niet terugkomen.
# machine-drafted, needs native review
settings-general-your-name = Je naam
settings-general-your-name-help = Staat op stempels en markeringen die je toevoegt. Alleen opgeslagen in de markeringsbestanden die je maakt.
settings-general-single-instance = Bestanden openen in het bestaande venster
settings-general-single-instance-help = Als { -app-name } al actief is, wordt een andere afbeelding in dat venster getoond in plaats van een nieuw exemplaar te starten.
settings-general-file-associations = Bestandskoppelingen…
settings-general-file-associations-help = Maak { -app-name } de standaardviewer voor de ondersteunde afbeeldingsindelingen.

## Weergave

settings-viewing-initial-zoom = Begininzoom
settings-viewing-initial-zoom-help = Hoe groot een nieuw geopende afbeelding wordt getoond.
settings-viewing-initial-zoom-fit = Passend in venster
settings-viewing-initial-zoom-fit-if-larger = Grote passend maken, kleine op { PERCENT(100) }
settings-viewing-initial-zoom-actual-size = Werkelijke grootte ({ PERCENT(100) })
settings-viewing-initial-zoom-fill = Venster vullen
settings-viewing-wheel-behaviour = Muiswiel
settings-viewing-wheel-behaviour-help = Wat scrollen met het wiel doet. Houd Ctrl ingedrukt voor de andere actie.
settings-viewing-wheel-behaviour-help-macos = Wat scrollen met het wiel doet. Houd ⌘ ingedrukt voor de andere actie.
settings-viewing-wheel-behaviour-zoom = Zoomen
settings-viewing-wheel-behaviour-navigate = Volgende en vorige afbeelding
settings-viewing-keep-zoom = Zoom behouden tussen afbeeldingen
settings-viewing-keep-zoom-help = Ingezoomd openen de volgende en vorige afbeelding met dezelfde zoom en tonen ze hetzelfde deel van het beeld: handig om de scherpte van een reeks te controleren.
settings-viewing-zoom-animation = Zoomen animeren
settings-viewing-zoom-animation-help = Vloeiend zoomen in plaats van verspringen. Wordt automatisch uitgeschakeld als ‘Beweging beperken’ aan staat.
settings-viewing-zoom-animation-speed = Snelheid zoomanimatie
settings-viewing-zoom-animation-speed-help = Hoe snel geanimeerd zoomen tot rust komt.
settings-viewing-zoom-animation-speed-relaxed = Rustig
settings-viewing-zoom-animation-speed-normal = Normaal
settings-viewing-zoom-animation-speed-fast = Snel
settings-viewing-background = Achtergrond
settings-viewing-background-help = De kleur achter de afbeelding in een venster.
settings-viewing-background-theme = Zoals het thema
settings-viewing-background-black = Zwart
settings-viewing-background-dark-grey = Donkergrijs
settings-viewing-background-light-grey = Lichtgrijs
settings-viewing-background-white = Wit
settings-viewing-background-custom = Aangepaste kleur
settings-viewing-background-color = Aangepaste achtergrondkleur
settings-viewing-background-color-help = Gebruikt als Achtergrond op ‘Aangepaste kleur’ staat.
settings-viewing-fullscreen-background = Achtergrond volledig scherm
settings-viewing-fullscreen-background-help = De kleur achter de afbeelding op volledig scherm.
settings-viewing-transparency = Transparante delen
settings-viewing-transparency-help = Hoe transparante delen van een afbeelding worden getekend.
settings-viewing-transparency-checkerboard = Dambordpatroon
settings-viewing-transparency-background = Achtergrondkleur
settings-viewing-transparency-black = Zwart
settings-viewing-transparency-white = Wit
settings-viewing-pixel-grid-threshold = Pixelraster tonen vanaf
settings-viewing-pixel-grid-threshold-help = Teken een raster tussen pixels als je minstens zo ver bent ingezoomd.
settings-viewing-smoothing-high-zoom = Vergrote pixels
settings-viewing-smoothing-high-zoom-help = Hoe pixels eruitzien als de afbeelding groter dan { PERCENT(100) } wordt getoond.
settings-viewing-smoothing-high-zoom-auto = Vloeiend, scherp vanaf { PERCENT(400) }
settings-viewing-smoothing-high-zoom-smooth = Altijd vloeiend
settings-viewing-smoothing-high-zoom-pixelated = Altijd scherpe vierkantjes
settings-viewing-double-click = Dubbelklikken op de afbeelding
settings-viewing-double-click-help = Dubbelklikken op de lege achtergrond schakelt altijd volledig scherm aan of uit.
settings-viewing-double-click-toggle-zoom = Wisselen tussen passend en { PERCENT(100) }
settings-viewing-double-click-fullscreen = Volledig scherm aan/uit
settings-viewing-double-click-nothing = Niets doen
settings-viewing-fullscreen-on-open = Afbeeldingen openen op volledig scherm
settings-viewing-fullscreen-on-open-help = Start op volledig scherm als een afbeelding van buiten { -app-name } wordt geopend.
settings-viewing-fullscreen-mode = Stijl van volledig scherm
settings-viewing-fullscreen-mode-help = Systeemeigen volledig scherm gebruikt een eigen Space; direct volledig scherm bedekt de huidige zonder animatie.
settings-viewing-fullscreen-mode-native = Systeemeigen (eigen Space)
settings-viewing-fullscreen-mode-borderless = Direct
settings-viewing-toolbar-autohide = Werkbalk automatisch verbergen
settings-viewing-toolbar-autohide-help = De werkbalk en randpijlen vervagen als de aanwijzer stilstaat en komen terug als hij beweegt.
settings-viewing-toolbar-autohide-delay = Werkbalk verbergen na
settings-viewing-toolbar-autohide-delay-help = Hoe lang de aanwijzer stil moet staan voordat de werkbalk vervaagt.
settings-viewing-toolbar-items = Werkbalkknoppen
settings-viewing-toolbar-items-help = Welke knoppen de werkbalk toont, in volgorde.
settings-viewing-toolbar-drag = Werkbalk herschikken door te slepen
settings-viewing-toolbar-drag-help = Sleep een knop langs de werkbalk om hem te verplaatsen. Een korte klik drukt hem nog steeds in; Esc zet een gesleepte knop terug.
settings-viewing-toolbar-items-open = Openen
settings-viewing-toolbar-items-prev = Vorige afbeelding
settings-viewing-toolbar-items-next = Volgende afbeelding
settings-viewing-toolbar-items-zoom-out = Uitzoomen
settings-viewing-toolbar-items-zoom-level = Zoomniveau
settings-viewing-toolbar-items-zoom-in = Inzoomen
settings-viewing-toolbar-items-fit = Passend in venster
settings-viewing-toolbar-items-rotate-left = Linksom draaien
settings-viewing-toolbar-items-rotate-right = Rechtsom draaien
settings-viewing-toolbar-items-info = Informatie
settings-viewing-toolbar-items-copy = Afbeelding kopiëren
settings-viewing-toolbar-items-delete = Naar prullenbak verplaatsen
settings-viewing-toolbar-items-palette = Opdrachtenpalet
settings-viewing-toolbar-items-settings = Instellingen
settings-viewing-toolbar-items-fullscreen = Volledig scherm
# machine-drafted, needs native review
settings-viewing-toolbar-items-spacer = Flexibele ruimte
settings-viewing-toolbar-items-separator = Scheidingslijn
settings-toolbar-preview = Voorbeeld van werkbalk
settings-toolbar-hidden = Verborgen knoppen
settings-toolbar-add-spacer = Flexibele ruimte toevoegen
settings-toolbar-add-separator = Scheidingslijn toevoegen
settings-toolbar-reset = Standaard herstellen
settings-toolbar-remove = { $item } verwijderen
settings-toolbar-hint = Sleep een rij om hem te verplaatsen, of selecteer hem en druk op Alt+↑ of Alt+↓. Tab gaat tussen het zoekveld en de lijst.
settings-toolbar-move-up = Omhoog
settings-toolbar-move-down = Omlaag
settings-toolbar-move-start = Naar het begin
settings-toolbar-move-end = Naar het einde
settings-toolbar-menu-hide = Verbergen
settings-toolbar-menu-show = Tonen
settings-toolbar-menu-remove = Verwijderen
settings-toolbar-row-menu = Meer acties voor { $item }
settings-toolbar-position = Positie { $index } van { $count }
settings-toolbar-hidden-state = Verborgen

## Bladeren

settings-browsing-sort-order = Afbeeldingen sorteren op
settings-browsing-sort-order-help = De volgorde bij het bladeren door een map. Sorteren op naam zet ‘img2’ vóór ‘img10’ en volgt het alfabet van je taal.
settings-browsing-sort-order-natural = Naam
settings-browsing-sort-order-date-taken = Opnamedatum
settings-browsing-sort-order-modified = Wijzigingsdatum
settings-browsing-sort-order-size = Bestandsgrootte
settings-browsing-sort-order-type = Bestandstype
settings-browsing-sort-descending = Omgekeerde volgorde
settings-browsing-sort-descending-help = Sorteren van laatste naar eerste.
settings-browsing-loop-folder = Doorgaan aan het eind van de map
settings-browsing-loop-folder-help = Ga na de laatste afbeelding verder met de eerste, en andersom.
settings-browsing-include-subfolders = Submappen meenemen
settings-browsing-include-subfolders-help = Ook door afbeeldingen in submappen bladeren.
settings-browsing-hold-acceleration = Versnellen bij ingedrukt houden
settings-browsing-hold-acceleration-help = Hoe snel het bladeren versnelt terwijl je een toets of knop ingedrukt houdt.
settings-browsing-hold-acceleration-off = Niet versnellen
settings-browsing-hold-acceleration-gentle = Rustig
settings-browsing-hold-acceleration-normal = Normaal
settings-browsing-hold-acceleration-fast = Snel
settings-browsing-confirm-trash = Vragen voor verplaatsen naar prullenbak
settings-browsing-confirm-trash-help = Verplaatsen naar de prullenbak kan altijd ongedaan worden gemaakt, dus { -app-name } vraagt het standaard niet.
settings-browsing-arrows-follow-reading-direction = Pijltoetsen volgen leesrichting
settings-browsing-arrows-follow-reading-direction-help = Laat in talen die van rechts naar links lezen de linkerpijltoets naar de volgende afbeelding gaan.

## Kleur

settings-colour-management = Kleurbeheer
settings-colour-management-help = Toon kleuren zoals ze bedoeld zijn, met het kleurprofiel van de afbeelding en dat van je beeldscherm.
settings-colour-display-profile = Beeldschermprofiel
settings-colour-display-profile-help = Het kleurprofiel van je beeldscherm. Automatisch gebruikt het profiel dat je besturingssysteem doorgeeft.
settings-colour-rendering-intent = Weergavemethode
settings-colour-rendering-intent-help = Hoe kleuren die het beeldscherm niet kan tonen binnen het bereik worden gebracht.
settings-colour-rendering-intent-perceptual = Perceptueel
settings-colour-rendering-intent-relative-colorimetric = Relatief colorimetrisch
settings-colour-rendering-intent-saturation = Verzadiging
settings-colour-rendering-intent-absolute-colorimetric = Absoluut colorimetrisch
settings-colour-black-point-compensation = Zwartpuntcompensatie
settings-colour-black-point-compensation-help = Behoud detail in de schaduwen als het zwart van het beeldscherm lichter is dan dat van de afbeelding.
settings-colour-untagged-images = Afbeeldingen zonder kleurprofiel
settings-colour-untagged-images-help = Hoe afbeeldingen worden getoond die niet aangeven welke kleuren ze gebruiken.
settings-colour-untagged-images-srgb = Als sRGB behandelen
settings-colour-untagged-images-display = Ongewijzigd tonen
settings-colour-dithering = Dithering
settings-colour-dithering-help = Voeg fijne blauwe ruis toe bij het verlagen van de kleurdiepte; dat voorkomt banding in luchten en verlopen.
settings-colour-hdr-output = HDR-uitvoer
settings-colour-hdr-output-help = Toon HDR-afbeeldingen met hun volle helderheid op beeldschermen die dat ondersteunen. Automatisch gebruikt HDR als het beeldscherm extra helderheid meldt; anders worden heldere delen voorzichtig gecomprimeerd zodat ze passen.
settings-colour-hdr-output-auto = Automatisch
settings-colour-hdr-output-on = Aan
settings-colour-hdr-output-off = Uit
# machine-drafted, needs native review
settings-colour-ocio-config = OCIO-configuratie
settings-colour-ocio-config-help = Een OpenColorIO-configuratie (.ocio) voor afbeeldingen in EXR, Radiance HDR en PFM. Leeg gebruikt de configuratie die $OCIO noemt.
settings-colour-ocio-display = OCIO-display
settings-colour-ocio-display-help = Het display van de OCIO-configuratie dat je beeldscherm is.
settings-colour-ocio-display-default = De eerste van de configuratie
settings-colour-ocio-view = OCIO-weergave
settings-colour-ocio-view-help = Hoe scene-linear afbeeldingen op dat display worden getoond, zoals een weergave met tone mapping.
settings-colour-ocio-view-default = De eerste van het display
settings-colour-ocio-input = OCIO-invoerkleurruimte
settings-colour-ocio-input-help = De kleurruimte van de waarden van de afbeeldingen.
settings-colour-ocio-input-default = De scene-linear-rol van de configuratie
# machine-drafted, needs native review
settings-colour-proof-profile = Softproofprofiel
settings-colour-proof-profile-help = Het printerprofiel dat de softproof van Ontwikkelen het laatst gebruikte.
settings-colour-proof-intent = Softproof-weergave
settings-colour-proof-intent-help = Hoe kleuren die de printer niet kan printen worden aangepast.
settings-colour-proof-intent-perceptual = Perceptueel
settings-colour-proof-intent-relative = Relatief colorimetrisch
settings-colour-proof-bpc = Zwartpuntcompensatie bij softproof
settings-colour-proof-bpc-help = Zwart afbeelden op het zwart van de printer.
settings-colour-proof-paper = Papierwit bij softproof
settings-colour-proof-paper-help = De eigen kleur van het papier tonen.
settings-colour-proof-ink = Inktzwart bij softproof
settings-colour-proof-ink-help = Tonen hoe donker het zwart van de inkt is.

## Metagegevens en privacy

settings-metadata-show-info-on-open = Informatiepaneel tonen bij openen
settings-metadata-show-info-on-open-help = Open het paneel met camera-, datum- en locatiegegevens telkens als een afbeelding wordt geopend.
settings-metadata-info-detail = Informatiepaneel toont
settings-metadata-info-detail-help = Het overzicht legt de nuttigste details in gewone taal uit; alle velden toont alles wat in het bestand staat.
settings-metadata-info-detail-summary = Overzicht
settings-metadata-info-detail-all = Alle velden
settings-metadata-reverse-geocoding = Plaatsnamen tonen bij GPS-posities
settings-metadata-reverse-geocoding-help = Zoek de dichtstbijzijnde plaats op in ingebouwde gegevens. Werkt offline: posities verlaten je computer nooit.
settings-metadata-hide-gps = GPS-posities op het scherm verbergen
settings-metadata-hide-gps-help = Toon geen coördinaten of plaatsen, bijvoorbeeld als je je scherm deelt. Bestanden worden niet gewijzigd.
# machine-drafted, needs native review
settings-metadata-develop-sidecar = Ontwikkelbewerkingen naast de foto opslaan
# machine-drafted, needs native review
settings-metadata-develop-sidecar-help = Schrijft IMG_1234.CR2.specular.xmp naast het bestand, zodat bewerkingen meereizen. Uit: bewerkingen blijven alleen in de bibliotheek.
# machine-drafted, needs native review
settings-metadata-develop-import-lightroom = Lightroom-instellingen importeren in Ontwikkelen
# machine-drafted, needs native review
settings-metadata-develop-import-lightroom-help = Een foto met Lightroom-instellingen en zonder bewerking opent ermee, bij benadering. Uit: Ontwikkelen biedt een knop.
settings-metadata-units = Eenheden
settings-metadata-units-help = Eenheden voor hoogte, afstanden en snelheden.
settings-metadata-units-auto = Regionale notatie volgen
settings-metadata-units-metric = Metrisch
settings-metadata-units-imperial = Imperiaal
# machine-drafted, needs native review
settings-metadata-basemap = Achtergrondkaart
settings-metadata-basemap-help = De kaart onder fotolocaties. Online kaarten downloaden tegels voor het gebied dat je bekijkt.
settings-metadata-basemap-off = Alleen ingebouwde kaart (offline)
settings-metadata-basemap-openstreetmap = OpenStreetMap (online)
settings-metadata-basemap-custom = Andere tegelserver (online)
settings-metadata-basemap-file = Mijn eigen kaartbestand
settings-metadata-basemap-url = Adres van de tegelserver
settings-metadata-basemap-url-help = Een adres met {"{"}z{"}"}, {"{"}x{"}"} en {"{"}y{"}"}, bijvoorbeeld https://tiles.example.org/{"{"}z{"}"}/{"{"}x{"}"}/{"{"}y{"}"}.png
settings-metadata-basemap-path = Kaartbestand
settings-metadata-basemap-path-help = Een MBTiles- of PMTiles-bestand. Het werkt offline.
settings-metadata-basemap-cache = Cache voor kaarttegels
settings-metadata-basemap-cache-help = Schijfruimte voor gedownloade tegels. De oudste gaan eerst weg.
# machine-drafted, needs native review
settings-metadata-basemap-allowed = Toegestane tegelservers
settings-metadata-basemap-allowed-help = Kaartservers waarvan je het downloaden van tegels hebt toegestaan.

## Prestaties

settings-performance-ram-cache = Geheugencache
settings-performance-ram-cache-help = Geheugen om gedecodeerde afbeeldingen klaar te houden. Automatisch kiest op basis van het geheugen van je computer.
settings-performance-vram-cache = Cache in GPU-geheugen
settings-performance-vram-cache-help = GPU-geheugen voor afbeeldingen en tegels. Automatisch kiest op basis van je GPU.
settings-performance-prefetch-depth = Afbeeldingen vooraf laden
settings-performance-prefetch-depth-help = Hoeveel afbeeldingen in de richting waarin je bladert vooraf worden gedecodeerd.
settings-performance-decoder-threads = Decodeerthreads
settings-performance-decoder-threads-help = CPU-threads voor het decoderen van afbeeldingen. Automatisch gebruikt alle prestatiekernen en past zich aan het bestand aan.
settings-performance-gpu-adapter = GPU
settings-performance-gpu-adapter-help = Welke GPU wordt gebruikt als de computer er meer dan één heeft.
settings-performance-gpu-adapter-high-performance = Hoogste prestaties
settings-performance-gpu-adapter-low-power = Laagste stroomverbruik
settings-performance-processing = Waar afbeeldingen worden verwerkt
settings-performance-processing-help = Sommig werk kan op de CPU, de GPU of een hardwaredecoder draaien. Automatisch meet deze computer en kiest de snelste manier voor de afbeelding die je bekijkt en de zuinigste manier voor werk op de achtergrond. Wat er op deze computer is gekozen, en waarom, staat hieronder.
settings-performance-processing-automatic = Automatisch
settings-performance-processing-prefer-gpu = Bij voorkeur de GPU
settings-performance-processing-cpu-only = Alleen de CPU
settings-performance-drag-preview = Voorbeeld tijdens slepen
settings-performance-drag-preview-help = Hoe Ontwikkelen de foto toont terwijl u een schuifregelaar sleept. Automatisch gebruikt alleen een kleiner beeld als de computer het niet bijhoudt.
settings-performance-drag-preview-automatic = Automatisch
settings-performance-drag-preview-sharp = Altijd scherp
settings-performance-drag-preview-fast = Altijd snel

## Instellingen → Prestaties → Hoe { -app-name } op deze computer werkt
settings-paths-title = Hoe { -app-name } op deze computer werkt
settings-paths-intro = { -app-name } meet op deze computer hoe snel elke manier om deze taken uit te voeren is en gebruikt de beste. Tijden gelden voor een foto van { $mpx } megapixel.
settings-paths-intro-measuring = Nog niet klaar met meten: { -app-name } meet terwijl de computer inactief is.
settings-paths-intro-battery = Op batterij: de keuzes geven voorrang aan een lager energieverbruik.
settings-paths-keywords = GPU CPU NPU videokaart grafische kaart hardwareversnelling decoder benchmark snelheid prestaties
paths-engine-cpu = CPU
paths-engine-cpu-parallel = CPU, alle kernen
paths-engine-gpu = GPU
paths-engine-hw-decoder = Hardwaredecoder
paths-engine-npu = NPU
settings-paths-your-setting = { $engine } (jouw instelling)
settings-paths-measured-faster = Gemeten op deze computer: { $ratio } keer zo snel als de { $other }.
settings-paths-measured-similar = Gemeten op deze computer: ongeveer even snel als de { $other }.
settings-paths-measured-power = Op batterij verbruikt dit de minste energie (gemeten op deze computer).
settings-paths-not-measured = Nog niet gemeten: de gebruikelijke keuze voor dit soort computer.
settings-paths-set-by = Gekozen door je instelling ‘{ $setting }: { $option }’.
settings-paths-blocked-check = De { $engine } is op { $date } uitgeschakeld: het resultaat kwam niet overeen met dat van de CPU.
settings-paths-blocked-error = De { $engine } is op { $date } uitgeschakeld na een fout.
settings-paths-blocked-lost = De { $engine } is op { $date } uitgeschakeld nadat hij niet meer reageerde.
settings-paths-blocked-undated = De { $engine } is uitgeschakeld na een fout.
settings-paths-checking = De { $engine } wordt eerst met de CPU vergeleken voordat hij wordt gebruikt voor de afbeelding die je bekijkt.
settings-paths-exploring = De { $engine } wordt nu op de achtergrond uitgeprobeerd.
settings-paths-unavailable = Deze computer heeft hiervoor geen { $engine }.
settings-paths-background-cheaper = Op de achtergrond: de { $engine }, die volgens metingen { $percent } minder CPU-tijd gebruikt.
settings-paths-background-power = Op de achtergrond: de { $engine }, om energie te besparen.
settings-paths-background = Op de achtergrond: de { $engine }.
settings-paths-detail-measured = { $engine }: { $time } ms, { $cpu } ms CPU-tijd, { $runs ->
    [one] { $runs } meting
   *[other] { $runs } metingen
} ({ $confidence })
settings-paths-detail-guess = { $engine }: ongeveer { $time } ms (schatting)
settings-paths-confidence-low = lage betrouwbaarheid
settings-paths-confidence-medium = gemiddelde betrouwbaarheid
settings-paths-confidence-high = hoge betrouwbaarheid
settings-paths-machine = Gemeten voor { $gpu } · { $driver } · { $cpu }
settings-paths-retry = Opnieuw proberen
settings-paths-measure-again = Opnieuw meten
settings-paths-measure-again-question = Wat op deze computer is gemeten vergeten, inclusief de uitgeschakelde paden, en opnieuw meten terwijl de computer inactief is?
settings-paths-measure-again-confirm = Metingen vergeten
settings-performance-display-cache = Weergavecache op schijf bewaren
settings-performance-display-cache-help = Bewaar kopieën van afbeeldingen op schermgrootte, zodat mappen die je al hebt gezien meteen openen.
settings-performance-display-cache-size = Grootte weergavecache
settings-performance-display-cache-size-help = De oudste items worden verwijderd als de cache vol is.
settings-performance-display-cache-location = Locatie weergavecache
settings-performance-display-cache-location-help = Een map op een snelle schijf werkt het best.
settings-performance-fast-simd = SIMD-instructies gebruiken
settings-performance-fast-simd-help = Gebruik de vectorinstructies van de CPU voor het decoderen. Zet dit alleen uit om een probleem op te sporen.
settings-performance-fast-dct-scaling = Decoderen op schermgrootte
settings-performance-fast-dct-scaling-help = Decodeer grote JPEG’s direct op de grootte waarop ze worden getoond. Zet dit alleen uit om een probleem op te sporen.
settings-performance-fast-parallel-decode = Decoderen op meerdere kernen
settings-performance-fast-parallel-decode-help = Verdeel grote afbeeldingen over CPU-kernen. Zet dit alleen uit om een probleem op te sporen.
settings-performance-fast-restart-index = Grote JPEG’s indexeren voor zoomen
settings-performance-fast-restart-index-help = Onthoud waar elk deel van een JPEG begint, zodat ingezoomde gebieden snel worden gedecodeerd. Zet dit alleen uit om een probleem op te sporen.
settings-performance-fast-yuv-upload = Kleuren omzetten op de GPU
settings-performance-fast-yuv-upload-help = Zet JPEG-kleuren om op de GPU of de CPU. Automatisch kiest wat op deze computer sneller is. Zet dit alleen uit om een probleem op te sporen.
settings-performance-fast-yuv-upload-auto = Automatisch
settings-performance-fast-yuv-upload-on = Aan
settings-performance-fast-yuv-upload-off = Uit
settings-performance-fast-async-io = Asynchroon bestanden lezen
settings-performance-fast-async-io-help = Lees vooruit met de snelste bestandsinterface van het besturingssysteem. Zet dit alleen uit om een probleem op te sporen.
settings-performance-fast-mmap = Geheugengemapte bestanden
settings-performance-fast-mmap-help = Lees bestanden zonder ze te kopiëren. Zet dit alleen uit om een probleem op te sporen.
settings-performance-hardware-decode = Hardwaredecodering
settings-performance-hardware-decode-help = Laat de ingebouwde decoder van de GPU JPEG-foto’s openen. Automatisch gebruikt hem waar dat helpt, meestal voor afbeeldingen die op de achtergrond worden voorbereid. Hij wordt eerst gecontroleerd tegen de normale decoder, en alles wat hij niet aankan, opent op de gewone manier.
settings-performance-hardware-decode-auto = Automatisch
settings-performance-hardware-decode-on = Aan
settings-performance-hardware-decode-off = Uit
settings-performance-frame-time-hud = Frametijd-HUD
settings-performance-frame-time-hud-help = Toon decodeer- en frametijden en welke snelle paden worden gebruikt.
settings-performance-stall-warning = Vastgelopen laden melden na
settings-performance-stall-warning-help = Als een afbeelding na zo lang nog laadt, schrijf dan de laadstatus en de openstaande taken naar het logboek (standaardfout, of SPECULAR_LOG_FILE).
# machine-drafted, needs native review
settings-performance-insights = Prestatie-inzichten
settings-performance-insights-help = Als afbeeldingen steeds om dezelfde reden traag openen, ‘Waarom is dit traag?’ in de hoek tonen met wat zou helpen. Wat het leert, blijft op deze computer.
settings-performance-insights-storage = Inzichten over trage opslag
settings-performance-insights-storage-help = Inzichten tonen als het lezen van bestanden (van een netwerkshare, een trage schijf of archieven) het openen van afbeeldingen steeds ophoudt.
settings-performance-insights-decoder = Inzichten over decoderen
settings-performance-insights-decoder-help = Inzichten tonen als afbeeldingen steeds op de CPU worden gedecodeerd terwijl een hardwaredecoder zou kunnen helpen.
# machine-drafted, needs native review
settings-performance-insights-restore = Verborgen inzichten weer tonen
settings-performance-insights-restore-help = Haalt de inzichten terug die je met × bij ‘Waarom is dit traag?’ hebt verborgen, zowel voor het openen van afbeeldingen als voor miniaturen. Niet beschikbaar zolang er geen verborgen zijn.

## Toegankelijkheid

settings-accessibility-reduce-motion = Beweging beperken
settings-accessibility-reduce-motion-help = Vervang zoom-, schuif- en vervaaganimaties door directe overgangen.
settings-accessibility-reduce-motion-system = Systeem volgen
settings-accessibility-reduce-motion-on = Aan
settings-accessibility-reduce-motion-off = Uit
settings-accessibility-increase-contrast = Contrast verhogen
settings-accessibility-increase-contrast-help = Sterkere randen en tekst, en minder doorschijnendheid.
settings-accessibility-increase-contrast-system = Systeem volgen
settings-accessibility-increase-contrast-on = Aan
settings-accessibility-increase-contrast-off = Uit
settings-accessibility-text-scale = Tekstgrootte
settings-accessibility-text-scale-help = Grootte van tekst ten opzichte van de standaardgrootte.
settings-accessibility-ui-scale = Interfaceschaal
settings-accessibility-ui-scale-help = Grootte van de hele interface, bovenop de schaling die in je besturingssysteem is ingesteld.
settings-accessibility-focus-ring-always = Focusrand altijd tonen
settings-accessibility-focus-ring-always-help = Toon welk element de toetsenbordfocus heeft, ook als je de muis gebruikt.
settings-accessibility-screen-reader-verbosity = Detail van schermlezer
# machine-drafted, needs native review
settings-accessibility-screen-reader-verbosity-help = Hoeveel een schermlezer zegt als je tussen afbeeldingen wisselt, en hoeveel van de uitlezing onder de aanwijzer hij voorleest.
settings-accessibility-screen-reader-verbosity-concise = Beknopt
settings-accessibility-screen-reader-verbosity-normal = Normaal
settings-accessibility-screen-reader-verbosity-verbose = Uitgebreid
# machine-drafted, needs native review
settings-accessibility-announce-cursor = Waarde onder de aanwijzer voorlezen
settings-accessibility-announce-cursor-help = Een schermlezer leest de pixelwaarde, HU, temperatuur of positie onder de aanwijzer of het toetsenbordkruis voor zodra die stopt met bewegen.

## Privacy

# machine-drafted, needs native review
settings-privacy-no-network = Geen netwerk
# machine-drafted, needs native review
settings-privacy-no-network-help = Uit: alleen deze maken verbinding, elk pas als je het aanzet of er iets mee opent: zoeken naar updates, downloads van lokale AI-modellen, online kaarten, afbeeldingen vanaf een webadres of cloudopslag en servers die je toevoegt (DICOMweb, OMERO, kaarten). Aan: niets maakt verbinding.

## Geavanceerd

settings-advanced-portable-mode = Draagbare modus
settings-advanced-portable-mode-help = Bewaar instellingen en caches in de map naast het programma in plaats van in je gebruikersprofiel.
settings-advanced-log-level = Logboekdetail
settings-advanced-log-level-help = Hoeveel { -app-name } naar het logbestand schrijft.
settings-advanced-log-level-error = Alleen fouten
settings-advanced-log-level-warn = Waarschuwingen
settings-advanced-log-level-info = Informatie
settings-advanced-log-level-debug = Foutopsporing
settings-advanced-log-level-trace = Alles
settings-advanced-developer-mode = Ontwikkelaarsmodus
settings-advanced-developer-mode-help = Toon hulpmiddelen voor het ontwikkelen en testen van { -app-name }, zoals de pseudotalen in de talenlijst.
# machine-drafted, needs native review
settings-advanced-audit-log = Lokaal auditlogboek
# machine-drafted, needs native review
settings-advanced-audit-log-help = Leg geopende medische beelden, exports, anonimiseringen, afdrukken en verzendingen naar een PACS vast in audit.jsonl in de gegevensmap. Het blijft op deze computer.
settings-advanced-hw-report = Hardwarerapport…
settings-advanced-hw-report-help = Toon welke CPU- en GPU-functies { -app-name } heeft gevonden en welke snelle paden het gebruikt.
# machine-drafted, needs native review
settings-advanced-renderer-info = Rendererinformatie…
settings-advanced-renderer-info-help = Toon de grafische adapter, het stuurprogramma, de weergaveformaten en de geheugenlimieten die { -app-name } gebruikt, klaar om in een foutrapport te kopiëren.
settings-advanced-edit-settings-file = settings.toml bewerken
settings-advanced-edit-settings-file-help = Bewerk het instellingenbestand in de eigen editor van { -app-name }. Die controleert het bestand terwijl u typt en slaat het alleen op als er geen fouten in staan.
settings-advanced-import = Instellingen importeren…
settings-advanced-import-help = Laad instellingen uit een bestand dat uit { -app-name } is geëxporteerd.
settings-advanced-export = Instellingen exporteren…
settings-advanced-export-help = Sla alle instellingen op in één bestand, als back-up of voor een andere computer.
settings-advanced-reset-all = Alle instellingen herstellen…
settings-advanced-reset-all-help = Zet elke instelling terug naar de standaardwaarde. Dit kan ongedaan worden gemaakt.

## settings.toml editor (machine-drafted, needs native review)

config-editor-title = settings.toml
config-editor-save = Opslaan
config-editor-revert = Terugzetten
config-editor-close = Sluiten
config-editor-modified = Gewijzigd
config-editor-text = Instellingenbestand
config-editor-position = Regel { $line }, kolom { $column }
config-editor-saved = settings.toml opgeslagen en toegepast.
config-editor-cant-save =
    { $count ->
        [one] Kan niet opslaan: 1 probleem. Los het eerst op.
       *[other] Kan niet opslaan: { $count } problemen. Los ze eerst op.
    }
config-editor-save-failed = Kan settings.toml niet opslaan: { $reason }
config-editor-conflict = settings.toml is elders gewijzigd.
config-editor-reload = Opnieuw laden
config-editor-keep-mine = Mijn versie houden
config-editor-discard-question = Niet-opgeslagen wijzigingen in settings.toml verwerpen?
config-editor-discard = Verwerpen
config-editor-keep-editing = Verder bewerken
config-editor-find-placeholder = Zoeken in bestand
config-editor-find-count = { $current } van { $total }
config-editor-find-none = Geen resultaten
config-editor-find-previous = Vorig resultaat
config-editor-find-next = Volgend resultaat
config-editor-find-close = Zoeken sluiten
config-editor-problems = Problemen
config-editor-problems-count =
    { $count ->
        [0] Geen problemen
        [one] 1 probleem
       *[other] { $count } problemen
    }
config-editor-no-problems = Geen problemen. Het bestand kan worden opgeslagen.
config-editor-problem-at = Regel { $line }: { $message }
config-editor-severity-error = Fout
config-editor-severity-warning = Waarschuwing
config-editor-severity-info = Opmerking
config-editor-problem-syntax = Geen geldige TOML: { $reason }
config-editor-problem-duplicate = { $key } staat twee keer in dezelfde tabel. Verwijder er een.
config-editor-problem-unknown-section = Er is geen instellingensectie [{ $key }].
config-editor-problem-unknown-section-suggestion = Er is geen instellingensectie [{ $key }]. Bedoelde u [{ $suggestion }]?
config-editor-problem-unknown-key = Er is geen instelling { $key }.
config-editor-problem-unknown-key-suggestion = Er is geen instelling { $key }. Bedoelde u { $suggestion }?
config-editor-problem-invalid = Ongeldige waarde voor { $key }: { $reason }
config-editor-problem-migrated = { $key } is geschreven zoals een oudere versie van { -app-name } het schreef. Het wordt bij het lezen omgezet.
config-editor-problem-newer-version = Dit bestand is geschreven door een nieuwere versie van { -app-name } (formaat { $found }; deze versie leest { $supported }). Onbekende instellingen blijven behouden.
config-editor-problem-command = Opdracht { $index } in [[commands]]: { $reason }
config-editor-problem-hook = Hook-programma { $index } in [[hooks]]: { $reason }

## keymap.toml in the same editor (machine-drafted, needs native review)

config-editor-keymap-title = keymap.toml
config-editor-keymap-text = Bestand met sneltoetsen
config-editor-keymap-saved = keymap.toml opgeslagen en toegepast.
config-editor-keymap-save-failed = Kan keymap.toml niet opslaan: { $reason }
config-editor-keymap-conflict = keymap.toml is elders gewijzigd.
config-editor-keymap-discard-question = Niet-opgeslagen wijzigingen in keymap.toml verwerpen?
config-editor-new-file = nieuw bestand, wordt bij opslaan aangemaakt
config-editor-no-folder = er is geen instellingenmap om het in op te slaan.

## Reset all settings confirmation (machine-drafted, needs native review)

settings-reset-all-confirm = Alle instellingen terugzetten naar de standaardwaarden?
settings-reset-all-confirm-detail = Dit geldt voor alle onderdelen van Instellingen. Je kunt het daarna ongedaan maken.
settings-reset-all-confirm-ok = Alles terugzetten
settings-reset-all-done = Alle instellingen zijn teruggezet naar de standaardwaarden.

## Hardware video encoding (machine-drafted, needs native review)
paths-engine-hw-encoder = Hardware-encoder

## Path timing units (machine-drafted, needs native review)
settings-paths-checking-before-use = De { $engine } wordt eerst met de CPU vergeleken voordat hij wordt gebruikt.
settings-paths-unit-thumbnail = Tijden gelden voor één miniatuur van { $size }
settings-paths-unit-frame = Tijden gelden per videoframe van { $size }
# machine-drafted, needs native review
settings-metadata-develop-calibration = Map met kalibratiebeelden
# machine-drafted, needs native review
settings-metadata-develop-calibration-help = Flat- en darkframes voor Ontwikkelen. Bestands- of mapnamen bevatten "flat" of "dark".

## Colour swatches under a colour setting's field (machine-drafted, needs native review)
settings-colour-white = Wit
settings-colour-light-grey = Lichtgrijs
settings-colour-grey = Grijs
settings-colour-dark-grey = Donkergrijs
settings-colour-black = Zwart
settings-colour-yellow = Geel
settings-colour-orange = Oranje
settings-colour-red = Rood
settings-colour-magenta = Magenta
settings-colour-blue = Blauw
settings-colour-cyan = Cyaan
settings-colour-green = Groen
