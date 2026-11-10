# machine-drafted, needs native review
# Specular — Gallringsläge (DESIGN.md §6.3).

palette-cull = Gå till eller lämna gallringsläget
cheatsheet-cull = Gallringsläge
settings-section-cull = Gallringsläge
settings-cull-auto-enter = Gallra RAW-mappar
settings-cull-auto-enter-help = Öppna en mapp med mest RAW-filer direkt i gallringsläget.
settings-cull-auto-advance = Nästa bild efter markering
settings-cull-auto-advance-help = Gå vidare efter val, avvisning eller stjärnbetyg. Etiketter och avmarkering stannar på bilden.
settings-cull-show = Visa för RAW+JPEG-par
settings-cull-show-help = En RAW- och en JPEG-fil med samma namn gallras som en bild och markeringar gäller båda. JPEG-filen öppnas snabbare.
settings-cull-show-jpeg = JPEG
settings-cull-show-raw = RAW

# machine-drafted, needs native review
settings-cull-bursts = Stapla serier
settings-cull-bursts-help = Bilder tagna i snabb följd med en kamera visas som en stapel. S öppnar eller stänger en stapel, B behåller bilden som visas och avvisar resten av stapeln.
settings-cull-burst-gap = Seriepaus
settings-cull-burst-gap-help = Den längsta pausen mellan två bilder i en serie.
settings-cull-watch = Visa nya filer
settings-cull-watch-help = Filer som dyker upp i mappen medan du gallrar (en ansluten kamera, en import) läggs till sist när de är helt skrivna.
settings-cull-follow-newest = Följ den nyaste filen
settings-cull-follow-newest-help = Hoppa till varje ny fil när den kommer. T växlar detta under gallring.
settings-cull-slot-1 = Mapp 1
settings-cull-slot-1-help = Alt+1 kopierar bilden (med dess RAW eller JPEG och sidofiler) till den här mappen, Alt+Skift+1 flyttar den dit.
settings-cull-slot-1-unset = Inte vald
settings-cull-slot-2 = Mapp 2
settings-cull-slot-2-help = Alt+2 kopierar bilden (med dess RAW eller JPEG och sidofiler) till den här mappen, Alt+Skift+2 flyttar den dit.
settings-cull-slot-2-unset = Inte vald
settings-cull-slot-3 = Mapp 3
settings-cull-slot-3-help = Alt+3 kopierar bilden (med dess RAW eller JPEG och sidofiler) till den här mappen, Alt+Skift+3 flyttar den dit.
settings-cull-slot-3-unset = Inte vald
settings-cull-collisions = När ett namn är upptaget
settings-cull-collisions-help = Filer i mapparna ersätts aldrig.
settings-cull-collisions-suffix = Lägg till ett nummer
settings-cull-collisions-skip = Hoppa över bilden
# machine-drafted, needs native review
# Gallringsläget valt utan att någon mapp eller några galleribilder är öppna.
cull-nothing = Inget att gallra – öppna först en mapp med foton
cull-slot-unset = Ingen mapp { $slot } vald – välj en i Inställningar → Gallringsläge
cull-slot-missing = Mapp { $slot } är inte tillgänglig: { $reason }
cull-copied = Kopierad till { $folder }
cull-moved = Flyttad till { $folder }
cull-slot-renamed = Sparad som { $name } i { $folder }
cull-slot-skipped = { $folder } har redan { $name } – hoppades över
cull-slot-same = Bilden finns redan i { $folder }
cull-slot-failed = { $op ->
    [move] Det gick inte att flytta { $file }
   *[copy] Det gick inte att kopiera { $file }
}: { $reason }
cull-copy-kept = { $count ->
    [one] Behöll en kopia som ändrats sedan dess
   *[other] Behöll { $count } kopior som ändrats sedan dess
}
cull-undo-failed = Det gick inte att ångra för { $file }: { $reason }
cull-burst-open = Serie { $index } av { $count }
cull-burst-closed = Serie med { $count }
cull-burst-toggle = Öppna eller stäng serien
cull-burst-best = 1 behållen, { $count } avvisade
cull-burst-none = Inte del av en serie
cull-follow-on = Följer nya filer
cull-follow-off = Följer inte nya filer
cull-new-files = { $count ->
    [one] 1 ny fil
   *[other] { $count } nya filer
}
cull-key-copy = Kopiera till mapp 1–3
cull-key-move = Flytta till mapp 1–3
cull-key-burst = Öppna eller stäng en serie
cull-key-best = Behåll den här, avvisa resten av serien
cull-key-follow = Följ nya filer
cull-key-subframes = Visa eller dölj tabellen med delbilder
cull-key-sort = Sortera efter nästa kolumn
cull-key-sort-reverse = Vänd ordningen
cull-key-metric = Visa nästa mått i diagrammet
cull-key-sigma-down = Autoavvisning: sänk k
cull-key-sigma-up = Autoavvisning: höj k
cull-key-auto-reject = Avvisa de föreslagna bilderna
cull-key-move-rejects = Flytta avvisade bilder till en undermapp
cull-key-same-stretch = Samma sträckning för alla bilder

