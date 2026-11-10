# machine-drafted, needs native review
# Specular — Importeren van camera’s, telefoons en geheugenkaarten. Nederlands.
# De kaart wordt alleen gelezen; bestanden worden er alleen op verzoek van verwijderd, na een gecontroleerde kopie.

palette-import = Foto’s importeren…
disabled-no-import-source = Geen geheugenkaart, camera of telefoon aangesloten

## Instellingen → Importeren
settings-section-import = Importeren
settings-import-on-card = Bij het plaatsen van een kaart
settings-import-on-card-help = Wat er gebeurt als een geheugenkaart, camera of telefoon met foto’s wordt aangesloten.
settings-import-on-card-ask = Vragen
settings-import-on-card-open = Importeren openen
settings-import-on-card-nothing = Niets doen
settings-import-destination = Importeren naar
settings-import-destination-help = De map voor geïmporteerde foto’s en video’s. Niet ingesteld: je map Afbeeldingen.
settings-import-destination-unset = Map Afbeeldingen
settings-import-folder-pattern = Mappen
settings-import-folder-pattern-help = De mappen die erin worden gemaakt: {"{"}year{"}"}, {"{"}month{"}"}, {"{"}day{"}"}, {"{"}camera{"}"} en {"{"}event{"}"}, gescheiden door /.
settings-import-rename-pattern = Naam wijzigen
settings-import-rename-pattern-help = Nieuwe bestandsnamen, zoals bij Converteren: {"{"}name{"}"}, {"{"}n:03{"}"}, {"{"}date:%Y%m%d{"}"}, {"{"}camera{"}"}, {"{"}event{"}"}. Leeg houdt de namen van de camera.
settings-import-keep-pairs = RAW en JPEG bij elkaar houden
settings-import-keep-pairs-help = Een RAW-bestand en de bijbehorende JPEG komen in één map met één naam, ook na hernoemen.
settings-import-low-res-videos = Videokopieën in lage resolutie importeren
settings-import-low-res-videos-help = Sommige camera’s nemen van elke video een kleine voorbeeldkopie op (.LRV). Uit: ze blijven op de kaart.
settings-import-eject = Uitwerpen als het klaar is
settings-import-eject-help = Werpt de kaart uit zodra alles is gekopieerd, zodat je hem kunt verwijderen.

## Het aanbod bij het plaatsen van een kaart
import-offer = Importeren van { $name }?
import-offer-button = Importeren

## De importweergave
import-title = Importeren
import-from = van { $name }
import-source-next = Volgende bron
import-close = Sluiten
import-scanning = Nieuwe foto’s zoeken… { $done } van { $total }
import-scanning-start = Nieuwe foto’s zoeken…
import-none-new = Alles op { $name } is al eerder geïmporteerd.
import-empty = Er staan geen foto’s of video’s op { $name }.
import-no-source = Geen geheugenkaart, camera of telefoon aangesloten.
import-summary = { $count ->
    [one] { $count } nieuw item
   *[other] { $count } nieuwe items
} · { $size }
import-already = { $count } eerder geïmporteerd
import-unreadable = { $count ->
    [one] { $count } bestand kon niet worden gelezen
   *[other] { $count } bestanden konden niet worden gelezen
}
import-selected = { $count } van { $total } geselecteerd
import-select-all = Alles selecteren
import-select-none = Niets selecteren
import-badge-pair = RAW+JPEG
import-badge-raw = RAW
import-badge-video = Video

import-destination = Importeren naar
import-choose = Kiezen…
import-folder-pattern = Mappen
import-event = Gebeurtenis
import-event-hint = Optioneel, voor {"{"}event{"}"}
import-rename = Naam wijzigen
import-rename-hint = Leeg houdt de namen van de camera
import-preview = Eerste bestand: { $path }
import-pattern-error = { $error }
import-keep-pairs = RAW en JPEG bij elkaar houden
import-eject = Uitwerpen als het klaar is
import-delete = Na importeren van de kaart verwijderen
import-free = { $size } te kopiëren · { $free } vrije ruimte
import-free-unknown = { $size } te kopiëren
import-no-space = Niet genoeg vrije ruimte in { $folder }: deze import heeft { $need } nodig en er is maar { $free } vrij. Maak ruimte vrij of kies een andere map.
import-start = { $count ->
    [one] { $count } item importeren
   *[other] { $count } items importeren
}
import-nothing-selected = Selecteer de foto’s om te importeren

import-confirm-delete = { $count ->
    [one] Na kopiëren en controleren wordt { $count } bestand van { $name } verwijderd. Dit kan niet ongedaan worden gemaakt.
   *[other] Na kopiëren en controleren worden { $count } bestanden van { $name } verwijderd. Dit kan niet ongedaan worden gemaakt.
}
import-confirm-yes = Importeren en verwijderen
import-confirm-no = Terug

import-progress = { $done } van { $total } importeren · { $size }
import-progress-eta = { $done } van { $total } importeren · { $size } · { $time }
import-paused = Gepauzeerd · { $done } van { $total }
import-pause = Pauzeren
import-resume = Hervatten
import-cancel = Annuleren
import-hide = Verbergen
import-stopping = Stoppen…
import-running = Er loopt al een import.

import-done = { $count ->
    [one] { $count } bestand geïmporteerd
   *[other] { $count } bestanden geïmporteerd
} naar { $folder }
import-done-existed = { $count ->
        [one] { $count } stond er al
       *[other] { $count } stonden er al
    }
import-done-deleted = { $count } van de kaart verwijderd
import-done-failed = { $count ->
    [one] { $count } bestand kon niet worden geïmporteerd
   *[other] { $count } bestanden konden niet worden geïmporteerd
}
import-done-cancelled = Import geannuleerd na { $count ->
        [one] { $count } bestand
       *[other] { $count } bestanden
    }. Opnieuw importeren gaat verder waar het stopte.
import-done-nothing = Er is niets nieuws geïmporteerd.
import-ejected = { $name } kan worden verwijderd.
import-eject-failed = { $name } kon niet worden uitgeworpen: { $error }
import-failed-destination = De map { $folder } kan niet worden gebruikt: { $error }
import-ledger-failed = De lijst met geïmporteerde bestanden kon niet worden gelezen ({ $error }); eerder geïmporteerde bestanden worden misschien opnieuw aangeboden.

## Schermlezer
import-a11y-group = Importeren
import-a11y-tile = { $name }, { $date }
import-a11y-tile-selected = { $name }, { $date }, geselecteerd
