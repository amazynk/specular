# machine-drafted, needs native review
## Astronomy interop: DS9 regions, plate solving, SAMP, HiPS sky surveys.

palette-science-load-regions = Læg DS9-regioner over billedet…
palette-science-load-regions-help = Tegn formerne fra en DS9-regionsfil (.reg) på billedet.
palette-science-export-regions = Gem regioner som DS9…
palette-science-export-regions-help = Gem målingerne og de overlagte regioner som en DS9-regionsfil.
palette-science-edit-regions = Redigér regioner som målinger
palette-science-edit-regions-help = Gør de overlagte regioner til målinger, som du kan flytte og ændre.
palette-science-clear-regions = Fjern de overlagte regioner
palette-science-clear-regions-help = Fjern DS9-regionerne fra billedet.
palette-science-plate-solve = Astrometrisk løsning
palette-science-plate-solve-help = Find billedets himmelkoordinater med ASTAP eller astrometry.net. Kør igen for at stoppe.
palette-science-save-plate-solution = Gem den astrometriske løsning
palette-science-save-plate-solution-help = Gem de fundne himmelkoordinater ved siden af billedet som en .wcs-fil.
palette-science-sky-survey = Himmelkortlægning som baggrund
palette-science-sky-survey-help = Vis den næste himmelkortlægning omkring billedet, eller ingen.
palette-science-samp-send-image = Send billedet til SAMP-programmer
palette-science-samp-send-image-help = Åbn billedet i DS9, Aladin og de andre programmer på SAMP-hubben.
palette-science-samp-send-position = Send positionen til SAMP-programmer
palette-science-samp-send-position-help = Ret de andre programmer mod himmelpositionen under markøren.

disabled-samp-off = Opret først forbindelse til en SAMP-hub (Indstillinger › Integration).

settings-integration-samp = Opret forbindelse til en SAMP-hub
settings-integration-samp-help = Udveksl billeder, tabeller og himmelpositioner med DS9, TOPCAT, Aladin og andre astronomiprogrammer. Kun en hub på denne computer bruges.
settings-integration-astap-path = ASTAP-programmet
settings-integration-astap-path-help = Den astrometriske løser ASTAP, når den ikke findes af sig selv.
settings-integration-solve-field-path = astrometry.net-programmet
settings-integration-solve-field-path-help = Programmet solve-field fra astrometry.net, når det ikke findes af sig selv.

astro-regions-filter = DS9-regioner
astro-regions-loaded = { $name }: { $count ->
        [one] { $count } region.
       *[other] { $count } regioner.
    }
astro-regions-refused = { $count ->
        [one] { $count } linje blev ikke læst.
       *[other] { $count } linjer blev ikke læst.
    } Første: linje { $line }, { $reason }.
astro-regions-approximate = { $count ->
        [one] { $count } panda tegnet som sine indre og ydre kanter.
       *[other] { $count } pandaer tegnet som deres indre og ydre kanter.
    }
astro-regions-none-here = { $name }: ingen region falder på dette billede.
astro-regions-failed = { $name } kunne ikke læses: { $reason }
astro-regions-cleared = Regionerne er fjernet.
astro-regions-nothing = Ingen regioner eller målinger at gemme.
astro-regions-saved = { $name } er gemt.
astro-regions-skipped = { $count ->
        [one] { $count } form uden for himmelkoordinaterne blev udeladt.
       *[other] { $count } former uden for himmelkoordinaterne blev udeladt.
    }
astro-regions-save-failed = Regionerne kunne ikke gemmes: { $reason }
astro-regions-edited = { $count ->
        [one] { $count } region er nu en måling.
       *[other] { $count } regioner er nu målinger.
    }
astro-regions-no-wcs = Himmelregioner kræver et billede med himmelkoordinater.

astro-solve-no-solver = Ingen astrometrisk løser fundet. Installér ASTAP eller astrometry.net, eller vælg en under Indstillinger › Integration.
astro-solve-running = Løser med { $solver }… { $seconds } s
astro-solve-stop = Stop
astro-solve-stopped = Den astrometriske løsning er stoppet.
astro-solve-done = Løst: { $ra }, { $dec }, { $scale } pr. pixel.
astro-solve-failed = Den astrometriske løsning mislykkedes: { $reason }
astro-solve-not-local = Kun billeder i en mappe på denne computer kan løses.
astro-solve-none = Ingen astrometrisk løsning for dette billede endnu.
astro-solve-saved = { $name } er gemt. Selve billedet er uændret.
astro-solve-sidecar = Himmelkoordinater fra { $name }.

astro-samp-connected = Forbundet til SAMP-hubben.
astro-samp-no-hub = SAMP: { $reason }. Prøver igen, når en hub starter.
astro-samp-lost = SAMP-hubben stoppede. Prøver igen, når en starter.
astro-samp-sent = { $count ->
        [one] Sendt til { $count } program.
       *[other] Sendt til { $count } programmer.
    }
astro-samp-nobody = Intet program på hubben lytter efter dette.
astro-samp-failed = SAMP: { $reason }
astro-samp-refused = SAMP: { $url } ligger ikke på denne computer, så den åbnes ikke.
astro-samp-pointed = Rettet mod { $ra }, { $dec }.
astro-samp-off-image = { $ra }, { $dec } ligger ikke på dette billede.
astro-samp-no-table = SAMP: den tabel er ikke lagt over her.
astro-samp-rows = { $count ->
        [one] { $count } katalogkilde fremhævet.
       *[other] { $count } katalogkilder fremhævet.
    }

astro-survey-off = Himmelkortlægning slået fra.
astro-survey-on = Himmelkortlægning: { $name }. { $credit }
astro-survey-failed = { $name } kan ikke vises: { $reason }
astro-survey-credit = { $name }: { $credit }

net-what-sky-survey = Fliser fra himmelkortlægning fra { $host }
