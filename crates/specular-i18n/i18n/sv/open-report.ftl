# machine-drafted, needs native review
# Specular — Öppningsrapport: varför det tog så lång tid att öppna en bild (informationspanelen,
# avsnittet ”Prestanda”; DESIGN.md §12.7).

open-report-title = Prestanda

## Headlines
open-report-fast = Öppnades på { $ms } ms
open-report-display-cache = Visad från visningscachen
open-report-slow-read = Långsam lagring: { $percent } av tiden gick åt till att läsa filen ({ $speed } MB/s)
# machine-drafted, needs native review
open-report-slow-read-no-speed = Långsam lagring: { $percent } av tiden gick åt till att läsa filen (en liten fil: mest väntan på att lagringen svarar)
open-report-archive-read = { $percent } av tiden gick åt till att packa upp arkivet
open-report-startup = { $percent } av tiden gick åt till att starta { -app-name }
open-report-checked = Avkodad två gånger: maskinvaruavkodarens första resultat jämfördes med CPU:ns ({ $percent } av tiden)
open-report-hardware = Avkodad av maskinvaruavkodaren, som tog { $percent } av tiden
open-report-decode = Stor bild: att avkoda { $mpx } megapixel tog { $percent } av tiden
open-report-upload = { $percent } av tiden gick åt till att skicka bilden till GPU:n
open-report-waiting = { $percent } av tiden gick åt till att vänta på annat arbete
open-report-stage = { $stage } tog { $percent } av tiden
open-report-memory-short = Det var ont om minne, så systemet kan ha behövt växla ut

## Why a decode ran on the processor although a hardware decoder can do it
open-report-sw-not-probed = Avkodad i programvara: maskinvaruavkodaren höll fortfarande på att identifieras
open-report-sw-no-device = Avkodad i programvara: { $api } är inte tillgängligt på den här datorn
open-report-sw-no-codec = Avkodad i programvara: { $api } saknar { $codec }-avkodare på den här GPU:n
open-report-sw-blocked-mismatch = Avkodad i programvara: maskinvaruavkodaren stängdes av eftersom dess resultat inte stämde med CPU:ns
open-report-sw-blocked-error = Avkodad i programvara: maskinvaruavkodaren stängdes av efter ett fel
open-report-sw-blocked-lost = Avkodad i programvara: maskinvaruavkodaren stängdes av eftersom den slutade svara
open-report-sw-failed = Avkodad i programvara: maskinvaruavkodaren misslyckades med den här bilden
open-report-sw-busy = Avkodad i programvara: maskinvaruavkodaren var upptagen
open-report-sw-unsupported = Avkodad i programvara: maskinvaruavkodaren kan inte ta den här bilden
open-report-sw-unavailable = Avkodad i programvara: maskinvaruavkodning är av tills { -app-name } startas om
open-report-sw-cpu-only = Avkodad i programvara: bearbetningen är inställd på ”Endast CPU”
open-report-sw-setting-off = Avkodad i programvara: maskinvaruavkodning är avstängd i inställningarna
open-report-sw-unchecked = Avkodad i programvara: maskinvaruavkodaren är inte kontrollerad än (den provas först på en bild som förbereds i bakgrunden)
open-report-sw-lower-power = Avkodad i programvara: på batteri drar CPU:n mindre ström för detta
open-report-sw-faster = Avkodad i programvara: snabbare på den här datorn (cirka { $cpu } ms, mot { $hw } ms med maskinvaruavkodaren)
open-report-sw-scaled = Avkodad i programvara: maskinvaruavkodaren avkodar bara bilder i full storlek

## Hints
open-report-hint-copy-local = Kopiera filen till en lokal disk så öppnas den snabbare.
open-report-hint-extract = Packa upp arkivet i en mapp så öppnas sidorna snabbare.
open-report-hint-try-again = För att prova maskinvaruavkodaren igen: Inställningar → Prestanda → Försök igen.
open-report-hint-automatic = Ställ ”Var bilder bearbetas” på Automatiskt under Inställningar → Prestanda.
open-report-hint-turn-on = Ställ ”Maskinvaruavkodning” på Automatiskt under Inställningar → Prestanda.
open-report-hint-free-memory = Stäng andra program för att frigöra minne.
open-report-hint-once-only = Bara den första bilden kontrolleras så här.

## Rows
open-report-total = Tills skarp
open-report-first-pixels = Första pixlarna
# machine-drafted, needs native review
open-report-whole = Hela bilden visas
open-report-stage-startup = Uppstart
open-report-stage-read = Läsa filen
open-report-stage-parse = Läsa huvudet
open-report-stage-colour = Förbereda färghantering
open-report-stage-decode = Avkodning
open-report-stage-upload = Skicka till GPU:n
open-report-stage-waiting = Vänta på annat arbete
open-report-decoder = Avkodad av
open-report-decoder-checked = { $engine }, kontrollerad mot CPU:n
open-report-decoder-cache = Visningscache
# $scale is the reduction: 4 for a level a quarter of the full width and height.
open-report-decoder-scaled = { $engine }, i 1/{ $scale } storlek
open-report-decoder-scaled-full = { $engine }, i full storlek, sedan förminskad till 1/{ $scale }
open-report-decoder-scaled-no-index = { $engine }, i 1/{ $scale } storlek, utan snabbstartsindexet
open-report-colour-conversion = Färger omvandlade av

