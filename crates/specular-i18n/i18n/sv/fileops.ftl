# machine-drafted, needs native review
# Filåtgärder: byt namn, byt namn på flera, kopiera och flytta till en mapp, rotation sparad
# i filen, ångra och gör om, skrivbordsbakgrund, dela, skicka via e-post, exportera och
# importera inställningar.

## Paletten, fusklappen och menyer

palette-undo = Ångra
palette-undo-help = Ta tillbaka den senaste ändringen av filer: flytt till papperskorgen, namnbyte, kopiering eller flytt, rotation.
palette-redo = Gör om
palette-redo-help = Gör om det som den senaste ångringen tog tillbaka.
palette-rename = Byt namn…
palette-rename-help = Ge filen ett nytt namn; sidecar-filer och RAW+JPEG-partnern följer med.
palette-batch-rename = Byt namn på filer…
palette-batch-rename-help = Byt namn på de markerade filerna, eller hela mappen, efter ett mönster, med förhandsvisning.
palette-copy-to-folder = Kopiera till mapp…
palette-copy-to-folder-help = Kopiera filen eller markeringen till en annan mapp.
palette-move-to-folder = Flytta till mapp…
palette-move-to-folder-help = Flytta filen eller markeringen till en annan mapp.
palette-rotate-file-left = Rotera filen åt vänster
palette-rotate-file-left-help = Vrid själva filen ett kvarts varv åt vänster, förlustfritt där formatet tillåter.
palette-rotate-file-right = Rotera filen åt höger
palette-rotate-file-right-help = Vrid själva filen ett kvarts varv åt höger, förlustfritt där formatet tillåter.
palette-set-wallpaper-help = Använd bilden som skrivbordsbakgrund.
palette-share = Dela…
palette-share-help = Skicka filen till en annan app med systemets delningsmeny.
palette-send-by-email = Skicka via e-post…
palette-send-by-email-help = Skapa ett nytt e-postmeddelande i din e-postapp med filen bifogad.
# machine-drafted, needs native review
palette-share-with-zynk = Dela med Zynk…
palette-share-with-zynk-help = Skapa en webblänk till filen i Zynk-appen.
palette-send-with-zynk = Skicka med Zynk…
palette-send-with-zynk-help = Skicka filen till en kontakt eller till en av dina enheter med Zynk.
palette-export-settings = Exportera inställningar…
palette-export-settings-help = Spara alla inställningar och dina kortkommandon i en fil.
palette-import-settings = Importera inställningar…
palette-import-settings-help = Läs inställningar från en fil, efter att ha visat vad som skulle ändras.

disabled-in-archive-file = Sidan ligger i ett arkiv och har ingen egen fil
disabled-not-rotatable = Den här filtypen kan inte spara en rotation
disabled-not-on-this-desktop = Ditt skrivbord erbjuder inte detta

## Inställningar

settings-browsing-rotate-trim-edges = Beskär JPEG-kanter för att rotera förlustfritt
settings-browsing-rotate-trim-edges-help = En JPEG vars storlek inte är en multipel av 8 eller 16 pixlar kan bara vridas förlustfritt genom att de sista pixlarna skärs bort. Utan detta ändras i stället orienteringstaggen, och beskärningen erbjuds varje gång.
settings-browsing-rotate-keep-modified = Behåll ändringsdatum vid rotation
settings-browsing-rotate-keep-modified-help = En roterad fil behåller datumet då den senast ändrades, så den stannar på sin plats vid sortering efter datum.
settings-browsing-recent-folders = Senaste mappar
settings-browsing-recent-folders-help = Mappar som senast användes för kopiering och flytt.

## Dialogrutor

fileop-cancel = Avbryt
rename-title = Byt namn
rename-field-label = Namn
rename-button = Byt namn
rename-sidecars-follow = Bytte också namn: { $files }
rename-extension-changed = Att ändra filändelsen konverterar inte filen; andra appar kanske inte kan öppna den.
rename-problem-empty = Skriv ett namn.
rename-problem-invalid-char = Namn får inte innehålla ”{ $char }”.
rename-problem-reserved = ”{ $name }” är ett namn som Windows reserverar för enheter.
rename-problem-too-long = Namnet är för långt; namn får vara högst 255 byte.
rename-problem-dot = ”.” och ”..” kan inte användas som namn.
rename-problem-trailing = Namn får inte sluta med mellanslag eller punkt.
rename-problem-exists = En annan fil har redan det här namnet.
rename-problem-read-only = Den här mappen kan inte ändras.
rename-problem-duplicate = Två filer skulle få det här namnet.

