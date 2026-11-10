# machine-drafted, needs native review

settings-section-decoders = Valfria avkodare
settings-decoders-look-again = Sök igen
settings-decoders-look-again-help = Sök efter avkodare som installerats sedan dess.
    { -app-name } söker också varje gång fönstret kommer fram.
settings-decoders-braw = Använd Blackmagic RAW SDK
settings-decoders-braw-help = Blackmagic RAW-klipp öppnas med det när det är installerat.
settings-decoders-braw-path = Blackmagic RAW SDK
settings-decoders-braw-path-help = Lämna tomt för att hitta det där installationsprogrammet lägger det.
settings-decoders-dwg = Använd ODA File Converter
settings-decoders-dwg-help = DWG-ritningar öppnas med det när det är installerat.
    En kopia av ritningen konverteras i en privat mapp, som sedan raderas.
settings-decoders-dwg-path = Programmet ODA File Converter
settings-decoders-dwg-path-help = Lämna tomt för att hitta det där installationsprogrammet lägger det.
settings-decoders-occt = Använd OpenCASCADE
settings-decoders-occt-help = STEP- och IGES-modeller öppnas med det när det är installerat.
settings-decoders-occt-path = OpenCASCADE-bibliotek
settings-decoders-occt-path-help = Lämna tomt för att hitta dem där systemet har sina bibliotek.
# machine-drafted, needs native review
settings-decoders-czi = Använd libCZI
settings-decoders-czi-help = Zeiss CZI-filer öppnas med det när det är installerat.
    Det körs i { -app-name }s isolerade hjälpprocess, utan åtkomst till dina andra filer.
settings-decoders-czi-path = Mapp för libCZI-biblioteket
settings-decoders-czi-path-help = Lämna tomt för att hitta det där systemet lagrar bibliotek.

decoders-intro = Vissa format öppnas med gratis programvara från deras tillverkare.
    Installera den en gång: { -app-name } hittar och använder den. Programmet laddar aldrig ned den.
decoders-keywords = valfria avkodare tillverkare SDK codec insticksprogram Blackmagic RAW BRAW RED R3D DWG ODA File Converter OpenCASCADE STEP IGES Zeiss CZI libCZI behöver avkodare
decoders-row = { $format } ({ $extensions })
decoders-state-found = Hittad
decoders-state-off = Av
decoders-state-not-found = Inte installerad
decoders-state-broken = Kan inte användas
decoders-state-refused = Inte möjligt
decoders-state-planned = Inte tillgänglig än
decoders-state-checking = Söker…
decoders-uses = Öppnas med { $decoder } från { $vendor }.
decoders-not-found = Den är gratis. Installera den och tryck sedan på Sök igen.
decoders-off = Avstängd: de här filerna öppnas inte.
decoders-version = Version { $version }.
decoders-r3d-why = REDs licens låter bara program som byggts med dess SDK, enligt REDs egna villkor, läsa R3D-filer.
    { -app-name } kan inte godta de villkoren, så R3D-filer öppnas inte.
decoders-ecw-why = Hexagons licens tillåter bara program som levereras med ECW SDK att använda det, inte en kopia du själv har installerat.
    Därför öppnas inte ECW-filer. JPEG 2000-filer från ECW SDK öppnas.
decoders-mrsid-why = MrSID SDK:s licens tillåter bara program som levereras med SDK:t att använda det, inte en kopia du själv har installerat.
    Därför öppnas inte MrSID-filer.
# machine-drafted, needs native review
# DJI radiometric JPEGs (drone thermal cameras): the photo and metadata open; the
# temperatures would need DJI's SDK.
decoders-dji-thermal-why = DJI:s SDK-licens tillåter bara program som levereras med SDK:t att använda det, inte en kopia du själv har installerat. Fotot och dess metadata öppnas ändå.
decoders-occt-why = STEP- och IGES-solider kräver OpenCASCADE, som den här versionen av { -app-name } inte kan använda.
decoders-get-row = Tillverkarens nedladdningssida
decoders-get = Hämta
decoders-locate-row = Installerad någon annanstans?
decoders-locate = Leta upp…
decoders-forget-row = Du valde den här själv.
decoders-forget = Glöm…
decoders-forget-ask = Glömma ditt val av { $decoder }?
decoders-forget-detail = { -app-name } letar efter den själv igen. Inget tas bort från disken.
decoders-forget-ok = Glöm
decoders-locate-title = Leta upp { $decoder }
decoders-locate-wrong = Det där är inte { $decoder }.
decoders-located = Använder ditt val av { $decoder }.
decoders-forgotten = { -app-name } letar efter { $decoder } själv igen.
decoders-still-missing = { $decoder } hittas fortfarande inte.
decoders-now-found = Hittade { $decoder }.
decoders-looked = Sökte efter avkodare igen.

decoders-card-title = { $format } behöver den kostnadsfria avkodaren från { $vendor }
decoders-card-what = { $file } öppnas med { $decoder }. Den är gratis.
decoders-card-how = Installera den och kom sedan tillbaka: { -app-name } hittar den själv.
decoders-card-get = Hämta
decoders-card-installed = Jag har installerat den
decoders-card-locate = Leta upp…

gallery-badge-needs-decoder = Behöver avkodare
gallery-needs-decoder = Öppnas med { $decoder } från { $vendor }, gratis men inte installerad.

error-reason-needs-decoder = { $format }-filer behöver { $decoder } från { $vendor }, gratis men inte installerad.
