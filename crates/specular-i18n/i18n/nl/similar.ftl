# machine-drafted, needs native review
# Specular — Duplicaten en vergelijkbare foto’s (galerijcategorieën). Nederlands.

similar-duplicates = Duplicaten
similar-similar = Vergelijkbare foto’s

## Weergavemenu en paletitems
similar-menu-entry = { $name } — { $groups ->
    [one] { $groups } groep
   *[other] { $groups } groepen
}, { $size } vrij te maken
similar-menu-scanning = { $name } — zoeken ({ PERCENT($percent) })
similar-menu-none = { $name } — niets gevonden

## Kop
similar-summary = { $groups ->
    [one] { $groups } groep te beoordelen
   *[other] { $groups } groepen te beoordelen
}, { $files ->
    [one] { $files } bestand
   *[other] { $files } bestanden
} te verwijderen, { $size } vrij te maken
similar-progress = { $resolved } van { $total } afgehandeld
similar-skipped = { $count } overgeslagen
similar-scanning = Duplicaten en vergelijkbare foto’s zoeken… { $done } van { $total }
# machine-drafted, needs native review
similar-checking-sharpness = Scherpte controleren… { $done } van { $total }
# machine-drafted, needs native review
# De lijst terwijl de eerste scan loopt (de voortgang staat in de kop erboven).
similar-waiting = Groepen verschijnen hier wanneer de scan klaar is.
# machine-drafted, needs native review
similar-stopped = Scan gestopt: niets gegroepeerd
similar-stopped-sharpness = Scherptecontrole gestopt: beste keuzes op basis van miniaturen
similar-stopped-empty = De scan is gestopt. Wat al is gemeten, blijft bewaard: Opnieuw scannen gaat daar verder.
similar-scan-again = Opnieuw scannen
similar-empty-duplicates = Geen duplicaten in deze map.
similar-empty-similar = Geen vergelijkbare foto’s in deze map.
similar-all-done = Alle groepen beoordeeld.

## Vergelijkbare foto’s: hoeveel foto’s op elkaar moeten lijken om te groeperen
similar-sensitivity = Gevoeligheid
similar-sensitivity-strict = Streng
similar-sensitivity-loose = Ruim

## Voettekst van de groepslijst
similar-accept-all = Alle voorstellen accepteren
similar-show-skipped = Overgeslagen groepen tonen
similar-discard-with = Weggooien: { $action }
similar-action-trash = Naar prullenbak verplaatsen
similar-action-move = Naar een map verplaatsen…
similar-action-move-to = Naar ‘{ $folder }’ verplaatsen
similar-action-reject = Alleen als afgewezen markeren
similar-choose-folder = Map kiezen voor weggegooide bestanden…

## Bevestiging voor alles tegelijk
similar-confirm = De voorstellen van { $groups ->
    [one] { $groups } groep
   *[other] { $groups } groepen
} accepteren? { $files ->
    [one] { $files } bestand
   *[other] { $files } bestanden
} ({ $size }): { $action }.
similar-confirm-yes = { $files ->
    [one] Accepteren en { $files } bestand weggooien
   *[other] Accepteren en { $files } bestanden weggooien
}
similar-cancel = Annuleren

## Een groep
similar-group-title = Groep { $index } van { $total }
similar-group-photos = { $count ->
    [one] { $count } foto
   *[other] { $count } foto’s
}
similar-to-free = { $size } vrij te maken

## Beoordelingsacties en -toestanden
similar-keep = Behouden
similar-discard = Weggooien
similar-undecided = Onbeslist
similar-accept-next = Accepteren en volgende
similar-not-similar = Niet vergelijkbaar
similar-skip = Overslaan
similar-compare = Vergelijken
similar-zoom = { PERCENT(100) }
similar-back = Terug
similar-done = Klaar
similar-suggested-keep = Voorstel: behouden
similar-suggested-discard = Voorstel: weggooien

## Meldingen
similar-toast-accepted = { $count ->
    [one] { $count } groep geaccepteerd
   *[other] { $count } groepen geaccepteerd
}
similar-toast-rejected = { $count ->
    [one] { $count } bestand als afgewezen gemarkeerd
   *[other] { $count } bestanden als afgewezen gemarkeerd
}
similar-toast-protected = Foto’s met een beoordeling, markering, label, trefwoorden of bewerkingen worden altijd behouden.
# machine-drafted, needs native review
similar-zoom-already = Deze foto wordt al op { PERCENT(100) } of groter getoond

## Toegankelijkheid
similar-a11y-member = { $name }: { $state }, { $reason }

## Toetstips
similar-hint-list = ↑↓ kiezen · Enter beoordelen · A alles accepteren · Tab wisselen · Esc sluiten

## Waarom een foto wordt behouden
similar-reason-rated = Beoordeeld
similar-reason-flagged = Gemarkeerd als keuze
similar-reason-labelled = Heeft een kleurlabel
similar-reason-tagged = Heeft trefwoorden
similar-reason-edited = Heeft bewerkingen
similar-reason-original = Het origineel
similar-reason-raw-original = RAW-origineel
similar-reason-highest-resolution = Hoogste resolutie
similar-reason-deeper-colour = Grotere kleurdiepte
similar-reason-least-compressed = Minst gecomprimeerd
similar-reason-has-metadata = Behoudt metagegevens
similar-reason-oldest = Oudste kopie
similar-reason-sharpest = Scherpste
similar-reason-steadiest = Minste bewegingsonscherpte
similar-reason-best-exposure = Best belicht
similar-reason-least-noise = Minste ruis
similar-reason-also-good = Ook een goede

## Waarom wordt voorgesteld een foto weg te gooien
similar-reason-same-file = Identiek bestand
similar-reason-derivative = Afgeleide kopie
similar-reason-rendering-of-raw = Weergave van de RAW
similar-reason-smaller-copy = Kleinere kopie
similar-reason-shallower-colour = Minder kleurdiepte
similar-reason-more-compressed = Sterker gecomprimeerd
similar-reason-no-metadata = Metagegevens verwijderd
similar-reason-newer-copy = Nieuwere kopie
similar-reason-blurrier = Onscherper
similar-reason-motion-blur = Bewegingsonscherpte
similar-reason-overexposed = Overbelicht
similar-reason-underexposed = Onderbelicht
similar-reason-noisier = Meer ruis
similar-reason-lower-resolution = Lagere resolutie