batch-rename-title =
    { $count ->
        [one] Byt namn på { $count } fil
       *[other] Byt namn på { $count } filer
    }
batch-rename-field-label = Mönster
batch-rename-tokens = Använd { "{" }name{ "}" }, { "{" }n:3{ "}" } (001, 002, …), { "{" }date:%Y-%m-%d{ "}" }, { "{" }camera{ "}" }, { "{" }w{ "}" }×{ "{" }h{ "}" } och { "{" }ext{ "}" }.
batch-rename-summary =
    { $count ->
        [one] { $count } fil får nytt namn
       *[other] { $count } filer får nya namn
    }{ $problems ->
        [0] .
        [one] ; { $problems } kan inte och förblir som den är.
       *[other] ; { $problems } kan inte och förblir som de är.
    }
batch-rename-reading = Läser datum och kameror…
batch-rename-preview = Förhandsvisning
batch-rename-unchanged = behåller sitt namn
batch-rename-more =
    { $count ->
        [one] { $count } till nedan
       *[other] { $count } till nedan
    }

transfer-copy-title-one = Kopiera ”{ $file }” till en mapp
transfer-copy-title =
    { $count ->
        [one] Kopiera { $count } fil till en mapp
       *[other] Kopiera { $count } filer till en mapp
    }
transfer-move-title-one = Flytta ”{ $file }” till en mapp
transfer-move-title =
    { $count ->
        [one] Flytta { $count } fil till en mapp
       *[other] Flytta { $count } filer till en mapp
    }
transfer-recent = Senaste mappar
transfer-folder = Mapp
transfer-choose-folder = Välj en mapp…
transfer-copy-dialog = Kopiera till mapp
transfer-move-dialog = Flytta till mapp
transfer-sidecars =
    { $count ->
        [one] { $count } sidecar-fil följer med.
       *[other] { $count } sidecar-filer följer med.
    }
transfer-taken =
    { $count ->
        [one] { $count } namn används redan i { $folder }.
       *[other] { $count } namn används redan i { $folder }.
    }
transfer-when-taken = När ett namn redan används
transfer-skip = Hoppa över
transfer-keep-both = Behåll båda
transfer-replace-option = Ersätt
transfer-copy = Kopiera
transfer-move = Flytta
transfer-replace-title =
    { $count ->
        [one] Ersätta { $count } fil i { $folder }?
       *[other] Ersätta { $count } filer i { $folder }?
    }
transfer-replace-note = Filerna som finns där nu flyttas till papperskorgen. Ångra tar tillbaka dem.
transfer-replace = Ersätt
transfer-back = Tillbaka
transfer-stop = Stoppa

settings-file-import-title = Importera inställningar
settings-file-import-dialog = Importera inställningar
settings-file-export-dialog = Exportera inställningar
settings-file-import-changes = Ändringar
settings-file-import-change = { $before } → { $after }
settings-file-import-no-key = ingen tangent
settings-file-import-unknown = Inte en inställning i den här versionen; utelämnad.
settings-file-import-keymap = Kortkommandon
settings-file-import-nothing = Inget skulle ändras: de här inställningarna gäller redan.
settings-file-import-summary =
    { $count ->
        [one] { $count } ändring. Importen kan ångras i Inställningar.
       *[other] { $count } ändringar. Importen kan ångras i Inställningar.
    }
settings-file-import-secrets =
    { $count ->
        [one] { $count } post ser ut som ett lösenord eller en nyckel och importeras aldrig.
       *[other] { $count } poster ser ut som lösenord eller nycklar och importeras aldrig.
    }
settings-file-import-commands =
    { $count ->
        [one] { $count } användarkommando importeras inte: kommandon startar program. Kopiera det själv till inställningsfilen om du litar på det.
       *[other] { $count } användarkommandon importeras inte: kommandon startar program. Kopiera dem själv till inställningsfilen om du litar på dem.
    }
settings-file-import-button = Importera

## Resultat

fileop-renamed = Nytt namn: { $file }
fileop-renamed-count =
    { $count ->
        [one] Bytte namn på { $count } fil
       *[other] Bytte namn på { $count } filer
    }
fileop-copied =
    { $count ->
        [one] Kopierade { $count } fil till { $folder }
       *[other] Kopierade { $count } filer till { $folder }
    }
fileop-moved =
    { $count ->
        [one] Flyttade { $count } fil till { $folder }
       *[other] Flyttade { $count } filer till { $folder }
    }
fileop-skipped =
    { $count ->
        [one] { $count } hoppades över: namnet används redan där.
       *[other] { $count } hoppades över: namnen används redan där.
    }
