# machine-drafted, needs native review
# Specular — Sorteringstilstand (DESIGN.md §6.3).

palette-cull = Gå til eller forlad sorteringstilstand
cheatsheet-cull = Sorteringstilstand
settings-section-cull = Sorteringstilstand
settings-cull-auto-enter = Sortér RAW-mapper
settings-cull-auto-enter-help = Åbn en mappe med mest RAW-filer direkte i sorteringstilstand.
settings-cull-auto-advance = Næste billede efter markering
settings-cull-auto-advance-help = Gå videre efter valg, afvisning eller stjernebedømmelse. Etiketter og fjernet markering bliver på billedet.
settings-cull-show = Vis for RAW+JPEG-par
settings-cull-show-help = En RAW- og en JPEG-fil med samme navn sorteres som ét billede, og markeringer gælder begge. JPEG-filen åbner hurtigere.
settings-cull-show-jpeg = JPEG
settings-cull-show-raw = RAW

# machine-drafted, needs native review
settings-cull-bursts = Stak serier
settings-cull-bursts-help = Billeder taget hurtigt efter hinanden med ét kamera vises som én stak. S åbner eller lukker en stak, B beholder billedet på skærmen og afviser resten af stakken.
settings-cull-burst-gap = Seriepause
settings-cull-burst-gap-help = Den længste pause mellem to billeder i en serie.
settings-cull-watch = Vis nye filer
settings-cull-watch-help = Filer, der dukker op i mappen under sortering (et tilsluttet kamera, en import), tilføjes til sidst, når de er skrevet færdige.
settings-cull-follow-newest = Følg den nyeste fil
settings-cull-follow-newest-help = Spring til hver ny fil, når den kommer. T slår dette til og fra under sortering.
settings-cull-slot-1 = Mappe 1
settings-cull-slot-1-help = Alt+1 kopierer billedet (med dets RAW eller JPEG og sidefiler) til denne mappe, Alt+Skift+1 flytter det dertil.
settings-cull-slot-1-unset = Ikke valgt
settings-cull-slot-2 = Mappe 2
settings-cull-slot-2-help = Alt+2 kopierer billedet (med dets RAW eller JPEG og sidefiler) til denne mappe, Alt+Skift+2 flytter det dertil.
settings-cull-slot-2-unset = Ikke valgt
settings-cull-slot-3 = Mappe 3
settings-cull-slot-3-help = Alt+3 kopierer billedet (med dets RAW eller JPEG og sidefiler) til denne mappe, Alt+Skift+3 flytter det dertil.
settings-cull-slot-3-unset = Ikke valgt
settings-cull-collisions = Når et navn er optaget
settings-cull-collisions-help = Filer i mapperne erstattes aldrig.
settings-cull-collisions-suffix = Tilføj et nummer
settings-cull-collisions-skip = Spring billedet over
# machine-drafted, needs native review
# Sorteringstilstand valgt uden en mappe eller galleribilleder åbne.
cull-nothing = Intet at sortere – åbn først en mappe med fotos
cull-slot-unset = Ingen mappe { $slot } valgt – vælg en under Indstillinger → Sorteringstilstand
cull-slot-missing = Mappe { $slot } er ikke tilgængelig: { $reason }
cull-copied = Kopieret til { $folder }
cull-moved = Flyttet til { $folder }
cull-slot-renamed = Gemt som { $name } i { $folder }
cull-slot-skipped = { $folder } har allerede { $name } – sprunget over
cull-slot-same = Billedet er allerede i { $folder }
cull-slot-failed = { $op ->
    [move] Kunne ikke flytte { $file }
   *[copy] Kunne ikke kopiere { $file }
}: { $reason }
cull-copy-kept = { $count ->
    [one] Beholdt en kopi, der er ændret siden
   *[other] Beholdt { $count } kopier, der er ændret siden
}
cull-undo-failed = Kunne ikke fortryde for { $file }: { $reason }
cull-burst-open = Serie { $index } af { $count }
cull-burst-closed = Serie på { $count }
cull-burst-toggle = Åbn eller luk serien
cull-burst-best = 1 beholdt, { $count } afvist
cull-burst-none = Ikke del af en serie
cull-follow-on = Følger nye filer
cull-follow-off = Følger ikke nye filer
cull-new-files = { $count ->
    [one] 1 ny fil
   *[other] { $count } nye filer
}
cull-key-copy = Kopiér til mappe 1–3
cull-key-move = Flyt til mappe 1–3
cull-key-burst = Åbn eller luk en serie
cull-key-best = Behold denne, afvis resten af serien
cull-key-follow = Følg nye filer
cull-key-subframes = Vis eller skjul tabellen med delbilleder
cull-key-sort = Sortér efter næste kolonne
cull-key-sort-reverse = Vend rækkefølgen
cull-key-metric = Vis næste mål i diagrammet
cull-key-sigma-down = Autoafvisning: sænk k
cull-key-sigma-up = Autoafvisning: hæv k
cull-key-auto-reject = Afvis de foreslåede billeder
cull-key-move-rejects = Flyt afviste billeder til en undermappe
cull-key-same-stretch = Samme strækning for alle billeder

