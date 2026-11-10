# machine-drafted, needs native review
# Specular — Baggrundsopgaver: indikatoren, mens noget kører i baggrunden, og panelet, den åbner. Dansk.

## Panelet

work-title = Baggrundsopgaver
work-show = Vis baggrundsopgaver
work-empty = Intet kører i baggrunden.

## Opgaver

work-kind-library = Fotobibliotek
work-kind-ai-search = AI-søgning
work-kind-faces = Personer
work-kind-text = Tekst i fotos
work-kind-culling = Sorteringsscorer
work-kind-download = Download af AI-model
work-kind-similar = Lignende billeder

## Trin

work-step-finding = Finder fotos
work-step-reading = Læser fotodetaljer
work-step-signatures = Sammenligner fotos
work-step-grouping = Grupperer lignende billeder
work-step-text-index = Opbygger søgeindekset
work-step-embedding = Indekserer fotos til AI-søgning
work-step-switching = Indekserer igen til den nye modelstørrelse
work-step-faces = Finder ansigter
work-step-ocr = Læser tekst i fotos
work-step-scoring = Vurderer skarphed og eksponering
work-step-downloading = Downloader
work-step-focus = Kontrollerer skarphed
work-step-item = { $step } · { $item }

## Fremskridt

work-count-photos = { $done } af { $total } { $count ->
    [one] foto
   *[other] fotos
}
work-count = { $done } af { $total }
work-count-left = { $count } · { $left } tilbage
work-so-far-photos = { $done } { $count ->
    [one] foto
   *[other] fotos
} indtil videre
work-so-far = { $done } indtil videre

## Resterende tid

work-eta-estimating = Beregner…
work-eta-seconds = Ca. { $n ->
    [one] { $n } sekund
   *[other] { $n } sekunder
} tilbage
work-eta-minutes = Ca. { $n ->
    [one] { $n } minut
   *[other] { $n } minutter
} tilbage
work-eta-hours = Ca. { $h ->
    [one] { $h } time
   *[other] { $h } timer
} tilbage
work-eta-hours-minutes = Ca. { $h ->
    [one] { $h } time
   *[other] { $h } timer
} og { $m ->
    [one] { $m } minut
   *[other] { $m } minutter
} tilbage

## Hvorfor en opgave venter

work-paused-browsing = Sat på pause, mens du kigger billeder igennem
work-paused-battery = Sat på pause på batteristrøm
work-paused-low-power = Sat på pause i strømsparetilstand
work-paused-heat = Sat på pause, mens computeren er varm
work-paused-memory = Sat på pause, mens hukommelsen er lav
work-paused-user = Sat på pause

## Hvad der følger

work-then = Derefter: { $steps }

## Knapper

work-pause = Pause
work-resume = Genoptag
work-settings = Indstillinger…

## Indikatoren

work-chip = { $job } · { $done } af { $total }
work-chip-so-far = { $job } · { $done }
work-chip-paused = { $job } · { $reason }
work-chip-more = { $chip } (+{ $more })
work-chip-name = Baggrundsopgaver: { $status }

## Meddelelser

work-started = { $job } startet i baggrunden
work-finished = { $job } fuldført
