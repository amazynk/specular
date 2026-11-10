# machine-drafted, needs native review
# Specular — Bakgrundsarbete: indikatorn medan något körs i bakgrunden, och panelen den öppnar. Svenska.

## Panelen

work-title = Bakgrundsarbete
work-show = Visa bakgrundsarbete
work-empty = Inget körs i bakgrunden.

## Jobb

work-kind-library = Fotobibliotek
work-kind-ai-search = AI-sökning
work-kind-faces = Personer
work-kind-text = Text i foton
work-kind-culling = Gallringspoäng
work-kind-download = Nedladdning av AI-modell
work-kind-similar = Liknande bilder

## Steg

work-step-finding = Söker efter foton
work-step-reading = Läser fotodetaljer
work-step-signatures = Jämför foton
work-step-grouping = Grupperar liknande bilder
work-step-text-index = Bygger sökindexet
work-step-embedding = Indexerar foton för AI-sökning
work-step-switching = Indexerar om för den nya modellstorleken
work-step-faces = Söker efter ansikten
work-step-ocr = Läser text i foton
work-step-scoring = Bedömer skärpa och exponering
work-step-downloading = Laddar ned
work-step-focus = Kontrollerar skärpa
work-step-item = { $step } · { $item }

## Förlopp

work-count-photos = { $done } av { $total } { $count ->
    [one] foto
   *[other] foton
}
work-count = { $done } av { $total }
work-count-left = { $count } · { $left } kvar
work-so-far-photos = { $done } { $count ->
    [one] foto
   *[other] foton
} hittills
work-so-far = { $done } hittills

## Återstående tid

work-eta-estimating = Beräknar…
work-eta-seconds = Cirka { $n ->
    [one] { $n } sekund
   *[other] { $n } sekunder
} kvar
work-eta-minutes = Cirka { $n ->
    [one] { $n } minut
   *[other] { $n } minuter
} kvar
work-eta-hours = Cirka { $h ->
    [one] { $h } timme
   *[other] { $h } timmar
} kvar
work-eta-hours-minutes = Cirka { $h ->
    [one] { $h } timme
   *[other] { $h } timmar
} och { $m ->
    [one] { $m } minut
   *[other] { $m } minuter
} kvar

## Varför ett jobb väntar

work-paused-browsing = Pausat medan du bläddrar
work-paused-battery = Pausat vid batteridrift
work-paused-low-power = Pausat i energisparläge
work-paused-heat = Pausat medan datorn är varm
work-paused-memory = Pausat medan det är ont om minne
work-paused-user = Pausat

## Vad som följer

work-then = Sedan: { $steps }

## Knappar

work-pause = Pausa
work-resume = Återuppta
work-settings = Inställningar…

## Indikatorn

work-chip = { $job } · { $done } av { $total }
work-chip-so-far = { $job } · { $done }
work-chip-paused = { $job } · { $reason }
work-chip-more = { $chip } (+{ $more })
work-chip-name = Bakgrundsarbete: { $status }

## Meddelanden

work-started = { $job } startade i bakgrunden
work-finished = { $job }: klart
