# machine-drafted, needs native review
# Specular — Åbningsrapport: hvorfor det tog så lang tid at åbne et billede (informationspanelet,
# afsnittet “Ydeevne”; DESIGN.md §12.7).

open-report-title = Ydeevne

## Headlines
open-report-fast = Åbnet på { $ms } ms
open-report-display-cache = Vist fra visningscachen
open-report-slow-read = Langsomt lager: { $percent } af tiden gik med at læse filen ({ $speed } MB/s)
# machine-drafted, needs native review
open-report-slow-read-no-speed = Langsomt lager: { $percent } af tiden gik med at læse filen (en lille fil: mest ventetid på, at lageret svarer)
open-report-archive-read = { $percent } af tiden gik med at pakke arkivet ud
open-report-startup = { $percent } af tiden gik med at starte { -app-name }
open-report-checked = Afkodet to gange: hardwareafkoderens første resultat blev sammenlignet med CPU'ens ({ $percent } af tiden)
open-report-hardware = Afkodet af hardwareafkoderen, som brugte { $percent } af tiden
open-report-decode = Stort billede: at afkode { $mpx } megapixel tog { $percent } af tiden
open-report-upload = { $percent } af tiden gik med at sende billedet til GPU'en
open-report-waiting = { $percent } af tiden gik med at vente på andet arbejde
open-report-stage = { $stage } tog { $percent } af tiden
open-report-memory-short = Hukommelsen var knap, så systemet måtte måske bruge swap

## Why a decode ran on the processor although a hardware decoder can do it
open-report-sw-not-probed = Afkodet i software: hardwareafkoderen var stadig ved at blive fundet
open-report-sw-no-device = Afkodet i software: { $api } er ikke tilgængelig på denne computer
open-report-sw-no-codec = Afkodet i software: { $api } har ingen { $codec }-afkoder på denne GPU
open-report-sw-blocked-mismatch = Afkodet i software: hardwareafkoderen blev slået fra, fordi dens resultat ikke passede med CPU'ens
open-report-sw-blocked-error = Afkodet i software: hardwareafkoderen blev slået fra efter en fejl
open-report-sw-blocked-lost = Afkodet i software: hardwareafkoderen blev slået fra, fordi den holdt op med at svare
open-report-sw-failed = Afkodet i software: hardwareafkoderen fejlede på dette billede
open-report-sw-busy = Afkodet i software: hardwareafkoderen var optaget
open-report-sw-unsupported = Afkodet i software: hardwareafkoderen kan ikke håndtere dette billede
open-report-sw-unavailable = Afkodet i software: hardwareafkodning er slået fra, indtil { -app-name } genstartes
open-report-sw-cpu-only = Afkodet i software: behandlingen er sat til “Kun CPU”
open-report-sw-setting-off = Afkodet i software: hardwareafkodning er slået fra i indstillingerne
open-report-sw-unchecked = Afkodet i software: hardwareafkoderen er ikke kontrolleret endnu (den prøves først på et billede, der forberedes i baggrunden)
open-report-sw-lower-power = Afkodet i software: på batteri bruger CPU'en mindre strøm til dette
open-report-sw-faster = Afkodet i software: hurtigere på denne computer (ca. { $cpu } ms mod { $hw } ms med hardwareafkoderen)
open-report-sw-scaled = Afkodet i software: hardwareafkoderen afkoder kun billeder i fuld størrelse

## Hints
open-report-hint-copy-local = Kopiér filen til en lokal disk for at åbne den hurtigere.
open-report-hint-extract = Pak arkivet ud i en mappe for at åbne siderne hurtigere.
open-report-hint-try-again = For at prøve hardwareafkoderen igen: Indstillinger → Ydeevne → Prøv igen.
open-report-hint-automatic = Sæt “Hvor billeder behandles” til Automatisk under Indstillinger → Ydeevne.
open-report-hint-turn-on = Sæt “Hardwareafkodning” til Automatisk under Indstillinger → Ydeevne.
open-report-hint-free-memory = Luk andre programmer for at frigøre hukommelse.
open-report-hint-once-only = Kun det første billede kontrolleres på denne måde.

## Rows
open-report-total = Til skarpt
open-report-first-pixels = Første pixels
# machine-drafted, needs native review
open-report-whole = Hele billedet vist
open-report-stage-startup = Opstart
open-report-stage-read = Læse filen
open-report-stage-parse = Læse headeren
open-report-stage-colour = Forberede farvestyring
open-report-stage-decode = Afkodning
open-report-stage-upload = Sende til GPU'en
open-report-stage-waiting = Vente på andet arbejde
open-report-decoder = Afkodet af
open-report-decoder-checked = { $engine }, kontrolleret mod CPU'en
open-report-decoder-cache = Visningscache
# $scale is the reduction: 4 for a level a quarter of the full width and height.
open-report-decoder-scaled = { $engine }, i 1/{ $scale } størrelse
open-report-decoder-scaled-full = { $engine }, i fuld størrelse, derefter formindsket til 1/{ $scale }
open-report-decoder-scaled-no-index = { $engine }, i 1/{ $scale } størrelse, uden hurtigstartindekset
open-report-colour-conversion = Farver omregnet af

