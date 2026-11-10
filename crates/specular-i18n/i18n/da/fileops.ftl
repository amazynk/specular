# machine-drafted, needs native review
# Filhandlinger: omdøb, omdøb flere, kopiér og flyt til en mappe, rotation gemt i filen, fortryd og
# annuller fortryd, skrivebordsbaggrund, del, send med e-mail, eksport og import af indstillinger.

## Palet, oversigt og menuer

palette-undo = Fortryd
palette-undo-help = Tag den seneste ændring af filer tilbage: flytning til papirkurven, omdøbning, kopiering eller flytning, rotation.
palette-redo = Annuller fortryd
palette-redo-help = Gør igen, hvad den seneste fortrydelse tog tilbage.
palette-rename = Omdøb…
palette-rename-help = Giv filen et nyt navn; sidevognsfiler og RAW+JPEG-partner følger med.
palette-batch-rename = Omdøb filer…
palette-batch-rename-help = Omdøb de valgte filer, eller hele mappen, efter et mønster, med forhåndsvisning.
palette-copy-to-folder = Kopiér til mappe…
palette-copy-to-folder-help = Kopiér filen eller det valgte til en anden mappe.
palette-move-to-folder = Flyt til mappe…
palette-move-to-folder-help = Flyt filen eller det valgte til en anden mappe.
palette-rotate-file-left = Rotér fil til venstre
palette-rotate-file-left-help = Drej selve filen en kvart omgang til venstre, uden tab hvor formatet tillader det.
palette-rotate-file-right = Rotér fil til højre
palette-rotate-file-right-help = Drej selve filen en kvart omgang til højre, uden tab hvor formatet tillader det.
palette-set-wallpaper-help = Brug billedet som skrivebordsbaggrund.
palette-share = Del…
palette-share-help = Send filen til en anden app med systemets delingsmenu.
palette-send-by-email = Send med e-mail…
palette-send-by-email-help = Start en ny e-mail i din mailapp med filen vedhæftet.
# machine-drafted, needs native review
palette-share-with-zynk = Del med Zynk…
palette-share-with-zynk-help = Lav et weblink til filen i Zynk-appen.
palette-send-with-zynk = Send med Zynk…
palette-send-with-zynk-help = Send filen til en kontakt eller en af dine enheder med Zynk.
palette-export-settings = Eksportér indstillinger…
palette-export-settings-help = Gem alle indstillinger og dine tastaturgenveje i en fil.
palette-import-settings = Importér indstillinger…
palette-import-settings-help = Læs indstillinger fra en fil, efter at det er vist, hvad der ændres.

disabled-in-archive-file = Denne side ligger i et arkiv og har ingen fil for sig selv
disabled-not-rotatable = Denne filtype kan ikke gemme en rotation
disabled-not-on-this-desktop = Dit skrivebord tilbyder ikke dette

## Indstillinger

settings-browsing-rotate-trim-edges = Beskær JPEG-kanter for at rotere uden tab
settings-browsing-rotate-trim-edges-help = En JPEG, hvis størrelse ikke er et multiplum af 8 eller 16 pixel, kan kun drejes uden tab ved at skære de sidste pixel af. Uden dette ændres retningsmærket i stedet, og du tilbydes beskæringen hver gang.
settings-browsing-rotate-keep-modified = Bevar ændringsdatoen ved rotation
settings-browsing-rotate-keep-modified-help = En roteret fil beholder sin seneste ændringsdato, så den bliver på sin plads ved sortering efter dato.
settings-browsing-recent-folders = Seneste mapper
settings-browsing-recent-folders-help = Mapper brugt senest til kopiering og flytning.

## Dialoger

fileop-cancel = Annuller
rename-title = Omdøb
rename-field-label = Navn
rename-button = Omdøb
rename-sidecars-follow = Også omdøbt: { $files }
rename-extension-changed = Ændret filtypenavn konverterer ikke filen; andre apps kan måske ikke åbne den.
rename-problem-empty = Skriv et navn.
rename-problem-invalid-char = Navne må ikke indeholde “{ $char }”.
rename-problem-reserved = “{ $name }” er et navn, Windows reserverer til enheder.
rename-problem-too-long = Navnet er for langt; højst 255 byte.
rename-problem-dot = “.” og “..” kan ikke bruges som navne.
rename-problem-trailing = Navne må ikke slutte med mellemrum eller punktum.
rename-problem-exists = En anden fil har allerede dette navn.
rename-problem-read-only = Denne mappe kan ikke ændres.
rename-problem-duplicate = To filer ville få dette navn.

batch-rename-title =
    { $count ->
        [one] Omdøb { $count } fil
       *[other] Omdøb { $count } filer
    }