## Strip tooltips. machine-drafted, needs native review
cull-tip-burst-open = Åbn serien
cull-tip-burst-close = Luk serien
cull-tip-exit = Forlad sorteringstilstand
cull-tip-label = Etiket: { $label }

## Astronomy subframes. machine-drafted, needs native review
cull-astro-panel = Delbilleder
cull-astro-title = { $count ->
    [one] 1 delbillede
   *[other] { $count } delbilleder
}
cull-astro-pending = måler { $count }
cull-astro-col-name = Billede
cull-astro-col-filter = Filter
cull-astro-col-exposure = Eksp.
cull-astro-col-hfd = HFD
cull-astro-col-fwhm = FWHM
cull-astro-col-ecc = Eks.
cull-astro-col-stars = Stjerner
cull-astro-col-bg = Baggrund
cull-astro-col-snr = SNR
cull-astro-col-noise = Støj
cull-astro-same-stretch = Samme strækning
cull-astro-own-stretch = Egen strækning
cull-astro-suggest-high = { $metric } over { $value } (median + { $k }σ): { $count ->
    [one] 1 billede
   *[other] { $count } billeder
}
cull-astro-suggest-low = { $metric } under { $value } (median − { $k }σ): { $count ->
    [one] 1 billede
   *[other] { $count } billeder
}
cull-astro-suggest-none = Intet billede ud over { $k }σ
cull-astro-suggest-few = Autoafvisning kræver 3 målte billeder
cull-astro-suggested = foreslået til afvisning
cull-astro-k = k { $k }
cull-astro-reject = Afvis { $count }
cull-astro-move = Flyt afviste…
cull-astro-chart = { $metric } over { $count ->
        [one] { $count } billede
       *[other] { $count } billeder
    }, median { $median }.
cull-astro-sort-by = Sortér efter { $column }
cull-astro-ascending = stigende
cull-astro-descending = faldende
cull-astro-sorted = Sorteret efter { $column }
cull-astro-reversed = Rækkefølgen vendt
cull-astro-metric = Diagram: { $metric }
cull-astro-k-set = Autoafvisning ved { $k }σ
cull-astro-none = Ingen delbilleder her
cull-astro-same-on = Samme strækning for alle billeder
cull-astro-same-off = Hvert billede strækkes for sig
cull-astro-line-hfd = HFD { $value } px
cull-astro-line-fwhm = FWHM { $value } px
cull-astro-line-fwhm-arcsec = FWHM { $value } px ({ $arcsec }″)
cull-astro-line-ecc = excentricitet { $value }
cull-astro-line-stars = { $count ->
    [one] 1 stjerne
   *[other] { $count } stjerner
}
cull-astro-measuring = måler stjerner…
cull-astro-reject-confirm = { $count ->
    [one] Afvis det foreslåede billede?
   *[other] Afvis de { $count } foreslåede billeder?
}
cull-astro-reject-detail = { $rule }. Fortryd bringer markeringerne tilbage.
cull-astro-reject-ok = Afvis
cull-astro-nothing-to-reject = Intet billede at afvise
cull-astro-rejected = { $count ->
    [one] Afviste 1 billede
   *[other] Afviste { $count } billeder
}
cull-move-rejects-confirm = { $count ->
    [one] Flyt det afviste billede til mappen “{ $folder }”?
   *[other] Flyt de { $count } afviste billeder til mappen “{ $folder }”?
}
cull-move-rejects-detail = Filerne flyttes, de slettes ikke. Fortryd lægger dem tilbage.
cull-move-rejects-ok = Flyt
cull-move-rejects-none = Ingen afviste billeder at flytte
cull-moved-rejects = { $count ->
    [one] Flyttede 1 billede til “{ $folder }”
   *[other] Flyttede { $count } billeder til “{ $folder }”
}
