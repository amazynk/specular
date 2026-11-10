# machine-drafted, needs native review
# Specular — Openingsrapport: waarom het openen van een afbeelding zo lang duurde (infopaneel,
# sectie ‘Prestaties’; DESIGN.md §12.7).

open-report-title = Prestaties

## Headlines
open-report-fast = Geopend in { $ms } ms
open-report-display-cache = Getoond uit de weergavecache
open-report-slow-read = Trage opslag: { $percent } van de tijd ging naar het lezen van het bestand ({ $speed } MB/s)
# machine-drafted, needs native review
open-report-slow-read-no-speed = Trage opslag: { $percent } van de tijd ging naar het lezen van het bestand (klein bestand: vooral wachten tot de opslag reageert)
open-report-archive-read = { $percent } van de tijd ging naar het uitpakken van het archief
open-report-startup = { $percent } van de tijd ging naar het starten van { -app-name }
open-report-checked = Twee keer gedecodeerd: het eerste resultaat van de hardwaredecoder werd vergeleken met dat van de CPU ({ $percent } van de tijd)
open-report-hardware = Gedecodeerd door de hardwaredecoder, die { $percent } van de tijd nodig had
open-report-decode = Grote afbeelding: het decoderen van { $mpx } megapixel kostte { $percent } van de tijd
open-report-upload = { $percent } van de tijd ging naar het verzenden van de afbeelding naar de GPU
open-report-waiting = { $percent } van de tijd ging naar wachten op ander werk
open-report-stage = { $stage } kostte { $percent } van de tijd
open-report-memory-short = Het geheugen was krap, dus het systeem moest mogelijk swappen

## Why a decode ran on the processor although a hardware decoder can do it
open-report-sw-not-probed = Softwarematig gedecodeerd: de hardwaredecoder werd nog gedetecteerd
open-report-sw-no-device = Softwarematig gedecodeerd: { $api } is niet beschikbaar op deze computer
open-report-sw-no-codec = Softwarematig gedecodeerd: { $api } heeft op deze GPU geen { $codec }-decoder
open-report-sw-blocked-mismatch = Softwarematig gedecodeerd: de hardwaredecoder is uitgeschakeld omdat zijn resultaat niet overeenkwam met dat van de CPU
open-report-sw-blocked-error = Softwarematig gedecodeerd: de hardwaredecoder is uitgeschakeld na een fout
open-report-sw-blocked-lost = Softwarematig gedecodeerd: de hardwaredecoder is uitgeschakeld omdat hij niet meer reageerde
open-report-sw-failed = Softwarematig gedecodeerd: de hardwaredecoder faalde bij deze afbeelding
open-report-sw-busy = Softwarematig gedecodeerd: de hardwaredecoder was bezet
open-report-sw-unsupported = Softwarematig gedecodeerd: de hardwaredecoder kan deze afbeelding niet aan
open-report-sw-unavailable = Softwarematig gedecodeerd: hardwaredecodering staat uit tot { -app-name } opnieuw start
open-report-sw-cpu-only = Softwarematig gedecodeerd: verwerking staat op ‘Alleen de CPU’
open-report-sw-setting-off = Softwarematig gedecodeerd: hardwaredecodering is uitgeschakeld in de instellingen
open-report-sw-unchecked = Softwarematig gedecodeerd: de hardwaredecoder is nog niet gecontroleerd (hij wordt eerst geprobeerd op een afbeelding die op de achtergrond wordt voorbereid)
open-report-sw-lower-power = Softwarematig gedecodeerd: op de accu verbruikt de CPU hiervoor minder energie
open-report-sw-faster = Softwarematig gedecodeerd: sneller op deze computer (ongeveer { $cpu } ms, tegen { $hw } ms met de hardwaredecoder)
open-report-sw-scaled = Softwarematig gedecodeerd: de hardwaredecoder decodeert afbeeldingen alleen op volledige grootte

