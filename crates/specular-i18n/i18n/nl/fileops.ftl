# machine-drafted, needs native review
# Bestandsbewerkingen: naam wijzigen, meerdere namen wijzigen, kopiëren en verplaatsen naar
# een map, draaiing opgeslagen in het bestand, ongedaan maken en opnieuw, achtergrond, delen,
# versturen per e-mail, instellingen exporteren en importeren.

## Palet, spiekbrief en menu’s

palette-undo = Ongedaan maken
palette-undo-help = De laatste wijziging aan bestanden terugdraaien: verplaatsen naar de prullenbak, naam wijzigen, kopiëren of verplaatsen, draaien.
palette-redo = Opnieuw
palette-redo-help = Opnieuw doen wat de laatste ongedaanmaking terugdraaide.
palette-rename = Naam wijzigen…
palette-rename-help = Het bestand een nieuwe naam geven; sidecars en de RAW+JPEG-partner gaan mee.
palette-batch-rename = Namen van bestanden wijzigen…
palette-batch-rename-help = De geselecteerde bestanden, of de hele map, hernoemen met een patroon, met voorbeeld.
palette-copy-to-folder = Kopiëren naar map…
palette-copy-to-folder-help = Het bestand of de selectie naar een andere map kopiëren.
palette-move-to-folder = Verplaatsen naar map…
palette-move-to-folder-help = Het bestand of de selectie naar een andere map verplaatsen.
palette-rotate-file-left = Bestand linksom draaien
palette-rotate-file-left-help = Het bestand zelf een kwartslag linksom draaien, zonder verlies waar het formaat dat toelaat.
palette-rotate-file-right = Bestand rechtsom draaien
palette-rotate-file-right-help = Het bestand zelf een kwartslag rechtsom draaien, zonder verlies waar het formaat dat toelaat.
palette-set-wallpaper-help = De afbeelding als bureaubladachtergrond gebruiken.
palette-share = Delen…
palette-share-help = Het bestand naar een andere app sturen via het deelmenu van je systeem.
palette-send-by-email = Versturen per e-mail…
palette-send-by-email-help = Een nieuwe e-mail in je mailapp starten met het bestand als bijlage.
# machine-drafted, needs native review
palette-share-with-zynk = Delen met Zynk…
palette-share-with-zynk-help = Maak in de Zynk-app een weblink naar het bestand.
palette-send-with-zynk = Versturen met Zynk…
palette-send-with-zynk-help = Stuur het bestand met Zynk naar een contact of naar een van je apparaten.
palette-export-settings = Instellingen exporteren…
palette-export-settings-help = Alle instellingen en je sneltoetsen in een bestand opslaan.
palette-import-settings = Instellingen importeren…
palette-import-settings-help = Instellingen uit een bestand lezen, na te tonen wat er zou veranderen.

disabled-in-archive-file = Deze pagina zit in een archief en heeft geen eigen bestand
disabled-not-rotatable = Dit soort bestand kan geen draaiing vastleggen
disabled-not-on-this-desktop = Je bureaublad biedt dit niet

## Instellingen

settings-browsing-rotate-trim-edges = JPEG-randen bijsnijden om zonder verlies te draaien
settings-browsing-rotate-trim-edges-help = Een JPEG waarvan de afmetingen geen veelvoud van 8 of 16 pixels zijn, kan alleen zonder verlies worden gedraaid door de laatste pixels weg te snijden. Zonder deze optie wordt in plaats daarvan de oriëntatietag gewijzigd en wordt het bijsnijden telkens aangeboden.
settings-browsing-rotate-keep-modified = Wijzigingsdatum behouden bij draaien
settings-browsing-rotate-keep-modified-help = Een gedraaid bestand houdt de datum van de laatste wijziging, zodat het bij sorteren op datum op zijn plek blijft.
settings-browsing-recent-folders = Recente mappen
settings-browsing-recent-folders-help = Mappen die het laatst zijn gebruikt om te kopiëren en te verplaatsen.

## Vensters

fileop-cancel = Annuleren
rename-title = Naam wijzigen
rename-field-label = Naam
rename-button = Naam wijzigen
rename-sidecars-follow = Ook hernoemd: { $files }
rename-extension-changed = Een andere extensie zet het bestand niet om; andere apps openen het mogelijk niet.
rename-problem-empty = Typ een naam.
rename-problem-invalid-char = Namen mogen geen ‘{ $char }’ bevatten.
rename-problem-reserved = ‘{ $name }’ is een naam die Windows voor apparaten reserveert.
rename-problem-too-long = Die naam is te lang; namen mogen maximaal 255 bytes hebben.
rename-problem-dot = ‘.’ en ‘..’ kunnen niet als naam worden gebruikt.
rename-problem-trailing = Namen mogen niet eindigen op een spatie of een punt.
rename-problem-exists = Een ander bestand heeft deze naam al.
rename-problem-read-only = Deze map kan niet worden gewijzigd.
rename-problem-duplicate = Twee bestanden zouden deze naam krijgen.