## Strip tooltips. machine-drafted, needs native review
cull-tip-burst-open = Öppna serien
cull-tip-burst-close = Stäng serien
cull-tip-exit = Lämna gallringsläget
cull-tip-label = Etikett: { $label }

## Astronomy subframes. machine-drafted, needs native review
cull-astro-panel = Delbilder
cull-astro-title = { $count ->
    [one] 1 delbild
   *[other] { $count } delbilder
}
cull-astro-pending = mäter { $count }
cull-astro-col-name = Bild
cull-astro-col-filter = Filter
cull-astro-col-exposure = Exp.
cull-astro-col-hfd = HFD
cull-astro-col-fwhm = FWHM
cull-astro-col-ecc = Exc.
cull-astro-col-stars = Stjärnor
cull-astro-col-bg = Bakgrund
cull-astro-col-snr = SNR
cull-astro-col-noise = Brus
cull-astro-same-stretch = Samma sträckning
cull-astro-own-stretch = Egen sträckning
cull-astro-suggest-high = { $metric } över { $value } (median + { $k }σ): { $count ->
    [one] 1 bild
   *[other] { $count } bilder
}
cull-astro-suggest-low = { $metric } under { $value } (median − { $k }σ): { $count ->
    [one] 1 bild
   *[other] { $count } bilder
}
cull-astro-suggest-none = Ingen bild bortom { $k }σ
cull-astro-suggest-few = Autoavvisning kräver 3 uppmätta bilder
cull-astro-suggested = föreslagen för avvisning
cull-astro-k = k { $k }
cull-astro-reject = Avvisa { $count }
cull-astro-move = Flytta avvisade…
cull-astro-chart = { $metric } över { $count ->
        [one] { $count } bild
       *[other] { $count } bilder
    }, median { $median }.
cull-astro-sort-by = Sortera efter { $column }
cull-astro-ascending = stigande
cull-astro-descending = fallande
cull-astro-sorted = Sorterat efter { $column }
cull-astro-reversed = Ordningen omvänd
cull-astro-metric = Diagram: { $metric }
cull-astro-k-set = Autoavvisning vid { $k }σ
cull-astro-none = Inga delbilder här
cull-astro-same-on = Samma sträckning för alla bilder
cull-astro-same-off = Varje bild sträcks för sig
cull-astro-line-hfd = HFD { $value } px
cull-astro-line-fwhm = FWHM { $value } px
cull-astro-line-fwhm-arcsec = FWHM { $value } px ({ $arcsec }″)
cull-astro-line-ecc = excentricitet { $value }
cull-astro-line-stars = { $count ->
    [one] 1 stjärna
   *[other] { $count } stjärnor
}
cull-astro-measuring = mäter stjärnor…
cull-astro-reject-confirm = { $count ->
    [one] Avvisa den föreslagna bilden?
   *[other] Avvisa de { $count } föreslagna bilderna?
}
cull-astro-reject-detail = { $rule }. Ångra tar tillbaka markeringarna.
cull-astro-reject-ok = Avvisa
cull-astro-nothing-to-reject = Ingen bild att avvisa
cull-astro-rejected = { $count ->
    [one] Avvisade 1 bild
   *[other] Avvisade { $count } bilder
}
cull-move-rejects-confirm = { $count ->
    [one] Flytta den avvisade bilden till mappen ”{ $folder }”?
   *[other] Flytta de { $count } avvisade bilderna till mappen ”{ $folder }”?
}
cull-move-rejects-detail = Filerna flyttas, de raderas inte. Ångra flyttar tillbaka dem.
cull-move-rejects-ok = Flytta
cull-move-rejects-none = Inga avvisade bilder att flytta
cull-moved-rejects = { $count ->
    [one] Flyttade 1 bild till ”{ $folder }”
   *[other] Flyttade { $count } bilder till ”{ $folder }”
}