## Hints
open-report-hint-copy-local = Kopieer het bestand naar een lokale schijf om het sneller te openen.
open-report-hint-extract = Pak het archief uit naar een map om de pagina’s sneller te openen.
open-report-hint-try-again = Om de hardwaredecoder opnieuw te proberen: Instellingen → Prestaties → Opnieuw proberen.
open-report-hint-automatic = Zet ‘Waar afbeeldingen worden verwerkt’ op Automatisch in Instellingen → Prestaties.
open-report-hint-turn-on = Zet ‘Hardwaredecodering’ op Automatisch in Instellingen → Prestaties.
open-report-hint-free-memory = Sluit andere programma’s om geheugen vrij te maken.
open-report-hint-once-only = Alleen de eerste afbeelding wordt zo gecontroleerd.

## Rows
open-report-total = Tot scherp
open-report-first-pixels = Eerste pixels
# machine-drafted, needs native review
open-report-whole = Hele afbeelding zichtbaar
open-report-stage-startup = Opstarten
open-report-stage-read = Bestand lezen
open-report-stage-parse = Header lezen
open-report-stage-colour = Kleurbeheer voorbereiden
open-report-stage-decode = Decoderen
open-report-stage-upload = Naar de GPU sturen
open-report-stage-waiting = Wachten op ander werk
open-report-decoder = Gedecodeerd door
open-report-decoder-checked = { $engine }, gecontroleerd met de CPU
open-report-decoder-cache = Weergavecache
# $scale is the reduction: 4 for a level a quarter of the full width and height.
open-report-decoder-scaled = { $engine }, op 1/{ $scale } van de grootte
open-report-decoder-scaled-full = { $engine }, op volledige grootte, daarna verkleind tot 1/{ $scale }
open-report-decoder-scaled-no-index = { $engine }, op 1/{ $scale } van de grootte, zonder de snelstartindex
open-report-colour-conversion = Kleuren omgezet door

## Advisor: a pattern across recent opens, what would help and by how much (rows)
# $count and $total are counts of opens, $format a file format ("HEIC"), $speed a formatted
# number (MB/s), $low and $high formatted numbers ("2.5").
open-report-advice-recurring = Terugkerend
open-report-advice-change = Zou helpen
open-report-advice-gain = Geschatte winst
open-report-advice-software = { $count } van de laatste { $total } trage { $format }-afbeeldingen { $count ->
    [one] werd
   *[other] werden
} om deze reden softwarematig gedecodeerd
open-report-advice-slow-storage = { $count } van de laatste { $total } trage afbeeldingen { $count ->
    [one] werd
   *[other] werden
} opgehouden door het lezen van het bestand (ongeveer { $speed } MB/s)
# machine-drafted, needs native review
open-report-advice-slow-storage-no-speed = { $count } van de laatste { $total } trage afbeeldingen { $count ->
    [one] werd
   *[other] werden
} opgehouden door het lezen van het bestand (kleine bestanden: vooral wachten tot de opslag reageert)
open-report-advice-archive = { $count } van de laatste { $total } trage afbeeldingen { $count ->
    [one] werd
   *[other] werden
} opgehouden door het uitpakken van het archief
open-report-advice-hardware-decoder = Een GPU met een { $codec }-hardwaredecoder
open-report-advice-gain-range = Ongeveer { $low } tot { $high } keer zo snel
open-report-advice-gain-one = Ongeveer { $low } keer zo snel