batch-rename-field-label = Mønster
batch-rename-tokens = Brug { "{" }name{ "}" }, { "{" }n:3{ "}" } (001, 002, …), { "{" }date:%Y-%m-%d{ "}" }, { "{" }camera{ "}" }, { "{" }w{ "}" }×{ "{" }h{ "}" } og { "{" }ext{ "}" }.
batch-rename-summary =
    { $count ->
        [one] { $count } fil bliver omdøbt
       *[other] { $count } filer bliver omdøbt
    }{ $problems ->
        [0] .
        [one] ; { $problems } kan ikke og forbliver uændret.
       *[other] ; { $problems } kan ikke og forbliver uændrede.
    }
batch-rename-reading = Læser datoer og kameraer…
batch-rename-preview = Forhåndsvisning
batch-rename-unchanged = beholder navnet
batch-rename-more =
    { $count ->
        [one] { $count } mere nedenfor
       *[other] { $count } mere nedenfor
    }

transfer-copy-title-one = Kopiér “{ $file }” til en mappe
transfer-copy-title =
    { $count ->
        [one] Kopiér { $count } fil til en mappe
       *[other] Kopiér { $count } filer til en mappe
    }
transfer-move-title-one = Flyt “{ $file }” til en mappe
transfer-move-title =
    { $count ->
        [one] Flyt { $count } fil til en mappe
       *[other] Flyt { $count } filer til en mappe
    }
transfer-recent = Seneste mapper
transfer-folder = Mappe
transfer-choose-folder = Vælg en mappe…
transfer-copy-dialog = Kopiér til mappe
transfer-move-dialog = Flyt til mappe
transfer-sidecars =
    { $count ->
        [one] { $count } sidevognsfil følger med.
       *[other] { $count } sidevognsfiler følger med.
    }
transfer-taken =
    { $count ->
        [one] { $count } navn er allerede brugt i { $folder }.
       *[other] { $count } navne er allerede brugt i { $folder }.
    }
transfer-when-taken = Når et navn allerede er brugt
transfer-skip = Spring over
transfer-keep-both = Behold begge
transfer-replace-option = Erstat
transfer-copy = Kopiér
transfer-move = Flyt
transfer-replace-title =
    { $count ->
        [one] Erstat { $count } fil i { $folder }?
       *[other] Erstat { $count } filer i { $folder }?
    }
transfer-replace-note = De nuværende filer dér flyttes til papirkurven. Fortryd henter dem tilbage.
transfer-replace = Erstat
transfer-back = Tilbage
transfer-stop = Stop

settings-file-import-title = Importér indstillinger
settings-file-import-dialog = Importér indstillinger
settings-file-export-dialog = Eksportér indstillinger
settings-file-import-changes = Ændringer
settings-file-import-change = { $before } → { $after }
settings-file-import-no-key = ingen tast
settings-file-import-unknown = Ikke en indstilling i denne version; udeladt.
settings-file-import-keymap = Tastaturgenveje
settings-file-import-nothing = Intet ville ændre sig: disse indstillinger gælder allerede.
settings-file-import-summary =
    { $count ->
        [one] { $count } ændring. Importen kan fortrydes i Indstillinger.
       *[other] { $count } ændringer. Importen kan fortrydes i Indstillinger.
    }
settings-file-import-secrets =
    { $count ->
        [one] { $count } post ligner en adgangskode eller nøgle og importeres aldrig.
       *[other] { $count } poster ligner adgangskoder eller nøgler og importeres aldrig.
    }
settings-file-import-commands =
    { $count ->
        [one] { $count } brugerkommando importeres ikke: kommandoer starter programmer. Kopiér den selv ind i din indstillingsfil, hvis du stoler på den.
       *[other] { $count } brugerkommandoer importeres ikke: kommandoer starter programmer. Kopiér dem selv ind i din indstillingsfil, hvis du stoler på dem.
    }
settings-file-import-button = Importér

## Resultater

fileop-renamed = Omdøbt til { $file }
fileop-renamed-count =
    { $count ->
        [one] Omdøbte { $count } fil
       *[other] Omdøbte { $count } filer
    }
fileop-copied =
    { $count ->
        [one] Kopierede { $count } fil til { $folder }
       *[other] Kopierede { $count } filer til { $folder }
    }
fileop-moved =
    { $count ->
        [one] Flyttede { $count } fil til { $folder }
       *[other] Flyttede { $count } filer til { $folder }
    }
fileop-skipped =
    { $count ->
        [one] { $count } blev sprunget over: navnet er allerede brugt dér.
       *[other] { $count } blev sprunget over: navnene er allerede brugt dér.
    }
