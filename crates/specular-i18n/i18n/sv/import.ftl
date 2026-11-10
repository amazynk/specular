# machine-drafted, needs native review
# Specular — Import från kameror, telefoner och minneskort. Svenska.
# Kortet läses bara; filer raderas från det bara på begäran, efter en kontrollerad kopia.

palette-import = Importera bilder…
disabled-no-import-source = Inget minneskort, ingen kamera eller telefon är ansluten

## Inställningar → Import
settings-section-import = Import
settings-import-on-card = När ett kort sätts i
settings-import-on-card-help = Vad som händer när ett minneskort, en kamera eller en telefon med bilder ansluts.
settings-import-on-card-ask = Fråga
settings-import-on-card-open = Öppna Import
settings-import-on-card-nothing = Gör ingenting
settings-import-destination = Importera till
settings-import-destination-help = Mappen där importerade bilder och videor hamnar. Om den inte är angiven hamnar de i mappen Bilder.
settings-import-destination-unset = Mappen Bilder
settings-import-folder-pattern = Mappar
settings-import-folder-pattern-help = Mapparna som skapas i den: {"{"}year{"}"}, {"{"}month{"}"}, {"{"}day{"}"}, {"{"}camera{"}"} och {"{"}event{"}"}, åtskilda med /.
settings-import-rename-pattern = Byt namn
settings-import-rename-pattern-help = Nya filnamn, som i Konvertera: {"{"}name{"}"}, {"{"}n:03{"}"}, {"{"}date:%Y%m%d{"}"}, {"{"}camera{"}"}, {"{"}event{"}"}. Tomt behåller kamerans namn.
settings-import-keep-pairs = Håll ihop RAW och JPEG
settings-import-keep-pairs-help = En RAW-fil och JPEG-filen som togs med den hamnar i samma mapp med samma namn, även när de byter namn.
settings-import-low-res-videos = Importera videokopior med låg upplösning
settings-import-low-res-videos-help = Vissa kameror spelar in en liten förhandsvisningskopia av varje video (.LRV). Av: de stannar på kortet.
settings-import-eject = Mata ut när klart
settings-import-eject-help = Matar ut kortet när allt är kopierat, så att det kan tas ut.

## Erbjudandet när ett kort sätts i
import-offer = Importera från { $name }?
import-offer-button = Importera

## Importvyn
import-title = Importera
import-from = från { $name }
import-source-next = Nästa källa
import-close = Stäng
import-scanning = Letar efter nya bilder… { $done } av { $total }
import-scanning-start = Letar efter nya bilder…
import-none-new = Allt på { $name } har importerats tidigare.
import-empty = Det finns inga bilder eller videor på { $name }.
import-no-source = Inget minneskort, ingen kamera eller telefon är ansluten.
import-summary = { $count ->
    [one] { $count } nytt objekt
   *[other] { $count } nya objekt
} · { $size }
import-already = { $count } importerade tidigare
import-unreadable = { $count ->
    [one] { $count } fil kunde inte läsas
   *[other] { $count } filer kunde inte läsas
}
import-selected = { $count } av { $total } markerade
import-select-all = Markera alla
import-select-none = Avmarkera alla
import-badge-pair = RAW+JPEG
import-badge-raw = RAW
import-badge-video = Video

import-destination = Importera till
import-choose = Välj…
import-folder-pattern = Mappar
import-event = Händelse
import-event-hint = Valfritt, för {"{"}event{"}"}
import-rename = Byt namn
import-rename-hint = Tomt behåller kamerans namn
import-preview = Första filen: { $path }
import-pattern-error = { $error }
import-keep-pairs = Håll ihop RAW och JPEG
import-eject = Mata ut när klart
import-delete = Radera från kortet efter import
import-free = { $size } att kopiera · { $free } ledigt utrymme
import-free-unknown = { $size } att kopiera
import-no-space = Inte tillräckligt med ledigt utrymme i { $folder }: importen behöver { $need } och bara { $free } är ledigt. Frigör utrymme eller välj en annan mapp.
import-start = { $count ->
    [one] Importera { $count } objekt
   *[other] Importera { $count } objekt
}
import-nothing-selected = Markera bilderna som ska importeras

import-confirm-delete = { $count ->
    [one] När den har kopierats och kontrollerats raderas { $count } fil från { $name }. Det går inte att ångra.
   *[other] När de har kopierats och kontrollerats raderas { $count } filer från { $name }. Det går inte att ångra.
}
import-confirm-yes = Importera och radera
import-confirm-no = Tillbaka

import-progress = Importerar { $done } av { $total } · { $size }
import-progress-eta = Importerar { $done } av { $total } · { $size } · { $time }
import-paused = Pausad · { $done } av { $total }
import-pause = Pausa
import-resume = Återuppta
import-cancel = Avbryt
import-hide = Dölj
import-stopping = Stoppar…
import-running = En import pågår redan.

import-done = { $count ->
    [one] { $count } fil importerad
   *[other] { $count } filer importerade
} till { $folder }
import-done-existed = { $count } fanns redan där
import-done-deleted = { $count } raderade från kortet
import-done-failed = { $count ->
    [one] { $count } fil kunde inte importeras
   *[other] { $count } filer kunde inte importeras
}
import-done-cancelled = Importen avbröts efter { $count ->
        [one] { $count } fil
       *[other] { $count } filer
    }. En ny import fortsätter där den slutade.
import-done-nothing = Inget nytt importerades.
import-ejected = { $name } kan tas ut.
import-eject-failed = { $name } kunde inte matas ut: { $error }
import-failed-destination = Mappen { $folder } kan inte användas: { $error }
import-ledger-failed = Listan över importerade filer kunde inte läsas ({ $error }); filer som importerats tidigare kan erbjudas igen.

## Skärmläsare
import-a11y-group = Import
import-a11y-tile = { $name }, { $date }
import-a11y-tile-selected = { $name }, { $date }, markerad