## Performance insights: the indicator in the corner and the panel it opens
# machine-drafted, needs native review
# $total is a formatted duration ("240 ms"), $format a file format ("HEIC").
open-report-insights-title = Prestatie-inzichten
open-report-insights-indicator = Waarom is dit traag?
open-report-insights-announce = Prestatie-inzicht: afbeeldingen openen steeds om dezelfde reden traag. Open ‘Waarom is dit traag?’ om te zien wat zou helpen.
open-report-insights-hide = Verbergen
open-report-insights-last = Laatste trage afbeelding: { $total } tot scherp
open-report-insights-now = Wat u nu kunt doen
open-report-insights-hardware = Hardware die zou helpen
open-report-insights-none = Geen terugkerende vertraging
open-report-insights-none-detail = Inzichten verschijnen als meerdere afbeeldingen om dezelfde reden traag openen. Om ze te vinden, bewaart { -app-name } een week aan trage openingen op deze computer; er wordt niets verstuurd.
open-report-insights-software = { $format }-afbeeldingen worden op de CPU gedecodeerd
open-report-insights-storage = Het lezen van de bestanden houdt afbeeldingen op
open-report-insights-archive = Het uitpakken van archieven houdt afbeeldingen op
open-report-insights-nothing-now = Geen instelling in { -app-name } verandert hier iets aan.
open-report-insights-display-cache = Schakel de weergavecache in: afbeeldingen die u eerder hebt geopend, openen dan vanuit een snelle kopie op deze computer.
open-report-insights-prefetch = Houd meer naburige afbeeldingen gereed, zodat de volgende worden voorbereid terwijl u kijkt.
open-report-insights-hw-ssd = Een snelle interne schijf (een NVMe-SSD) voor deze bestanden
open-report-insights-action-turn-on = Inschakelen
open-report-insights-action-try-again = Opnieuw proberen
open-report-insights-action-automatic = Op Automatisch zetten
open-report-insights-dismiss-storage = Niet meer tonen voor opslag
open-report-insights-dismiss-decoder = Niet meer tonen voor decoderen
open-report-insights-off = Inzichten uitschakelen
open-report-insights-on = Inzichten inschakelen
open-report-insights-restore = Verborgen inzichten weer tonen
open-report-insights-power = Deze computer bespaart energie (op de accu of in een energiebesparingsmodus), waardoor decoderen op de CPU trager gaat. Met de stekker erin openen afbeeldingen misschien sneller.

## Performance insights in the gallery: the same, about making thumbnails
# machine-drafted, needs native review
# $count and $total are counts of thumbnails, $total in -last-thumbnail a formatted duration
# ("240 ms"), $format a file format ("HEIC"), $speed a formatted number (MB/s).
open-report-insights-announce-thumbnails = Prestatie-inzicht: miniaturen verschijnen steeds om dezelfde reden traag. Open ‘Waarom is dit traag?’ om te zien wat zou helpen.
open-report-insights-last-thumbnail = Laatste trage miniatuur: { $total } om te maken
open-report-insights-thumbnails-software = Miniaturen van { $format }-afbeeldingen worden op de CPU gedecodeerd
open-report-insights-thumbnails-storage = Het lezen van de bestanden houdt miniaturen op
open-report-insights-thumbnails-archive = Het uitpakken van archieven houdt miniaturen op
open-report-insights-thumbnail-cache = Miniaturen kunnen niet op deze computer worden bewaard (de miniaturencache is niet beschikbaar), dus worden ze telkens opnieuw gemaakt als de map wordt getoond.
open-report-advice-thumbnails-software = { $count } van de laatste { $total } trage { $format }-miniaturen { $count ->
    [one] werd
   *[other] werden
} om deze reden softwarematig gedecodeerd
open-report-advice-thumbnails-slow-storage = { $count } van de laatste { $total } trage miniaturen { $count ->
    [one] werd
   *[other] werden
} opgehouden door het lezen van het bestand (ongeveer { $speed } MB/s)
# machine-drafted, needs native review
open-report-advice-thumbnails-slow-storage-no-speed = { $count } van de laatste { $total } trage miniaturen { $count ->
    [one] werd
   *[other] werden
} opgehouden door het lezen van het bestand (kleine bestanden: vooral wachten tot de opslag reageert)
open-report-advice-thumbnails-archive = { $count } van de laatste { $total } trage miniaturen { $count ->
    [one] werd
   *[other] werden
} opgehouden door het uitpakken van het archief
