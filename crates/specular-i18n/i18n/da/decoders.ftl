# machine-drafted, needs native review

settings-section-decoders = Valgfrie dekodere
settings-decoders-look-again = Søg igen
settings-decoders-look-again-help = Søg efter dekodere, der er installeret siden.
    { -app-name } søger også, hver gang vinduet kommer frem.
settings-decoders-braw = Brug Blackmagic RAW SDK
settings-decoders-braw-help = Blackmagic RAW-klip åbnes med det, når det er installeret.
settings-decoders-braw-path = Blackmagic RAW SDK
settings-decoders-braw-path-help = Lad feltet være tomt for at finde det, hvor installationsprogrammet lægger det.
settings-decoders-dwg = Brug ODA File Converter
settings-decoders-dwg-help = DWG-tegninger åbnes med det, når det er installeret.
    En kopi af tegningen konverteres i en privat mappe, som derefter slettes.
settings-decoders-dwg-path = Programmet ODA File Converter
settings-decoders-dwg-path-help = Lad feltet være tomt for at finde det, hvor installationsprogrammet lægger det.
settings-decoders-occt = Brug OpenCASCADE
settings-decoders-occt-help = STEP- og IGES-modeller åbnes med det, når det er installeret.
settings-decoders-occt-path = OpenCASCADE-biblioteker
settings-decoders-occt-path-help = Lad feltet være tomt for at finde dem, hvor systemet har sine biblioteker.
# machine-drafted, needs native review
settings-decoders-czi = Brug libCZI
settings-decoders-czi-help = Zeiss CZI-filer åbnes med det, når det er installeret.
    Det kører i { -app-name }s sandkasse-hjælper uden adgang til dine andre filer.
settings-decoders-czi-path = Mappe med libCZI-biblioteket
settings-decoders-czi-path-help = Lad feltet være tomt for at finde det, hvor systemet gemmer biblioteker.

decoders-intro = Nogle formater åbnes med gratis software fra deres producenter.
    Installér det én gang: { -app-name } finder og bruger det. Programmet downloader det aldrig.
decoders-keywords = valgfrie dekodere producent SDK codec plugin Blackmagic RAW BRAW RED R3D DWG ODA File Converter OpenCASCADE STEP IGES Zeiss CZI libCZI kræver dekoder
decoders-row = { $format } ({ $extensions })
decoders-state-found = Fundet
decoders-state-off = Fra
decoders-state-not-found = Ikke installeret
decoders-state-broken = Kan ikke bruges
decoders-state-refused = Ikke muligt
decoders-state-planned = Ikke tilgængelig endnu
decoders-state-checking = Søger…
decoders-uses = Åbnes med { $decoder } fra { $vendor }.
decoders-not-found = Det er gratis. Installér det, og tryk så på Søg igen.
decoders-off = Slået fra: disse filer åbnes ikke.
decoders-version = Version { $version }.
decoders-r3d-why = REDs licens lader kun programmer, der er bygget med deres SDK på REDs egne vilkår, læse R3D-filer.
    { -app-name } kan ikke acceptere de vilkår, så R3D-filer åbnes ikke.
decoders-ecw-why = Hexagons licens tillader kun programmer, der leverer deres ECW SDK, at bruge det – ikke en kopi, du selv har installeret.
    Derfor åbnes ECW-filer ikke. JPEG 2000-filer fra ECW SDK åbnes.
decoders-mrsid-why = MrSID SDK’ens licens tillader kun programmer, der leverer SDK’et, at bruge det – ikke en kopi, du selv har installeret.
    Derfor åbnes MrSID-filer ikke.
# machine-drafted, needs native review
# DJI radiometric JPEGs (drone thermal cameras): the photo and metadata open; the
# temperatures would need DJI's SDK.
decoders-dji-thermal-why = DJI’s SDK-licens tillader kun programmer, der leverer SDK’et, at bruge det – ikke en kopi, du selv har installeret. Fotoet og dets metadata åbnes stadig.
decoders-occt-why = STEP- og IGES-solider kræver OpenCASCADE, som denne version af { -app-name } ikke kan bruge.
decoders-get-row = Producentens downloadside
decoders-get = Hent
decoders-locate-row = Installeret et andet sted?
decoders-locate = Find…
decoders-forget-row = Du valgte denne selv.
decoders-forget = Glem…
decoders-forget-ask = Glem dit valg af { $decoder }?
decoders-forget-detail = { -app-name } leder selv efter det igen. Intet fjernes fra disken.
decoders-forget-ok = Glem
decoders-locate-title = Find { $decoder }
decoders-locate-wrong = Det er ikke { $decoder }.
decoders-located = Bruger dit valg af { $decoder }.
decoders-forgotten = { -app-name } leder selv efter { $decoder } igen.
decoders-still-missing = { $decoder } er stadig ikke fundet.
decoders-now-found = Fandt { $decoder }.
decoders-looked = Søgte efter dekodere igen.

decoders-card-title = { $format } kræver den gratis dekoder fra { $vendor }
decoders-card-what = { $file } åbnes med { $decoder }. Det er gratis.
decoders-card-how = Installér det, og kom så tilbage: { -app-name } finder det selv.
decoders-card-get = Hent
decoders-card-installed = Jeg har installeret det
decoders-card-locate = Find…

gallery-badge-needs-decoder = Kræver dekoder
gallery-needs-decoder = Åbnes med { $decoder } fra { $vendor }, gratis, men ikke installeret.

error-reason-needs-decoder = { $format }-filer kræver { $decoder } fra { $vendor }, gratis, men ikke installeret.
