# machine-drafted, needs native review
# Specular — Achtergrondtaken: de indicator terwijl er iets op de achtergrond loopt, en het paneel dat hij opent. Nederlands.

## Het paneel

work-title = Achtergrondtaken
work-show = Achtergrondtaken tonen
work-empty = Er loopt niets op de achtergrond.

## Taken

work-kind-library = Fotobibliotheek
work-kind-ai-search = AI-zoeken
work-kind-faces = Personen
work-kind-text = Tekst in foto’s
work-kind-culling = Selectiescores
work-kind-download = Download van AI-model
work-kind-similar = Vergelijkbare foto’s

## Stappen

work-step-finding = Foto’s zoeken
work-step-reading = Fotogegevens lezen
work-step-signatures = Foto’s vergelijken
work-step-grouping = Vergelijkbare foto’s groeperen
work-step-text-index = Zoekindex opbouwen
work-step-embedding = Foto’s indexeren voor AI-zoeken
work-step-switching = Opnieuw indexeren voor de nieuwe modelgrootte
work-step-faces = Gezichten zoeken
work-step-ocr = Tekst in foto’s lezen
work-step-scoring = Scherpte en belichting beoordelen
work-step-downloading = Downloaden
work-step-focus = Scherpte controleren
work-step-item = { $step } · { $item }

## Voortgang

work-count-photos = { $done } van { $total } { $count ->
    [one] foto
   *[other] foto’s
}
work-count = { $done } van { $total }
work-count-left = { $count } · nog { $left }
work-so-far-photos = Tot nu toe { $done } { $count ->
    [one] foto
   *[other] foto’s
}
work-so-far = Tot nu toe { $done }

## Resterende tijd

work-eta-estimating = Schatten…
work-eta-seconds = Nog ongeveer { $n ->
    [one] { $n } seconde
   *[other] { $n } seconden
}
work-eta-minutes = Nog ongeveer { $n ->
    [one] { $n } minuut
   *[other] { $n } minuten
}
work-eta-hours = Nog ongeveer { $h ->
    [one] { $h } uur
   *[other] { $h } uur
}
work-eta-hours-minutes = Nog ongeveer { $h ->
    [one] { $h } uur
   *[other] { $h } uur
} en { $m ->
    [one] { $m } minuut
   *[other] { $m } minuten
}

## Waarom een taak wacht

work-paused-browsing = Gepauzeerd terwijl je bladert
work-paused-battery = Gepauzeerd op batterijvoeding
work-paused-low-power = Gepauzeerd in energiebesparingsmodus
work-paused-heat = Gepauzeerd zolang de computer warm is
work-paused-memory = Gepauzeerd zolang er weinig geheugen is
work-paused-user = Gepauzeerd

## Wat volgt

work-then = Daarna: { $steps }

## Knoppen

work-pause = Pauzeren
work-resume = Hervatten
work-settings = Instellingen…

## De indicator

work-chip = { $job } · { $done } van { $total }
work-chip-so-far = { $job } · { $done }
work-chip-paused = { $job } · { $reason }
work-chip-more = { $chip } (+{ $more })
work-chip-name = Achtergrondtaken: { $status }

## Meldingen

work-started = { $job } gestart op de achtergrond
work-finished = { $job } voltooid