## Advisor: a pattern across recent opens, what would help and by how much (rows)
# $count and $total are counts of opens, $format a file format ("HEIC"), $speed a formatted
# number (MB/s), $low and $high formatted numbers ("2.5").
open-report-advice-recurring = Återkommande
open-report-advice-change = Skulle hjälpa
open-report-advice-gain = Uppskattad vinst
open-report-advice-software = { $count } av de senaste { $total } långsamma { $format }-bilderna avkodades i programvara av det här skälet
open-report-advice-slow-storage = { $count } av de senaste { $total } långsamma bilderna fördröjdes av att filen lästes (cirka { $speed } MB/s)
# machine-drafted, needs native review
open-report-advice-slow-storage-no-speed = { $count } av de senaste { $total } långsamma bilderna fördröjdes av att filen lästes (små filer: mest väntan på att lagringen svarar)
open-report-advice-archive = { $count } av de senaste { $total } långsamma bilderna fördröjdes av att arkivet packades upp
open-report-advice-hardware-decoder = En GPU med maskinvaruavkodare för { $codec }
open-report-advice-gain-range = Cirka { $low } till { $high } gånger snabbare
open-report-advice-gain-one = Cirka { $low } gånger snabbare

## Performance insights: the indicator in the corner and the panel it opens
# machine-drafted, needs native review
# $total is a formatted duration ("240 ms"), $format a file format ("HEIC").
open-report-insights-title = Prestandainsikter
open-report-insights-indicator = Varför går det långsamt?
open-report-insights-announce = Prestandainsikt: bilder öppnas gång på gång långsamt av samma skäl. Öppna ”Varför går det långsamt?” för att se vad som skulle hjälpa.
open-report-insights-hide = Dölj
open-report-insights-last = Senaste långsamma bild: { $total } tills skarp
open-report-insights-now = Det här kan du göra nu
open-report-insights-hardware = Maskinvara som skulle hjälpa
open-report-insights-none = Ingen återkommande fördröjning
open-report-insights-none-detail = Insikter visas när flera bilder öppnas långsamt av samma skäl. För att hitta dem sparar { -app-name } en veckas långsamma öppningar på den här datorn; inget skickas någonstans.
open-report-insights-software = { $format }-bilder avkodas på CPU:n
open-report-insights-storage = Läsningen av filerna fördröjer bilderna
open-report-insights-archive = Uppackningen av arkiv fördröjer bilderna
open-report-insights-nothing-now = Inga inställningar i { -app-name } ändrar på det här.
open-report-insights-display-cache = Slå på visningscachen: bilder du har öppnat förut öppnas då från en snabb kopia på den här datorn.
open-report-insights-prefetch = Håll fler närliggande bilder redo, så att nästa förbereds medan du tittar.
open-report-insights-hw-ssd = En snabb intern enhet (en NVMe-SSD) för de här filerna
open-report-insights-action-turn-on = Slå på
open-report-insights-action-try-again = Försök igen
open-report-insights-action-automatic = Ställ på Automatiskt
open-report-insights-dismiss-storage = Visa inte igen för lagring
open-report-insights-dismiss-decoder = Visa inte igen för avkodning
open-report-insights-off = Stäng av insikter
open-report-insights-on = Slå på insikter
open-report-insights-restore = Visa dolda insikter igen
open-report-insights-power = Den här datorn sparar ström (på batteri eller i ett energisparläge), vilket gör avkodningen på CPU:n långsammare. Med nätström ansluten kan bilderna öppnas snabbare.

## Performance insights in the gallery: the same, about making thumbnails
# machine-drafted, needs native review
# $count and $total are counts of thumbnails, $total in -last-thumbnail a formatted duration
# ("240 ms"), $format a file format ("HEIC"), $speed a formatted number (MB/s).
open-report-insights-announce-thumbnails = Prestandainsikt: miniatyrer visas gång på gång långsamt av samma skäl. Öppna ”Varför går det långsamt?” för att se vad som skulle hjälpa.
open-report-insights-last-thumbnail = Senaste långsamma miniatyr: { $total } att skapa
open-report-insights-thumbnails-software = Miniatyrer av { $format }-bilder avkodas på CPU:n
open-report-insights-thumbnails-storage = Läsningen av filerna fördröjer miniatyrerna
open-report-insights-thumbnails-archive = Uppackningen av arkiv fördröjer miniatyrerna
open-report-insights-thumbnail-cache = Miniatyrer kan inte sparas på den här datorn (miniatyrcachen är inte tillgänglig), så de skapas på nytt varje gång mappen visas.
open-report-advice-thumbnails-software = { $count } av de senaste { $total } långsamma { $format }-miniatyrerna avkodades i programvara av det här skälet
open-report-advice-thumbnails-slow-storage = { $count } av de senaste { $total } långsamma miniatyrerna fördröjdes av att filen lästes (cirka { $speed } MB/s)
# machine-drafted, needs native review
open-report-advice-thumbnails-slow-storage-no-speed = { $count } av de senaste { $total } långsamma miniatyrerna fördröjdes av att filen lästes (små filer: mest väntan på att lagringen svarar)
open-report-advice-thumbnails-archive = { $count } av de senaste { $total } långsamma miniatyrerna fördröjdes av att arkivet packades upp
