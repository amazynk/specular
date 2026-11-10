# machine-drafted, needs native review
# Specular — indstillingsvinduet. Dansk.

## Vinduesramme

settings-title = Indstillinger
settings-search-placeholder = Søg i indstillinger
settings-search-no-results = Ingen indstillinger matcher { $query }
settings-reset = Nulstil
settings-reset-section = Nulstil sektion
settings-modified = Ændret
settings-default-value = Standard: { $value }
settings-restart-required = Træder i kraft efter en genstart
settings-restart-now = Genstart nu
settings-undo = Fortryd
settings-redo = Annuller fortryd
settings-value-on = Til
settings-value-off = Fra
settings-value-auto = Automatisk
settings-value-system = Følg systemet
settings-value-system-language = Systemstandard
settings-value-follow-language = Følg brugerfladens sprog
settings-value-default-location = Standardplacering
settings-choose = Vælg…
settings-clear = Ryd
settings-file-reloaded = settings.toml blev ændret uden for { -app-name } og er indlæst igen.
settings-warning-unknown-key = Ukendt indstilling { $key } i settings.toml. Den bevares i filen, men ignoreres.
settings-warning-invalid-value = Ugyldig værdi for { $key }: { $reason } Standardværdien bruges i stedet.
settings-warning-newer-version = settings.toml blev skrevet af en nyere version af { -app-name }. Indstillinger, som denne version ikke kender, bevares.
settings-warning-syntax = settings.toml kan ikke læses (linje { $line }): { $reason } Standardværdierne bruges, indtil filen er rettet.

## Valideringsfejl (udskrives også af `specular --set`)

settings-error-unknown-key = Der findes ingen indstilling med navnet { $key }.
settings-error-unknown-key-suggestion = Der findes ingen indstilling med navnet { $key }. Mente du { $suggestion }?
settings-error-not-bool = Forventede til eller fra (true eller false).
settings-error-not-integer = Forventede et heltal.
settings-error-not-number = Forventede et tal.
settings-error-out-of-range = Skal være mellem { $min } og { $max }.
settings-error-out-of-range-auto = Skal være 0 (automatisk) eller mellem { $min } og { $max }.
settings-error-unknown-option = Forventede en af: { $options }.
settings-error-bad-colour = Forventede en farve som #1E1E1E eller #1E1E1E80.
settings-error-bad-language = Forventede en sprogkode som de, pt-BR eller zh-Hans, eller ”system”.
settings-error-bad-path = Forventede en sti til en fil eller mappe.
settings-error-not-settable = Dette punkt er en kommando og har ingen værdi.
settings-error-syntax = Forventede nøgle=værdi, f.eks. viewing.wheel_behaviour=navigate.
# machine-drafted, needs native review
settings-error-bad-text = Må ikke indeholde tabulatorer, linjeskift eller andre kontroltegn.
settings-error-invalid = { $reason }
settings-error-tile-url = Kræver en http- eller https-adresse med {"{"}z{"}"}, {"{"}x{"}"} og {"{"}y{"}"}.

## Enheder

unit-percent = { PERCENT($value) }
unit-milliseconds = { $value } ms
unit-seconds = { $value } s
unit-mebibytes = { $value } MiB
# machine-drafted, needs native review
unit-luminance = { $value } cd/m²
unit-images =
    { $value ->
        [one] { $value } billede
       *[other] { $value } billeder
    }
