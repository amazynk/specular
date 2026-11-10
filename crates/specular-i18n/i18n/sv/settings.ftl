# machine-drafted, needs native review
# Specular — inställningsfönstret. Svenska.

## Fönsterelement

settings-title = Inställningar
settings-search-placeholder = Sök i inställningar
settings-search-no-results = Inga inställningar matchar { $query }
settings-reset = Återställ
settings-reset-section = Återställ avsnitt
settings-modified = Ändrad
settings-default-value = Standard: { $value }
settings-restart-required = Börjar gälla efter omstart
settings-restart-now = Starta om nu
settings-undo = Ångra
settings-redo = Gör om
settings-value-on = På
settings-value-off = Av
settings-value-auto = Automatiskt
settings-value-system = Följ systemet
settings-value-system-language = Systemets standard
settings-value-follow-language = Följ gränssnittsspråket
settings-value-default-location = Standardplats
settings-choose = Välj…
settings-clear = Rensa
settings-file-reloaded = settings.toml ändrades utanför { -app-name } och har lästs in igen.
settings-warning-unknown-key = Okänd inställning { $key } i settings.toml. Den behålls i filen men ignoreras.
settings-warning-invalid-value = Ogiltigt värde för { $key }: { $reason } Standardvärdet används i stället.
settings-warning-newer-version = settings.toml skrevs av en nyare version av { -app-name }. Inställningar som den här versionen inte känner till behålls.
settings-warning-syntax = settings.toml kan inte läsas (rad { $line }): { $reason } Standardvärdena används tills filen är rättad.

## Valideringsfel (skrivs även ut av `specular --set`)

settings-error-unknown-key = Det finns ingen inställning som heter { $key }.
settings-error-unknown-key-suggestion = Det finns ingen inställning som heter { $key }. Menade du { $suggestion }?
settings-error-not-bool = Förväntade on eller off (true eller false).
settings-error-not-integer = Förväntade ett heltal.
settings-error-not-number = Förväntade ett tal.
settings-error-out-of-range = Måste vara mellan { $min } och { $max }.
settings-error-out-of-range-auto = Måste vara 0 (automatiskt) eller mellan { $min } och { $max }.
settings-error-unknown-option = Förväntade något av: { $options }.
settings-error-bad-colour = Förväntade en färg som #1E1E1E eller #1E1E1E80.
settings-error-bad-language = Förväntade en språktagg som de, pt-BR eller zh-Hans, eller ”system”.
settings-error-bad-path = Förväntade en sökväg till en fil eller mapp.
settings-error-not-settable = Den här posten är ett kommando och har inget värde.
settings-error-syntax = Förväntade nyckel=värde, till exempel viewing.wheel_behaviour=navigate.
# machine-drafted, needs native review
settings-error-bad-text = Får inte innehålla tabbar, radbrytningar eller andra styrtecken.
settings-error-invalid = { $reason }
settings-error-tile-url = Kräver en http- eller https-adress med {"{"}z{"}"}, {"{"}x{"}"} och {"{"}y{"}"}.

## Enheter

unit-percent = { PERCENT($value) }
unit-milliseconds = { $value } ms
unit-seconds = { $value } s
unit-mebibytes = { $value } MiB
# machine-drafted, needs native review
unit-luminance = { $value } cd/m²
unit-images =
    { $value ->
        [one] { $value } bild
       *[other] { $value } bilder
    }
unit-threads =
    { $value ->
        [one] { $value } tråd
       *[other] { $value } trådar
    }