## Advisor: a pattern across recent opens, what would help and by how much (rows)
# $count and $total are counts of opens, $format a file format ("HEIC"), $speed a formatted
# number (MB/s), $low and $high formatted numbers ("2.5").
open-report-advice-recurring = Tilbagevendende
open-report-advice-change = Ville hjælpe
open-report-advice-gain = Anslået gevinst
open-report-advice-software = { $count } af de seneste { $total } langsomme { $format }-billeder blev afkodet i software af denne grund
open-report-advice-slow-storage = { $count } af de seneste { $total } langsomme billeder blev forsinket af læsningen af filen (ca. { $speed } MB/s)
# machine-drafted, needs native review
open-report-advice-slow-storage-no-speed = { $count } af de seneste { $total } langsomme billeder blev forsinket af læsningen af filen (små filer: mest ventetid på, at lageret svarer)
open-report-advice-archive = { $count } af de seneste { $total } langsomme billeder blev forsinket af udpakningen af arkivet
open-report-advice-hardware-decoder = En GPU med en { $codec }-hardwareafkoder
open-report-advice-gain-range = Ca. { $low } til { $high } gange hurtigere
open-report-advice-gain-one = Ca. { $low } gange hurtigere

## Performance insights: the indicator in the corner and the panel it opens
# machine-drafted, needs native review
# $total is a formatted duration ("240 ms"), $format a file format ("HEIC").
open-report-insights-title = Indsigt i ydeevne
open-report-insights-indicator = Hvorfor er det langsomt?
open-report-insights-announce = Indsigt i ydeevne: billeder bliver ved med at åbne langsomt af samme grund. Åbn “Hvorfor er det langsomt?” for at se, hvad der ville hjælpe.
open-report-insights-hide = Skjul
open-report-insights-last = Seneste langsomme billede: { $total } til skarpt
open-report-insights-now = Det kan du gøre nu
open-report-insights-hardware = Hardware, der ville hjælpe
open-report-insights-none = Ingen tilbagevendende forsinkelse
open-report-insights-none-detail = Indsigt vises, når flere billeder åbner langsomt af samme grund. For at finde dem gemmer { -app-name } en uges langsomme åbninger på denne computer; intet sendes nogen steder hen.
open-report-insights-software = { $format }-billeder afkodes på CPU'en
open-report-insights-storage = Læsningen af filerne forsinker billederne
open-report-insights-archive = Udpakningen af arkiver forsinker billederne
open-report-insights-nothing-now = Ingen indstilling i { -app-name } ændrer dette.
open-report-insights-display-cache = Slå visningscachen til: billeder, du har åbnet før, åbner så fra en hurtig kopi på denne computer.
open-report-insights-prefetch = Hold flere nabobilleder klar, så de næste forberedes, mens du kigger.
open-report-insights-hw-ssd = Et hurtigt internt drev (en NVMe-SSD) til disse filer
open-report-insights-action-turn-on = Slå til
open-report-insights-action-try-again = Prøv igen
open-report-insights-action-automatic = Sæt til Automatisk
open-report-insights-dismiss-storage = Vis ikke igen for lager
open-report-insights-dismiss-decoder = Vis ikke igen for afkodning
open-report-insights-off = Slå indsigt fra
open-report-insights-on = Slå indsigt til
open-report-insights-restore = Vis skjulte indsigter igen
open-report-insights-power = Denne computer sparer strøm (på batteri eller i en strømsparetilstand), hvilket gør afkodningen på CPU'en langsommere. Med strømmen tilsluttet åbner billederne måske hurtigere.

## Performance insights in the gallery: the same, about making thumbnails
# machine-drafted, needs native review
# $count and $total are counts of thumbnails, $total in -last-thumbnail a formatted duration
# ("240 ms"), $format a file format ("HEIC"), $speed a formatted number (MB/s).
open-report-insights-announce-thumbnails = Indsigt i ydeevne: miniaturer bliver ved med at blive vist langsomt af samme grund. Åbn “Hvorfor er det langsomt?” for at se, hvad der ville hjælpe.
open-report-insights-last-thumbnail = Seneste langsomme miniature: { $total } at lave
open-report-insights-thumbnails-software = Miniaturer af { $format }-billeder afkodes på CPU'en
open-report-insights-thumbnails-storage = Læsningen af filerne forsinker miniaturerne
open-report-insights-thumbnails-archive = Udpakningen af arkiver forsinker miniaturerne
open-report-insights-thumbnail-cache = Miniaturer kan ikke gemmes på denne computer (miniaturecachen er ikke tilgængelig), så de laves igen, hver gang mappen vises.
open-report-advice-thumbnails-software = { $count } af de seneste { $total } langsomme { $format }-miniaturer blev afkodet i software af denne grund
open-report-advice-thumbnails-slow-storage = { $count } af de seneste { $total } langsomme miniaturer blev forsinket af læsningen af filen (ca. { $speed } MB/s)
# machine-drafted, needs native review
open-report-advice-thumbnails-slow-storage-no-speed = { $count } af de seneste { $total } langsomme miniaturer blev forsinket af læsningen af filen (små filer: mest ventetid på, at lageret svarer)
open-report-advice-thumbnails-archive = { $count } af de seneste { $total } langsomme miniaturer blev forsinket af udpakningen af arkivet
