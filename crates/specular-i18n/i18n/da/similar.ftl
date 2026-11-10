# machine-drafted, needs native review
# Specular — Dubletter og lignende billeder (gallerikategorier). Dansk.

similar-duplicates = Dubletter
similar-similar = Lignende billeder

## Punkter i Vis-menuen og paletten
similar-menu-entry = { $name } — { $groups ->
    [one] { $groups } gruppe
   *[other] { $groups } grupper
}, { $size } kan frigøres
similar-menu-scanning = { $name } — søger ({ PERCENT($percent) })
similar-menu-none = { $name } — ingen fundet

## Overskrift
similar-summary = { $groups ->
    [one] { $groups } gruppe at gennemgå
   *[other] { $groups } grupper at gennemgå
}, { $files ->
    [one] { $files } fil
   *[other] { $files } filer
} at kassere, { $size } kan frigøres
similar-progress = { $resolved } af { $total } afgjort
similar-skipped = { $count } sprunget over
similar-scanning = Søger efter dubletter og lignende billeder… { $done } af { $total }
# machine-drafted, needs native review
similar-checking-sharpness = Kontrollerer skarphed… { $done } af { $total }
# machine-drafted, needs native review
# Listen, mens den første scanning kører (dens fremskridt vises i overskriften ovenfor).
similar-waiting = Grupperne vises her, når scanningen er færdig.
# machine-drafted, needs native review
similar-stopped = Scanningen er stoppet: intet grupperet
similar-stopped-sharpness = Kontrol af skarphed stoppet: bedste valg ud fra miniaturer
similar-stopped-empty = Scanningen blev stoppet. Det, den har målt, gemmes: Scan igen fortsætter derfra.
similar-scan-again = Scan igen
similar-empty-duplicates = Ingen dubletter i mappen.
similar-empty-similar = Ingen lignende billeder i mappen.
similar-all-done = Alle grupper er gennemgået.

## Lignende billeder: hvor ens fotos skal være for at blive grupperet
similar-sensitivity = Følsomhed
similar-sensitivity-strict = Streng
similar-sensitivity-loose = Løs

## Bunden af gruppelisten
similar-accept-all = Accepter alle forslag
similar-show-skipped = Vis oversprungne grupper
similar-discard-with = Kassér: { $action }
similar-action-trash = Flyt til papirkurven
similar-action-move = Flyt til en mappe…
similar-action-move-to = Flyt til ”{ $folder }”
similar-action-reject = Markér kun som afvist
similar-choose-folder = Vælg mappe til kasserede filer…

## Samlet bekræftelse
similar-confirm = Vil du acceptere forslagene for { $groups ->
    [one] { $groups } gruppe
   *[other] { $groups } grupper
}? { $files ->
    [one] { $files } fil
   *[other] { $files } filer
} ({ $size }): { $action }.
similar-confirm-yes = { $files ->
    [one] Accepter og kassér { $files } fil
   *[other] Accepter og kassér { $files } filer
}
similar-cancel = Annuller

## En gruppe
similar-group-title = Gruppe { $index } af { $total }
similar-group-photos = { $count ->
    [one] { $count } foto
   *[other] { $count } fotos
}
similar-to-free = { $size } kan frigøres

## Gennemgangshandlinger og tilstande
similar-keep = Behold
similar-discard = Kassér
similar-undecided = Ikke afgjort
similar-accept-next = Accepter og næste
similar-not-similar = Ikke ens
similar-skip = Spring over
similar-compare = Sammenlign
similar-zoom = { PERCENT(100) }
similar-back = Tilbage
similar-done = Udført
similar-suggested-keep = Forslag: behold
similar-suggested-discard = Forslag: kassér

## Meddelelser
similar-toast-accepted = { $count ->
    [one] { $count } gruppe accepteret
   *[other] { $count } grupper accepteret
}
similar-toast-rejected = { $count ->
    [one] { $count } fil markeret som afvist
   *[other] { $count } filer markeret som afvist
}
similar-toast-protected = Fotos med bedømmelse, flag, etiket, nøgleord eller redigeringer beholdes altid.
# machine-drafted, needs native review
similar-zoom-already = Dette foto vises allerede i { PERCENT(100) } eller større

## Tilgængelighed
similar-a11y-member = { $name }: { $state }, { $reason }

## Tastaturtip
similar-hint-list = ↑↓ vælg · Enter gennemgå · A accepter alle · Tab skift · Esc luk

## Hvorfor et foto beholdes
similar-reason-rated = Bedømt
similar-reason-flagged = Markeret som udvalgt
similar-reason-labelled = Har en farveetiket
similar-reason-tagged = Har nøgleord
similar-reason-edited = Har redigeringer
similar-reason-original = Originalen
similar-reason-raw-original = RAW-original
similar-reason-highest-resolution = Højeste opløsning
similar-reason-deeper-colour = Større farvedybde
similar-reason-least-compressed = Mindst komprimeret
similar-reason-has-metadata = Bevarer sine metadata
similar-reason-oldest = Ældste kopi
similar-reason-sharpest = Skarpest
similar-reason-steadiest = Mindst bevægelsesslør
similar-reason-best-exposure = Bedst eksponeret
similar-reason-least-noise = Mindst støj
similar-reason-also-good = Også et godt billede

## Hvorfor et foto foreslås kasseret
similar-reason-same-file = Identisk fil
similar-reason-derivative = Afledt kopi
similar-reason-rendering-of-raw = Gengivelse af RAW-filen
similar-reason-smaller-copy = Mindre kopi
similar-reason-shallower-colour = Mindre farvedybde
similar-reason-more-compressed = Mere komprimeret
similar-reason-no-metadata = Metadata fjernet
similar-reason-newer-copy = Nyere kopi
similar-reason-blurrier = Mere uskarp
similar-reason-motion-blur = Bevægelsesslør
similar-reason-overexposed = Overeksponeret
similar-reason-underexposed = Undereksponeret
similar-reason-noisier = Mere støj
similar-reason-lower-resolution = Lavere opløsning
