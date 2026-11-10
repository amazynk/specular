# machine-drafted, needs native review
# Specular — Library (DESIGN §10).

palette-library = Knihovna

## Top bar
library-title = Knihovna
library-locations-button = Umístění

## Locations sidebar
library-locations = Umístění
library-all = Všechna umístění
library-add = Přidat umístění…
library-remove = Odebrat { $location } z knihovny
library-offline = Offline
library-location-pending = K indexování: { $count }
library-a11y-location = { $location }, { $count ->
    [one] { $count } obrázek
    [few] { $count } obrázky
    [many] { $count } obrázku
   *[other] { $count } obrázků
}
library-a11y-location-offline = { $location }, Offline, { $count ->
    [one] { $count } obrázek
    [few] { $count } obrázky
    [many] { $count } obrázku
   *[other] { $count } obrázků
}

## Indexing progress
library-walking = Nalezené obrázky: { $count } · { $location }
# machine-drafted, needs native review
library-walking-count = Nalezené obrázky: { $count }
library-reading = Indexování… { $done } z { $total }
library-held = Indexování se při procházení pozastaví
library-up-to-date = Aktuální
# machine-drafted, needs native review
library-stopped = Indexování zastaveno
library-stopped-pending = Indexování zastaveno · k indexování: { $count }
library-resume = Pokračovat
# machine-drafted, needs native review
# The indexer stopped by itself (a database error, a bug) or stopped moving; $reason says why.
library-halted = Indexování zastaveno: { $reason }
library-reason-stalled = už { $minutes ->
    [one] minutu
    [few] { $minutes } minuty
    [many] { $minutes } minuty
   *[other] { $minutes } minut
} se nic nehnulo. Soubor nebo disk možná neodpovídá.
library-retry = Zkusit znovu
library-retry-description = Spustí indexování znovu. V tom, co už je zaindexováno, lze dál hledat.
library-save-diagnostics = Uložit diagnostický balíček…
library-stop-description = Zastaví indexování. V tom, co už bylo zaindexováno, lze dál hledat; Pokračovat nebo Obnovit (F5) v něm pokračuje.
library-resume-description = Znovu projde všechna umístění a zaindexuje, co zbývá.
library-opening = Otevírání knihovny…
library-searching = Hledání…

## Empty and error states
library-empty = Zatím žádná umístění. Přidejte složku: její obrázky a obrázky z podsložek se zobrazí zde v jedné mřížce.
library-empty-offline = Tato umístění jsou offline. Jejich obrázky se znovu zobrazí, až budou zpět.
library-indexing = Hledání obrázků…
# machine-drafted, needs native review
library-remove-confirm = Odebrat „{ $location }“ z knihovny?
library-remove-confirm-detail = Vaše fotky zůstanou na disku i s hodnocením, štítky a klíčovými slovy. Knihovna zapomene, co tam zaindexovala; po opětovném přidání složky ji zaindexuje znovu.
library-remove-confirm-ok = Odebrat
library-indexing-in = Hledání obrázků v { $location }… zatím nalezeno: { $count }
# machine-drafted, needs native review
library-empty-stopped = Indexování bylo zastaveno dříve, než se tyto obrázky načetly. Pokračovat naváže od tohoto místa.
library-unavailable = Knihovnu nelze otevřít: { $reason }
library-read-only = Tuto knihovnu zapsal novější Specular: lze v ní hledat, ale nelze ji aktualizovat.

## Screen readers
library-a11y-list = Knihovna, { $count ->
    [one] { $count } obrázek
    [few] { $count } obrázky
    [many] { $count } obrázku
   *[other] { $count } obrázků
}
library-a11y-added = { $location } přidáno do knihovny
library-a11y-removed = { $location } odebráno z knihovny

## Adding the home folder
# machine-drafted, needs native review
library-home-confirm = Přidat vše z „{ $folder }“?
library-home-confirm-detail = Složka obsahuje soubory programů, stažené soubory a mezipaměti, nejen fotky. Indexování trvá déle a knihovna se zaplní ikonami a dalšími obrázky.
library-home-confirm-pictures = Vaše složka Obrázky obsahuje jen vaše obrázky.
library-home-confirm-use-pictures = Přidat Obrázky
library-home-confirm-keep = Přidat domovskou složku

## Folder watches
# machine-drafted, needs native review
library-unwatched = Některé složky se nesledují
library-unwatched-detail = { $minutes ->
    [one] Změny se projeví do minuty
    [few] Změny se projeví do { $minutes } min
    [many] Změny se projeví do { $minutes } min
   *[other] Změny se projeví do { $minutes } min
}
library-unwatched-description = Specularu došly sledování složek. Otevře nápovědu, jak limit zvýšit.

## Settings → Library
# machine-drafted, needs native review
settings-section-library = Knihovna
settings-library-skip-folders = Přeskakovat složky s názvem
settings-library-skip-folders-help = Knihovna nehledá ve složkách s těmito názvy, oddělenými čárkami. Skryté složky se přeskakují vždy.
settings-library-skip-marked = Přeskakovat složky obsahující
settings-library-skip-marked-help = Ani ve složkách, které obsahují něco z toho: .git u kódových repozitářů, pyvenv.cfg u prostředí Pythonu, CACHEDIR.TAG u mezipamětí. Vlastní složka umístění se nikdy nepřeskakuje.
settings-library-show-timeline = Časová osa
settings-library-show-timeline-help = Nad mřížkou knihovny ukazuje, kolik fotek má každý rok, měsíc nebo den.
