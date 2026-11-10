# machine-drafted, needs native review
# Specular — Dubbletter och liknande bilder (gallerikategorier). Svenska.

similar-duplicates = Dubbletter
similar-similar = Liknande bilder

## Vymeny och palettposter
similar-menu-entry = { $name } — { $groups ->
    [one] { $groups } grupp
   *[other] { $groups } grupper
}, { $size } att frigöra
similar-menu-scanning = { $name } — söker ({ PERCENT($percent) })
similar-menu-none = { $name } — inga hittades

## Rubrik
similar-summary = { $groups ->
    [one] { $groups } grupp att granska
   *[other] { $groups } grupper att granska
}, { $files ->
    [one] { $files } fil
   *[other] { $files } filer
} att kassera, { $size } att frigöra
similar-progress = { $resolved } av { $total } klara
similar-skipped = { $count } överhoppade
similar-scanning = Letar efter dubbletter och liknande bilder… { $done } av { $total }
# machine-drafted, needs native review
similar-checking-sharpness = Kontrollerar skärpa… { $done } av { $total }
# machine-drafted, needs native review
# Listan medan den första sökningen pågår (förloppet visas i rubriken ovanför).
similar-waiting = Grupper visas här när sökningen är klar.
# machine-drafted, needs native review
similar-stopped = Sökningen stoppad: inget grupperat
similar-stopped-sharpness = Skärpekontrollen stoppad: bästa val utifrån miniatyrer
similar-stopped-empty = Sökningen stoppades. Det som mättes sparas: Sök igen fortsätter därifrån.
similar-scan-again = Sök igen
similar-empty-duplicates = Inga dubbletter i den här mappen.
similar-empty-similar = Inga liknande bilder i den här mappen.
similar-all-done = Alla grupper har granskats.

## Liknande bilder: hur lika foton måste vara för att grupperas
similar-sensitivity = Känslighet
similar-sensitivity-strict = Strikt
similar-sensitivity-loose = Generös

## Sidfot för grupplistan
similar-accept-all = Godkänn alla förslag
similar-show-skipped = Visa överhoppade grupper
similar-discard-with = Kassera: { $action }
similar-action-trash = Flytta till papperskorgen
similar-action-move = Flytta till en mapp…
similar-action-move-to = Flytta till ”{ $folder }”
similar-action-reject = Flagga endast som förkastad
similar-choose-folder = Välj mapp för kasserade filer…

## Massbekräftelse
similar-confirm = Godkänna förslagen för { $groups ->
    [one] { $groups } grupp
   *[other] { $groups } grupper
}? { $files ->
    [one] { $files } fil
   *[other] { $files } filer
} ({ $size }): { $action }.
similar-confirm-yes = { $files ->
    [one] Godkänn och kassera { $files } fil
   *[other] Godkänn och kassera { $files } filer
}
similar-cancel = Avbryt

## En grupp
similar-group-title = Grupp { $index } av { $total }
similar-group-photos = { $count ->
    [one] { $count } foto
   *[other] { $count } foton
}
similar-to-free = { $size } att frigöra

## Granskningsåtgärder och lägen
similar-keep = Behåll
similar-discard = Kassera
similar-undecided = Obestämd
similar-accept-next = Godkänn och nästa
similar-not-similar = Inte lika
similar-skip = Hoppa över
similar-compare = Jämför
similar-zoom = { PERCENT(100) }
similar-back = Tillbaka
similar-done = Klar
similar-suggested-keep = Förslag: behåll
similar-suggested-discard = Förslag: kassera

## Aviseringar
similar-toast-accepted = { $count ->
    [one] { $count } grupp godkändes
   *[other] { $count } grupper godkändes
}
similar-toast-rejected = { $count ->
    [one] { $count } fil flaggades som förkastad
   *[other] { $count } filer flaggades som förkastade
}
similar-toast-protected = Foton med betyg, flagga, etikett, nyckelord eller redigeringar behålls alltid.
# machine-drafted, needs native review
similar-zoom-already = Det här fotot visas redan i { PERCENT(100) } eller större

## Tillgänglighet
similar-a11y-member = { $name }: { $state }, { $reason }

## Tangenttips
similar-hint-list = ↑↓ välj · Retur granska · A godkänn alla · Tabb växla · Esc stäng

## Varför ett foto behålls
similar-reason-rated = Har betyg
similar-reason-flagged = Flaggad som vald
similar-reason-labelled = Har en färgetikett
similar-reason-tagged = Har nyckelord
similar-reason-edited = Har redigeringar
similar-reason-original = Originalet
similar-reason-raw-original = RAW-original
similar-reason-highest-resolution = Högst upplösning
similar-reason-deeper-colour = Större färgdjup
similar-reason-least-compressed = Minst komprimerad
similar-reason-has-metadata = Behåller sin metadata
similar-reason-oldest = Äldsta kopian
similar-reason-sharpest = Skarpast
similar-reason-steadiest = Minst rörelseoskärpa
similar-reason-best-exposure = Bäst exponerad
similar-reason-least-noise = Minst brus
similar-reason-also-good = Också en bra bild

## Varför ett foto föreslås för kassering
similar-reason-same-file = Identisk fil
similar-reason-derivative = Härledd kopia
similar-reason-rendering-of-raw = Framkallning av RAW-filen
similar-reason-smaller-copy = Mindre kopia
similar-reason-shallower-colour = Mindre färgdjup
similar-reason-more-compressed = Mer komprimerad
similar-reason-no-metadata = Metadata borttagen
similar-reason-newer-copy = Nyare kopia
similar-reason-blurrier = Suddigare
similar-reason-motion-blur = Rörelseoskärpa
similar-reason-overexposed = Överexponerad
similar-reason-underexposed = Underexponerad
similar-reason-noisier = Mer brus
similar-reason-lower-resolution = Lägre upplösning
