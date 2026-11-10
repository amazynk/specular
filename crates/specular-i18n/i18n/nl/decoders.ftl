# machine-drafted, needs native review

settings-section-decoders = Optionele decoders
settings-decoders-look-again = Opnieuw zoeken
settings-decoders-look-again-help = Zoek naar decoders die sindsdien zijn geïnstalleerd.
    { -app-name } zoekt ook telkens als het venster naar voren komt.
settings-decoders-braw = Blackmagic RAW SDK gebruiken
settings-decoders-braw-help = Blackmagic RAW-clips openen ermee als het geïnstalleerd is.
settings-decoders-braw-path = Blackmagic RAW SDK
settings-decoders-braw-path-help = Laat leeg om het te vinden waar het installatieprogramma het neerzet.
settings-decoders-dwg = ODA File Converter gebruiken
settings-decoders-dwg-help = DWG-tekeningen openen ermee als het geïnstalleerd is.
    Een kopie van de tekening wordt omgezet in een privémap, die daarna wordt verwijderd.
settings-decoders-dwg-path = Programma ODA File Converter
settings-decoders-dwg-path-help = Laat leeg om het te vinden waar het installatieprogramma het neerzet.
settings-decoders-occt = OpenCASCADE gebruiken
settings-decoders-occt-help = STEP- en IGES-modellen openen ermee als het geïnstalleerd is.
settings-decoders-occt-path = OpenCASCADE-bibliotheken
settings-decoders-occt-path-help = Laat leeg om ze te vinden waar het systeem bibliotheken bewaart.
# machine-drafted, needs native review
settings-decoders-czi = libCZI gebruiken
settings-decoders-czi-help = Zeiss CZI-bestanden openen ermee als het geïnstalleerd is.
    Het draait in het afgeschermde hulpproces van { -app-name }, zonder toegang tot je andere bestanden.
settings-decoders-czi-path = Map van de libCZI-bibliotheek
settings-decoders-czi-path-help = Laat leeg om haar te vinden waar het systeem bibliotheken bewaart.

decoders-intro = Sommige formaten openen met gratis software van hun makers.
    Installeer die één keer: { -app-name } vindt en gebruikt hem. Het downloadt hem nooit.
decoders-keywords = optionele decoders leverancier SDK codec plug-in Blackmagic RAW BRAW RED R3D DWG ODA File Converter OpenCASCADE STEP IGES Zeiss CZI libCZI decoder nodig
decoders-row = { $format } ({ $extensions })
decoders-state-found = Gevonden
decoders-state-off = Uit
decoders-state-not-found = Niet geïnstalleerd
decoders-state-broken = Niet bruikbaar
decoders-state-refused = Niet mogelijk
decoders-state-planned = Nog niet beschikbaar
decoders-state-checking = Zoeken…
decoders-uses = Opent met { $decoder } van { $vendor }.
decoders-not-found = Het is gratis. Installeer het en druk dan op Opnieuw zoeken.
decoders-off = Uitgezet: deze bestanden openen niet.
decoders-version = Versie { $version }.
decoders-r3d-why = De licentie van RED laat alleen programma’s die met zijn SDK zijn gebouwd, onder RED’s eigen voorwaarden, R3D-bestanden lezen.
    { -app-name } kan die voorwaarden niet aannemen, dus R3D-bestanden openen niet.
decoders-ecw-why = De licentie van Hexagon staat de ECW SDK alleen toe in programma’s die hem meeleveren, niet een kopie die je zelf hebt geïnstalleerd.
    Daarom gaan ECW-bestanden niet open. JPEG 2000-bestanden uit de ECW SDK wel.
decoders-mrsid-why = De licentie van de MrSID SDK staat hem alleen toe in programma’s die hem meeleveren, niet een kopie die je zelf hebt geïnstalleerd.
    Daarom gaan MrSID-bestanden niet open.
# machine-drafted, needs native review
# DJI radiometric JPEGs (drone thermal cameras): the photo and metadata open; the
# temperatures would need DJI's SDK.
decoders-dji-thermal-why = De licentie van de DJI SDK staat hem alleen toe in programma’s die hem meeleveren, niet een kopie die je zelf hebt geïnstalleerd. De foto en de metadata gaan wel open.
decoders-occt-why = STEP- en IGES-solids hebben OpenCASCADE nodig, dat deze build van { -app-name } niet kan gebruiken.
decoders-get-row = De downloadpagina van de maker
decoders-get = Downloaden
decoders-locate-row = Ergens anders geïnstalleerd?
decoders-locate = Aanwijzen…
decoders-forget-row = Je hebt deze zelf gekozen.
decoders-forget = Vergeten…
decoders-forget-ask = Je keuze voor { $decoder } vergeten?
decoders-forget-detail = { -app-name } zoekt er weer zelf naar. Er wordt niets van de schijf verwijderd.
decoders-forget-ok = Vergeten
decoders-locate-title = { $decoder } aanwijzen
decoders-locate-wrong = Dat is niet { $decoder }.
decoders-located = Je keuze voor { $decoder } wordt gebruikt.
decoders-forgotten = { -app-name } zoekt weer zelf naar { $decoder }.
decoders-still-missing = { $decoder } is nog steeds niet gevonden.
decoders-now-found = { $decoder } gevonden.
decoders-looked = Opnieuw naar decoders gezocht.

decoders-card-title = { $format } heeft de gratis decoder van { $vendor } nodig
decoders-card-what = { $file } opent met { $decoder }. Het is gratis.
decoders-card-how = Installeer het en kom dan terug: { -app-name } vindt het zelf.
decoders-card-get = Downloaden
decoders-card-installed = Ik heb het geïnstalleerd
decoders-card-locate = Aanwijzen…

gallery-badge-needs-decoder = Decoder nodig
gallery-needs-decoder = Opent met { $decoder } van { $vendor }, gratis maar niet geïnstalleerd.

error-reason-needs-decoder = { $format }-bestanden hebben { $decoder } van { $vendor } nodig, gratis maar niet geïnstalleerd.