batch-rename-title =
    { $count ->
        [one] Naam van { $count } bestand wijzigen
       *[other] Namen van { $count } bestanden wijzigen
    }
batch-rename-field-label = Patroon
batch-rename-tokens = Gebruik { "{" }name{ "}" }, { "{" }n:3{ "}" } (001, 002, …), { "{" }date:%Y-%m-%d{ "}" }, { "{" }camera{ "}" }, { "{" }w{ "}" }×{ "{" }h{ "}" } en { "{" }ext{ "}" }.
batch-rename-summary =
    { $count ->
        [one] { $count } bestand wordt hernoemd
       *[other] { $count } bestanden worden hernoemd
    }{ $problems ->
        [0] .
        [one] ; { $problems } kan dat niet en blijft zoals het is.
       *[other] ; { $problems } kunnen dat niet en blijven zoals ze zijn.
    }
batch-rename-reading = Datums en camera’s lezen…
batch-rename-preview = Voorbeeld
batch-rename-unchanged = houdt zijn naam
batch-rename-more =
    { $count ->
        [one] Nog { $count } hieronder
       *[other] Nog { $count } hieronder
    }

transfer-copy-title-one = ‘{ $file }’ naar een map kopiëren
transfer-copy-title =
    { $count ->
        [one] { $count } bestand naar een map kopiëren
       *[other] { $count } bestanden naar een map kopiëren
    }
transfer-move-title-one = ‘{ $file }’ naar een map verplaatsen
transfer-move-title =
    { $count ->
        [one] { $count } bestand naar een map verplaatsen
       *[other] { $count } bestanden naar een map verplaatsen
    }
transfer-recent = Recente mappen
transfer-folder = Map
transfer-choose-folder = Kies een map…
transfer-copy-dialog = Kopiëren naar map
transfer-move-dialog = Verplaatsen naar map
transfer-sidecars =
    { $count ->
        [one] { $count } sidecarbestand gaat mee.
       *[other] { $count } sidecarbestanden gaan mee.
    }
transfer-taken =
    { $count ->
        [one] { $count } naam is al in gebruik in { $folder }.
       *[other] { $count } namen zijn al in gebruik in { $folder }.
    }
transfer-when-taken = Als een naam al in gebruik is
transfer-skip = Overslaan
transfer-keep-both = Beide behouden
transfer-replace-option = Vervangen
transfer-copy = Kopiëren
transfer-move = Verplaatsen
transfer-replace-title =
    { $count ->
        [one] { $count } bestand in { $folder } vervangen?
       *[other] { $count } bestanden in { $folder } vervangen?
    }
transfer-replace-note = De bestanden die er nu staan gaan naar de prullenbak. Ongedaan maken haalt ze terug.
transfer-replace = Vervangen
transfer-back = Terug
transfer-stop = Stoppen

settings-file-import-title = Instellingen importeren
settings-file-import-dialog = Instellingen importeren
settings-file-export-dialog = Instellingen exporteren
settings-file-import-changes = Wijzigingen
settings-file-import-change = { $before } → { $after }
settings-file-import-no-key = geen toets
settings-file-import-unknown = Geen instelling van deze versie; weggelaten.
settings-file-import-keymap = Sneltoetsen
settings-file-import-nothing = Er verandert niets: deze instellingen zijn al actief.
settings-file-import-summary =
    { $count ->
        [one] { $count } wijziging. Importeren kan in Instellingen ongedaan worden gemaakt.
       *[other] { $count } wijzigingen. Importeren kan in Instellingen ongedaan worden gemaakt.
    }
settings-file-import-secrets =
    { $count ->
        [one] { $count } item lijkt op een wachtwoord of sleutel en wordt nooit geïmporteerd.
       *[other] { $count } items lijken op wachtwoorden of sleutels en worden nooit geïmporteerd.
    }
settings-file-import-commands =
    { $count ->
        [one] { $count } gebruikersopdracht wordt niet geïmporteerd: opdrachten starten programma’s. Kopieer hem zelf naar je instellingenbestand als je hem vertrouwt.
       *[other] { $count } gebruikersopdrachten worden niet geïmporteerd: opdrachten starten programma’s. Kopieer ze zelf naar je instellingenbestand als je ze vertrouwt.
    }
settings-file-import-button = Importeren

## Resultaten

fileop-renamed = Hernoemd naar { $file }
fileop-renamed-count =
    { $count ->
        [one] { $count } bestand hernoemd
       *[other] { $count } bestanden hernoemd
    }
fileop-copied =
    { $count ->
        [one] { $count } bestand gekopieerd naar { $folder }
       *[other] { $count } bestanden gekopieerd naar { $folder }
    }
fileop-moved =
    { $count ->
        [one] { $count } bestand verplaatst naar { $folder }
       *[other] { $count } bestanden verplaatst naar { $folder }
    }
fileop-skipped =
    { $count ->
        [one] { $count } overgeslagen: de naam is daar al in gebruik.
       *[other] { $count } overgeslagen: de namen zijn daar al in gebruik.
    }
