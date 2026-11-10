# machine-drafted, needs native review
# Specular — Import fra kameraer, telefoner og hukommelseskort. Dansk.
# Kortet læses kun; filer slettes kun fra det, når du beder om det, efter en kontrolleret kopi.

palette-import = Importér fotos…
disabled-no-import-source = Der er intet hukommelseskort, kamera eller telefon tilsluttet

## Indstillinger → Import
settings-section-import = Import
settings-import-on-card = Når et kort indsættes
settings-import-on-card-help = Hvad der sker, når et hukommelseskort, et kamera eller en telefon med fotos tilsluttes.
settings-import-on-card-ask = Spørg
settings-import-on-card-open = Åbn Import
settings-import-on-card-nothing = Gør intet
settings-import-destination = Importér til
settings-import-destination-help = Mappen, som importerede fotos og videoer lægges i. Hvis den ikke er angivet, lægges de i mappen Billeder.
settings-import-destination-unset = Mappen Billeder
settings-import-folder-pattern = Mapper
settings-import-folder-pattern-help = Mapperne, der oprettes i den: {"{"}year{"}"}, {"{"}month{"}"}, {"{"}day{"}"}, {"{"}camera{"}"} og {"{"}event{"}"}, adskilt af /.
settings-import-rename-pattern = Omdøb
settings-import-rename-pattern-help = Nye filnavne, som i Konvertér: {"{"}name{"}"}, {"{"}n:03{"}"}, {"{"}date:%Y%m%d{"}"}, {"{"}camera{"}"}, {"{"}event{"}"}. Tomt beholder kameraets navne.
settings-import-keep-pairs = Hold RAW og JPEG sammen
settings-import-keep-pairs-help = En RAW-fil og den JPEG, der blev taget med den, lægges i samme mappe med samme navn, også når de omdøbes.
settings-import-low-res-videos = Importér videokopier i lav opløsning
settings-import-low-res-videos-help = Nogle kameraer optager en lille eksempelkopi af hver video (.LRV). Fra: de bliver på kortet.
settings-import-eject = Skub ud, når det er færdigt
settings-import-eject-help = Skubber kortet ud, når alt er kopieret, så det kan tages ud.

## Tilbuddet, når et kort indsættes
import-offer = Importér fra { $name }?
import-offer-button = Importér

## Importvisningen
import-title = Import
import-from = fra { $name }
import-source-next = Næste kilde
import-close = Luk
import-scanning = Leder efter nye fotos… { $done } af { $total }
import-scanning-start = Leder efter nye fotos…
import-none-new = Alt på { $name } er importeret før.
import-empty = Der er ingen fotos eller videoer på { $name }.
import-no-source = Der er intet hukommelseskort, kamera eller telefon tilsluttet.
import-summary = { $count ->
    [one] { $count } nyt element
   *[other] { $count } nye elementer
} · { $size }
import-already = { $count } importeret før
import-unreadable = { $count ->
    [one] { $count } fil kunne ikke læses
   *[other] { $count } filer kunne ikke læses
}
import-selected = { $count } af { $total } valgt
import-select-all = Markér alt
import-select-none = Fravælg alt
import-badge-pair = RAW+JPEG
import-badge-raw = RAW
import-badge-video = Video

import-destination = Importér til
import-choose = Vælg…
import-folder-pattern = Mapper
import-event = Begivenhed
import-event-hint = Valgfrit, til {"{"}event{"}"}
import-rename = Omdøb
import-rename-hint = Tomt beholder kameraets navne
import-preview = Første fil: { $path }
import-pattern-error = { $error }
import-keep-pairs = Hold RAW og JPEG sammen
import-eject = Skub ud, når det er færdigt
import-delete = Slet fra kortet efter import
import-free = { $size } at kopiere · { $free } ledig plads
import-free-unknown = { $size } at kopiere
import-no-space = Der er ikke nok ledig plads i { $folder }: importen kræver { $need }, og kun { $free } er ledig. Frigør plads, eller vælg en anden mappe.
import-start = { $count ->
    [one] Importér { $count } element
   *[other] Importér { $count } elementer
}
import-nothing-selected = Vælg de fotos, der skal importeres

import-confirm-delete = { $count ->
    [one] Når den er kopieret og kontrolleret, slettes { $count } fil fra { $name }. Det kan ikke fortrydes.
   *[other] Når de er kopieret og kontrolleret, slettes { $count } filer fra { $name }. Det kan ikke fortrydes.
}
import-confirm-yes = Importér og slet
import-confirm-no = Tilbage

import-progress = Importerer { $done } af { $total } · { $size }
import-progress-eta = Importerer { $done } af { $total } · { $size } · { $time }
import-paused = På pause · { $done } af { $total }
import-pause = Pause
import-resume = Genoptag
import-cancel = Annuller
import-hide = Skjul
import-stopping = Stopper…
import-running = En import er allerede i gang.

import-done = { $count ->
    [one] { $count } fil importeret
   *[other] { $count } filer importeret
} til { $folder }
import-done-existed = { $count } fandtes allerede
import-done-deleted = { $count } slettet fra kortet
import-done-failed = { $count ->
    [one] { $count } fil kunne ikke importeres
   *[other] { $count } filer kunne ikke importeres
}
import-done-cancelled = Importen blev annulleret efter { $count ->
        [one] { $count } fil
       *[other] { $count } filer
    }. En ny import fortsætter, hvor den stoppede.
import-done-nothing = Intet nyt blev importeret.
import-ejected = { $name } kan tages ud.
import-eject-failed = { $name } kunne ikke skubbes ud: { $error }
import-failed-destination = Mappen { $folder } kan ikke bruges: { $error }
import-ledger-failed = Listen over importerede filer kunne ikke læses ({ $error }); filer, der er importeret før, kan blive tilbudt igen.

## Skærmlæser
import-a11y-group = Import
import-a11y-tile = { $name }, { $date }
import-a11y-tile-selected = { $name }, { $date }, valgt
