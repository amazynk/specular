# machine-drafted, needs native review
## Astronomy interop: DS9 regions, plate solving, SAMP, HiPS sky surveys.

palette-science-load-regions = Lägg DS9-regioner över bilden…
palette-science-load-regions-help = Rita formerna från en DS9-regionfil (.reg) på bilden.
palette-science-export-regions = Spara regioner som DS9…
palette-science-export-regions-help = Spara mätningarna och de överlagda regionerna som en DS9-regionfil.
palette-science-edit-regions = Redigera regioner som mätningar
palette-science-edit-regions-help = Gör om de överlagda regionerna till mätningar som du kan flytta och ändra.
palette-science-clear-regions = Ta bort de överlagda regionerna
palette-science-clear-regions-help = Ta bort DS9-regionerna från bilden.
palette-science-plate-solve = Astrometrisk lösning
palette-science-plate-solve-help = Hitta bildens himmelskoordinater med ASTAP eller astrometry.net. Kör igen för att stoppa.
palette-science-save-plate-solution = Spara den astrometriska lösningen
palette-science-save-plate-solution-help = Spara de funna himmelskoordinaterna bredvid bilden, som en .wcs-fil.
palette-science-sky-survey = Himmelskartläggning som bakgrund
palette-science-sky-survey-help = Visa nästa himmelskartläggning runt bilden, eller ingen.
palette-science-samp-send-image = Skicka bilden till SAMP-program
palette-science-samp-send-image-help = Öppna bilden i DS9, Aladin och de andra programmen på SAMP-hubben.
palette-science-samp-send-position = Skicka positionen till SAMP-program
palette-science-samp-send-position-help = Rikta de andra programmen mot himmelspositionen under pekaren.

disabled-samp-off = Anslut först till en SAMP-hubb (Inställningar › Integrering).

settings-integration-samp = Anslut till en SAMP-hubb
settings-integration-samp-help = Utbyt bilder, tabeller och himmelspositioner med DS9, TOPCAT, Aladin och andra astronomiprogram. Bara en hubb på den här datorn används.
settings-integration-astap-path = ASTAP-programmet
settings-integration-astap-path-help = Den astrometriska lösaren ASTAP, när den inte hittas av sig själv.
settings-integration-solve-field-path = astrometry.net-programmet
settings-integration-solve-field-path-help = Programmet solve-field från astrometry.net, när det inte hittas av sig själv.

astro-regions-filter = DS9-regioner
astro-regions-loaded = { $name }: { $count ->
        [one] { $count } region.
       *[other] { $count } regioner.
    }
astro-regions-refused = { $count ->
        [one] { $count } rad lästes inte.
       *[other] { $count } rader lästes inte.
    } Första: rad { $line }, { $reason }.
astro-regions-approximate = { $count ->
        [one] { $count } panda ritad som sin inre och yttre kant.
       *[other] { $count } pandor ritade som sina inre och yttre kanter.
    }
astro-regions-none-here = { $name }: ingen region hamnar på den här bilden.
astro-regions-failed = { $name } kunde inte läsas: { $reason }
astro-regions-cleared = Regionerna har tagits bort.
astro-regions-nothing = Inga regioner eller mätningar att spara.
astro-regions-saved = { $name } har sparats.
astro-regions-skipped = { $count ->
        [one] { $count } form utanför himmelskoordinaterna utelämnades.
       *[other] { $count } former utanför himmelskoordinaterna utelämnades.
    }
astro-regions-save-failed = Regionerna kunde inte sparas: { $reason }
astro-regions-edited = { $count ->
        [one] { $count } region är nu en mätning.
       *[other] { $count } regioner är nu mätningar.
    }
astro-regions-no-wcs = Himmelsregioner kräver en bild med himmelskoordinater.

astro-solve-no-solver = Ingen astrometrisk lösare hittades. Installera ASTAP eller astrometry.net, eller välj en under Inställningar › Integrering.
astro-solve-running = Löser med { $solver }… { $seconds } s
astro-solve-stop = Stoppa
astro-solve-stopped = Den astrometriska lösningen stoppades.
astro-solve-done = Löst: { $ra }, { $dec }, { $scale } per pixel.
astro-solve-failed = Den astrometriska lösningen misslyckades: { $reason }
astro-solve-not-local = Bara bilder i en mapp på den här datorn kan lösas.
astro-solve-none = Ingen astrometrisk lösning för den här bilden än.
astro-solve-saved = { $name } har sparats. Själva bilden är oförändrad.
astro-solve-sidecar = Himmelskoordinater från { $name }.

astro-samp-connected = Ansluten till SAMP-hubben.
astro-samp-no-hub = SAMP: { $reason }. Försöker igen när en hubb startar.
astro-samp-lost = SAMP-hubben stoppades. Försöker igen när en startar.
astro-samp-sent = { $count ->
        [one] Skickat till { $count } program.
       *[other] Skickat till { $count } program.
    }
astro-samp-nobody = Inget program på hubben lyssnar efter detta.
astro-samp-failed = SAMP: { $reason }
astro-samp-refused = SAMP: { $url } finns inte på den här datorn, så den öppnas inte.
astro-samp-pointed = Riktat mot { $ra }, { $dec }.
astro-samp-off-image = { $ra }, { $dec } ligger inte på den här bilden.
astro-samp-no-table = SAMP: den tabellen är inte överlagd här.
astro-samp-rows = { $count ->
        [one] { $count } katalogkälla markerad.
       *[other] { $count } katalogkällor markerade.
    }

astro-survey-off = Himmelskartläggning av.
astro-survey-on = Himmelskartläggning: { $name }. { $credit }
astro-survey-failed = { $name } kan inte visas: { $reason }
astro-survey-credit = { $name }: { $credit }

net-what-sky-survey = Rutor från himmelskartläggning från { $host }
