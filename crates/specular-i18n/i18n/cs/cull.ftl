# machine-drafted, needs native review
# Specular — Režim třídění (DESIGN.md §6.3).

palette-cull = Přejít do režimu třídění nebo jej opustit
cheatsheet-cull = Režim třídění
settings-section-cull = Režim třídění
settings-cull-auto-enter = Třídit složky RAW
settings-cull-auto-enter-help = Otevřít složku s převahou souborů RAW rovnou v režimu třídění.
settings-cull-auto-advance = Po označení další obrázek
settings-cull-auto-advance-help = Po výběru, zamítnutí nebo hodnocení hvězdičkami přejít dál. Štítky a zrušení označení zůstávají na obrázku.
settings-cull-show = Zobrazit u dvojic RAW+JPEG
settings-cull-show-help = Soubor RAW a JPEG se stejným názvem se třídí jako jeden obrázek a označení platí pro oba. JPEG se otevře rychleji.
settings-cull-show-jpeg = JPEG
settings-cull-show-raw = RAW

# machine-drafted, needs native review
settings-cull-bursts = Seskupit série
settings-cull-bursts-help = Snímky pořízené rychle po sobě jedním fotoaparátem se zobrazí jako jeden stoh. S otevře nebo zavře stoh, B ponechá zobrazený obrázek a zbytek stohu odmítne.
settings-cull-burst-gap = Pauza v sérii
settings-cull-burst-gap-help = Nejdelší pauza mezi dvěma snímky jedné série.
settings-cull-watch = Zobrazovat nové soubory
settings-cull-watch-help = Soubory, které se ve složce objeví během třídění (připojený fotoaparát, import), se přidají na konec, jakmile jsou celé zapsané.
settings-cull-follow-newest = Sledovat nejnovější soubor
settings-cull-follow-newest-help = Přejít na každý nový soubor, jakmile dorazí. T to při třídění přepíná.
settings-cull-slot-1 = Složka 1
settings-cull-slot-1-help = Alt+1 zkopíruje obrázek (s jeho RAW nebo JPEG a doprovodnými soubory) do této složky, Alt+Shift+1 ho tam přesune.
settings-cull-slot-1-unset = Nenastaveno
settings-cull-slot-2 = Složka 2
settings-cull-slot-2-help = Alt+2 zkopíruje obrázek (s jeho RAW nebo JPEG a doprovodnými soubory) do této složky, Alt+Shift+2 ho tam přesune.
settings-cull-slot-2-unset = Nenastaveno
settings-cull-slot-3 = Složka 3
settings-cull-slot-3-help = Alt+3 zkopíruje obrázek (s jeho RAW nebo JPEG a doprovodnými soubory) do této složky, Alt+Shift+3 ho tam přesune.
settings-cull-slot-3-unset = Nenastaveno
settings-cull-collisions = Když je název obsazen
settings-cull-collisions-help = Soubory ve složkách se nikdy nenahrazují.
settings-cull-collisions-suffix = Přidat číslo
settings-cull-collisions-skip = Přeskočit obrázek
# machine-drafted, needs native review
# Režim třídění zvolen bez otevřené složky nebo obrázků galerie.
cull-nothing = Není co třídit — nejprve otevřete složku s fotkami
cull-slot-unset = Složka { $slot } není nastavená — zvolte ji v Nastavení → Režim třídění
cull-slot-missing = Složka { $slot } není dostupná: { $reason }
cull-copied = Zkopírováno do { $folder }
cull-moved = Přesunuto do { $folder }
cull-slot-renamed = Uloženo jako { $name } v { $folder }
cull-slot-skipped = { $folder } už obsahuje { $name } — přeskočeno
cull-slot-same = Obrázek už je v { $folder }
cull-slot-failed = { $op ->
    [move] { $file } nelze přesunout
   *[copy] { $file } nelze zkopírovat
}: { $reason }
cull-copy-kept = Ponechané kopie změněné mezitím: { $count }
cull-undo-failed = Nelze vrátit zpět pro { $file }: { $reason }
cull-burst-open = Série { $index } z { $count }
cull-burst-closed = Série: { $count }
cull-burst-toggle = Otevřít nebo zavřít sérii
cull-burst-best = Ponecháno 1, odmítnuto: { $count }
cull-burst-none = Není součástí série
cull-follow-on = Sledování nových souborů
cull-follow-off = Bez sledování nových souborů
cull-new-files = Nové soubory: { $count }
cull-key-copy = Kopírovat do složky 1–3
cull-key-move = Přesunout do složky 1–3
cull-key-burst = Otevřít nebo zavřít sérii
cull-key-best = Ponechat tento, zbytek série odmítnout
cull-key-follow = Sledovat nové soubory
cull-key-subframes = Zobrazit nebo skrýt tabulku dílčích snímků
cull-key-sort = Řadit podle dalšího sloupce
cull-key-sort-reverse = Obrátit pořadí
cull-key-metric = Další veličina v grafu
cull-key-sigma-down = Automatické odmítnutí: snížit k
cull-key-sigma-up = Automatické odmítnutí: zvýšit k
cull-key-auto-reject = Odmítnout navržené snímky
cull-key-move-rejects = Přesunout odmítnuté snímky do podsložky
cull-key-same-stretch = Stejné roztažení pro všechny snímky