fileop-stopped = Gestopt; wat gedaan is, blijft gedaan.
fileop-failed-file =
    { $count ->
        [one] Kan { $file } niet kopiëren of verplaatsen: { $reason }
       *[other] Kan { $count } bestanden niet kopiëren of verplaatsen; het eerste, { $file }: { $reason }
    }
fileop-progress-copy =
    { $count ->
        [one] { $count } bestand kopiëren… { PERCENT($percent) }
       *[other] { $count } bestanden kopiëren… { PERCENT($percent) }
    }
fileop-progress-move =
    { $count ->
        [one] { $count } bestand verplaatsen… { PERCENT($percent) }
       *[other] { $count } bestanden verplaatsen… { PERCENT($percent) }
    }
fileop-done-status = Klaar
fileop-busy = De vorige bestandswijziging loopt nog; probeer het zo opnieuw.
fileop-nothing-to-redo = Niets om opnieuw te doen
fileop-undone = Ongedaan gemaakt: { $what }
fileop-redone = Opnieuw gedaan: { $what }
fileop-kind-rename = naam wijzigen
fileop-kind-batch-rename = namen van meerdere bestanden wijzigen
fileop-kind-move = verplaatsen naar map
fileop-kind-copy = kopiëren naar map
fileop-kind-rotate = draaiing
rename-failed = Kan naam niet wijzigen: { $reason }

undo-problem-changed = Kan niet ongedaan maken: { $file } is sindsdien gewijzigd en die wijziging zou verloren gaan.
undo-problem-missing = Kan niet ongedaan maken: { $file } bestaat niet meer.
undo-problem-taken = Kan niet ongedaan maken: een ander bestand heet nu { $file }.
undo-problem-io = Kan de wijziging aan { $file } niet ongedaan maken: { $reason }

rotate-done-lossless = { $file } zonder verlies gedraaid.
rotate-done-lossless-trimmed = { $file } zonder verlies gedraaid; de rand is bijgesneden tot { $width } × { $height }.
rotate-done-tag = { $file } gedraaid door de oriëntatietag te wijzigen.
rotate-done-sidecar = Draaiing van { $file } opgeslagen in { $sidecar }; het RAW-bestand zelf wordt nooit gewijzigd.
rotate-tag-unreadable = De beeldgegevens konden niet ver genoeg worden gelezen om te draaien.
rotate-tag-multi-picture = Het bevat meer dan één afbeelding, en de eerste draaien zou de andere beschadigen.
rotate-tag-trailer = Na de afbeelding staan gegevens die draaien zou verschuiven.
rotate-tag-not-aligned = De pixels zonder verlies draaien zou het bijsnijden tot { $width } × { $height }.
rotate-trim-offer = Bijsnijden en draaien
rotate-problem-unsupported = { $file } is een soort bestand dat geen draaiing kan vastleggen.
rotate-problem-too-large = { $file } is te groot om te draaien.
rotate-problem-damaged = { $file } is beschadigd waar de oriëntatie staat: { $reason }
rotate-problem-io = Kan { $file } niet draaien: { $reason }

share-failed = Kan niet delen: { $reason }
email-failed = Kan je mailapp niet starten: { $reason }
# machine-drafted, needs native review
zynk-opening = Zynk openen…
zynk-failed = Kan Zynk niet bereiken: { $reason }
zynk-get-title-send = ‘Versturen met Zynk’ heeft de Zynk-app nodig
zynk-get-title-share = ‘Delen met Zynk’ heeft de Zynk-app nodig
zynk-get-about = Zynk is een gratis app voor Mac, Windows, Linux, iPhone en Android. Hij stuurt foto’s, video’s en hele mappen rechtstreeks naar je andere apparaten en naar andere mensen, ongeacht de grootte. Overdrachten zijn end-to-end versleuteld en gaan verder waar ze gebleven waren als de verbinding wegvalt. Voor een ontvanger zonder Zynk maakt hij een weblink waarmee de bestanden in de browser te downloaden zijn.
zynk-get-macos = Download hem in de Mac App Store. Zodra hij is geïnstalleerd, geeft { -app-name } je bestanden er direct aan door.
zynk-get-windows = Download hem in de Microsoft Store. Zodra hij is geïnstalleerd, geeft { -app-name } je bestanden er direct aan door.
zynk-get-linux = Zynk voor Linux is een AppImage, er hoeft niets geïnstalleerd te worden: download hem, zet hem in ~/Applications en maak hem uitvoerbaar (in de eigenschappen, of met chmod +x). { -app-name } vindt hem daar, en overal elders zolang Zynk draait.
zynk-get = Zynk downloaden
zynk-send-old-windows = ‘Versturen met Zynk’ vereist Windows 10 versie 2004 of nieuwer. ‘Delen met Zynk’ werkt wel op deze Windows-versie.
settings-file-export-done = Instellingen opgeslagen in { $file }
settings-file-export-failed = Kan de instellingen niet opslaan: { $reason }
settings-file-import-unreadable = Kan { $file } niet lezen: { $reason }
settings-file-import-done =
    { $count ->
        [one] { $count } wijziging geïmporteerd
       *[other] { $count } wijzigingen geïmporteerd
    }