unit-threads =
    { $value ->
        [one] { $value } tråd
       *[other] { $value } tråde
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

## Sektioner

settings-section-general = Generelt
settings-section-viewing = Visning
settings-section-browsing = Gennemsyn
settings-section-colour = Farve
settings-section-metadata = Metadata og privatliv
# machine-drafted, needs native review
settings-section-privacy = Privatliv
settings-section-performance = Ydeevne
settings-section-accessibility = Tilgængelighed
settings-section-keys = Taster
settings-section-advanced = Avanceret

## Generelt

settings-general-language = Brugerfladens sprog
settings-general-language-help = Ændringer træder i kraft med det samme. ”Systemstandard” følger operativsystemets sprog.
settings-general-region-formats = Områdeformater
settings-general-region-formats-help = Hvordan datoer, klokkeslæt, tal og størrelser skrives. Som standard følger de brugerfladens sprog.
settings-general-theme = Tema
settings-general-theme-help = Lyst eller mørkt udseende for vinduer og knapper. Billeder ændres aldrig.
settings-general-theme-system = Følg systemet
settings-general-theme-light = Lyst
settings-general-theme-dark = Mørkt
# machine-drafted, needs native review
settings-general-on-start = Når { -app-name } åbnes uden en fil
settings-general-on-start-help = Hvad der vises, når { -app-name } åbnes alene. En fil, der åbnes fra filhåndteringen eller kommandolinjen, vises altid i fremviseren.
settings-general-on-start-start-screen = Startskærmen
settings-general-on-start-gallery = Galleri for den seneste mappe
settings-general-on-start-library = Bibliotek
settings-general-start-tips = Vis »Prøv dette« på startskærmen
settings-general-start-tips-help = En række kort med nyttige funktioner og deres taster. »Vis velkomst« i kommandopaletten henter dem tilbage.
settings-general-tips = Vis tips
settings-general-tips-help = Et kort tip, én gang, første gang en funktion kan hjælpe, f.eks. tasten, der viser en mappe som galleri. Aldrig under et diasshow.
settings-general-tips-shown = Allerede viste tips
settings-general-tips-shown-help = De engangstips, der allerede er givet, så de ikke vises igen.
# machine-drafted, needs native review
settings-general-your-name = Dit navn
settings-general-your-name-help = Vises på stempler og markeringer, du tilføjer. Gemmes kun i de markeringsfiler, du laver.
settings-general-single-instance = Åbn filer i det eksisterende vindue
settings-general-single-instance-help = Når { -app-name } allerede kører, vises et nyt billede i det vindue i stedet for at starte en ny forekomst.
settings-general-file-associations = Filtilknytninger…
settings-general-file-associations-help = Gør { -app-name } til standardfremviser for de billedformater, programmet understøtter.

## Visning

settings-viewing-initial-zoom = Startzoom
settings-viewing-initial-zoom-help = Hvor stort et nyåbnet billede vises.
settings-viewing-initial-zoom-fit = Tilpas til vinduet
settings-viewing-initial-zoom-fit-if-larger = Tilpas store billeder, vis små i { PERCENT(100) }
settings-viewing-initial-zoom-actual-size = Faktisk størrelse ({ PERCENT(100) })
settings-viewing-initial-zoom-fill = Udfyld vinduet
settings-viewing-wheel-behaviour = Musehjul
settings-viewing-wheel-behaviour-help = Hvad det gør at rulle med hjulet. Hold Ctrl nede for den anden handling.
settings-viewing-wheel-behaviour-help-macos = Hvad det gør at rulle med hjulet. Hold ⌘ nede for den anden handling.
settings-viewing-wheel-behaviour-zoom = Zoom
settings-viewing-wheel-behaviour-navigate = Næste og forrige billede
settings-viewing-keep-zoom = Behold zoom mellem billeder
settings-viewing-keep-zoom-help = Når du har zoomet ind, åbner næste og forrige billede med samme zoom og viser samme del af billedet: praktisk til at tjekke fokus i en serie.
settings-viewing-zoom-animation = Animér zoom
settings-viewing-zoom-animation-help = Zoom jævnt i stedet for at springe. Slås automatisk fra, når ”Reducer bevægelse” er slået til.
settings-viewing-zoom-animation-speed = Hastighed for zoomanimation
settings-viewing-zoom-animation-speed-help = Hvor hurtigt animerede zoom falder på plads.
settings-viewing-zoom-animation-speed-relaxed = Rolig
settings-viewing-zoom-animation-speed-normal = Normal
settings-viewing-zoom-animation-speed-fast = Hurtig
settings-viewing-background = Baggrund
settings-viewing-background-help = Farven bag billedet i et vindue.
settings-viewing-background-theme = Følg temaet
settings-viewing-background-black = Sort
settings-viewing-background-dark-grey = Mørkegrå
settings-viewing-background-light-grey = Lysegrå
settings-viewing-background-white = Hvid
settings-viewing-background-custom = Brugerdefineret farve
settings-viewing-background-color = Brugerdefineret baggrundsfarve
settings-viewing-background-color-help = Bruges, når Baggrund er sat til ”Brugerdefineret farve”.
settings-viewing-fullscreen-background = Baggrund i fuld skærm
settings-viewing-fullscreen-background-help = Farven bag billedet i fuld skærm.
settings-viewing-transparency = Gennemsigtige områder
settings-viewing-transparency-help = Hvordan gennemsigtige dele af et billede tegnes.
settings-viewing-transparency-checkerboard = Skakternet
settings-viewing-transparency-background = Baggrundsfarve
settings-viewing-transparency-black = Sort
settings-viewing-transparency-white = Hvid
settings-viewing-pixel-grid-threshold = Vis pixelgitter fra
settings-viewing-pixel-grid-threshold-help = Tegn et gitter mellem pixel, når der er zoomet mindst så meget ind.
settings-viewing-smoothing-high-zoom = Forstørrede pixel
settings-viewing-smoothing-high-zoom-help = Hvordan pixel ser ud, når billedet vises større end { PERCENT(100) }.
settings-viewing-smoothing-high-zoom-auto = Bløde, derefter skarpe fra { PERCENT(400) }
settings-viewing-smoothing-high-zoom-smooth = Altid bløde
settings-viewing-smoothing-high-zoom-pixelated = Altid skarpe kvadrater
settings-viewing-double-click = Dobbeltklik på billedet
settings-viewing-double-click-help = Dobbeltklik på den tomme baggrund slår altid fuld skærm til/fra.
settings-viewing-double-click-toggle-zoom = Skift mellem tilpas og { PERCENT(100) }
settings-viewing-double-click-fullscreen = Slå fuld skærm til/fra
settings-viewing-double-click-nothing = Gør ingenting
settings-viewing-fullscreen-on-open = Åbn billeder i fuld skærm
settings-viewing-fullscreen-on-open-help = Start i fuld skærm, når et billede åbnes uden for { -app-name }.
settings-viewing-fullscreen-mode = Type af fuld skærm
settings-viewing-fullscreen-mode-help = Systemets fulde skærm bruger sit eget Space; øjeblikkelig fuld skærm dækker det aktuelle uden animation.
settings-viewing-fullscreen-mode-native = Systemets (eget Space)
settings-viewing-fullscreen-mode-borderless = Øjeblikkelig
settings-viewing-toolbar-autohide = Skjul værktøjslinjen automatisk
settings-viewing-toolbar-autohide-help = Værktøjslinjen og kantpilene toner ud, når markøren står stille, og kommer tilbage, når den bevæges.
settings-viewing-toolbar-autohide-delay = Skjul værktøjslinjen efter
settings-viewing-toolbar-autohide-delay-help = Hvor længe markøren skal stå stille, før værktøjslinjen toner ud.
settings-viewing-toolbar-items = Knapper på værktøjslinjen
settings-viewing-toolbar-items-help = Hvilke knapper værktøjslinjen viser, i rækkefølge.
settings-viewing-toolbar-drag = Omarranger værktøjslinjen ved at trække
settings-viewing-toolbar-drag-help = Træk en knap langs værktøjslinjen for at flytte den. Et kort klik trykker stadig på den; Esc lægger en trukket knap tilbage.
settings-viewing-toolbar-items-open = Åbn
settings-viewing-toolbar-items-prev = Forrige billede
settings-viewing-toolbar-items-next = Næste billede
settings-viewing-toolbar-items-zoom-out = Zoom ud
settings-viewing-toolbar-items-zoom-level = Zoomniveau
settings-viewing-toolbar-items-zoom-in = Zoom ind
settings-viewing-toolbar-items-fit = Tilpas til vinduet
settings-viewing-toolbar-items-rotate-left = Roter mod uret
settings-viewing-toolbar-items-rotate-right = Roter med uret
settings-viewing-toolbar-items-info = Oplysninger
settings-viewing-toolbar-items-copy = Kopiér billede
settings-viewing-toolbar-items-delete = Flyt til papirkurven
settings-viewing-toolbar-items-palette = Kommandopalet
settings-viewing-toolbar-items-settings = Indstillinger
settings-viewing-toolbar-items-fullscreen = Fuld skærm
# machine-drafted, needs native review
settings-viewing-toolbar-items-spacer = Fleksibelt mellemrum
settings-viewing-toolbar-items-separator = Skillelinje
settings-toolbar-preview = Forhåndsvisning af værktøjslinjen
settings-toolbar-hidden = Skjulte knapper
settings-toolbar-add-spacer = Tilføj fleksibelt mellemrum
settings-toolbar-add-separator = Tilføj skillelinje
settings-toolbar-reset = Nulstil til standard
settings-toolbar-remove = Fjern { $item }
settings-toolbar-hint = Træk en række for at flytte den, eller vælg den og tryk Alt+↑ eller Alt+↓. Tab skifter mellem søgefeltet og listen.
settings-toolbar-move-up = Flyt op
settings-toolbar-move-down = Flyt ned
settings-toolbar-move-start = Flyt først
settings-toolbar-move-end = Flyt sidst
settings-toolbar-menu-hide = Skjul
settings-toolbar-menu-show = Vis
settings-toolbar-menu-remove = Fjern
settings-toolbar-row-menu = Flere handlinger for { $item }
settings-toolbar-position = Position { $index } af { $count }
settings-toolbar-hidden-state = Skjult

## Gennemsyn

settings-browsing-sort-order = Sortér billeder efter
settings-browsing-sort-order-help = Rækkefølgen, når du bladrer gennem en mappe. Navnesortering sætter ”img2” før ”img10” og følger dit sprogs alfabet.
settings-browsing-sort-order-natural = Navn
settings-browsing-sort-order-date-taken = Optagelsesdato
settings-browsing-sort-order-modified = Ændringsdato
settings-browsing-sort-order-size = Filstørrelse
settings-browsing-sort-order-type = Filtype
settings-browsing-sort-descending = Omvendt rækkefølge
settings-browsing-sort-descending-help = Sortér fra sidste til første.
settings-browsing-loop-folder = Start forfra ved mappens slutning
settings-browsing-loop-folder-help = Fortsæt med det første billede efter det sidste og omvendt.
settings-browsing-include-subfolders = Medtag undermapper
settings-browsing-include-subfolders-help = Bladr også gennem billeder i undermapper.
settings-browsing-hold-acceleration = Acceleration, når næste eller forrige holdes nede
settings-browsing-hold-acceleration-help = Hvor hurtigt bladringen accelererer, mens en tast eller knap holdes nede.
settings-browsing-hold-acceleration-off = Ingen acceleration
settings-browsing-hold-acceleration-gentle = Blød
settings-browsing-hold-acceleration-normal = Normal
settings-browsing-hold-acceleration-fast = Hurtig
settings-browsing-confirm-trash = Spørg før flytning til papirkurven
settings-browsing-confirm-trash-help = Flytning til papirkurven kan altid fortrydes, så { -app-name } spørger ikke som standard.
settings-browsing-arrows-follow-reading-direction = Piletaster følger læseretningen
settings-browsing-arrows-follow-reading-direction-help = I sprog, der læses fra højre mod venstre, går venstre piletast til næste billede.

## Farve

settings-colour-management = Farvestyring
settings-colour-management-help = Vis farver, som de var tænkt, ved hjælp af billedets farveprofil og skærmens profil.
settings-colour-display-profile = Skærmprofil
settings-colour-display-profile-help = Skærmens farveprofil. Automatisk bruger den, som operativsystemet angiver.
settings-colour-rendering-intent = Gengivelseshensigt
settings-colour-rendering-intent-help = Hvordan farver, som skærmen ikke kan vise, bringes inden for dens område.
settings-colour-rendering-intent-perceptual = Perceptuel
settings-colour-rendering-intent-relative-colorimetric = Relativ kolorimetrisk
settings-colour-rendering-intent-saturation = Mætning
settings-colour-rendering-intent-absolute-colorimetric = Absolut kolorimetrisk
settings-colour-black-point-compensation = Sortpunktskompensation
settings-colour-black-point-compensation-help = Bevar detaljer i skyggerne, når skærmens sort er lysere end billedets.
settings-colour-untagged-images = Billeder uden farveprofil
settings-colour-untagged-images-help = Hvordan billeder, der ikke angiver deres farver, vises.
settings-colour-untagged-images-srgb = Behandl som sRGB
settings-colour-untagged-images-display = Vis uændret
settings-colour-dithering = Dithering
settings-colour-dithering-help = Tilføj fin blå støj ved reduktion af farvedybden for at undgå striber i himmel og forløb.
settings-colour-hdr-output = HDR-output
settings-colour-hdr-output-help = Vis HDR-billeder med fuld lysstyrke på skærme, der understøtter det. Automatisk bruger HDR, når skærmen angiver ekstra lysstyrke; ellers komprimeres lyse områder blødt, så de passer.
settings-colour-hdr-output-auto = Automatisk
settings-colour-hdr-output-on = Til
settings-colour-hdr-output-off = Fra
# machine-drafted, needs native review
settings-colour-ocio-config = OCIO-konfiguration
settings-colour-ocio-config-help = En OpenColorIO-konfiguration (.ocio) til billeder i EXR, Radiance HDR og PFM. Tom bruger den, som $OCIO angiver.
settings-colour-ocio-display = OCIO-skærm
settings-colour-ocio-display-help = Den skærm (display) i OCIO-konfigurationen, som svarer til din skærm.
settings-colour-ocio-display-default = Konfigurationens første
settings-colour-ocio-view = OCIO-visning
settings-colour-ocio-view-help = Hvordan scenelineære (scene-linear) billeder vises på den skærm, f.eks. en tonemappet visning.
settings-colour-ocio-view-default = Skærmens første
settings-colour-ocio-input = OCIO-inputfarverum
settings-colour-ocio-input-help = Farverummet for billedernes værdier.
settings-colour-ocio-input-default = Konfigurationens scene-linear-rolle
# machine-drafted, needs native review
settings-colour-proof-profile = Softproof-profil
settings-colour-proof-profile-help = Printerprofilen, som Fremkald brugte til softproof sidst.
settings-colour-proof-intent = Softproof-gengivelse
settings-colour-proof-intent-help = Hvordan farver, printeren ikke kan printe, hentes ind.
settings-colour-proof-intent-perceptual = Perceptuel
settings-colour-proof-intent-relative = Relativ kolorimetrisk
settings-colour-proof-bpc = Sortpunktskompensering i softproof
settings-colour-proof-bpc-help = Afbild sort til printerens sort.
settings-colour-proof-paper = Papirhvid i softproof
settings-colour-proof-paper-help = Vis papirets egen farve.
settings-colour-proof-ink = Blæksort i softproof
settings-colour-proof-ink-help = Vis, hvor mørk blækkets sort er.

## Metadata og privatliv

settings-metadata-show-info-on-open = Vis oplysningspanelet ved åbning
settings-metadata-show-info-on-open-help = Åbn panelet med kamera, dato og sted, hver gang et billede åbnes.
settings-metadata-info-detail = Oplysningspanelet viser
settings-metadata-info-detail-help = Oversigten forklarer de mest nyttige detaljer i et enkelt sprog; alle felter viser alt, der er gemt i filen.
settings-metadata-info-detail-summary = Oversigt
settings-metadata-info-detail-all = Alle felter
settings-metadata-reverse-geocoding = Vis stednavne for GPS-positioner
settings-metadata-reverse-geocoding-help = Slå den nærmeste by op i indbyggede data. Det virker offline: positioner forlader aldrig din computer.
settings-metadata-hide-gps = Skjul GPS-positioner på skærmen
settings-metadata-hide-gps-help = Vis ikke koordinater eller steder, f.eks. når du deler skærm. Filerne ændres ikke.
# machine-drafted, needs native review
settings-metadata-develop-sidecar = Gem fremkaldelsesredigeringer ved siden af billedet
# machine-drafted, needs native review
settings-metadata-develop-sidecar-help = Skriver IMG_1234.CR2.specular.xmp ved siden af filen, så redigeringerne følger med. Fra: redigeringerne bliver kun i biblioteket.
# machine-drafted, needs native review
settings-metadata-develop-import-lightroom = Importér Lightroom-indstillinger i Fremkald
# machine-drafted, needs native review
settings-metadata-develop-import-lightroom-help = Et foto med Lightroom-indstillinger og ingen redigering åbner med dem, omtrentligt. Fra: Fremkald viser en knap.
settings-metadata-units = Enheder
settings-metadata-units-help = Enheder for højde, afstande og hastigheder.
settings-metadata-units-auto = Følg områdeformater
settings-metadata-units-metric = Metrisk
settings-metadata-units-imperial = Britisk/amerikansk
# machine-drafted, needs native review
settings-metadata-basemap = Baggrundskort
settings-metadata-basemap-help = Kortet under fotosteder. Onlinekort henter fliser for det område, du ser på.
settings-metadata-basemap-off = Kun indbygget kort (offline)
settings-metadata-basemap-openstreetmap = OpenStreetMap (online)
settings-metadata-basemap-custom = Anden fliseserver (online)
settings-metadata-basemap-file = Min egen kortfil
settings-metadata-basemap-url = Fliseserverens adresse
settings-metadata-basemap-url-help = En adresse med {"{"}z{"}"}, {"{"}x{"}"} og {"{"}y{"}"}, for eksempel https://tiles.example.org/{"{"}z{"}"}/{"{"}x{"}"}/{"{"}y{"}"}.png
settings-metadata-basemap-path = Kortfil
settings-metadata-basemap-path-help = En MBTiles- eller PMTiles-fil. Den virker offline.
settings-metadata-basemap-cache = Cache til kortfliser
settings-metadata-basemap-cache-help = Diskplads til hentede fliser. De ældste fjernes først.
# machine-drafted, needs native review
settings-metadata-basemap-allowed = Tilladte fliseservere
settings-metadata-basemap-allowed-help = Kortservere, du har accepteret at hente fliser fra.

## Ydeevne

settings-performance-ram-cache = Hukommelsescache
settings-performance-ram-cache-help = Hukommelse til at holde afkodede billeder klar. Automatisk vælger ud fra, hvor meget hukommelse computeren har.
settings-performance-vram-cache = Cache i GPU-hukommelsen
settings-performance-vram-cache-help = GPU-hukommelse til billeder og fliser. Automatisk vælger ud fra din GPU.
settings-performance-prefetch-depth = Billeder, der forberedes på forhånd
settings-performance-prefetch-depth-help = Hvor mange billeder i den retning, du bevæger dig, der afkodes på forhånd.
settings-performance-decoder-threads = Afkodningstråde
settings-performance-decoder-threads-help = CPU-tråde til afkodning af billeder. Automatisk bruger alle ydelseskerner og tilpasser sig filen.
settings-performance-gpu-adapter = GPU
settings-performance-gpu-adapter-help = Hvilken GPU der bruges, når computeren har mere end én.
settings-performance-gpu-adapter-high-performance = Højeste ydeevne
settings-performance-gpu-adapter-low-power = Laveste strømforbrug
settings-performance-processing = Hvor billeder behandles
settings-performance-processing-help = Noget arbejde kan køre på CPU'en, GPU'en eller en hardwareafkoder. Automatisk måler denne computer og vælger den hurtigste måde for det billede, du ser på, og den mest økonomiske for arbejde i baggrunden. Hvad der er valgt på denne computer, og hvorfor, vises nedenfor.
settings-performance-processing-automatic = Automatisk
settings-performance-processing-prefer-gpu = Foretræk GPU'en
settings-performance-processing-cpu-only = Kun CPU
settings-performance-drag-preview = Forhåndsvisning under træk
settings-performance-drag-preview-help = Hvordan fremkaldelse viser billedet, mens du trækker i en skyder. Automatisk bruger et mindre billede, kun når computeren ikke kan følge med.
settings-performance-drag-preview-automatic = Automatisk
settings-performance-drag-preview-sharp = Altid skarp
settings-performance-drag-preview-fast = Altid hurtig

## Indstillinger → Ydeevne → Sådan kører { -app-name } på denne computer (vælgeren af
## behandlingsveje, DESIGN.md §12.3.1). $engine og $other er hw-report-engine-*-navnene (små
## bogstaver, bruges midt i en sætning); paths-engine-* er de selvstændige navne.
settings-paths-title = Sådan kører { -app-name } på denne computer
settings-paths-intro = { -app-name } tager tid på hver måde at udføre disse opgaver på denne computer og bruger den bedste. Tiderne gælder et foto på { $mpx } megapixel.
settings-paths-intro-measuring = Målingerne er ikke færdige endnu: { -app-name } måler, mens computeren er inaktiv.
settings-paths-intro-battery = På batteri: valgene prioriterer lavere energiforbrug.
settings-paths-keywords = GPU CPU NPU grafikkort hardwareacceleration afkoder benchmark hastighed ydeevne
paths-engine-cpu = CPU
paths-engine-cpu-parallel = CPU, alle kerner
paths-engine-gpu = GPU
paths-engine-hw-decoder = Hardwareafkoder
paths-engine-npu = NPU
settings-paths-your-setting = { $engine } (din indstilling)
settings-paths-measured-faster = Målt på denne computer: { $ratio } gange så hurtig som { $other }.
settings-paths-measured-similar = Målt på denne computer: omtrent lige så hurtig som { $other }.
settings-paths-measured-power = På batteri bruger dette mindst energi (målt på denne computer).
settings-paths-not-measured = Ikke målt endnu: det sædvanlige valg for denne type computer.
settings-paths-set-by = Valgt af din indstilling ”{ $setting }: { $option }”.
settings-paths-blocked-check = Slået fra den { $date }: resultatet fra { $engine } stemte ikke med CPU'ens.
settings-paths-blocked-error = Slået fra den { $date } efter en fejl i { $engine }.
settings-paths-blocked-lost = Slået fra den { $date }, efter at { $engine } holdt op med at svare.
settings-paths-blocked-undated = Slået fra efter en fejl i { $engine }.
settings-paths-checking = Resultatet fra { $engine } kontrolleres mod CPU'en, før det bruges til det billede, du ser på.
settings-paths-exploring = Afprøver { $engine } i baggrunden nu.
settings-paths-unavailable = Denne computer har ingen { $engine } til dette.
settings-paths-background-cheaper = I baggrunden: { $engine }, som er målt til at bruge { $percent } mindre CPU-tid.
settings-paths-background-power = I baggrunden: { $engine } for at spare energi.
settings-paths-background = I baggrunden: { $engine }.
settings-paths-detail-measured = { $engine }: { $time } ms, { $cpu } ms CPU-tid, { $runs ->
    [one] { $runs } måling
   *[other] { $runs } målinger
} ({ $confidence })
settings-paths-detail-guess = { $engine }: ca. { $time } ms (skøn)
settings-paths-confidence-low = lav sikkerhed
settings-paths-confidence-medium = middel sikkerhed
settings-paths-confidence-high = høj sikkerhed
settings-paths-machine = Målt for { $gpu } · { $driver } · { $cpu }
settings-paths-retry = Prøv igen
settings-paths-measure-again = Mål igen
settings-paths-measure-again-question = Vil du glemme det, der er målt på denne computer, herunder de fravalgte veje, og måle igen, mens computeren er inaktiv?
settings-paths-measure-again-confirm = Glem målinger
settings-performance-display-cache = Gem en visningscache på disken
settings-performance-display-cache-help = Gem skærmstore kopier af billeder, så mapper, du har set, åbnes med det samme.
settings-performance-display-cache-size = Størrelse på visningscache
settings-performance-display-cache-size-help = De ældste poster fjernes, når cachen er fuld.
settings-performance-display-cache-location = Placering af visningscache
settings-performance-display-cache-location-help = En mappe på et hurtigt drev fungerer bedst.
settings-performance-fast-simd = Brug SIMD-instruktioner
settings-performance-fast-simd-help = Brug CPU'ens vektorinstruktioner til afkodning. Slå kun fra for at fejlfinde et problem.
settings-performance-fast-dct-scaling = Afkod i skærmstørrelse
settings-performance-fast-dct-scaling-help = Afkod store JPEG-filer direkte i den størrelse, de vises i. Slå kun fra for at fejlfinde et problem.
settings-performance-fast-parallel-decode = Afkod på flere kerner
settings-performance-fast-parallel-decode-help = Fordel store billeder på flere CPU-kerner. Slå kun fra for at fejlfinde et problem.
settings-performance-fast-restart-index = Indekser store JPEG-filer til zoom
settings-performance-fast-restart-index-help = Husk, hvor hver del af en JPEG-fil starter, så indzoomede områder afkodes hurtigt. Slå kun fra for at fejlfinde et problem.
settings-performance-fast-yuv-upload = Konverter farver på GPU'en
settings-performance-fast-yuv-upload-help = Konverter JPEG-farver på GPU'en eller CPU'en. Automatisk vælger den, der er hurtigst på denne computer. Slå kun fra for at fejlfinde et problem.
settings-performance-fast-yuv-upload-auto = Automatisk
settings-performance-fast-yuv-upload-on = Til
settings-performance-fast-yuv-upload-off = Fra
settings-performance-fast-async-io = Asynkron fillæsning
settings-performance-fast-async-io-help = Læs på forhånd med operativsystemets hurtigste filgrænseflade. Slå kun fra for at fejlfinde et problem.
settings-performance-fast-mmap = Hukommelsestilknyttede filer
settings-performance-fast-mmap-help = Læs filer uden at kopiere dem. Slå kun fra for at fejlfinde et problem.
settings-performance-hardware-decode = Hardwareafkodning
settings-performance-hardware-decode-help = Lad GPU'ens indbyggede afkoder åbne JPEG-fotos. Automatisk bruger den, hvor det hjælper, typisk til billeder, der forberedes i baggrunden. Den kontrolleres først mod den normale afkoder, og alt, hvad den ikke kan håndtere, åbnes normalt.
settings-performance-hardware-decode-auto = Automatisk
settings-performance-hardware-decode-on = Til
settings-performance-hardware-decode-off = Fra
settings-performance-frame-time-hud = Billedtids-HUD
settings-performance-frame-time-hud-help = Vis afkodnings- og billedtider, og hvilke hurtige veje der bruges.
settings-performance-stall-warning = Rapportér hængende indlæsninger efter
settings-performance-stall-warning-help = Når et billede stadig indlæses efter så lang tid, skrives dets indlæsningstilstand og de ventende job til loggen (standardfejl eller SPECULAR_LOG_FILE).
# machine-drafted, needs native review
settings-performance-insights = Indsigt i ydeevne
settings-performance-insights-help = Vis “Hvorfor er det langsomt?” i hjørnet med det, der ville hjælpe, når billeder bliver ved med at åbne langsomt af samme grund. Det, der læres, bliver på denne computer.
settings-performance-insights-storage = Indsigt om langsomt lager
settings-performance-insights-storage-help = Vis indsigt, når læsning af filer (fra et netværksdrev, et langsomt drev eller arkiver) bliver ved med at forsinke åbningen af billeder.
settings-performance-insights-decoder = Indsigt om afkodning
settings-performance-insights-decoder-help = Vis indsigt, når billeder bliver ved med at blive afkodet på CPU'en, selvom en hardwareafkoder kunne hjælpe.
# machine-drafted, needs native review
settings-performance-insights-restore = Vis skjulte indsigter igen
settings-performance-insights-restore-help = Henter de indsigter tilbage, du skjulte med × på “Hvorfor er det langsomt?”, både for åbning af billeder og for miniaturer. Ikke tilgængelig, så længe ingen er skjult.

## Tilgængelighed

settings-accessibility-reduce-motion = Reducer bevægelse
settings-accessibility-reduce-motion-help = Erstat zoom-, glide- og toningsanimationer med øjeblikkelige skift.
settings-accessibility-reduce-motion-system = Følg systemet
settings-accessibility-reduce-motion-on = Til
settings-accessibility-reduce-motion-off = Fra
settings-accessibility-increase-contrast = Øg kontrasten
settings-accessibility-increase-contrast-help = Kraftigere kanter og tekst og mindre gennemsigtighed.
settings-accessibility-increase-contrast-system = Følg systemet
settings-accessibility-increase-contrast-on = Til
settings-accessibility-increase-contrast-off = Fra
settings-accessibility-text-scale = Tekststørrelse
settings-accessibility-text-scale-help = Tekstens størrelse i forhold til standardstørrelsen.
settings-accessibility-ui-scale = Skalering af brugerfladen
settings-accessibility-ui-scale-help = Størrelsen på hele brugerfladen, oven i skærmskaleringen i operativsystemet.
settings-accessibility-focus-ring-always = Vis altid fokusrammen
settings-accessibility-focus-ring-always-help = Vis, hvilket element der har tastaturfokus, også når du bruger musen.
settings-accessibility-screen-reader-verbosity = Detaljer for skærmlæser
# machine-drafted, needs native review
settings-accessibility-screen-reader-verbosity-help = Hvor meget en skærmlæser oplyser, når du skifter mellem billeder, og hvor meget af aflæsningen under markøren den læser op.
settings-accessibility-screen-reader-verbosity-concise = Kort
settings-accessibility-screen-reader-verbosity-normal = Normal
settings-accessibility-screen-reader-verbosity-verbose = Detaljeret
# machine-drafted, needs native review
settings-accessibility-announce-cursor = Læs værdien under markøren
settings-accessibility-announce-cursor-help = En skærmlæser læser pixelværdien, HU, temperaturen eller positionen under markøren eller tastaturkrydshåret, når det holder op med at bevæge sig.

## Privatliv

# machine-drafted, needs native review
settings-privacy-no-network = Intet netværk
# machine-drafted, needs native review
settings-privacy-no-network-help = Fra: kun disse opretter forbindelse, hver først når du slår den til eller åbner noget med den: opdateringstjek, download af lokale AI-modeller, onlinekort, billeder fra en webadresse eller skylager og servere, du tilføjer (DICOMweb, OMERO, kort). Til: intet opretter forbindelse.

## Avanceret

settings-advanced-portable-mode = Bærbar tilstand
settings-advanced-portable-mode-help = Gem indstillinger og cache i mappen ved siden af programmet i stedet for i din brugerprofil.
settings-advanced-log-level = Logdetaljer
settings-advanced-log-level-help = Hvor meget { -app-name } skriver til sin logfil.
settings-advanced-log-level-error = Kun fejl
settings-advanced-log-level-warn = Advarsler
settings-advanced-log-level-info = Oplysninger
settings-advanced-log-level-debug = Fejlfinding
settings-advanced-log-level-trace = Alt
settings-advanced-developer-mode = Udviklertilstand
settings-advanced-developer-mode-help = Vis værktøjer til udvikling og test af { -app-name }, f.eks. pseudosprogene i sproglisten.
# machine-drafted, needs native review
settings-advanced-audit-log = Lokal revisionslog
# machine-drafted, needs native review
settings-advanced-audit-log-help = Registrer åbnede medicinske billeder, eksporter, anonymiseringer, udskrifter og afsendelser til et PACS i audit.jsonl i datamappen. Den bliver på denne computer.
settings-advanced-hw-report = Hardwarerapport…
settings-advanced-hw-report-help = Vis, hvilke CPU- og GPU-funktioner { -app-name } har fundet, og hvilke hurtige veje der bruges.
# machine-drafted, needs native review
settings-advanced-renderer-info = Rendererinformation…
settings-advanced-renderer-info-help = Vis grafikadapteren, driveren, skærmformaterne og hukommelsesgrænserne, { -app-name } bruger, klar til at kopiere ind i en fejlrapport.
settings-advanced-edit-settings-file = Rediger settings.toml
settings-advanced-edit-settings-file-help = Rediger indstillingsfilen i { -app-name }s egen editor. Den kontrollerer filen, mens du skriver, og gemmer den kun, når den er fejlfri.
settings-advanced-import = Importér indstillinger…
settings-advanced-import-help = Indlæs indstillinger fra en fil, der er eksporteret fra { -app-name }.
settings-advanced-export = Eksportér indstillinger…
settings-advanced-export-help = Gem alle indstillinger i én fil, til sikkerhedskopi eller en anden computer.
settings-advanced-reset-all = Nulstil alle indstillinger…
settings-advanced-reset-all-help = Sæt alle indstillinger tilbage til standard. Dette kan fortrydes.

## settings.toml editor (machine-drafted, needs native review)

config-editor-title = settings.toml
config-editor-save = Gem
config-editor-revert = Gendan
config-editor-close = Luk
config-editor-modified = Ændret
config-editor-text = Indstillingsfil
config-editor-position = Linje { $line }, kolonne { $column }
config-editor-saved = settings.toml er gemt og anvendt.
config-editor-cant-save =
    { $count ->
        [one] Kan ikke gemme: 1 problem. Ret det først.
       *[other] Kan ikke gemme: { $count } problemer. Ret dem først.
    }
config-editor-save-failed = Kunne ikke gemme settings.toml: { $reason }
config-editor-conflict = settings.toml er ændret et andet sted.
config-editor-reload = Genindlæs
config-editor-keep-mine = Behold mine
config-editor-discard-question = Kassér ikke-gemte ændringer i settings.toml?
config-editor-discard = Kassér
config-editor-keep-editing = Fortsæt redigering
config-editor-find-placeholder = Søg i filen
config-editor-find-count = { $current } af { $total }
config-editor-find-none = Ingen resultater
config-editor-find-previous = Forrige resultat
config-editor-find-next = Næste resultat
config-editor-find-close = Luk søgning
config-editor-problems = Problemer
config-editor-problems-count =
    { $count ->
        [0] Ingen problemer
        [one] 1 problem
       *[other] { $count } problemer
    }
config-editor-no-problems = Ingen problemer. Filen kan gemmes.
config-editor-problem-at = Linje { $line }: { $message }
config-editor-severity-error = Fejl
config-editor-severity-warning = Advarsel
config-editor-severity-info = Bemærkning
config-editor-problem-syntax = Ugyldig TOML: { $reason }
config-editor-problem-duplicate = { $key } er angivet to gange i samme tabel. Fjern den ene.
config-editor-problem-unknown-section = Der findes ingen indstillingssektion [{ $key }].
config-editor-problem-unknown-section-suggestion = Der findes ingen indstillingssektion [{ $key }]. Mente du [{ $suggestion }]?
config-editor-problem-unknown-key = Der findes ingen indstilling { $key }.
config-editor-problem-unknown-key-suggestion = Der findes ingen indstilling { $key }. Mente du { $suggestion }?
config-editor-problem-invalid = Ugyldig værdi for { $key }: { $reason }
config-editor-problem-migrated = { $key } er skrevet, som en ældre version af { -app-name } skrev det. Det konverteres ved læsning.
config-editor-problem-newer-version = Filen er skrevet af en nyere version af { -app-name } (format { $found }; denne version læser { $supported }). Ukendte indstillinger bevares.
config-editor-problem-command = Kommando { $index } i [[commands]]: { $reason }
config-editor-problem-hook = Krog { $index } i [[hooks]]: { $reason }

## keymap.toml in the same editor (machine-drafted, needs native review)

config-editor-keymap-title = keymap.toml
config-editor-keymap-text = Fil med tastaturgenveje
config-editor-keymap-saved = keymap.toml er gemt og anvendt.
config-editor-keymap-save-failed = Kan ikke gemme keymap.toml: { $reason }
config-editor-keymap-conflict = keymap.toml er ændret et andet sted.
config-editor-keymap-discard-question = Kassér dine ikke-gemte ændringer i keymap.toml?
config-editor-new-file = ny fil, oprettes ved gemning
config-editor-no-folder = der er ingen indstillingsmappe at gemme den i.

## Reset all settings confirmation (machine-drafted, needs native review)

settings-reset-all-confirm = Nulstil alle indstillinger til standard?
settings-reset-all-confirm-detail = Det påvirker alle dele af Indstillinger. Det kan fortrydes bagefter.
settings-reset-all-confirm-ok = Nulstil alt
settings-reset-all-done = Alle indstillinger er nulstillet til standard.

## Hardware video encoding (machine-drafted, needs native review)
paths-engine-hw-encoder = Hardwarekoder

## Path timing units (machine-drafted, needs native review)
settings-paths-checking-before-use = Resultatet fra { $engine } kontrolleres mod CPU'en, før det bruges.
settings-paths-unit-thumbnail = Tiderne gælder én miniature på { $size }
settings-paths-unit-frame = Tiderne gælder pr. videobillede på { $size }
# machine-drafted, needs native review
settings-metadata-develop-calibration = Mappe med kalibreringsbilleder
# machine-drafted, needs native review
settings-metadata-develop-calibration-help = Flat- og dark-billeder til Fremkald. Fil- eller mappenavne indeholder "flat" eller "dark".

## Colour swatches under a colour setting's field (machine-drafted, needs native review)
settings-colour-white = Hvid
settings-colour-light-grey = Lysegrå
settings-colour-grey = Grå
settings-colour-dark-grey = Mørkegrå
settings-colour-black = Sort
settings-colour-yellow = Gul
settings-colour-orange = Orange
settings-colour-red = Rød
settings-colour-magenta = Magenta
settings-colour-blue = Blå
settings-colour-cyan = Cyan
settings-colour-green = Grøn