unit-bytes =
    { $value ->
        [one] { $value } byte
       *[other] { $value } byte
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

## Avsnitt

settings-section-general = Allmänt
settings-section-viewing = Visning
settings-section-browsing = Bläddring
settings-section-colour = Färg
settings-section-metadata = Metadata och integritet
# machine-drafted, needs native review
settings-section-privacy = Integritet
settings-section-performance = Prestanda
settings-section-accessibility = Hjälpmedel
settings-section-keys = Tangenter
settings-section-advanced = Avancerat

## Allmänt

settings-general-language = Gränssnittsspråk
settings-general-language-help = Ändringar börjar gälla direkt. ”Systemets standard” följer operativsystemets språk.
settings-general-region-formats = Regionala format
settings-general-region-formats-help = Hur datum, tider, tal och storlekar skrivs. Som standard följer de gränssnittsspråket.
settings-general-theme = Tema
settings-general-theme-help = Ljust eller mörkt utseende för fönster och kontroller. Bilder ändras aldrig.
settings-general-theme-system = Följ systemet
settings-general-theme-light = Ljust
settings-general-theme-dark = Mörkt
# machine-drafted, needs native review
settings-general-on-start = När { -app-name } öppnas utan fil
settings-general-on-start-help = Vad som visas när { -app-name } öppnas utan fil. En fil som öppnas från filhanteraren eller kommandoraden visas alltid i visaren.
settings-general-on-start-start-screen = Startskärmen
settings-general-on-start-gallery = Galleri för den senaste mappen
settings-general-on-start-library = Bibliotek
settings-general-start-tips = Visa ”Att prova” på startskärmen
settings-general-start-tips-help = En rad kort med användbara funktioner och deras tangenter. ”Visa välkomst” i kommandopaletten tar tillbaka dem.
settings-general-tips = Visa tips
settings-general-tips-help = Ett kort tips, en gång, första gången en funktion kan hjälpa, till exempel tangenten som visar en mapp som galleri. Aldrig under ett bildspel.
settings-general-tips-shown = Redan visade tips
settings-general-tips-shown-help = Engångstipsen som redan visats, så att de inte visas igen.
# machine-drafted, needs native review
settings-general-your-name = Ditt namn
settings-general-your-name-help = Visas på stämplar och markeringar som du lägger till. Sparas bara i markeringsfilerna du skapar.
settings-general-single-instance = Öppna filer i det befintliga fönstret
settings-general-single-instance-help = När { -app-name } redan körs visas en ny bild i det fönstret i stället för att en ny instans startas.
settings-general-file-associations = Filkopplingar…
settings-general-file-associations-help = Gör { -app-name } till standardvisare för de bildformat som stöds.

## Visning

settings-viewing-initial-zoom = Startzoom
settings-viewing-initial-zoom-help = Hur stor en nyöppnad bild visas.
settings-viewing-initial-zoom-fit = Anpassa till fönstret
settings-viewing-initial-zoom-fit-if-larger = Anpassa stora bilder, visa små i { PERCENT(100) }
settings-viewing-initial-zoom-actual-size = Verklig storlek ({ PERCENT(100) })
settings-viewing-initial-zoom-fill = Fyll fönstret
settings-viewing-wheel-behaviour = Mushjul
settings-viewing-wheel-behaviour-help = Vad mushjulet gör. Håll ned Ctrl för den andra åtgärden.
settings-viewing-wheel-behaviour-help-macos = Vad mushjulet gör. Håll ned ⌘ för den andra åtgärden.
settings-viewing-wheel-behaviour-zoom = Zooma
settings-viewing-wheel-behaviour-navigate = Nästa och föregående bild
settings-viewing-keep-zoom = Behåll zoom mellan bilder
settings-viewing-keep-zoom-help = Inzoomat öppnas nästa och föregående bild med samma zoom och visar samma del av bilden: praktiskt för att kontrollera skärpan i en serie.
settings-viewing-zoom-animation = Animera zoomning
settings-viewing-zoom-animation-help = Zooma mjukt i stället för att hoppa. Stängs av automatiskt när ”Minska rörelse” är på.
settings-viewing-zoom-animation-speed = Hastighet för zoomanimering
settings-viewing-zoom-animation-speed-help = Hur snabbt animerade zoomningar stannar.
settings-viewing-zoom-animation-speed-relaxed = Lugn
settings-viewing-zoom-animation-speed-normal = Normal
settings-viewing-zoom-animation-speed-fast = Snabb
settings-viewing-background = Bakgrund
settings-viewing-background-help = Färgen bakom bilden i ett fönster.
settings-viewing-background-theme = Som temat
settings-viewing-background-black = Svart
settings-viewing-background-dark-grey = Mörkgrå
settings-viewing-background-light-grey = Ljusgrå
settings-viewing-background-white = Vit
settings-viewing-background-custom = Anpassad färg
settings-viewing-background-color = Anpassad bakgrundsfärg
settings-viewing-background-color-help = Används när Bakgrund är inställd på ”Anpassad färg”.
settings-viewing-fullscreen-background = Bakgrund i helskärm
settings-viewing-fullscreen-background-help = Färgen bakom bilden i helskärm.
settings-viewing-transparency = Genomskinliga områden
settings-viewing-transparency-help = Hur genomskinliga delar av en bild ritas.
settings-viewing-transparency-checkerboard = Schackrutor
settings-viewing-transparency-background = Bakgrundsfärg
settings-viewing-transparency-black = Svart
settings-viewing-transparency-white = Vit
settings-viewing-pixel-grid-threshold = Visa pixelrutnät från
settings-viewing-pixel-grid-threshold-help = Rita ett rutnät mellan pixlarna när du har zoomat in minst så här mycket.
settings-viewing-smoothing-high-zoom = Förstorade pixlar
settings-viewing-smoothing-high-zoom-help = Hur pixlar ser ut när bilden visas större än { PERCENT(100) }.
settings-viewing-smoothing-high-zoom-auto = Mjuka, sedan skarpa från { PERCENT(400) }
settings-viewing-smoothing-high-zoom-smooth = Alltid mjuka
settings-viewing-smoothing-high-zoom-pixelated = Alltid skarpa rutor
settings-viewing-double-click = Dubbelklick på bilden
settings-viewing-double-click-help = Dubbelklick på den tomma bakgrunden växlar alltid helskärm.
settings-viewing-double-click-toggle-zoom = Växla mellan anpassad och { PERCENT(100) }
settings-viewing-double-click-fullscreen = Växla helskärm
settings-viewing-double-click-nothing = Gör ingenting
settings-viewing-fullscreen-on-open = Öppna bilder i helskärm
settings-viewing-fullscreen-on-open-help = Starta i helskärm när en bild öppnas utifrån { -app-name }.
settings-viewing-fullscreen-mode = Helskärmsläge
settings-viewing-fullscreen-mode-help = Inbyggd helskärm använder ett eget Space; direkt helskärm täcker det aktuella utan animering.
settings-viewing-fullscreen-mode-native = Inbyggd (eget Space)
settings-viewing-fullscreen-mode-borderless = Direkt
settings-viewing-toolbar-autohide = Dölj verktygsfältet automatiskt
settings-viewing-toolbar-autohide-help = Verktygsfältet och kantpilarna tonas ut när pekaren är stilla och kommer tillbaka när den rör sig.
settings-viewing-toolbar-autohide-delay = Dölj verktygsfältet efter
settings-viewing-toolbar-autohide-delay-help = Hur länge pekaren måste vara stilla innan verktygsfältet tonas ut.
settings-viewing-toolbar-items = Knappar i verktygsfältet
settings-viewing-toolbar-items-help = Vilka knappar verktygsfältet visar, i ordning.
settings-viewing-toolbar-drag = Ordna om verktygsfältet genom att dra
settings-viewing-toolbar-drag-help = Dra en knapp längs verktygsfältet för att flytta den. Ett kort klick trycker fortfarande på den; Esc lägger tillbaka en dragen knapp.
settings-viewing-toolbar-items-open = Öppna
settings-viewing-toolbar-items-prev = Föregående bild
settings-viewing-toolbar-items-next = Nästa bild
settings-viewing-toolbar-items-zoom-out = Zooma ut
settings-viewing-toolbar-items-zoom-level = Zoomnivå
settings-viewing-toolbar-items-zoom-in = Zooma in
settings-viewing-toolbar-items-fit = Anpassa till fönstret
settings-viewing-toolbar-items-rotate-left = Rotera moturs
settings-viewing-toolbar-items-rotate-right = Rotera medurs
settings-viewing-toolbar-items-info = Information
settings-viewing-toolbar-items-copy = Kopiera bild
settings-viewing-toolbar-items-delete = Flytta till papperskorgen
settings-viewing-toolbar-items-palette = Kommandopalett
settings-viewing-toolbar-items-settings = Inställningar
settings-viewing-toolbar-items-fullscreen = Helskärm
# machine-drafted, needs native review
settings-viewing-toolbar-items-spacer = Flexibelt utrymme
settings-viewing-toolbar-items-separator = Avgränsare
settings-toolbar-preview = Förhandsvisning av verktygsfältet
settings-toolbar-hidden = Dolda knappar
settings-toolbar-add-spacer = Lägg till flexibelt utrymme
settings-toolbar-add-separator = Lägg till avgränsare
settings-toolbar-reset = Återställ standard
settings-toolbar-remove = Ta bort { $item }
settings-toolbar-hint = Dra en rad för att flytta den, eller markera den och tryck Alt+↑ eller Alt+↓. Tabb växlar mellan sökfältet och listan.
settings-toolbar-move-up = Flytta upp
settings-toolbar-move-down = Flytta ned
settings-toolbar-move-start = Flytta först
settings-toolbar-move-end = Flytta sist
settings-toolbar-menu-hide = Dölj
settings-toolbar-menu-show = Visa
settings-toolbar-menu-remove = Ta bort
settings-toolbar-row-menu = Fler åtgärder för { $item }
settings-toolbar-position = Position { $index } av { $count }
settings-toolbar-hidden-state = Dold

## Bläddring

settings-browsing-sort-order = Sortera bilder efter
settings-browsing-sort-order-help = Ordningen när du bläddrar i en mapp. Namnordning sätter ”img2” före ”img10” och följer ditt språks alfabet.
settings-browsing-sort-order-natural = Namn
settings-browsing-sort-order-date-taken = Fotodatum
settings-browsing-sort-order-modified = Ändringsdatum
settings-browsing-sort-order-size = Filstorlek
settings-browsing-sort-order-type = Filtyp
settings-browsing-sort-descending = Omvänd ordning
settings-browsing-sort-descending-help = Sortera från sist till först.
settings-browsing-loop-folder = Börja om i slutet av mappen
settings-browsing-loop-folder-help = Fortsätt med den första bilden efter den sista, och tvärtom.
settings-browsing-include-subfolders = Ta med undermappar
settings-browsing-include-subfolders-help = Bläddra även bland bilder i undermappar.
settings-browsing-hold-acceleration = Acceleration när nästa eller föregående hålls ned
settings-browsing-hold-acceleration-help = Hur snabbt bläddringen ökar medan en tangent eller knapp hålls ned.
settings-browsing-hold-acceleration-off = Ingen acceleration
settings-browsing-hold-acceleration-gentle = Försiktig
settings-browsing-hold-acceleration-normal = Normal
settings-browsing-hold-acceleration-fast = Snabb
settings-browsing-confirm-trash = Fråga innan flytt till papperskorgen
settings-browsing-confirm-trash-help = Flytt till papperskorgen kan alltid ångras, så { -app-name } frågar inte som standard.
settings-browsing-arrows-follow-reading-direction = Piltangenter följer läsriktningen
settings-browsing-arrows-follow-reading-direction-help = I språk som skrivs från höger till vänster går vänsterpilen till nästa bild.

## Färg

settings-colour-management = Färghantering
settings-colour-management-help = Visa färgerna som de var tänkta, med bildens färgprofil och skärmens profil.
settings-colour-display-profile = Skärmprofil
settings-colour-display-profile-help = Skärmens färgprofil. Automatiskt använder den som operativsystemet rapporterar.
settings-colour-rendering-intent = Återgivningsmetod
settings-colour-rendering-intent-help = Hur färger som skärmen inte kan visa förs in i dess omfång.
settings-colour-rendering-intent-perceptual = Perceptuell
settings-colour-rendering-intent-relative-colorimetric = Relativ kolorimetrisk
settings-colour-rendering-intent-saturation = Mättnad
settings-colour-rendering-intent-absolute-colorimetric = Absolut kolorimetrisk
settings-colour-black-point-compensation = Svartpunktskompensation
settings-colour-black-point-compensation-help = Behåll detaljer i skuggorna när skärmens svärta är ljusare än bildens.
settings-colour-untagged-images = Bilder utan färgprofil
settings-colour-untagged-images-help = Hur bilder visas som inte anger vilka färger de använder.
settings-colour-untagged-images-srgb = Behandla som sRGB
settings-colour-untagged-images-display = Visa oförändrade
settings-colour-dithering = Gitterutjämning
settings-colour-dithering-help = Lägg till fint blått brus när färgdjupet minskas, vilket förhindrar bandning i himlar och övertoningar.
settings-colour-hdr-output = HDR-utdata
settings-colour-hdr-output-help = Visa HDR-bilder med full ljusstyrka på skärmar som stöder det. Automatiskt använder HDR när skärmen rapporterar extra ljusstyrka; annars komprimeras ljusa områden varsamt för att passa.
settings-colour-hdr-output-auto = Automatiskt
settings-colour-hdr-output-on = På
settings-colour-hdr-output-off = Av
# machine-drafted, needs native review
settings-colour-ocio-config = OCIO-konfiguration
settings-colour-ocio-config-help = En OpenColorIO-konfiguration (.ocio) för bilder i EXR, Radiance HDR och PFM. Tomt använder den som $OCIO anger.
settings-colour-ocio-display = OCIO-skärm
settings-colour-ocio-display-help = Den skärm (display) i OCIO-konfigurationen som motsvarar din bildskärm.
settings-colour-ocio-display-default = Konfigurationens första
settings-colour-ocio-view = OCIO-vy
settings-colour-ocio-view-help = Hur scenlinjära (scene-linear) bilder visas på den skärmen, till exempel en tonmappad vy.
settings-colour-ocio-view-default = Skärmens första
settings-colour-ocio-input = OCIO-indatafärgrymd
settings-colour-ocio-input-help = Färgrymden för bildernas värden.
settings-colour-ocio-input-default = Konfigurationens scene-linear-roll
# machine-drafted, needs native review
settings-colour-proof-profile = Profil för skärmprovtryck
settings-colour-proof-profile-help = Skrivarprofilen som skärmprovtrycket i Framkalla använde senast.
settings-colour-proof-intent = Återgivning vid skärmprovtryck
settings-colour-proof-intent-help = Hur färger som skrivaren inte kan skriva ut anpassas.
settings-colour-proof-intent-perceptual = Perceptuell
settings-colour-proof-intent-relative = Relativ kolorimetrisk
settings-colour-proof-bpc = Svartpunktskompensation vid provtryck
settings-colour-proof-bpc-help = Avbildar svart på skrivarens svart.
settings-colour-proof-paper = Papperets vitt vid provtryck
settings-colour-proof-paper-help = Visar papperets egen färg.
settings-colour-proof-ink = Bläckets svart vid provtryck
settings-colour-proof-ink-help = Visar hur mörkt bläckets svart är.

## Metadata och integritet

settings-metadata-show-info-on-open = Visa informationspanelen vid öppning
settings-metadata-show-info-on-open-help = Öppna panelen med kamera, datum och plats varje gång en bild öppnas.
settings-metadata-info-detail = Informationspanelen visar
settings-metadata-info-detail-help = Sammanfattningen förklarar de mest användbara detaljerna i klartext; alla fält listar allt som finns i filen.
settings-metadata-info-detail-summary = Sammanfattning
settings-metadata-info-detail-all = Alla fält
settings-metadata-reverse-geocoding = Visa ortnamn för GPS-positioner
settings-metadata-reverse-geocoding-help = Slå upp närmaste ort i inbyggda data. Det fungerar offline: positioner lämnar aldrig din dator.
settings-metadata-hide-gps = Dölj GPS-positioner på skärmen
settings-metadata-hide-gps-help = Visa inte koordinater eller platser, till exempel när du delar skärmen. Filerna ändras inte.
# machine-drafted, needs native review
settings-metadata-develop-sidecar = Spara framkallningsändringar bredvid bilden
# machine-drafted, needs native review
settings-metadata-develop-sidecar-help = Skriver IMG_1234.CR2.specular.xmp bredvid filen så att ändringarna följer med. Av: ändringarna finns bara i biblioteket.
# machine-drafted, needs native review
settings-metadata-develop-import-lightroom = Importera Lightroom-inställningar i Framkalla
# machine-drafted, needs native review
settings-metadata-develop-import-lightroom-help = Ett foto med Lightroom-inställningar och ingen redigering öppnas med dem, ungefärligt. Av: Framkalla visar en knapp.
settings-metadata-units = Enheter
settings-metadata-units-help = Enheter för höjd, avstånd och hastighet.
settings-metadata-units-auto = Följ regionala format
settings-metadata-units-metric = Metriska
settings-metadata-units-imperial = Brittiska/amerikanska
# machine-drafted, needs native review
settings-metadata-basemap = Bakgrundskarta
settings-metadata-basemap-help = Kartan under fotoplatser. Onlinekartor hämtar rutor för området du tittar på.
settings-metadata-basemap-off = Bara inbyggd karta (offline)
settings-metadata-basemap-openstreetmap = OpenStreetMap (online)
settings-metadata-basemap-custom = Annan ruttjänst (online)
settings-metadata-basemap-file = Min egen kartfil
settings-metadata-basemap-url = Ruttjänstens adress
settings-metadata-basemap-url-help = En adress med {"{"}z{"}"}, {"{"}x{"}"} och {"{"}y{"}"}, till exempel https://tiles.example.org/{"{"}z{"}"}/{"{"}x{"}"}/{"{"}y{"}"}.png
settings-metadata-basemap-path = Kartfil
settings-metadata-basemap-path-help = En MBTiles- eller PMTiles-fil. Den fungerar offline.
settings-metadata-basemap-cache = Cache för kartrutor
settings-metadata-basemap-cache-help = Diskutrymme för hämtade rutor. De äldsta tas bort först.
# machine-drafted, needs native review
settings-metadata-basemap-allowed = Tillåtna ruttjänster
settings-metadata-basemap-allowed-help = Karttjänster som du har godkänt att hämta rutor från.

## Prestanda

settings-performance-ram-cache = Minnescache
settings-performance-ram-cache-help = Minne som används för att hålla avkodade bilder redo. Automatiskt väljer utifrån hur mycket minne datorn har.
settings-performance-vram-cache = Cache i GPU-minnet
settings-performance-vram-cache-help = GPU-minne som används för bilder och rutor. Automatiskt väljer utifrån din GPU.
settings-performance-prefetch-depth = Bilder att förbereda i förväg
settings-performance-prefetch-depth-help = Hur många bilder i den riktning du bläddrar som avkodas i förväg.
settings-performance-decoder-threads = Avkodningstrådar
settings-performance-decoder-threads-help = CPU-trådar som används för att avkoda bilder. Automatiskt använder alla prestandakärnor och anpassar sig efter filen.
settings-performance-gpu-adapter = GPU
settings-performance-gpu-adapter-help = Vilken GPU som används när datorn har fler än en.
settings-performance-gpu-adapter-high-performance = Högsta prestanda
settings-performance-gpu-adapter-low-power = Lägsta strömförbrukning
settings-performance-processing = Var bilder bearbetas
settings-performance-processing-help = En del arbete kan köras på CPU:n, GPU:n eller en maskinvaruavkodare. Automatiskt mäter den här datorn och väljer det snabbaste sättet för bilden du tittar på och det mest sparsamma för arbete i bakgrunden. Vad som valdes på den här datorn, och varför, visas nedan.
settings-performance-processing-automatic = Automatiskt
settings-performance-processing-prefer-gpu = Föredra GPU:n
settings-performance-processing-cpu-only = Endast CPU
settings-performance-drag-preview = Förhandsvisning vid dragning
settings-performance-drag-preview-help = Hur framkallning visar bilden medan du drar i ett reglage. Automatiskt använder en mindre bild bara när datorn inte hinner med.
settings-performance-drag-preview-automatic = Automatiskt
settings-performance-drag-preview-sharp = Alltid skarp
settings-performance-drag-preview-fast = Alltid snabb

## Inställningar → Prestanda → Så körs { -app-name } på den här datorn. $engine står efter
## ett kolon eller i obestämd form, eftersom namnet inte kan böjas.
settings-paths-title = Så körs { -app-name } på den här datorn
settings-paths-intro = { -app-name } tar tid på varje sätt att utföra de här jobben på den här datorn och använder det bästa. Tiderna gäller ett foto på { $mpx } megapixel.
settings-paths-intro-measuring = Mätningen är inte klar än: { -app-name } mäter när datorn är inaktiv.
settings-paths-intro-battery = På batteri: valen prioriterar lägre energiförbrukning.
settings-paths-keywords = GPU CPU NPU grafikkort maskinvaruacceleration avkodare prestandatest benchmark hastighet prestanda
paths-engine-cpu = CPU
paths-engine-cpu-parallel = CPU, alla kärnor
paths-engine-gpu = GPU
paths-engine-hw-decoder = Maskinvaruavkodare
paths-engine-npu = NPU
settings-paths-your-setting = { $engine } (din inställning)
settings-paths-measured-faster = Uppmätt på den här datorn: { $ratio } gånger så snabbt som med { $other }.
settings-paths-measured-similar = Uppmätt på den här datorn: ungefär lika snabbt som med { $other }.
settings-paths-measured-power = På batteri drar detta minst energi (uppmätt på den här datorn).
settings-paths-not-measured = Inte uppmätt än: det vanliga valet för den här typen av dator.
settings-paths-set-by = Valt av din inställning ”{ $setting }: { $option }”.
settings-paths-blocked-check = Avstängd { $date } eftersom resultatet inte stämde med CPU:ns: { $engine }.
settings-paths-blocked-error = Avstängd { $date } efter ett fel: { $engine }.
settings-paths-blocked-lost = Avstängd { $date } efter att ha slutat svara: { $engine }.
settings-paths-blocked-undated = Avstängd efter ett fel: { $engine }.
settings-paths-checking = Kontrolleras mot CPU:n innan den används för bilden du tittar på: { $engine }.
settings-paths-exploring = Provas nu i bakgrunden: { $engine }.
settings-paths-unavailable = Den här datorn saknar { $engine } för detta.
settings-paths-background-cheaper = I bakgrunden: { $engine }, som enligt mätningar använder { $percent } mindre CPU-tid.
settings-paths-background-power = I bakgrunden: { $engine }, för att spara energi.
settings-paths-background = I bakgrunden: { $engine }.
settings-paths-detail-measured = { $engine }: { $time } ms, { $cpu } ms CPU-tid, { $runs ->
    [one] { $runs } mätning
   *[other] { $runs } mätningar
} ({ $confidence })
settings-paths-detail-guess = { $engine }: cirka { $time } ms (uppskattning)
settings-paths-confidence-low = låg säkerhet
settings-paths-confidence-medium = medelhög säkerhet
settings-paths-confidence-high = hög säkerhet
settings-paths-machine = Uppmätt för { $gpu } · { $driver } · { $cpu }
settings-paths-retry = Försök igen
settings-paths-measure-again = Mät igen
settings-paths-measure-again-question = Vill du glömma det som har mätts på den här datorn, inklusive de avstängda vägarna, och mäta igen när datorn är inaktiv?
settings-paths-measure-again-confirm = Glöm mätningar
settings-performance-display-cache = Spara en visningscache på disken
settings-performance-display-cache-help = Lagra kopior av bilder i skärmstorlek så att mappar du har sett öppnas direkt.
settings-performance-display-cache-size = Storlek på visningscache
settings-performance-display-cache-size-help = De äldsta posterna tas bort när cachen är full.
settings-performance-display-cache-location = Plats för visningscache
settings-performance-display-cache-location-help = En mapp på en snabb enhet fungerar bäst.
settings-performance-fast-simd = Använd SIMD-instruktioner
settings-performance-fast-simd-help = Använd CPU:ns vektorinstruktioner för avkodning. Stäng av endast för att felsöka ett problem.
settings-performance-fast-dct-scaling = Avkoda i skärmstorlek
settings-performance-fast-dct-scaling-help = Avkoda stora JPEG-bilder direkt i den storlek de visas. Stäng av endast för att felsöka ett problem.
settings-performance-fast-parallel-decode = Avkoda på flera kärnor
settings-performance-fast-parallel-decode-help = Dela upp stora bilder på flera CPU-kärnor. Stäng av endast för att felsöka ett problem.
settings-performance-fast-restart-index = Indexera stora JPEG-bilder för zoomning
settings-performance-fast-restart-index-help = Kom ihåg var varje del av en JPEG börjar, så att inzoomade områden avkodas snabbt. Stäng av endast för att felsöka ett problem.
settings-performance-fast-yuv-upload = Konvertera färger på GPU:n
settings-performance-fast-yuv-upload-help = Konvertera JPEG-färger på GPU:n eller CPU:n. Automatiskt väljer det som är snabbast på den här datorn. Stäng av endast för att felsöka ett problem.
settings-performance-fast-yuv-upload-auto = Automatiskt
settings-performance-fast-yuv-upload-on = På
settings-performance-fast-yuv-upload-off = Av
settings-performance-fast-async-io = Asynkron filläsning
settings-performance-fast-async-io-help = Läs i förväg med operativsystemets snabbaste filgränssnitt. Stäng av endast för att felsöka ett problem.
settings-performance-fast-mmap = Minnesmappade filer
settings-performance-fast-mmap-help = Läs filer utan att kopiera dem. Stäng av endast för att felsöka ett problem.
settings-performance-hardware-decode = Maskinvaruavkodning
settings-performance-hardware-decode-help = Låt GPU:ns inbyggda avkodare öppna JPEG-foton. Automatiskt använder den där det hjälper, oftast för bilder som förbereds i bakgrunden. Den kontrolleras först mot den vanliga avkodaren, och allt den inte klarar öppnas som vanligt.
settings-performance-hardware-decode-auto = Automatiskt
settings-performance-hardware-decode-on = På
settings-performance-hardware-decode-off = Av
settings-performance-frame-time-hud = Bildtids-HUD
settings-performance-frame-time-hud-help = Visa avkodnings- och bildrutetider och vilka snabba vägar som används.
settings-performance-stall-warning = Rapportera fastnade inläsningar efter
settings-performance-stall-warning-help = När en bild fortfarande läses in efter så här lång tid skrivs dess inläsningsstatus och väntande jobb till loggen (standard error eller SPECULAR_LOG_FILE).
# machine-drafted, needs native review
settings-performance-insights = Prestandainsikter
settings-performance-insights-help = Visa ”Varför går det långsamt?” i hörnet med vad som skulle hjälpa när bilder gång på gång öppnas långsamt av samma skäl. Det som lärs in stannar på den här datorn.
settings-performance-insights-storage = Insikter om långsam lagring
settings-performance-insights-storage-help = Visa insikter när läsning av filer (från en nätverksresurs, en långsam enhet eller arkiv) gång på gång fördröjer öppnandet av bilder.
settings-performance-insights-decoder = Insikter om avkodning
settings-performance-insights-decoder-help = Visa insikter när bilder gång på gång avkodas på CPU:n fast en maskinvaruavkodare skulle kunna hjälpa.
# machine-drafted, needs native review
settings-performance-insights-restore = Visa dolda insikter igen
settings-performance-insights-restore-help = Tar tillbaka insikterna du dolde med × vid ”Varför går det långsamt?”, både för att öppna bilder och för miniatyrer. Inte tillgänglig så länge inga är dolda.

## Hjälpmedel

settings-accessibility-reduce-motion = Minska rörelse
settings-accessibility-reduce-motion-help = Ersätt zoom-, glid- och toningsanimeringar med omedelbara ändringar.
settings-accessibility-reduce-motion-system = Följ systemet
settings-accessibility-reduce-motion-on = På
settings-accessibility-reduce-motion-off = Av
settings-accessibility-increase-contrast = Öka kontrasten
settings-accessibility-increase-contrast-help = Tydligare kanter och text, och mindre genomskinlighet.
settings-accessibility-increase-contrast-system = Följ systemet
settings-accessibility-increase-contrast-on = På
settings-accessibility-increase-contrast-off = Av
settings-accessibility-text-scale = Textstorlek
settings-accessibility-text-scale-help = Textens storlek i förhållande till standardstorleken.
settings-accessibility-ui-scale = Gränssnittsskala
settings-accessibility-ui-scale-help = Storleken på hela gränssnittet, utöver skalningen som är inställd i operativsystemet.
settings-accessibility-focus-ring-always = Visa alltid fokusramen
settings-accessibility-focus-ring-always-help = Visa vilken kontroll som har tangentbordsfokus även när du använder musen.
settings-accessibility-screen-reader-verbosity = Detaljnivå för skärmläsare
# machine-drafted, needs native review
settings-accessibility-screen-reader-verbosity-help = Hur mycket en skärmläsare läser upp när du går mellan bilder och hur mycket av avläsningen under markören den läser upp.
settings-accessibility-screen-reader-verbosity-concise = Kortfattad
settings-accessibility-screen-reader-verbosity-normal = Normal
settings-accessibility-screen-reader-verbosity-verbose = Utförlig
# machine-drafted, needs native review
settings-accessibility-announce-cursor = Läs upp värdet under markören
settings-accessibility-announce-cursor-help = En skärmläsare läser upp pixelvärdet, HU, temperaturen eller positionen under markören eller tangentbordsmålkorset när det slutar röra sig.

## Integritet

# machine-drafted, needs native review
settings-privacy-no-network = Inget nätverk
# machine-drafted, needs native review
settings-privacy-no-network-help = Av: bara dessa ansluter, var och en först när du slår på den eller öppnar något med den: uppdateringskontroll, nedladdning av lokala AI-modeller, onlinekartor, bilder från en webbadress eller molnlagring och servrar du lägger till (DICOMweb, OMERO, kartor). På: inget ansluter.

## Avancerat

settings-advanced-portable-mode = Portabelt läge
settings-advanced-portable-mode-help = Spara inställningar och cache i mappen bredvid programmet i stället för i din användarprofil.
settings-advanced-log-level = Loggnivå
settings-advanced-log-level-help = Hur mycket { -app-name } skriver till sin loggfil.
settings-advanced-log-level-error = Endast fel
settings-advanced-log-level-warn = Varningar
settings-advanced-log-level-info = Information
settings-advanced-log-level-debug = Felsökning
settings-advanced-log-level-trace = Allt
settings-advanced-developer-mode = Utvecklarläge
settings-advanced-developer-mode-help = Visa verktyg för att utveckla och testa { -app-name }, till exempel pseudospråken i språklistan.
# machine-drafted, needs native review
settings-advanced-audit-log = Lokal granskningslogg
# machine-drafted, needs native review
settings-advanced-audit-log-help = Registrera öppnade medicinska bilder, exporter, anonymiseringar, utskrifter och sändningar till ett PACS i audit.jsonl i datamappen. Den stannar på den här datorn.
settings-advanced-hw-report = Maskinvarurapport…
settings-advanced-hw-report-help = Visa vilka CPU- och GPU-funktioner { -app-name } har identifierat och vilka snabba vägar den använder.
# machine-drafted, needs native review
settings-advanced-renderer-info = Renderarinformation…
settings-advanced-renderer-info-help = Visa grafikadaptern, drivrutinen, skärmformaten och minnesgränserna som { -app-name } använder, redo att kopiera till en felrapport.
settings-advanced-edit-settings-file = Redigera settings.toml
settings-advanced-edit-settings-file-help = Redigera inställningsfilen i { -app-name }s egen redigerare. Den kontrollerar filen medan du skriver och sparar den bara när den saknar fel.
settings-advanced-import = Importera inställningar…
settings-advanced-import-help = Läs in inställningar från en fil som exporterats från { -app-name }.
settings-advanced-export = Exportera inställningar…
settings-advanced-export-help = Spara alla inställningar i en fil, som säkerhetskopia eller för en annan dator.
settings-advanced-reset-all = Återställ alla inställningar…
settings-advanced-reset-all-help = Återställ alla inställningar till standard. Detta kan ångras.

## settings.toml editor (machine-drafted, needs native review)

config-editor-title = settings.toml
config-editor-save = Spara
config-editor-revert = Återställ
config-editor-close = Stäng
config-editor-modified = Ändrad
config-editor-text = Inställningsfil
config-editor-position = Rad { $line }, kolumn { $column }
config-editor-saved = settings.toml sparades och tillämpades.
config-editor-cant-save =
    { $count ->
        [one] Kan inte spara: 1 problem. Åtgärda det först.
       *[other] Kan inte spara: { $count } problem. Åtgärda dem först.
    }
config-editor-save-failed = Det gick inte att spara settings.toml: { $reason }
config-editor-conflict = settings.toml ändrades någon annanstans.
config-editor-reload = Läs in igen
config-editor-keep-mine = Behåll mina
config-editor-discard-question = Kasta osparade ändringar i settings.toml?
config-editor-discard = Kasta
config-editor-keep-editing = Fortsätt redigera
config-editor-find-placeholder = Sök i filen
config-editor-find-count = { $current } av { $total }
config-editor-find-none = Inga träffar
config-editor-find-previous = Föregående träff
config-editor-find-next = Nästa träff
config-editor-find-close = Stäng sökningen
config-editor-problems = Problem
config-editor-problems-count =
    { $count ->
        [0] Inga problem
        [one] 1 problem
       *[other] { $count } problem
    }
config-editor-no-problems = Inga problem. Filen kan sparas.
config-editor-problem-at = Rad { $line }: { $message }
config-editor-severity-error = Fel
config-editor-severity-warning = Varning
config-editor-severity-info = Notering
config-editor-problem-syntax = Ogiltig TOML: { $reason }
config-editor-problem-duplicate = { $key } anges två gånger i samma tabell. Ta bort en av dem.
config-editor-problem-unknown-section = Det finns ingen inställningssektion [{ $key }].
config-editor-problem-unknown-section-suggestion = Det finns ingen inställningssektion [{ $key }]. Menade du [{ $suggestion }]?
config-editor-problem-unknown-key = Det finns ingen inställning { $key }.
config-editor-problem-unknown-key-suggestion = Det finns ingen inställning { $key }. Menade du { $suggestion }?
config-editor-problem-invalid = Ogiltigt värde för { $key }: { $reason }
config-editor-problem-migrated = { $key } är skrivet som en äldre version av { -app-name } skrev det. Det konverteras vid läsning.
config-editor-problem-newer-version = Filen skrevs av en nyare version av { -app-name } (format { $found }; den här versionen läser { $supported }). Okända inställningar behålls.
config-editor-problem-command = Kommando { $index } i [[commands]]: { $reason }
config-editor-problem-hook = Krok { $index } i [[hooks]]: { $reason }

## keymap.toml in the same editor (machine-drafted, needs native review)

config-editor-keymap-title = keymap.toml
config-editor-keymap-text = Fil med kortkommandon
config-editor-keymap-saved = keymap.toml sparades och tillämpades.
config-editor-keymap-save-failed = Det går inte att spara keymap.toml: { $reason }
config-editor-keymap-conflict = keymap.toml har ändrats på annat håll.
config-editor-keymap-discard-question = Vill du kasta dina osparade ändringar i keymap.toml?
config-editor-new-file = ny fil, skapas när den sparas
config-editor-no-folder = det finns ingen inställningsmapp att spara den i.

## Reset all settings confirmation (machine-drafted, needs native review)

settings-reset-all-confirm = Återställa alla inställningar till standard?
settings-reset-all-confirm-detail = Det påverkar alla delar av Inställningar. Det går att ångra efteråt.
settings-reset-all-confirm-ok = Återställ allt
settings-reset-all-done = Alla inställningar har återställts till standard.

## Hardware video encoding (machine-drafted, needs native review)
paths-engine-hw-encoder = Maskinvarukodare

## Path timing units (machine-drafted, needs native review)
settings-paths-checking-before-use = Kontrolleras mot CPU:n innan den används: { $engine }.
settings-paths-unit-thumbnail = Tiderna gäller en miniatyr på { $size }
settings-paths-unit-frame = Tiderna gäller per videobildruta på { $size }
# machine-drafted, needs native review
settings-metadata-develop-calibration = Mapp för kalibreringsbilder
# machine-drafted, needs native review
settings-metadata-develop-calibration-help = Flat- och darkbilder för Framkalla. Fil- eller mappnamnen innehåller "flat" eller "dark".

## Colour swatches under a colour setting's field (machine-drafted, needs native review)
settings-colour-white = Vit
settings-colour-light-grey = Ljusgrå
settings-colour-grey = Grå
settings-colour-dark-grey = Mörkgrå
settings-colour-black = Svart
settings-colour-yellow = Gul
settings-colour-orange = Orange
settings-colour-red = Röd
settings-colour-magenta = Magenta
settings-colour-blue = Blå
settings-colour-cyan = Cyan
settings-colour-green = Grön