## Strip tooltips. machine-drafted, needs native review
cull-tip-burst-open = Otevřít sérii
cull-tip-burst-close = Zavřít sérii
cull-tip-exit = Opustit režim třídění
cull-tip-label = Štítek: { $label }

## Astronomy subframes. machine-drafted, needs native review
cull-astro-panel = Dílčí snímky
cull-astro-title = { $count ->
    [one] 1 dílčí snímek
    [few] { $count } dílčí snímky
    [many] { $count } dílčího snímku
   *[other] { $count } dílčích snímků
}
cull-astro-pending = měří se: { $count }
cull-astro-col-name = Snímek
cull-astro-col-filter = Filtr
cull-astro-col-exposure = Expo.
cull-astro-col-hfd = HFD
cull-astro-col-fwhm = FWHM
cull-astro-col-ecc = Výstř.
cull-astro-col-stars = Hvězdy
cull-astro-col-bg = Pozadí
cull-astro-col-snr = SNR
cull-astro-col-noise = Šum
cull-astro-same-stretch = Stejné roztažení
cull-astro-own-stretch = Vlastní roztažení
cull-astro-suggest-high = { $metric } nad { $value } (medián + { $k }σ): { $count ->
    [one] 1 snímek
    [few] { $count } snímky
    [many] { $count } snímku
   *[other] { $count } snímků
}
cull-astro-suggest-low = { $metric } pod { $value } (medián − { $k }σ): { $count ->
    [one] 1 snímek
    [few] { $count } snímky
    [many] { $count } snímku
   *[other] { $count } snímků
}
cull-astro-suggest-none = Žádný snímek za hranicí { $k }σ
cull-astro-suggest-few = Automatické odmítnutí potřebuje 3 změřené snímky
cull-astro-suggested = navrženo k odmítnutí
cull-astro-k = k { $k }
cull-astro-reject = Odmítnout { $count }
cull-astro-move = Přesunout odmítnuté…
cull-astro-chart = { $metric }, snímky: { $count }, medián { $median }.
cull-astro-sort-by = Řadit podle: { $column }
cull-astro-ascending = vzestupně
cull-astro-descending = sestupně
cull-astro-sorted = Seřazeno podle: { $column }
cull-astro-reversed = Pořadí obráceno
cull-astro-metric = Graf: { $metric }
cull-astro-k-set = Automatické odmítnutí při { $k }σ
cull-astro-none = Žádné dílčí snímky
cull-astro-same-on = Stejné roztažení pro všechny snímky
cull-astro-same-off = Každý snímek roztažen zvlášť
cull-astro-line-hfd = HFD { $value } px
cull-astro-line-fwhm = FWHM { $value } px
cull-astro-line-fwhm-arcsec = FWHM { $value } px ({ $arcsec }″)
cull-astro-line-ecc = výstřednost { $value }
cull-astro-line-stars = { $count ->
    [one] 1 hvězda
    [few] { $count } hvězdy
    [many] { $count } hvězdy
   *[other] { $count } hvězd
}
cull-astro-measuring = měří se hvězdy…
cull-astro-reject-confirm = { $count ->
    [one] Odmítnout navržený snímek?
    [few] Odmítnout { $count } navržené snímky?
    [many] Odmítnout { $count } navrženého snímku?
   *[other] Odmítnout { $count } navržených snímků?
}
cull-astro-reject-detail = { $rule }. Zpět vrátí značky.
cull-astro-reject-ok = Odmítnout
cull-astro-nothing-to-reject = Žádný snímek k odmítnutí
cull-astro-rejected = { $count ->
    [one] Odmítnut 1 snímek
    [few] Odmítnuty { $count } snímky
    [many] Odmítnuto { $count } snímku
   *[other] Odmítnuto { $count } snímků
}
cull-move-rejects-confirm = { $count ->
    [one] Přesunout odmítnutý snímek do složky „{ $folder }“?
    [few] Přesunout { $count } odmítnuté snímky do složky „{ $folder }“?
    [many] Přesunout { $count } odmítnutého snímku do složky „{ $folder }“?
   *[other] Přesunout { $count } odmítnutých snímků do složky „{ $folder }“?
}
cull-move-rejects-detail = Soubory se přesunou, nesmažou. Zpět je vrátí.
cull-move-rejects-ok = Přesunout
cull-move-rejects-none = Žádné odmítnuté snímky k přesunutí
cull-moved-rejects = { $count ->
    [one] Přesunut 1 snímek do „{ $folder }“
    [few] Přesunuty { $count } snímky do „{ $folder }“
    [many] Přesunuto { $count } snímku do „{ $folder }“
   *[other] Přesunuto { $count } snímků do „{ $folder }“
}