fileop-stopped = Stoppat; det som är gjort förblir gjort.
fileop-failed-file =
    { $count ->
        [one] Det gick inte att kopiera eller flytta { $file }: { $reason }
       *[other] Det gick inte att kopiera eller flytta { $count } filer; den första, { $file }: { $reason }
    }
fileop-progress-copy =
    { $count ->
        [one] Kopierar { $count } fil… { PERCENT($percent) }
       *[other] Kopierar { $count } filer… { PERCENT($percent) }
    }
fileop-progress-move =
    { $count ->
        [one] Flyttar { $count } fil… { PERCENT($percent) }
       *[other] Flyttar { $count } filer… { PERCENT($percent) }
    }
fileop-done-status = Klart
fileop-busy = Den senaste filändringen pågår fortfarande; försök igen om en stund.
fileop-nothing-to-redo = Inget att göra om
fileop-undone = Ångrat: { $what }
fileop-redone = Gjort om: { $what }
fileop-kind-rename = namnbyte
fileop-kind-batch-rename = namnbyte på flera filer
fileop-kind-move = flytt till mapp
fileop-kind-copy = kopiering till mapp
fileop-kind-rotate = rotation
rename-failed = Det gick inte att byta namn: { $reason }

undo-problem-changed = Kan inte ångra: { $file } har ändrats sedan dess, och den ändringen skulle gå förlorad.
undo-problem-missing = Kan inte ångra: { $file } finns inte längre.
undo-problem-taken = Kan inte ångra: en annan fil heter nu { $file }.
undo-problem-io = Kan inte ångra ändringen av { $file }: { $reason }

rotate-done-lossless = Roterade { $file } helt förlustfritt.
rotate-done-lossless-trimmed = Roterade { $file } helt förlustfritt; kanten beskars till { $width } × { $height }.
rotate-done-tag = Roterade { $file } genom att ändra orienteringstaggen.
rotate-done-sidecar = Sparade rotationen av { $file } i { $sidecar }; själva RAW-filen ändras aldrig.
rotate-tag-unreadable = Bilddata kunde inte läsas tillräckligt långt för att vrida den.
rotate-tag-multi-picture = Den innehåller mer än en bild, och att vrida den första skulle förstöra de andra.
rotate-tag-trailer = Efter bilden finns data som en vridning skulle flytta.
rotate-tag-not-aligned = Att vrida pixlarna förlustfritt skulle beskära den till { $width } × { $height }.
rotate-trim-offer = Beskär och vrid
rotate-problem-unsupported = { $file } är en filtyp som inte kan spara en rotation.
rotate-problem-too-large = { $file } är för stor för att rotera.
rotate-problem-damaged = { $file } är skadad där orienteringen lagras: { $reason }
rotate-problem-io = Det gick inte att rotera { $file }: { $reason }

share-failed = Det gick inte att dela: { $reason }
email-failed = Det gick inte att starta e-postappen: { $reason }
# machine-drafted, needs native review
zynk-opening = Öppnar Zynk…
zynk-failed = Kunde inte nå Zynk: { $reason }
zynk-get-title-send = ”Skicka med Zynk” kräver appen Zynk
zynk-get-title-share = ”Dela med Zynk” kräver appen Zynk
zynk-get-about = Zynk är en gratis app för Mac, Windows, Linux, iPhone och Android. Den skickar foton, videor och hela mappar direkt till dina andra enheter och till andra personer, oavsett storlek. Överföringarna är totalsträckskrypterade och fortsätter där de slutade om anslutningen bryts. För en mottagare som inte har Zynk skapar den en webblänk som filerna kan laddas ner från i webbläsaren.
zynk-get-macos = Hämta den från Mac App Store. När den är installerad lämnar { -app-name } dina filer direkt till den.
zynk-get-windows = Hämta den från Microsoft Store. När den är installerad lämnar { -app-name } dina filer direkt till den.
zynk-get-linux = Zynk för Linux är en AppImage, utan något att installera: ladda ned den, lägg den i ~/Applications och gör den körbar (i dess egenskaper, eller med chmod +x). { -app-name } hittar den där, och var som helst annars medan Zynk körs.
zynk-get = Hämta Zynk
zynk-send-old-windows = ”Skicka med Zynk” kräver Windows 10 version 2004 eller senare. ”Dela med Zynk” fungerar på den här Windows-versionen.
settings-file-export-done = Inställningarna sparades i { $file }
settings-file-export-failed = Det gick inte att spara inställningarna: { $reason }
settings-file-import-unreadable = Det gick inte att läsa { $file }: { $reason }
settings-file-import-done =
    { $count ->
        [one] Importerade { $count } ändring
       *[other] Importerade { $count } ändringar
    }
