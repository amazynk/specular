# machine-drafted, needs native review
## Astronomy interop: DS9 regions, plate solving, SAMP, HiPS sky surveys.

palette-science-load-regions = Překrýt oblasti DS9…
palette-science-load-regions-help = Nakreslit do obrázku tvary ze souboru oblastí DS9 (.reg).
palette-science-export-regions = Uložit oblasti jako DS9…
palette-science-export-regions-help = Uložit měření a překryté oblasti jako soubor oblastí DS9.
palette-science-edit-regions = Upravit oblasti jako měření
palette-science-edit-regions-help = Převést překryté oblasti na měření, která lze přesouvat a měnit.
palette-science-clear-regions = Odebrat překryté oblasti
palette-science-clear-regions-help = Odebrat oblasti DS9 z obrázku.
palette-science-plate-solve = Astrometrické řešení
palette-science-plate-solve-help = Zjistit nebeské souřadnice snímku pomocí ASTAP nebo astrometry.net. Dalším spuštěním se zastaví.
palette-science-save-plate-solution = Uložit astrometrické řešení
palette-science-save-plate-solution-help = Uložit nalezené nebeské souřadnice vedle obrázku jako soubor .wcs.
palette-science-sky-survey = Přehlídka oblohy na pozadí
palette-science-sky-survey-help = Zobrazit kolem snímku další přehlídku oblohy, nebo žádnou.
palette-science-samp-send-image = Odeslat obrázek aplikacím SAMP
palette-science-samp-send-image-help = Otevřít tento obrázek v DS9, Aladinu a dalších aplikacích na rozbočovači SAMP.
palette-science-samp-send-position = Odeslat polohu aplikacím SAMP
palette-science-samp-send-position-help = Namířit ostatní aplikace na polohu na obloze pod ukazatelem.

disabled-samp-off = Nejprve se připojte k rozbočovači SAMP (Nastavení › Integrace).

settings-integration-samp = Připojit k rozbočovači SAMP
settings-integration-samp-help = Vyměňovat obrázky, tabulky a polohy na obloze s DS9, TOPCAT, Aladinem a dalšími astronomickými aplikacemi. Používá se jen rozbočovač na tomto počítači.
settings-integration-astap-path = Program ASTAP
settings-integration-astap-path-help = Astrometrický řešič ASTAP, pokud se nenajde sám.
settings-integration-solve-field-path = Program astrometry.net
settings-integration-solve-field-path-help = Program solve-field z astrometry.net, pokud se nenajde sám.

astro-regions-filter = Oblasti DS9
astro-regions-loaded = { $name }: { $count ->
        [one] { $count } oblast.
        [few] { $count } oblasti.
        [many] { $count } oblasti.
       *[other] { $count } oblastí.
    }
astro-regions-refused = Nepřečtené řádky: { $count }. První: řádek { $line }, { $reason }.
astro-regions-approximate = Tvary panda nakreslené jako vnitřní a vnější okraj: { $count }.
astro-regions-none-here = { $name }: žádná oblast nepadá do tohoto snímku.
astro-regions-failed = { $name } nelze přečíst: { $reason }
astro-regions-cleared = Oblasti odebrány.
astro-regions-nothing = Žádné oblasti ani měření k uložení.
astro-regions-saved = Uloženo: { $name }.
astro-regions-skipped = Vynechané tvary mimo nebeské souřadnice: { $count }.
astro-regions-save-failed = Oblasti nelze uložit: { $reason }
astro-regions-edited = Oblasti převedené na měření: { $count }.
astro-regions-no-wcs = Oblasti na obloze vyžadují snímek s nebeskými souřadnicemi.

astro-solve-no-solver = Nenalezen žádný astrometrický řešič. Nainstalujte ASTAP nebo astrometry.net, nebo jej vyberte v Nastavení › Integrace.
astro-solve-running = Řešení pomocí { $solver }… { $seconds } s
astro-solve-stop = Zastavit
astro-solve-stopped = Astrometrické řešení zastaveno.
astro-solve-done = Vyřešeno: { $ra }, { $dec }, { $scale } na pixel.
astro-solve-failed = Astrometrické řešení selhalo: { $reason }
astro-solve-not-local = Řešit lze jen obrázky ve složce na tomto počítači.
astro-solve-none = Pro tento snímek zatím není astrometrické řešení.
astro-solve-saved = Uloženo: { $name }. Samotný obrázek se nezměnil.
astro-solve-sidecar = Nebeské souřadnice z { $name }.

astro-samp-connected = Připojeno k rozbočovači SAMP.
astro-samp-no-hub = SAMP: { $reason }. Nový pokus, až se spustí rozbočovač.
astro-samp-lost = Rozbočovač SAMP se zastavil. Nový pokus, až se nějaký spustí.
astro-samp-sent = Odesláno aplikacím: { $count }.
astro-samp-nobody = Žádná aplikace na rozbočovači tuto zprávu nepřijímá.
astro-samp-failed = SAMP: { $reason }
astro-samp-refused = SAMP: { $url } není na tomto počítači, proto se neotevře.
astro-samp-pointed = Namířeno na { $ra }, { $dec }.
astro-samp-off-image = { $ra }, { $dec } není v tomto snímku.
astro-samp-no-table = SAMP: tato tabulka zde není překrytá.
astro-samp-rows = Zvýrazněné zdroje katalogu: { $count }.

astro-survey-off = Přehlídka oblohy vypnuta.
astro-survey-on = Přehlídka oblohy: { $name }. { $credit }
astro-survey-failed = { $name } nelze zobrazit: { $reason }
astro-survey-credit = { $name }: { $credit }

net-what-sky-survey = Dlaždice přehlídky oblohy z { $host }