fileop-stopped = Stoppet; det, der er gjort, forbliver gjort.
fileop-failed-file =
    { $count ->
        [one] Kunne ikke kopiere eller flytte { $file }: { $reason }
       *[other] Kunne ikke kopiere eller flytte { $count } filer; den første, { $file }: { $reason }
    }
fileop-progress-copy =
    { $count ->
        [one] Kopierer { $count } fil… { PERCENT($percent) }
       *[other] Kopierer { $count } filer… { PERCENT($percent) }
    }
fileop-progress-move =
    { $count ->
        [one] Flytter { $count } fil… { PERCENT($percent) }
       *[other] Flytter { $count } filer… { PERCENT($percent) }
    }
fileop-done-status = Færdig
fileop-busy = Den seneste filændring er stadig i gang; prøv igen om et øjeblik.
fileop-nothing-to-redo = Intet at gøre igen
fileop-undone = Fortrudt: { $what }
fileop-redone = Gjort igen: { $what }
fileop-kind-rename = omdøbning
fileop-kind-batch-rename = omdøbning af flere filer
fileop-kind-move = flytning til mappe
fileop-kind-copy = kopiering til mappe
fileop-kind-rotate = rotation
rename-failed = Kunne ikke omdøbe: { $reason }

undo-problem-changed = Kan ikke fortryde: { $file } er ændret siden, og en fortrydelse ville miste den ændring.
undo-problem-missing = Kan ikke fortryde: { $file } findes ikke længere.
undo-problem-taken = Kan ikke fortryde: en anden fil hedder nu { $file }.
undo-problem-io = Kan ikke fortryde ændringen af { $file }: { $reason }

rotate-done-lossless = Roterede { $file } helt uden tab.
rotate-done-lossless-trimmed = Roterede { $file } helt uden tab; kanten blev beskåret til { $width } × { $height }.
rotate-done-tag = Roterede { $file } ved at ændre retningsmærket.
rotate-done-sidecar = Gemte rotationen af { $file } i { $sidecar }; selve RAW-filen ændres aldrig.
rotate-tag-unreadable = Billeddataene kunne ikke læses langt nok til at dreje den.
rotate-tag-multi-picture = Den indeholder mere end ét billede, og at dreje det første ville ødelægge de andre.
rotate-tag-trailer = Der følger data efter billedet, som en drejning ville flytte.
rotate-tag-not-aligned = At dreje pixelene uden tab ville beskære den til { $width } × { $height }.
rotate-trim-offer = Beskær og drej
rotate-problem-unsupported = { $file } er en filtype, der ikke kan gemme en rotation.
rotate-problem-too-large = { $file } er for stor til at rotere.
rotate-problem-damaged = { $file } er beskadiget, hvor retningen er gemt: { $reason }
rotate-problem-io = Kunne ikke rotere { $file }: { $reason }

share-failed = Kunne ikke dele: { $reason }
email-failed = Kunne ikke starte din mailapp: { $reason }
# machine-drafted, needs native review
zynk-opening = Åbner Zynk…
zynk-failed = Kunne ikke nå Zynk: { $reason }
zynk-get-title-send = “Send med Zynk” kræver Zynk-appen
zynk-get-title-share = “Del med Zynk” kræver Zynk-appen
zynk-get-about = Zynk er en gratis app til Mac, Windows, Linux, iPhone og Android. Den sender billeder, videoer og hele mapper direkte til dine andre enheder og til andre mennesker, uanset størrelse. Overførsler er end-to-end-krypterede og fortsætter, hvor de slap, hvis forbindelsen afbrydes. For en modtager, der ikke har Zynk, laver den et weblink, som filerne kan downloades fra i browseren.
zynk-get-macos = Hent den i Mac App Store. Når den er installeret, giver { -app-name } dine filer direkte videre til den.
zynk-get-windows = Hent den i Microsoft Store. Når den er installeret, giver { -app-name } dine filer direkte videre til den.
zynk-get-linux = Zynk til Linux er en AppImage, uden noget at installere: download den, læg den i ~/Applications, og gør den eksekverbar (i dens egenskaber eller med chmod +x). { -app-name } finder den der, og alle andre steder, mens Zynk kører.
zynk-get = Hent Zynk
zynk-send-old-windows = “Send med Zynk” kræver Windows 10 version 2004 eller nyere. “Del med Zynk” virker på denne Windows-version.
settings-file-export-done = Indstillinger gemt i { $file }
settings-file-export-failed = Kunne ikke gemme indstillingerne: { $reason }
settings-file-import-unreadable = Kunne ikke læse { $file }: { $reason }
settings-file-import-done =
    { $count ->
        [one] Importerede { $count } ændring
       *[other] Importerede { $count } ændringer
    }
